//! Browser transport: one binary WebSocket is one ordered TCP byte stream.
use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{BinaryType, CloseEvent, MessageEvent, WebSocket};

const MAX_BUFFERED: usize = 4 * 1024 * 1024;

const MAX_PENDING_EVENTS: usize = 2048;

thread_local! {
    static GATEWAY: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Set the WebSocket-to-TCP gateway before logging in. An HTTPS page requires
/// `wss:`. There is deliberately no default public gateway.

pub fn configure_websocket_gateway(endpoint: &str) -> io::Result<()> {
    let url = web_sys::Url::new(endpoint).map_err(js_error)?;

    if !matches!(url.protocol().as_str(), "ws:" | "wss:")
        || url.hostname().is_empty()
        || !url.username().is_empty()
        || !url.password().is_empty()
        || !url.hash().is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gateway must be an absolute ws:// or wss:// URL without credentials or a fragment",
        ));
    }

    GATEWAY.with(|gateway| *gateway.borrow_mut() = Some(url.href()));

    Ok(())
}

fn js_error(value: JsValue) -> io::Error {
    io::Error::other(value.as_string().unwrap_or_else(|| format!("{value:?}")))
}

struct Shared {
    queue: RefCell<VecDeque<Vec<u8>>>,
    queued_bytes: Cell<usize>,
    pending_events: Cell<usize>,
    closed_event: Cell<bool>,
    decoder: RefCell<Decoder>,
    tx: std::sync::mpsc::Sender<Event>,
    diagnostics: Arc<Diagnostics>,
    observer: Observer,
    id: u64,
    timer: Cell<Option<i32>>,
}

impl Shared {
    fn clear_timer(&self) {
        if let Some(timer) = self.timer.take()
            && let Some(window) = web_sys::window()
        {
            window.clear_timeout_with_handle(timer);
        }
    }

    fn close(&self, socket: &WebSocket, operation: &'static str, detail: String) {
        self.clear_timer();

        self.queue.borrow_mut().clear();

        self.queued_bytes.set(0);

        let report = self
            .diagnostics
            .report(operation, None, Some(detail), None, true);

        if !self.closed_event.replace(true) {
            let _ = self.tx.send(Event::Closed(Some(report.to_string())));
        }

        let _ = socket.close();
    }

    fn event(&self, socket: &WebSocket, event: Event) -> bool {
        if self.pending_events.get() >= MAX_PENDING_EVENTS {
            self.close(
                socket,
                "receive_backpressure",
                "too many unprocessed server packets".into(),
            );

            return false;
        }

        self.pending_events.set(self.pending_events.get() + 1);

        self.tx.send(event).is_ok()
    }
}

pub(super) struct Socket {
    socket: WebSocket,
    shared: Rc<Shared>,
    _open: Closure<dyn FnMut(web_sys::Event)>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _error: Closure<dyn FnMut(web_sys::Event)>,
    _close: Closure<dyn FnMut(CloseEvent)>,
    _timeout: Closure<dyn FnMut()>,
}

impl Socket {
    pub(super) fn shutdown(&self, _how: std::net::Shutdown) -> io::Result<()> {
        self.shared.clear_timer();

        self.socket.close().map_err(js_error)
    }

    pub(super) fn consumed_event(&self) {
        self.shared
            .pending_events
            .set(self.shared.pending_events.get().saturating_sub(1));
    }
}

impl io::Write for Socket {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.shared.diagnostics.terminal().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "WebSocket is closed",
            ));
        }

        let pending = self.shared.queued_bytes.get() + self.socket.buffered_amount() as usize;

        if pending.saturating_add(bytes.len()) > MAX_BUFFERED {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "WebSocket send buffer exceeded 4 MiB; connection closed to preserve packet ordering",
            ));
        }

        match self.socket.ready_state() {
            WebSocket::CONNECTING => {
                self.shared.queue.borrow_mut().push_back(bytes.to_vec());

                self.shared
                    .queued_bytes
                    .set(self.shared.queued_bytes.get() + bytes.len());
            }
            WebSocket::OPEN => self.socket.send_with_u8_array(bytes).map_err(js_error)?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "WebSocket is closed",
                ));
            }
        }

        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        self.shared.clear_timer();

        self.socket.set_onopen(None);

        self.socket.set_onmessage(None);

        self.socket.set_onerror(None);

        self.socket.set_onclose(None);

        let _ = self.socket.close();
    }
}

impl Link {
    pub(super) fn open(
        address: &str,
        port: u16,
        timeout: Duration,
        observer: Observer,
    ) -> io::Result<Self> {
        let endpoint = GATEWAY
            .with(|gateway| gateway.borrow().clone())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotConnected,
                    "no browser game gateway is configured; configure a WebSocket-to-TCP relay",
                )
            })?;

        let url = web_sys::Url::new(&endpoint).map_err(js_error)?;

        url.search_params().set("host", address);

        url.search_params().set("port", &port.to_string());

        let socket = WebSocket::new(&url.href()).map_err(js_error)?;

        socket.set_binary_type(BinaryType::Arraybuffer);

        let id = NEXT_LINK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let peer = format!("{address}:{port}");

        let diagnostics = Arc::new(Diagnostics::new(id, peer.clone(), observer.clone()));

        let (tx, inbound) = std::sync::mpsc::channel();

        let shared = Rc::new(Shared {
            queue: RefCell::new(VecDeque::from([HELLO.to_vec()])),
            queued_bytes: Cell::new(HELLO.len()),
            pending_events: Cell::new(0),
            closed_event: Cell::new(false),
            decoder: RefCell::new(Decoder::new()),
            tx,
            diagnostics: diagnostics.clone(),
            observer,
            id,
            timer: Cell::new(None),
        });

        // Count queued bytes consistently with writes made while CONNECTING.
        diagnostics.bytes(true, HELLO.len());

        let state = shared.clone();

        let ws = socket.clone();

        let onopen = Closure::new(move |_: web_sys::Event| {
            state.clear_timer();

            if state.diagnostics.terminal().is_some() {
                let _ = ws.close();

                return;
            }

            loop {
                let Some(bytes) = state.queue.borrow_mut().pop_front() else {
                    break;
                };

                if let Err(error) = ws.send_with_u8_array(&bytes) {
                    state.close(&ws, "open_write", js_error(error).to_string());

                    return;
                }

                state
                    .queued_bytes
                    .set(state.queued_bytes.get().saturating_sub(bytes.len()));
            }

            if let Some(observer) = &state.observer {
                observer.event(state.id, "connected_hello_sent");
            }
        });

        socket.set_onopen(Some(onopen.as_ref().unchecked_ref()));

        let state = shared.clone();

        let ws = socket.clone();

        let onmessage = Closure::new(move |event: MessageEvent| {
            if state.diagnostics.terminal().is_some() {
                return;
            }

            let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                state.close(
                    &ws,
                    "websocket_frame",
                    "gateway sent a non-binary WebSocket message".into(),
                );

                return;
            };

            if buffer.byte_length() as usize > MAX_BUFFERED {
                state.close(
                    &ws,
                    "receive_backpressure",
                    "gateway frame exceeded 4 MiB".into(),
                );

                return;
            }

            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();

            state.diagnostics.bytes(false, bytes.len());

            let (packets, error) = {
                let mut decoder = state.decoder.borrow_mut();

                decoder.feed(&bytes);

                let result = decoder.drain();

                state.diagnostics.pending(decoder.pending());

                result
            };

            for packet in packets {
                state.diagnostics.packet(state.diagnostics.metadata(
                    false,
                    packet.header.opcode,
                    packet.header.size as usize,
                    packet.header.tick,
                ));

                if let Some(observer) = &state.observer {
                    observer.packet(state.id, false, &packet);
                }

                if !packet.checksum_ok {
                    state.diagnostics.report(
                        "checksum",
                        None,
                        Some(format!(
                            "discarded checksum-invalid opcode {:#06x}",
                            packet.header.opcode
                        )),
                        None,
                        false,
                    );

                    continue;
                }

                let tick = packet.header.tick;

                if !state.event(
                    &ws,
                    Event::Message {
                        message: Box::new(server::classify(packet)),
                        tick,
                    },
                ) {
                    return;
                }
            }

            if let Some(error) = error {
                state
                    .diagnostics
                    .report("decode", None, Some(error.to_string()), None, false);

                state.event(&ws, Event::Decode(error));
            }
        });

        socket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        let state = shared.clone();

        let ws = socket.clone();

        let onerror = Closure::new(move |_: web_sys::Event| {
            state.close(&ws, "websocket_error",
                "browser could not reach the game gateway; check relay availability, TLS, and origin policy".into());
        });

        socket.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        let state = shared.clone();

        let ws = socket.clone();

        let onclose = Closure::new(move |event: CloseEvent| {
            state.close(
                &ws,
                "websocket_closed",
                format!("code {}: {}", event.code(), event.reason()),
            );
        });

        socket.set_onclose(Some(onclose.as_ref().unchecked_ref()));

        let state = shared.clone();

        let ws = socket.clone();

        let ontimeout = Closure::new(move || {
            state.close(
                &ws,
                "connect_timeout",
                "game gateway connection timed out".into(),
            );
        });

        // Own handlers before the fallible timer registration so any error
        // detaches callbacks and closes the socket before closures are dropped.
        let socket = Socket {
            socket,
            shared,
            _open: onopen,
            _message: onmessage,
            _error: onerror,
            _close: onclose,
            _timeout: ontimeout,
        };

        let window =
            web_sys::window().ok_or_else(|| io::Error::other("browser Window is unavailable"))?;

        let timer = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                socket._timeout.as_ref().unchecked_ref(),
                timeout.as_millis().min(i32::MAX as u128) as i32,
            )
            .map_err(js_error)?;

        socket.shared.timer.set(Some(timer));

        Ok(Link {
            id,
            inbound,
            peer,
            diagnostics,
            socket,
        })
    }
}

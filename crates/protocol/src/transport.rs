#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;

use crate::client;
use crate::codec::{self, DecodeError, Decoder, HELLO, Packet};
use crate::keys::{self, Schedule};
use crate::rng::Lcg;
use crate::server::{self, ServerMessage};

#[cfg(not(target_arch = "wasm32"))]
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
use web::Socket as TcpStream;
#[cfg(target_arch = "wasm32")]
pub use web::configure_websocket_gateway;

#[cfg(not(target_arch = "wasm32"))]
type Peer = SocketAddr;

#[cfg(target_arch = "wasm32")]
type Peer = String;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

mod diagnostics;

#[cfg(not(target_arch = "wasm32"))]
mod outbound;

use diagnostics::{Diagnostics, write_counted};
pub use diagnostics::{DisconnectReport, FailedWrite, IoFailure, PacketMetadata};

pub const PORT: u16 = 8281;

#[cfg(not(target_arch = "wasm32"))]
const READ_CHUNK: usize = 128 * 1024;

#[cfg(not(target_arch = "wasm32"))]
const MAX_PENDING_EVENTS: usize = 2_048;

pub trait TrafficObserver: Send + Sync {
    fn packet(&self, connection: u64, outgoing: bool, packet: &Packet);

    fn event(&self, connection: u64, event: &str);

    fn diagnostic(&self, _report: &DisconnectReport) {}

    fn error(&self) -> Option<String> {
        None
    }
}

type Observer = Option<std::sync::Arc<dyn TrafficObserver>>;

static NEXT_LINK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct Link {
    id: u64,
    socket: TcpStream,
    #[cfg(not(target_arch = "wasm32"))]
    writer: outbound::Writer,
    inbound: Receiver<Event>,
    peer: Peer,
    diagnostics: Arc<Diagnostics>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Link {
    fn open(
        address: &str,
        port: u16,
        timeout: Duration,
        observer: Observer,
    ) -> std::io::Result<Link> {
        let id = NEXT_LINK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let diagnostics = Arc::new(Diagnostics::new(
            id,
            format!("{address}:{port}"),
            observer.clone(),
        ));

        let failed = |operation, error: std::io::Error| {
            diagnostics.report(operation, Some(&error), None, None, true);

            error
        };

        let endpoint = (address, port)
            .to_socket_addrs()
            .map_err(|error| failed("resolve", error))?
            .next()
            .ok_or_else(|| {
                failed(
                    "resolve",
                    std::io::Error::other(format!("{address}:{port} resolved to no address")),
                )
            })?;

        let socket =
            TcpStream::connect_timeout(&endpoint, timeout).map_err(|e| failed("connect", e))?;

        let options = socket2::SockRef::from(&socket);

        for result in [
            options.set_send_buffer_size(65_536),
            options.set_recv_buffer_size(65_536),
            options.set_tcp_keepalive(
                &socket2::TcpKeepalive::new()
                    .with_time(Duration::from_secs(30))
                    .with_interval(Duration::from_secs(10)),
            ),
        ] {
            if let Err(error) = result {
                tracing::warn!(% error, "could not apply socket option");
            }
        }

        let peer = socket.peer_addr().map_err(|e| failed("peer_address", e))?;

        diagnostics.peer(peer);

        write_counted(&mut &socket, &HELLO, |n| diagnostics.bytes(true, n))
            .map_err(|(e, _)| failed("hello_write", e))?;

        if let Some(observer) = &observer {
            observer.event(id, "connected_hello_sent");
        }

        let reader = socket.try_clone().map_err(|e| failed("reader_clone", e))?;

        let writer = outbound::Writer::new(
            socket.try_clone().map_err(|e| failed("writer_clone", e))?,
            id,
            observer.clone(),
            diagnostics.clone(),
        )
        .map_err(|e| failed("writer_start", e))?;

        let (tx, inbound) = std::sync::mpsc::sync_channel(MAX_PENDING_EVENTS);

        let reader_diagnostics = diagnostics.clone();

        std::thread::Builder::new()
            .name(format!("nwyd-net {peer}"))
            .spawn(move || read_loop(reader, tx, id, observer, reader_diagnostics))
            .map_err(|e| failed("reader_spawn", e))?;

        Ok(Link {
            id,
            socket,
            writer,
            inbound,
            peer,
            diagnostics,
        })
    }
}

#[derive(Debug)]
pub enum Event {
    Message {
        message: Box<ServerMessage>,
        tick: u32,
    },
    Decode(DecodeError),
    Closed(Option<String>),
}

pub struct Connection {
    observer: Observer,
    link_id: u64,
    socket: TcpStream,
    #[cfg(not(target_arch = "wasm32"))]
    writer: outbound::Writer,
    inbound: Receiver<Event>,
    schedule: Schedule,
    client_id: u16,
    rng: Lcg,
    peer: Peer,
    sent: u64,
    received: u64,
    closed: bool,
    diagnostics: Arc<Diagnostics>,
    pub send_state: crate::send_state::SharedSender,
}

impl Connection {
    pub fn connect(
        address: &str,
        port: u16,
        timeout: Duration,
        seed: u32,
    ) -> std::io::Result<Connection> {
        Self::connect_observed(address, port, timeout, seed, None)
    }

    pub fn connect_observed(
        address: &str,
        port: u16,
        timeout: Duration,
        seed: u32,
        observer: Observer,
    ) -> std::io::Result<Connection> {
        let Link {
            id,
            socket,
            #[cfg(not(target_arch = "wasm32"))]
            writer,
            inbound,
            peer,
            diagnostics,
        } = Link::open(address, port, timeout, observer.clone())?;

        Ok(Connection {
            observer,
            link_id: id,
            socket,
            #[cfg(not(target_arch = "wasm32"))]
            writer,
            inbound,
            send_state: Default::default(),
            schedule: Schedule::default(),
            client_id: 0,
            rng: Lcg::new(seed),
            peer,
            sent: 0,
            received: 0,
            closed: false,
            diagnostics,
        })
    }

    pub fn reconnect(
        &mut self,
        address: &str,
        port: u16,
        timeout: Duration,
    ) -> std::io::Result<()> {
        self.diagnostics.report("replaced", None, None, None, true);

        self.closed = true;

        #[cfg(not(target_arch = "wasm32"))]
        let drained = {
            let (tx, inbound) = std::sync::mpsc::channel();
            drop(tx);
            self.inbound = inbound;
            self.writer.finish()
        };

        let _ = self.socket.shutdown(std::net::Shutdown::Both);

        if let Some(observer) = &self.observer {
            observer.event(self.link_id, "replaced");
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Err(error) = drained {
            tracing::warn!(%error, "old connection stopped before queued packets finished");
        }

        let Link {
            id,
            socket,
            #[cfg(not(target_arch = "wasm32"))]
            writer,
            inbound,
            peer,
            diagnostics,
        } = Link::open(address, port, timeout, self.observer.clone())?;

        let old_socket = std::mem::replace(&mut self.socket, socket);

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.writer = writer;
        }

        self.diagnostics = diagnostics;

        self.link_id = id;

        self.inbound = inbound;

        self.peer = peer;

        self.closed = false;

        drop(old_socket);

        Ok(())
    }

    pub fn id(&self) -> u64 {
        self.link_id
    }

    pub fn disconnect_report(&self) -> Option<DisconnectReport> {
        self.diagnostics.terminal()
    }

    pub fn peer(&self) -> Peer {
        self.peer.clone()
    }

    pub fn client_id(&self) -> u16 {
        self.client_id
    }

    pub fn counters(&self) -> (u64, u64) {
        (self.sent, self.received)
    }

    pub fn is_closed(&self) -> bool {
        self.closed || self.diagnostics.terminal().is_some()
    }

    pub fn schedule_is_live(&self) -> bool {
        self.schedule.active()
    }

    pub fn send(&mut self, body: &mut [u8], tick: u32) -> std::io::Result<()> {
        self.send_with_client_id(body, self.client_id, tick)
    }

    pub fn send_with_client_id(
        &mut self,
        body: &mut [u8],
        client_id: u16,
        tick: u32,
    ) -> std::io::Result<()> {
        if let Some(report) = self.diagnostics.terminal() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                report.to_string(),
            ));
        }

        if !(crate::header::MIN_SIZE..=crate::header::MAX_SIZE).contains(&body.len()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid outgoing packet size",
            ));
        }

        #[cfg(not(target_arch = "wasm32"))]
        let reservation = match self.writer.reserve(body.len()) {
            Ok(reservation) => reservation,
            Err(error) => {
                self.diagnostics
                    .report("send_queue", Some(&error), None, None, true);
                let _ = self.socket.shutdown(std::net::Shutdown::Both);
                return Err(error);
            }
        };

        let tick = self
            .send_state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .prepare(body, tick);

        let metadata = self.diagnostics.metadata(
            true,
            crate::header::opcode_of(body).unwrap(),
            body.len(),
            tick,
        );

        let key = self
            .schedule
            .next_key()
            .unwrap_or_else(|| (self.rng.next() & 0xFF) as u8);

        codec::seal(body, key, client_id, tick);

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Err(error) = reservation.send(body, metadata) {
                self.diagnostics
                    .report("send_queue", Some(&error), None, None, true);
                let _ = self.socket.shutdown(std::net::Shutdown::Both);
                return Err(error);
            }
        }

        #[cfg(target_arch = "wasm32")]
        if let Err((error, bytes_written)) =
            write_counted(&mut self.socket, body, |n| self.diagnostics.bytes(true, n))
        {
            self.diagnostics.report(
                "write",
                Some(&error),
                None,
                Some(FailedWrite {
                    packet: metadata,
                    bytes_written,
                }),
                true,
            );

            let _ = self.socket.shutdown(std::net::Shutdown::Both);

            return Err(error);
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.diagnostics.packet(metadata);

            if let Some(observer) = &self.observer {
                let mut decoder = Decoder::new();
                decoder.feed(body);
                for packet in decoder.drain().0 {
                    observer.packet(self.link_id, true, &packet);
                }
            }
        }

        self.sent += 1;

        Ok(())
    }

    pub fn poll(&mut self) -> Vec<Event> {
        let events = self.drain_events();

        for event in &events {
            if let Event::Message { message, .. } = event {
                self.apply_message_state(message);
            }
        }

        events
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();

        loop {
            match self.inbound.try_recv() {
                Ok(event) => {
                    #[cfg(target_arch = "wasm32")]
                    self.socket.consumed_event();

                    match &event {
                        Event::Message { .. } => {
                            self.received += 1;
                        }
                        Event::Closed(_) => self.closed = true,
                        Event::Decode(_) => {}
                    }

                    events.push(event);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.closed {
                        self.closed = true;

                        let report =
                            self.diagnostics
                                .report("reader_stopped", None, None, None, true);

                        events.push(Event::Closed(Some(report.to_string())));
                    }

                    break;
                }
            }
        }

        events
    }

    pub fn apply_message_state(&mut self, message: &ServerMessage) {
        match message {
            ServerMessage::LoginAccepted(accept) => self.schedule.install(accept.key),
            ServerMessage::EnterWorld(world) => self.client_id = world.client_id,
            _ => {}
        }
    }

    pub fn keepalive(&mut self, tick: u32) -> std::io::Result<()> {
        self.send(&mut client::keepalive(), tick)
    }

    pub fn set_dynamic_key(&mut self, value: u32) {
        self.schedule.dynamic = value;
    }

    pub fn quit(&mut self, tick: u32) -> std::io::Result<()> {
        self.send(&mut client::quit(), tick)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn begin_close(&mut self) {
        self.diagnostics
            .report("local_close", None, None, None, true);
        self.closed = true;
        let (tx, inbound) = std::sync::mpsc::channel();
        drop(tx);
        self.inbound = inbound;
        self.writer.close();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn close_finished(&self) -> bool {
        self.writer.is_finished()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn close(&mut self) -> std::io::Result<()> {
        self.begin_close();
        self.writer.finish()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.diagnostics
            .report("local_close", None, None, None, true);

        #[cfg(target_arch = "wasm32")]
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_loop(
    mut socket: impl Read,
    tx: std::sync::mpsc::SyncSender<Event>,
    id: u64,
    observer: Observer,
    diagnostics: Arc<Diagnostics>,
) {
    let mut decoder = Decoder::new();

    let mut chunk = vec![0u8; READ_CHUNK];

    loop {
        let read = match socket.read(&mut chunk) {
            Ok(0) => {
                let report = diagnostics.report("peer_eof", None, None, None, true);

                let _ = tx.send(Event::Closed(Some(report.to_string())));

                return;
            }
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let report = diagnostics.report("read", Some(&error), None, None, true);

                let _ = tx.send(Event::Closed(Some(report.to_string())));

                return;
            }
        };

        diagnostics.bytes(false, read);

        decoder.feed(&chunk[..read]);

        let (packets, error) = decoder.drain();

        diagnostics.pending(decoder.pending());

        for packet in packets {
            diagnostics.packet(diagnostics.metadata(
                false,
                packet.header.opcode,
                packet.header.size as usize,
                packet.header.tick,
            ));

            if let Some(observer) = &observer {
                observer.packet(id, false, &packet);
            }

            if !packet.checksum_ok {
                diagnostics.report(
                    "checksum",
                    None,
                    Some(format!(
                        "discarded checksum-invalid opcode {:#06x}; waiting for the next read",
                        packet.header.opcode
                    )),
                    None,
                    false,
                );

                continue;
            }

            let tick = packet.header.tick;

            let message = Box::new(server::classify(packet));

            if tx.send(Event::Message { message, tick }).is_err() {
                return;
            }
        }

        if let Some(error) = error {
            diagnostics.report("decode", None, Some(error.to_string()), None, false);

            if tx.send(Event::Decode(error)).is_err() {
                return;
            }
        }
    }
}

pub mod keepalive {

    pub const FIELD_MS: u32 = 30_000;

    pub const SELECT_CHAR_MS: u32 = 300_000;
}

pub use keys::TABLE as KEY_TABLE_LEN;

pub type RawPacket = Packet;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod native_tests;

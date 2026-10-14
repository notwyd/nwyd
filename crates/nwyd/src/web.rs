//! Public browser embedding API, consumed by web/play.js.

use std::cell::Cell;
use wasm_bindgen::prelude::*;

thread_local! { static STARTED: Cell<bool> = const { Cell::new(false) }; }

const MAX_ASSET_BYTES: usize = 1024 * 1024 * 1024;

#[wasm_bindgen]

pub struct GameFiles {
    root: String,
    files: Vec<(String, Vec<u8>)>,
    bytes: usize,
}

#[wasm_bindgen]

impl GameFiles {
    #[wasm_bindgen(constructor)]

    pub fn new(root: String) -> GameFiles {
        static PANIC_HOOK: std::sync::Once = std::sync::Once::new();

        PANIC_HOOK.call_once(|| {
            std::panic::set_hook(Box::new(|info| {
                console_error_panic_hook::hook(info);

                report_error(&format!("The game stopped unexpectedly: {info}"));
            }));
        });

        Self {
            root,
            files: Vec::new(),
            bytes: 0,
        }
    }

    /// Each path is relative to the selected directory; bytes never leave the browser.

    pub fn add_file(&mut self, path: String, bytes: Vec<u8>) -> Result<(), JsValue> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|total| *total <= MAX_ASSET_BYTES)
            .ok_or_else(|| {
                JsValue::from_str("Game assets exceed the 1 GiB browser import limit.")
            })?;

        self.files.push((path, bytes));

        Ok(())
    }

    /// Resolves after the shared renderer and browser event loop have started.

    pub async fn start(
        self,
        canvas: web_sys::HtmlCanvasElement,
        gateway: String,
        http_proxy: String,
    ) -> Result<(), JsValue> {
        if STARTED.with(|started| started.get()) {
            return Err(JsValue::from_str(
                "The game is already running. Reload the page to restart.",
            ));
        }

        if !gateway.is_empty() {
            protocol::transport::configure_websocket_gateway(&gateway).map_err(js_error)?;
        }

        if !http_proxy.is_empty() {
            crate::http_web::configure_proxy(&http_proxy).map_err(js_error)?;
        }

        let game = assets::GameDir::from_files(self.root, self.files).map_err(js_error)?;

        let client = crate::startup::load_client(game, crate::analytics::Analytics::default())
            .map_err(js_error)?;

        // Creating a winit event loop is one-shot even if GPU initialization fails.
        STARTED.with(|started| started.set(true));

        client.run_on_canvas(canvas).await.map_err(js_error)
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&format!("{error:#}"))
}

pub(crate) async fn delay(milliseconds: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        web_sys::window()
            .expect("browser window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, milliseconds)
            .expect("browser timeout");
    });

    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

pub(crate) fn report_error(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));

    dispatch_message("nwyd-error", message);
}

pub(crate) fn report_notice(message: &str) {
    dispatch_message("nwyd-notice", message);
}

fn dispatch_message(event_name: &str, message: &str) {
    let init = web_sys::CustomEventInit::new();

    init.set_detail(&JsValue::from_str(message));

    if let (Some(window), Ok(event)) = (
        web_sys::window(),
        web_sys::CustomEvent::new_with_event_init_dict(event_name, &init),
    ) {
        let _ = window.dispatch_event(&event);
    }
}

#[wasm_bindgen]

pub fn requires_reload() -> bool {
    STARTED.with(|started| started.get())
}

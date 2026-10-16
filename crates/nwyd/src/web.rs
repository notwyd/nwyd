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

                report_error(&format!("O jogo foi interrompido inesperadamente: {info}"));
            }));
        });

        Self {
            root,
            files: Vec::new(),
            bytes: 0,
        }
    }

    pub fn add_file(&mut self, path: String, bytes: Vec<u8>) -> Result<(), JsValue> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|total| *total <= MAX_ASSET_BYTES)
            .ok_or_else(|| {
                JsValue::from_str(
                    "Os arquivos do jogo excedem o limite de importação de 1 GiB do navegador.",
                )
            })?;

        self.files.push((path, bytes));

        Ok(())
    }

    pub async fn start(
        self,
        canvas: web_sys::HtmlCanvasElement,
        gateway: String,
        http_proxy: String,
    ) -> Result<(), JsValue> {
        if STARTED.with(|started| started.get()) {
            return Err(JsValue::from_str(
                "O jogo já está em execução. Recarregue a página para reiniciar.",
            ));
        }

        if !gateway.is_empty() {
            protocol::transport::configure_websocket_gateway(&gateway)
                .map_err(|error| js_error(error, "O endereço de conexão do relay é inválido."))?;
        }

        if !http_proxy.is_empty() {
            crate::http_web::configure_proxy(&http_proxy)
                .map_err(|error| js_error(error, "O endereço HTTP do relay é inválido."))?;
        }

        let game = assets::GameDir::from_files(self.root, self.files).map_err(|error| {
            js_error(error, "Não foi possível carregar os arquivos do jogo. Selecione uma instalação completa e válida.")
        })?;

        #[cfg(feature = "analytics")]
        let analytics = {
            let base_url = web_sys::window()
                .and_then(|window| window.location().origin().ok())
                .unwrap_or_default();

            crate::analytics::configured(&crate::analytics::Config {
                base_url,
                ..Default::default()
            })
            .unwrap_or_else(|error| {
                tracing::warn!("analytics disabled: {error}");

                crate::analytics::Analytics::default()
            })
        };

        #[cfg(not(feature = "analytics"))]
        let analytics = crate::analytics::Analytics::default();

        let client = crate::startup::load_client(game, analytics)
            .map_err(|error| {
                js_error(error, "Não foi possível preparar o jogo. Verifique se a pasta selecionada contém todos os arquivos de uma instalação válida.")
            })?;

        STARTED.with(|started| started.set(true));

        client.run_on_canvas(canvas).await.map_err(|error| {
            js_error(error, "Não foi possível iniciar os gráficos do jogo. Verifique se o navegador e a placa de vídeo oferecem suporte a WebGPU.")
        })
    }
}

fn js_error(error: impl std::fmt::Display, message: &str) -> JsValue {
    web_sys::console::error_1(&JsValue::from_str(&format!("{error:#}")));

    JsValue::from_str(message)
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

    dispatch_message(
        "nwyd-error",
        "O jogo foi interrompido por um erro inesperado.",
    );
}

pub(crate) fn report_notice(message: &str) {
    dispatch_message("nwyd-notice", message);
}

pub(crate) fn report_closed() {
    dispatch_message("nwyd-closed", "");
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

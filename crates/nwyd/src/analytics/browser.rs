use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/analytics/browser.mjs")]
extern "C" {
    type BrowserAnalytics;

    #[wasm_bindgen(catch, js_name = createAnalytics)]
    fn create_analytics(endpoint: &str) -> Result<BrowserAnalytics, JsValue>;

    #[wasm_bindgen(method, getter, js_name = sessionID)]
    fn session_id(this: &BrowserAnalytics) -> String;

    #[wasm_bindgen(catch, js_name = sendAnalytics)]
    fn send_analytics(delivery: &BrowserAnalytics, body: &[u8]) -> Result<(), JsValue>;

    #[wasm_bindgen(catch, js_name = disposeAnalytics)]
    fn dispose_analytics(delivery: &BrowserAnalytics) -> Result<(), JsValue>;
}

pub(super) struct Delivery(BrowserAnalytics);

impl Delivery {
    pub(super) fn new(endpoint: &str) -> anyhow::Result<Self> {
        create_analytics(endpoint)
            .map(Self)
            .map_err(|error| anyhow::anyhow!("could not initialize browser analytics: {error:?}"))
    }

    pub(super) fn session_id(&self) -> String {
        self.0.session_id()
    }

    pub(super) fn send(&self, body: &[u8]) {
        if send_analytics(&self.0, body).is_err() {
            tracing::warn!("analytics delivery failed");
        }
    }
}

impl Drop for Delivery {
    fn drop(&mut self) {
        let _ = dispose_analytics(&self.0);
    }
}

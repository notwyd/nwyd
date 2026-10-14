//! Bounded asynchronous HTTP for the browser. Legacy HTTP endpoints can be
//! reached through an explicitly configured, server-side allowlisted proxy.

use std::{cell::RefCell, time::Duration};
use wasm_bindgen::prelude::*;

thread_local! {
    static PROXY: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn configure_proxy(endpoint: &str) -> Result<(), String> {
    let url = url::Url::parse(endpoint).map_err(|error| error.to_string())?;

    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "HTTP proxy must be an absolute HTTP(S) URL without credentials or a fragment".into(),
        );
    }

    PROXY.with(|proxy| *proxy.borrow_mut() = Some(url.into()));

    Ok(())
}

#[wasm_bindgen(inline_js = r#"
export async function nwyd_http(url, body, timeout, limit) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeout);
    try {
        const response = await fetch(url, {
            method: body === undefined ? 'GET' : 'POST',
            body,
            headers: body === undefined ? undefined : {'Content-Type': 'application/x-www-form-urlencoded'},
            credentials: 'omit',
            cache: 'no-store',
            signal: controller.signal,
        });
        if (!response.body) return [response.status, new Uint8Array()];
        const reader = response.body.getReader();
        const chunks = [];
        let size = 0;
        try {
            while (size < limit) {
                const {done, value} = await reader.read();
                if (done) break;
                const chunk = value.subarray(0, limit - size);
                chunks.push(chunk);
                size += chunk.length;
            }
        } finally {
            await reader.cancel().catch(() => {});
        }
        const result = new Uint8Array(size);
        let at = 0;
        for (const chunk of chunks) { result.set(chunk, at); at += chunk.length; }
        return [response.status, result];
    } finally {
        clearTimeout(timer);
    }
}
export function nwyd_delay(ms) { return new Promise(resolve => setTimeout(resolve, ms)); }
"#)]
extern "C" {

    #[wasm_bindgen(catch)]
    async fn nwyd_http(
        url: &str,
        body: Option<String>,
        timeout: u32,
        limit: usize,
    ) -> Result<JsValue, JsValue>;

    async fn nwyd_delay(ms: u32);

}

pub(crate) async fn fetch(
    address: &str,
    body: Option<&str>,
    timeout: Duration,
    limit: usize,
) -> Result<(u16, Vec<u8>), String> {
    let address = PROXY.with(|proxy| {
        if let Some(proxy) = proxy.borrow().as_ref() {
            let mut url = url::Url::parse(proxy).expect("validated proxy URL");

            url.query_pairs_mut().append_pair("url", address);

            url.to_string()
        } else {
            address.to_owned()
        }
    });

    let response = nwyd_http(
        &address,
        body.map(str::to_owned),
        timeout.as_millis().min(i32::MAX as u128) as u32,
        limit,
    )
    .await
    .map_err(|error| {
        format!(
            "browser HTTP request failed (check HTTPS, CORS, or the configured proxy): {error:?}"
        )
    })?;

    let response = js_sys::Array::from(&response);

    let status = response
        .get(0)
        .as_f64()
        .ok_or("browser returned an invalid HTTP status")? as u16;

    Ok((status, js_sys::Uint8Array::new(&response.get(1)).to_vec()))
}

pub(crate) async fn delay(milliseconds: u32) {
    nwyd_delay(milliseconds).await;
}

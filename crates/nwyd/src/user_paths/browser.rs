use super::browser_store::SessionPreferences;
use std::{cell::RefCell, io, path::Path};

thread_local! {
    static SESSION: RefCell<SessionPreferences> = RefCell::default();
}

fn storage() -> io::Result<web_sys::Storage> {
    web_sys::window()
        .ok_or_else(|| io::Error::other("browser window is unavailable"))?
        .local_storage()
        .map_err(storage_error)?
        .ok_or_else(|| io::Error::other("browser local storage is unavailable"))
}

fn storage_error(error: wasm_bindgen::JsValue) -> io::Error {
    io::Error::other(format!("accessing browser preferences: {error:?}"))
}

fn key(path: &Path) -> String {
    format!("nwyd:preferences:{}", path.display())
}

pub(crate) fn read(path: &Path) -> io::Result<Vec<u8>> {
    let key = key(path);

    SESSION.with(|session| {
        session
            .borrow_mut()
            .read(&key, || storage()?.get_item(&key).map_err(storage_error))
    })
}

pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let key = key(path);

    SESSION.with(|session| {
        session.borrow_mut().write(&key, bytes, |value| {
            storage()?.set_item(&key, value).map_err(storage_error)
        })
    })
}

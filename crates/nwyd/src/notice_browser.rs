use formats::{
    config::ini,
    text::{Codepage, codepage},
};
use std::io;

pub fn manual_url(raw: &[u8], server: usize) -> Option<String> {
    let ini = ini::parse(&codepage::decode(raw, Codepage::Windows1252));

    if ini.get_int("Notice", "dp").unwrap_or(-1) == -1 {
        return None;
    }

    let url = ini.get("Notice", &format!("url_{server}"))?;

    let url = url
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(url);

    valid_web_url(url).then(|| url.to_owned())
}

fn valid_web_url(url: &str) -> bool {
    if url.len() > 1023
        || url
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '"' | '\\'))
    {
        return false;
    }

    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };

    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }

    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();

    !authority.is_empty() && !authority.contains('@')
}

pub fn dispatch(
    raw: &[u8],
    server: usize,
    open: impl FnOnce(&str) -> io::Result<()>,
) -> io::Result<bool> {
    let Some(url) = manual_url(raw, server) else {
        return Ok(false);
    };

    dispatch_url(&url, open)
}

pub fn dispatch_url(url: &str, open: impl FnOnce(&str) -> io::Result<()>) -> io::Result<bool> {
    if !valid_web_url(url) {
        return Ok(false);
    }

    open(url)?;

    Ok(true)
}

pub fn community_url(
    base: &str,
    account: &str,
    server: usize,
    launcher_kind: u16,
) -> Option<String> {
    if !valid_web_url(base) {
        return None;
    }

    let mut identity = String::new();

    for byte in codepage::encode(account, Codepage::Windows1252) {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            identity.push(char::from(byte));
        } else {
            use std::fmt::Write;

            write!(&mut identity, "%{byte:02X}").expect("String formatting cannot fail");
        }
    }

    let url = format!("{base}?id={identity}&server={server}&ltype={launcher_kind}");

    valid_web_url(&url).then_some(url)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open(url: &str) -> io::Result<()> {
    if !valid_web_url(url) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a web URL"));
    }

    let url = url.to_owned();

    std::thread::Builder::new()
        .name("nwyd-notice-browser".into())
        .spawn(move || {
            if let Err(error) = platform_open(&url) {
                tracing::warn!(% error, "NWYD: could not open the notice browser");
            }
        })
        .map(|_| ())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn platform_open(url: &str) -> io::Result<()> {
    use std::process::{Command, Stdio};

    let program = if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "xdg-open"
    };

    let status = Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "browser helper exited with {status}"
        )))
    }
}

#[cfg(target_os = "windows")]
fn platform_open(url: &str) -> io::Result<()> {
    use std::ffi::c_void;

    #[link(name = "ole32")]
    unsafe extern "system" {

        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;

        fn CoUninitialize();

    }

    #[link(name = "shell32")]
    unsafe extern "system" {

        fn ShellExecuteW(
            window: *mut c_void,
            verb: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> *mut c_void;

    }

    let verb: Vec<_> = "open".encode_utf16().chain([0]).collect();

    let file: Vec<_> = url.encode_utf16().chain([0]).collect();

    unsafe {
        let initialized = CoInitializeEx(std::ptr::null_mut(), 6);

        if initialized < 0 {
            return Err(io::Error::other(format!(
                "COM initialization failed: {initialized:#x}"
            )));
        }

        let result = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        ) as isize;

        CoUninitialize();

        if result > 32 {
            Ok(())
        } else {
            Err(io::Error::other(format!("ShellExecuteW failed: {result}")))
        }
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
fn platform_open(_: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no browser adapter for this platform",
    ))
}

#[cfg(target_arch = "wasm32")]
pub fn open(url: &str) -> io::Result<()> {
    if !valid_web_url(url) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a web URL"));
    }

    web_sys::window()
        .ok_or_else(|| io::Error::other("browser window unavailable"))?
        .open_with_url_and_target_and_features(url, "_blank", "noopener,noreferrer")
        .map(|_| ())
        .map_err(|_| io::Error::other("could not open browser tab"))
}

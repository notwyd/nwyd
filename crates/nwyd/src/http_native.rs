use std::{
    ffi::{CString, c_void},
    io,
    sync::{Mutex, OnceLock},
};
use windows::{
    Win32::Networking::WinInet::*,
    core::{PCSTR, s},
};

struct Handle(usize);

impl Handle {
    fn new(raw: *mut c_void) -> io::Result<Self> {
        if raw.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(raw as usize))
        }
    }

    fn raw(&self) -> *const c_void {
        self.0 as *const c_void
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = unsafe { InternetCloseHandle(self.raw()) };
    }
}

fn cstring(value: &str) -> io::Result<CString> {
    CString::new(value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "embedded NUL in HTTP address"))
}

fn api_error(error: windows::core::Error) -> io::Error {
    io::Error::other(error.to_string())
}

fn timeout(handle: &Handle, option: u32, milliseconds: u32) {
    let _ = unsafe {
        InternetSetOptionW(
            Some(handle.raw()),
            option,
            Some((&milliseconds as *const u32).cast()),
            4,
        )
    };
}

fn read(handle: &Handle, limit: usize, once: bool) -> Vec<u8> {
    crate::http_response::read_bounded(limit, once, |out| {
        let mut count = 0;

        unsafe {
            InternetReadFile(
                handle.raw(),
                out.as_mut_ptr().cast(),
                out.len() as u32,
                &mut count,
            )
        }
        .map_err(api_error)?;

        Ok(count as usize)
    })
}

pub(crate) struct Get {
    session: Option<Handle>,
    timeouts_ms: Option<u32>,
}

impl Get {
    pub(crate) fn new(timeouts_ms: Option<u32>) -> Self {
        Self {
            session: None,
            timeouts_ms,
        }
    }

    pub(crate) fn get(&mut self, url: &str, limit: usize, once: bool) -> io::Result<Vec<u8>> {
        get_on_session(self.session_raw()?, url, limit, once)
    }

    fn session_raw(&mut self) -> io::Result<usize> {
        if self.session.is_none() {
            let session = Handle::new(unsafe {
                InternetOpenA(s!("MS"), 0, PCSTR::null(), PCSTR::null(), 0)
            })?;

            if let Some(ms) = self.timeouts_ms {
                for option in [
                    INTERNET_OPTION_CONNECT_TIMEOUT,
                    INTERNET_OPTION_SEND_TIMEOUT,
                    INTERNET_OPTION_RECEIVE_TIMEOUT,
                ] {
                    timeout(&session, option, ms);
                }
            }

            self.session = Some(session);
        }

        Ok(self.session.as_ref().unwrap().0)
    }
}

fn get_on_session(session: usize, url: &str, limit: usize, once: bool) -> io::Result<Vec<u8>> {
    let url = cstring(url)?;

    let request = Handle::new(unsafe {
        InternetOpenUrlA(
            session as *const c_void,
            PCSTR(url.as_ptr().cast()),
            None,
            0x04000000,
            None,
        )
    })?;

    Ok(read(&request, limit, once))
}

pub(crate) fn guildmark_get(url: &str, limit: usize) -> io::Result<Vec<u8>> {
    static SESSION: OnceLock<Mutex<Get>> = OnceLock::new();

    let session = {
        let mut get = SESSION
            .get_or_init(|| Mutex::new(Get::new(Some(5000))))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        get.session_raw()?
    };

    get_on_session(session, url, limit, false)
}

#[derive(Default)]
pub(crate) struct Post {
    connection: Option<Handle>,
    session: Option<Handle>,
    hostname: Option<String>,
}

impl Post {
    pub(crate) fn post(
        &mut self,
        address: &str,
        body: &[u8],
        limit: usize,
    ) -> io::Result<(u16, Vec<u8>)> {
        let url = url::Url::parse(address)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid guild URL"))?;

        let host = url
            .host_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "guild URL has no host"))?;

        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported guild URL",
            ));
        }

        let port = url.port_or_known_default().unwrap();

        let hostname = host.to_ascii_lowercase();

        if self.session.is_none() {
            let session = Handle::new(unsafe {
                InternetOpenA(s!("HTTP"), 1, PCSTR::null(), PCSTR::null(), 0)
            })?;

            timeout(&session, INTERNET_OPTION_CONNECT_TIMEOUT, 1000);

            self.session = Some(session);
        }

        if self.hostname.as_ref() != Some(&hostname) || self.connection.is_none() {
            self.connection = None;

            let host_c = cstring(host)?;

            let connected = Handle::new(unsafe {
                InternetConnectA(
                    self.session.as_ref().unwrap().raw(),
                    PCSTR(host_c.as_ptr().cast()),
                    port,
                    s!(""),
                    s!(""),
                    INTERNET_SERVICE_HTTP,
                    0,
                    None,
                )
            });

            let connection = match connected {
                Ok(connection) => connection,
                Err(error) => {
                    self.hostname = None;

                    self.session = None;

                    return Err(error);
                }
            };

            self.connection = Some(connection);

            self.hostname = Some(hostname);
        }

        let mut path = url.path().to_owned();

        if let Some(query) = url.query() {
            path.push('?');

            path.push_str(query);
        }

        let path = cstring(&path)?;

        let accepts = [s!("*/*"), PCSTR::null()];

        let flags = if url.scheme() == "https" {
            0xa4c00000
        } else {
            0xa4400000
        };

        let request = Handle::new(unsafe {
            HttpOpenRequestA(
                self.connection.as_ref().unwrap().raw(),
                s!("POST"),
                PCSTR(path.as_ptr().cast()),
                s!("HTTP/1.1"),
                s!(""),
                Some(accepts.as_ptr()),
                flags,
                None,
            )
        })?;

        unsafe {
            HttpSendRequestA(
                request.raw(),
                Some(b"Content-Type: application/x-www-form-urlencoded"),
                Some(body.as_ptr().cast()),
                body.len() as u32,
            )
        }
        .map_err(api_error)?;

        let mut status = 0u32;

        let mut size = 4;

        unsafe {
            HttpQueryInfoW(
                request.raw(),
                HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
                Some((&mut status as *mut u32).cast()),
                &mut size,
                None,
            )
        }
        .map_err(api_error)?;

        if status != 200 {
            return Ok((status as u16, Vec::new()));
        }

        Ok((status as u16, read(&request, limit, false)))
    }
}

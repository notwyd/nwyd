use std::ffi::{CString, c_void};
use windows::{
    Win32::Networking::WinInet::*,
    core::{PCSTR, s},
};

pub(super) struct Client {
    host: CString,
    path: CString,
    port: u16,
    flags: u32,
    session: usize,
    connection: usize,
}

impl Client {
    pub(super) fn new(url: &url::Url) -> Self {
        let mut path = url.path().to_owned();

        if let Some(query) = url.query() {
            path.push('?');

            path.push_str(query);
        }

        Self {
            host: CString::new(url.host_str().unwrap()).expect("URL host has no NUL"),
            path: CString::new(path).expect("URL path has no NUL"),
            port: url.port_or_known_default().unwrap(),
            flags: if url.scheme() == "https" {
                0xa4c00000
            } else {
                0xa4400000
            },
            session: 0,
            connection: 0,
        }
    }

    pub(super) fn post(&mut self, body: &[u8]) {
        unsafe {
            if self.session == 0 {
                self.session = InternetOpenA(
                    s!("HTTP"),
                    INTERNET_OPEN_TYPE_DIRECT.0,
                    PCSTR::null(),
                    PCSTR::null(),
                    0,
                ) as usize;

                if self.session == 0 {
                    tracing::warn!("analytics WinINet session failed");

                    return;
                }

                let timeout = 3000u32;

                let _ = InternetSetOptionW(
                    Some(self.session as *const c_void),
                    INTERNET_OPTION_CONNECT_TIMEOUT,
                    Some((&timeout as *const u32).cast()),
                    4,
                );
            }

            if self.connection == 0 {
                self.connection = InternetConnectA(
                    self.session as *const c_void,
                    PCSTR(self.host.as_ptr().cast()),
                    self.port,
                    s!(""),
                    s!(""),
                    INTERNET_SERVICE_HTTP,
                    0,
                    None,
                ) as usize;

                if self.connection == 0 {
                    let _ = InternetCloseHandle(self.session as *const c_void);

                    self.session = 0;

                    tracing::warn!("analytics WinINet connection failed");

                    return;
                }
            }

            let accepts = [s!("*/*"), PCSTR::null()];

            let request = HttpOpenRequestA(
                self.connection as *const c_void,
                s!("POST"),
                PCSTR(self.path.as_ptr().cast()),
                s!("HTTP/1.1"),
                s!(""),
                Some(accepts.as_ptr()),
                self.flags,
                None,
            );

            if request.is_null() {
                tracing::warn!("analytics WinINet request failed");

                return;
            }

            let sent = HttpSendRequestA(
                request,
                Some(b"Content-Type:application/json"),
                Some(body.as_ptr().cast()),
                body.len() as u32,
            );

            if sent.is_ok() {
                let mut status = 0u32;

                let mut size = 4u32;

                let _ = HttpQueryInfoW(
                    request,
                    HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
                    Some((&mut status as *mut u32).cast()),
                    &mut size,
                    None,
                );

                if status == 200 {
                    let mut buffer = [0u8; 32767];

                    let mut offset = 0;

                    while offset < buffer.len() {
                        let mut read = 0;

                        if InternetReadFile(
                            request,
                            buffer.as_mut_ptr().add(offset).cast(),
                            (buffer.len() - offset) as u32,
                            &mut read,
                        )
                        .is_err()
                            || read == 0
                        {
                            break;
                        }

                        offset += read as usize;
                    }
                } else {
                    tracing::warn!(status, "analytics event rejected");
                }
            } else {
                tracing::warn!("analytics WinINet delivery failed");
            }

            let _ = InternetCloseHandle(request);
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            if self.connection != 0 {
                let _ = InternetCloseHandle(self.connection as *const c_void);
            }

            if self.session != 0 {
                let _ = InternetCloseHandle(self.session as *const c_void);
            }
        }
    }
}

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    io,
    sync::mpsc::{Receiver, TryRecvError, channel},
    time::Duration,
};

const METADATA_URL: &str = "https://github.com/notwyd/nwyd/releases/latest/download/version.json";
const DOWNLOAD_BASE: &str = "https://github.com/notwyd/nwyd/releases/latest/download";
const MAX_METADATA_BYTES: usize = 16 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub commit: String,
    pub build_number: u64,
    pub download_url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Status {
    Checking,
    UpToDate,
    Available(AvailableUpdate),
    Unavailable,
    #[default]
    Development,
}

#[derive(Default)]
pub struct Checker {
    receiver: Option<Receiver<Status>>,
    status: Status,
}

impl Checker {
    pub fn start() -> Self {
        Self::start_with(
            option_env!("NWYD_BUILD_COMMIT_FULL").unwrap_or("unknown"),
            option_env!("NWYD_BUILD_NUMBER")
                .and_then(|number| number.parse().ok())
                .unwrap_or(0),
            current_target(),
            fetch_metadata,
        )
    }

    fn start_with(
        commit: &str,
        build_number: u64,
        target: Option<&'static str>,
        fetch: impl FnOnce() -> io::Result<Vec<u8>> + Send + 'static,
    ) -> Self {
        if build_number == 0 || !valid_commit(commit) {
            return Self::default();
        }

        let Some(target) = target.filter(|target| asset_for_target(target).is_some()) else {
            return Self::default();
        };

        let commit = commit.to_owned();
        let (sender, receiver) = channel();
        let worker = std::thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                let status = fetch().map_or(Status::Unavailable, |bytes| {
                    status_from_metadata(&bytes, &commit, build_number, target)
                });

                let _ = sender.send(status);
            });

        if worker.is_err() {
            return Self {
                receiver: None,
                status: Status::Unavailable,
            };
        }

        Self {
            receiver: Some(receiver),
            status: Status::Checking,
        }
    }

    pub fn poll(&mut self) -> &Status {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(status) => {
                    self.status = status;
                    self.receiver = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.status = Status::Unavailable;
                    self.receiver = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }

        &self.status
    }
}

#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    commit: String,
    build_number: u64,
    assets: BTreeMap<String, String>,
}

fn valid_commit(commit: &str) -> bool {
    commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn current_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

fn asset_for_target(target: &str) -> Option<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" => Some("nwyd-x86_64-unknown-linux-gnu.tar.gz"),
        "x86_64-pc-windows-msvc" => Some("nwyd-x86_64-pc-windows-msvc.tar.gz"),
        "aarch64-apple-darwin" => Some("nwyd-aarch64-apple-darwin.tar.gz"),
        _ => None,
    }
}

fn status_from_metadata(bytes: &[u8], commit: &str, build_number: u64, target: &str) -> Status {
    if build_number == 0 || !valid_commit(commit) {
        return Status::Development;
    }

    let Some(asset) = asset_for_target(target) else {
        return Status::Development;
    };

    if bytes.len() > MAX_METADATA_BYTES {
        return Status::Unavailable;
    }

    let Ok(manifest) = serde_json::from_slice::<Manifest>(bytes) else {
        return Status::Unavailable;
    };

    if manifest.schema_version != 1
        || !valid_commit(&manifest.commit)
        || manifest.build_number == 0
        || manifest.assets.get(target).map(String::as_str) != Some(asset)
    {
        return Status::Unavailable;
    }

    if manifest.build_number <= build_number || manifest.commit.eq_ignore_ascii_case(commit) {
        return Status::UpToDate;
    }

    Status::Available(AvailableUpdate {
        commit: manifest.commit.to_ascii_lowercase(),
        build_number: manifest.build_number,
        download_url: format!("{DOWNLOAD_BASE}/{asset}"),
    })
}

#[cfg(windows)]
fn fetch_metadata() -> io::Result<Vec<u8>> {
    use std::{
        ffi::{CString, c_void},
        time::Instant,
    };
    use windows::{
        Win32::Networking::WinInet::*,
        core::{PCSTR, s},
    };

    struct Handle(*mut c_void);

    impl Handle {
        fn new(value: *mut c_void) -> io::Result<Self> {
            if value.is_null() {
                Err(io::Error::last_os_error())
            } else {
                Ok(Self(value))
            }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            let _ = unsafe { InternetCloseHandle(self.0) };
        }
    }

    fn timeout(handle: &Handle, option: u32, duration: Duration) -> io::Result<()> {
        let milliseconds = duration.as_millis().max(1) as u32;

        unsafe {
            InternetSetOptionW(
                Some(handle.0),
                option,
                Some((&milliseconds as *const u32).cast()),
                4,
            )
        }
        .map_err(|error| io::Error::other(error.to_string()))
    }

    let started = Instant::now();
    let session =
        Handle::new(unsafe { InternetOpenA(s!("NWYD"), 0, PCSTR::null(), PCSTR::null(), 0) })?;

    for option in [
        INTERNET_OPTION_CONNECT_TIMEOUT,
        INTERNET_OPTION_SEND_TIMEOUT,
        INTERNET_OPTION_RECEIVE_TIMEOUT,
    ] {
        timeout(&session, option, REQUEST_TIMEOUT)?;
    }

    let url = CString::new(METADATA_URL).unwrap();
    let request = Handle::new(unsafe {
        InternetOpenUrlA(
            session.0,
            PCSTR(url.as_ptr().cast()),
            Some(b"Accept: application/json\r\nCache-Control: no-cache\r\n"),
            INTERNET_FLAG_RELOAD
                | INTERNET_FLAG_NO_CACHE_WRITE
                | INTERNET_FLAG_PRAGMA_NOCACHE
                | INTERNET_FLAG_NO_UI,
            None,
        )
    })?;

    let mut status = 0u32;
    let mut size = 4;

    unsafe {
        HttpQueryInfoW(
            request.0,
            HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
            Some((&mut status as *mut u32).cast()),
            &mut size,
            None,
        )
    }
    .map_err(|error| io::Error::other(error.to_string()))?;

    if status != 200 {
        return Err(io::Error::other("update metadata request failed"));
    }

    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];

    while bytes.len() <= MAX_METADATA_BYTES {
        let remaining = REQUEST_TIMEOUT
            .checked_sub(started.elapsed())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "update request timed out"))?;

        timeout(&request, INTERNET_OPTION_RECEIVE_TIMEOUT, remaining)?;

        let capacity = chunk.len().min(MAX_METADATA_BYTES + 1 - bytes.len());
        let mut count = 0;

        unsafe {
            InternetReadFile(
                request.0,
                chunk.as_mut_ptr().cast(),
                capacity as u32,
                &mut count,
            )
        }
        .map_err(|error| io::Error::other(error.to_string()))?;

        if count == 0 {
            break;
        }

        bytes.extend_from_slice(&chunk[..count as usize]);
    }

    Ok(bytes)
}

#[cfg(not(windows))]
fn fetch_metadata() -> io::Result<Vec<u8>> {
    use std::io::Read;

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .user_agent("NWYD")
        .max_redirects(5)
        .max_response_header_size(16 * 1024)
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into();

    let mut response = agent
        .get(METADATA_URL)
        .header("Accept", "application/json")
        .header("Cache-Control", "no-cache")
        .call()
        .map_err(io::Error::other)?;

    if response.status().as_u16() != 200 {
        return Err(io::Error::other("update metadata request failed"));
    }

    let mut bytes = Vec::new();

    response
        .body_mut()
        .as_reader()
        .take((MAX_METADATA_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;

    Ok(bytes)
}

#[cfg(test)]
mod tests;

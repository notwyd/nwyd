#[cfg(target_os = "windows")]
pub mod native;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod webkit;

#[cfg(target_os = "windows")]
pub use native::Browser;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use webkit::Browser;

pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; Trident/7.0; rv:11.0) like Gecko";

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]

pub fn open(
    parent: &winit::window::Window,
    url: &str,
    retail_exe: &std::path::Path,
) -> anyhow::Result<Browser> {
    #[cfg(target_os = "windows")]
    {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let RawWindowHandle::Win32(handle) = parent.window_handle()?.as_raw() else {
            anyhow::bail!("Nell requires a native Windows window");
        };

        Ok(native::Browser::open(
            windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut _),
            url,
            retail_exe,
        )?)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = retail_exe;

        webkit::Browser::open(parent, url)
    }
}

#[derive(Debug)]

pub struct State {
    pub supported: bool,
    pub visible: bool,
    pub pending: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            supported: cfg!(any(
                target_os = "windows",
                target_os = "linux",
                target_os = "macos"
            )),
            visible: false,
            pending: None,
        }
    }
}

impl State {
    pub fn close(&mut self) {
        self.visible = false;

        self.pending = None;
    }
}

pub fn service_url(
    base: &str,
    account: &str,
    server: usize,
    empty: usize,
    kind: u16,
) -> Option<String> {
    let url = crate::notice_browser::community_url(base, account, server, kind)?;

    let (prefix, launcher) = url.rsplit_once("&ltype=")?;

    (empty <= 120).then(|| format!("{prefix}&Empty={empty}&ltype={launcher}"))
}

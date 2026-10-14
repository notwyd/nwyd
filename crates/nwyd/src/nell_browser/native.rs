use std::{
    cell::{Cell, RefCell},
    path::Path,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        Networking::WinInet::DeleteUrlCacheEntryW,
        System::{Com::*, LibraryLoader::*, Ole::*, Variant::VARIANT},
        UI::{
            Controls::DRAWITEMSTRUCT,
            Input::KeyboardAndMouse::SetFocus,
            Shell::{IWebBrowser2, WebBrowser},
            WindowsAndMessaging::*,
        },
    },
    core::{BOOL, BSTR, IUnknownImpl, Interface, OutRef, PCWSTR, Ref, Result, implement, w},
};

mod doc_ui;

use doc_ui::IDocHostUIHandler;
use windows::Win32::System::Com::Urlmon::{
    URLMON_OPTION_USERAGENT, UrlMkGetSessionOption, UrlMkSetSessionOption,
};

pub fn system_user_agent() -> Result<String> {
    let mut bytes = vec![0xffu8; 65536];

    let mut size = 0;

    let status = unsafe {
        UrlMkGetSessionOption(
            URLMON_OPTION_USERAGENT,
            Some(bytes.as_mut_ptr().cast()),
            bytes.len() as u32,
            &mut size,
            None,
        )
    };

    decode_user_agent(status, &bytes, size)
}

fn decode_user_agent(status: Result<()>, bytes: &[u8], size: u32) -> Result<String> {
    let failure = match status {
        Err(error) if error.code() != E_OUTOFMEMORY => return Err(error),
        Err(error) => error,
        Ok(()) => E_FAIL.into(),
    };

    bytes
        .get(..size as usize)
        .and_then(|written| std::ffi::CStr::from_bytes_with_nul(written).ok())
        .filter(|agent| !agent.is_empty())
        .map(|agent| agent.to_string_lossy().into_owned())
        .ok_or(failure)
}

struct UserAgent(String);

impl UserAgent {
    fn set(value: &str) -> Result<Self> {
        let previous = system_user_agent()?;

        set_user_agent(value)?;

        Ok(Self(previous))
    }
}

fn set_user_agent(value: &str) -> Result<()> {
    let bytes =
        std::ffi::CString::new(value).map_err(|_| windows::core::Error::from(E_INVALIDARG))?;

    unsafe {
        UrlMkSetSessionOption(
            URLMON_OPTION_USERAGENT,
            Some(bytes.as_ptr().cast()),
            bytes.as_bytes().len() as u32,
            None,
        )
    }
}

impl Drop for UserAgent {
    fn drop(&mut self) {
        let _ = set_user_agent(&self.0);
    }
}

const WIDTH: i32 = 800;

const HEIGHT: i32 = 560;

const CLOSE_ID: usize = 999;

const RECTANGLE: RECT = RECT {
    left: 0,
    top: 0,
    right: WIDTH,
    bottom: HEIGHT,
};

thread_local! {
    static HOST : Cell < Option < HWND >> = const { Cell::new(None) }; static ACTIVE :
    RefCell < Option < (HWND, IOleInPlaceActiveObject) >> = const { RefCell::new(None) };
}

pub fn message_hook(raw: *const std::ffi::c_void) -> bool {
    let msg = unsafe { &*(raw.cast::<MSG>()) };

    let Some(hwnd) = HOST.with(Cell::get) else {
        return false;
    };

    if !(WM_KEYFIRST..=WM_KEYLAST).contains(&msg.message) {
        return false;
    }

    unsafe {
        if msg.message == WM_KEYDOWN && msg.wParam.0 == 27 {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));

            return true;
        }

        if (msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd).as_bool())
            && let Some((_, active)) = ACTIVE.with(|slot| slot.borrow().clone())
        {
            return (active.vtable().TranslateAccelerator)(active.as_raw(), msg) == S_OK;
        }
    }

    false
}

struct WindowState {
    closed: Cell<bool>,
    bitmap: HBITMAP,
}

unsafe extern "system" fn host_proc(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lp.0 as *const CREATESTRUCTW);

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }

        let state = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowState).as_ref();

        if let Some(state) = state {
            if message == WM_CLOSE || (message == WM_COMMAND && wp.0 & 0xffff == CLOSE_ID) {
                state.closed.set(true);

                let _ = ShowWindow(hwnd, SW_HIDE);

                return LRESULT(0);
            }

            if message == WM_DRAWITEM && wp.0 == CLOSE_ID && !state.bitmap.is_invalid() {
                let draw = &*(lp.0 as *const DRAWITEMSTRUCT);

                let dc = CreateCompatibleDC(Some(draw.hDC));

                if !dc.is_invalid() {
                    let old = SelectObject(dc, state.bitmap.into());

                    let _ = StretchBlt(draw.hDC, 0, 0, 86, 21, Some(dc), 0, 0, 86, 21, SRCCOPY);

                    SelectObject(dc, old);

                    let _ = DeleteDC(dc);
                }

                return LRESULT(1);
            }
        }

        if message == WM_NCDESTROY {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }

        DefWindowProcW(hwnd, message, wp, lp)
    }
}

#[implement(IOleClientSite, IOleInPlaceSite, IOleInPlaceFrame, IDocHostUIHandler)]

struct Site {
    hwnd: HWND,
}

impl IOleClientSite_Impl for Site_Impl {
    fn SaveObject(&self) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetMoniker(&self, _: &OLEGETMONIKER, _: &OLEWHICHMK) -> Result<IMoniker> {
        Err(E_NOTIMPL.into())
    }

    fn GetContainer(&self) -> Result<IOleContainer> {
        Err(E_NOINTERFACE.into())
    }

    fn ShowObject(&self) -> Result<()> {
        Ok(())
    }

    fn OnShowWindow(&self, _: BOOL) -> Result<()> {
        Ok(())
    }

    fn RequestNewObjectLayout(&self) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
}

impl IOleWindow_Impl for Site_Impl {
    fn GetWindow(&self) -> Result<HWND> {
        Ok(self.hwnd)
    }

    fn ContextSensitiveHelp(&self, _: BOOL) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
}

impl IOleInPlaceSite_Impl for Site_Impl {
    fn CanInPlaceActivate(&self) -> Result<()> {
        Ok(())
    }

    fn OnInPlaceActivate(&self) -> Result<()> {
        Ok(())
    }

    fn OnUIActivate(&self) -> Result<()> {
        Ok(())
    }

    fn GetWindowContext(
        &self,
        frame: OutRef<IOleInPlaceFrame>,
        doc: OutRef<IOleInPlaceUIWindow>,
        pos: *mut RECT,
        clip: *mut RECT,
        info: *mut OLEINPLACEFRAMEINFO,
    ) -> Result<()> {
        frame.write(Some(self.to_interface()))?;

        doc.write(None)?;

        unsafe {
            if pos.is_null() || clip.is_null() || info.is_null() {
                return Err(E_POINTER.into());
            }

            *pos = RECTANGLE;

            *clip = RECTANGLE;

            *info = OLEINPLACEFRAMEINFO {
                cb: std::mem::size_of::<OLEINPLACEFRAMEINFO>() as u32,
                hwndFrame: self.hwnd,
                ..Default::default()
            };
        }

        Ok(())
    }

    fn Scroll(&self, _: &SIZE) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn OnUIDeactivate(&self, _: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnInPlaceDeactivate(&self) -> Result<()> {
        Ok(())
    }

    fn DiscardUndoState(&self) -> Result<()> {
        Ok(())
    }

    fn DeactivateAndUndo(&self) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn OnPosRectChange(&self, _: *const RECT) -> Result<()> {
        Ok(())
    }
}

impl IOleInPlaceUIWindow_Impl for Site_Impl {
    fn GetBorder(&self) -> Result<RECT> {
        Err(E_NOTIMPL.into())
    }

    fn RequestBorderSpace(&self, _: *const RECT) -> Result<()> {
        Err(INPLACE_E_NOTOOLSPACE.into())
    }

    fn SetBorderSpace(&self, _: *const RECT) -> Result<()> {
        Ok(())
    }

    fn SetActiveObject(&self, object: Ref<IOleInPlaceActiveObject>, _: &PCWSTR) -> Result<()> {
        let previous =
            ACTIVE.with(|slot| slot.replace(object.as_ref().map(|o| (self.hwnd, o.clone()))));

        drop(previous);

        Ok(())
    }
}

impl IOleInPlaceFrame_Impl for Site_Impl {
    fn InsertMenus(&self, _: HMENU, _: *mut OLEMENUGROUPWIDTHS) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn SetMenu(&self, _: HMENU, _: isize, _: HWND) -> Result<()> {
        Ok(())
    }

    fn RemoveMenus(&self, _: HMENU) -> Result<()> {
        Ok(())
    }

    fn SetStatusText(&self, _: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn EnableModeless(&self, _: BOOL) -> Result<()> {
        Ok(())
    }

    fn TranslateAccelerator(&self, _: *const MSG, _: u16) -> Result<()> {
        Err(S_FALSE.into())
    }
}

struct Apartment;

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { OleUninitialize() }
    }
}

pub struct Browser {
    ole: Option<IOleObject>,
    browser: Option<IWebBrowser2>,
    site: Option<IOleClientSite>,
    hwnd: HWND,
    parent: HWND,
    state: Box<WindowState>,
    cache_url: Vec<u16>,
    cache_retry: Option<Instant>,
    _user_agent: Option<UserAgent>,
    _apartment: Apartment,
}

impl Browser {
    pub fn open(parent: HWND, url: &str, retail_exe: &Path) -> Result<Self> {
        Self::open_with_user_agent(parent, url, retail_exe, Some(super::USER_AGENT))
    }

    pub fn open_with_user_agent(
        parent: HWND,
        url: &str,
        retail_exe: &Path,
        agent: Option<&str>,
    ) -> Result<Self> {
        unsafe {
            OleInitialize(None)?;

            let apartment = Apartment;

            let user_agent = agent.map(UserAgent::set).transpose()?;

            let mut host = Self {
                ole: None,
                browser: None,
                site: None,
                hwnd: HWND::default(),
                parent,
                state: Box::new(WindowState {
                    closed: Cell::new(false),
                    bitmap: HBITMAP::default(),
                }),
                cache_url: url.encode_utf16().chain(Some(0)).collect(),
                cache_retry: Some(Instant::now()),
                _user_agent: user_agent,
                _apartment: apartment,
            };

            let instance: HINSTANCE = GetModuleHandleW(None)?.into();

            let class = WNDCLASSW {
                lpfnWndProc: Some(host_proc),
                hInstance: instance,
                lpszClassName: w!("NWYD.OleBrowser"),
                ..Default::default()
            };

            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(windows::core::Error::from_thread());
            }

            host.hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class.lpszClassName,
                w!(""),
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                0,
                0,
                WIDTH,
                HEIGHT + 21,
                Some(parent),
                None,
                Some(instance),
                Some((&*host.state as *const WindowState).cast()),
            )?;

            use std::os::windows::ffi::OsStrExt;

            let path: Vec<_> = retail_exe
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();

            if let Ok(module) =
                LoadLibraryExW(PCWSTR(path.as_ptr()), None, LOAD_LIBRARY_AS_DATAFILE)
            {
                host.state.bitmap =
                    LoadBitmapW(Some(HINSTANCE(module.0)), PCWSTR(171usize as *const u16));

                let _ = FreeLibrary(module);
            }

            let style = if host.state.bitmap.is_invalid() {
                BS_PUSHBUTTON
            } else {
                BS_OWNERDRAW
            };

            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("Button"),
                w!("Fechar"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style as u32),
                714,
                HEIGHT,
                86,
                21,
                Some(host.hwnd),
                Some(HMENU(CLOSE_ID as *mut _)),
                Some(instance),
                None,
            )?;

            let site: IOleClientSite = Site { hwnd: host.hwnd }.into();

            let ole: IOleObject = CoCreateInstance(&WebBrowser, None, CLSCTX_INPROC_SERVER)?;

            host.ole = Some(ole.clone());

            host.site = Some(site.clone());

            ole.SetClientSite(&site)?;

            ole.SetHostNames(w!("My Host Name"), PCWSTR::null())?;

            if let Ok(persist) = ole.cast::<IPersistStreamInit>() {
                persist.InitNew()?;
            }

            OleSetContainedObject(&ole, true)?;

            ole.DoVerb(
                OLEIVERB_SHOW.0,
                std::ptr::null(),
                &site,
                -1,
                host.hwnd,
                &RECTANGLE,
            )?;

            let browser: IWebBrowser2 = ole.cast()?;

            browser.SetLeft(0)?;

            browser.SetTop(0)?;

            browser.SetWidth(WIDTH)?;

            browser.SetHeight(HEIGHT)?;

            let destination = VARIANT::from(BSTR::from(url));

            browser.Navigate2(&destination, None, None, None, None)?;

            let active = ole.cast::<IOleInPlaceActiveObject>()?;

            let previous = ACTIVE.with(|slot| slot.replace(Some((host.hwnd, active))));

            drop(previous);

            host.browser = Some(browser);

            host.reposition()?;

            let _ = ShowWindow(host.hwnd, SW_SHOW);

            HOST.with(|slot| slot.set(Some(host.hwnd)));

            let _ = SetFocus(Some(parent));

            Ok(host)
        }
    }

    pub fn closed(&self) -> bool {
        self.state.closed.get() || unsafe { !IsWindow(Some(self.hwnd)).as_bool() }
    }

    pub fn close(&self) {
        self.state.closed.set(true);

        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn reposition(&self) -> Result<()> {
        unsafe {
            let mut rect = RECT::default();

            GetClientRect(self.parent, &mut rect)?;

            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                (rect.right - WIDTH) / 2,
                (rect.bottom - HEIGHT) / 2,
                WIDTH,
                HEIGHT + 21,
                SWP_NOACTIVATE,
            )
        }
    }

    pub fn poll(&mut self) {
        if self
            .cache_retry
            .is_some_and(|at| at.elapsed() > Duration::from_secs(1))
        {
            let deleted = unsafe { DeleteUrlCacheEntryW(PCWSTR(self.cache_url.as_ptr())).is_ok() };

            self.cache_retry = (!deleted).then(Instant::now);
        }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        HOST.with(|slot| slot.set(None));

        drop(ACTIVE.with(|slot| slot.borrow_mut().take()));

        unsafe {
            if let Some(browser) = self.browser.take() {
                let _ = browser.Stop();
            }

            if let Some(ole) = self.ole.take() {
                let _ = ole.Close(OLECLOSE_NOSAVE);

                let _ = ole.SetClientSite(None);
            }

            drop(ACTIVE.with(|slot| slot.borrow_mut().take()));

            self.site.take();

            if !self.hwnd.is_invalid() && IsWindow(Some(self.hwnd)).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }

            if !self.state.bitmap.is_invalid() {
                let _ = DeleteObject(self.state.bitmap.into());
            }

            if IsWindow(Some(self.parent)).as_bool() {
                let _ = SetFocus(Some(self.parent));
            }
        }
    }
}

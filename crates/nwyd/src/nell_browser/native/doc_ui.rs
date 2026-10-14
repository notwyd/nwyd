#![allow(non_snake_case)]

use std::ffi::c_void;
use windows::Win32::{Foundation::*, UI::WindowsAndMessaging::MSG};
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, interface};

#[repr(C)]

pub struct HostInfo {
    size: u32,
    flags: u32,
    double_click: u32,
    css: *mut u16,
    namespace: *mut u16,
}

#[interface("bd3f23c0-d43e-11cf-893b-00aa00bdce1a")]

pub unsafe trait IDocHostUIHandler: IUnknown {
    fn ShowContextMenu(
        &self,
        id: u32,
        point: *const POINT,
        command: *mut c_void,
        dispatch: *mut c_void,
    ) -> HRESULT;

    fn GetHostInfo(&self, info: *mut HostInfo) -> HRESULT;

    fn ShowUI(
        &self,
        id: u32,
        active: *mut c_void,
        command: *mut c_void,
        frame: *mut c_void,
        doc: *mut c_void,
    ) -> HRESULT;

    fn HideUI(&self) -> HRESULT;

    fn UpdateUI(&self) -> HRESULT;

    fn EnableModeless(&self, enable: i32) -> HRESULT;

    fn OnDocWindowActivate(&self, active: i32) -> HRESULT;

    fn OnFrameWindowActivate(&self, active: i32) -> HRESULT;

    fn ResizeBorder(&self, rect: *const RECT, window: *mut c_void, frame: i32) -> HRESULT;

    fn TranslateAccelerator(&self, msg: *const MSG, group: *const GUID, id: u32) -> HRESULT;

    fn GetOptionKeyPath(&self, key: *mut *mut u16, reserved: u32) -> HRESULT;

    fn GetDropTarget(&self, target: *mut c_void, out: *mut *mut c_void) -> HRESULT;

    fn GetExternal(&self, out: *mut *mut c_void) -> HRESULT;

    fn TranslateUrl(&self, flags: u32, url: *mut u16, out: *mut *mut u16) -> HRESULT;

    fn FilterDataObject(&self, object: *mut c_void, out: *mut *mut c_void) -> HRESULT;
}

impl IDocHostUIHandler_Impl for super::Site_Impl {
    unsafe fn ShowContextMenu(
        &self,
        _: u32,
        _: *const POINT,
        _: *mut c_void,
        _: *mut c_void,
    ) -> HRESULT {
        S_OK
    }

    unsafe fn GetHostInfo(&self, info: *mut HostInfo) -> HRESULT {
        if info.is_null() {
            return E_POINTER;
        }

        unsafe {
            *info = HostInfo {
                size: std::mem::size_of::<HostInfo>() as u32,
                flags: 4,
                double_click: 0,
                css: std::ptr::null_mut(),
                namespace: std::ptr::null_mut(),
            };
        }

        S_OK
    }

    unsafe fn ShowUI(
        &self,
        _: u32,
        _: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
    ) -> HRESULT {
        S_OK
    }

    unsafe fn HideUI(&self) -> HRESULT {
        S_OK
    }

    unsafe fn UpdateUI(&self) -> HRESULT {
        S_OK
    }

    unsafe fn EnableModeless(&self, _: i32) -> HRESULT {
        S_OK
    }

    unsafe fn OnDocWindowActivate(&self, _: i32) -> HRESULT {
        S_OK
    }

    unsafe fn OnFrameWindowActivate(&self, _: i32) -> HRESULT {
        S_OK
    }

    unsafe fn ResizeBorder(&self, _: *const RECT, _: *mut c_void, _: i32) -> HRESULT {
        S_OK
    }

    unsafe fn TranslateAccelerator(&self, _: *const MSG, _: *const GUID, _: u32) -> HRESULT {
        S_FALSE
    }

    unsafe fn GetOptionKeyPath(&self, out: *mut *mut u16, _: u32) -> HRESULT {
        unsafe { decline(out) }
    }

    unsafe fn GetDropTarget(&self, _: *mut c_void, out: *mut *mut c_void) -> HRESULT {
        unsafe { decline(out) }
    }

    unsafe fn GetExternal(&self, out: *mut *mut c_void) -> HRESULT {
        unsafe { decline(out) }
    }

    unsafe fn TranslateUrl(&self, _: u32, _: *mut u16, out: *mut *mut u16) -> HRESULT {
        unsafe { decline(out) }
    }

    unsafe fn FilterDataObject(&self, _: *mut c_void, out: *mut *mut c_void) -> HRESULT {
        unsafe { decline(out) }
    }
}

unsafe fn decline<T>(out: *mut *mut T) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }

    unsafe {
        *out = std::ptr::null_mut();
    }

    S_FALSE
}

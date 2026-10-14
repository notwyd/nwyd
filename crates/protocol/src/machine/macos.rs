use crate::client::{Fingerprint, MAC_LEN};
use std::ffi::{CStr, c_char, c_void};
use std::{io, ptr};

const UTF8: u32 = 0x0800_0100;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {

    fn IOServiceMatching(name: *const c_char) -> *mut c_void;

    fn IOServiceGetMatchingService(port: u32, matching: *mut c_void) -> u32;

    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: *const c_void,
        allocator: *const c_void,
        options: u32,
    ) -> *const c_void;

    fn IOObjectRelease(object: u32) -> i32;

}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {

    fn CFStringCreateWithCString(
        allocator: *const c_void,
        value: *const c_char,
        encoding: u32,
    ) -> *const c_void;

    fn CFStringGetCString(
        value: *const c_void,
        buffer: *mut c_char,
        size: isize,
        encoding: u32,
    ) -> u8;

    fn CFStringGetTypeID() -> usize;

    fn CFGetTypeID(value: *const c_void) -> usize;

    fn CFRelease(value: *const c_void);

}

struct Property(*const c_void);

impl Drop for Property {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

fn platform_uuid() -> io::Result<String> {
    let matching = unsafe { IOServiceMatching(c"IOPlatformExpertDevice".as_ptr()) };

    if matching.is_null() {
        return Err(io::Error::other("could not locate the platform service"));
    }

    let service = unsafe { IOServiceGetMatchingService(0, matching) };

    if service == 0 {
        return Err(io::Error::other("platform service is unavailable"));
    }

    let key = unsafe { CFStringCreateWithCString(ptr::null(), c"IOPlatformUUID".as_ptr(), UTF8) };

    if key.is_null() {
        unsafe { IOObjectRelease(service) };

        return Err(io::Error::other(
            "could not create the platform identity key",
        ));
    }

    let key = Property(key);

    let value = unsafe { IORegistryEntryCreateCFProperty(service, key.0, ptr::null(), 0) };

    unsafe { IOObjectRelease(service) };

    if value.is_null() {
        return Err(io::Error::other("platform identity is unavailable"));
    }

    let value = Property(value);

    if unsafe { CFGetTypeID(value.0) != CFStringGetTypeID() } {
        return Err(io::Error::other("platform identity has an invalid type"));
    }

    let mut buffer = [0 as c_char; 37];

    if unsafe { CFStringGetCString(value.0, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) } == 0
    {
        return Err(io::Error::other("could not read the platform identity"));
    }

    let bytes = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_bytes();

    if bytes.len() != 36
        || bytes.iter().enumerate().any(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                *b != b'-'
            } else {
                !b.is_ascii_hexdigit()
            }
        })
    {
        return Err(io::Error::other("platform identity is not a UUID"));
    }

    Ok(String::from_utf8_lossy(bytes).into_owned())
}

struct Interfaces(*mut libc::ifaddrs);

impl Drop for Interfaces {
    fn drop(&mut self) {
        unsafe { libc::freeifaddrs(self.0) };
    }
}

fn network_address() -> io::Result<[u8; MAC_LEN]> {
    let mut first = ptr::null_mut();

    if unsafe { libc::getifaddrs(&mut first) } != 0 {
        return Err(io::Error::last_os_error());
    }

    let interfaces = Interfaces(first);

    let mut current = interfaces.0;

    let mut addresses = Vec::new();

    while !current.is_null() {
        let interface = unsafe { &*current };

        current = interface.ifa_next;

        if interface.ifa_addr.is_null()
            || interface.ifa_name.is_null()
            || interface.ifa_flags & libc::IFF_LOOPBACK as u32 != 0
        {
            continue;
        }

        let address = unsafe { &*interface.ifa_addr };

        if i32::from(address.sa_family) != libc::AF_LINK {
            continue;
        }

        let name = unsafe { CStr::from_ptr(interface.ifa_name) }.to_bytes();

        if !name.starts_with(b"en") {
            continue;
        }

        let data = unsafe {
            std::slice::from_raw_parts(interface.ifa_addr.cast::<u8>(), usize::from(address.sa_len))
        };

        let offset = std::mem::offset_of!(libc::sockaddr_dl, sdl_data);

        if data.len() < offset || data[4] != 6 || usize::from(data[6]) != MAC_LEN {
            continue;
        }

        let start = offset + usize::from(data[5]);

        let Some(bytes) = data.get(start..start + MAC_LEN) else {
            continue;
        };

        let mac: [u8; MAC_LEN] = bytes.try_into().unwrap();

        if mac != [0; MAC_LEN] && mac[0] & 1 == 0 {
            addresses.push((name.to_vec(), mac));
        }
    }

    addresses.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    addresses
        .into_iter()
        .next()
        .map(|(_, mac)| mac)
        .ok_or_else(|| io::Error::other("no usable Ethernet or Wi-Fi network address"))
}

pub(super) fn fingerprint() -> io::Result<Fingerprint> {
    let uuid = platform_uuid()?;

    let mac = network_address()?;

    Ok(Fingerprint::from_adapter(&uuid, mac))
}

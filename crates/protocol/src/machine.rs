use crate::client::Fingerprint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalFingerprint {
    pub fingerprint: Fingerprint,
    pub native: bool,
}

pub fn local_fingerprint() -> std::io::Result<LocalFingerprint> {
    #[cfg(windows)]
    return native::fingerprint().map(|fingerprint| LocalFingerprint {
        fingerprint,
        native: true,
    });

    #[cfg(target_os = "linux")]
    return portable_linux::fingerprint().map(|fingerprint| LocalFingerprint {
        fingerprint,
        native: false,
    });

    #[cfg(target_os = "macos")]
    return macos::fingerprint().map(|fingerprint| LocalFingerprint {
        fingerprint,
        native: false,
    });

    #[cfg(target_arch = "wasm32")]
    return browser::fingerprint().map(|fingerprint| LocalFingerprint {
        fingerprint,
        native: false,
    });

    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "macos",
        target_arch = "wasm32"
    )))]
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "automatic machine identity is unavailable on this platform; could not identify the current machine",
    ))
}

#[cfg(windows)]
fn adapter_score(description: &[u8], kind: u32, address_len: u32, gateway: &[u8]) -> i32 {
    let description = description.to_ascii_lowercase();

    if address_len == 0
        || [
            b"tap-".as_slice(),
            b"vpn",
            b"virtual",
            b"vmware",
            b"hyper-v",
            b"bluetooth",
        ]
        .iter()
        .any(|needle| {
            description
                .windows(needle.len())
                .any(|part| part == *needle)
        })
    {
        return -100;
    }

    i32::from(matches!(kind, 6 | 0x47)) * 20
        + i32::from(!gateway.is_empty() && gateway != b"0.0.0.0") * 10
}

#[cfg(windows)]
mod native {

    use super::*;
    use windows::Win32::NetworkManagement::IpHelper::{GetAdaptersInfo, IP_ADAPTER_INFO};

    fn text(bytes: &[i8]) -> Vec<u8> {
        bytes
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as u8)
            .collect()
    }

    pub(super) fn fingerprint() -> std::io::Result<Fingerprint> {
        let mut size = 0;

        let code = unsafe { GetAdaptersInfo(None, &mut size) };

        if code != 111 {
            return Err(std::io::Error::from_raw_os_error(code as i32));
        }

        {
            let count = (size as usize).div_ceil(std::mem::size_of::<IP_ADAPTER_INFO>());

            let mut adapters = vec![IP_ADAPTER_INFO::default(); count.max(1)];

            let code = unsafe { GetAdaptersInfo(Some(adapters.as_mut_ptr()), &mut size) };

            if code != 0 {
                return Err(std::io::Error::from_raw_os_error(code as i32));
            }

            let mut next = adapters.as_ptr();

            let mut best_score = -1;

            let mut best = Fingerprint::ZERO;

            while !next.is_null() {
                let adapter = unsafe { &*next };

                let score = adapter_score(
                    &text(&adapter.Description),
                    adapter.Type,
                    adapter.AddressLength,
                    &text(&adapter.GatewayList.IpAddress.String),
                );

                if score > best_score {
                    best_score = score;

                    let name = String::from_utf8_lossy(&text(&adapter.AdapterName)).into_owned();

                    let mut mac = [0; 6];

                    mac.copy_from_slice(&adapter.Address[..6]);

                    best = Fingerprint::from_adapter(&name, mac);
                }

                next = adapter.Next;
            }

            Ok(best)
        }
    }
}

#[cfg(target_os = "linux")]
mod portable_linux {

    use super::*;

    pub(super) fn fingerprint() -> std::io::Result<Fingerprint> {
        let host = ["/etc/machine-id", "/var/lib/dbus/machine-id"]
            .into_iter()
            .find_map(|path| {
                std::fs::read_to_string(path)
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .ok_or_else(|| {
                std::io::Error::other(
                    "no stable host identifier; could not identify the current machine",
                )
            })?;

        let mut interfaces: Vec<_> = std::fs::read_dir("/sys/class/net")?
            .filter_map(Result::ok)
            .collect();

        interfaces.sort_by_key(|entry| (!entry.path().join("device").exists(), entry.file_name()));

        let mac = interfaces
            .into_iter()
            .find_map(|entry| {
                if entry.file_name() == "lo" {
                    return None;
                }

                let value = std::fs::read_to_string(entry.path().join("address")).ok()?;

                let bytes = value
                    .trim()
                    .split(':')
                    .map(|part| u8::from_str_radix(part, 16))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;

                let mac: [u8; 6] = bytes.try_into().ok()?;

                (mac != [0; 6] && mac[0] & 1 == 0).then_some(mac)
            })
            .ok_or_else(|| {
                std::io::Error::other(
                    "no usable network address; could not identify the current machine",
                )
            })?;

        let seed = fnv1a64(format!("wyd-portable-adapter-v1:{}", host.trim()).as_bytes());

        Ok(Fingerprint {
            guid: portable_guid(seed),
            mac,
        })
    }
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
fn portable_guid(seed: u64) -> [u32; 4] {
    let mut state = seed;

    let mut guid = [0u8; 16];

    guid[..8].copy_from_slice(&splitmix64(&mut state).to_le_bytes());

    guid[8..].copy_from_slice(&splitmix64(&mut state).to_le_bytes());

    guid[6] = (guid[6] & 0x0F) | 0x40;

    guid[8] = (guid[8] & 0x3F) | 0x80;

    std::array::from_fn(|i| u32::from_be_bytes(guid[i * 4..i * 4 + 4].try_into().unwrap()))
}

#[cfg(target_os = "linux")]
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);

    let mut z = *state;

    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);

    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

    z ^ (z >> 31)
}

#[cfg(target_os = "linux")]
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;

    for byte in bytes {
        hash ^= *byte as u64;

        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }

    hash
}

#[cfg(target_arch = "wasm32")]
mod browser {

    use super::*;
    use std::cell::Cell;

    thread_local! {
        static IDENTITY: Cell<Option<Fingerprint>> = const { Cell::new(None) };
    }

    pub(super) fn fingerprint() -> std::io::Result<Fingerprint> {
        IDENTITY.with(|identity| {
            if let Some(value) = identity.get() {
                return Ok(value);
            }

            let window = web_sys::window()
                .ok_or_else(|| std::io::Error::other("browser Window is unavailable"))?;

            let storage = window.local_storage().ok().flatten();

            let stored = storage
                .as_ref()
                .and_then(|storage| storage.get_item("nwyd.browser-device.v1").ok().flatten());

            let mut bytes = [0u8; 22];

            let valid = stored
                .as_deref()
                .filter(|value| value.len() == 44 && value.is_ascii())
                .and_then(|value| {
                    (0..22)
                        .map(|i| u8::from_str_radix(&value[i * 2..i * 2 + 2], 16))
                        .collect::<Result<Vec<_>, _>>()
                        .ok()
                });

            if let Some(saved) = valid {
                bytes.copy_from_slice(&saved);
            } else {
                window
                    .crypto()
                    .and_then(|crypto| crypto.get_random_values_with_u8_array(&mut bytes))
                    .map_err(|_| {
                        std::io::Error::other("browser could not create a device identity")
                    })?;

                bytes[6] = (bytes[6] & 0x0f) | 0x40;

                bytes[8] = (bytes[8] & 0x3f) | 0x80;

                bytes[16] = (bytes[16] & 0xfc) | 0x02;

                if let Some(storage) = storage {
                    let value = bytes
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>();

                    let _ = storage.set_item("nwyd.browser-device.v1", &value);
                }
            }

            let fingerprint = Fingerprint {
                guid: std::array::from_fn(|i| {
                    u32::from_be_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                }),
                mac: bytes[16..].try_into().unwrap(),
            };

            identity.set(Some(fingerprint));

            Ok(fingerprint)
        })
    }
}

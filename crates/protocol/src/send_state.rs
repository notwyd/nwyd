use formats::net::client_config::{Config, normalize_chat};
use std::sync::{Arc, Mutex};

pub type SharedSender = Arc<Mutex<Sender>>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Profile {
    BuildC9D8,
    #[default]
    BuildC9D9,
}

impl Profile {
    pub const fn from_build_word(word: u32) -> Option<Self> {
        match word {
            0xc9d8 => Some(Self::BuildC9D8),
            0xc9d9 => Some(Self::BuildC9D9),
            _ => None,
        }
    }

    pub const fn build_word(self) -> u32 {
        match self {
            Self::BuildC9D8 => 0xc9d8,
            Self::BuildC9D9 => 0xc9d9,
        }
    }

    pub const fn login_version(self) -> i32 {
        (self.build_word().wrapping_shl(12).wrapping_add(0x301)) as i32
    }
}

pub struct Sender {
    profile: Profile,
    account: [u8; 16],
    baseline: u32,
    pub config: Arc<Config>,
    pub last_tick: u32,
    notice: Option<usize>,
}

impl Default for Sender {
    fn default() -> Self {
        Self::new(Profile::default())
    }
}

impl Sender {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            account: [0; 16],
            baseline: 0,
            config: Arc::default(),
            last_tick: 0,
            notice: None,
        }
    }

    pub const fn profile(&self) -> Profile {
        self.profile
    }

    pub fn prepare(&mut self, frame: &mut [u8], tick: u32) -> u32 {
        if let Some(limit) = normalize_chat(frame, &self.config) {
            self.notice = Some(limit);
        }

        let login =
            frame.len() >= 40 && matches!(u16::from_le_bytes([frame[4], frame[5]]), 0x20c | 0x20d);

        if login {
            self.account.fill(0);

            for (dst, &src) in self.account.iter_mut().zip(&frame[24..40]) {
                if src == 0 {
                    break;
                }

                *dst = src;
            }
        }

        let tick = if self.account[0] != 0 {
            let tick = encode_account_tick(self.profile, tick, &self.account, self.baseline);

            if login {
                self.baseline = tick;
            }

            tick
        } else {
            tick
        };

        self.last_tick = tick;

        tick
    }

    pub fn take_notice(&mut self) -> Option<usize> {
        self.notice.take()
    }
}

fn encode_account_tick(profile: Profile, tick: u32, account: &[u8; 16], baseline: u32) -> u32 {
    let mut hash = match profile {
        Profile::BuildC9D8 => ((tick ^ 0xb77d3257) & !1)
            .rotate_right(4)
            .wrapping_mul(0x6024eddd),
        Profile::BuildC9D9 => baseline
            .wrapping_add(0x813de8fa)
            .rotate_right(7)
            .wrapping_mul(0xff0af98d),
    };

    let start = if account[0].eq_ignore_ascii_case(&b'V') && account[1] == b'_' {
        2
    } else {
        0
    };

    for byte in account[start..14].iter().copied().take_while(|&b| b != 0) {
        hash = (hash ^ u32::from(byte.to_ascii_uppercase())).wrapping_mul(0x01000193);
    }

    hash = match profile {
        Profile::BuildC9D8 => {
            hash ^= baseline.wrapping_add(0xede6b3ec);
            hash = hash.rotate_right(12).wrapping_mul(0xb1fbfcf1);
            hash = (hash ^ (hash >> 15)).wrapping_mul(0x27964caf);
            hash = (hash ^ (hash >> 17)).wrapping_mul(0x06f40df3);
            hash = (hash ^ (hash >> 17)).wrapping_mul(0xe42fd6f5);
            hash ^ (hash >> 15)
        }
        Profile::BuildC9D9 => {
            hash ^= tick & !1;
            hash =
                (hash.rotate_right(6).wrapping_mul(0xf805ea6f) ^ 0xc9d9).wrapping_mul(0xc6d70d2d);
            hash = (hash ^ (hash >> 15)).wrapping_mul(0xf7620a55);
            hash = (hash ^ (hash >> 15)).wrapping_mul(0xc60d5e45);
            hash = (hash ^ (hash >> 18)).wrapping_mul(0x15ad3e71);
            hash ^ (hash >> 18)
        }
    };

    tick ^ ((hash ^ tick) & 1)
}

#[cfg(test)]
mod vectors;

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(opcode: u16, account: &[u8]) -> [u8; 40] {
        let mut frame = [0; 40];
        frame[4..6].copy_from_slice(&opcode.to_le_bytes());
        frame[24..24 + account.len()].copy_from_slice(account);
        frame
    }

    #[test]
    fn profiles_pair_build_words_with_login_versions() {
        for (profile, word, version) in [
            (Profile::BuildC9D8, 0xc9d8, 0x0c9d8301),
            (Profile::BuildC9D9, 0xc9d9, 0x0c9d9301),
        ] {
            assert_eq!(Profile::from_build_word(word), Some(profile));
            assert_eq!(profile.build_word(), word);
            assert_eq!(profile.login_version(), version);
            assert_eq!(Sender::new(profile).profile(), profile);
        }

        for word in [0, 735, 739, 0xc9d7, 0xc9da, 0x0c9d9301, u32::MAX] {
            assert_eq!(Profile::from_build_word(word), None);
        }

        assert_eq!(Profile::default(), Profile::BuildC9D9);
        assert_eq!(Sender::default().profile(), Profile::BuildC9D9);
    }

    #[test]
    fn dll_735_and_739_machine_vectors() {
        for &(tick, name, baseline, old, new) in vectors::CASES {
            let mut account = [0; 16];
            account[..name.len()].copy_from_slice(name);

            for (profile, expected) in [(Profile::BuildC9D8, old), (Profile::BuildC9D9, new)] {
                let actual = encode_account_tick(profile, tick, &account, baseline);
                assert_eq!(
                    actual, expected,
                    "{profile:?}, tick {tick:#x}, account {name:?}, baseline {baseline:#x}"
                );
                assert_eq!(actual & !1, tick & !1);
                assert_eq!(
                    encode_account_tick(profile, actual, &account, baseline),
                    actual
                );
            }
        }
    }

    #[test]
    fn shared_sender_retains_state_between_connections_and_logins() {
        for (profile, expected) in [
            (
                Profile::BuildC9D8,
                [
                    1234,
                    1002,
                    1004,
                    u32::MAX,
                    u32::MAX,
                    100,
                    101,
                    102,
                    105,
                    107,
                ],
            ),
            (
                Profile::BuildC9D9,
                [
                    1234,
                    1003,
                    1005,
                    u32::MAX,
                    u32::MAX - 1,
                    101,
                    101,
                    102,
                    104,
                    106,
                ],
            ),
        ] {
            let sender: SharedSender = Arc::new(Mutex::new(Sender::new(profile)));
            let frames: &[(u16, &[u8], u32)] = &[
                (0x100, b"", 1234),
                (0x20c, b"Player", 1002),
                (0x100, b"", 1004),
                (0x20d, b"V_Ranger", u32::MAX - 1),
                (0x100, b"", u32::MAX),
                (0x20c, b"MiXeD", 100),
                (0x20c, b"", 101),
                (0x100, b"", 102),
                (0x20c, b"Player", 104),
                (0x100, b"", 106),
            ];

            for range in [0..3, 3..frames.len()] {
                let connection_sender = Arc::clone(&sender);

                for index in range {
                    let (opcode, account, tick) = frames[index];
                    let mut sender = connection_sender.lock().unwrap();
                    assert_eq!(
                        sender.prepare(&mut frame(opcode, account), tick),
                        expected[index],
                        "{profile:?}, frame {index}"
                    );
                    assert_eq!(sender.last_tick, expected[index]);
                }
            }

            let sender = sender.lock().unwrap();
            assert_eq!(sender.account, *b"Player\0\0\0\0\0\0\0\0\0\0");
            assert_eq!(sender.baseline, expected[8]);
            assert_eq!(sender.profile(), profile);
        }
    }

    #[test]
    fn login_capture_requires_complete_frame_and_clears_previous_account() {
        for profile in [Profile::BuildC9D8, Profile::BuildC9D9] {
            let mut sender = Sender::new(profile);
            let mut login = frame(0x20c, b"Player");
            assert_eq!(sender.prepare(&mut login[..39], 1002), 1002);
            assert_eq!(sender.account, [0; 16]);
            assert_eq!(sender.baseline, 0);

            sender.prepare(&mut frame(0x20c, b"abcdefghijklmnop"), 1002);
            assert_eq!(sender.account, *b"abcdefghijklmnop");

            sender.prepare(&mut frame(0x20d, b"ABC\0DEF"), 1004);
            assert_eq!(sender.account, *b"ABC\0\0\0\0\0\0\0\0\0\0\0\0\0");

            let baseline = sender.baseline;
            assert_eq!(sender.prepare(&mut frame(0x20c, b""), 1006), 1006);
            assert_eq!(sender.account, [0; 16]);
            assert_eq!(sender.baseline, baseline);
            assert_eq!(sender.prepare(&mut [], 1008), 1008);
        }
    }
}

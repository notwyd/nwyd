use formats::net::client_config::{Config, normalize_chat};
use std::sync::{Arc, Mutex};

pub type SharedSender = Arc<Mutex<Sender>>;

#[derive(Default)]
pub struct Sender {
    account: [u8; 16],
    baseline: u32,
    pub config: Arc<Config>,
    pub last_tick: u32,
    notice: Option<usize>,
}

impl Sender {
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
            let tick = encode_account_tick(tick, &self.account, self.baseline);

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

fn encode_account_tick(tick: u32, account: &[u8; 16], baseline: u32) -> u32 {
    let mut hash = ((tick ^ 0xb77d3257) & !1)
        .rotate_right(4)
        .wrapping_mul(0x6024eddd);

    let start = if account[0].eq_ignore_ascii_case(&b'V') && account[1] == b'_' {
        2
    } else {
        0
    };

    for byte in account[start..14].iter().copied().take_while(|&b| b != 0) {
        hash = (hash ^ u32::from(byte.to_ascii_uppercase())).wrapping_mul(0x01000193);
    }

    hash ^= baseline.wrapping_add(0xede6b3ec);

    hash = hash.rotate_right(12).wrapping_mul(0xb1fbfcf1);

    hash = (hash ^ (hash >> 15)).wrapping_mul(0x27964caf);

    hash = (hash ^ (hash >> 17)).wrapping_mul(0x06f40df3);

    hash = (hash ^ (hash >> 17)).wrapping_mul(0xe42fd6f5);

    tick ^ ((hash ^ (hash >> 15) ^ tick) & 1)
}

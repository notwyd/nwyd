#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Clock {
    pub year: i32,
    pub day: i32,
    pub queried_ms: u32,
    pub server_seconds: u32,
    pub received_raw_ms: u32,
}

impl Clock {
    pub fn seed(&mut self, seconds: u32, raw_ms: u32) {
        self.server_seconds = seconds;

        self.received_raw_ms = raw_ms;
    }

    pub fn receive(&mut self, text: &str) -> bool {
        if !text.starts_with("!#") {
            return false;
        }

        let bytes = formats::text::codepage::encode(text, formats::text::Codepage::Windows1252);

        let Some(digits) = bytes.get(2..7) else {
            return true;
        };

        let mut d = [0; 5];

        for i in 0..4 {
            d[i] = if digits[i].is_ascii_digit() {
                i32::from(digits[i] - b'0')
            } else {
                0
            };
        }

        d[4] = i32::from((i16::from(digits[4] as i8) - 48) as u16);

        self.year = d[0] * 10 + d[1];

        self.day = (d[2] * 10 + d[3]) * 10 + d[4];

        true
    }

    pub fn query_due(&mut self, now: u32) -> bool {
        if self.year == 0 && self.day == 0
            || self.queried_ms != 0 && self.queried_ms.wrapping_add(3_600_000) < now
        {
            self.queried_ms = now;

            true
        } else {
            false
        }
    }

    pub fn duration(&self, copied_raw: i32, live_raw: i32, seconds: i32, raw_ms: u32) -> Duration {
        if copied_raw > 20_000_000 {
            let elapsed = raw_ms.wrapping_sub(self.received_raw_ms) / 1000;

            return Duration::seconds(
                live_raw
                    .wrapping_sub(elapsed as i32)
                    .wrapping_sub(self.server_seconds as i32),
            );
        }

        if copied_raw >= 1_000_000 {
            let kind = live_raw / 1_000_000;

            if kind == 0 {
                return Duration::Empty;
            }

            let year = (live_raw % 1_000_000) / 10_000;

            let length = if year % 4 == 0 { 366 } else { 365 };

            let bonus: i32 = match kind {
                4 => 7,
                5 => 15,
                6 => 30,
                _ => 0,
            };

            return Duration::Unit {
                record: 291,
                value: bonus
                    .wrapping_sub(self.year.wrapping_sub(year).wrapping_mul(length))
                    .wrapping_sub(self.day)
                    .wrapping_add(live_raw % 10_000),
            };
        }

        Duration::seconds(seconds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration {
    Empty,
    Seconds(i32),
    Unit { record: usize, value: i32 },
}

impl Duration {
    pub fn compact_text(self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Seconds(value) => value.max(0).to_string(),
            Self::Unit { record, value } => {
                let unit = match record {
                    291 => "d",
                    292 => "H",
                    293 => "M",
                    _ => return String::new(),
                };

                format!("{value}{unit}")
            }
        }
    }

    fn seconds(value: i32) -> Self {
        if value > 86_400 {
            Self::Unit {
                record: 291,
                value: value / 86_400,
            }
        } else if value > 3600 {
            Self::Unit {
                record: 292,
                value: value / 3600,
            }
        } else if value > 600 {
            Self::Unit {
                record: 293,
                value: value / 60,
            }
        } else {
            Self::Seconds(value)
        }
    }

    pub fn text(self, strings: &ui::Strings) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Seconds(value) => format!("{value:>5}"),
            Self::Unit { record, value } => strings
                .translate(&format!("ss_{record}"))
                .replacen("%d", &value.to_string(), 1)
                .replace("%%", "%"),
        }
    }
}

pub fn query_frame(client_id: u16) -> Vec<u8> {
    let mut frame = vec![0; 160];

    protocol::header::set_opcode(&mut frame, 0x334);

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame[12..15].copy_from_slice(b"day");

    frame
}

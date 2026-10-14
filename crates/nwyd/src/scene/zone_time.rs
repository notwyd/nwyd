pub type Shared = std::rc::Rc<std::cell::Cell<Clock>>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Clock {
    pub hours: u16,
    pub minutes: u16,
    pub seconds: u16,
    pub received_ms: u32,
}

impl Clock {
    pub fn enter(&mut self, now: u32) -> (i64, bool) {
        let query = self.received_ms == 0 || self.received_ms.wrapping_add(60_000) < now;

        if self.received_ms == 0 {
            *self = Self {
                received_ms: now,
                ..Self::default()
            };
        }

        let elapsed = now.wrapping_sub(self.received_ms) / 1000;

        (
            i64::from(elapsed)
                + i64::from(self.seconds)
                + (i64::from(self.minutes) + i64::from(self.hours) * 60) * 60,
            query,
        )
    }

    pub fn receive(&mut self, text: &str, now: u32) -> bool {
        if !text.starts_with("!!") || text.starts_with("!!!!") {
            return false;
        }

        let bytes = formats::text::codepage::encode(text, formats::text::Codepage::Windows1252);

        let Some(digits) = bytes.get(2..8) else {
            return true;
        };

        let digit = |i: usize| {
            if digits[i].is_ascii_digit() {
                u16::from(digits[i] - b'0')
            } else {
                0
            }
        };

        *self = Self {
            hours: digit(0) * 10 + digit(1),
            minutes: digit(2) * 10 + digit(3),
            seconds: digit(4) * 10 + digit(5),
            received_ms: now,
        };

        true
    }
}

pub fn query_frame(client_id: u16) -> Vec<u8> {
    let mut frame = vec![0; 160];

    protocol::header::set_opcode(&mut frame, 0x334);

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame[12..15].copy_from_slice(b"nig");

    frame
}

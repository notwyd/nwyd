pub const TABLE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Schedule {
    pub keys: [u8; TABLE],
    pub index: usize,
    pub dynamic: u32,
}

impl Schedule {
    pub fn install(&mut self, keys: [u8; TABLE]) {
        self.keys = keys;

        self.index = 0;
    }

    pub fn active(&self) -> bool {
        self.keys[0] != 0
    }

    pub fn next_key(&mut self) -> Option<u8> {
        if !self.active() {
            return None;
        }

        let key = if self.index < TABLE {
            let key = (self.keys[self.index] as i8 as i32) ^ 0xFF;

            self.index += 1;

            key
        } else if self.dynamic != 0 {
            derive_dynamic(self.dynamic) ^ 0xFF
        } else if (self.keys[15] as i8) & 1 != 0 {
            let k = |i: usize| self.keys[i] as i8 as i32;

            (k(13) - k(9) + k(11) + 4) ^ 0xFF
        } else {
            let k = |i: usize| self.keys[i] as i8 as i32;

            (k(5) + k(3) + k(1) - 0x57) ^ 0xFF
        };

        match key {
            0 => None,
            key => Some(key as u8),
        }
    }

    pub fn inbound_is_checked(&self) -> bool {
        false
    }
}

pub fn derive_dynamic(d: u32) -> i32 {
    let b0 = d as u8 as i8;

    let b1 = (d >> 8) as u8 as i8;

    let b2 = (d >> 16) as u8 as i8;

    let b3 = (d >> 24) as u8 as i8;

    let m = match (b0, b3) {
        (0, 0) => 13i8,
        (0, _) => b3,
        _ => b0,
    };

    let r = (b2 as i32) - (b1 as i32) * (m as i32) + (b3 as i32);

    match r {
        0 => b0 as i32,
        r => r,
    }
}

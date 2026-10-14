use crate::error::{Kind, Result};

const KIND: Kind = Kind::Action;

pub const PAYLOAD_LEN: usize = 0x28;

pub const PATH_OFF: usize = 0x0C;

pub const PATH_LEN: usize = 24;

pub const PATH_MAX: usize = PATH_LEN - 1;

pub const POSITION_LIMIT: u16 = 5000;

pub const MIN_SPEED: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Order {
    Walk,
    Teleport,
    Act(i32),
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Action {
    pub x: i16,
    pub z: i16,
    pub kind: i32,
    pub speed: i32,
    pub path: [u8; PATH_LEN],
    pub dest_x: u16,
    pub dest_z: u16,
}

impl Default for Action {
    fn default() -> Action {
        Action {
            x: 0,
            z: 0,
            kind: 0,
            speed: 0,
            path: [0; PATH_LEN],
            dest_x: 0,
            dest_z: 0,
        }
    }
}

impl Action {
    pub fn order(&self) -> Order {
        match self.kind {
            0 | 2 => Order::Walk,
            7 => Order::Teleport,
            k if k > 0 => Order::Act(k),
            k => {
                debug_assert!(k < 0);

                Order::Nothing
            }
        }
    }

    pub fn dirs(&self) -> &[u8] {
        let end = self.path.iter().position(|b| *b == 0).unwrap_or(PATH_LEN);

        &self.path[..end]
    }

    pub fn from(&self) -> (i32, i32) {
        (i32::from(self.x), i32::from(self.z))
    }

    pub fn to(&self) -> (i32, i32) {
        (i32::from(self.dest_x), i32::from(self.dest_z))
    }

    pub fn tiles_per_second(&self) -> f32 {
        self.speed.max(MIN_SPEED) as f32
    }

    pub fn out_of_range(&self) -> bool {
        self.dest_x > POSITION_LIMIT
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; PAYLOAD_LEN];

        out[0x00..0x02].copy_from_slice(&self.x.to_le_bytes());

        out[0x02..0x04].copy_from_slice(&self.z.to_le_bytes());

        out[0x04..0x08].copy_from_slice(&self.kind.to_le_bytes());

        out[0x08..0x0C].copy_from_slice(&self.speed.to_le_bytes());

        out[PATH_OFF..PATH_OFF + PATH_LEN].copy_from_slice(&self.path);

        out[0x24..0x26].copy_from_slice(&self.dest_x.to_le_bytes());

        out[0x26..0x28].copy_from_slice(&self.dest_z.to_le_bytes());

        out
    }

    pub fn walk(from: (i32, i32), to: (i32, i32), speed: i32, dirs: &[u8]) -> Action {
        let mut path = [0u8; PATH_LEN];

        let take = dirs.len().min(PATH_MAX);

        path[..take].copy_from_slice(&dirs[..take]);

        Action {
            x: from.0 as i16,
            z: from.1 as i16,
            kind: 0,
            speed,
            path,
            dest_x: to.0 as u16,
            dest_z: to.1 as u16,
        }
    }
}

pub fn parse(payload: &[u8]) -> Result<Action> {
    let mut path = [0u8; PATH_LEN];

    for (i, byte) in path.iter_mut().enumerate() {
        *byte = crate::util::byte_at(payload, PATH_OFF + i, KIND)?;
    }

    Ok(Action {
        x: crate::util::u16_at(payload, 0x00, KIND)? as i16,
        z: crate::util::u16_at(payload, 0x02, KIND)? as i16,
        kind: crate::util::u32_at(payload, 0x04, KIND)? as i32,
        speed: crate::util::u32_at(payload, 0x08, KIND)? as i32,
        path,
        dest_x: crate::util::u16_at(payload, 0x24, KIND)?,
        dest_z: crate::util::u16_at(payload, 0x26, KIND)?,
    })
}

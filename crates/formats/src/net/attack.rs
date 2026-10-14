use crate::error::{Kind, Result};

const KIND: Kind = Kind::Attack;

pub const ATTACK: u16 = 0x39D;

pub const ATTACK_AREA: u16 = 0x39E;

pub const ATTACK_MANY: u16 = 0x367;

pub const ATTACK_FRAME_LEN: usize = 0x48;

pub const AREA_FRAME_LEN: usize = 0x50;

pub const MANY_FRAME_LEN: usize = 0xA8;

pub const HEADER_LEN: usize = 0x0C;

pub const OPCODE_OFF: usize = 0x04;

pub const ATTACK_PAYLOAD_LEN: usize = ATTACK_FRAME_LEN - HEADER_LEN;

pub const AREA_PAYLOAD_LEN: usize = AREA_FRAME_LEN - HEADER_LEN;

pub const MANY_PAYLOAD_LEN: usize = MANY_FRAME_LEN - HEADER_LEN;

pub const HITS: usize = 13;

pub const HITS_SHORT: usize = 1;

pub const HITS_AREA: usize = 2;

pub const TRAILING_ID: usize = 4;

pub const HIT_OFF: usize = 0x30;

pub const HIT_STRIDE: usize = 8;

pub const ATTACK_ACTIONS: [u8; 3] = [4, 5, 6];

pub const ACTION_NONE: u8 = 0xFF;

pub const FLAG_VOLLEY: u8 = 8;

pub const FLAG_TRAIL: u8 = 4;

pub const FLAG_CRITICAL: u8 = 1;

pub const SPLIT_BITS: [(u8, usize); 3] = [(8, 2), (0x10, 3), (0x20, 4)];

pub const FLAG_SECOND: u8 = 8;

pub const DAMAGE_NONE: i32 = -1;

pub const DAMAGE_UNKNOWN: i32 = -2;

pub const MISS: [i32; 2] = [-3, -4];

pub const SKILL_NONE: i16 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hit {
    pub entity: u16,
    pub damage: i32,
}

impl Hit {
    pub fn missed(&self) -> bool {
        MISS.contains(&self.damage)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attack {
    pub from: (u16, u16),
    pub to: (u16, u16),
    pub attacker: u16,
    pub skill: u16,
    pub action: u8,
    pub echo: bool,
    pub flags: u8,
    pub variant: u8,
    pub damage: i32,
    pub skill_id: i16,
    pub hits: [Hit; HITS],
    pub used: usize,
}

impl Default for Attack {
    fn default() -> Attack {
        Attack {
            from: (0, 0),
            to: (0, 0),
            attacker: 0,
            skill: 0,
            action: 0,
            echo: false,
            flags: 0,
            variant: 0,
            damage: DAMAGE_NONE,
            skill_id: SKILL_NONE,
            hits: [Hit::default(); HITS],
            used: 1,
        }
    }
}

pub fn hits_for(opcode: u16) -> usize {
    match opcode {
        ATTACK => HITS_SHORT,
        ATTACK_AREA => HITS_AREA,
        _ => HITS,
    }
}

pub fn frame_len(opcode: u16) -> usize {
    match opcode {
        ATTACK_AREA => AREA_FRAME_LEN,
        ATTACK_MANY => MANY_FRAME_LEN,
        _ => ATTACK_FRAME_LEN,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emission {
    Ordinary,
    Automatic { area_branch: bool },
}

impl Emission {
    pub fn frame_len(self, opcode: u16) -> usize {
        match (self, opcode) {
            (Self::Automatic { area_branch: false }, ATTACK) => MANY_FRAME_LEN,
            _ => frame_len(opcode),
        }
    }
}

impl Attack {
    pub fn landed(&self) -> &[Hit] {
        &self.hits[..self.used.min(HITS)]
    }

    pub fn critical(&self) -> bool {
        self.flags & FLAG_CRITICAL != 0
    }

    pub fn splits(&self) -> usize {
        SPLIT_BITS
            .iter()
            .find(|(bit, _)| self.flags & bit != 0)
            .map_or(1, |(_, count)| *count)
    }

    pub fn describe_hits(&self) -> String {
        let hits: Vec<String> = self
            .landed()
            .iter()
            .map(|hit| match hit.missed() {
                true => format!("{} missed", hit.entity),
                false => format!("{} for {}", hit.entity, hit.damage),
            })
            .collect();

        hits.join(", ")
    }

    pub fn to_bytes(&self, opcode: u16) -> Vec<u8> {
        self.to_bytes_for(opcode, Emission::Ordinary)
    }

    pub fn to_bytes_for(&self, opcode: u16, emission: Emission) -> Vec<u8> {
        let mut out = vec![0u8; emission.frame_len(opcode)];

        out[OPCODE_OFF..OPCODE_OFF + 2].copy_from_slice(&opcode.to_le_bytes());

        let payload = &mut out[HEADER_LEN..];

        payload[0x16..0x18].copy_from_slice(&self.from.0.to_le_bytes());

        payload[0x18..0x1A].copy_from_slice(&self.from.1.to_le_bytes());

        payload[0x1A..0x1C].copy_from_slice(&self.to.0.to_le_bytes());

        payload[0x1C..0x1E].copy_from_slice(&self.to.1.to_le_bytes());

        payload[0x1E..0x20].copy_from_slice(&self.attacker.to_le_bytes());

        payload[0x20..0x22].copy_from_slice(&self.skill.to_le_bytes());

        payload[0x22] = self.action;

        payload[0x23] = u8::from(self.echo);

        payload[0x24] = self.flags;

        payload[0x25] = self.variant;

        payload[0x28..0x2C].copy_from_slice(&self.damage.to_le_bytes());

        payload[0x2C..0x2E].copy_from_slice(&self.skill_id.to_le_bytes());

        for (i, hit) in self.hits[..hits_for(opcode).min(HITS)].iter().enumerate() {
            let at = HIT_OFF + i * HIT_STRIDE;

            payload[at..at + 2].copy_from_slice(&hit.entity.to_le_bytes());

            payload[at + 4..at + 8].copy_from_slice(&hit.damage.to_le_bytes());
        }

        out
    }
}

pub fn parse(opcode: u16, payload: &[u8]) -> Result<Attack> {
    let used = hits_for(opcode);

    let mut hits = [Hit::default(); HITS];

    for (i, hit) in hits[..used].iter_mut().enumerate() {
        let at = HIT_OFF + i * HIT_STRIDE;

        hit.entity = crate::util::u16_at(payload, at, KIND)?;

        hit.damage = crate::util::u32_at(payload, at + 4, KIND)? as i32;
    }

    Ok(Attack {
        from: (
            crate::util::u16_at(payload, 0x16, KIND)?,
            crate::util::u16_at(payload, 0x18, KIND)?,
        ),
        to: (
            crate::util::u16_at(payload, 0x1A, KIND)?,
            crate::util::u16_at(payload, 0x1C, KIND)?,
        ),
        attacker: crate::util::u16_at(payload, 0x1E, KIND)?,
        skill: crate::util::u16_at(payload, 0x20, KIND)?,
        action: crate::util::byte_at(payload, 0x22, KIND)?,
        echo: crate::util::byte_at(payload, 0x23, KIND)? != 0,
        flags: crate::util::byte_at(payload, 0x24, KIND)?,
        variant: crate::util::byte_at(payload, 0x25, KIND)?,
        damage: crate::util::u32_at(payload, 0x28, KIND)? as i32,
        skill_id: crate::util::u16_at(payload, 0x2C, KIND)? as i16,
        hits,
        used,
    })
}

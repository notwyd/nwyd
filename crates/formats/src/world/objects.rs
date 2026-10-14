use crate::error::{Error, Kind, Result};

const BASE_LEN: usize = 28;

const EXTENDED_LEN: usize = 36;

pub const MAX_RECORDS: usize = 4096;

pub fn is_extended(kind: u32) -> bool {
    matches!(kind, 0x1F5..= 0x1FA | 0x1FF..= 0x206 | 0x208..= 0x257)
}

pub const MAX_KIND: u32 = 3025;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectClass {
    StaticMesh,
    Sea,
    Suppressed,
    Grass,
    Tree,
    Butterfly,
    Fish,
    Prop,
    Ship,
    Particle,
    Light,
    Glow,
    Dust,
    Drop,
    House,
    Skipped,
}

impl ObjectClass {
    pub fn uses_mesh_list_kind(self) -> bool {
        matches!(
            self,
            ObjectClass::StaticMesh | ObjectClass::House | ObjectClass::Ship
        )
    }
}

pub fn house_type(kind: u32) -> Option<u8> {
    Some(match kind {
        0xFB..=0xFE | 0x6A0 => 0,
        0x1DA => 1,
        0xC3 | 0x111 | 0x112 | 0x2B9 | 0x2BB | 0x5F0 | 0x5FF | 0x69F | 0x7C9 => 2,
        0x124 | 0x1EA | 0x5F6 | 0x681 | 0x7D5 => 3,
        0x25F => 4,
        0x262 => 5,
        0x266 => 6,
        0x6D6 => 7,
        0x6CB => 8,
        0x6AF => 9,
        0x73F => 0xB,
        _ => return None,
    })
}

pub fn house_leaf_id(kind: u32) -> u16 {
    match kind {
        0x111 => 0x118,
        0x112 => 0x119,
        0xC3 | 0x124 | 0x1EA | 0x2B9 | 0x2BB | 0x5F0 | 0x5F6 | 0x5FF | 0x681 | 0x69F | 0x7C9
        | 0x7D5 => (kind + 1) as u16,
        _ => 0x118,
    }
}

pub fn classify(kind: u32) -> ObjectClass {
    match kind {
        0x79 => ObjectClass::Skipped,
        2 => ObjectClass::Sea,
        8 | 9 | 0x0D | 0x213 => ObjectClass::Dust,
        0x0A => ObjectClass::Drop,
        3 | 5 => ObjectClass::Prop,
        4 | 6 | 7 | 0x157 => ObjectClass::Butterfly,
        0x0C | 0x158 => ObjectClass::Fish,
        0x137..=0x142 => ObjectClass::Grass,
        0x14B..=0x156 | 0x15F..=0x17A => ObjectClass::Tree,
        0x291 | 0x292 => ObjectClass::Suppressed,
        0x1E7..=0x1E9 => ObjectClass::Ship,
        0x1F5..=0x1F9 => ObjectClass::Particle,
        0x1FA | 0x1FF..=0x206 => ObjectClass::Light,
        0x208..=0x212 | 0x214 | 0x7BC => ObjectClass::Glow,
        0xC3
        | 0xFB..=0xFE
        | 0x111
        | 0x112
        | 0x124
        | 0x1DA
        | 0x1EA
        | 0x25F
        | 0x262
        | 0x266
        | 0x2B9
        | 0x2BB
        | 0x5F0
        | 0x5F6
        | 0x5FF
        | 0x681
        | 0x69F
        | 0x6A0
        | 0x6AF
        | 0x6CB
        | 0x6D6
        | 0x73F
        | 0x7C9
        | 0x7D5 => ObjectClass::House,
        _ => ObjectClass::StaticMesh,
    }
}

impl ObjRecord {
    pub fn class(&self) -> ObjectClass {
        classify(self.kind)
    }
}

impl ObjRecord {
    pub fn is_plausible(&self) -> bool {
        self.kind <= MAX_KIND
            && (-20.0..=140.0).contains(&self.x)
            && (-20.0..=140.0).contains(&self.z)
            && (-40.0..=50.0).contains(&self.y)
            && self.yaw.is_finite()
            && self.yaw.abs() <= 20.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjRecord {
    pub kind: u32,
    pub x: f32,
    pub z: f32,
    pub y: f32,
    pub yaw: f32,
    pub p0: i32,
    pub p1: i32,
    pub extra: Option<[f32; 2]>,
}

impl ObjRecord {
    pub fn encoded_len(&self) -> usize {
        if self.extra.is_some() {
            EXTENDED_LEN
        } else {
            BASE_LEN
        }
    }

    pub fn world_position(&self, zone_x: u32, zone_y: u32) -> [f32; 3] {
        let (ox, oz) = super::zone_origin(zone_x, zone_y);

        [self.x + ox, self.y, self.z + oz]
    }

    pub fn mask_id(&self) -> usize {
        self.p1.max(0) as usize
    }
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::ObjectStream,
        message: format!("Field*.dat: {}", message.into()),
    }
}

#[inline]
fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(b[off..off + 4].try_into().expect("caller checked bounds"))
}

#[inline]
fn i32_at(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().expect("caller checked bounds"))
}

pub fn parse(bytes: &[u8]) -> Result<Vec<ObjRecord>> {
    let mut out = Vec::new();

    let mut off = 0usize;

    while off < bytes.len() {
        if out.len() >= MAX_RECORDS {
            break;
        }

        let remaining = bytes.len() - off;

        if remaining < BASE_LEN {
            return Err(err(format!(
                "{remaining} trailing bytes at offset {off} after {} records — too short for a record",
                out.len()
            )));
        }

        let r = &bytes[off..];

        let kind = u32::from_le_bytes(r[0..4].try_into().expect("checked"));

        let extended = is_extended(kind);

        let len = if extended { EXTENDED_LEN } else { BASE_LEN };

        if remaining < len {
            return Err(err(format!(
                "kind {kind:#x} at offset {off} needs {len} bytes but only {remaining} remain"
            )));
        }

        out.push(ObjRecord {
            kind,
            x: f32_at(r, 0x04),
            z: f32_at(r, 0x08),
            y: f32_at(r, 0x0C),
            yaw: f32_at(r, 0x10),
            p0: i32_at(r, 0x14),
            p1: i32_at(r, 0x18),
            extra: extended.then(|| [f32_at(r, 0x1C), f32_at(r, 0x20)]),
        });

        off += len;
    }

    Ok(out)
}

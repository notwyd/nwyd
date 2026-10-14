use super::envelope::{self, Integrity};
use crate::data::item_effect::Effects;
use crate::error::{Kind, Result};
use crate::net::account::Item;
use crate::text::{Codepage, codepage};

pub const RECORDS: usize = 20;

pub const RECORD_LEN: usize = 908;

pub const BODY_LEN: usize = RECORDS * RECORD_LEN;

pub const CHECKSUM: i32 = 0x000F_1D62;

pub const SLOT_NAMES: usize = 52;

pub const NAME_LEN: usize = 16;

pub const EQUIP_SLOTS: usize = 11;

const OFF_NAMES: usize = 0x004;

const OFF_EQUIP: usize = 0x344;

const OFF_COLOR: usize = 0x370;

const OFF_HEADS: usize = 0x374;

const OFF_MOB_CLASS: usize = 0x384;

const OFF_MOB_TEMPLATE: usize = 0x388;

pub const EQUIP_TYPE_CODES: [i32; EQUIP_SLOTS] = [
    0, 0x0B, 0x65, 0x29, 0x15, 0x01, 0x67, 0x1F, 0x20, 0x03, 0x0D,
];

pub const HEAD_MODELS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub id: u32,
    pub raw_names: Vec<Vec<u8>>,
    pub equip_override: [u32; EQUIP_SLOTS],
    pub color_effect: u32,
    pub head_by_class: [u32; HEAD_MODELS],
    pub mob_class: i32,
    pub mob_template_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketPatch {
    pub hands: [Option<(u16, u8)>; 2],
    pub costume: Option<u16>,
    pub mount_refine: Option<u8>,
}

impl Record {
    pub fn packet_patch(&self, hands: [u16; 2], class: i32, effects: &Effects<'_>) -> PacketPatch {
        let hands = hands.map(|index| {
            if index == 0 || self.equip_override[0] == 0 {
                return None;
            }

            let family = effects.total_scaled(
                &Item {
                    index,
                    ..Item::default()
                },
                0x15,
            );

            Some((
                self.packet_hand_override(family).unwrap_or(index),
                self.color_effect as u8,
            ))
        });

        PacketPatch {
            hands,
            costume: (self.head_by_class[0] != 0)
                .then(|| self.head_for_class_family(class as usize) as u16),
            mount_refine: (self.mob_class != 0).then_some(self.mob_class as u8),
        }
    }

    pub fn name(&self, slot: usize, cp: Codepage) -> Option<String> {
        self.raw_names.get(slot).map(|n| codepage::decode(n, cp))
    }

    pub fn equip_override_for(&self, type_code: i32) -> Option<u32> {
        let slot = EQUIP_TYPE_CODES.iter().position(|&c| c == type_code)?;

        Some(self.equip_override[slot]).filter(|&v| v != 0)
    }

    pub fn packet_hand_override(&self, type_code: i32) -> Option<u16> {
        if self.equip_override[0] == 0 {
            return None;
        }

        let slot = EQUIP_TYPE_CODES.iter().position(|&c| c == type_code)?;

        Some(self.equip_override[slot] as u16)
    }

    pub fn head_for_class_family(&self, family: usize) -> u32 {
        self.head_by_class[if family < HEAD_MODELS { family } else { 0 }]
    }

    pub fn is_blank(&self) -> bool {
        self.id == 0 && self.raw_names.iter().all(|n| n.is_empty())
    }

    pub fn named_slots(&self) -> usize {
        self.raw_names.iter().filter(|n| !n.is_empty()).count()
    }
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Record>> {
    let body = envelope::open(
        bytes,
        BODY_LEN,
        Integrity::ConstantA(CHECKSUM),
        Kind::ArenaVisualData,
    )?;

    Ok(parse_body(&body))
}

fn parse_body(body: &[u8]) -> Vec<Record> {
    (0..RECORDS)
        .map(|i| {
            let r = &body[i * RECORD_LEN..(i + 1) * RECORD_LEN];

            let u32_at =
                |off: usize| u32::from_le_bytes(r[off..off + 4].try_into().expect("in record"));

            let raw_names = (0..SLOT_NAMES)
                .map(|n| {
                    let f = &r[OFF_NAMES + n * NAME_LEN..OFF_NAMES + (n + 1) * NAME_LEN];

                    let end = f.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);

                    f[..end].to_vec()
                })
                .collect();

            let mut equip_override = [0u32; EQUIP_SLOTS];

            for (s, v) in equip_override.iter_mut().enumerate() {
                *v = u32_at(OFF_EQUIP + s * 4);
            }

            let mut head_by_class = [0u32; HEAD_MODELS];

            for (s, v) in head_by_class.iter_mut().enumerate() {
                *v = u32_at(OFF_HEADS + s * 4);
            }

            Record {
                id: u32_at(0),
                raw_names,
                equip_override,
                color_effect: u32_at(OFF_COLOR),
                head_by_class,
                mob_class: u32_at(OFF_MOB_CLASS) as i32,
                mob_template_id: u32_at(OFF_MOB_TEMPLATE),
            }
        })
        .collect()
}

pub fn lookup(table: &[Record], arena: u32) -> Option<&Record> {
    if arena == 0 || arena >= 20 {
        return None;
    }

    table.get(arena as usize).filter(|r| r.id == arena)
}

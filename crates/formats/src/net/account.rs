use crate::error::{Error, Kind, Result};
use crate::text::{Codepage, codepage};
use crate::util::{u16_at, u32_at};

const KIND: Kind = Kind::AccountInfo;

pub const SLOTS: usize = 4;

pub const KEY_LEN: usize = 16;

pub const NAME_LEN: usize = 16;

pub const ITEM_LEN: usize = 12;

pub const EQUIP_SLOTS: usize = 18;

pub const EQUIP_SHOWN: usize = 17;

pub const EQUIP_FIRST_SHOWN: usize = 1;

pub const STATUS_LEN: usize = 0x30;

pub const EQUIP_LEN: usize = 0xD8;

pub const BLOCK_LEN: usize = 0x4B8;

pub const CARGO_SLOTS: usize = 128;

pub const CARGO_LEN: usize = CARGO_SLOTS * ITEM_LEN;

pub const EVOLVED_BIT: u32 = 0x4000_0000;

pub const MOUNT_ID_MAX: i32 = 14_499;

const PAD_OFF: usize = KEY_LEN;

const PAD_LEN: usize = 4;

const BLOCK_OFF: usize = PAD_OFF + PAD_LEN;

const CARGO_OFF: usize = BLOCK_OFF + BLOCK_LEN;

const EXTRA_OFF: usize = CARGO_OFF + CARGO_LEN;

const ACCOUNT_OFF: usize = EXTRA_OFF + 4;

const TAIL_OFF: usize = ACCOUNT_OFF + NAME_LEN;

const REFRESH_BLOCK_OFF: usize = 4;

const X_OFF: usize = 0x000;

const Z_OFF: usize = 0x008;

const NAME_OFF: usize = 0x010;

const STATUS_OFF: usize = 0x050;

const EQUIP_OFF: usize = 0x110;

const GUILD_OFF: usize = 0x470;

const GOLD_OFF: usize = 0x478;

const EXP_OFF: usize = 0x488;

const UNKNOWN_4A8_OFF: usize = 0x4A8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Effect {
    pub id: u8,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Item {
    pub index: u16,
    pub appearance: u16,
    pub reserved: u16,
    pub effects: [Effect; 3],
}

impl Item {
    pub fn is_empty(&self) -> bool {
        self.index == 0
    }

    pub fn refine(&self) -> Option<u8> {
        self.effects.iter().find(|e| e.id == 0x2B).map(|e| e.value)
    }

    pub fn to_bytes(self) -> [u8; ITEM_LEN] {
        let mut out = [0u8; ITEM_LEN];

        out[0..2].copy_from_slice(&self.index.to_le_bytes());

        out[2..4].copy_from_slice(&self.appearance.to_le_bytes());

        out[4..6].copy_from_slice(&self.reserved.to_le_bytes());

        for (n, effect) in self.effects.iter().enumerate() {
            out[6 + n * 2] = effect.id;

            out[7 + n * 2] = effect.value;
        }

        out
    }
}

pub(crate) fn item_at(buf: &[u8], off: usize) -> Result<Item> {
    let mut effects = [Effect::default(); 3];

    for (n, effect) in effects.iter_mut().enumerate() {
        *effect = Effect {
            id: crate::util::byte_at(buf, off + 6 + n * 2, KIND)?,
            value: crate::util::byte_at(buf, off + 7 + n * 2, KIND)?,
        };
    }

    Ok(Item {
        index: u16_at(buf, off, KIND)?,
        appearance: u16_at(buf, off + 2, KIND)?,
        reserved: u16_at(buf, off + 4, KIND)?,
        effects,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Status {
    pub level: u16,
    pub unset_02: u16,
    pub defense: u32,
    pub attack_damage: u32,
    pub flags: u32,
    pub max_hp: u32,
    pub max_mp: u32,
    pub hp: u32,
    pub mp: u32,
    pub stat: [u16; 4],
    pub mastery: [u16; 4],
}

impl Status {
    pub fn shown_level(&self) -> u32 {
        u32::from(self.level) + 1
    }

    pub fn skill_mastery(&self, group: usize) -> Option<u16> {
        self.mastery.get(group + 1).copied()
    }

    pub fn to_bytes(self) -> [u8; STATUS_LEN] {
        let mut out = [0u8; STATUS_LEN];

        out[0x00..0x02].copy_from_slice(&self.level.to_le_bytes());

        out[0x02..0x04].copy_from_slice(&self.unset_02.to_le_bytes());

        out[0x04..0x08].copy_from_slice(&self.defense.to_le_bytes());

        out[0x08..0x0C].copy_from_slice(&self.attack_damage.to_le_bytes());

        out[0x0C..0x10].copy_from_slice(&self.flags.to_le_bytes());

        out[0x10..0x14].copy_from_slice(&self.max_hp.to_le_bytes());

        out[0x14..0x18].copy_from_slice(&self.max_mp.to_le_bytes());

        out[0x18..0x1C].copy_from_slice(&self.hp.to_le_bytes());

        out[0x1C..0x20].copy_from_slice(&self.mp.to_le_bytes());

        for (n, value) in self.stat.iter().enumerate() {
            out[0x20 + n * 2..0x22 + n * 2].copy_from_slice(&value.to_le_bytes());
        }

        for (n, value) in self.mastery.iter().enumerate() {
            out[0x28 + n * 2..0x2A + n * 2].copy_from_slice(&value.to_le_bytes());
        }

        out
    }
}

pub(crate) fn status_at(buf: &[u8], off: usize) -> Result<Status> {
    let mut stat = [0u16; 4];

    let mut mastery = [0u16; 4];

    for (n, value) in stat.iter_mut().enumerate() {
        *value = u16_at(buf, off + 0x20 + n * 2, KIND)?;
    }

    for (n, value) in mastery.iter_mut().enumerate() {
        *value = u16_at(buf, off + 0x28 + n * 2, KIND)?;
    }

    Ok(Status {
        level: u16_at(buf, off, KIND)?,
        unset_02: u16_at(buf, off + 0x02, KIND)?,
        defense: u32_at(buf, off + 0x04, KIND)?,
        attack_damage: u32_at(buf, off + 0x08, KIND)?,
        flags: u32_at(buf, off + 0x0C, KIND)?,
        max_hp: u32_at(buf, off + 0x10, KIND)?,
        max_mp: u32_at(buf, off + 0x14, KIND)?,
        hp: u32_at(buf, off + 0x18, KIND)?,
        mp: u32_at(buf, off + 0x1C, KIND)?,
        stat,
        mastery,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Equipment {
    pub items: [Item; EQUIP_SLOTS],
}

impl Default for Equipment {
    fn default() -> Equipment {
        Equipment {
            items: [Item::default(); EQUIP_SLOTS],
        }
    }
}

impl Equipment {
    pub fn body(&self) -> Item {
        self.items[0]
    }

    pub fn costume(&self) -> Item {
        self.items[12]
    }

    pub fn mount(&self) -> Item {
        self.items[14]
    }

    pub fn mount_id(&self) -> i32 {
        i32::from(self.mount().index as i16)
    }

    pub fn mount_in_item_list(&self) -> bool {
        (1..=MOUNT_ID_MAX).contains(&self.mount_id())
    }

    pub fn mount_maturity(&self) -> u8 {
        self.mount().effects[1].id
    }

    pub fn evolved(&self) -> bool {
        self.body().effects[1].value != 0
    }

    pub fn cape(&self) -> Item {
        self.items[15]
    }

    pub fn class(&self) -> u8 {
        let body = i32::from(self.body().index as i16);

        let r = body % 10;

        let class = if r == 1 {
            body / 10
        } else if r < 6 {
            return 2;
        } else {
            r - 6
        };

        if (0..=3).contains(&class) {
            class as u8
        } else {
            0
        }
    }

    pub fn is_monster(&self) -> bool {
        (self.body().index as i16) >= 0x28
    }

    pub fn is_transcendent(&self) -> bool {
        matches!(
            self.cape().index,
            0xC7D | 0xC7E | 0xC7F | 0x23D | 0x6E7 | 0x6EA | 0x192F | 0x1930 | 0x1931
        )
    }

    pub fn to_bytes(&self) -> [u8; EQUIP_LEN] {
        let mut out = [0u8; EQUIP_LEN];

        for (n, item) in self.items.iter().enumerate() {
            let at = n * ITEM_LEN;

            out[at..at + ITEM_LEN].copy_from_slice(&item.to_bytes());
        }

        out
    }
}

pub(crate) fn equipment_at(buf: &[u8], off: usize) -> Result<Equipment> {
    let mut items = [Item::default(); EQUIP_SLOTS];

    for (n, item) in items.iter_mut().enumerate() {
        *item = item_at(buf, off + n * ITEM_LEN)?;
    }

    Ok(Equipment { items })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Slot {
    pub x: u16,
    pub z: u16,
    pub name: [u8; NAME_LEN],
    pub status: Status,
    pub equipment: Equipment,
    pub guild: u16,
    pub gold: u32,
    pub exp: u64,
    pub unknown_4a8: u32,
}

impl Slot {
    pub fn is_empty(&self) -> bool {
        self.name[0] == 0
    }

    pub fn name(&self) -> String {
        decode_name(&self.name)
    }
}

fn decode_name(raw: &[u8; NAME_LEN]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);

    codepage::decode(&raw[..end], Codepage::Windows1252)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Block {
    pub slots: [Slot; SLOTS],
}

impl Block {
    pub fn first_free(&self) -> Option<usize> {
        self.slots.iter().position(Slot::is_empty)
    }

    pub fn occupied(&self) -> usize {
        self.slots.iter().filter(|s| !s.is_empty()).count()
    }

    pub fn to_bytes(&self) -> [u8; BLOCK_LEN] {
        let mut out = [0u8; BLOCK_LEN];

        for (i, slot) in self.slots.iter().enumerate() {
            out[X_OFF + i * 2..X_OFF + i * 2 + 2].copy_from_slice(&slot.x.to_le_bytes());

            out[Z_OFF + i * 2..Z_OFF + i * 2 + 2].copy_from_slice(&slot.z.to_le_bytes());

            let at = NAME_OFF + i * NAME_LEN;

            out[at..at + NAME_LEN].copy_from_slice(&slot.name);

            let at = STATUS_OFF + i * STATUS_LEN;

            out[at..at + STATUS_LEN].copy_from_slice(&slot.status.to_bytes());

            let at = EQUIP_OFF + i * EQUIP_LEN;

            out[at..at + EQUIP_LEN].copy_from_slice(&slot.equipment.to_bytes());

            let at = GUILD_OFF + i * 2;

            out[at..at + 2].copy_from_slice(&slot.guild.to_le_bytes());

            let at = GOLD_OFF + i * 4;

            out[at..at + 4].copy_from_slice(&slot.gold.to_le_bytes());

            let at = EXP_OFF + i * 8;

            out[at..at + 8].copy_from_slice(&slot.exp.to_le_bytes());

            let at = UNKNOWN_4A8_OFF + i * 4;

            out[at..at + 4].copy_from_slice(&slot.unknown_4a8.to_le_bytes());
        }

        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accept {
    pub key: [u8; KEY_LEN],
    pub pad: [u8; 4],
    pub block: Block,
    pub cargo: [Item; CARGO_SLOTS],
    pub extra: u32,
    pub account: [u8; NAME_LEN],
    pub tail: Vec<u8>,
}

impl Accept {
    pub fn account(&self) -> String {
        decode_name(&self.account)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; TAIL_OFF];

        out[0..KEY_LEN].copy_from_slice(&self.key);

        out[PAD_OFF..PAD_OFF + PAD_LEN].copy_from_slice(&self.pad);

        out[BLOCK_OFF..CARGO_OFF].copy_from_slice(&self.block.to_bytes());

        for (n, item) in self.cargo.iter().enumerate() {
            let at = CARGO_OFF + n * ITEM_LEN;

            out[at..at + ITEM_LEN].copy_from_slice(&item.to_bytes());
        }

        out[EXTRA_OFF..EXTRA_OFF + 4].copy_from_slice(&self.extra.to_le_bytes());

        out[ACCOUNT_OFF..TAIL_OFF].copy_from_slice(&self.account);

        out.extend_from_slice(&self.tail);

        out
    }

    pub fn refresh_payload(block: &Block) -> Vec<u8> {
        let mut out = vec![0u8; REFRESH_BLOCK_OFF];

        out.extend_from_slice(&block.to_bytes());

        out
    }
}

pub fn parse_slot_effect(payload: &[u8]) -> Result<u32> {
    u32_at(payload, 0, KIND)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteHold {
    pub deadline: i32,
    pub current: i32,
}

pub fn parse_delete_hold(payload: &[u8]) -> Result<DeleteHold> {
    Ok(DeleteHold {
        deadline: u32_at(payload, 0, KIND)? as i32,
        current: u32_at(payload, 4, KIND)? as i32,
    })
}

impl DeleteHold {
    pub fn remaining(self) -> [i64; 3] {
        let seconds = (i64::from(self.deadline) - i64::from(self.current)).max(0);

        [seconds / 86400, seconds / 3600 % 24, seconds / 60 % 60]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionMetadata {
    pub index: u32,
    pub words: [u32; 3],
}

pub fn parse_selection_metadata(payload: &[u8]) -> Result<SelectionMetadata> {
    Ok(SelectionMetadata {
        index: u32_at(payload, 0, KIND)?,
        words: [
            u32_at(payload, 4, KIND)?,
            u32_at(payload, 8, KIND)?,
            u32_at(payload, 12, KIND)?,
        ],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameResult {
    pub result: i32,
    pub slot: i32,
}

pub fn parse_name_result(payload: &[u8]) -> Result<NameResult> {
    if payload.len() != 40 {
        return Err(Error::structure(KIND, "0xFAA requires a40-byte payload"));
    }

    Ok(NameResult {
        result: u32_at(payload, 0, KIND)? as i32,
        slot: u32_at(payload, 4, KIND)? as i32,
    })
}

pub fn parse_accept(payload: &[u8]) -> Result<Accept> {
    if payload.len() < TAIL_OFF {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: TAIL_OFF,
            len: payload.len(),
        });
    }

    let mut cargo = [Item::default(); CARGO_SLOTS];

    for (n, item) in cargo.iter_mut().enumerate() {
        *item = item_at(payload, CARGO_OFF + n * ITEM_LEN)?;
    }

    Ok(Accept {
        key: fixed(payload, 0)?,
        pad: fixed(payload, PAD_OFF)?,
        block: block_at(payload, BLOCK_OFF)?,
        cargo,
        extra: u32_at(payload, EXTRA_OFF, KIND)?,
        account: fixed(payload, ACCOUNT_OFF)?,
        tail: payload[TAIL_OFF..].to_vec(),
    })
}

pub fn parse_refresh(payload: &[u8]) -> Result<Block> {
    block_at(payload, REFRESH_BLOCK_OFF)
}

fn block_at(buf: &[u8], base: usize) -> Result<Block> {
    let end = base.checked_add(BLOCK_LEN).ok_or(Error::Truncated {
        kind: KIND,
        offset: base,
        needed: BLOCK_LEN,
        len: buf.len(),
    })?;

    if buf.len() < end {
        return Err(Error::Truncated {
            kind: KIND,
            offset: base,
            needed: BLOCK_LEN,
            len: buf.len(),
        });
    }

    let mut slots = [Slot::default(); SLOTS];

    for (i, slot) in slots.iter_mut().enumerate() {
        *slot = Slot {
            x: u16_at(buf, base + X_OFF + i * 2, KIND)?,
            z: u16_at(buf, base + Z_OFF + i * 2, KIND)?,
            name: fixed(buf, base + NAME_OFF + i * NAME_LEN)?,
            status: status_at(buf, base + STATUS_OFF + i * STATUS_LEN)?,
            equipment: equipment_at(buf, base + EQUIP_OFF + i * EQUIP_LEN)?,
            guild: u16_at(buf, base + GUILD_OFF + i * 2, KIND)?,
            gold: u32_at(buf, base + GOLD_OFF + i * 4, KIND)?,
            exp: u64_at(buf, base + EXP_OFF + i * 8)?,
            unknown_4a8: u32_at(buf, base + UNKNOWN_4A8_OFF + i * 4, KIND)?,
        };
    }

    Ok(Block { slots })
}

fn u64_at(buf: &[u8], off: usize) -> Result<u64> {
    let lo = u32_at(buf, off, KIND)?;

    let hi = u32_at(buf, off + 4, KIND)?;

    Ok(u64::from(lo) | (u64::from(hi) << 32))
}

pub(crate) fn fixed<const N: usize>(buf: &[u8], off: usize) -> Result<[u8; N]> {
    let end = off.checked_add(N).ok_or(Error::Truncated {
        kind: KIND,
        offset: off,
        needed: N,
        len: buf.len(),
    })?;

    let slice = buf.get(off..end).ok_or(Error::Truncated {
        kind: KIND,
        offset: off,
        needed: N,
        len: buf.len(),
    })?;

    Ok(slice.try_into().expect("slice is N bytes"))
}

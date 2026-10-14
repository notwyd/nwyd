use crate::error::{Error, Kind, Result};
use crate::net::account::{self, Equipment, ITEM_LEN, Item, NAME_LEN, Status};

const KIND: Kind = Kind::EnterWorld;

pub const MOB_LEN: usize = 0x558;

pub const CARRY_SLOTS: usize = 64;

pub const CARRY_SHOWN: usize = 60;

pub const BAG_SLOTS: [usize; 2] = [60, 61];

pub const BAG_EXTENSION_ITEM: u16 = 0xD8B;

pub const QUICK_SLOTS: usize = 20;

pub const QUICK_PAGES: usize = 2;

pub const QUICK_PAGE_SLOTS: usize = 10;

pub const QUICK_HEAD: usize = 4;

pub const QUICK_EMPTY: u8 = 0xFF;

pub const QUICK_CLASS_SPAN: u8 = 0x18;

pub const QUICK_BAR: u16 = 0x378;

pub const QUICK_BAR_FRAME_LEN: usize = 0x20;

pub const QUICK_BAR_PAYLOAD_LEN: usize = QUICK_BAR_FRAME_LEN - 0x0C;

pub const QUICK_SECOND_WINDOW: std::ops::RangeInclusive<u8> = 0x69..=0x74;

pub const QUICK_MASTERY_SPAN: u8 = 12;

pub const OM_SELECTED_SLOT: usize = 0x167D;

pub const OM_QUICK_BAR: usize = 0x167E;

pub const OM_SELECTED_ITEM_SLOT: usize = 0x1692;

pub const OM_QUICK_SLOT_CACHE: usize = 0x1693;

pub const RESURRECT_SELECTED: u8 = 99;

pub const AFFECT_LEN: usize = 0x178;

pub const MINIMUM_LEN: usize = TIME_OFF + 4;

const X_OFF: usize = 0x000;

const Z_OFF: usize = 0x002;

const MOB_OFF: usize = 0x004;

const AFTER_MOB_OFF: usize = MOB_OFF + MOB_LEN;

const CLIENT_ID_OFF: usize = 0x55E;

const WEATHER_OFF: usize = 0x560;

const QUICK_OFF: usize = 0x562;

const QUICK_LEN: usize = QUICK_SLOTS - QUICK_HEAD;

const UNKNOWN_572_OFF: usize = QUICK_OFF + QUICK_LEN;

const EXTRA_OFF: usize = 0x574;

const UNKNOWN_578_OFF: usize = EXTRA_OFF + 4;

const AFFECT_OFF: usize = 0x694;

const UNKNOWN_80C_OFF: usize = AFFECT_OFF + AFFECT_LEN;

const TIME_OFF: usize = 0x810;

const NAME_OFF: usize = 0x000;

const UNKNOWN_010_OFF: usize = 0x010;

const CLASS_OFF: usize = 0x014;

const UNKNOWN_015_OFF: usize = 0x015;

const GOLD_OFF: usize = 0x018;

const UNKNOWN_01C_OFF: usize = 0x01C;

const EXP_OFF: usize = 0x020;

const POS_X_OFF: usize = 0x028;

const POS_Z_OFF: usize = 0x02A;

const BASE_STATUS_OFF: usize = 0x02C;

const STATUS_OFF: usize = 0x05C;

const EQUIP_OFF: usize = 0x08C;

const CARRY_OFF: usize = 0x164;

const UNKNOWN_464_OFF: usize = CARRY_OFF + CARRY_SLOTS * ITEM_LEN;

const QUICK_HEAD_OFF: usize = 0x474;

const UNKNOWN_478_OFF: usize = QUICK_HEAD_OFF + QUICK_HEAD;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Mob {
    pub name: [u8; NAME_LEN],
    pub unknown_010: [u8; 4],
    pub class: u8,
    pub unknown_015: [u8; 3],
    pub gold: u32,
    pub unknown_01c: u32,
    pub exp: u64,
    pub x: u16,
    pub z: u16,
    pub base_status: Status,
    pub status: Status,
    pub equipment: Equipment,
    pub carry: [Item; CARRY_SLOTS],
    pub unknown_464: [u8; 16],
    pub quick_head: [u8; QUICK_HEAD],
    pub unknown_478: [u8; MOB_LEN - UNKNOWN_478_OFF],
}

impl Default for Mob {
    fn default() -> Mob {
        Mob {
            name: [0; NAME_LEN],
            unknown_010: [0; 4],
            class: 0,
            unknown_015: [0; 3],
            gold: 0,
            unknown_01c: 0,
            exp: 0,
            x: 0,
            z: 0,
            base_status: Status::default(),
            status: Status::default(),
            equipment: Equipment::default(),
            carry: [Item::default(); CARRY_SLOTS],
            unknown_464: [0; 16],
            quick_head: [0; QUICK_HEAD],
            unknown_478: [0; MOB_LEN - UNKNOWN_478_OFF],
        }
    }
}

impl Mob {
    pub fn name(&self) -> String {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);

        crate::text::codepage::decode(&self.name[..end], crate::text::Codepage::Windows1252)
    }

    pub fn bag_extension(&self, bag: usize) -> Option<Item> {
        let item = *BAG_SLOTS.get(bag).and_then(|&slot| self.carry.get(slot))?;

        (item.index == BAG_EXTENSION_ITEM).then_some(item)
    }

    pub fn guild(&self) -> u16 {
        u16::from_le_bytes([self.unknown_010[2], self.unknown_010[3]])
    }

    pub fn guild_rank(&self) -> u8 {
        self.unknown_478[0]
    }

    pub fn skill_mask(&self) -> [u32; 2] {
        [0, 1].map(|n| {
            let at = n * 4;

            u32::from_le_bytes([
                self.unknown_464[at],
                self.unknown_464[at + 1],
                self.unknown_464[at + 2],
                self.unknown_464[at + 3],
            ])
        })
    }

    pub fn evolution_tier(&self) -> u8 {
        if !self.equipment.evolved() {
            return 0;
        }

        1 + u8::from(self.skill_mask()[0] & account::EVOLVED_BIT != 0)
    }

    pub fn stat_points(&self) -> i16 {
        i16_at(&self.unknown_464, 0x46C - UNKNOWN_464_OFF)
    }

    pub fn mastery_points(&self) -> i16 {
        i16_at(&self.unknown_464, 0x46E - UNKNOWN_464_OFF)
    }

    pub fn skill_points(&self) -> i16 {
        i16_at(&self.unknown_464, 0x470 - UNKNOWN_464_OFF)
    }

    pub fn critical(&self) -> u8 {
        self.unknown_464[0x472 - UNKNOWN_464_OFF]
    }

    pub fn mana_saving(&self) -> u8 {
        self.unknown_464[0x473 - UNKNOWN_464_OFF]
    }

    pub fn attack_speed(&self) -> i16 {
        i16_at(&self.unknown_478, 0x47A - UNKNOWN_478_OFF)
    }

    pub fn sheet_value8(&self) -> i16 {
        i16_at(&self.unknown_478, 0x47C - UNKNOWN_478_OFF)
    }

    pub fn sheet_value9(&self) -> i16 {
        i16_at(&self.unknown_478, 0x47E - UNKNOWN_478_OFF)
    }

    pub fn regen(&self) -> [i16; 4] {
        [0x54E, 0x550, 0x54A, 0x54C].map(|off| i16_at(&self.unknown_478, off - UNKNOWN_478_OFF))
    }

    pub fn magic_bonus(&self) -> i16 {
        i16_at(&self.unknown_478, 0x552 - UNKNOWN_478_OFF)
    }

    pub fn to_bytes(&self) -> [u8; MOB_LEN] {
        let mut out = [0u8; MOB_LEN];

        out[NAME_OFF..NAME_OFF + NAME_LEN].copy_from_slice(&self.name);

        out[UNKNOWN_010_OFF..CLASS_OFF].copy_from_slice(&self.unknown_010);

        out[CLASS_OFF] = self.class;

        out[UNKNOWN_015_OFF..GOLD_OFF].copy_from_slice(&self.unknown_015);

        out[GOLD_OFF..UNKNOWN_01C_OFF].copy_from_slice(&self.gold.to_le_bytes());

        out[UNKNOWN_01C_OFF..EXP_OFF].copy_from_slice(&self.unknown_01c.to_le_bytes());

        out[EXP_OFF..POS_X_OFF].copy_from_slice(&self.exp.to_le_bytes());

        out[POS_X_OFF..POS_Z_OFF].copy_from_slice(&self.x.to_le_bytes());

        out[POS_Z_OFF..BASE_STATUS_OFF].copy_from_slice(&self.z.to_le_bytes());

        out[BASE_STATUS_OFF..STATUS_OFF].copy_from_slice(&self.base_status.to_bytes());

        out[STATUS_OFF..EQUIP_OFF].copy_from_slice(&self.status.to_bytes());

        out[EQUIP_OFF..CARRY_OFF].copy_from_slice(&self.equipment.to_bytes());

        for (n, item) in self.carry.iter().enumerate() {
            let at = CARRY_OFF + n * ITEM_LEN;

            out[at..at + ITEM_LEN].copy_from_slice(&item.to_bytes());
        }

        out[UNKNOWN_464_OFF..QUICK_HEAD_OFF].copy_from_slice(&self.unknown_464);

        out[QUICK_HEAD_OFF..UNKNOWN_478_OFF].copy_from_slice(&self.quick_head);

        out[UNKNOWN_478_OFF..].copy_from_slice(&self.unknown_478);

        out
    }
}

fn mob_at(buf: &[u8], off: usize) -> Result<Mob> {
    let mut carry = [Item::default(); CARRY_SLOTS];

    for (n, item) in carry.iter_mut().enumerate() {
        *item = account::item_at(buf, off + CARRY_OFF + n * ITEM_LEN)?;
    }

    Ok(Mob {
        name: account::fixed(buf, off + NAME_OFF)?,
        unknown_010: account::fixed(buf, off + UNKNOWN_010_OFF)?,
        class: crate::util::byte_at(buf, off + CLASS_OFF, KIND)?,
        unknown_015: account::fixed(buf, off + UNKNOWN_015_OFF)?,
        gold: crate::util::u32_at(buf, off + GOLD_OFF, KIND)?,
        unknown_01c: crate::util::u32_at(buf, off + UNKNOWN_01C_OFF, KIND)?,
        exp: u64_at(buf, off + EXP_OFF)?,
        x: crate::util::u16_at(buf, off + POS_X_OFF, KIND)?,
        z: crate::util::u16_at(buf, off + POS_Z_OFF, KIND)?,
        base_status: account::status_at(buf, off + BASE_STATUS_OFF)?,
        status: account::status_at(buf, off + STATUS_OFF)?,
        equipment: account::equipment_at(buf, off + EQUIP_OFF)?,
        carry,
        unknown_464: account::fixed(buf, off + UNKNOWN_464_OFF)?,
        quick_head: account::fixed(buf, off + QUICK_HEAD_OFF)?,
        unknown_478: account::fixed(buf, off + UNKNOWN_478_OFF)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct EnterWorld {
    pub x: u16,
    pub z: u16,
    pub mob: Mob,
    pub unknown_55c: [u8; 2],
    pub client_id: u16,
    pub weather: u16,
    pub quick_rest: [u8; QUICK_LEN],
    pub unknown_572: [u8; 2],
    pub extra: u32,
    pub unknown_578: [u8; AFFECT_OFF - UNKNOWN_578_OFF],
    pub affects: [u8; AFFECT_LEN],
    pub unknown_80c: [u8; 4],
    pub time: u32,
    pub tail: Vec<u8>,
}

impl EnterWorld {
    pub fn zone(&self) -> (u8, u8) {
        ((self.x >> 7) as u8, (self.z >> 7) as u8)
    }

    pub fn quick_slots(&self) -> [u8; QUICK_SLOTS] {
        let base = self.mob.class;

        let mut flat = [0u8; QUICK_SLOTS];

        flat[..QUICK_HEAD].copy_from_slice(&self.mob.quick_head);

        flat[QUICK_HEAD..].copy_from_slice(&self.quick_rest);

        for slot in flat.iter_mut() {
            if *slot < QUICK_CLASS_SPAN {
                *slot = slot.wrapping_add(base.wrapping_mul(QUICK_CLASS_SPAN));
            }
        }

        flat
    }

    pub fn pages_of_ten(&self) -> [[u8; QUICK_PAGE_SLOTS]; QUICK_PAGES] {
        let flat = self.quick_slots();

        std::array::from_fn(|page| std::array::from_fn(|slot| flat[page * QUICK_PAGE_SLOTS + slot]))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; MINIMUM_LEN];

        out[X_OFF..Z_OFF].copy_from_slice(&self.x.to_le_bytes());

        out[Z_OFF..MOB_OFF].copy_from_slice(&self.z.to_le_bytes());

        out[MOB_OFF..AFTER_MOB_OFF].copy_from_slice(&self.mob.to_bytes());

        out[AFTER_MOB_OFF..CLIENT_ID_OFF].copy_from_slice(&self.unknown_55c);

        out[CLIENT_ID_OFF..WEATHER_OFF].copy_from_slice(&self.client_id.to_le_bytes());

        out[WEATHER_OFF..QUICK_OFF].copy_from_slice(&self.weather.to_le_bytes());

        out[QUICK_OFF..UNKNOWN_572_OFF].copy_from_slice(&self.quick_rest);

        out[UNKNOWN_572_OFF..EXTRA_OFF].copy_from_slice(&self.unknown_572);

        out[EXTRA_OFF..UNKNOWN_578_OFF].copy_from_slice(&self.extra.to_le_bytes());

        out[UNKNOWN_578_OFF..AFFECT_OFF].copy_from_slice(&self.unknown_578);

        out[AFFECT_OFF..UNKNOWN_80C_OFF].copy_from_slice(&self.affects);

        out[UNKNOWN_80C_OFF..TIME_OFF].copy_from_slice(&self.unknown_80c);

        out[TIME_OFF..MINIMUM_LEN].copy_from_slice(&self.time.to_le_bytes());

        out.extend_from_slice(&self.tail);

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<EnterWorld> {
    if payload.len() < MINIMUM_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: MINIMUM_LEN,
            len: payload.len(),
        });
    }

    Ok(EnterWorld {
        x: crate::util::u16_at(payload, X_OFF, KIND)?,
        z: crate::util::u16_at(payload, Z_OFF, KIND)?,
        mob: mob_at(payload, MOB_OFF)?,
        unknown_55c: account::fixed(payload, AFTER_MOB_OFF)?,
        client_id: crate::util::u16_at(payload, CLIENT_ID_OFF, KIND)?,
        weather: crate::util::u16_at(payload, WEATHER_OFF, KIND)?,
        quick_rest: account::fixed(payload, QUICK_OFF)?,
        unknown_572: account::fixed(payload, UNKNOWN_572_OFF)?,
        extra: crate::util::u32_at(payload, EXTRA_OFF, KIND)?,
        unknown_578: account::fixed(payload, UNKNOWN_578_OFF)?,
        affects: account::fixed(payload, AFFECT_OFF)?,
        unknown_80c: account::fixed(payload, UNKNOWN_80C_OFF)?,
        time: crate::util::u32_at(payload, TIME_OFF, KIND)?,
        tail: payload[MINIMUM_LEN..].to_vec(),
    })
}

pub fn unrebase_for_send(bar: &[u8; QUICK_SLOTS], class: u8) -> [u8; QUICK_SLOTS] {
    bar.map(|b| {
        if b <= 0x5F {
            b.wrapping_sub(class.wrapping_mul(24))
        } else if (0x69..=0x7F).contains(&b) {
            b.wrapping_sub(class.wrapping_mul(12))
        } else {
            b
        }
    })
}

pub fn parse_quick_bar(payload: &[u8]) -> Result<[u8; QUICK_SLOTS]> {
    let bytes = payload
        .get(..QUICK_BAR_PAYLOAD_LEN)
        .ok_or(Error::Truncated {
            kind: Kind::QuickBar,
            offset: 0,
            needed: QUICK_BAR_PAYLOAD_LEN,
            len: payload.len(),
        })?;

    let mut bar = [0u8; QUICK_SLOTS];

    bar.copy_from_slice(bytes);

    Ok(bar)
}

pub fn rebase_for_receive(bar: &[u8; QUICK_SLOTS], class: u8) -> [u8; QUICK_SLOTS] {
    bar.map(|b| {
        if QUICK_SECOND_WINDOW.contains(&b) {
            b.wrapping_add(class.wrapping_mul(QUICK_MASTERY_SPAN))
        } else if b < QUICK_CLASS_SPAN {
            b.wrapping_add(class.wrapping_mul(QUICK_CLASS_SPAN))
        } else {
            b
        }
    })
}

pub fn selected(bar: &[u8; QUICK_SLOTS], selected: u8) -> Option<u8> {
    usize::try_from(selected as i8)
        .ok()
        .and_then(|slot| bar.get(slot).copied())
}

fn i16_at(buf: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([buf[off], buf[off + 1]])
}

fn u64_at(buf: &[u8], off: usize) -> Result<u64> {
    let low = u64::from(crate::util::u32_at(buf, off, KIND)?);

    let high = u64::from(crate::util::u32_at(buf, off + 4, KIND)?);

    Ok(low | high << 32)
}

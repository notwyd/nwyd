use formats::mesh::appearance;
use formats::net::account::{self, Block, Equipment, Item, Status};
use formats::net::enter::{CARRY_SLOTS, EnterWorld};

pub const ZONE_SHIFT: u32 = 7;

pub const NAME_TAIL: usize = 14;

pub const WEARER_FLAGS_OFF: usize = 0x546 - 0x478;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Entered {
    pub client_id: u16,
    pub weather: u16,
    pub server_seconds: u32,
    pub slot: usize,
    pub x: u16,
    pub z: u16,
    pub name: [u8; account::NAME_LEN],
    pub class: u8,
    pub status: Status,
    pub base_status: Status,
    pub equipment: Equipment,
    pub kingdom: u8,
    pub carry: [Item; CARRY_SLOTS],
    pub guild: u16,
    pub mob_guild: u16,
    pub mob_guild_rank: u8,
    pub account_body: u16,
    pub exp: u64,
    pub gold: u32,
    pub wearer: appearance::Wearer,
    pub warehouse: Warehouse,
    pub cp: u32,
    pub skill_mask: [u32; 2],
    pub quest_flags: [u8; 12],
    pub derived: Derived,
    pub quick: [u8; formats::net::enter::QUICK_SLOTS],
    pub quick_selected: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Derived {
    pub stat_points: i16,
    pub mastery_points: i16,
    pub skill_points: i16,
    pub critical: u8,
    pub mana_saving: u8,
    pub attack_speed: i16,
    pub value8: i16,
    pub value9: i16,
    pub regen: [i16; 4],
    pub magic_bonus: i16,
    pub words: [u16; 6],
}

impl Derived {
    pub fn of(mob: &formats::net::enter::Mob) -> Derived {
        Derived {
            stat_points: mob.stat_points(),
            mastery_points: mob.mastery_points(),
            skill_points: mob.skill_points(),
            critical: mob.critical(),
            mana_saving: mob.mana_saving(),
            attack_speed: mob.attack_speed(),
            value8: mob.sheet_value8(),
            value9: mob.sheet_value9(),
            regen: mob.regen(),
            magic_bonus: mob.magic_bonus(),
            words: [0; 6],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Warehouse {
    pub cargo: [Item; account::CARGO_SLOTS],
    pub gold: u32,
}

impl Default for Warehouse {
    fn default() -> Warehouse {
        Warehouse {
            cargo: [Item::default(); account::CARGO_SLOTS],
            gold: 0,
        }
    }
}

impl Warehouse {
    pub fn of(accept: &account::Accept) -> Warehouse {
        Warehouse {
            cargo: accept.cargo,
            gold: accept.extra,
        }
    }
}

pub fn class_from_body(body: u16) -> Option<u8> {
    let body = i32::from(body as i16);

    if body >= 0x28 {
        return None;
    }

    let class = if body == 0x20 {
        2
    } else {
        let remainder = body % 10;

        if (6..=9).contains(&remainder) {
            remainder - 6
        } else {
            body / 10
        }
    };

    Some(class as u8)
}

impl Entered {
    pub fn with_warehouse(mut self, warehouse: Warehouse) -> Entered {
        self.warehouse = warehouse;

        self
    }

    pub fn from_packet(packet: &EnterWorld, slot: usize, block: Option<&Block>) -> Entered {
        tracing::debug!(
            target : "nwyd::body_state", event = "enter_world_114", entity = packet
            .client_id, slot, wire_body = packet.mob.equipment.body().index, account_body
            = ? block.and_then(| b | b.slots.get(slot)).map(| s | s.equipment.body()
            .index), retained_body = block.and_then(| b | b.slots.get(slot)).map_or(0, |
            s | s.equipment.body().index), wire_class = packet.mob.class, equipment = ?
            packet.mob.equipment.items.map(| item | item.index), costume_flags = packet
            .mob.unknown_478[WEARER_FLAGS_OFF], "Body state at world entry"
        );

        Entered {
            cp: 0,
            skill_mask: packet.mob.skill_mask(),
            quest_flags: std::array::from_fn(|i| packet.affects[i]),
            derived: Derived::of(&packet.mob),
            client_id: packet.client_id,
            weather: packet.weather,
            server_seconds: packet.time,
            slot,
            x: packet.x,
            z: packet.z,
            name: packet.mob.name,
            class: packet.mob.class,
            status: packet.mob.status,
            base_status: packet.mob.base_status,
            equipment: packet.mob.equipment,
            kingdom: 0,
            carry: packet.mob.carry,
            guild: block.and_then(|b| b.slots.get(slot)).map_or(0, |s| s.guild),
            mob_guild: packet.mob.guild(),
            mob_guild_rank: packet.mob.guild_rank(),
            account_body: block
                .and_then(|b| b.slots.get(slot))
                .map_or(0, |s| s.equipment.body().index),
            exp: packet.mob.exp,
            gold: packet.mob.gold,
            wearer: appearance::Wearer {
                rank_word: u16::from(packet.mob.name[NAME_TAIL])
                    | (u16::from(packet.mob.name[NAME_TAIL + 1]) << 8),
                flags: packet.mob.unknown_478[WEARER_FLAGS_OFF],
            },
            warehouse: Warehouse::default(),
            quick: packet.quick_slots(),
            quick_selected: 0,
        }
    }

    pub fn bag_extension(&self, bag: usize) -> Option<Item> {
        let item = *formats::net::enter::BAG_SLOTS
            .get(bag)
            .and_then(|&slot| self.carry.get(slot))?;

        (item.index == formats::net::enter::BAG_EXTENSION_ITEM).then_some(item)
    }

    pub fn zone(&self) -> (u8, u8) {
        ((self.x >> ZONE_SHIFT) as u8, (self.z >> ZONE_SHIFT) as u8)
    }

    pub fn zone_name(&self) -> String {
        let (zx, zz) = self.zone();

        format!("Field{zx:02}{zz:02}")
    }

    pub fn position(&self) -> glam::Vec3 {
        glam::Vec3::new(f32::from(self.x) + 0.5, 0.0, f32::from(self.z) + 0.5)
    }

    pub fn scales_on_the_clamped_curve(&self) -> bool {
        self.client_id < 1_000
    }

    pub fn speed(&self) -> f32 {
        move_speed(&self.status)
    }

    pub fn name(&self) -> String {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(account::NAME_LEN);

        formats::text::codepage::decode(&self.name[..end], formats::text::Codepage::Windows1252)
    }

    pub fn refresh_class(&mut self) {
        let previous_class = self.class;

        if let Some(class) = class_from_body(self.equipment.body().index) {
            self.class = class;
        }

        if previous_class != self.class {
            tracing::debug!(
                target : "nwyd::body_state", event = "refresh_class", entity = self
                .client_id, slot = self.slot, live_body = self.equipment.body().index,
                retained_body = self.account_body, previous_class, class = self.class,
                "Body eligibility state refreshed"
            );
        }
    }
}

pub const SPEED_BYTE: u32 = 0x0D;

pub fn move_speed(status: &Status) -> f32 {
    let value = ((status.flags >> (SPEED_BYTE % 4 * 8)) & 0x0F) as u8;

    let clamped = match value {
        0 => 1,
        v if v > 7 => 7,
        v => v,
    };

    f32::from(clamped)
}

pub const SKILLS_PER_CLASS: i32 = 0x18;

pub const CLASS_SKILL_CODES: i32 = 0x60;

pub const SHARED_CODES: std::ops::RangeInclusive<i32> = 0x60..=0x67;

pub const SHARED_CODE_SHIFT: i32 = 0x48;

pub fn learned(player: &Entered, code: i32) -> bool {
    if code < 0 {
        return code < CLASS_SKILL_CODES;
    }

    if code < CLASS_SKILL_CODES {
        if code / SKILLS_PER_CLASS != i32::from(player.class) {
            return false;
        }

        return player.skill_mask[0] & (1u32 << (code % SKILLS_PER_CLASS)) != 0;
    }

    if code > *SHARED_CODES.end() {
        return true;
    }

    player.skill_mask[0] & (1u32 << ((code - SHARED_CODE_SHIFT) & 0x1F)) != 0
}

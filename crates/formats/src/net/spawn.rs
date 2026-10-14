use crate::error::{Kind, Result};
use crate::net::account::{self, Status};

const KIND: Kind = Kind::Spawn;

pub const EQUIP_SLOTS: usize = 18;

pub const AFFECTS: usize = 32;

pub const AFFECT_LEN: usize = AFFECTS * 2;

pub const NAME_LEN: usize = 12;

pub const MOB_NAME_LEN: usize = 15;

pub const GUILD_NAME_LEN: usize = 26;

pub const MOTTO_LEN: usize = 24;

pub const MOTTO_MAX: usize = 22;

pub const PLAYER_ID_LIMIT: u16 = 1000;

pub const HAND_SLOTS: [usize; 2] = [6, 7];

pub const SPECIAL_SLOT: usize = 13;

pub const ALT_HP_ITEMS: [u16; 3] = [0x312, 0x790, 0x791];

pub const AFFECT_TRANSFORM: u8 = b'2';

pub const CREATE_TYPE_MASK: u16 = 0x7FFF;

const X_OFF: usize = 0x000;

const Z_OFF: usize = 0x002;

const ID_OFF: usize = 0x004;

const NAME_OFF: usize = 0x006;

const NAME_LEVEL_OFF: usize = 0x012;

const UNKNOWN_013_OFF: usize = 0x013;

const LEVEL_OFF: usize = 0x014;

const EQUIP_OFF: usize = 0x016;

const AFFECT_OFF: usize = EQUIP_OFF + EQUIP_SLOTS * 2;

const GUILD_OFF: usize = AFFECT_OFF + AFFECT_LEN;

const GUILD_RANK_OFF: usize = 0x07C;

const UNKNOWN_07D_OFF: usize = 0x07D;

const EXTRA_OFF: usize = 0x07E;

const SCORE_OFF: usize = 0x080;

const CREATE_TYPE_OFF: usize = SCORE_OFF + account::STATUS_LEN;

const REFINE_OFF: usize = 0x0B2;

const GUILD_NAME_OFF: usize = REFINE_OFF + EQUIP_SLOTS;

const EFFECT_OFF: usize = GUILD_NAME_OFF + GUILD_NAME_LEN;

const FLAGS_OFF: usize = EFFECT_OFF + EQUIP_SLOTS;

const MOTTO_OFF: usize = FLAGS_OFF + 1;

pub const MINIMUM_LEN: usize = MOTTO_OFF;

pub const SHOP_LEN: usize = MOTTO_OFF + MOTTO_LEN;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Spawn {
    pub x: i16,
    pub z: i16,
    pub id: u16,
    pub name: [u8; NAME_LEN],
    pub name_level: u8,
    pub unknown_013: u8,
    pub level: u16,
    pub equip: [u16; EQUIP_SLOTS],
    pub affect: [u8; AFFECT_LEN],
    pub guild: u16,
    pub guild_rank: u8,
    pub unknown_07d: u8,
    pub extra: u16,
    pub score: Status,
    pub create_type: u16,
    pub refine: [u8; EQUIP_SLOTS],
    pub guild_name: [u8; GUILD_NAME_LEN],
    pub effect: [u8; EQUIP_SLOTS],
    pub flags: u8,
    pub motto: [u8; MOTTO_LEN],
    pub tail: Vec<u8>,
}

impl Default for Spawn {
    fn default() -> Spawn {
        Spawn {
            x: 0,
            z: 0,
            id: 0,
            name: [0; NAME_LEN],
            name_level: 0,
            unknown_013: 0,
            level: 0,
            equip: [0; EQUIP_SLOTS],
            affect: [0; AFFECT_LEN],
            guild: 0,
            guild_rank: 0,
            unknown_07d: 0,
            extra: 0,
            score: Status::default(),
            create_type: 0,
            refine: [0; EQUIP_SLOTS],
            guild_name: [0; GUILD_NAME_LEN],
            effect: [0; EQUIP_SLOTS],
            flags: 0,
            motto: [0; MOTTO_LEN],
            tail: Vec::new(),
        }
    }
}

impl Spawn {
    pub fn is_player(&self) -> bool {
        self.id < PLAYER_ID_LIMIT
    }

    pub fn name(&self) -> String {
        let mut run = [0u8; MOB_NAME_LEN];

        run[..NAME_LEN].copy_from_slice(&self.name);

        run[NAME_LEN] = self.name_level;

        run[NAME_LEN + 1] = self.unknown_013;

        run[NAME_LEN + 2] = self.level.to_le_bytes()[0];

        let len = match self.is_player() {
            true => NAME_LEN,
            false => MOB_NAME_LEN,
        };

        decode(&run[..len])
    }

    pub fn guild_name(&self) -> String {
        decode(&self.guild_name[..GUILD_NAME_LEN - 1])
    }

    pub fn motto(&self) -> String {
        decode(&self.motto[..MOTTO_MAX])
    }

    pub fn level(&self) -> u16 {
        match self.is_player() {
            true => self.level,
            false => 0,
        }
    }

    pub fn plate_level(&self) -> u8 {
        match self.is_player() {
            true => self.name_level,
            false => MOB_PLATE_LEVEL,
        }
    }

    pub fn hands(&self) -> (i32, i32) {
        (
            i32::from(self.equip[HAND_SLOTS[1]] as i16),
            i32::from(self.equip[HAND_SLOTS[0]] as i16),
        )
    }

    pub fn transforming(&self) -> bool {
        self.affect
            .chunks_exact(2)
            .any(|pair| pair[1] == AFFECT_TRANSFORM)
    }

    pub fn apply_arena(
        &mut self,
        table: &[crate::data::arena_visual::Record],
        effects: &crate::data::item_effect::Effects<'_>,
    ) -> bool {
        if !self.is_player() || !self.transforming() {
            return false;
        }

        let Some(row) = crate::data::arena_visual::lookup(table, u32::from(self.effect[12])) else {
            return false;
        };

        if let Some(name) = row
            .raw_names
            .get(usize::from(self.refine[12]))
            .filter(|s| !s.is_empty())
        {
            self.name.fill(0);

            let len = name.len().min(self.name.len());

            self.name[..len].copy_from_slice(&name[..len]);
        }

        let class = effects.total_scaled(
            &account::Item {
                index: self.extra,
                ..Default::default()
            },
            0x12,
        );

        let patch = row.packet_patch([self.equip[6], self.equip[7]], class, effects);

        for (hand, part) in patch.hands.into_iter().enumerate() {
            if let Some((item, colour)) = part {
                self.equip[hand + 6] = item;

                self.refine[hand + 6] = colour;

                self.effect[hand + 6] = colour;
            }
        }

        if let Some(item) = patch.costume {
            self.equip[12] = item;
        }

        if let Some(value) = patch.mount_refine {
            self.refine[14] = value;
        }

        self.flags = 0;

        true
    }

    pub fn alt_hp_multiplier(&self) -> Option<u32> {
        let item = self.equip[SPECIAL_SLOT];

        if !ALT_HP_ITEMS.contains(&item) {
            return None;
        }

        let base = u32::from(self.effect[SPECIAL_SLOT]).max(2);

        Some(match item {
            0x790 => base * 10,
            0x791 => base * 1000,
            _ => base,
        })
    }

    pub fn shop_rewrite(&mut self) -> bool {
        if self.motto().is_empty() {
            return false;
        }

        self.refine[1..].fill(0);

        self.affect[2..].fill(0);

        self.equip[0] = SHOP_BODY;

        self.equip[HAND_SLOTS[0]] = 0;

        self.equip[HAND_SLOTS[1]] = 0;

        self.equip[MOUNT_SLOT] = 0;

        self.score.stat[SHOP_CON_STAT] = SHOP_CON;

        true
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; MINIMUM_LEN.max(self.written_len())];

        out[X_OFF..Z_OFF].copy_from_slice(&self.x.to_le_bytes());

        out[Z_OFF..ID_OFF].copy_from_slice(&self.z.to_le_bytes());

        out[ID_OFF..NAME_OFF].copy_from_slice(&self.id.to_le_bytes());

        out[NAME_OFF..NAME_LEVEL_OFF].copy_from_slice(&self.name);

        out[NAME_LEVEL_OFF] = self.name_level;

        out[UNKNOWN_013_OFF] = self.unknown_013;

        out[LEVEL_OFF..EQUIP_OFF].copy_from_slice(&self.level.to_le_bytes());

        for (slot, id) in self.equip.iter().enumerate() {
            let at = EQUIP_OFF + slot * 2;

            out[at..at + 2].copy_from_slice(&id.to_le_bytes());
        }

        out[AFFECT_OFF..GUILD_OFF].copy_from_slice(&self.affect);

        out[GUILD_OFF..GUILD_RANK_OFF].copy_from_slice(&self.guild.to_le_bytes());

        out[GUILD_RANK_OFF] = self.guild_rank;

        out[UNKNOWN_07D_OFF] = self.unknown_07d;

        out[EXTRA_OFF..SCORE_OFF].copy_from_slice(&self.extra.to_le_bytes());

        out[SCORE_OFF..CREATE_TYPE_OFF].copy_from_slice(&self.score.to_bytes());

        out[CREATE_TYPE_OFF..REFINE_OFF].copy_from_slice(&self.create_type.to_le_bytes());

        out[REFINE_OFF..GUILD_NAME_OFF].copy_from_slice(&self.refine);

        out[GUILD_NAME_OFF..EFFECT_OFF].copy_from_slice(&self.guild_name);

        out[EFFECT_OFF..FLAGS_OFF].copy_from_slice(&self.effect);

        out[FLAGS_OFF] = self.flags;

        if out.len() >= SHOP_LEN {
            out[MOTTO_OFF..SHOP_LEN].copy_from_slice(&self.motto);
        }

        if !self.tail.is_empty() {
            let at = out.len() - self.tail.len();

            out[at..].copy_from_slice(&self.tail);
        }

        out
    }

    fn written_len(&self) -> usize {
        let body = match self.motto == [0; MOTTO_LEN] {
            true => MINIMUM_LEN,
            false => SHOP_LEN,
        };

        body + self.tail.len()
    }
}

pub const MOB_PLATE_LEVEL: u8 = 0x4B;

pub const MOUNT_SLOT: usize = 14;

pub const SHOP_BODY: u16 = 0xE6;

pub const SHOP_CON: u16 = 15000;

pub const SHOP_CON_STAT: usize = 3;

fn decode(run: &[u8]) -> String {
    let end = run.iter().position(|&b| b == 0).unwrap_or(run.len());

    crate::text::codepage::decode(&run[..end], crate::text::Codepage::Windows1252)
}

pub fn parse(payload: &[u8]) -> Result<Spawn> {
    let mut equip = [0u16; EQUIP_SLOTS];

    for (slot, id) in equip.iter_mut().enumerate() {
        *id = crate::util::u16_at(payload, EQUIP_OFF + slot * 2, KIND)?;
    }

    let carries_motto = payload.len() >= SHOP_LEN;

    let body = match carries_motto {
        true => SHOP_LEN,
        false => MINIMUM_LEN,
    };

    Ok(Spawn {
        x: crate::util::u16_at(payload, X_OFF, KIND)? as i16,
        z: crate::util::u16_at(payload, Z_OFF, KIND)? as i16,
        id: crate::util::u16_at(payload, ID_OFF, KIND)?,
        name: account::fixed(payload, NAME_OFF)?,
        name_level: crate::util::byte_at(payload, NAME_LEVEL_OFF, KIND)?,
        unknown_013: crate::util::byte_at(payload, UNKNOWN_013_OFF, KIND)?,
        level: crate::util::u16_at(payload, LEVEL_OFF, KIND)?,
        equip,
        affect: account::fixed(payload, AFFECT_OFF)?,
        guild: crate::util::u16_at(payload, GUILD_OFF, KIND)?,
        guild_rank: crate::util::byte_at(payload, GUILD_RANK_OFF, KIND)?,
        unknown_07d: crate::util::byte_at(payload, UNKNOWN_07D_OFF, KIND)?,
        extra: crate::util::u16_at(payload, EXTRA_OFF, KIND)?,
        score: account::status_at(payload, SCORE_OFF)?,
        create_type: crate::util::u16_at(payload, CREATE_TYPE_OFF, KIND)?,
        refine: account::fixed(payload, REFINE_OFF)?,
        guild_name: account::fixed(payload, GUILD_NAME_OFF)?,
        effect: account::fixed(payload, EFFECT_OFF)?,
        flags: crate::util::byte_at(payload, FLAGS_OFF, KIND)?,
        motto: match carries_motto {
            true => account::fixed(payload, MOTTO_OFF)?,
            false => [0; MOTTO_LEN],
        },
        tail: payload[body.min(payload.len())..].to_vec(),
    })
}

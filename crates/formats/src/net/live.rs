use crate::error::{Error, Kind as ErrorKind, Result};
use crate::net::account::{self, ITEM_LEN, Item, STATUS_LEN, Status};
use crate::net::affect::{AFFECT_LEN, AFFECTS, Affect};
use crate::net::spawn::EQUIP_SLOTS;
use crate::util::{u16_at, u32_at};

const KIND: ErrorKind = ErrorKind::LiveState;

pub const VITALS_FRAME_LEN: usize = 0x1C;

pub const VITALS_PAYLOAD_LEN: usize = VITALS_FRAME_LEN - 0x0C;

pub const ACTOR_STATUS_FRAME_LEN: usize = 0xB0;

pub const ACTOR_STATUS_PAYLOAD_LEN: usize = ACTOR_STATUS_FRAME_LEN - 0x0C;

pub const PLAYER_PROGRESSION_FRAME_LEN: usize = 0x30;

pub const PLAYER_PROGRESSION_PAYLOAD_LEN: usize = PLAYER_PROGRESSION_FRAME_LEN - 0x0C;

pub const EQUIP_FRAME_LEN: usize = 0x58;

pub const EQUIP_PAYLOAD_LEN: usize = EQUIP_FRAME_LEN - 0x0C;

pub const GOLD_PAYLOAD_LEN: usize = 4;

pub const VANISH_FRAME_LEN: usize = 0x10;

pub const VANISH_PAYLOAD_LEN: usize = VANISH_FRAME_LEN - 0x0C;

pub const HP_DELTA_FRAME_LEN: usize = 0x14;

pub const HP_DELTA_PAYLOAD_LEN: usize = HP_DELTA_FRAME_LEN - 0x0C;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct Vitals {
    pub hp: i32,
    pub mp: i32,
    pub unknown_08: u32,
    pub unknown_0c: u32,
}

impl Vitals {
    pub fn is_dead(&self) -> bool {
        self.hp < 1
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; VITALS_PAYLOAD_LEN];

        out[0x00..0x04].copy_from_slice(&self.hp.to_le_bytes());

        out[0x04..0x08].copy_from_slice(&self.mp.to_le_bytes());

        out[0x08..0x0C].copy_from_slice(&self.unknown_08.to_le_bytes());

        out[0x0C..0x10].copy_from_slice(&self.unknown_0c.to_le_bytes());

        out
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct HpDelta {
    pub hp: i32,
    pub delta: i16,
}

impl HpDelta {
    pub fn draws(&self) -> bool {
        self.delta != 0
    }

    pub fn is_heal(&self) -> bool {
        self.delta > 0
    }

    pub fn text(&self) -> Option<String> {
        match self.delta {
            0 => None,
            d if d > 0 => Some(format!("+ {d}")),
            d => Some(format!("{}", -i32::from(d))),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; HP_DELTA_PAYLOAD_LEN];

        out[0x00..0x04].copy_from_slice(&self.hp.to_le_bytes());

        out[0x04..0x06].copy_from_slice(&self.delta.to_le_bytes());

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Container {
    Equipment,
    Carry,
    Cargo,
    Other(i16),
}

impl Container {
    pub fn of(value: i16) -> Container {
        match value {
            0 => Container::Equipment,
            1 => Container::Carry,
            2 => Container::Cargo,
            other => Container::Other(other),
        }
    }

    pub fn value(&self) -> i16 {
        match self {
            Container::Equipment => 0,
            Container::Carry => 1,
            Container::Cargo => 2,
            Container::Other(value) => *value,
        }
    }

    pub fn slots(&self) -> Option<usize> {
        match self {
            Container::Equipment => Some(EQUIP_SLOTS),
            Container::Carry => Some(crate::net::enter::CARRY_SLOTS),
            Container::Cargo => Some(account::CARGO_SLOTS),
            Container::Other(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct SlotPatch {
    pub container: Container,
    pub slot: i16,
    pub item: Item,
}

impl SlotPatch {
    pub fn index(&self) -> Option<usize> {
        let slot = usize::try_from(self.slot).ok()?;

        match slot < self.container.slots()? {
            true => Some(slot),
            false => None,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; VITALS_PAYLOAD_LEN];

        out[0x00..0x02].copy_from_slice(&self.container.value().to_le_bytes());

        out[0x02..0x04].copy_from_slice(&self.slot.to_le_bytes());

        out[0x04..0x04 + ITEM_LEN].copy_from_slice(&self.item.to_bytes());

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct ActorStatus {
    pub score: Status,
    pub critical: u8,
    pub mana_saving: u8,
    pub affects: [Affect; AFFECTS],
    pub guild: u16,
    pub unknown_74: u16,
    pub unknown_76: u32,
    pub unknown_7a: u32,
    pub restore_alternate_hp: u32,
    pub unknown_84: u32,
    pub magic_bonus: u16,
    pub transformation_variant: u8,
    pub words: [u16; 6],
    pub player_words: [u16; 3],
    pub mount_hp_bonus: u16,
}

impl Default for ActorStatus {
    fn default() -> ActorStatus {
        ActorStatus {
            score: Status::default(),
            critical: 0,
            mana_saving: 0,
            affects: [Affect {
                time_8s: 0,
                kind: 0,
            }; AFFECTS],
            guild: 0,
            unknown_74: 0,
            unknown_76: 0,
            unknown_7a: 0,
            restore_alternate_hp: 0,
            unknown_84: 0,
            magic_bonus: 0,
            transformation_variant: 0,
            words: [0; 6],
            player_words: [0; 3],
            mount_hp_bonus: 0,
        }
    }
}

pub const ACTOR_STATUS_AFFECT_OFF: usize = 0x32;

impl ActorStatus {
    pub fn gm_hidden(&self, id: u16) -> bool {
        id < crate::net::spawn::PLAYER_ID_LIMIT && self.score.flags.to_le_bytes()[0] & 1 == 1
    }

    pub fn clamped_hp(&self) -> u32 {
        self.score.hp.min(self.score.max_hp)
    }

    pub fn reconcile(standing: Affect, incoming: Affect, elapsed_ms: u32) -> bool {
        let elapsed_s = (elapsed_ms / 1000) as i32;

        let scaled = i32::from(standing.time_8s) * 8 - elapsed_s - 4;

        let scaled = i32::from(scaled as i16);

        let predicted = (scaled + (scaled >> 31 & 7)) >> 3;

        predicted != i32::from(incoming.time_8s) || incoming.kind != standing.kind
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; ACTOR_STATUS_PAYLOAD_LEN];

        out[0x00..STATUS_LEN].copy_from_slice(&self.score.to_bytes());

        out[0x30] = self.critical;

        out[0x31] = self.mana_saving;

        for (slot, affect) in self.affects.iter().enumerate() {
            let at = ACTOR_STATUS_AFFECT_OFF + slot * AFFECT_LEN;

            out[at] = affect.time_8s;

            out[at + 1] = affect.kind;
        }

        out[0x72..0x74].copy_from_slice(&self.guild.to_le_bytes());

        out[0x74..0x76].copy_from_slice(&self.unknown_74.to_le_bytes());

        out[0x76..0x7A].copy_from_slice(&self.unknown_76.to_le_bytes());

        out[0x7A..0x7E].copy_from_slice(&self.unknown_7a.to_le_bytes());

        out[0x80..0x84].copy_from_slice(&self.restore_alternate_hp.to_le_bytes());

        out[0x84..0x88].copy_from_slice(&self.unknown_84.to_le_bytes());

        out[0x88..0x8A].copy_from_slice(&self.magic_bonus.to_le_bytes());

        out[0x8C] = self.transformation_variant;

        for (n, word) in self.words.iter().enumerate() {
            out[0x8E + n * 2..0x90 + n * 2].copy_from_slice(&word.to_le_bytes());
        }

        for (n, word) in self.player_words.iter().enumerate() {
            out[0x9A + n * 2..0x9C + n * 2].copy_from_slice(&word.to_le_bytes());
        }

        out[0xA0..0xA2].copy_from_slice(&self.mount_hp_bonus.to_le_bytes());

        out
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct PlayerProgression {
    pub cp: u32,
    pub exp: u64,
    pub mask: [u32; 2],
    pub words: [u16; 3],
    pub gold: u32,
}

impl PlayerProgression {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; PLAYER_PROGRESSION_PAYLOAD_LEN];

        out[0x00..0x04].copy_from_slice(&self.cp.to_le_bytes());

        out[0x04..0x0C].copy_from_slice(&self.exp.to_le_bytes());

        out[0x0C..0x10].copy_from_slice(&self.mask[0].to_le_bytes());

        out[0x10..0x14].copy_from_slice(&self.mask[1].to_le_bytes());

        for (n, word) in self.words.iter().enumerate() {
            out[0x14 + n * 2..0x16 + n * 2].copy_from_slice(&word.to_le_bytes());
        }

        out[0x1C..0x20].copy_from_slice(&self.gold.to_le_bytes());

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct EquipChange {
    pub equip: [u16; EQUIP_SLOTS],
    pub refine: [u8; EQUIP_SLOTS],
    pub effect: [u8; EQUIP_SLOTS],
    pub flags: u8,
}

impl Default for EquipChange {
    fn default() -> EquipChange {
        EquipChange {
            equip: [0; EQUIP_SLOTS],
            refine: [0; EQUIP_SLOTS],
            effect: [0; EQUIP_SLOTS],
            flags: 0,
        }
    }
}

pub const COSTUME_SLOT: usize = 12;

pub const COSTUME_TRIGGER: u16 = 10;

impl EquipChange {
    pub fn apply_arena(
        &mut self,
        table: &[crate::data::arena_visual::Record],
        effects: &crate::data::item_effect::Effects<'_>,
    ) -> bool {
        let Some((arena, _role)) = self.costume_burst() else {
            return false;
        };

        let Some(row) = crate::data::arena_visual::lookup(table, u32::from(arena)) else {
            return false;
        };

        let class = effects.total_scaled(
            &super::account::Item {
                index: u16::from(self.flags),
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

        if self.equip[14] != 0
            && let Some(value) = patch.mount_refine
        {
            self.refine[14] = value;
        }

        self.flags = 0;

        true
    }

    pub fn hands(&self) -> (i32, i32) {
        (
            i32::from(self.equip[7] as i16),
            i32::from(self.equip[6] as i16),
        )
    }

    pub fn costume_burst(&self) -> Option<(u8, u8)> {
        match self.equip[COSTUME_SLOT] == COSTUME_TRIGGER && self.effect[COSTUME_SLOT] != 0 {
            true => Some((self.effect[COSTUME_SLOT], self.refine[COSTUME_SLOT])),
            false => None,
        }
    }

    pub fn silent(&self, standing: (i16, i32), incoming: (i16, i32)) -> bool {
        standing == incoming
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; EQUIP_PAYLOAD_LEN];

        for (n, id) in self.equip.iter().enumerate() {
            out[n * 2..n * 2 + 2].copy_from_slice(&id.to_le_bytes());
        }

        out[REFINE_OFF..REFINE_OFF + EQUIP_SLOTS].copy_from_slice(&self.refine);

        out[EFFECT_OFF..EFFECT_OFF + EQUIP_SLOTS].copy_from_slice(&self.effect);

        out[FLAGS_OFF] = self.flags;

        out
    }
}

pub const REFINE_OFF: usize = EQUIP_SLOTS * 2;

pub const EFFECT_OFF: usize = REFINE_OFF + EQUIP_SLOTS;

pub const FLAGS_OFF: usize = EFFECT_OFF + EQUIP_SLOTS;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct Gold {
    pub gold: u32,
}

impl Gold {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.gold.to_le_bytes().to_vec()
    }
}

fn need(payload: &[u8], len: usize) -> Result<()> {
    match payload.len() < len {
        true => Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: len,
            len: payload.len(),
        }),
        false => Ok(()),
    }
}

pub fn parse_vitals(payload: &[u8]) -> Result<Vitals> {
    need(payload, VITALS_PAYLOAD_LEN)?;

    Ok(Vitals {
        hp: u32_at(payload, 0x00, KIND)? as i32,
        mp: u32_at(payload, 0x04, KIND)? as i32,
        unknown_08: u32_at(payload, 0x08, KIND)?,
        unknown_0c: u32_at(payload, 0x0C, KIND)?,
    })
}

pub fn parse_hp_delta(payload: &[u8]) -> Result<HpDelta> {
    need(payload, HP_DELTA_PAYLOAD_LEN)?;

    Ok(HpDelta {
        hp: u32_at(payload, 0x00, KIND)? as i32,
        delta: u16_at(payload, 0x04, KIND)? as i16,
    })
}

pub fn parse_slot_patch(payload: &[u8]) -> Result<SlotPatch> {
    need(payload, VITALS_PAYLOAD_LEN)?;

    Ok(SlotPatch {
        container: Container::of(u16_at(payload, 0x00, KIND)? as i16),
        slot: u16_at(payload, 0x02, KIND)? as i16,
        item: account::item_at(payload, 0x04)?,
    })
}

pub fn parse_actor_status(payload: &[u8]) -> Result<ActorStatus> {
    need(payload, ACTOR_STATUS_PAYLOAD_LEN)?;

    let mut score = account::status_at(payload, 0)?;

    score.hp = score.hp.min(score.max_hp);

    let mut words = [0u16; 6];

    for (n, word) in words.iter_mut().enumerate() {
        *word = u16_at(payload, 0x8E + n * 2, KIND)?;
    }

    let mut player_words = [0u16; 3];

    for (n, word) in player_words.iter_mut().enumerate() {
        *word = u16_at(payload, 0x9A + n * 2, KIND)?;
    }

    Ok(ActorStatus {
        score,
        critical: payload[0x30],
        mana_saving: payload[0x31],
        affects: Affect::all(&payload[ACTOR_STATUS_AFFECT_OFF..]),
        guild: u16_at(payload, 0x72, KIND)?,
        unknown_74: u16_at(payload, 0x74, KIND)?,
        unknown_76: u32_at(payload, 0x76, KIND)?,
        unknown_7a: u32_at(payload, 0x7A, KIND)?,
        restore_alternate_hp: u32_at(payload, 0x80, KIND)?,
        unknown_84: u32_at(payload, 0x84, KIND)?,
        magic_bonus: u16_at(payload, 0x88, KIND)?,
        transformation_variant: payload[0x8C],
        words,
        player_words,
        mount_hp_bonus: u16_at(payload, 0xA0, KIND)?,
    })
}

pub fn parse_player_progression(payload: &[u8]) -> Result<PlayerProgression> {
    need(payload, PLAYER_PROGRESSION_PAYLOAD_LEN)?;

    let mut words = [0u16; 3];

    for (n, word) in words.iter_mut().enumerate() {
        *word = u16_at(payload, 0x14 + n * 2, KIND)?;
    }

    Ok(PlayerProgression {
        cp: u32_at(payload, 0x00, KIND)?,
        exp: u64::from(u32_at(payload, 0x04, KIND)?)
            | u64::from(u32_at(payload, 0x08, KIND)?) << 32,
        mask: [u32_at(payload, 0x0C, KIND)?, u32_at(payload, 0x10, KIND)?],
        words,
        gold: u32_at(payload, 0x1C, KIND)?,
    })
}

pub fn parse_equip(payload: &[u8]) -> Result<EquipChange> {
    need(payload, FLAGS_OFF + 1)?;

    let mut equip = [0u16; EQUIP_SLOTS];

    for (slot, id) in equip.iter_mut().enumerate() {
        *id = u16_at(payload, slot * 2, KIND)?;
    }

    let mut refine = [0u8; EQUIP_SLOTS];

    refine.copy_from_slice(&payload[REFINE_OFF..REFINE_OFF + EQUIP_SLOTS]);

    let mut effect = [0u8; EQUIP_SLOTS];

    effect.copy_from_slice(&payload[EFFECT_OFF..EFFECT_OFF + EQUIP_SLOTS]);

    Ok(EquipChange {
        equip,
        refine,
        effect,
        flags: payload[FLAGS_OFF],
    })
}

pub fn parse_gold(payload: &[u8]) -> Result<Gold> {
    need(payload, GOLD_PAYLOAD_LEN)?;

    Ok(Gold {
        gold: u32_at(payload, 0x00, KIND)?,
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct Vanish {
    pub mode: i32,
}

impl Vanish {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.mode.to_le_bytes().to_vec()
    }
}

pub fn parse_vanish(payload: &[u8]) -> Result<Vanish> {
    need(payload, VANISH_PAYLOAD_LEN)?;

    Ok(Vanish {
        mode: u32_at(payload, 0x00, KIND)? as i32,
    })
}

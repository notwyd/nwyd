use formats::net::attack::{self as wire, Attack, Hit};

mod emission;

use super::actors::{Actor, Model};
use super::damage::{self, Number};
use super::field::FieldScene;
use super::hover::PLAYER_ID_LIMIT;
use super::interact::{NPC_BODIES, NPC_BODY_33, NPC_BODY_43};
use super::skill_fx;
pub use emission::{automatic_flags, weapon_variant};
use formats::mesh::appearance;
use formats::world::attribute_map::{AttributeMap, flags};
use formats::world::path::{self, HeightField};

pub const ATTACK_INTERVAL_MS: u32 = 1_000;

pub const AUTO_INTERVAL_MS: u32 = 0xC8;

pub const PURSUE_INTERVAL_MS: u32 = 1_000;

pub const REACH_EFFECT: u8 = 0x1B;

pub const REACH_BONUS_BIT: u32 = 0x2000_0000;

pub const REACH_FLOOR: i32 = 2;

pub const REACH_FLOOR_CLASS: u8 = 3;

pub const REACH_FLOOR_BIT: u32 = 0x0008_0000;

pub const REACH_SLOTS: usize = 18;

pub const PURSUE_STEPS: i32 = 500;

pub const INVISIBLE_CLASS: i32 = 0x42;

pub const SEE_INVISIBLE: bool = false;

pub const ALLIED_GUILD_INERT: u16 = 0;

pub const RETALIATION_SLOTS: usize = 20;

pub const RETALIATION_WINDOW_MS: u32 = 5_000;

pub const AUTO_TOGGLE_KEYS: [char; 2] = ['y', 'Y'];

pub fn npc_bodies() -> [i32; 7] {
    let [a, b, c, d, e] = NPC_BODIES;

    [a, b, c, d, e, NPC_BODY_33, NPC_BODY_43]
}

pub const ATTACKABLE_GRADES: [u8; 2] = [0, 0xF];

pub const AUTO_LOCK_STAMPS: [usize; 3] = [4, 3, 1];

pub fn manual_war_area(position: glam::Vec3) -> bool {
    let x = position.x.trunc() as i32;

    let z = position.z.trunc() as i32;

    matches!(x >> ZONE_SHIFT, 8 | 9) && matches!(z >> ZONE_SHIFT, 15 | 16)
}

pub fn auto_war_area(position: glam::Vec3) -> bool {
    let x = position.x.trunc() as i32;

    let z = position.z.trunc() as i32;

    (0x10 < (x >> ZONE_SHIFT) && (x >> ZONE_SHIFT) < 0x14) && 0xE80 < (z & !0x7F)
}

pub const CANNON_ATTACK_PERMISSION: bool = false;

pub fn chase_block(actor: &Actor) -> u8 {
    actor.flags.get(formats::net::affect::Flags::PAIR)
}

pub fn melee_block(actor: &Actor) -> u8 {
    actor.flags.get(formats::net::affect::Flags::PAIR + 1)
}

pub fn action_busy(actor: &Actor, models: &[Model], now_ms: u32) -> bool {
    if !(4..=9).contains(&actor.action) {
        return false;
    }

    let Some(model) = models.get(actor.model) else {
        return false;
    };

    let Some(mut frames) = model.animation.frames(actor.anim.slot) else {
        return false;
    };

    if frames > super::corpse::CLIP_TAIL_FRAMES {
        frames -= super::corpse::CLIP_TAIL_FRAMES;
    }

    let frames = u32::try_from(frames).unwrap_or(u32::MAX);

    let span = frames
        .wrapping_mul(actor.anim.sub_frame_ms)
        .wrapping_mul(super::corpse::SUB_STEPS_PER_FRAME);

    now_ms < actor.anim.start_ms.wrapping_add(span)
}

pub const TEXT_ANCHOR_LIFT: f32 = 1.0;

pub const MIDPOINT_CLASS: i32 = 0x38;

pub const MIDPOINT_SCALE: f32 = 0.5;

pub const MIDPOINT_REACH_DISCOUNT: i32 = 0xC;

pub const TEXT_DROP: f32 = 80.0;

pub const TEXT_CRIT_LIFT: f32 = 10.0;

pub const SPLIT_STEP_X: f32 = 10.0;

pub const SPLIT_STEP: f32 = 10.0;

pub const SPLIT_STEP_CRIT: f32 = 20.0;

pub const STAGGER_SKILL: i16 = 0x4F;

pub const STAGGER_MS: u32 = 0x64;

pub fn magic_skill(skill_id: i16) -> bool {
    (0..0x68).contains(&skill_id)
}

pub const HIDDEN_ZONE_X: [i32; 2] = [0x11, 0x13];

pub const HIDDEN_ZONE_Z: i32 = 0x1E;

pub const HIDDEN_ZONE_HOLE: (i32, i32) = (0x12, 0x1E);

pub const ZONE_SHIFT: u32 = 7;

pub const DAMAGE_SPLIT: i32 = 1;

pub fn safe_here(map: Option<&AttributeMap>, cell: (i32, i32)) -> bool {
    attribute_at(map, cell).is_some_and(|byte| byte & flags::BLOCKED != 0)
}

pub fn duel_here(map: Option<&AttributeMap>, cell: (i32, i32)) -> bool {
    attribute_at(map, cell).is_some_and(|byte| byte & flags::SAFE_ZONE != 0)
}

pub fn attribute_at(map: Option<&AttributeMap>, cell: (i32, i32)) -> Option<u8> {
    let quarter = |v: i32| (v + ((v >> 31) & 3)) >> 2;

    let (x, z) = (quarter(cell.0), quarter(cell.1));

    map?.at(usize::try_from(x).ok()?, usize::try_from(z).ok()?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Retaliation {
    pub attacker: Option<u16>,
    pub stamp_ms: u32,
    pub damage: i32,
}

impl Retaliation {
    pub fn fresh(&self, now_ms: u32) -> Option<u16> {
        let attacker = self.attacker?;

        (self.stamp_ms >= now_ms.wrapping_sub(RETALIATION_WINDOW_MS)).then_some(attacker)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub last_attack_ms: u32,
    pub cycle: usize,
    pub auto_cycle: usize,
    pub skill: u16,
    pub from: Option<(u16, u16)>,
    pub move_stop_last: (i32, i32),
    pub stuck: Option<u16>,
    pub approach_target: Option<u16>,
    pub last_pursue_ms: u32,
    pub auto: bool,
    pub auto_target: Option<u16>,
    pub last_auto_ms: u32,
    pub clicked: Option<u16>,
    pub retaliation: [Retaliation; RETALIATION_SLOTS],
}

impl Default for State {
    fn default() -> State {
        State {
            last_attack_ms: 0,
            cycle: 0,
            auto_cycle: 0,
            skill: 0,
            from: None,
            move_stop_last: (0, 0),
            stuck: None,
            approach_target: None,
            last_pursue_ms: 0,
            auto: true,
            auto_target: None,
            last_auto_ms: 0,
            clicked: None,
            retaliation: [Retaliation::default(); RETALIATION_SLOTS],
        }
    }
}

impl State {
    pub fn remember_move(&mut self, action: &formats::net::action::Action) {
        self.move_stop_last = action.from();

        self.from = Some((action.dest_x, action.dest_z));
    }

    pub fn toggle_auto(&mut self) -> bool {
        self.auto = !self.auto;

        if !self.auto {
            self.auto_target = None;
        }

        self.auto
    }

    pub fn hit_by(&mut self, attacker: u16, damage: i32, now_ms: u32) -> Option<usize> {
        let slot = self
            .retaliation
            .iter()
            .position(|entry| entry.attacker == Some(attacker))
            .or_else(|| {
                self.retaliation
                    .iter()
                    .position(|entry| entry.attacker.is_none())
            })?;

        let entry = &mut self.retaliation[slot];

        entry.stamp_ms = now_ms;

        entry.damage = entry.damage.wrapping_add(damage);

        entry.attacker = Some(attacker);

        Some(slot)
    }

    pub fn despawned(&mut self, entity: u16) {
        for entry in self.retaliation.iter_mut() {
            if entry.attacker == Some(entity) {
                *entry = Retaliation::default();
            }
        }

        if self.stuck == Some(entity) {
            self.stuck = None;
        }

        if self.approach_target == Some(entity) {
            self.approach_target = None;
        }

        if self.auto_target == Some(entity) {
            self.auto_target = None;
        }
    }
}

pub fn invisible_body(target: &Actor) -> bool {
    target.class == INVISIBLE_CLASS && target.flags.invisible() && !SEE_INVISIBLE
}

pub fn guild_shields(scene: &FieldScene, index: usize, player: usize) -> bool {
    let theirs = scene.world.actors.actors[index].guild;

    let mine = scene.world.actors.actors[player].guild;

    (mine == theirs || scene.siege.ally == theirs) && mine != 0 && theirs != 0
}

pub fn attackable(target: &Actor) -> Option<&'static str> {
    if !target.highlight {
        return Some(crate::internal_reason!(
            "NWYD-R0001",
            "the cursor is not on it"
        ));
    }

    if target.tracked {
        return Some(crate::internal_reason!(
            "NWYD-R0002",
            "it is tracked or in your party"
        ));
    }

    if !target.shop_title.is_empty() {
        return Some(crate::internal_reason!(
            "NWYD-R0003",
            "it is running a personal shop"
        ));
    }

    if target.entity < PLAYER_ID_LIMIT {
        return None;
    }

    let grade = u8::try_from(target.status.flags & 0xF).unwrap_or(0);

    if !ATTACKABLE_GRADES.contains(&grade) {
        return Some(crate::internal_reason!(
            "NWYD-R0004",
            "its grade nibble is 1..=0xE — an NPC"
        ));
    }

    if npc_bodies().contains(&target.body) {
        return Some(crate::internal_reason!(
            "NWYD-R0005",
            "its body item is one of the seven NPC bodies"
        ));
    }

    None
}

pub fn refuse(scene: &FieldScene, index: usize, player: usize) -> Option<&'static str> {
    let target = &scene.world.actors.actors[index];

    let me = &scene.world.actors.actors[player];

    let is_player = target.entity < PLAYER_ID_LIMIT;

    let map = scene.world.actors.attributes();

    let target_duel = duel_here(map, cell_of(target));

    let me_duel = duel_here(map, cell_of(me));

    let grade = u8::try_from(target.status.flags & 0xF).unwrap_or(0);

    if invisible_body(target) {
        return Some(crate::internal_reason!(
            "NWYD-R0006",
            "it is an invisible class-0x42 body and you cannot detect invisibility"
        ));
    }

    if target.tracked {
        return Some(crate::internal_reason!(
            "NWYD-R0002",
            "it is tracked or in your party"
        ));
    }

    if target.guild != 0 && (me.guild == target.guild || scene.siege.ally == target.guild) {
        return Some(crate::internal_reason!(
            "NWYD-R0007",
            "it is in your guild or your ally's"
        ));
    }

    if is_player && !(target_duel && me_duel) {
        return Some(crate::internal_reason!(
            "NWYD-R0008",
            "players may only fight on a duel cell"
        ));
    }

    if melee_block(me) != 0 {
        return Some(crate::internal_reason!(
            "NWYD-R0009",
            "an affect has the player's melee-block flag set"
        ));
    }

    if !scene.pk && target.green_name {
        let same_positive_kingdom = me.kingdom > 0 && me.kingdom == target.kingdom;

        if target.guild != 0 {
            if scene.siege.war_guild != target.guild && scene.siege.war_state == 0 {
                return Some(crate::internal_reason!(
                    "NWYD-R0010",
                    "a green-name target is outside the current guild war"
                ));
            }
        } else if scene.siege.war_state == 0 || same_positive_kingdom {
            return Some(crate::internal_reason!(
                "NWYD-R0011",
                "a green-name guildless target is protected by the war state"
            ));
        }
    }

    if !target.highlight {
        return Some(crate::internal_reason!(
            "NWYD-R0001",
            "the cursor is not on it"
        ));
    }

    if !is_player {
        if !ATTACKABLE_GRADES.contains(&grade) {
            return Some(crate::internal_reason!(
                "NWYD-R0004",
                "its grade nibble is 1..=0xE — an NPC"
            ));
        }

        if npc_bodies().contains(&target.body) {
            return Some(crate::internal_reason!(
                "NWYD-R0005",
                "its body item is one of the seven NPC bodies"
            ));
        }

        if grade == 0xF
            && (me.kingdom == 0
                || me.kingdom == 3
                || (target.kingdom > 0 && me.kingdom > 0 && target.kingdom == me.kingdom))
        {
            return Some(crate::internal_reason!(
                "NWYD-R0012",
                "a grade-0xF body's kingdom is protected"
            ));
        }
    }

    if !target.shop_title.is_empty() {
        return Some(crate::internal_reason!(
            "NWYD-R0003",
            "it is running a personal shop"
        ));
    }

    if safe_here(map, cell_of(me)) {
        return Some(crate::internal_reason!(
            "NWYD-R0013",
            "you are standing in a safe zone"
        ));
    }

    if safe_here(map, cell_of(target)) {
        return Some(crate::internal_reason!(
            "NWYD-R0014",
            "it is standing in a safe zone"
        ));
    }

    if (is_player || target.tracked || target.green_name) && !target_duel {
        return Some(crate::internal_reason!(
            "NWYD-R0015",
            "a green-name body must stand on a duel cell"
        ));
    }

    if me.kingdom == 0
        && is_player
        && target_duel
        && !scene.pk
        && scene.siege.war_guild != target.guild
    {
        return Some(crate::internal_reason!(
            "NWYD-R0016",
            "a neutral-kingdom duel target is outside the war guild"
        ));
    }

    if !scene.pk && is_player {
        if scene.siege.war_state == 1 {
            if me.kingdom == target.kingdom {
                return Some(crate::internal_reason!(
                    "NWYD-R0017",
                    "the active war protects a player in your kingdom"
                ));
            }
        } else if scene.siege.war_state == 0 {
            if scene.siege.war_guild != target.guild {
                return Some(crate::internal_reason!(
                    "NWYD-R0018",
                    "the quiet war state protects a player outside its guild"
                ));
            }

            if me.kingdom == target.kingdom {
                return Some(crate::internal_reason!(
                    "NWYD-R0019",
                    "the quiet war state protects a player in your kingdom"
                ));
            }
        }
    }

    let in_war_area = manual_war_area(me.position);

    if !scene.pk {
        if target.green_name
            && target.guild != 0
            && scene.siege.war_guild != target.guild
            && !CANNON_ATTACK_PERMISSION
        {
            return Some(crate::internal_reason!(
                "NWYD-R0020",
                "a green-name target is outside the war guild"
            ));
        }

        if scene.siege.war_state > 0 {
            if me.kingdom > 0 && me.kingdom == target.kingdom {
                return Some(crate::internal_reason!(
                    "NWYD-R0021",
                    "the active war protects a body in your kingdom"
                ));
            }

            if me.kingdom == 3 && (is_player || (target.kingdom > 0 && target.kingdom != 4)) {
                return Some(crate::internal_reason!(
                    "NWYD-R0022",
                    "kingdom 3 may attack only the active-war exception"
                ));
            }

            if !CANNON_ATTACK_PERMISSION && is_player && !in_war_area {
                return Some(crate::internal_reason!(
                    "NWYD-R0023",
                    "an active-war player target is outside the four-block area"
                ));
            }
        }
    } else if scene.siege.war_state > 0
        && me.kingdom > 0
        && me.kingdom == target.kingdom
        && !in_war_area
    {
        return Some(crate::internal_reason!(
            "NWYD-R0024",
            "PK mode protects the same kingdom outside the war area"
        ));
    }

    None
}

pub fn auto_refuse(scene: &FieldScene, index: usize, player: usize) -> Option<&'static str> {
    let target = &scene.world.actors.actors[index];

    let me = &scene.world.actors.actors[player];

    let is_player = target.entity < PLAYER_ID_LIMIT;

    let map = scene.world.actors.attributes();

    let target_grade = u8::try_from(target.status.flags & 0xF).unwrap_or(0);

    let me_grade = u8::try_from(me.status.flags & 0xF).unwrap_or(0);

    let same_positive_kingdom = me.kingdom > 0 && me.kingdom == target.kingdom;

    let in_war_area = auto_war_area(me.position);

    if !is_player
        && (!ATTACKABLE_GRADES.contains(&target_grade) || npc_bodies().contains(&target.body))
    {
        return Some(crate::internal_reason!(
            "NWYD-R0025",
            "the automatic target is an NPC"
        ));
    }

    if me_grade == 0xF && me.kingdom > 0 {
        return Some(crate::internal_reason!(
            "NWYD-R0026",
            "the grade-0xF local player has a positive kingdom"
        ));
    }

    if melee_block(me) != 0 {
        return Some(crate::internal_reason!(
            "NWYD-R0009",
            "an affect has the player's melee-block flag set"
        ));
    }

    if invisible_body(target) {
        return Some(crate::internal_reason!(
            "NWYD-R0006",
            "it is an invisible class-0x42 body and you cannot detect invisibility"
        ));
    }

    if safe_here(map, cell_of(me)) {
        return Some(crate::internal_reason!(
            "NWYD-R0013",
            "you are standing in a safe zone"
        ));
    }

    if safe_here(map, cell_of(target)) {
        return Some(crate::internal_reason!(
            "NWYD-R0014",
            "it is standing in a safe zone"
        ));
    }

    if super::cast::forbidden(cell_of(me)) {
        return Some(crate::internal_reason!(
            "NWYD-R0027",
            "the local player is inside the 9x9 forbidden square"
        ));
    }

    if AUTO_LOCK_STAMPS
        .iter()
        .any(|&slot| scene.lock.stamps[slot] != 0)
    {
        return Some(crate::internal_reason!(
            "NWYD-R0028",
            "a pending action blocks automatic attacks"
        ));
    }

    if scene
        .exit
        .is_some_and(|exit| exit.kind == super::field::ExitKind::Quit)
    {
        return Some(crate::internal_reason!(
            "NWYD-R0029",
            "the quit countdown is active"
        ));
    }

    if scene.pk {
        return None;
    }

    if target.green_name && target.guild != scene.siege.war_guild && target.guild != 0 {
        if scene.siege.war_state == 0 && !in_war_area {
            return Some(crate::internal_reason!(
                "NWYD-R0030",
                "a green automatic target is outside its quiet-war area"
            ));
        }

        if same_positive_kingdom {
            return Some(crate::internal_reason!(
                "NWYD-R0031",
                "a green automatic target shares the local kingdom"
            ));
        }
    }

    if scene.siege.war_state == 0 && !in_war_area {
        if target.guild != 0 && scene.siege.war_guild != target.guild {
            return Some(crate::internal_reason!(
                "NWYD-R0032",
                "the automatic target is outside the quiet guild war"
            ));
        }

        if scene.attack.auto
            && scene.siege.war_guild != 0
            && scene.siege.war_guild != target.guild
            && is_player
        {
            return Some(crate::internal_reason!(
                "NWYD-R0033",
                "the armed automatic target is a player outside the quiet guild war"
            ));
        }
    } else if same_positive_kingdom {
        return Some(crate::internal_reason!(
            "NWYD-R0034",
            "the automatic target shares the local kingdom"
        ));
    }

    None
}

fn manual_area_candidate(
    scene: &FieldScene,
    body: &Actor,
    player: usize,
    cell: (i32, i32),
) -> bool {
    if body.entity < PLAYER_ID_LIMIT {
        if !duel_here(scene.world.actors.attributes(), cell) || body.tracked {
            return false;
        }

        if scene.pk {
            return true;
        }

        if scene.siege.war_state == 0 && scene.siege.war_guild != body.guild {
            return false;
        }

        if (scene.siege.war_state as i32) > 0 {
            let me = &scene.world.actors.actors[player];

            if ((me.kingdom as i8) > 0 && me.kingdom == body.kingdom) || me.kingdom == 3 {
                return false;
            }

            return CANNON_ATTACK_PERMISSION;
        }

        return true;
    }

    scene.pk || !body.green_name || (body.guild != 0 && scene.siege.war_guild == body.guild)
}

fn auto_area_candidate(scene: &FieldScene, body: &Actor, player: usize, cell: (i32, i32)) -> bool {
    let me = &scene.world.actors.actors[player];

    if body.entity < PLAYER_ID_LIMIT {
        if !duel_here(scene.world.actors.attributes(), cell) || body.tracked {
            return false;
        }

        if scene.pk {
            return true;
        }

        if scene.siege.war_state == 0 {
            return scene.siege.war_guild == body.guild;
        }

        if (me.kingdom as i8) > 0 && me.kingdom == body.kingdom {
            return false;
        }

        if me.kingdom == 3 {
            return false;
        }

        return CANNON_ATTACK_PERMISSION;
    }

    scene.pk || !body.green_name || (body.guild != 0 && scene.siege.war_guild == body.guild)
}

pub fn reach(scene: &FieldScene) -> i32 {
    let effects = scene.statics.effects();

    let best = scene.entered.equipment.items[..REACH_SLOTS]
        .iter()
        .filter(|item| item.index != 0)
        .map(|item| effects.total_scaled(item, REACH_EFFECT))
        .max()
        .unwrap_or(0);

    let floor = best < REACH_FLOOR
        && scene.entered.class == REACH_FLOOR_CLASS
        && scene.entered.skill_mask[0] & REACH_FLOOR_BIT != 0;

    let best = match floor {
        true => REACH_FLOOR,
        false => best,
    };

    best + i32::from(scene.entered.skill_mask[0] & REACH_BONUS_BIT != 0)
}

pub fn cell_of(actor: &Actor) -> (i32, i32) {
    (actor.position.x as i32, actor.position.z as i32)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispatched {
    Nothing,
    Declined {
        entity: u16,
        why: &'static str,
    },
    Swung {
        entity: u16,
        opcode: u16,
        action: u8,
        latched: bool,
    },
    TooSoon {
        entity: u16,
    },
    Pursued {
        entity: u16,
        tile: (i32, i32),
    },
    Unreachable {
        entity: u16,
    },
}

impl Dispatched {
    pub fn consumed(&self) -> bool {
        matches!(
            self,
            Dispatched::Swung { .. } | Dispatched::TooSoon { .. } | Dispatched::Pursued { .. }
        )
    }
}

pub fn dispatch(scene: &mut FieldScene, shift: bool, now_ms: u32) -> Dispatched {
    let Some(entity) = scene.hovered else {
        scene.cc.target = None;

        return Dispatched::Nothing;
    };

    let Some(index) = scene.world.actors.entity(entity) else {
        return Dispatched::Nothing;
    };

    let Some(player) = scene.player else {
        return Dispatched::Nothing;
    };

    if index == player {
        return Dispatched::Nothing;
    }

    if let Some(why) = refuse(scene, index, player) {
        return Dispatched::Declined { entity, why };
    }

    swing(scene, entity, index, player, true, shift, now_ms)
}

pub fn settled_tick(scene: &mut FieldScene, now_ms: u32) -> Dispatched {
    let Some(entity) = scene.attack.stuck else {
        return Dispatched::Nothing;
    };

    if scene.attack.auto {
        scene.attack.auto_target = Some(entity);
    }

    let Some(index) = scene.world.actors.entity(entity) else {
        return Dispatched::Nothing;
    };

    let Some(player) = scene.player else {
        return Dispatched::Nothing;
    };

    if let Some(why) = auto_refuse(scene, index, player) {
        return Dispatched::Declined { entity, why };
    }

    scene.attack.auto_target = Some(entity);

    swing(
        scene,
        entity,
        index,
        player,
        false,
        shift_held(scene),
        now_ms,
    )
}

pub fn auto_tick(scene: &mut FieldScene, now_ms: u32) -> Dispatched {
    if !scene.attack.auto {
        return Dispatched::Nothing;
    }

    let Some(entity) = scene.attack.auto_target else {
        return Dispatched::Nothing;
    };

    let Some(index) = scene.world.actors.entity(entity) else {
        scene.attack.auto_target = None;

        return Dispatched::Nothing;
    };

    if scene.world.actors.actors[index].is_dead {
        scene.attack.auto_target = None;

        return Dispatched::Nothing;
    }

    if now_ms.wrapping_sub(scene.attack.last_auto_ms) <= AUTO_INTERVAL_MS {
        return Dispatched::Nothing;
    }

    let Some(player) = scene.player else {
        return Dispatched::Nothing;
    };

    if guild_shields(scene, index, player) {
        let why = crate::internal_reason!("NWYD-R0191", "it is in your guild");

        return Dispatched::Declined { entity, why };
    }

    let unlatch = invisible_body(&scene.world.actors.actors[index]);

    scene.attack.last_auto_ms = now_ms;

    let dispatched = match auto_refuse(scene, index, player) {
        Some(why) => Dispatched::Declined { entity, why },
        None => swing(
            scene,
            entity,
            index,
            player,
            false,
            shift_held(scene),
            now_ms,
        ),
    };

    if unlatch {
        scene.attack.auto_target = None;
    }

    dispatched
}

fn swing(
    scene: &mut FieldScene,
    entity: u16,
    index: usize,
    player: usize,
    manual: bool,
    shift: bool,
    now_ms: u32,
) -> Dispatched {
    if now_ms <= scene.attack.last_attack_ms.wrapping_add(ATTACK_INTERVAL_MS) {
        return Dispatched::TooSoon { entity };
    }

    let reach = reach(scene);

    let here = cell_of(&scene.world.actors.actors[player]);

    let there = cell_of(&scene.world.actors.actors[index]);

    let distance = path::tile_distance(here, there);

    let midpoint_reach = {
        let target = &scene.world.actors.actors[index];

        target.class == MIDPOINT_CLASS && target.parts[0].mesh == 0
    };

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let blocked = path::line_of_sight(&field, here, there, path::SIGHT_TOLERANCE).is_some();

    let distance = if midpoint_reach {
        if manual && super::cast::forbidden(here) {
            return Dispatched::Declined {
                entity,
                why: crate::internal_reason!(
                    "NWYD-R0192",
                    "the local player is inside the siege target's 9x9 forbidden square"
                ),
            };
        }

        distance.saturating_sub(MIDPOINT_REACH_DISCOUNT).max(0)
    } else {
        distance
    };

    if manual {
        scene.attack.cycle = (scene.attack.cycle + 1) % 3;

        scene.attack.clicked = Some(entity);

        scene.world.actors.actors[player].skill_child = Some(entity);
    } else {
        scene.attack.auto_cycle = (scene.attack.auto_cycle + 1) % 3;
    }

    if !manual && (distance > reach || blocked && !midpoint_reach) {
        return auto_pursue(scene, entity, player, here, there, reach, shift);
    }

    if distance <= reach {
        if blocked && !midpoint_reach {
            return Dispatched::Unreachable { entity };
        }

        return send(scene, entity, index, player, manual, distance, now_ms);
    }

    if shift {
        return Dispatched::Unreachable { entity };
    }

    if chase_block(&scene.world.actors.actors[player]) != 0 {
        return Dispatched::Unreachable { entity };
    }

    if action_busy(
        &scene.world.actors.actors[player],
        &scene.world.actors.models,
        now_ms,
    ) {
        return Dispatched::Unreachable { entity };
    }

    let Some(tile) = chase_cell(scene, here, there, reach) else {
        return Dispatched::Unreachable { entity };
    };

    scene.cc.target = Some(entity);

    scene.attack.last_pursue_ms = now_ms;

    scene.cc.auto_move = tile;

    Dispatched::Pursued { entity, tile }
}

fn shift_held(scene: &FieldScene) -> bool {
    scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.shift)
}

fn auto_pursue(
    scene: &mut FieldScene,
    entity: u16,
    player: usize,
    here: (i32, i32),
    there: (i32, i32),
    reach: i32,
    shift: bool,
) -> Dispatched {
    let me = &scene.world.actors.actors[player];

    if !scene.attack.auto || !matches!(me.action, 0 | 1) || chase_block(me) != 0 || shift {
        return Dispatched::Unreachable { entity };
    }

    let Some(tile) = automatic_chase_cell(here, there, reach) else {
        return Dispatched::Unreachable { entity };
    };

    scene.cc.auto_move = tile;

    super::field::order(scene, tile);

    Dispatched::Pursued { entity, tile }
}

pub fn automatic_chase_cell(
    mut here: (i32, i32),
    there: (i32, i32),
    reach: i32,
) -> Option<(i32, i32)> {
    let mut steps = 0;

    while reach < path::tile_distance(here, there) {
        if steps > PURSUE_STEPS {
            return None;
        }

        here.0 += (there.0 - here.0).signum();

        here.1 += (there.1 - here.1).signum();

        steps += 1;
    }

    Some(here)
}

fn send(
    scene: &mut FieldScene,
    entity: u16,
    index: usize,
    player: usize,
    manual: bool,
    distance: i32,
    now_ms: u32,
) -> Dispatched {
    let action = wire::ATTACK_ACTIONS[if manual {
        scene.attack.cycle
    } else {
        scene.attack.auto_cycle
    }];

    let here = cell_of(&scene.world.actors.actors[player]);

    let there = cell_of(&scene.world.actors.actors[index]);

    let from = scene.attack.from.unwrap_or((here.0 as u16, here.1 as u16));

    let mut attack = Attack {
        from,
        to: (there.0 as u16, there.1 as u16),
        attacker: scene.entered.client_id,
        skill: scene.attack.skill,
        action,
        echo: false,
        flags: 0,
        variant: 0,
        damage: wire::DAMAGE_NONE,
        skill_id: wire::SKILL_NONE,
        ..Attack::default()
    };

    attack.hits[0] = Hit {
        entity,
        damage: wire::DAMAGE_UNKNOWN,
    };

    if manual && scene.world.actors.actors[player].class == TRANSFORMED_CLASS {
        match distance < TRANSFORMED_REACH {
            true => attack.action = wire::ATTACK_ACTIONS[0],
            false => {
                attack.action = wire::ATTACK_ACTIONS[1];

                attack.skill_id = TRANSFORMED_SKILL;

                attack.variant = TRANSFORMED_VARIANT;
            }
        }
    }

    apply_weapon(scene, &mut attack);

    if !manual {
        attack.flags = automatic_flags(
            &mut scene.attack.skill,
            scene.entered.derived.critical,
            scene.entered.derived.attack_speed,
        );
    }

    let opcode = match manual {
        true => area_sweep(scene, &mut attack, player, index),
        false => auto_area_sweep(scene, &mut attack, player, index),
    };

    if manual && scene.world.actors.actors[player].mover.moving && needs_move_stop(attack.skill_id)
    {
        let next = scene
            .attack
            .from
            .map(|(x, z)| (i32::from(x), i32::from(z)))
            .unwrap_or((0, 0));

        let current = (i32::from(attack.from.0), i32::from(attack.from.1));

        scene
            .outbox
            .push(protocol::client::move_stop(protocol::client::MoveStop {
                next,
                current,
                last: scene.attack.move_stop_last,
            }));
    }

    let emission = if manual {
        wire::Emission::Ordinary
    } else {
        automatic_emission(scene)
    };

    scene.outbox.push(attack.to_bytes_for(opcode, emission));

    scene.attack.last_attack_ms = now_ms;

    let latched = scene.attack.auto;

    if latched {
        scene.attack.auto_target = Some(entity);
    }

    scene.cc.target = None;

    let mut echo = attack;

    echo.echo = true;

    if scene.entered.skill_mask[0] & REACH_BONUS_BIT != 0 {
        echo.flags |= if manual { wire::FLAG_TRAIL } else { 2 };
    }

    apply(scene, opcode, &echo, now_ms);

    Dispatched::Swung {
        entity,
        opcode,
        action,
        latched,
    }
}

pub const TRANSFORMED_CLASS: i32 = 0x21;

pub const TRANSFORMED_REACH: i32 = 2;

pub const TRANSFORMED_SKILL: i16 = 0x69;

pub const TRANSFORMED_VARIANT: u8 = 2;

pub const WEAPON_SLOT: usize = 6;

pub const WEAPON_KIND_EFFECT: u8 = 0x15;

pub const WEAPON_SKILLS: [(i32, i16); 4] = [(0x65, 0x97), (0x66, 0x98), (0x67, 0x99), (0x68, 0x68)];

pub fn needs_move_stop(skill_id: i16) -> bool {
    skill_id == wire::SKILL_NONE || skill_id == 0x68 || skill_id > 0x96
}

pub fn weapon_skill(scene: &FieldScene) -> Option<i16> {
    let item = scene.entered.equipment.items.get(WEAPON_SLOT)?;

    if item.index == 0 {
        return None;
    }

    let kind = scene
        .statics
        .effects()
        .total_scaled(item, WEAPON_KIND_EFFECT);

    WEAPON_SKILLS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, record)| *record)
}

pub fn apply_weapon(scene: &FieldScene, attack: &mut Attack) {
    let item = &scene.entered.equipment.items[WEAPON_SLOT];

    let kind = scene
        .statics
        .effects()
        .total_scaled(item, WEAPON_KIND_EFFECT);

    let Some((_, record)) = WEAPON_SKILLS
        .iter()
        .find(|(candidate, _)| *candidate == kind)
    else {
        return;
    };

    attack.skill_id = *record;

    let appearance = item.appearance as i16;

    let id = if kind == 0x67 && (1..0x38a4).contains(&appearance) {
        appearance as usize
    } else {
        item.index as usize
    };

    if let Some(entry) = scene.statics.items.get(id)
        && let Some(variant) = weapon_variant(kind, entry.mesh)
    {
        attack.variant = variant;
    }
}

pub const AREA_CLASS: u8 = 3;

pub const AREA_ALWAYS_BIT: u32 = 0x0020_0000;

pub const AREA_SWEEP_BIT: u32 = 0x0000_0040;

pub(super) fn automatic_emission(scene: &FieldScene) -> wire::Emission {
    wire::Emission::Automatic {
        area_branch: scene.entered.class == AREA_CLASS
            && scene.entered.skill_mask[0] & (AREA_ALWAYS_BIT | AREA_SWEEP_BIT) != 0,
    }
}

pub fn area_sweep(scene: &FieldScene, attack: &mut Attack, player: usize, target: usize) -> u16 {
    area_sweep_with(scene, attack, player, target, true)
}

pub(super) fn auto_area_sweep(
    scene: &FieldScene,
    attack: &mut Attack,
    player: usize,
    target: usize,
) -> u16 {
    area_sweep_with(scene, attack, player, target, false)
}

fn area_sweep_with(
    scene: &FieldScene,
    attack: &mut Attack,
    player: usize,
    target: usize,
    manual: bool,
) -> u16 {
    if scene.entered.class != AREA_CLASS {
        return wire::ATTACK;
    }

    let mask = scene.entered.skill_mask[0];

    let always = mask & AREA_ALWAYS_BIT != 0;

    if mask & AREA_SWEEP_BIT == 0 {
        return match always {
            true => wire::ATTACK_AREA,
            false => wire::ATTACK,
        };
    }

    let here = cell_of(&scene.world.actors.actors[player]);

    let there = cell_of(&scene.world.actors.actors[target]);

    let step = |a: i32, b: i32| match (b - a).signum() {
        0 => 0,
        n => n,
    };

    let cell = (
        there.0 + step(here.0, there.0),
        there.1 + step(here.1, there.1),
    );

    let swept = scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .rev()
        .filter(|&(index, _)| index != player && index != target)
        .filter(|(_, body)| cell_of(body) == cell)
        .find(|(_, body)| {
            !body.is_dead
                && match manual {
                    true => manual_area_candidate(scene, body, player, cell),
                    false => auto_area_candidate(scene, body, player, cell),
                }
        })
        .map(|(_, body)| body.entity);

    if let Some(entity) = swept {
        attack.hits[1] = Hit {
            entity,
            damage: wire::DAMAGE_UNKNOWN,
        };
    }

    match !always && swept.is_none() {
        true => wire::ATTACK,
        false => {
            attack.used = wire::HITS_AREA;

            wire::ATTACK_AREA
        }
    }
}

pub fn chase_cell(
    scene: &FieldScene,
    here: (i32, i32),
    there: (i32, i32),
    reach: i32,
) -> Option<(i32, i32)> {
    let (mut x, mut z) = match scene.attack.from {
        Some((fx, fz)) => (i32::from(fx), i32::from(fz)),
        None => here,
    };

    if (x, z) == there {
        return None;
    }

    let step_x = if x < there.0 { 1 } else { -1 };

    let step_z = if z < there.1 { 1 } else { -1 };

    if reach < path::tile_distance((x, z), there) {
        let mut steps = 0;

        loop {
            if steps > PURSUE_STEPS {
                return None;
            }

            if x != there.0 {
                x += step_x;
            }

            if z != there.1 {
                z += step_z;
            }

            steps += 1;

            if reach >= path::tile_distance((x, z), there) {
                break;
            }
        }
    }

    Some((x, z))
}

#[derive(Debug, Clone)]
pub struct Applied {
    pub effect_context: Option<super::effect_context::Construction>,
    pub attacker: u16,
    pub index: Option<usize>,
    pub echo: bool,
    pub played: Option<usize>,
    pub trail: bool,
    pub hits: Vec<(Hit, Option<usize>)>,
    pub raised: usize,
    pub hurt: Vec<(u16, u32)>,
}

impl Applied {
    pub fn landed(&self) -> usize {
        self.hits
            .iter()
            .filter(|(_, index)| index.is_some())
            .count()
    }

    pub fn redraws(&self) -> bool {
        self.played.is_some() || self.trail || self.raised > 0 || !self.hurt.is_empty()
    }
}

pub fn apply(scene: &mut FieldScene, opcode: u16, attack: &Attack, now_ms: u32) -> Applied {
    let index = scene.world.actors.entity(attack.attacker);

    let trail = attack.flags & wire::FLAG_TRAIL != 0;

    let hits: Vec<(Hit, Option<usize>)> = attack
        .landed()
        .iter()
        .map(|hit| (*hit, scene.world.actors.entity(hit.entity)))
        .collect();

    let mut applied = Applied {
        effect_context: None,
        attacker: attack.attacker,
        index,
        echo: attack.echo,
        played: None,
        trail,
        hits,
        raised: 0,
        hurt: Vec::new(),
    };

    let Some(index) = index else {
        let ask = scene
            .roster
            .saw(attack.attacker, super::roster::FROM_ATTACK, now_ms);

        if let Some(line) = ask.describe() {
            tracing::info!("{line}");
        }

        if let Some(packet) = ask.packet() {
            scene.outbox.push(packet);
        }

        missing_caster_area(scene, opcode, attack, now_ms, &mut applied);

        return applied;
    };

    let target = applied.hits.first().and_then(|(_, at)| *at);

    if scene.world.actors.actors[index].class == 32
        && attack.action == 4
        && target.is_some()
        && target == scene.player
    {
        scene.world.actors.actors[index].impact_grid_ms = now_ms;
    }

    let toward = match target {
        Some(at) => {
            let position = scene.world.actors.actors[at].position;

            [position.x, position.z]
        }
        None => [f32::from(attack.to.0) + 0.5, f32::from(attack.to.1) + 0.5],
    };

    if trail {
        let special = swing_kind(&scene.world.actors.actors[index]);

        let mounted = scene.world.actors.actors[index].mounted;

        for hand in scene.world.actors.actors[index].swings.iter_mut() {
            hand.start(now_ms, special, mounted);
        }
    }

    let motion = match attack.skill_id {
        0x68 => 0x27,
        skill => skill as i8,
    };

    let music_slot = scene.city.music_slot;

    let actor = &mut scene.world.actors.actors[index];

    let pick = super::swing::trail_pick(
        u32::from(actor.entity),
        motion,
        music_slot,
        actor.body as i16,
    );

    for hand in &mut actor.swings {
        hand.adopt_trail(pick);
    }

    scene.world.actors.actors[index]
        .mover
        .look_at(toward, now_ms);

    let action = usize::from(attack.action);

    let is_local_player = Some(index) == scene.player;

    let caster = super::skill_anim::Caster::of(&scene.world.actors.actors[index], is_local_player);

    let caster_position = glam::Vec2::new(
        scene.world.actors.actors[index].position.x,
        scene.world.actors.actors[index].position.z,
    );

    let animation = match target {
        Some(at) => {
            let target = &scene.world.actors.actors[at];

            scene.world.actors.skill_anim.retain_targeted_anchor(
                caster.entity,
                false,
                caster_position,
                super::skill_anim::AnchorTarget {
                    position: glam::Vec2::new(target.position.x, target.position.z),
                    model: target.model_id,
                    scale: target.scale,
                    class: target.class,
                    part0_mesh: target.parts[0].mesh,
                },
                attack.skill_id as u16,
            );

            scene.world.actors.skill_anim.targeted(
                caster,
                action,
                attack.skill_id as u16,
                scene.statics.skills.as_ref(),
            )
        }
        None => {
            scene.world.actors.skill_anim.retain_ground_anchor(
                caster.entity,
                false,
                caster_position,
                glam::Vec2::new(f32::from(attack.to.0), f32::from(attack.to.1)),
                attack.skill_id as i8,
            );

            scene.world.actors.skill_anim.ground(
                caster,
                action,
                attack.skill_id as i8,
                scene.statics.skills.as_ref(),
            )
        }
    };

    let models = &scene.world.actors.models;

    let actor = &mut scene.world.actors.actors[index];

    match animation {
        super::skill_anim::Cast::Fallback(Some(action)) => {
            actor.play(action, now_ms, models);

            applied.played = Some(actor.action);
        }
        super::skill_anim::Cast::Chain(chain) => {
            if chain.stand02_first {
                actor.play(super::skill_anim::STAND02, now_ms, models);
            }

            if let Some(action) = chain.play {
                actor.play(action, now_ms, models);

                applied.played = Some(actor.action);
            }
        }
        super::skill_anim::Cast::Fallback(None) | super::skill_anim::Cast::Silent(_) => {}
    }

    if let Some(at) = target
        && at != index
        && Some(at) != scene.player
        && attack.skill_id != 0x1B
    {
        let from = scene.world.actors.actors[index].position;

        scene.world.actors.actors[at]
            .mover
            .look_at([from.x, from.z], now_ms);
    }

    if attack.flags & wire::FLAG_VOLLEY != 0 {
        scene.world.actors.actors[index].volley.armed = true;
    }

    if skill_fx::supports_delayed_effect(attack.skill_id)
        && effect_admitted(attack, is_local_player)
    {
        let to = skill_destination(scene, index, target, attack);

        let entity = target.map(|at| scene.world.actors.actors[at].entity);

        scene.world.actors.actors[index].pending = skill_fx::schedule_effect(
            attack.skill_id,
            attack.variant,
            attack.action,
            entity,
            to,
            now_ms,
        );
    }

    let mut context = super::effect_context::Construction::capture(
        scene,
        index,
        attack.hits.first().map(|hit| hit.entity),
        &attack.hits,
    );

    if attack.skill_id == 4 && (scene.player != Some(index) || attack.echo || attack.action == 0xFE)
    {
        let actor = &mut scene.world.actors.actors[index];

        actor.flags.set(0xB30, 1);

        actor.skill_afterimage_window = Some(now_ms);
    }

    context.producer = u8::try_from(attack.skill_id)
        .ok()
        .map(|skill| (formats::vfx::Chain::Impact, skill));

    let target_fx = super::targets::raise_with(scene, attack, &context, now_ms);

    applied.raised += super::impact_particles::raise_immediate(scene, attack, &context, now_ms);

    applied.raised += super::meteor_storm::raise(scene, attack, opcode, &context, now_ms);

    applied.raised += super::impact_particles::raise(scene, attack, opcode, &context, now_ms);

    applied.raised += super::impact_particles::raise_grid(scene, attack, &context, now_ms);

    applied.raised +=
        super::impact_particles::raise_sparks(scene, attack, opcode, &context, now_ms);

    applied.raised += super::impact_particles::raise_projectile(scene, attack, &context, now_ms);

    applied.raised += super::impact_scene::raise(scene, attack, &context, now_ms);

    if attack.skill_id == 119 && effect_admitted(attack, is_local_player) {
        let actor = &mut scene.world.actors.actors[index];

        let spin = actor.spin.get_or_insert(Spin::default());

        spin.end = now_ms.wrapping_add(8000);

        spin.sample = now_ms;

        actor.play(8, now_ms, &scene.world.actors.models);
    }

    applied.raised += super::impact_special::raise(scene, attack, &context, now_ms);

    applied.raised += super::start::raise(scene, attack, opcode, &context, now_ms);

    applied.raised += super::impact_particles::raise_area(scene, attack, opcode, &context, now_ms);

    applied.effect_context = Some(context);

    if let Some(line) = target_fx.describe() {
        tracing::info!("{line}");
    }

    super::hit_reaction::schedule_hit_reactions(scene, opcode, attack, index, now_ms);

    let mut text_delay = 0;

    for (entry, (hit, at)) in applied.hits.clone().into_iter().enumerate() {
        let Some(at) = at else {
            continue;
        };

        applied.raised += entries(
            scene,
            attack,
            index,
            at,
            hit,
            entry,
            now_ms,
            &mut applied.hurt,
            &mut text_delay,
        );
    }

    let _ = opcode;

    applied
}

pub fn effect_admitted(attack: &Attack, local: bool) -> bool {
    !local || attack.echo || attack.action == 0xFE
}

fn missing_caster_area(
    scene: &mut FieldScene,
    opcode: u16,
    attack: &Attack,
    now: u32,
    applied: &mut Applied,
) {
    let row = if attack.skill_id == 0x68 {
        0x27
    } else {
        attack.skill_id as usize
    };

    if !scene
        .statics
        .skills
        .as_ref()
        .and_then(|s| s.get(row))
        .is_some_and(|s| matches!(s.target_shape, 3..=6))
    {
        return;
    }

    for (hit, at) in applied
        .hits
        .clone()
        .into_iter()
        .take(super::impact_particles::member_limit(opcode))
    {
        let Some(at) = at else {
            continue;
        };

        if hit.missed() {
            applied.raised += miss(scene, at, 0, attack, now);

            continue;
        }

        let heal = magic_skill(attack.skill_id)
            && super::spawn::heal_skill(&scene.statics, attack.skill_id)
            && !(-5..0).contains(&hit.damage);

        if !attack.echo {
            applied
                .hurt
                .extend(hurt_by(scene, at, hit.damage, DAMAGE_SPLIT, !heal));
        }

        if heal {
            applied.raised += healed(scene, at, hit, 0, attack, now);
        } else {
            let position = scene.world.actors.actors[at].position + glam::Vec3::Y;

            if project_text(scene, position.to_array()).is_some() {
                return;
            }
        }
    }

    let (x, z) = (f32::from(attack.to.0), f32::from(attack.to.1));

    let Some(y) =
        super::effect_context::height(&scene.world.actors.window, scene.world.actors.zone, x, z)
    else {
        return;
    };

    let cell = glam::Vec3::new(x, y, z);

    let source = glam::Vec3::new(
        f32::from(attack.from.0) + 0.5,
        y,
        f32::from(attack.from.1) + 0.5,
    );

    let victims: Vec<_> = attack
        .hits
        .iter()
        .map(|hit| {
            scene
                .world
                .actors
                .entity(hit.entity)
                .map(|i| scene.world.actors.actors[i].position)
        })
        .collect();

    applied.raised +=
        super::impact_particles::raise_area_at(scene, attack, opcode, source, 0.0, &victims, now);

    if attack.skill_id == 0x1A {
        let ground = super::shade::Ground::new(
            &scene.world.terrain,
            &scene.world.actors.heights,
            scene.world.streamed.iter().map(|s| &s.terrain).collect(),
        );

        let mut out = Vec::new();

        super::skill::flash::spawn_with_ground(
            source + glam::Vec3::Y * 0.5,
            0,
            now,
            &ground,
            &mut out,
        );

        applied.raised += out.len();

        for effect in out {
            scene.world.effects.spawn(effect);
        }
    }

    let cannon = super::effect_context::cannon_at(scene, source);

    let viewer = scene.player.map(|i| scene.world.actors.actors[i].position);

    let meteors =
        super::meteor_storm::area_launches(attack.skill_id, cell, source, cannon, viewer, now);

    if meteors.iter().any(|meteor| meteor.mode == 2) {
        super::cannon::arm(scene, cannon);
    }

    applied.raised += meteors.len();

    for meteor in meteors {
        scene.world.effects.spawn(Box::new(meteor));
    }
}

#[allow(clippy::too_many_arguments)]
fn entries(
    scene: &mut FieldScene,
    attack: &Attack,
    attacker: usize,
    target: usize,
    hit: Hit,
    entry: usize,
    now_ms: u32,
    hurt: &mut Vec<(u16, u32)>,
    text_delay: &mut u32,
) -> usize {
    if attack.echo && Some(attacker) == scene.player {
        return 0;
    }

    let per = DAMAGE_SPLIT;

    if hit.missed() {
        return miss(scene, target, *text_delay, attack, now_ms);
    }

    let heal = magic_skill(attack.skill_id)
        && super::spawn::heal_skill(&scene.statics, attack.skill_id)
        && !(-5..0).contains(&hit.damage);

    if !attack.echo {
        hurt.extend(hurt_by(scene, target, hit.damage, per, !heal));

        if Some(target) == scene.player {
            scene
                .attack
                .hit_by(attack.attacker, hit.damage / per, now_ms);
        }
    }

    match heal {
        true => healed(scene, target, hit, *text_delay, attack, now_ms),
        false => damaged(
            scene, attack, attacker, target, hit, per, entry, now_ms, text_delay,
        ),
    }
}

fn miss(
    scene: &mut FieldScene,
    target: usize,
    delay_ms: u32,
    attack: &Attack,
    now_ms: u32,
) -> usize {
    let actor = &scene.world.actors.actors[target];

    let at = [
        actor.position.x,
        actor.position.y + TEXT_ANCHOR_LIFT,
        actor.position.z,
    ];

    let Some(screen) = project_text(scene, at) else {
        return 0;
    };

    if !shows_numbers(scene, attack, target) {
        return 0;
    }

    let drop = (scene.layout.sy * TEXT_DROP).trunc();

    let where_ = (screen.0, (screen.1 + drop).trunc());

    raise(scene, "miss", where_, damage::KIND_RED, delay_ms, now_ms)
}

fn healed(
    scene: &mut FieldScene,
    target: usize,
    hit: Hit,
    delay_ms: u32,
    attack: &Attack,
    now_ms: u32,
) -> usize {
    let actor = &scene.world.actors.actors[target];

    let at = [
        actor.position.x,
        actor.position.y + TEXT_ANCHOR_LIFT,
        actor.position.z,
    ];

    let Some(screen) = project_text(scene, at) else {
        return 0;
    };

    let split = i32::try_from(attack.splits()).unwrap_or(1);

    let text = match hidden(scene, attack, target) {
        true => "?".to_string(),
        false => format!("+ {}", -(hit.damage / split)),
    };

    if !shows_numbers(scene, attack, target) {
        return 0;
    }

    let drop = (scene.layout.sy * TEXT_DROP).trunc();

    let where_ = (screen.0, (screen.1 + drop).trunc());

    raise(scene, &text, where_, damage::KIND_BLUE, delay_ms, now_ms)
}

#[allow(clippy::too_many_arguments)]
fn damaged(
    scene: &mut FieldScene,
    attack: &Attack,
    attacker: usize,
    target: usize,
    hit: Hit,
    _per: i32,
    entry: usize,
    now_ms: u32,
    text_delay: &mut u32,
) -> usize {
    let at = text_anchor(scene, attacker, target);

    let Some(screen) = project_text(scene, at) else {
        return 0;
    };

    let row = if attack.skill_id == 0x68 {
        0x27
    } else {
        attack.skill_id as usize
    };

    let area = scene
        .statics
        .skills
        .as_ref()
        .and_then(|skills| skills.get(row))
        .is_some_and(|skill| matches!(skill.target_shape, 3..=6));

    let extra = if !area && attack.flags & 2 != 0 {
        attack.hits.get(1).map_or(0, |hit| hit.damage)
    } else {
        0
    };

    let (parts, count) = damage_parts(hit.damage, attack.flags, area, extra);

    let show = shows_numbers(scene, attack, target)
        && (area || !matches!(attack.to, (1129, 1707) | (1116, 1707) | (1094, 1690)));

    let concealed = hidden(scene, attack, target);

    if show
        && parts.iter().any(|&part| part > 0)
        && let Some(player) = scene.player
    {
        scene.world.actors.actors[player].skill_critical = attack.critical();
    }

    let (mut kind, scale) = kind_for(scene, attack, attacker, target);

    if scale.is_some() && kind == damage::KIND_CRIT {
        kind = damage::KIND_CRIT_SMALL;
    }

    let sy = scene.layout.sy;

    let critical = damage::POP_KINDS.contains(&kind);

    let drop = (sy * TEXT_DROP).trunc();

    let mut raised = 0;

    for (k, &part) in parts[..count].iter().enumerate() {
        if part <= 0 {
            continue;
        }

        if area {
            *text_delay = text_delay.wrapping_add(stagger(attack, entry));
        }

        if let Some(mesh) = affect_hit_mesh(&scene.world.actors.actors[target], now_ms) {
            scene.world.effects.spawn(Box::new(mesh));
        }

        if !show {
            continue;
        }

        let text = if concealed {
            "?".to_string()
        } else if k == 0 && extra > 0 {
            format!("{part} + {extra}")
        } else {
            part.to_string()
        };

        let part_index = k;

        let k = k as f32;

        let repeated = entry > 0 && attack.hits[entry - 1].entity == hit.entity;

        let (x, y) = if area {
            let y = if critical {
                (screen.1 - sy * TEXT_CRIT_LIFT - k * SPLIT_STEP_CRIT * sy + drop).trunc()
            } else {
                (screen.1 - k * SPLIT_STEP * sy).trunc() + drop
            };

            (screen.0 - k * SPLIT_STEP_X, y)
        } else {
            ordinary_number_origin(
                screen,
                sy,
                part_index,
                attack.critical(),
                repeated.then_some(entry > 1 && attack.hits[entry - 2].entity == hit.entity),
                Some(target) == scene.player,
            )
        };

        let life = damage_text_lifetime(area, attack.critical(), count, part_index, repeated);

        let mut number =
            new_number_with_life(scene, &text, (x, y), kind, *text_delay, now_ms, life);

        if !area {
            number.mode = part_index;

            number.drift = damage::Drift(if attack.critical() { 9 } else { 1 });
        }

        if let Some(scale) = scale {
            number.scale = scale;
        }

        scene.damage.push(number);

        raised += 1;
    }

    raised
}

fn ordinary_number_origin(
    screen: (f32, f32),
    sy: f32,
    part: usize,
    critical: bool,
    repeated: Option<bool>,
    local: bool,
) -> (f32, f32) {
    let k = part as f32;

    let step = if critical { 40.0 } else { 20.0 };

    let Some(third) = repeated else {
        let (lift, drop) = if critical {
            (20.0, -80.0)
        } else {
            (10.0, -40.0)
        };

        return (
            screen.0,
            (screen.1 + (lift + k * step) * sy).trunc() - (drop * sy).trunc(),
        );
    };

    let (x, drop) = match (critical, local, third) {
        (true, _, true) => ((4.0 - k) * 10.0, -90.0),
        (true, _, false) => ((2.0 - k) * 10.0, -70.0),
        (false, true, _) => ((2.0 - k) * 10.0, -60.0),
        (false, false, true) => ((3.0 - k) * 10.0, -80.0),
        (false, false, false) => (15.0 - k * 10.0, -60.0),
    };

    (
        screen.0 + x,
        (screen.1 - k * step * sy).trunc() - (drop * sy).trunc(),
    )
}

fn damage_parts(damage: i32, flags: u8, area: bool, extra: i32) -> ([i32; 4], usize) {
    let bits = if area {
        [(8, 2), (16, 3), (32, 4)]
    } else {
        [(32, 4), (16, 3), (8, 2)]
    };

    let count = bits
        .into_iter()
        .find(|(bit, _)| flags & bit != 0)
        .map_or(1, |(_, n)| n);

    let total = if area {
        damage
    } else {
        damage.wrapping_sub(extra)
    };

    let share = if area {
        total / count as i32
    } else if count == 1 {
        total
    } else {
        (total.wrapping_mul(100) as u32 / (count as u32 * 100)) as i32
    };

    let mut parts = [0; 4];

    parts[..count].fill(share);

    if !area {
        parts[count - 1] = total.wrapping_sub(share.wrapping_mul(count as i32 - 1));

        for part in &mut parts {
            if !(1..=2_000_000).contains(part) {
                *part = 0;
            }
        }
    }

    (parts, count)
}

fn damage_text_lifetime(
    area: bool,
    critical: bool,
    count: usize,
    part: usize,
    repeated: bool,
) -> u32 {
    if area {
        return 1500;
    }

    if repeated {
        return if critical { 1800 } else { 1200 };
    }

    let lives = match (critical, count) {
        (false, 1) => [1200, 0, 0, 0],
        (false, 2) => [1200, 1500, 0, 0],
        (false, 3) => [1300, 1500, 1800, 0],
        (false, _) => [1300, 1500, 1700, 1900],
        (true, 1) => [1900, 0, 0, 0],
        (true, 2) => [1800, 2200, 0, 0],
        (true, 3) => [1700, 1900, 2200, 0],
        (true, _) => [1700, 1700, 1800, 1900],
    };

    lives[part]
}

fn affect_hit_mesh(actor: &Actor, now_ms: u32) -> Option<super::effect_mesh::EffectMesh> {
    if actor.flags.get(0xB58) != 1 {
        return None;
    }

    let Some(&(_, height)) = super::pick::PICK_VOLUME.get(actor.model_id as usize) else {
        tracing::warn!(
            entity = actor.entity,
            model = actor.model_id,
            "hit affect has no body height"
        );

        return None;
    };

    let point = if actor.mounted {
        actor.skill_anchor.unwrap_or(actor.position)
    } else {
        actor.position
    };

    let mut mesh = super::effect_mesh::EffectMesh::new(2838, 0xFF999999, actor.yaw, 4, now_ms);

    mesh.position = point
        + glam::Vec3::Y * (height * actor.scale * 0.5 + if actor.mounted { -0.3 } else { 0.3 });

    mesh.texture_bias = 413;

    mesh.lifetime_ms = 500;

    mesh.period_ms = 500;

    mesh.blend = 1;

    mesh.pulse = false;

    mesh.scale_xz = 2.5;

    mesh.scale_y = 2.5;

    Some(mesh)
}

pub fn stagger(attack: &Attack, entry: usize) -> u32 {
    match attack.skill_id == STAGGER_SKILL {
        true => u32::try_from(entry).unwrap_or(0) * STAGGER_MS,
        false => 0,
    }
}

fn skill_destination(
    scene: &FieldScene,
    attacker: usize,
    target: Option<usize>,
    attack: &Attack,
) -> glam::Vec3 {
    let Some(at) = target else {
        let cell = (i32::from(attack.to.0), i32::from(attack.to.1));

        let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

        return glam::Vec3::new(
            f32::from(attack.to.0),
            field.ground(cell.0, cell.1),
            f32::from(attack.to.1),
        );
    };

    let t = scene.world.actors.actors[at].position;

    let a = scene.world.actors.actors[attacker].position;

    let target = &scene.world.actors.actors[at];

    match target.class == MIDPOINT_CLASS && target.parts[0].mesh == 0 {
        true => glam::Vec3::new((a.x + t.x) * 0.5, t.y, (a.z + t.z) * 0.5),
        false => glam::Vec3::new(t.x, t.y, t.z),
    }
}

pub fn text_anchor(scene: &FieldScene, attacker: usize, target: usize) -> [f32; 3] {
    let t = scene.world.actors.actors[target].position;

    let a = scene.world.actors.actors[attacker].position;

    let body = &scene.world.actors.actors[target];

    match body.class == MIDPOINT_CLASS && body.parts[0].mesh == 0 {
        true => [(a.x + t.x) * 0.5, t.y - TEXT_ANCHOR_LIFT, (a.z + t.z) * 0.5],
        false => [t.x, t.y + TEXT_ANCHOR_LIFT, t.z],
    }
}

pub fn project_text(scene: &FieldScene, at: [f32; 3]) -> Option<(f32, f32)> {
    let (view, projection) = scene.projected?;

    let at = glam::Vec3::new(at[0], at[1], at[2]);

    let (x, y) = super::nameplate::project(&view, &projection, at, &scene.layout)?;

    Some((x.trunc(), y.trunc()))
}

pub fn shows_numbers(scene: &FieldScene, attack: &Attack, target: usize) -> bool {
    scene.world.actors.effects_detail.effects
        || Some(target) == scene.player
        || scene
            .player
            .and_then(|i| scene.world.actors.actors.get(i))
            .is_some_and(|body| body.entity == attack.attacker)
}

pub fn hidden(scene: &FieldScene, attack: &Attack, target: usize) -> bool {
    let actor = &scene.world.actors.actors[target];

    let (zx, zz) = (
        (actor.position.x as i32) >> ZONE_SHIFT,
        (actor.position.z as i32) >> ZONE_SHIFT,
    );

    if zx < HIDDEN_ZONE_X[0] || zx > HIDDEN_ZONE_X[1] {
        return false;
    }

    if zz < HIDDEN_ZONE_Z || (zx, zz) == HIDDEN_ZONE_HOLE {
        return false;
    }

    let me = scene.entered.client_id;

    let mob = |id: u16| id >= PLAYER_ID_LIMIT;

    !((attack.attacker == me && mob(actor.entity)) || (actor.entity == me && mob(attack.attacker)))
}

pub fn kind_for(
    scene: &FieldScene,
    attack: &Attack,
    attacker: usize,
    target: usize,
) -> (u8, Option<f32>) {
    let kind = match attack.critical() {
        true => match magic_skill(attack.skill_id) {
            true => damage::KIND_MAGIC_CRIT,
            false => damage::KIND_CRIT,
        },
        false if Some(target) == scene.player => damage::KIND_RED,
        false => match magic_skill(attack.skill_id) {
            true => damage::KIND_MAGIC,
            false => damage::KIND_YELLOW,
        },
    };

    let body = &scene.world.actors.actors[target];

    let small =
        body.class == MIDPOINT_CLASS && body.parts[0].mesh == 0 && Some(attacker) != scene.player;

    (kind, small.then_some(MIDPOINT_SCALE))
}

fn raise(
    scene: &mut FieldScene,
    text: &str,
    at: (f32, f32),
    kind: u8,
    delay_ms: u32,
    now_ms: u32,
) -> usize {
    let number = new_number(scene, text, at, kind, delay_ms, now_ms);

    scene.damage.push(number);

    1
}

fn new_number(
    scene: &FieldScene,
    text: &str,
    at: (f32, f32),
    kind: u8,
    delay_ms: u32,
    now_ms: u32,
) -> Number {
    new_number_with_life(scene, text, at, kind, delay_ms, now_ms, damage::LIFE_MS)
}

fn new_number_with_life(
    scene: &FieldScene,
    text: &str,
    at: (f32, f32),
    kind: u8,
    delay_ms: u32,
    now_ms: u32,
    life_ms: u32,
) -> Number {
    let items = damage::set_of(kind)
        .and_then(|set| damage::set_items(&scene.sets, set))
        .unwrap_or(&[]);

    Number::new(
        text,
        at,
        kind,
        delay_ms,
        life_ms,
        now_ms,
        items,
        &scene.layout,
    )
}

pub fn hurt_by(
    scene: &mut FieldScene,
    target: usize,
    damage: i32,
    split: i32,
    alternate: bool,
) -> Option<(u16, u32)> {
    let actor = &mut scene.world.actors.actors[target];

    hurt_actor(actor, damage, split, alternate).map(|after| (actor.entity, after))
}

fn hurt_actor(actor: &mut Actor, damage: i32, split: i32, alternate: bool) -> Option<u32> {
    let before = actor.status.hp;

    let alternate_before = actor.alternate_hp;

    let after = match actor
        .alternate_hp
        .as_mut()
        .filter(|hp| alternate && hp.active())
    {
        Some(hp) => u32::try_from(hp.hurt(damage).max(0))
            .unwrap_or(0)
            .min(actor.status.max_hp),
        None => {
            let after = i64::from(before) - i64::from(damage / split);

            u32::try_from(after.max(0))
                .unwrap_or(0)
                .min(actor.status.max_hp)
        }
    };

    actor.status.hp = after;

    if after == before && actor.alternate_hp == alternate_before {
        return None;
    }

    Some(after)
}

pub fn swing_kind(actor: &Actor) -> u8 {
    if actor.model_id == 3 || actor.model_id == 8 || actor.class == 0x28 {
        return 5;
    }

    match actor.weapon_class {
        0x65 => 2,
        0x29 => 3,
        0x67 => 4,
        _ => 1,
    }
}

pub fn describe(dispatched: &Dispatched) -> Option<String> {
    match dispatched {
        Dispatched::Nothing => None,
        Dispatched::Declined { entity, why } => {
            Some(format!("NWYD: entity {entity} cannot be attacked — {why}"))
        }
        Dispatched::Swung {
            entity,
            opcode,
            action,
            latched,
        } => Some(format!(
            "NWYD: sent {opcode:#05X} at entity {entity}, action {action}{}",
            match latched {
                true => " (latched for the auto-attack)",
                false => "",
            },
        )),
        Dispatched::TooSoon { .. } => None,
        Dispatched::Pursued { entity, tile } => Some(format!(
            "NWYD: walking to {tile:?} to reach entity {entity}"
        )),
        Dispatched::Unreachable { entity } => Some(format!(
            "NWYD: entity {entity} is out of reach and the line to it is broken"
        )),
    }
}

pub const SWING_ACTIONS: [usize; 3] = [
    appearance::ACTION_ATTACK1,
    appearance::ACTION_ATTACK2,
    appearance::ACTION_ATTACK3,
];

pub const SETTLE_ACTIONS: std::ops::RangeInclusive<usize> =
    appearance::ACTION_ATTACK1..=appearance::ACTION_SKILL03;

pub const REDIRECTED_ACTIONS: [usize; 2] = [0xD, 0xF];

pub const REDIRECT_TARGETS: [usize; 2] = [0x18, 0x1A];

pub(crate) fn advance_volley(
    actor: &mut Actor,
    models: &[Model],
    now_ms: u32,
    visible: bool,
) -> bool {
    if !actor.volley.armed {
        return false;
    }

    if visible && actor.entity < 1000 && actor.action == 3 {
        actor.volley.armed = false;

        return false;
    }

    let Some(model) = models.get(actor.model) else {
        return true;
    };

    if !visible {
        if let Some(frames) = model.animation.frames(actor.anim.slot) {
            let span = (frames as u32)
                .saturating_mul(actor.anim.sub_frame_ms)
                .saturating_mul(4);

            if now_ms.wrapping_sub(actor.anim.start_ms) > span && actor.action != 11 {
                actor.volley.armed = false;
            }
        }

        return true;
    }

    if matches!(actor.action, 11 | 13 | 15) {
        return false;
    }

    if !super::corpse::clip_over(actor, model, now_ms) {
        return true;
    }

    let next = match actor.action {
        4 if actor.class != 33 => Some(5),
        4 | 6 => Some(4),
        5 => Some(6),
        _ => None,
    };

    if let Some(next) = next {
        actor.anim.slot = 0;

        actor.play(next, now_ms, models);
    }

    actor.volley.armed = false;

    true
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Spin {
    end: u32,
    sample: u32,
    phase: u8,
    angle: f32,
}

pub(crate) fn tick_spin(actor: &mut Actor, models: &[Model], now: u32) {
    let Some(mut spin) = actor.spin.take() else {
        return;
    };

    let previous_angle = spin.angle;

    let delta = previous_angle - actor.mover.yaw_start;

    actor.mover.yaw = actor.yaw;

    spin.angle += 0.15;

    if spin.angle > 360.0 {
        spin.angle = 0.0;
    }

    let active = spin.sample < spin.end;

    actor.mover.yaw_target = previous_angle;

    if active {
        spin.sample = now;

        let left = spin.end.wrapping_sub(now);

        if match spin.phase {
            0 => left < 6000,
            1 => left < 4000,
            2 => left < 2000,
            _ => false,
        } {
            spin.phase += 1;

            actor.play_mode(actor.action, now, models, true);
        }

        actor.mover.yaw_target = spin.angle;
    } else {
        actor.play(0, now, models);
    }

    let mix = (now.wrapping_sub(actor.mover.rot_start_ms) as f32 * 0.005).min(1.0);

    if (actor.mover.yaw - actor.mover.yaw_target).abs() > 0.017453292 {
        actor.mover.yaw = delta * mix + if active { 0.0 } else { actor.mover.yaw_start };
    }

    if active {
        actor.spin = Some(spin);
    }
}

pub fn settle(actor: &mut Actor, models: &[Model], now_ms: u32) -> bool {
    const NEVER_SETTLED: [usize; 6] = [
        appearance::ACTION_STAND01,
        appearance::ACTION_STAND02,
        appearance::ACTION_WALK,
        appearance::ACTION_RUN,
        appearance::ACTION_DIE,
        appearance::ACTION_DEAD,
    ];

    if NEVER_SETTLED.contains(&actor.action) {
        return false;
    }

    if actor.emote_hold && REDIRECT_TARGETS.contains(&actor.action) {
        return false;
    }

    let Some(model) = models.get(actor.model) else {
        return false;
    };

    if !super::corpse::clip_over(actor, model, now_ms) {
        return false;
    }

    if let Some(index) = REDIRECTED_ACTIONS
        .iter()
        .position(|action| *action == actor.action)
    {
        actor.play(REDIRECT_TARGETS[index], now_ms, models);

        actor.emote_hold = actor.action == REDIRECT_TARGETS[index];

        return false;
    }

    actor.clear_hand_actions();

    actor.play(appearance::ACTION_STAND02, now_ms, models);

    actor.action = appearance::ACTION_STAND02;

    true
}

pub fn describe_applied(applied: &Applied) -> Option<String> {
    if applied.echo {
        return None;
    }

    applied.index?;

    let numbers: Vec<String> = applied
        .hits
        .iter()
        .filter(|(_, index)| index.is_some())
        .map(|(hit, _)| match hit.missed() {
            true => format!("{} missed", hit.entity),
            false => format!("{} took {}", hit.entity, hit.damage),
        })
        .collect();

    if numbers.is_empty() {
        return None;
    }

    Some(format!(
        "NWYD: entity {} swung — {} ({} number(s) raised)",
        applied.attacker,
        numbers.join(", "),
        applied.raised,
    ))
}

use super::actors::Actor;
use super::attack;
use super::cc;
use super::field::FieldScene;
use super::nameplate::PLAYER_ID_LIMIT;
use super::panels;
use formats::data::skill_data::{self, Skill};
use formats::mesh::appearance;
use formats::net::attack::{self as wire, Attack, Hit};
use formats::net::enter as wire_enter;
use formats::world::attribute_map::flags as attribute_flags;
use formats::world::path::{self, HeightField};
use std::ops::RangeInclusive;

pub const TABLE_BOUND: u8 = 0xF8;

pub const CONTENT_CAST_BLOCKER: crate::frame::PanelId = panels::CONTENT_PORTAL;

pub const CONTENT_HANDIWORK: crate::frame::PanelId = 0x19;

pub const WEAPON_GATES: [(u8, i32); 3] = [(0x4F, 0x65), (0x4B, 0x65), (0x5C, 0x29)];

pub const REFUSED_CODES: [u8; 2] = [0x63, 0x53];

pub const HANDIWORK_CODE: u8 = 0x54;

pub const CANNON_CODE: u8 = 0x61;

pub const CANNON_IDS: RangeInclusive<i32> = 0x3A99..=0x3AFC;

pub const CANNON_CLASS: i16 = 0x2EA;

pub const ATTRACT_CODE: u8 = 6;

pub const POINT_CODE: u8 = 0x62;

pub const BLINK_CODE: u8 = 0x49;

pub const BLINK_STEP_BUDGET: usize = 8;

pub const BLINK_ACTION_KIND: i32 = 6;

pub const BLINK_THROTTLE_MS: u32 = 1_000;

pub const BLINK_MISS_Y: f32 = -10_000.0;

pub const PARTY_SUMMON_CODE: u8 = 0x2A;

pub const PARTY_SUMMON_STRDEF: [usize; 2] = [31, 32];

pub const MANA_STRDEF: usize = 30;

pub const COIN_ARMOR_CODE: u8 = 0x55;

pub const COIN_ARMOR_PRICE: i32 = 100;

pub const COIN_ARMOR_STACKS: i32 = 0;

pub const SELF_SHAPES: [i32; 5] = [0, 3, 4, 5, 6];

pub const TARGET_CENTER_CODES: [u8; 9] = [0x00, 0x10, 0x07, 0x11, 0x17, 0x23, 0x27, 0x33, 0x37];

pub const CAST_SELECTED_ONLY: u8 = 0x5F;

pub const SKILL_ATTACK_SELECTED_ONLY: u8 = 0x4F;

pub fn area_radius(shape: i32) -> Option<i32> {
    match shape {
        3 => Some(1),
        4 => Some(2),
        6 => Some(3),
        _ => None,
    }
}

pub const SEND_GATE_CODES: [u8; 3] = [0x61, 0x23, 0x33];

pub const SOUL_CODE: u8 = 0x66;

pub const SOUL_PREREQUISITES: [i32; 3] = [0x1F, 0x27, 0x2F];

pub const SOUL_STRDEF: usize = 357;

pub const SELF_STAMP_MOVER_CODES: [u8; 2] = [0xC8, 0xD8];

pub const SELF_STAMP_CODES: RangeInclusive<u8> = 0x38..=0x3F;

pub const COOLDOWN_REFINE_SLOT: usize = 4;

pub const COOLDOWN_REFINE_THRESHOLD: u32 = 8;

pub const COOLDOWN_FLOOR_S: i32 = 1;

pub const COOLDOWN_DISCOUNT_FLOOR_S: i32 = 2;

pub const COOLDOWN_AFFECT_FLAG: usize = formats::net::affect::Flags::HIDE_A;

pub const FORBIDDEN_X: RangeInclusive<i32> = 0x93A..=0x942;

pub const FORBIDDEN_Z: RangeInclusive<i32> = 0xF57..=0xF5F;

pub const STAMP_0X1B: usize = 0x1B;

pub const STAMP_0X1D: usize = 0x1D;

pub const MODE2_1B_CODE: u8 = 0x1B;

pub const SKILL_ATTACK_PLAYER_CODES: [u8; 3] = [0x1B, 0x1D, 0x2C];

pub const KNIFE_CODE: u8 = 0x56;

pub const RANGE_WEAPON_KIND: i32 = 0x64;

pub const MIDPOINT_DISCOUNT: i32 = 12;

pub const MIDPOINT_PART0_MESH: i16 = 0;

pub const TALK_ANSWER: bool = false;

pub const OBJECT_ACTION_ANSWER: bool = true;

pub const TALK_SPECIAL_CURSOR: u8 = 0x1F;

pub const TALK_EXCLUDED_A: std::ops::RangeInclusive<i16> = 0x6C5..=0x6C8;

pub const TALK_EXCLUDED_B: std::ops::RangeInclusive<i16> = 0xC49..=0xC4D;

pub const TALK_LEGACY_PORTALS: std::ops::Range<i16> = 0x1004..0x1068;

pub const TALK_REEL_A: i16 = super::reel::TALK_A;

pub const TALK_REEL_B: i16 = super::reel::TALK_B;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cast {
    Refused(&'static str),
    Talked(&'static str),
    ObjectAction {
        why: &'static str,
        entity: u16,
        opcode: Option<u16>,
    },
    Approached {
        entity: u16,
        tile: (i32, i32),
        walking: bool,
    },
    Handled(&'static str),
    Blinked {
        code: u8,
        row: usize,
        tile: (u16, u16),
    },
    Sent(Sent),
    Continued {
        first: Sent,
        continuation: Box<Cast>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sent {
    pub opcode: u16,
    pub code: u8,
    pub row: usize,
    pub targets: usize,
}

impl Cast {
    pub fn reached_the_wire(&self) -> bool {
        matches!(
            self,
            Cast::ObjectAction {
                opcode: Some(_),
                ..
            } | Cast::Blinked { .. }
                | Cast::Sent(_)
                | Cast::Continued { .. }
        )
    }

    pub fn answer(&self) -> bool {
        match self {
            Cast::Refused(_) => false,
            Cast::Talked(_) => TALK_ANSWER,
            Cast::Approached { .. } => false,
            Cast::Continued { continuation, .. } => continuation.answer(),
            Cast::ObjectAction { .. } | Cast::Handled(_) | Cast::Blinked { .. } | Cast::Sent(_) => {
                OBJECT_ACTION_ANSWER
            }
        }
    }

    pub fn describe(&self) -> Option<String> {
        match self {
            Cast::Refused(_) => None,
            Cast::Talked(why) => Some(format!(
                "NWYD: the cast fell through to the NPC talk ({why})"
            )),
            Cast::ObjectAction {
                why,
                entity,
                opcode,
            } => Some(match opcode {
                Some(opcode) => {
                    format!("NWYD: object {entity} accepted {why} as {opcode:#05X}")
                }
                None => {
                    format!("NWYD: object {entity} accepted the cast fallback ({why})")
                }
            }),
            Cast::Approached {
                entity,
                tile,
                walking,
            } => Some(format!(
                "NWYD: approaching cast target {entity} at {},{} (walking={walking})",
                tile.0, tile.1,
            )),
            Cast::Handled(what) => Some(format!("NWYD: the cast was consumed ({what})")),
            Cast::Blinked { code, row, tile } => Some(format!(
                "NWYD: cast blink skill {code:#04X} (row {row}) as 0x368 to {},{}",
                tile.0, tile.1,
            )),
            Cast::Sent(sent) => Some(format!(
                "NWYD: cast skill {:#04X} (row {}) as {:#05X} at {} target(s)",
                sent.code, sent.row, sent.opcode, sent.targets,
            )),
            Cast::Continued {
                first,
                continuation,
            } => {
                let first = format!(
                    "NWYD: cast skill {:#04X} (row {}) as {:#05X} at {} target(s)",
                    first.code, first.row, first.opcode, first.targets,
                );

                let continuation = continuation
                    .describe()
                    .unwrap_or_else(|| "NWYD: the continuation was refused".to_string());

                Some(format!("{first}; then {continuation}"))
            }
        }
    }
}

pub fn armed(scene: &FieldScene) -> Option<(u8, usize)> {
    let code = wire_enter::selected(&scene.entered.quick, scene.entered.quick_selected)?;

    Some((code, skill_data::wire_skill_row(code)))
}

fn advance_auto_skill(scene: &mut FieldScene) {
    let Some(page) = scene.hud.as_ref().map(|hud| hud.page) else {
        return;
    };

    let present =
        std::array::from_fn(|slot| scene.entered.quick[slot] != formats::net::enter::QUICK_EMPTY);

    let Some(slot) = super::skill_bar::next_auto_slot(
        scene.entered.quick_selected,
        page,
        scene.skill_bar.auto_count,
        scene.attack.auto,
        &present,
    ) else {
        return;
    };

    scene.entered.quick_selected = slot as u8;

    if let Some(hud) = scene.hud.as_mut() {
        hud.select_skill_only(slot);
    }
}

pub fn weapon_kind(scene: &FieldScene) -> i32 {
    let Some(item) = scene.entered.equipment.items.get(attack::WEAPON_SLOT) else {
        return 0;
    };

    scene
        .statics
        .effects()
        .total_scaled(item, attack::WEAPON_KIND_EFFECT)
}

pub fn mastery(scene: &FieldScene, code: u8) -> i32 {
    let level = i32::from(scene.entered.status.level);

    if code >= u8::try_from(super::enter::CLASS_SKILL_CODES).unwrap_or(0x60) {
        return level;
    }

    let class = i32::from(scene.entered.class);

    let group = (i32::from(code) - class * super::enter::SKILLS_PER_CLASS) / 8;

    let Ok(group) = usize::try_from(group) else {
        return 0;
    };

    scene
        .entered
        .status
        .skill_mastery(group)
        .map_or(0, i32::from)
}

pub fn mana_cost(scene: &FieldScene, skill: &Skill, code: u8) -> i32 {
    skill.cost_at(
        mastery(scene, code),
        i32::from(scene.entered.derived.mana_saving),
    )
}

pub fn cooldown_refined(scene: &FieldScene) -> bool {
    scene
        .entered
        .equipment
        .items
        .get(COOLDOWN_REFINE_SLOT)
        .is_some_and(|item| {
            appearance::refine_level(item, &scene.statics.items) > COOLDOWN_REFINE_THRESHOLD
        })
}

pub fn cooldown_s(skill: &Skill, cooldown_refined: bool, cooldown_affected: bool) -> i32 {
    let mut cooldown = skill.cooldown_secs;

    if cooldown_refined && cooldown >= COOLDOWN_DISCOUNT_FLOOR_S {
        cooldown -= 1;
    }

    if cooldown_affected {
        cooldown -= 1;
    }

    cooldown.max(COOLDOWN_FLOOR_S)
}

pub fn cooldown_affected(actor: &Actor) -> bool {
    actor.flags.get(COOLDOWN_AFFECT_FLAG) == 1
}

pub fn forbidden(cell: (i32, i32)) -> bool {
    FORBIDDEN_X.contains(&cell.0) && FORBIDDEN_Z.contains(&cell.1)
}

pub fn hidden(actor: &Actor) -> bool {
    actor.entity < PLAYER_ID_LIMIT && actor.status.flags & 1 != 0
}

fn alt_held(scene: &FieldScene) -> bool {
    scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.alt)
}

pub fn rooted(actor: &Actor) -> bool {
    actor.flags.get(formats::net::affect::Flags::WORD) == 1
}

fn owns_store(scene: &FieldScene) -> bool {
    scene
        .panels
        .as_ref()
        .and_then(|panels| panels.store.as_ref())
        .is_some_and(|store| store.owner == scene.entered.client_id)
}

pub fn admissible(
    scene: &FieldScene,
    target: Option<&Actor>,
    skill: &Skill,
) -> Option<&'static str> {
    if let Some(target) = target
        && skill.offensive == 1
        && target.tracked
    {
        return Some(crate::internal_reason!(
            "NWYD-R0177",
            "a offensive skill will not point at a tracked body"
        ));
    }

    let (Some(target), Some(player)) = (target, scene.player) else {
        return None;
    };

    let me = &scene.world.actors.actors[player];

    if skill.offensive == 1
        && me.guild != 0
        && scene.siege.ally != 0
        && (me.guild == target.guild || scene.siege.ally == target.guild)
    {
        return Some(crate::internal_reason!(
            "NWYD-R0178",
            "a offensive skill will not point at your guild or its ally"
        ));
    }

    if scene.pk || skill.offensive != 1 {
        return None;
    }

    let duel = attack::duel_here(scene.world.actors.attributes(), attack::cell_of(target));

    let protected = if scene.siege.war_state == 0 {
        (me.kingdom as i8) > 0 && target.kingdom == 0
    } else {
        me.kingdom == target.kingdom
    };

    if protected && target.entity < PLAYER_ID_LIMIT && duel {
        return Some(crate::internal_reason!(
            "NWYD-R0179",
            "the duel target is protected by the early kingdom/war rule"
        ));
    }

    if target.green_name
        && target.guild != 0
        && target.guild != scene.siege.war_guild
        && (scene.siege.war_state == 0 || !attack::CANNON_ATTACK_PERMISSION)
    {
        return Some(crate::internal_reason!(
            "NWYD-R0180",
            "the green target is outside the early cast's war guild"
        ));
    }

    None
}

fn same_positive_kingdom(me: &Actor, target: &Actor) -> bool {
    (me.kingdom as i8) > 0 && me.kingdom == target.kingdom
}

fn allied_target(scene: &FieldScene, me: &Actor, target: &Actor) -> bool {
    target.guild != 0 && (target.guild == me.guild || target.guild == scene.siege.ally)
}

fn kingdom_three_protects(scene: &FieldScene, me: &Actor, target: &Actor) -> bool {
    !scene.pk
        && (scene.siege.war_state as i32) > 0
        && me.kingdom == 3
        && (target.entity < PLAYER_ID_LIMIT || ((target.kingdom as i8) > 0 && target.kingdom != 4))
}

fn foreign_duel_player(scene: &FieldScene, me: &Actor, target: &Actor, offensive: bool) -> bool {
    !scene.pk
        && offensive
        && target.entity < PLAYER_ID_LIMIT
        && (target.guild == 0 || target.guild != scene.siege.war_guild)
        && attack::duel_here(scene.world.actors.attributes(), attack::cell_of(target))
        && (scene.siege.war_state == 0 || same_positive_kingdom(me, target))
}

fn foreign_green_guild(scene: &FieldScene, me: &Actor, target: &Actor) -> bool {
    !scene.pk
        && target.green_name
        && target.guild != 0
        && target.guild != scene.siege.war_guild
        && (scene.siege.war_state == 0
            || (same_positive_kingdom(me, target) && !attack::CANNON_ATTACK_PERMISSION))
}

fn guildless_green(scene: &FieldScene, me: &Actor, target: &Actor) -> bool {
    !scene.pk
        && target.green_name
        && target.guild == 0
        && (scene.siege.war_state == 0 || same_positive_kingdom(me, target))
}

fn grade_f_protected(me: &Actor, target: &Actor, offensive: bool) -> bool {
    offensive
        && target.status.flags & 0xF == 0xF
        && (me.kingdom == 0 || same_positive_kingdom(me, target))
}

fn area_pk_protects_selected(scene: &FieldScene, selected: Option<usize>) -> bool {
    let (Some(player), Some(selected)) = (scene.player, selected) else {
        return false;
    };

    let me = &scene.world.actors.actors[player];

    let at = attack::cell_of(me);

    (scene.siege.war_state as i32) > 0
        && same_positive_kingdom(me, &scene.world.actors.actors[selected])
        && matches!(at.0 >> 7, 8 | 9)
        && matches!(at.1 >> 7, 15 | 16)
}

pub fn opcode_for(skill: &Skill) -> u16 {
    match skill.max_targets {
        1 => wire::ATTACK,
        2 => wire::ATTACK_AREA,
        _ => wire::ATTACK_MANY,
    }
}

pub fn send_gate(code: u8, hits: &[Hit]) -> bool {
    hits.first().is_some_and(|hit| hit.entity != 0) || SEND_GATE_CODES.contains(&code)
}

#[derive(Debug)]
struct Sweep {
    hits: Vec<Hit>,
    to: (u16, u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cannon {
    entity: u16,
}

#[derive(Debug)]
enum AreaExit {
    Answer(Cast),
    Approach(Approach),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Approach {
    Order { entity: u16, tile: (i32, i32) },
    Refused(&'static str),
    Handled { entity: u16, what: &'static str },
}

impl From<Cast> for AreaExit {
    fn from(answer: Cast) -> Self {
        AreaExit::Answer(answer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SweepCaller {
    Cast,
    SkillAttack,
}

fn sweep_origin(scene: &FieldScene, player: usize) -> (i32, i32) {
    scene.attack.from.map_or_else(
        || attack::cell_of(&scene.world.actors.actors[player]),
        |(x, z)| (i32::from(x), i32::from(z)),
    )
}

fn build_ground_cast_target(
    scene: &FieldScene,
    player: usize,
    at: (f32, f32, f32),
    skill: &Skill,
) -> Result<Sweep, Cast> {
    let origin = sweep_origin(scene, player);

    let click = (at.0.trunc() as i32, at.2.trunc() as i32);

    let initial_distance = path::tile_distance(origin, click);

    let reach_bonus = i32::from(scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0);

    let raw_range = skill.range.wrapping_add(reach_bonus);

    let changed_by_los = {
        let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

        path::line_of_sight(&field, origin, click, path::SIGHT_TOLERANCE).is_some()
    };

    let mut initial_range = if raw_range == -1 { 1 } else { raw_range };

    if weapon_kind(scene) < RANGE_WEAPON_KIND {
        initial_range = initial_range.max(attack::reach(scene));
    }

    if initial_distance > initial_range {
        return Err(Cast::Handled(crate::internal_reason!(
            "NWYD-R0035",
            "0x62's click is beyond its initial maximum reach"
        )));
    }

    if changed_by_los {
        return Err(Cast::Handled(crate::internal_reason!(
            "NWYD-R0036",
            "0x62's line-of-sight probe rewrote the clicked cell"
        )));
    }

    if path::tile_distance(origin, click) > raw_range {
        return Err(Cast::Handled(crate::internal_reason!(
            "NWYD-R0037",
            "0x62's click is beyond its post-LOS range reach"
        )));
    }

    let to = (click.0 as u16, click.1 as u16);

    let attribute_cell = (i32::from(to.0), i32::from(to.1));

    let attribute =
        attack::attribute_at(scene.world.actors.attributes(), attribute_cell).unwrap_or_default();

    if attribute & attribute_flags::BLOCKED != 0 && attribute & attribute_flags::SAFE_ZONE == 0 {
        return Err(Cast::Handled(crate::internal_reason!(
            "NWYD-R0038",
            "0x62's click has attribute bit 0 without the bit-6 override"
        )));
    }

    Ok(Sweep {
        hits: vec![Hit {
            entity: scene.entered.client_id,
            damage: wire::DAMAGE_NONE,
        }],
        to,
    })
}

fn midpoint(actor: &Actor) -> bool {
    actor.class == attack::MIDPOINT_CLASS && actor.parts[0].mesh == MIDPOINT_PART0_MESH
}

fn cannon_under(scene: &FieldScene, player: usize) -> Option<Cannon> {
    let cell = attack::cell_of(&scene.world.actors.actors[player]);

    CANNON_IDS.clone().find_map(|raw| {
        let entity = u16::try_from(raw).ok()?;

        let cannon = scene.drops.get(entity)?;

        (cannon.item.index as i16 == CANNON_CLASS
            && (
                cannon.position.x.trunc() as i32,
                cannon.position.z.trunc() as i32,
            ) == cell)
            .then_some(Cannon { entity })
    })
}

fn cannon_faces(yaw: f32, from: (i32, i32), to: (i32, i32)) -> bool {
    let mut facing = ((to.0 - from.0) as f32).atan2((to.1 - from.1) as f32);

    if facing < 0.0 {
        facing += std::f32::consts::TAU;
    }

    yaw - std::f32::consts::FRAC_PI_2 <= facing && facing <= yaw + std::f32::consts::FRAC_PI_2
}

fn approach_tile(from: (i32, i32), to: (i32, i32), range: i32) -> Option<(i32, i32)> {
    let mut tile = from;

    if path::tile_distance(tile, to) <= range {
        return Some(tile);
    }

    let step = ((to.0 - tile.0).signum(), (to.1 - tile.1).signum());

    for _ in 0..=500 {
        if tile.0 != to.0 {
            tile.0 += step.0;
        }

        if tile.1 != to.1 {
            tile.1 += step.1;
        }

        if path::tile_distance(tile, to) <= range {
            return Some(tile);
        }
    }

    None
}

fn apply_approach(scene: &mut FieldScene, plan: Approach, now_ms: u32) -> Cast {
    scene.attack.approach_target = match plan {
        Approach::Order { entity, .. } | Approach::Handled { entity, .. } => Some(entity),
        Approach::Refused(_) => None,
    };

    scene.attack.last_pursue_ms = now_ms;

    let Some(player) = scene.player else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));
    };

    if attack::chase_block(&scene.world.actors.actors[player]) != 0 {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0040",
            "an affect has the caster's chase-block flag set"
        ));
    }

    match plan {
        Approach::Order { entity, tile } => {
            scene.cc.auto_move = tile;

            Cast::Approached {
                entity,
                tile,
                walking: false,
            }
        }
        Approach::Refused(why) => Cast::Refused(why),
        Approach::Handled { what, .. } => Cast::Handled(what),
    }
}

fn war_admits(scene: &FieldScene, actor: &Actor, offensive: bool, area: bool) -> bool {
    let map = scene.world.actors.attributes();

    let duel = attack::duel_here(map, attack::cell_of(actor));

    war_admits_values(
        duel,
        offensive,
        actor.tracked,
        actor.entity < PLAYER_ID_LIMIT,
        actor.guild,
        actor.green_name,
        cc::npc_body(actor),
        scene.siege.war_state,
        scene.siege.war_guild,
        scene.pk,
        scene
            .player
            .map_or(0, |i| scene.world.actors.actors[i].kingdom),
        actor.kingdom,
        area,
    )
}

#[allow(clippy::too_many_arguments)]
fn war_admits_values(
    duel: bool,
    offensive: bool,
    tracked: bool,
    player: bool,
    guild: u16,
    green: bool,
    npc: bool,
    war_state: u32,
    war_guild: u16,
    pk: bool,
    kingdom: u8,
    target_kingdom: u8,
    area: bool,
) -> bool {
    if !duel || !offensive {
        return true;
    }

    if tracked {
        return false;
    }

    let same = (kingdom as i8) > 0 && kingdom == target_kingdom;

    if !pk {
        if player
            && (guild == 0 || guild != war_guild)
            && (war_state == 0 || (kingdom != 0 && kingdom == target_kingdom))
        {
            return false;
        }

        if green
            && (guild != war_guild || guild == 0)
            && (war_state == 0 || (same && !attack::CANNON_ATTACK_PERMISSION))
        {
            return false;
        }

        if same {
            if war_state != 0 || !player {
                return false;
            }
        } else if (war_state as i32) > 0 {
            if player && !attack::CANNON_ATTACK_PERMISSION {
                return false;
            }

            if area
                && kingdom == 3
                && (player || ((target_kingdom as i8) > 0 && target_kingdom != 4))
            {
                return false;
            }
        }
    }

    player || green || !npc
}

fn area_line_clear(field: &HeightField<'_>, from: (i32, i32), to: (i32, i32)) -> bool {
    if from == to || from.0 == 0 || from.1 == 0 || to.0 == 0 || to.1 == 0 {
        return true;
    }

    let dx = (from.0 - to.0).abs();

    let dz = (from.1 - to.1).abs();

    let x_major = dz < dx;

    let (major0, major1, minor0, minor1) = match x_major {
        true => (from.0, to.0, from.1, to.1),
        false => (from.1, to.1, from.0, to.0),
    };

    if major0 == major1 {
        return true;
    }

    let step = if major0 < major1 { 1 } else { -1 };

    let slope = ((minor1 - minor0) * 1_000) / (major1 - major0);

    let cell = |major: i32, minor: i32| match x_major {
        true => (major, minor),
        false => (minor, major),
    };

    let mut previous = field.at(from.0, from.1);

    let mut major = major0 + step;

    loop {
        let minor = (minor0 * 1_000 + slope * (major - major0)) / 1_000;

        let (x, z) = cell(major, minor);

        let (ox, oz) = field.origin();

        if x - ox < 0 || x - ox >= 0x100 || z - oz < 0 || z - oz >= 0x100 {
            return false;
        }

        let height = field.at(x, z);

        if height > previous + path::SIGHT_TOLERANCE || height < previous - path::SIGHT_TOLERANCE {
            return false;
        }

        previous = height;

        if major == major1 {
            break;
        }

        major += step;
    }

    true
}

fn cone_sweep(
    scene: &FieldScene,
    player: usize,
    at: (f32, f32, f32),
    skill: &Skill,
    caller: SweepCaller,
) -> Sweep {
    let me = &scene.world.actors.actors[player];

    let origin = sweep_origin(scene, player);

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let facing = (at.0 - me.position.x).atan2(at.2 - me.position.z);

    let range = i32::from(me.status.mastery[3] / 75).min(3) + 3;

    let mut hits = Vec::new();

    let mut to = origin;

    for (index, actor) in scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .rev()
        .take(1_001)
    {
        if index == player {
            continue;
        }

        let cell = attack::cell_of(actor);

        let mut distance = path::tile_distance(origin, cell);

        if midpoint(actor) {
            distance = (distance - MIDPOINT_DISCOUNT).max(0);
        }

        if distance > range {
            continue;
        }

        let bearing = (actor.position.x - me.position.x).atan2(actor.position.z - me.position.z);

        if (bearing - facing).abs() >= std::f32::consts::FRAC_PI_4 {
            continue;
        }

        if !midpoint(actor)
            && path::line_of_sight(&field, origin, cell, path::SIGHT_TOLERANCE).is_some()
        {
            continue;
        }

        if actor.is_dead {
            continue;
        }

        let admitted = match caller {
            SweepCaller::Cast => {
                let duel = attack::duel_here(scene.world.actors.attributes(), cell);

                if !duel && skill.offensive == 1 && actor.entity < PLAYER_ID_LIMIT {
                    false
                } else {
                    war_admits(scene, actor, skill.offensive == 1, false)
                }
            }
            SweepCaller::SkillAttack => scene.pk || actor.entity >= PLAYER_ID_LIMIT,
        };

        if !admitted {
            continue;
        }

        hits.push(Hit {
            entity: actor.entity,
            damage: wire::DAMAGE_NONE,
        });

        to = cell;

        if sweep_full(skill.target_shape, skill.max_targets, hits.len()) {
            break;
        }
    }

    Sweep {
        hits,
        to: (to.0 as u16, to.1 as u16),
    }
}

fn party_hits(scene: &FieldScene, player: usize, skill: &Skill, capacity: usize) -> Vec<Hit> {
    let me = &scene.world.actors.actors[player];

    if !me.tracked {
        return vec![Hit {
            entity: scene.entered.client_id,
            damage: wire::DAMAGE_NONE,
        }];
    }

    let Some(panels) = scene.panels.as_ref() else {
        return Vec::new();
    };

    if panels.screen(super::panels::CONTENT_PARTY).is_none() {
        return Vec::new();
    }

    let origin = sweep_origin(scene, player);

    let map = scene.world.actors.attributes();

    let player_duel = attack::duel_here(map, attack::cell_of(me));

    let bonus = i32::from(scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0);

    let range = skill.range + bonus;

    let mut candidates = Vec::with_capacity(capacity);

    for member in &scene.party.rows {
        let Some(index) = scene.world.actors.entity(member.id) else {
            continue;
        };

        let actor = &scene.world.actors.actors[index];

        let cell = attack::cell_of(actor);

        if !player_duel && attack::duel_here(map, cell) {
            continue;
        }

        let distance = path::tile_distance(origin, cell);

        if distance < range {
            candidates.push((member.id, distance));

            if candidates.len() == capacity {
                break;
            }
        }
    }

    for first in 0..candidates.len() {
        let mut nearest = first;

        for next in first + 1..candidates.len() {
            if candidates[next].1 < candidates[nearest].1 {
                nearest = next;
            }
        }

        candidates.swap(first, nearest);
    }

    candidates
        .into_iter()
        .map(|(entity, _)| Hit {
            entity,
            damage: wire::DAMAGE_NONE,
        })
        .collect()
}

fn target_centered(code: u8) -> bool {
    TARGET_CENTER_CODES.contains(&code)
}

fn range_checked(code: u8, caller: SweepCaller) -> bool {
    target_centered(code)
        || match caller {
            SweepCaller::Cast => code == 0x4F || code == CANNON_CODE,
            SweepCaller::SkillAttack => code == 0x5F,
        }
}

fn sweep_full(shape: i32, max_targets: i32, count: usize) -> bool {
    (shape == 3 && count > 7) || count as i32 >= max_targets || count >= wire::HITS
}

fn area_sweep(
    scene: &FieldScene,
    player: usize,
    selected: Option<usize>,
    at: (f32, f32, f32),
    code: u8,
    skill: &Skill,
    caller: SweepCaller,
    approach: bool,
    cannon: Option<Cannon>,
    now_ms: u32,
) -> Result<Sweep, AreaExit> {
    let radius =
        area_radius(skill.target_shape).expect("the target_shape fork admitted only 3, 4 or 6");

    let origin = sweep_origin(scene, player);

    let player_cell = attack::cell_of(&scene.world.actors.actors[player]);

    let mut center = (at.0.trunc() as i32, at.2.trunc() as i32);

    if center == (0, 0)
        && let Some(index) = selected
    {
        center = attack::cell_of(&scene.world.actors.actors[index]);
    }

    let selected_only = match caller {
        SweepCaller::Cast => CAST_SELECTED_ONLY,
        SweepCaller::SkillAttack => SKILL_ATTACK_SELECTED_ONLY,
    };

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let mut hits = Vec::new();

    let mut checked_center = false;

    for (index, actor) in scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .rev()
        .take(1_001)
    {
        if code == selected_only && Some(index) != selected {
            continue;
        }

        if actor.guild != 0
            && skill.offensive == 1
            && (scene.world.actors.actors[player].guild == actor.guild
                || scene.siege.ally == actor.guild)
        {
            continue;
        }

        if !checked_center {
            if target_centered(code) {
                let Some(target) = selected else {
                    return Err(match caller {
                        SweepCaller::Cast => Cast::Handled(crate::internal_reason!(
                            "NWYD-R0041",
                            "a target-centered area code has no body"
                        )),
                        SweepCaller::SkillAttack => Cast::Refused(crate::internal_reason!(
                            "NWYD-R0041",
                            "a target-centered area code has no body"
                        )),
                    }
                    .into());
                };

                center = attack::cell_of(&scene.world.actors.actors[target]);
            }

            if !range_checked(code, caller) {
                center = origin;
            } else {
                let mut distance = path::tile_distance(origin, center);

                if code == 0
                    && distance == 2
                    && (center.0 == player_cell.0 || center.1 == player_cell.1)
                {
                    distance = 1;
                }

                if selected.is_some_and(|target| midpoint(&scene.world.actors.actors[target])) {
                    distance = (distance - MIDPOINT_DISCOUNT).max(0);
                }

                let bonus = i32::from(scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0);

                let range = if skill.range == -1 {
                    attack::reach(scene)
                } else {
                    skill.range + bonus
                };

                if distance > range {
                    return Err(match caller {
                        SweepCaller::SkillAttack => Cast::Refused(crate::internal_reason!(
                            "NWYD-R0042",
                            "the area center is further away than range"
                        ))
                        .into(),
                        SweepCaller::Cast if [0x23, 0x27, CANNON_CODE].contains(&code) => {
                            Cast::Refused(crate::internal_reason!(
                                "NWYD-R0042",
                                "the area center is further away than range"
                            ))
                            .into()
                        }
                        SweepCaller::Cast => {
                            if path::line_of_sight(
                                &field,
                                player_cell,
                                center,
                                path::SIGHT_TOLERANCE,
                            )
                            .is_some()
                                || !approach
                            {
                                Cast::Refused(crate::internal_reason!(
                                    "NWYD-R0043",
                                    "the out-of-range area center cannot be approached"
                                ))
                                .into()
                            } else if attack::action_busy(
                                &scene.world.actors.actors[player],
                                &scene.world.actors.models,
                                now_ms,
                            ) {
                                Cast::Handled(crate::internal_reason!(
                                    "NWYD-R0044",
                                    "the approach waits for action 4..=9 to finish"
                                ))
                                .into()
                            } else {
                                let actor = selected.map(|index| &scene.world.actors.actors[index]);

                                let point = if at.1 < cc::WALK_FLOOR_Y {
                                    actor.map_or(at, |actor| {
                                        (actor.position.x, actor.position.y, actor.position.z)
                                    })
                                } else {
                                    at
                                };

                                let point = (point.0.trunc() as i32, point.2.trunc() as i32);

                                if point == player_cell {
                                    Cast::Handled(crate::internal_reason!(
                                        "NWYD-R0045",
                                        "the approach point is already the caster's cell"
                                    ))
                                    .into()
                                } else if let Some(actor) = actor {
                                    let target = attack::cell_of(actor);

                                    let plan = match approach_tile(origin, target, range) {
                                        Some(tile) => Approach::Order {
                                            entity: actor.entity,
                                            tile,
                                        },
                                        None => Approach::Handled {
                                            entity: actor.entity,
                                            what: "the approach target stays out of range after 501 steps",
                                        },
                                    };

                                    AreaExit::Approach(plan)
                                } else {
                                    AreaExit::Approach(Approach::Refused(crate::internal_reason!(
                                        "NWYD-R0046",
                                        "the approach has no target body"
                                    )))
                                }
                            }
                        }
                    });
                }

                if code == CANNON_CODE {
                    let cannon = cannon.expect("the early 0x61 object search admitted the cast");

                    if distance < 4 {
                        return Err(Cast::Handled(crate::internal_reason!(
                            "NWYD-R0047",
                            "the cannon center is fewer than four cells away"
                        ))
                        .into());
                    }

                    let yaw = scene
                        .drops
                        .get(cannon.entity)
                        .expect("the single cast cannot remove its retained cannon")
                        .yaw;

                    if !cannon_faces(yaw, player_cell, center) {
                        return Err(Cast::Handled(crate::internal_reason!(
                            "NWYD-R0048",
                            "the cannon does not face the area center"
                        ))
                        .into());
                    }
                }

                if selected.is_none_or(|target| !midpoint(&scene.world.actors.actors[target]))
                    && path::line_of_sight(&field, player_cell, center, path::SIGHT_TOLERANCE)
                        .is_some()
                {
                    return Err(match caller {
                        SweepCaller::Cast => Cast::Handled(crate::internal_reason!(
                            "NWYD-R0049",
                            "the area center is behind terrain"
                        )),
                        SweepCaller::SkillAttack => Cast::Refused(crate::internal_reason!(
                            "NWYD-R0049",
                            "the area center is behind terrain"
                        )),
                    }
                    .into());
                }
            }

            checked_center = true;
        }

        let cell = attack::cell_of(actor);

        if matches!(caller, SweepCaller::Cast)
            && scene.pk
            && skill.offensive == 1
            && attack::duel_here(scene.world.actors.attributes(), cell)
            && area_pk_protects_selected(scene, selected)
        {
            return Err(Cast::Handled(crate::internal_reason!(
                "NWYD-R0050",
                "PK area cast protects the selected kingdom in the castle blocks"
            ))
            .into());
        }

        if !war_admits(scene, actor, skill.offensive == 1, true) {
            continue;
        }

        if actor.entity >= PLAYER_ID_LIMIT
            && !actor.tracked
            && !actor.green_name
            && cc::npc_body(actor)
        {
            continue;
        }

        if actor.is_dead {
            continue;
        }

        let actor_duel = attack::duel_here(scene.world.actors.attributes(), cell);

        let player_duel = attack::duel_here(
            scene.world.actors.attributes(),
            attack::cell_of(&scene.world.actors.actors[player]),
        );

        if actor_duel && !player_duel && skill.offensive == 0 {
            continue;
        }

        if path::tile_distance(center, cell) > radius
            || !area_line_clear(&field, center, cell)
            || index == player
        {
            continue;
        }

        if attack::safe_here(scene.world.actors.attributes(), cell) && skill.offensive != 0 {
            continue;
        }

        if actor.entity < PLAYER_ID_LIMIT {
            if skill.offensive != 0 && !actor_duel {
                continue;
            }
        } else if (actor.tracked || actor.green_name) && !actor_duel {
            continue;
        }

        if skill.range == -1 && hits.is_empty() {
            hits.push(Hit {
                entity: actor.entity,
                damage: wire::DAMAGE_UNKNOWN,
            });
        }

        hits.push(Hit {
            entity: actor.entity,
            damage: wire::DAMAGE_NONE,
        });

        if Some(index) == selected && !hits.is_empty() {
            let last = hits.len() - 1;

            hits.swap(0, last);
        }

        if sweep_full(skill.target_shape, skill.max_targets, hits.len()) {
            break;
        }
    }

    Ok(Sweep {
        hits,
        to: (center.0 as u16, center.1 as u16),
    })
}

fn preamble(scene: &FieldScene, player: usize) -> Result<(u8, usize), Cast> {
    let me = &scene.world.actors.actors[player];

    if hidden(me) {
        return Err(Cast::Refused(crate::internal_reason!(
            "NWYD-R0051",
            "the player is GM-hidden"
        )));
    }

    if attack::melee_block(me) != 0 {
        return Err(Cast::Refused(crate::internal_reason!(
            "NWYD-R0052",
            "an affect has the caster's melee-block flag set"
        )));
    }

    let blocked = scene
        .panels
        .as_ref()
        .is_some_and(|screens| screens.is_open(CONTENT_CAST_BLOCKER));

    if blocked {
        return Err(Cast::Refused(crate::internal_reason!(
            "NWYD-R0053",
            "the portal panel is up"
        )));
    }

    if rooted(me) {
        return Err(Cast::Refused(crate::internal_reason!(
            "NWYD-R0054",
            "an affect has the caster's casting-block flag set"
        )));
    }

    let Some((code, row)) = armed(scene) else {
        return Err(Cast::Refused(crate::internal_reason!(
            "NWYD-R0055",
            "the selected quick slot is outside the bar"
        )));
    };

    Ok((code, row))
}

fn targeted_neighbour(scene: &FieldScene, player: usize, target: usize) -> Option<u16> {
    let me = attack::cell_of(&scene.world.actors.actors[player]);

    let there = attack::cell_of(&scene.world.actors.actors[target]);

    let cell = (
        there.0 + (there.0 - me.0).signum(),
        there.1 + (there.1 - me.1).signum(),
    );

    scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, actor)| {
            if index == player || index == target || attack::cell_of(actor) != cell {
                return None;
            }

            if actor.entity < PLAYER_ID_LIMIT {
                return (scene.pk
                    && !actor.tracked
                    && attack::duel_here(scene.world.actors.attributes(), attack::cell_of(actor)))
                .then_some(actor.entity);
            }

            if !scene.pk
                && actor.green_name
                && (actor.guild == 0 || actor.guild != scene.siege.war_guild)
            {
                return None;
            }

            Some(actor.entity)
        })
}

#[allow(clippy::too_many_arguments)]
fn targeted(
    scene: &mut FieldScene,
    player: usize,
    mut target: Option<usize>,
    explicit: bool,
    at: (f32, f32, f32),
    approach: bool,
    code: u8,
    row: usize,
    skill: &Skill,
    now_ms: u32,
) -> Cast {
    if skill.target_shape == 2 && target.is_none() {
        scene.world.actors.actors[player].highlight = true;

        target = Some(player);
    }

    let Some(index) = target else {
        scene.hovered = None;

        return talk(
            scene,
            now_ms,
            crate::internal_reason!("NWYD-R0181", "the targeted arm has no body"),
        );
    };

    if !explicit && !scene.world.actors.actors[index].highlight {
        scene.hovered = None;

        return talk(
            scene,
            now_ms,
            crate::internal_reason!("NWYD-R0182", "the target is no longer hovered"),
        );
    }

    if matches!(scene.world.actors.actors[player].action, 2 | 3) {
        scene.hovered = None;

        return talk(
            scene,
            now_ms,
            crate::internal_reason!("NWYD-R0183", "the caster is in action 2 or 3"),
        );
    }

    if hidden(&scene.world.actors.actors[player]) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0056",
            "the player is GM-hidden"
        ));
    }

    if owns_store(scene) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0057",
            "the targeted arm sees the local personal-shop owner latch"
        ));
    }

    if mana_cost(scene, skill, code) > i32::try_from(scene.entered.status.mp).unwrap_or(i32::MAX) {
        mana_toast(scene);

        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0058",
            "not enough mana (strdef 30), and the targeted arm answers 1"
        ));
    }

    let map = scene.world.actors.attributes();

    let me = attack::cell_of(&scene.world.actors.actors[player]);

    let there = attack::cell_of(&scene.world.actors.actors[index]);

    let offensive = skill.offensive == 1;

    let harmless = skill.offensive == 0;

    let is_player = scene.world.actors.actors[index].entity < PLAYER_ID_LIMIT;

    let target_duel = attack::duel_here(map, there);

    let player_duel = attack::duel_here(map, me);

    let target_safe = attack::safe_here(map, there);

    let player_safe = attack::safe_here(map, me);

    if target_duel && !player_duel {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0059",
            "the target is on a duel cell and the caster is not"
        ));
    }

    if offensive && (player_safe || target_safe) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0060",
            "a offensive skill cannot cross a safe cell"
        ));
    }

    if offensive
        && (is_player || scene.world.actors.actors[index].tracked)
        && !(player_duel && target_duel)
    {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0061",
            "an offensive player/tracked target needs both duel cells"
        ));
    }

    if offensive
        && !target_duel
        && (scene.world.actors.actors[index].green_name || scene.world.actors.actors[index].tracked)
    {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0062",
            "an offensive cast will not take a green or tracked target here"
        ));
    }

    let local = &scene.world.actors.actors[player];

    let target = &scene.world.actors.actors[index];

    if foreign_green_guild(scene, local, target) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0063",
            "the green target is protected by the single-target war rule"
        ));
    }

    if foreign_duel_player(scene, local, target, offensive) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0064",
            "the player target is outside the single-target war guild"
        ));
    }

    if offensive && kingdom_three_protects(scene, local, target) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0065",
            "kingdom 3 protects this single-target body during war"
        ));
    }

    if harmless && !is_player && !target.tracked && !target.green_name {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0066",
            "a harmless ordinary cast will not take an untracked non-green mob"
        ));
    }

    if offensive && allied_target(scene, local, target) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0067",
            "an offensive target is in your guild or its ally"
        ));
    }

    if guildless_green(scene, local, target) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0068",
            "the guildless green target is protected by the war state"
        ));
    }

    if harmless && target_duel && !player_duel {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0069",
            "a harmless skill cannot enter a duel cell from outside"
        ));
    }

    if !is_player
        && !scene.world.actors.actors[index].tracked
        && !scene.world.actors.actors[index].green_name
        && cc::npc_body(&scene.world.actors.actors[index])
    {
        return talk(
            scene,
            now_ms,
            crate::internal_reason!("NWYD-R0184", "the target is an NPC body"),
        );
    }

    if !scene.world.actors.actors[index].shop_title.is_empty() {
        return talk(scene, now_ms, "the target is running a personal shop");
    }

    if now_ms.wrapping_sub(scene.attack.last_attack_ms) <= attack::ATTACK_INTERVAL_MS {
        return talk(
            scene,
            now_ms,
            "the targeted arm's shared one-second floor has not run out",
        );
    }

    if grade_f_protected(
        &scene.world.actors.actors[player],
        &scene.world.actors.actors[index],
        offensive,
    ) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0070",
            "the kingdom half of the targeted grade-0xF arm"
        ));
    }

    let here = sweep_origin(scene, player);

    let midpoint = midpoint(&scene.world.actors.actors[index]);

    let mut distance = path::tile_distance(here, there);

    if midpoint {
        distance = (distance - MIDPOINT_DISCOUNT).max(0);
    }

    let bonus = i32::from(scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0);

    let mut range = skill.range + bonus;

    if range == -1 {
        range = 1;
    }

    if weapon_kind(scene) < RANGE_WEAPON_KIND {
        range = range.max(attack::reach(scene));
    }

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let obstructed =
        !midpoint && path::line_of_sight(&field, here, there, path::SIGHT_TOLERANCE).is_some();

    if distance > range || obstructed {
        if !approach {
            return talk(
                scene,
                now_ms,
                "the target is out of range or behind terrain",
            );
        }

        if attack::action_busy(
            &scene.world.actors.actors[player],
            &scene.world.actors.models,
            now_ms,
        ) {
            return Cast::Handled(crate::internal_reason!(
                "NWYD-R0071",
                "the targeted approach waits for action 4..=9 to finish"
            ));
        }

        let actor = &scene.world.actors.actors[index];

        let point = if at.1 < cc::WALK_FLOOR_Y {
            there
        } else {
            (at.0.trunc() as i32, at.2.trunc() as i32)
        };

        if point == me {
            return Cast::Handled(crate::internal_reason!(
                "NWYD-R0072",
                "the targeted approach point is already the caster's cell"
            ));
        }

        let Some(tile) = approach_tile(here, there, range) else {
            return Cast::Handled(crate::internal_reason!(
                "NWYD-R0073",
                "the targeted body stays out of range after 501 steps"
            ));
        };

        return apply_approach(
            scene,
            Approach::Order {
                entity: actor.entity,
                tile,
            },
            now_ms,
        );
    }

    let entity = scene.world.actors.actors[index].entity;

    let mut hits = Vec::with_capacity(3);

    if skill.range == -1 {
        hits.push(Hit {
            entity,
            damage: wire::DAMAGE_UNKNOWN,
        });
    }

    hits.push(Hit {
        entity,
        damage: wire::DAMAGE_NONE,
    });

    if [0x02, 0x0C, 0x1C].contains(&code)
        && let Some(entity) = targeted_neighbour(scene, player, index)
    {
        hits.push(Hit {
            entity,
            damage: wire::DAMAGE_NONE,
        });
    }

    let sent = send(scene, player, code, row, skill, hits, None, now_ms);

    scene.attack.approach_target = None;

    sent
}

pub fn cast(
    scene: &mut FieldScene,
    at: (f32, f32, f32),
    now_ms: u32,
    approach: bool,
    target: Option<u16>,
) -> Cast {
    if scene.world.force_load.is_some() {
        return Cast::Refused(crate::internal_reason!("NWYD-R0074", "destination_loading"));
    }

    let Some(player) = scene.player else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));
    };

    if alt_held(scene) {
        return Cast::Refused(crate::internal_reason!("NWYD-R0075", "ALT is held"));
    }

    let clears_pickup = !hidden(&scene.world.actors.actors[player])
        && !scene
            .panels
            .as_ref()
            .is_some_and(|screens| screens.is_open(CONTENT_CAST_BLOCKER));

    if clears_pickup {
        scene.pending_pickup = None;
    }

    let (code, row) = match preamble(scene, player) {
        Ok(armed) => armed,
        Err(answer) => return answer,
    };

    let explicit_target = target.is_some();

    let target = target
        .or(scene.hovered)
        .and_then(|id| scene.world.actors.entity(id));

    if let Some(index) = target
        && attack::invisible_body(&scene.world.actors.actors[index])
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0006",
            "it is an invisible class-0x42 body and you cannot detect invisibility"
        ));
    }

    scene.attack.auto_target = None;

    if let Some(index) = target
        && code == ATTRACT_CODE
        && scene.world.actors.actors[index].class == attack::INVISIBLE_CLASS
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0076",
            "0x06 Attract does not take a class-0x42 body"
        ));
    }

    if !super::enter::learned(&scene.entered, i32::try_from(row).unwrap_or(i32::MAX)) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0077",
            "this character has not learned that skill"
        ));
    }

    if let Some((_, kind)) = WEAPON_GATES.iter().find(|(gate, _)| *gate == code)
        && weapon_kind(scene) != *kind
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0078",
            "the armed skill needs a different weapon kind (effect 0x15)"
        ));
    }

    if REFUSED_CODES.contains(&code) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0079",
            "0x63 Resurrect and 0x53 Craft_Armor_Abillty are refused by number"
        ));
    }

    if code == HANDIWORK_CODE {
        let up = scene
            .panels
            .as_ref()
            .is_some_and(|s| s.is_open(CONTENT_HANDIWORK));

        if !up && let Some(panels) = scene.panels.as_mut() {
            panels.open_mix((0, 0, 0), false);
        }

        return match up {
            true => Cast::Handled(crate::internal_reason!(
                "NWYD-R0080",
                "0x54 handiwork, and content 0x19 was already up"
            )),
            false => Cast::Handled(crate::internal_reason!(
                "NWYD-R0081",
                "handiwork opens the item mix panel"
            )),
        };
    }

    let cannon = if code == CANNON_CODE {
        let Some(cannon) = cannon_under(scene, player) else {
            return talk(
                scene,
                now_ms,
                "0x61 Conjure_Cannon found no cannon under the caster's cell",
            );
        };

        Some(cannon)
    } else {
        None
    };

    if row >= usize::from(TABLE_BOUND) {
        return talk(
            scene,
            now_ms,
            crate::internal_reason!("NWYD-R0185", "the folded row is past the 248-record table"),
        );
    }

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|table| table.get(row))
        .copied()
    else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0082",
            "SkillData.bin did not load, so no row can be read"
        ));
    };

    if owns_store(scene) {
        return talk(scene, now_ms, "the local player's personal shop is open");
    }

    if skill.is_passive() {
        return talk(
            scene,
            now_ms,
            crate::internal_reason!("NWYD-R0186", "the armed skill is a passive (passive == 1)"),
        );
    }

    if let Some(why) = admissible(scene, target.map(|i| &scene.world.actors.actors[i]), &skill) {
        return talk(scene, now_ms, why);
    }

    let cooldown = cooldown_s(
        &skill,
        cooldown_refined(scene),
        cooldown_affected(&scene.world.actors.actors[player]),
    );

    let cooldown_ms = cooldown.max(0) as u32 * 1_000;

    if now_ms.wrapping_sub(scene.cc.skill_stamps[row]) < cooldown_ms {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0083",
            "that skill's own cooldown has not run out"
        ));
    }

    if forbidden(attack::cell_of(&scene.world.actors.actors[player])) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0084",
            "the caster is inside the nine-by-nine no-cast square"
        ));
    }

    if now_ms.wrapping_sub(scene.attack.last_attack_ms) <= attack::ATTACK_INTERVAL_MS {
        return targeted(
            scene,
            player,
            target,
            explicit_target,
            at,
            approach,
            code,
            row,
            &skill,
            now_ms,
        );
    }

    if !SELF_SHAPES.contains(&skill.target_shape) {
        return targeted(
            scene,
            player,
            target,
            explicit_target,
            at,
            approach,
            code,
            row,
            &skill,
            now_ms,
        );
    }

    if mana_cost(scene, &skill, code) > i32::try_from(scene.entered.status.mp).unwrap_or(i32::MAX) {
        mana_toast(scene);

        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0085",
            "not enough mana (strdef 30, and this arm answers 0)"
        ));
    }

    if code == COIN_ARMOR_CODE {
        let price = COIN_ARMOR_STACKS.saturating_mul(COIN_ARMOR_PRICE);

        if price > i32::try_from(scene.entered.gold).unwrap_or(i32::MAX) {
            mana_toast(scene);

            return Cast::Refused(crate::internal_reason!(
                "NWYD-R0086",
                "0x55 Coin_Armor costs coins, and there are not enough"
            ));
        }
    }

    let map = scene.world.actors.attributes();

    if attack::safe_here(map, attack::cell_of(&scene.world.actors.actors[player]))
        && skill.offensive == 1
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0087",
            "a offensive skill cannot be cast from a safe cell"
        ));
    }

    let mut hits: Vec<Hit> = Vec::new();

    let mut destination = None;

    if skill.target_shape == SELF_SHAPES[0] {
        if code == PARTY_SUMMON_CODE {
            let record = if scene.world.actors.actors[player].tracked {
                31
            } else {
                32
            };

            super::message_panel::post(scene, record, 1000, now_ms);

            return Cast::Handled(crate::internal_reason!(
                "NWYD-R0088",
                "0x2A Party_Summon prints strdef 31 or 32 and sends nothing"
            ));
        }

        if code == BLINK_CODE {
            return blink(scene, at, now_ms, row, &skill);
        }

        if matches!(code, 0x1D | 0x2C) {
            hits = party_hits(scene, player, &skill, wire::HITS);
        } else {
            hits.push(Hit {
                entity: scene.entered.client_id,
                damage: wire::DAMAGE_NONE,
            });

            self_stamps(scene, code, row, now_ms);
        }
    } else if skill.target_shape == 5 {
        let sweep = cone_sweep(scene, player, at, &skill, SweepCaller::Cast);

        hits = sweep.hits;

        destination = Some(sweep.to);
    } else if area_radius(skill.target_shape).is_some() {
        let sweep = match area_sweep(
            scene,
            player,
            target,
            at,
            code,
            &skill,
            SweepCaller::Cast,
            approach,
            cannon,
            now_ms,
        ) {
            Ok(sweep) => sweep,
            Err(AreaExit::Answer(answer)) => return answer,
            Err(AreaExit::Approach(plan)) => return apply_approach(scene, plan, now_ms),
        };

        hits = sweep.hits;

        destination = Some(sweep.to);
    }

    if code == POINT_CODE {
        let sweep = match build_ground_cast_target(scene, player, at, &skill) {
            Ok(sweep) => sweep,
            Err(answer) => return answer,
        };

        hits = sweep.hits;

        destination = Some(sweep.to);
    }

    if code == SOUL_CODE
        && !SOUL_PREREQUISITES
            .iter()
            .any(|&c| learned_unclassed(scene, c))
    {
        super::message_panel::post(scene, SOUL_STRDEF, 2000, now_ms);

        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0089",
            "0x66 Soul_Of_Limits needs 0x1F, 0x27 or 0x2F (strdef 357)"
        ));
    }

    if !send_gate(code, &hits) {
        if code == COIN_ARMOR_CODE {
            return targeted(
                scene,
                player,
                target,
                explicit_target,
                at,
                approach,
                code,
                row,
                &skill,
                now_ms,
            );
        }

        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0090",
            "the send gate found no target and the code is not 0x61/0x23/0x33"
        ));
    }

    let first = send(scene, player, code, row, &skill, hits, destination, now_ms);

    if code != COIN_ARMOR_CODE {
        return first;
    }

    let Cast::Sent(first) = first else {
        unreachable!("the common sender always reports its frame")
    };

    let continuation = targeted(
        scene,
        player,
        target,
        explicit_target,
        at,
        approach,
        code,
        row,
        &skill,
        now_ms,
    );

    Cast::Continued {
        first,
        continuation: Box::new(continuation),
    }
}

fn self_stamps(scene: &mut FieldScene, code: u8, row: usize, now_ms: u32) {
    let folded = u8::try_from(row).unwrap_or(0);

    if SELF_STAMP_MOVER_CODES.contains(&folded) {
        scene.cc.skill_stamps[row] = now_ms;

        scene.cc.last_order_ms = now_ms;

        return;
    }

    let _ = code;

    if SELF_STAMP_CODES.contains(&folded) {
        scene.cc.skill_stamps[row] = now_ms;
    }
}

fn learned_unclassed(scene: &FieldScene, code: i32) -> bool {
    let bit = code.rem_euclid(super::enter::SKILLS_PER_CLASS);

    scene.entered.skill_mask[0] & (1u32 << bit) != 0
}

fn blink(
    scene: &mut FieldScene,
    at: (f32, f32, f32),
    now_ms: u32,
    row: usize,
    skill: &Skill,
) -> Cast {
    if skill.cost > i32::try_from(scene.entered.status.mp).unwrap_or(i32::MAX) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0091",
            "0x49 iIlusion does not have its raw SkillData mana price"
        ));
    }

    if now_ms.wrapping_sub(scene.cc.last_order_ms) <= BLINK_THROTTLE_MS {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0092",
            "0x49 iIlusion is inside its own one-second throttle"
        ));
    }

    if at.1 == BLINK_MISS_Y {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0093",
            "0x49 iIlusion was aimed at a point the ground pick never resolved"
        ));
    }

    let Some(player) = scene.player else {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0094",
            "0x49 iIlusion has no local body to start its path probe"
        ));
    };

    let from = attack::cell_of(&scene.world.actors.actors[player]);

    let requested = (at.0.trunc() as i32, at.2.trunc() as i32);

    let probe = {
        let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

        path::walk(
            &field,
            from,
            requested,
            BLINK_STEP_BUDGET,
            path::HEIGHT_TOLERANCE,
        )
    };

    if probe.dirs.is_empty() {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0095",
            "0x49 iIlusion's eight-step path probe did not move"
        ));
    }

    let tile = (probe.reached.0 as u16, probe.reached.1 as u16);

    let action = formats::net::action::Action {
        x: from.0 as i16,
        z: from.1 as i16,
        kind: BLINK_ACTION_KIND,
        speed: scene.wire_speed,
        path: [0; formats::net::action::PATH_LEN],
        dest_x: tile.0,
        dest_z: tile.1,
    };

    scene.attack.from = Some(tile);

    scene.outbox.push(protocol::client::act(&action));

    advance_auto_skill(scene);

    scene.cc.skill_stamps[BLINK_CODE as usize] = now_ms;

    scene.cc.last_order_ms = now_ms;

    Cast::Blinked {
        code: BLINK_CODE,
        row,
        tile,
    }
}

fn talk(scene: &mut FieldScene, now_ms: u32, why: &'static str) -> Cast {
    if armed(scene).is_some_and(|(code, _)| code == TALK_SPECIAL_CURSOR) {
        return Cast::Talked(crate::internal_reason!(
            "NWYD-R0187",
            "the armed slot holds 0x1F, which cannot be cast"
        ));
    }

    let Some(entity) = scene.hovered_drop else {
        return Cast::Talked(why);
    };

    let Some((item, hovered)) = scene
        .drops
        .get(entity)
        .map(|drop| (drop.item, drop.hovered))
        .or_else(|| {
            scene
                .gates
                .get(entity)
                .map(|gate| (gate.item, gate.hovered))
        })
    else {
        return Cast::Talked(crate::internal_reason!(
            "NWYD-R0188",
            "the selected object is no longer in the world"
        ));
    };

    if now_ms.wrapping_sub(scene.cc.last_action_ms) <= 1_000 {
        return Cast::Talked(crate::internal_reason!(
            "NWYD-R0189",
            "the shared one-second floor has not run out"
        ));
    }

    if !hovered {
        return Cast::Talked(crate::internal_reason!(
            "NWYD-R0190",
            "the claimed object's hover flag is clear"
        ));
    }

    let item_id = item.index as i16;

    if TALK_EXCLUDED_A.contains(&item_id) || TALK_EXCLUDED_B.contains(&item_id) {
        return Cast::ObjectAction {
            why: crate::internal_reason!(
                "NWYD-R0193",
                "an excluded object id reaches the handled epilogue"
            ),
            entity,
            opcode: None,
        };
    }

    let kind = scene
        .statics
        .effects()
        .total_scaled(&item, super::drops::GATE_EFFECT);

    if kind < 1 {
        if super::item_use::free_carry_slot(&scene.entered, 0, true) < 0 {
            if let Some(panels) = scene.panels.as_mut() {
                super::shop::bag_full(panels);
            }

            scene.cc.last_action_ms = now_ms;

            return Cast::ObjectAction {
                why: crate::internal_reason!(
                    "NWYD-R0194",
                    "the ordinary drop found no free carried-item cell"
                ),
                entity,
                opcode: None,
            };
        }

        let picked = pickup(scene, entity, now_ms);

        return Cast::ObjectAction {
            why: picked.reason(),
            entity,
            opcode: picked.opcode(),
        };
    }

    if kind == 10 && TALK_LEGACY_PORTALS.contains(&item_id) {
        if item_id == TALK_REEL_A || item_id == TALK_REEL_B {
            super::field::open_reel(scene, item_id);

            return Cast::ObjectAction {
                why: crate::internal_reason!(
                    "NWYD-R0195",
                    "the kind-10 object switches the legacy reel UI instead of sending"
                ),
                entity,
                opcode: None,
            };
        }
    }

    scene.outbox.push(protocol::client::open_gate(entity));

    scene.cc.last_action_ms = now_ms;

    Cast::ObjectAction {
        why: crate::internal_reason!("NWYD-R0196", "the picked gate asks the server to open"),
        entity,
        opcode: Some(protocol::client::OPEN_GATE),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pickup {
    Pending(&'static str),
    Walking {
        tile: (i32, i32),
        started: bool,
        sent: bool,
    },
    Sent {
        slot: i32,
    },
}

impl Pickup {
    fn reason(self) -> &'static str {
        match self {
            Pickup::Pending(why) => why,
            Pickup::Walking { .. } => "the ordinary drop was retained while the player walks to it",
            Pickup::Sent { .. } => "the ordinary drop was requested from the server",
        }
    }

    fn opcode(self) -> Option<u16> {
        match self {
            Pickup::Walking { sent: true, .. } => Some(protocol::client::MOVE),
            Pickup::Sent { .. } => Some(formats::net::ground_item::PICK_UP),
            _ => None,
        }
    }

    fn describe(self, entity: u16) -> Option<String> {
        match self {
            Pickup::Pending(_) => None,
            Pickup::Walking {
                tile,
                started,
                sent,
            } => Some(
                format!(
                    "NWYD: retained ground item {entity} and approached {},{} (walking={started})",
                    tile.0, tile.1,
                ) + if sent { " with 0x36C" } else { "" },
            ),
            Pickup::Sent { slot } => Some(format!(
                "NWYD: requested ground item {entity} into carry slot {slot}"
            )),
        }
    }
}

pub fn retry_pickup(scene: &mut FieldScene, now_ms: u32) -> Option<String> {
    let entity = scene.pending_pickup?;

    pickup(scene, entity, now_ms).describe(entity)
}

fn pickup(scene: &mut FieldScene, entity: u16, now_ms: u32) -> Pickup {
    let Some(drop) = scene.drops.get(entity).cloned() else {
        scene.pending_pickup = None;

        return Pickup::Pending("the retained ordinary drop no longer exists");
    };

    let Some(player) = scene.player else {
        return Pickup::Pending("the field has no local player for the pickup");
    };

    if usize::from(entity) < scene.init_items.len() {
        return Pickup::Pending("the object id is inside the InitItem range");
    }

    scene.pending_pickup = Some(entity);

    if now_ms.wrapping_sub(scene.cc.last_action_ms) <= 1_000 {
        return Pickup::Pending("the shared pickup floor has not run out");
    }

    let actor = &scene.world.actors.actors[player];

    let current = attack::cell_of(actor);

    let source = scene
        .attack
        .from
        .map(|(x, z)| (i32::from(x), i32::from(z)))
        .unwrap_or(current);

    let target = (drop.position.x as i32, drop.position.z as i32);

    let distance = path::tile_distance(source, target);

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let clear = path::line_of_sight(&field, source, target, path::SIGHT_TOLERANCE).is_none();

    let id = drop.item.index as i16;

    let level_allows = id != 0x6BF || (scene.entered.status.level as i16) > 999;

    let id_allows = level_allows && id != 0x167 && !TALK_EXCLUDED_A.contains(&id);

    if id_allows && distance < 2 && clear && current == target && scene.sent_dest == Some(current) {
        let slot = super::item_use::free_carry_slot(&scene.entered, 0, true);

        let money = scene
            .statics
            .effects()
            .total_scaled(&drop.item, formats::data::item_effect::effect::USE_KIND)
            == super::store::MONEY_USE_KIND;

        if (slot >= 0 || money) && id != 0x305 && id != 0x2EA {
            scene.outbox.push(protocol::client::pick_up_item(
                formats::net::ground_item::PickUp {
                    kind: 1,
                    slot,
                    entity,
                    x: target.0 as u16,
                    z: u32::from(target.1 as u16),
                },
            ));

            scene.pending_pickup = None;

            scene.attack.last_attack_ms = now_ms;

            scene.cc.last_action_ms = now_ms;

            return Pickup::Sent { slot };
        }
    }

    let action = scene.world.actors.actors[player].action;

    let shift = scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.shift);

    if id_allows && matches!(action, 0 | 1) && !shift {
        let before = scene.outbox.len();

        scene.cc.auto_move = target;

        let started = super::field::order(scene, target);

        return Pickup::Walking {
            tile: target,
            started,
            sent: scene.outbox.len() > before,
        };
    }

    Pickup::Pending("the retained ordinary drop could neither send nor approach")
}

pub fn skill_attack(scene: &mut FieldScene, target: u16, at: (f32, f32, f32), now_ms: u32) -> Cast {
    let Some(player) = scene.player else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));
    };

    let Some(index) = scene.world.actors.entity(target) else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0096",
            "no body in this world carries that id"
        ));
    };

    if hidden(&scene.world.actors.actors[player]) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0051",
            "the player is GM-hidden"
        ));
    }

    if alt_held(scene) {
        return Cast::Refused(crate::internal_reason!("NWYD-R0075", "ALT is held"));
    }

    if cc::npc_body(&scene.world.actors.actors[index]) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0097",
            "its grade nibble or its body item makes it an NPC"
        ));
    }

    if scene.world.actors.actors[index].green_name {
        return Cast::Refused(crate::internal_reason!("NWYD-R0098", "it is green-named"));
    }

    if scene.world.actors.actors[index].is_dead {
        return Cast::Refused(crate::internal_reason!("NWYD-R0099", "it is dead or dying"));
    }

    let map = scene.world.actors.attributes();

    if attack::safe_here(map, attack::cell_of(&scene.world.actors.actors[player])) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0013",
            "you are standing in a safe zone"
        ));
    }

    if attack::melee_block(&scene.world.actors.actors[player]) != 0 {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0052",
            "an affect has the caster's melee-block flag set"
        ));
    }

    if scene
        .panels
        .as_ref()
        .is_some_and(|screens| screens.is_open(CONTENT_CAST_BLOCKER))
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0053",
            "the portal panel is up"
        ));
    }

    let body = &scene.world.actors.actors[index];

    if body.class == cc::REFUSED_CLASS && body.body == cc::REFUSED_BODY {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0100",
            "it is a class-0x2C body wearing item 0xDB"
        ));
    }

    if rooted(&scene.world.actors.actors[player]) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0054",
            "an affect has the caster's casting-block flag set"
        ));
    }

    scene.pending_pickup = None;

    if attack::invisible_body(&scene.world.actors.actors[index]) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0006",
            "it is an invisible class-0x42 body and you cannot detect invisibility"
        ));
    }

    let Some((code, row)) = armed(scene) else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0055",
            "the selected quick slot is outside the bar"
        ));
    };

    let is_player = scene.world.actors.actors[index].entity < PLAYER_ID_LIMIT;

    if code == KNIFE_CODE && is_player {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0101",
            "0x56 Throwing_Knife will not take a player (id < 1000)"
        ));
    }

    if code == ATTRACT_CODE && scene.world.actors.actors[index].class == attack::INVISIBLE_CLASS {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0076",
            "0x06 Attract does not take a class-0x42 body"
        ));
    }

    if is_player && !SKILL_ATTACK_PLAYER_CODES.contains(&code) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0102",
            "only 0x1B, 0x1D and 0x2C may be pointed at a player"
        ));
    }

    if !super::enter::learned(&scene.entered, i32::from(code)) {
        advance_auto_skill(scene);

        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0077",
            "this character has not learned that skill"
        ));
    }

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|table| table.get(row))
        .copied()
    else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0082",
            "SkillData.bin did not load, so no row can be read"
        ));
    };

    if now_ms.wrapping_sub(scene.attack.last_attack_ms) < attack::ATTACK_INTERVAL_MS {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0103",
            "the shared one-second floor has not run out"
        ));
    }

    let cooldown = cooldown_s(
        &skill,
        cooldown_refined(scene),
        cooldown_affected(&scene.world.actors.actors[player]),
    );

    let cooldown_ms = cooldown.max(0) as u32 * 1_000;

    if now_ms.wrapping_sub(scene.cc.skill_stamps[row]) < cooldown_ms {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0083",
            "that skill's own cooldown has not run out"
        ));
    }

    if forbidden(attack::cell_of(&scene.world.actors.actors[player])) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0084",
            "the caster is inside the nine-by-nine no-cast square"
        ));
    }

    if code == WEAPON_GATES[0].0 && weapon_kind(scene) != WEAPON_GATES[0].1 {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0104",
            "0x4F Rapid_Hit needs weapon kind 0x65 (effect 0x15)"
        ));
    }

    if !SELF_SHAPES.contains(&skill.target_shape) {
        return single_target(scene, player, index, code, row, &skill, now_ms);
    }

    if mana_cost(scene, &skill, code) > i32::try_from(scene.entered.status.mp).unwrap_or(i32::MAX) {
        mana_toast(scene);

        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0105",
            "not enough mana (strdef 30)"
        ));
    }

    let sweep = if skill.target_shape == 5 {
        Some(cone_sweep(
            scene,
            player,
            at,
            &skill,
            SweepCaller::SkillAttack,
        ))
    } else if area_radius(skill.target_shape).is_some() {
        match area_sweep(
            scene,
            player,
            Some(index),
            at,
            code,
            &skill,
            SweepCaller::SkillAttack,
            false,
            None,
            now_ms,
        ) {
            Ok(sweep) => Some(sweep),
            Err(AreaExit::Answer(answer)) => return answer,
            Err(AreaExit::Approach(_)) => {
                unreachable!(
                    "{}",
                    crate::internal_reason!("NWYD-E1011", "the skill cannot approach its target")
                )
            }
        }
    } else {
        None
    };

    let (hits, destination) =
        sweep.map_or_else(|| (Vec::new(), None), |sweep| (sweep.hits, Some(sweep.to)));

    if !send_gate(code, &hits) {
        if code == 0x1D {
            let hits = party_hits(scene, player, &skill, wire::HITS - 1);

            return send_without_advance(scene, player, code, row, &skill, hits, None, now_ms);
        }

        if code == MODE2_1B_CODE {
            return cast_single_target_at_origin(scene, player, index, &skill, now_ms);
        }

        if code == COIN_ARMOR_CODE {
            return single_target(scene, player, index, code, row, &skill, now_ms);
        }

        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0106",
            "the send gate found no target and the code has no arm of its own"
        ));
    }

    send(scene, player, code, row, &skill, hits, destination, now_ms)
}

fn cast_single_target_at_origin(
    scene: &mut FieldScene,
    player: usize,
    target: usize,
    skill: &Skill,
    now_ms: u32,
) -> Cast {
    let origin = sweep_origin(scene, player);

    let target_cell = attack::cell_of(&scene.world.actors.actors[target]);

    let reach_bonus = i32::from(scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0);

    let range = skill.range.wrapping_add(reach_bonus);

    let changed_by_los = {
        let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

        path::line_of_sight(&field, origin, target_cell, path::SIGHT_TOLERANCE).is_some()
    };

    if path::tile_distance(origin, target_cell) > range {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0107",
            "mode-2 0x1B's target is beyond range plus the reach bit"
        ));
    }

    if changed_by_los {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0108",
            "mode-2 0x1B's line-of-sight probe rewrote the target cell"
        ));
    }

    let map = scene.world.actors.attributes();

    if !attack::duel_here(map, origin) && attack::duel_here(map, target_cell) {
        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0109",
            "mode-2 0x1B cannot enter a duel cell from outside"
        ));
    }

    send_single_target_at_origin(scene, target, origin, now_ms)
}

fn single_target(
    scene: &mut FieldScene,
    player: usize,
    index: usize,
    code: u8,
    row: usize,
    skill: &Skill,
    now_ms: u32,
) -> Cast {
    if owns_store(scene) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0110",
            "the local player's personal shop is open"
        ));
    }

    let map = scene.world.actors.attributes();

    let me = attack::cell_of(&scene.world.actors.actors[player]);

    let there = attack::cell_of(&scene.world.actors.actors[index]);

    let is_player = scene.world.actors.actors[index].entity < PLAYER_ID_LIMIT;

    let offensive = skill.offensive == 1;

    let harmless = skill.offensive == 0;

    if mana_cost(scene, skill, code) > i32::try_from(scene.entered.status.mp).unwrap_or(i32::MAX) {
        mana_toast(scene);

        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0105",
            "not enough mana (strdef 30)"
        ));
    }

    if offensive && attack::safe_here(map, there) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0014",
            "it is standing in a safe zone"
        ));
    }

    if offensive
        && (is_player || scene.world.actors.actors[index].tracked)
        && !(attack::duel_here(map, there) && attack::duel_here(map, me))
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0111",
            "players may only fight on a duel cell"
        ));
    }

    if harmless && attack::duel_here(map, there) && !attack::duel_here(map, me) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0112",
            "it is on a duel cell and you are not"
        ));
    }

    let local = &scene.world.actors.actors[player];

    let target = &scene.world.actors.actors[index];

    if offensive && !attack::duel_here(map, there) && (target.green_name || target.tracked) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0113",
            "an offensive mode-2 cast will not take a green or tracked target here"
        ));
    }

    if foreign_duel_player(scene, local, target, offensive) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0064",
            "the player target is outside the single-target war guild"
        ));
    }

    if foreign_green_guild(scene, local, target) || guildless_green(scene, local, target) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0063",
            "the green target is protected by the single-target war rule"
        ));
    }

    if offensive && kingdom_three_protects(scene, local, target) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0065",
            "kingdom 3 protects this single-target body during war"
        ));
    }

    if harmless && !is_player && !target.tracked {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0114",
            "a offensive-of-0 skill will not take a body at or above 1,000"
        ));
    }

    if offensive && allied_target(scene, local, target) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0067",
            "an offensive target is in your guild or its ally"
        ));
    }

    if !scene.world.actors.actors[index].shop_title.is_empty() {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0003",
            "it is running a personal shop"
        ));
    }

    if grade_f_protected(local, target, offensive) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0115",
            "the kingdom half of the grade-0xF arm"
        ));
    }

    let here = match scene.attack.from {
        Some((x, z)) => (i32::from(x), i32::from(z)),
        None => me,
    };

    let mut distance = path::tile_distance(here, there);

    let target = &scene.world.actors.actors[index];

    if target.class == attack::MIDPOINT_CLASS && target.parts[0].mesh == MIDPOINT_PART0_MESH {
        distance = (distance - MIDPOINT_DISCOUNT).max(0);
    }

    let bonus = i32::from(scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0);

    let mut range = skill.range + bonus;

    if range == -1 {
        range = 1;
    }

    if weapon_kind(scene) < RANGE_WEAPON_KIND {
        range = range.max(attack::reach(scene));
    }

    if distance > range {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0116",
            "it is further away than range and the weapon reach together"
        ));
    }

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    if !midpoint(target)
        && path::line_of_sight(&field, here, there, path::SIGHT_TOLERANCE).is_some()
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0117",
            "something is standing between you and it"
        ));
    }

    let entity = scene.world.actors.actors[index].entity;

    let hits = vec![Hit {
        entity,
        damage: wire::DAMAGE_NONE,
    }];

    let sent = send(scene, player, code, row, skill, hits, None, now_ms);

    scene.attack.approach_target = None;

    sent
}

fn send(
    scene: &mut FieldScene,
    player: usize,
    code: u8,
    row: usize,
    skill: &Skill,
    hits: Vec<Hit>,
    destination: Option<(u16, u16)>,
    now_ms: u32,
) -> Cast {
    send_inner(
        scene,
        player,
        code,
        row,
        skill,
        hits,
        destination,
        now_ms,
        true,
    )
}

fn send_without_advance(
    scene: &mut FieldScene,
    player: usize,
    code: u8,
    row: usize,
    skill: &Skill,
    hits: Vec<Hit>,
    destination: Option<(u16, u16)>,
    now_ms: u32,
) -> Cast {
    send_inner(
        scene,
        player,
        code,
        row,
        skill,
        hits,
        destination,
        now_ms,
        false,
    )
}

fn send_single_target_at_origin(
    scene: &mut FieldScene,
    target: usize,
    origin: (i32, i32),
    now_ms: u32,
) -> Cast {
    let opcode = wire::ATTACK_MANY;

    let origin = (origin.0 as u16, origin.1 as u16);

    let mut attack = Attack {
        from: origin,
        to: origin,
        attacker: scene.entered.client_id,
        action: wire::ACTION_NONE,
        damage: wire::DAMAGE_NONE,
        skill_id: i16::from(MODE2_1B_CODE),
        used: wire::hits_for(opcode),
        ..Attack::default()
    };

    attack.hits[0] = Hit {
        entity: scene.world.actors.actors[target].entity,
        damage: wire::DAMAGE_NONE,
    };

    let mut echo = attack;

    echo.echo = true;

    if scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0 {
        echo.flags |= wire::FLAG_TRAIL;
    }

    attack::apply(scene, opcode, &echo, now_ms);

    scene.outbox.push(attack.to_bytes(opcode));

    scene.attack.last_attack_ms = now_ms;

    scene.cc.skill_stamps[STAMP_0X1B] = now_ms;

    Cast::Sent(Sent {
        opcode,
        code: MODE2_1B_CODE,
        row: STAMP_0X1B,
        targets: 1,
    })
}

#[allow(clippy::too_many_arguments)]
fn send_inner(
    scene: &mut FieldScene,
    player: usize,
    code: u8,
    row: usize,
    skill: &Skill,
    hits: Vec<Hit>,
    destination: Option<(u16, u16)>,
    now_ms: u32,
    auto_advance: bool,
) -> Cast {
    let opcode = opcode_for(skill);

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    let from = scene.attack.from.unwrap_or((here.0 as u16, here.1 as u16));

    let mut attack = Attack {
        from,
        to: destination.unwrap_or(from),
        attacker: scene.entered.client_id,
        skill: 0,
        action: wire::ACTION_NONE,
        echo: false,
        flags: 0,
        variant: 0,
        damage: wire::DAMAGE_NONE,
        skill_id: i16::try_from(row).unwrap_or(wire::SKILL_NONE),
        used: wire::hits_for(opcode),
        ..Attack::default()
    };

    let keep = usize::try_from(skill.max_targets)
        .unwrap_or(0)
        .min(attack.hits.len());

    let targets = hits.len().min(keep);

    for (slot, hit) in hits.iter().take(targets).enumerate() {
        attack.hits[slot] = *hit;
    }

    let mut echo = attack;

    echo.echo = true;

    if scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0 {
        echo.flags |= wire::FLAG_TRAIL;
    }

    attack::apply(scene, opcode, &echo, now_ms);

    scene.outbox.push(attack.to_bytes(opcode));

    if auto_advance {
        advance_auto_skill(scene);
    }

    scene.attack.last_attack_ms = now_ms;

    scene.cc.skill_stamps[row] = now_ms;

    Cast::Sent(Sent {
        opcode,
        code,
        row,
        targets,
    })
}

pub const PANEL_STAMP_FLOOR_MS: u32 = 1_000;

pub const PANEL_CODE_BIAS: u8 = 0x78;

pub fn panel_code(id: i32) -> Option<u8> {
    if id == -1 || id == 0 {
        return None;
    }

    let biased = (id as u8).wrapping_add(PANEL_CODE_BIAS);

    (biased < 0x80).then_some(biased)
}

pub fn panel_cast(scene: &mut FieldScene, code: u8, now_ms: u32) -> Cast {
    let Some(player) = scene.player else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));
    };

    let row = usize::from(code);

    if !super::enter::learned(&scene.entered, i32::from(code)) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0077",
            "this character has not learned that skill"
        ));
    }

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|table| table.get(row))
        .copied()
    else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0082",
            "SkillData.bin did not load, so no row can be read"
        ));
    };

    if !skill.is_selectable() {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0118",
            "target_shape is neither 0 nor 2, so the panel does not offer this skill"
        ));
    }

    if skill.offensive == 1 {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0119",
            "offensive is 1 — this panel will not cast an offensive skill"
        ));
    }

    if !(skill.c11 > 0 || skill.c09 > 0) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0120",
            "both c11 and c09 are zero, which this panel refuses outright"
        ));
    }

    if skill.is_passive() {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0121",
            "the skill is a passive (passive == 1)"
        ));
    }

    let cooldown = cooldown_s(
        &skill,
        cooldown_refined(scene),
        cooldown_affected(&scene.world.actors.actors[player]),
    );

    let cooldown_ms = cooldown.max(0) as u32 * 1_000;

    let since = now_ms.wrapping_sub(scene.cc.skill_stamps[row]);

    if since < cooldown_ms {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0083",
            "that skill's own cooldown has not run out"
        ));
    }

    if since < PANEL_STAMP_FLOOR_MS {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0122",
            "this cell was clicked less than a second ago"
        ));
    }

    if now_ms.wrapping_sub(scene.attack.last_attack_ms) < attack::ATTACK_INTERVAL_MS {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0103",
            "the shared one-second floor has not run out"
        ));
    }

    if mana_cost(scene, &skill, code) > i32::try_from(scene.entered.status.mp).unwrap_or(i32::MAX) {
        mana_toast(scene);

        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0123",
            "not enough mana (strdef 30), and nothing is sent"
        ));
    }

    panel_send(scene, player, code, row, &skill, now_ms)
}

pub fn party_cast(scene: &mut FieldScene, member: usize, now_ms: u32) -> Cast {
    if scene
        .panels
        .as_ref()
        .and_then(|p| p.screen(super::panels::CONTENT_PARTY))
        .is_none()
    {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0124",
            "party controller is absent"
        ));
    }

    let Some(player) = scene
        .player
        .filter(|&i| scene.world.actors.actors.get(i).is_some())
    else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0125",
            "party cast has no live player"
        ));
    };

    let Some(target) = scene.party.rows.get(member).map(|m| m.id) else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0126",
            "party cast has no retained row"
        ));
    };

    let Some(code) = wire_enter::selected(&scene.entered.quick, scene.entered.quick_selected)
        .filter(|&code| code < 128)
    else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0127",
            "invalid signed party skill code"
        ));
    };

    let row = usize::from(code);

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|t| t.get(row))
        .copied()
    else {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0128",
            "party cast has no SkillData row"
        ));
    };

    let cooldown = cooldown_s(
        &skill,
        cooldown_refined(scene),
        cooldown_affected(&scene.world.actors.actors[player]),
    );

    if now_ms < scene.cc.skill_stamps[row].wrapping_add((cooldown as u32).wrapping_mul(1000)) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0129",
            "party skill cooldown has not elapsed"
        ));
    }

    if mana_cost(scene, &skill, code) > scene.entered.status.mp as i32 {
        mana_toast(scene);

        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0130",
            "party cast mana refusal"
        ));
    }

    let ready = now_ms > scene.attack.last_attack_ms.wrapping_add(1000);

    if ready && code == 42 && target == scene.world.actors.actors[player].entity {
        super::message_panel::post(scene, 40, 1000, now_ms);

        return Cast::Handled(crate::internal_reason!(
            "NWYD-R0131",
            "Party Summon cannot target the caster"
        ));
    }

    if !ready {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0132",
            "party cast shared stamp requires strictly more than one second"
        ));
    }

    if !matches!(code, 13 | 25 | 27 | 42 | 43 | 44 | 45) {
        return Cast::Refused(crate::internal_reason!(
            "NWYD-R0133",
            "no party-target branch for this skill"
        ));
    }

    let bonus = scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0;

    let origin = scene.attack.from.filter(|&(x, _)| x != 0).map_or_else(
        || attack::cell_of(&scene.world.actors.actors[player]),
        |(x, z)| (i32::from(x), i32::from(z)),
    );

    if code != 42 {
        let Some(index) = scene.world.actors.entity(target) else {
            return Cast::Refused(crate::internal_reason!(
                "NWYD-R0134",
                "party target body is absent"
            ));
        };

        let actor = &scene.world.actors.actors[index];

        if !actor.tracked {
            return Cast::Refused(crate::internal_reason!(
                "NWYD-R0135",
                "party target is not tracked"
            ));
        }

        let destination = attack::cell_of(actor);

        if path::tile_distance(origin, destination) > skill.range.wrapping_add(i32::from(bonus)) {
            return Cast::Refused(crate::internal_reason!(
                "NWYD-R0136",
                "party target exceeds range plus reach bonus"
            ));
        }

        let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

        if path::line_of_sight(&field, origin, destination, path::SIGHT_TOLERANCE).is_some() {
            return Cast::Refused(crate::internal_reason!(
                "NWYD-R0137",
                "party target line of sight changed the destination"
            ));
        }

        let map = scene.world.actors.attributes();

        if !attack::duel_here(map, origin) && attack::duel_here(map, destination) {
            return Cast::Refused(crate::internal_reason!(
                "NWYD-R0138",
                "party cast cannot enter a duel cell from outside"
            ));
        }
    }

    let client = scene.world.actors.actors[player].entity;

    let cell = (origin.0 as u16, origin.1 as u16);

    let mut packet = Attack {
        from: cell,
        to: cell,
        attacker: client,
        action: wire::ACTION_NONE,
        damage: wire::DAMAGE_NONE,
        skill_id: i16::from(code),
        used: wire::hits_for(wire::ATTACK_MANY),
        ..Default::default()
    };

    packet.hits[0] = Hit {
        entity: target,
        damage: wire::DAMAGE_NONE,
    };

    let mut frame = packet.to_bytes(wire::ATTACK_MANY);

    frame[6..8].copy_from_slice(&client.to_le_bytes());

    scene.outbox.push(frame);

    packet.echo = true;

    if bonus {
        packet.flags |= wire::FLAG_TRAIL;
    }

    attack::apply(scene, wire::ATTACK_MANY, &packet, now_ms);

    scene.attack.last_attack_ms = now_ms;

    scene.cc.skill_stamps[row] = now_ms;

    Cast::Sent(Sent {
        opcode: wire::ATTACK_MANY,
        code,
        row,
        targets: 1,
    })
}

pub(crate) fn mana_toast(scene: &mut FieldScene) {
    scene.world.play_sound(MANA_SOUND);

    if let Some(panels) = scene.panels.as_mut()
        && let Some(text) = panels
            .strings
            .strdef
            .as_ref()
            .map(|table| table.get(MANA_STRDEF).to_string())
    {
        super::chat::system(panels, &text, MANA_TOAST_COLOR);
    }
}

pub const MANA_TOAST_COLOR: u32 = 0xFFFF_AAAA;

pub const MANA_SOUND: i64 = 0x21;

fn panel_send(
    scene: &mut FieldScene,
    player: usize,
    code: u8,
    row: usize,
    skill: &Skill,
    now_ms: u32,
) -> Cast {
    let opcode = opcode_for(skill);

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    let cell = (here.0 as u16, here.1 as u16);

    let mut attack = Attack {
        from: cell,
        to: cell,
        attacker: scene.entered.client_id,
        skill: 0,
        action: wire::ACTION_NONE,
        echo: false,
        flags: 0,
        variant: 0,
        damage: wire::DAMAGE_NONE,
        skill_id: i16::from(code),
        used: wire::hits_for(opcode),
        ..Attack::default()
    };

    attack.hits[0] = Hit {
        entity: scene.entered.client_id,
        damage: wire::DAMAGE_NONE,
    };

    let mut echo = attack;

    echo.echo = true;

    scene.outbox.push(attack.to_bytes(opcode));

    attack::apply(scene, opcode, &echo, now_ms);

    scene.attack.last_attack_ms = now_ms;

    scene.attack.auto_target = None;

    scene.cc.skill_stamps[row] = now_ms;

    Cast::Sent(Sent {
        opcode,
        code,
        row,
        targets: 1,
    })
}

pub fn panel_casts(scene: &mut FieldScene, now_ms: u32) -> Vec<Cast> {
    let pending: Vec<u8> = match scene.panels.as_mut() {
        Some(panels) => std::mem::take(&mut panels.skill_casts),
        None => return Vec::new(),
    };

    pending
        .into_iter()
        .map(|code| panel_cast(scene, code, now_ms))
        .collect()
}

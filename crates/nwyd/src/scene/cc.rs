use super::actors::Actor;
use super::attack;
use super::cast;
use super::consume;
use super::field::{self, FieldScene};
use super::hud::MOVE_PLANS_A_PATH;
use super::nameplate::PLAYER_ID_LIMIT;
use super::panels;
use formats::data::item_effect::effect;
use formats::data::mount;
use formats::net::account;
use formats::net::affect;
use formats::net::attack::{self as wire, Attack, Hit};
use formats::net::enter::QUICK_PAGE_SLOTS;
use formats::world::path::{self, HeightField};

pub const MODAL_FREEZE_MS: u32 = 1_000;

pub const SKILL_STAMPS: usize = 248;

pub const STAMP_0X66: usize = 0x66;

pub const COOLDOWN_0X66_S: u32 = 0x4B0;

pub const COOLDOWN_0X66_EVOLVED_S: u32 = 0x3E8;

pub const COOLDOWN_0X38_MS: u32 = 0x1_3880;

pub const MP_GATE_DEFAULT: i32 = 1;

pub const MP_GATE_OFF: i32 = 3;

pub const ANCHOR_CLEARED: (f32, f32) = (0.0, 0.0);

pub const PAGE_SLOTS: usize = QUICK_PAGE_SLOTS;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Queued {
    pub at: (f32, f32, f32),
    pub approach: bool,
    pub target: Option<u16>,
}

#[derive(Debug, Clone, PartialEq)]

pub struct State {
    pub target: Option<u16>,
    pub anchor: (f32, f32),
    pub queued: Option<Queued>,
    pub freeze_ms: u32,
    pub mp_gate: i32,
    pub skill_stamps: [u32; SKILL_STAMPS],
    pub auto_move: (i32, i32),
    pub last_action_ms: u32,
    pub last_order_ms: u32,
    pub warp_hold_ms: u32,
    pub cycle: usize,
}

impl Default for State {
    fn default() -> State {
        State {
            target: None,
            anchor: ANCHOR_CLEARED,
            queued: None,
            freeze_ms: 0,
            mp_gate: MP_GATE_DEFAULT,
            skill_stamps: [0; SKILL_STAMPS],
            auto_move: (0, 0),
            last_action_ms: 0,
            last_order_ms: 0,
            warp_hold_ms: 0,
            cycle: 0,
        }
    }
}

pub const RETALIATION_REACH: i32 = 3;

pub fn raw_reach(scene: &FieldScene) -> i32 {
    let bonus = scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0;

    attack::reach(scene) - i32::from(bonus)
}

pub const REFUSED_CLASS: i32 = 0x2C;

pub const REFUSED_BODY: i32 = 0xDB;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Grade {
    Refused(&'static str),
    Distance(i32),
}

impl Grade {
    pub fn retaliation(self) -> Option<i32> {
        match self {
            Grade::Distance(distance) if distance > 0 => Some(distance),
            _ => None,
        }
    }

    pub fn scan(self) -> Option<i32> {
        match self {
            Grade::Distance(distance) => Some(distance),
            Grade::Refused(_) => None,
        }
    }
}

pub fn npc_body(target: &Actor) -> bool {
    let grade = u8::try_from(target.status.flags & 0xF).unwrap_or(0);

    !attack::ATTACKABLE_GRADES.contains(&grade) || attack::npc_bodies().contains(&target.body)
}

pub fn grade(scene: &FieldScene, player: usize, index: usize, now_ms: u32) -> Grade {
    grade_with_sight(scene, player, index, now_ms, true)
}

fn grade_with_sight(
    scene: &FieldScene,
    player: usize,
    index: usize,
    now_ms: u32,
    check_sight: bool,
) -> Grade {
    let target = &scene.world.actors.actors[index];

    let me = &scene.world.actors.actors[player];

    if target.class == REFUSED_CLASS && target.body == REFUSED_BODY {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0100",
            "it is a class-0x2C body wearing item 0xDB"
        ));
    }

    if me.kingdom != 0 && target.kingdom == me.kingdom {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0139",
            "it is in your kingdom"
        ));
    }

    if target.is_dead {
        return Grade::Refused(crate::internal_reason!("NWYD-R0099", "it is dead or dying"));
    }

    if target.entity < PLAYER_ID_LIMIT && target.status.flags & 1 != 0 {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0140",
            "the target is GM-hidden"
        ));
    }

    if target.entity < PLAYER_ID_LIMIT {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0141",
            "it is a player, and the macro never targets one"
        ));
    }

    if npc_body(target) {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0097",
            "its grade nibble or its body item makes it an NPC"
        ));
    }

    if target.green_name {
        return Grade::Refused(crate::internal_reason!("NWYD-R0098", "it is green-named"));
    }

    let mine = u8::try_from(me.status.flags & 0xF).unwrap_or(0);

    let local_kingdom = scene
        .player
        .map(|local| scene.world.actors.actors[local].kingdom)
        .unwrap_or(0);

    if mine == 0xF
        && (me.kingdom as i8) > 0
        && (local_kingdom as i8) > 0
        && me.kingdom == local_kingdom
    {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0142",
            "your own grade nibble is 0xF and you have a kingdom"
        ));
    }

    if now_ms
        < scene
            .attack
            .last_attack_ms
            .wrapping_add(attack::ATTACK_INTERVAL_MS)
    {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0143",
            "the shared one-second swing floor has not run out"
        ));
    }

    let here = attack::cell_of(me);

    let there = attack::cell_of(target);

    let distance = path::tile_distance(here, there);

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let ray = path::line_of_sight(&field, here, there, path::SIGHT_TOLERANCE).unwrap_or(there);

    if check_sight && (ray.0 == 0 || ray.1 == 0) {
        return Grade::Refused(crate::internal_reason!(
            "NWYD-R0144",
            "the ray gave up before it arrived"
        ));
    }

    tracing::debug!(
        target : "nwyd::mob_evidence", event = "macro_candidate", entity = target.entity,
        body = target.body, green_name = target.green_name, defense = target
        .status.defense, distance,
        "macro candidate admitted under current green-name policy"
    );

    Grade::Distance(distance)
}

pub fn scan_origin(scene: &FieldScene) -> Option<(i32, i32)> {
    if scene.combat.movement == MOVE_PLANS_A_PATH {
        return Some((scene.cc.anchor.0 as i32, scene.cc.anchor.1 as i32));
    }

    let player = scene.player?;

    Some(attack::cell_of(&scene.world.actors.actors[player]))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Acquired {
    Nothing,
    Stuck {
        entity: u16,
    },
    Retaliated {
        entity: u16,
        slot: usize,
        distance: i32,
    },
    Scanned {
        entity: u16,
        distance: i32,
    },
}

impl Acquired {
    pub fn entity(self) -> Option<u16> {
        match self {
            Acquired::Nothing => None,
            Acquired::Stuck { entity }
            | Acquired::Retaliated { entity, .. }
            | Acquired::Scanned { entity, .. } => Some(entity),
        }
    }
}

pub fn acquire(scene: &FieldScene, mut range: i32, now_ms: u32) -> Acquired {
    if let Some(entity) = scene.attack.stuck {
        return Acquired::Stuck { entity };
    }

    let Some(player) = scene.player else {
        return Acquired::Nothing;
    };

    if raw_reach(scene) > RETALIATION_REACH {
        for (slot, entry) in scene.attack.retaliation.iter().enumerate() {
            let Some(attacker) = entry.fresh(now_ms) else {
                continue;
            };

            let Some(index) = scene.world.actors.entity(attacker) else {
                continue;
            };

            let Some(distance) = grade(scene, player, index, now_ms).retaliation() else {
                continue;
            };

            return Acquired::Retaliated {
                entity: attacker,
                slot,
                distance,
            };
        }
    }

    let Some(origin) = scan_origin(scene) else {
        return Acquired::Nothing;
    };

    let mut best: Option<(u16, i32)> = None;

    for (index, actor) in scene.world.actors.actors.iter().enumerate().skip(1).rev() {
        if index == player {
            continue;
        }

        if path::tile_distance(origin, attack::cell_of(actor)) > range {
            continue;
        }

        let Some(distance) = grade(scene, player, index, now_ms).scan() else {
            continue;
        };

        let better = match best {
            Some((_, key)) => distance < key,
            None => true,
        };

        if better {
            best = Some((actor.entity, distance));

            range = distance;
        }
    }

    match best {
        Some((entity, distance)) => Acquired::Scanned { entity, distance },
        None => Acquired::Nothing,
    }
}

pub const WALK_INTERVAL_MS: u32 = 1_000;

pub const ORDER_INTERVAL_MS: u32 = 0xC8;

pub const WALK_FLOOR_Y: f32 = -5_000.0;

pub const SPEED_GATE: f64 = 0.9;

pub const WALK_CLICK: i32 = 0;

pub const WALK_MACRO: i32 = 1;

pub const WALK_PANEL: crate::frame::PanelId = panels::CONTENT_CARGO;

pub const FOCUSED: bool = true;

pub const CC_MODE_PHYSICAL: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Walked {
    Refused(&'static str),
    Inert,
    Ordered { tile: (i32, i32), walking: bool },
}

impl Walked {
    pub fn answer(&self) -> bool {
        !matches!(self, Walked::Refused(_))
    }

    pub fn describe(&self) -> Option<String> {
        match self {
            Walked::Refused(_) => None,
            Walked::Inert => Some("NWYD: the auto-move answered 1 with no zone loaded".to_string()),
            Walked::Ordered { tile, walking } => Some(format!(
                "NWYD: auto-move to {tile:?}{}",
                match walking {
                    true => "",
                    false => " (the mover would not start)",
                },
            )),
        }
    }
}

pub fn ground_y(scene: &FieldScene, cell: (i32, i32)) -> f32 {
    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    field.ground(cell.0, cell.1) + super::field::MARKER_LIFT
}

pub fn away_from_home(scene: &FieldScene) -> Option<i32> {
    let player = scene.player?;

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    Some(path::tile_distance(
        here,
        (scene.cc.anchor.0 as i32, scene.cc.anchor.1 as i32),
    ))
}

pub fn walk(scene: &mut FieldScene, dest: (f32, f32, f32), now_ms: u32, flag: i32) -> Walked {
    let Some(player) = scene.player else {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));
    };

    if scene.world.actors.actors[player].mover.server_driven {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0145",
            "the local body is in a server-driven move"
        ));
    }

    if now_ms < scene.cc.last_action_ms.wrapping_add(WALK_INTERVAL_MS) {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0146",
            "the one-second input floor has not run out"
        ));
    }

    match flag {
        WALK_CLICK => {
            let up = scene
                .panels
                .as_ref()
                .is_some_and(|screens| screens.is_open(WALK_PANEL));

            if up {
                return Walked::Refused(crate::internal_reason!(
                    "NWYD-R0147",
                    "the warehouse panel is up"
                ));
            }
        }
        WALK_MACRO if attack::chase_block(&scene.world.actors.actors[player]) != 0 => {
            return Walked::Refused(crate::internal_reason!(
                "NWYD-R0148",
                "an affect has the player's chase-block flag set"
            ));
        }
        WALK_MACRO => {}
        _ => {}
    }

    let focused = FOCUSED;

    if !focused && scene.combat.mode == super::hud::CC_MODE_DEFAULT {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0149",
            "the window is not focused and the CC mode is off"
        ));
    }

    if now_ms < scene.cc.last_order_ms.wrapping_add(ORDER_INTERVAL_MS) {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0150",
            "the 200 ms mover floor has not run out"
        ));
    }

    if scene
        .panels
        .as_ref()
        .and_then(|panels| panels.store.as_ref())
        .is_some_and(|contents| contents.owner == scene.entered.client_id)
    {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0110",
            "the local player's personal shop is open"
        ));
    }

    if (scene.attack.stuck.is_some() || scene.attack.approach_target.is_some())
        && now_ms
            < scene
                .attack
                .last_pursue_ms
                .wrapping_add(attack::PURSUE_INTERVAL_MS)
    {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0151",
            "a refused walk is latched and the chase second is still running"
        ));
    }

    if attack::action_busy(
        &scene.world.actors.actors[player],
        &scene.world.actors.models,
        now_ms,
    ) {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0152",
            "action 4..=9 has not reached its clipped end"
        ));
    }

    if dest.1 < WALK_FLOOR_Y {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0153",
            "the destination is under the -5000 floor"
        ));
    }

    let speed = super::enter::move_speed(&scene.entered.status);

    scene.world.actors.actors[player].speed = speed;

    scene.wire_speed = speed as i32;

    let tile = (dest.0 as i32, dest.2 as i32);

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    if tile == here {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0154",
            "the body is already standing on that cell"
        ));
    }

    scene.cc.auto_move = tile;

    let move_alpha = scene.world.actors.actors[player].mover.move_alpha;

    let walking = if f64::from(move_alpha) >= SPEED_GATE || move_alpha == 0.0 {
        field::order(scene, tile)
    } else {
        false
    };

    clear_walk_targets(scene);

    Walked::Ordered { tile, walking }
}

pub(super) fn clear_walk_targets(scene: &mut FieldScene) {
    scene.attack.auto_target = None;

    scene.attack.stuck = None;

    scene.attack.approach_target = None;

    scene.hovered = None;

    if scene.combat.movement == MOVE_PLANS_A_PATH && scene.combat.mode == CC_MODE_PHYSICAL {
        scene.cc.target = None;
    }
}

pub fn walk_home(scene: &mut FieldScene, now_ms: u32) -> Walked {
    let Some(_player) = scene.player else {
        return Walked::Refused(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));
    };

    let (ax, az) = scene.cc.anchor;

    let y = ground_y(scene, (ax as i32, az as i32));

    scene.sent_dest = Some((0, 0));

    walk(scene, (ax, y, az), now_ms, WALK_MACRO)
}

pub const MARCH_STEPS: i32 = 0x1F4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Marched {
    pub cell: (i32, i32),
    pub steps: i32,
    pub capped: bool,
}

pub fn march(scene: &FieldScene, there: (i32, i32), range: i32, distance: i32) -> Option<Marched> {
    let player = scene.player?;

    if distance - 1 <= range {
        return None;
    }

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    let mut cell = match scene.attack.from {
        Some((x, z)) => (i32::from(x), i32::from(z)),
        None => here,
    };

    let step_x = match cell.0 > there.0 {
        true => -1,
        false => 1,
    };

    let step_z = match cell.1 > there.1 {
        true => -1,
        false => 1,
    };

    let mut steps = 0;

    let mut capped = false;

    while path::tile_distance(cell, there) - 1 > range {
        if steps > MARCH_STEPS {
            capped = true;

            break;
        }

        if cell.0 != there.0 {
            cell.0 += step_x;
        }

        if cell.1 != there.1 {
            cell.1 += step_z;
        }

        steps += 1;
    }

    Some(Marched {
        cell,
        steps,
        capped,
    })
}

pub const SWING_HOLD_MS: u32 = 2_000;

pub const SWING_CAP: i32 = 0xF;

pub const SWING_TRAIL: bool = false;

pub const CHASE_STEPS_GUARD: i32 = 0x1F4;

pub const CHASE_PLAN_BUDGET: usize = path::STEP_BUDGET;

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Swung {
    Refused(&'static str),
    Swung {
        entity: u16,
        opcode: u16,
        action: u8,
    },
    Chased {
        entity: u16,
        tile: (i32, i32),
        walking: bool,
    },
}

impl Swung {
    pub fn code(&self) -> i32 {
        match self {
            Swung::Refused(_) => 0,
            Swung::Swung { .. } => 1,
            Swung::Chased { .. } => 2,
        }
    }

    pub fn keeps_the_target(&self) -> bool {
        self.code() >= 1
    }

    pub fn describe(&self) -> Option<String> {
        match self {
            Swung::Refused(_) => None,
            Swung::Swung {
                entity,
                opcode,
                action,
            } => Some(format!(
                "NWYD: the macro sent {opcode:#05X} at entity {entity}, action {action}"
            )),
            Swung::Chased {
                entity,
                tile,
                walking,
            } => Some(format!(
                "NWYD: the macro is chasing entity {entity} to {tile:?}{}",
                match walking {
                    true => "",
                    false => " (the order was refused)",
                },
            )),
        }
    }
}

pub fn swing(scene: &mut FieldScene, entity: u16, chase: bool, now_ms: u32) -> Swung {
    let Some(player) = scene.player else {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));
    };

    if scene.cc.warp_hold_ms != 0 {
        if now_ms.wrapping_sub(scene.cc.warp_hold_ms) > SWING_HOLD_MS {
            scene.cc.warp_hold_ms = 0;
        }

        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0155",
            "the two-second teleport hold is still running"
        ));
    }

    let Some(index) = scene.world.actors.entity(entity) else {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0156",
            "no body in this world carries that id"
        ));
    };

    let distance = match grade_with_sight(scene, player, index, now_ms, false) {
        Grade::Refused(why) => return Swung::Refused(why),
        Grade::Distance(distance) => distance,
    };

    scene.cc.cycle = (scene.cc.cycle + 1) % wire::ATTACK_ACTIONS.len();

    if distance > SWING_CAP {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0157",
            "it is past the hard fifteen-cell cap"
        ));
    }

    let reach = attack::reach(scene);

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    let there = attack::cell_of(&scene.world.actors.actors[index]);

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let ray = path::line_of_sight(&field, here, there, path::SIGHT_TOLERANCE).unwrap_or(there);

    if ray.0 == 0 || ray.1 == 0 {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0144",
            "the ray gave up before it arrived"
        ));
    }

    let arrived = scene.sent_dest == Some(here);

    if distance > reach || ray != there || !arrived {
        return chase_step(scene, entity, player, here, there, reach, chase);
    }

    if attack::melee_block(&scene.world.actors.actors[player]) == 1 {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0009",
            "an affect has the player's melee-block flag set"
        ));
    }

    if attack::invisible_body(&scene.world.actors.actors[index]) {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0006",
            "it is an invisible class-0x42 body and you cannot detect invisibility"
        ));
    }

    send(scene, entity, index, player, here, there, now_ms)
}

fn chase_step(
    scene: &mut FieldScene,
    entity: u16,
    player: usize,
    here: (i32, i32),
    there: (i32, i32),
    reach: i32,
    chase: bool,
) -> Swung {
    if !chase {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0158",
            "the movement mode forbids a chase"
        ));
    }

    if attack::chase_block(&scene.world.actors.actors[player]) == 1 {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0148",
            "an affect has the player's chase-block flag set"
        ));
    }

    if scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.shift) {
        return Swung::Refused(crate::internal_reason!(
            "NWYD-R0159",
            "held SHIFT suppresses the chase (GetAsyncKeyState(VK_SHIFT))"
        ));
    }

    let mut cell = here;

    let step_x = match cell.0 > there.0 {
        true => -1,
        false => 1,
    };

    let step_z = match cell.1 > there.1 {
        true => -1,
        false => 1,
    };

    let mut steps = 0;

    while path::tile_distance(cell, there) > reach && steps <= CHASE_STEPS_GUARD {
        if cell.0 != there.0 {
            cell.0 += step_x;
        }

        if cell.1 != there.1 {
            cell.1 += step_z;
        }

        steps += 1;
    }

    if scene.player == Some(player) {
        if scene.combat.movement == MOVE_PLANS_A_PATH {
            let anchor = (scene.cc.anchor.0 as i32, scene.cc.anchor.1 as i32);

            let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

            let planned = path::walk(
                &field,
                anchor,
                cell,
                CHASE_PLAN_BUDGET,
                path::HEIGHT_TOLERANCE,
            );

            if planned.reached != cell {
                return Swung::Refused(crate::internal_reason!(
                    "NWYD-R0160",
                    "the planner cannot reach the chase cell from the anchor"
                ));
            }
        }

        let marker_cell = (cell.0, cell.0);

        let at = glam::Vec3::new(cell.0 as f32, ground_y(scene, marker_cell), cell.0 as f32);

        scene.marker = Some(super::field::ClickMarker {
            at,
            tile: cell,
            walking: false,
        });
    }

    scene.cc.auto_move = cell;

    let walking = field::order(scene, cell);

    if let Some(marker) = scene.marker.as_mut()
        && marker.tile == cell
    {
        marker.walking = walking;
    }

    Swung::Chased {
        entity,
        tile: cell,
        walking,
    }
}

fn send(
    scene: &mut FieldScene,
    entity: u16,
    index: usize,
    player: usize,
    here: (i32, i32),
    there: (i32, i32),
    now_ms: u32,
) -> Swung {
    let action = wire::ATTACK_ACTIONS[scene.cc.cycle];

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

    attack::apply_weapon(scene, &mut attack);

    let opcode = attack::auto_area_sweep(scene, &mut attack, player, index);

    scene
        .outbox
        .push(attack.to_bytes_for(opcode, attack::automatic_emission(scene)));

    scene.attack.last_attack_ms = now_ms;

    let mut echo = attack;

    echo.echo = true;

    if scene.entered.skill_mask[0] & attack::REACH_BONUS_BIT != 0 {
        echo.flags |= 2;
    }

    attack::apply(scene, wire::ATTACK, &echo, now_ms);

    Swung::Swung {
        entity,
        opcode,
        action,
    }
}

pub const POTION_SAMPLE_MS: u32 = 3;

pub const FREEZE_STAMPS: [usize; 3] = [1, 3, 4];

pub const ACTION_LIMIT: usize = 1;

pub fn bar_page(scene: &FieldScene) -> usize {
    let page = scene.hud.as_ref().map_or(0, |hud| hud.page);

    usize::try_from(page)
        .unwrap_or(0)
        .min(super::hud::PAGES as usize - 1)
}

pub fn mount_hp_bonus(scene: &FieldScene) -> i32 {
    scene.player.map_or(0, |player| {
        i32::from(scene.world.actors.actors[player].mount_hp_bonus)
    })
}

pub const BUFF_CODES: [u8; 27] = [
    0x03, 0x05, 0x35, 0x36, 0x09, 0x0B, 0x25, 0x29, 0x2B, 0x2C, 0x2D, 0x2E, 0x40, 0x42, 0x44, 0x46,
    0x47, 0x57, 0x4B, 0x4C, 0x4D, 0x51, 0x55, 0x59, 0x5C, 0x82, 0x0D,
];

pub const AFFECT_FRESH_UNITS: i32 = 3;

pub const FEED_GAUGE_ALARM: std::ops::RangeInclusive<i32> = 1..=5;

pub const SUMMON_CODES: std::ops::RangeInclusive<u8> = 0x38..=0x3F;

pub const SOUL_DISCOUNT_TIER: i32 = 2;

pub const SOUL_DISCOUNT_LEVEL: u16 = 0x4F;

pub const QUEUED_APPROACH: bool = false;

pub const MOVE_STANDS_STILL: i32 = 2;

pub const CC_MODE_MAGICAL: i32 = 2;

pub const CC_MODE_BUFFS_ONLY: i32 = 3;

pub const REACH_WIDEN: i32 = 5;

pub const REACH_WIDE_FLOOR: i32 = 8;

pub fn evolution_tier(scene: &FieldScene) -> i32 {
    if !scene.entered.equipment.evolved() {
        return 0;
    }

    1 + i32::from(scene.entered.skill_mask[0] & account::EVOLVED_BIT != 0)
}

pub fn mount_max_hp(scene: &FieldScene) -> Option<i32> {
    let equipment = &scene.entered.equipment;

    if !equipment.mount_in_item_list() {
        return None;
    }

    let id = usize::try_from(equipment.mount_id()).ok()?;

    let index = scene
        .statics
        .items
        .get(id)
        .map_or(0, |entry| i32::from(entry.mount_index));

    let base = usize::try_from(index)
        .ok()
        .filter(|&i| i < mount::RECORDS)
        .and_then(|i| scene.statics.mounts.get(i))
        .map(|row| row.base_max_hp())
        .filter(|&hp| hp != 0)
        .unwrap_or(mount::BASE_MAX_HP_FALLBACK);

    let scaled = mount::maturity_scale(base, i32::from(equipment.mount_maturity()));

    let bonus = mount_hp_bonus(scene);

    Some(scaled.saturating_add(bonus.saturating_mul(scaled) / 100))
}

pub fn feed_threshold(scene: &FieldScene, mount_max: i32) -> i32 {
    match scene.combat.feed {
        0 => 0,
        percent => percent.saturating_mul(mount_max) / 100,
    }
}

#[derive(Debug, Clone, PartialEq)]

pub enum Consumed {
    Fed {
        gauge: bool,
        fed: consume::Fed,
    },
    Drank {
        which: consume::Which,
        drunk: consume::Drunk,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]

pub enum Stopped {
    #[default]
    Off,
    Idle(&'static str),
    Froze,
    Frozen {
        cleared: bool,
    },
    Fed {
        gauge: bool,
    },
    Buffed {
        slot: usize,
        code: u8,
        cast: cast::Cast,
    },
    Replayed(cast::Cast),
    Engaged,
    Cast(cast::Cast),
    Marched {
        tile: (i32, i32),
        walked: Walked,
    },
    WalkedHome(Walked),
    Acquired(Acquired),
    Dropped(&'static str),
}

#[derive(Debug, Clone, PartialEq, Default)]

pub struct Hunted {
    pub consumed: Vec<Consumed>,
    pub swung: Option<Swung>,
    pub stopped: Stopped,
}

pub fn describe(hunted: &Hunted) -> Option<String> {
    match &hunted.stopped {
        Stopped::Off | Stopped::Idle(_) | Stopped::Engaged => None,
        Stopped::Froze => Some("NWYD: the macro is frozen while a modal is open".to_string()),
        Stopped::Frozen { cleared } => match cleared {
            true => Some("NWYD: the macro's modal freeze expired, and cost a frame".to_string()),
            false => None,
        },
        Stopped::Fed { .. } => None,
        Stopped::Buffed { slot, code, cast } => Some(format!(
            "NWYD: the macro re-cast slot {slot} (code {code:#04X}) — {}",
            cast.describe().unwrap_or_else(|| "refused".to_string()),
        )),
        Stopped::Replayed(cast) => Some(format!(
            "NWYD: the macro replayed the queued right-click — {}",
            cast.describe().unwrap_or_else(|| "refused".to_string()),
        )),
        Stopped::Cast(cast) => cast.describe(),
        Stopped::Marched { tile, walked } => Some(format!(
            "NWYD: the macro is marching to {tile:?}{}",
            match walked.answer() {
                true => "",
                false => " (the walk was refused)",
            },
        )),
        Stopped::WalkedHome(walked) => walked.describe(),
        Stopped::Acquired(acquired) => acquired
            .entity()
            .map(|entity| format!("NWYD: the macro is hunting entity {entity}")),
        Stopped::Dropped(why) => Some(format!("NWYD: the macro dropped its target — {why}")),
    }
}

pub fn tick(scene: &mut FieldScene, now_ms: u32) -> Hunted {
    let mut out = Hunted::default();

    if scene.combat.mode == super::hud::CC_MODE_DEFAULT {
        return out;
    }

    let Some(player) = scene.player else {
        out.stopped = Stopped::Idle(crate::internal_reason!(
            "NWYD-R0039",
            "this scene has no local body"
        ));

        return out;
    };

    if cast::hidden(&scene.world.actors.actors[player]) {
        out.stopped = Stopped::Idle(crate::internal_reason!(
            "NWYD-R0161",
            "the player is GM-hidden"
        ));

        return out;
    }

    if FREEZE_STAMPS
        .iter()
        .any(|&slot| scene.lock.stamps[slot] != 0)
    {
        scene.cc.freeze_ms = now_ms;

        scene.cc.target = None;

        out.stopped = Stopped::Froze;

        return out;
    }

    if scene.world.actors.actors[player].is_dead {
        scene.cc.target = None;

        out.stopped = Stopped::Dropped("the player is dead or dying");

        return out;
    }

    consumables(scene, &mut out, now_ms);

    if out.stopped != Stopped::Off {
        return out;
    }

    if let Some(stopped) = freeze(scene, now_ms) {
        out.stopped = stopped;

        return out;
    }

    if scene.world.actors.actors[player].action > ACTION_LIMIT {
        out.stopped = Stopped::Idle(crate::internal_reason!(
            "NWYD-R0162",
            "the body is doing something other than standing or walking"
        ));

        return out;
    }

    if scene.hud.is_none() {
        out.stopped = Stopped::Idle(crate::internal_reason!("NWYD-R0163", "PanelHUD is not up"));

        return out;
    }

    if let Some(stopped) = buff_loop(scene, player, now_ms) {
        out.stopped = stopped;

        return out;
    }

    if scene.combat.mode == CC_MODE_BUFFS_ONLY {
        out.stopped = Stopped::Idle(crate::internal_reason!(
            "NWYD-R0164",
            "mode 3 is buffs and potions only"
        ));

        return out;
    }

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    if cast::forbidden(here) {
        out.stopped = Stopped::Idle(crate::internal_reason!(
            "NWYD-R0165",
            "the player is inside the nine-by-nine forbidden square"
        ));

        return out;
    }

    if attack::safe_here(scene.world.actors.attributes(), here) {
        out.stopped = Stopped::Idle(crate::internal_reason!(
            "NWYD-R0166",
            "the player is standing in a safe cell"
        ));

        return out;
    }

    out.stopped = match scene.combat.mode {
        CC_MODE_PHYSICAL => tick_physical_auto_combat(scene, player, &mut out.swung, now_ms),
        CC_MODE_MAGICAL => tick_skill_auto_combat(scene, player, now_ms),
        _ => Stopped::Idle(crate::internal_reason!(
            "NWYD-R0167",
            "the CC mode is neither 1 nor 2"
        )),
    };

    out
}

fn consumables(scene: &mut FieldScene, out: &mut Hunted, now_ms: u32) {
    let status = scene.entered.status;

    let cur_hp = i32::try_from(status.hp).unwrap_or(i32::MAX);

    let cur_mp = i32::try_from(status.mp).unwrap_or(i32::MAX);

    let max_hp = i32::try_from(status.max_hp).unwrap_or(i32::MAX);

    let max_mp = i32::try_from(status.max_mp).unwrap_or(i32::MAX);

    let item = scene.entered.equipment.mount();

    let (mount_hp, mount_food) = {
        let effects = scene.statics.effects();

        (
            effects.total_scaled(&item, effect::PET_PROGRESS),
            effects.total_scaled(&item, effect::PET_FOOD),
        )
    };

    let mount_max = mount_max_hp(scene);

    let feed_at = feed_threshold(scene, mount_max.unwrap_or(0));

    let hp_at = scene.combat.potion.saturating_mul(max_hp) / 100;

    let mp_at = scene.combat.potion.saturating_mul(max_mp) / 100;

    if !now_ms.is_multiple_of(POTION_SAMPLE_MS) {
        return;
    }

    if mount_max.is_some() && mount_hp > 0 && mount_hp < feed_at {
        let fed = field::feed_mount(scene, now_ms);

        let answered = fed.answer();

        out.consumed.push(Consumed::Fed { gauge: false, fed });

        if answered {
            out.stopped = Stopped::Fed { gauge: false };

            return;
        }
    }

    if cur_hp < hp_at {
        let drunk = field::drink(scene, consume::Which::Hp, now_ms);

        let answered = drunk.answer();

        out.consumed.push(Consumed::Drank {
            which: consume::Which::Hp,
            drunk,
        });

        if answered {
            out.stopped = Stopped::Idle(crate::internal_reason!(
                "NWYD-R0168",
                "the HP potion answered 1 (consume::POTION_ANSWER)"
            ));

            return;
        }
    }

    if scene.cc.mp_gate != MP_GATE_OFF && cur_mp < mp_at {
        let drunk = field::drink(scene, consume::Which::Mp, now_ms);

        let answered = drunk.answer();

        out.consumed.push(Consumed::Drank {
            which: consume::Which::Mp,
            drunk,
        });

        if answered {
            out.stopped = Stopped::Idle(crate::internal_reason!(
                "NWYD-R0169",
                "the MP potion answered 1 (consume::POTION_ANSWER)"
            ));

            return;
        }
    }

    if FEED_GAUGE_ALARM.contains(&mount_food) {
        let fed = field::feed_mount(scene, now_ms);

        let answered = fed.answer();

        out.consumed.push(Consumed::Fed { gauge: true, fed });

        if answered {
            out.stopped = Stopped::Fed { gauge: true };
        }
    }
}

fn freeze(scene: &mut FieldScene, now_ms: u32) -> Option<Stopped> {
    let stamp = scene.cc.freeze_ms;

    if stamp == 0 || stamp > now_ms {
        return None;
    }

    if now_ms.wrapping_sub(stamp) <= MODAL_FREEZE_MS {
        return Some(Stopped::Frozen { cleared: false });
    }

    scene.cc.freeze_ms = 0;

    Some(Stopped::Frozen { cleared: true })
}

pub fn buff_mastery(scene: &FieldScene, code: u8) -> Option<i32> {
    let group = match code {
        0x03 | 0x05 | 0x35 | 0x36 | 0x4A | 0x4C | 0x4D => 0,
        0x09 | 0x0B | 0x0D | 0x0F | 0x25 | 0x56 | 0x57 => 1,
        0x29 | 0x2B | 0x2C | 0x2D | 0x2E | 0x40 | 0x42 | 0x44 | 0x46 | 0x47 | 0x59 | 0x5A => 2,
        _ => return None,
    };

    scene.entered.status.skill_mastery(group).map(i32::from)
}

fn buff_loop(scene: &mut FieldScene, player: usize, now_ms: u32) -> Option<Stopped> {
    let first = bar_page(scene) * PAGE_SLOTS;

    for slot in first..first + PAGE_SLOTS {
        let Some(&code) = scene.entered.quick.get(slot) else {
            break;
        };

        if code == formats::net::enter::QUICK_EMPTY {
            continue;
        }

        let row = usize::from(code);

        if BUFF_CODES.contains(&code) {
            if buffed(scene, player, code) {
                continue;
            }

            let bonus = match buff_mastery(scene, code) {
                Some(mastery) => scene
                    .statics
                    .skills
                    .as_ref()
                    .and_then(|table| table.get(row))
                    .map_or(0, |skill| skill.recast_ms(mastery)),
                None => 0,
            };

            let bonus = u32::try_from(bonus).unwrap_or(0);

            if now_ms < scene.cc.skill_stamps[row].wrapping_add(bonus) {
                continue;
            }

            let weapon = scene
                .statics
                .effects()
                .total_scaled(&scene.entered.equipment.items[7], 0x15);

            if !formats::data::retail::skill_weapon_allowed(code, weapon) {
                continue;
            }

            let cast = attempt(scene, slot, now_ms);

            if cast.answer() {
                return Some(Stopped::Buffed { slot, code, cast });
            }

            continue;
        }

        if code == cast::SOUL_CODE {
            let mut seconds = COOLDOWN_0X66_S;

            if evolution_tier(scene) == SOUL_DISCOUNT_TIER
                && scene.entered.status.level >= SOUL_DISCOUNT_LEVEL
            {
                seconds = COOLDOWN_0X66_EVOLVED_S;
            }

            let due = seconds.saturating_mul(1_000);

            if now_ms < scene.cc.skill_stamps[STAMP_0X66].wrapping_add(due) {
                continue;
            }

            let cast = attempt(scene, slot, now_ms);

            return Some(Stopped::Buffed { slot, code, cast });
        }

        if SUMMON_CODES.contains(&code) {
            if now_ms < scene.cc.skill_stamps[row].wrapping_add(COOLDOWN_0X38_MS) {
                continue;
            }

            let cast = attempt(scene, slot, now_ms);

            return Some(Stopped::Buffed { slot, code, cast });
        }
    }

    None
}

fn buffed(scene: &FieldScene, player: usize, code: u8) -> bool {
    scene.world.actors.actors[player]
        .details
        .iter()
        .any(|detail| {
            detail.kind != 0
                && affect::source_matches(detail.kind, code)
                && detail.time_8s > AFFECT_FRESH_UNITS
        })
}

fn attempt(scene: &mut FieldScene, slot: usize, now_ms: u32) -> cast::Cast {
    scene.entered.quick_selected = u8::try_from(slot).unwrap_or(0);

    let at = field::cursor_ground(scene);

    cast::cast(scene, (at.x, at.y, at.z), now_ms, true, None)
}

fn tick_physical_auto_combat(
    scene: &mut FieldScene,
    player: usize,
    swung: &mut Option<Swung>,
    now_ms: u32,
) -> Stopped {
    if now_ms
        <= scene
            .attack
            .last_attack_ms
            .wrapping_add(attack::ATTACK_INTERVAL_MS)
    {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0170",
            "the shared one-second floor has not run out"
        ));
    }

    if let Some(queued) = scene.cc.queued.take() {
        return replay_queued_cast(scene, queued, now_ms);
    }

    if let Some(entity) = scene.cc.target {
        match scene
            .world
            .actors
            .entity(entity)
            .filter(|&index| index != player && index != 0)
        {
            None => scene.cc.target = None,
            Some(_) => {
                let chase = scene.combat.movement != MOVE_STANDS_STILL;

                let result = swing(scene, entity, chase, now_ms);

                if let Some(current) = scene.cc.target
                    && let Some(at) = scene.world.actors.entity(current)
                    && scene.world.actors.actors[at].flags.invisible()
                    && scene.world.actors.actors[at].class == attack::INVISIBLE_CLASS
                {
                    scene.cc.target = None;
                }

                if !result.keeps_the_target() {
                    scene.cc.target = None;
                }

                *swung = Some(result);
            }
        }
    }

    if scene.cc.target.is_some() {
        return Stopped::Engaged;
    }

    let base = attack::reach(scene);

    let wide = base.saturating_add(REACH_WIDEN).max(REACH_WIDE_FLOOR);

    let acquired = if scene.combat.movement == MOVE_PLANS_A_PATH {
        let first = acquire(scene, base, now_ms);

        scene.cc.target = first.entity();

        if first.entity().is_some() {
            first
        } else if away_from_home(scene).unwrap_or(0) > 0 {
            let _ = walk_home(scene, now_ms);

            first
        } else {
            let second = acquire(scene, wide, now_ms);

            scene.cc.target = second.entity();

            second
        }
    } else {
        let scanned = acquire(scene, wide, now_ms);

        scene.cc.target = scanned.entity();

        scanned
    };

    if scene.cc.target.is_some() || scene.combat.movement != MOVE_PLANS_A_PATH {
        return Stopped::Acquired(acquired);
    }

    if away_from_home(scene).unwrap_or(0) <= 0 {
        return Stopped::Acquired(acquired);
    }

    Stopped::WalkedHome(walk_home(scene, now_ms))
}

fn replay_queued_cast(scene: &mut FieldScene, queued: Queued, now_ms: u32) -> Stopped {
    if let Some(entity) = queued.target
        && scene
            .world
            .actors
            .entity(entity)
            .filter(|&index| index != 0)
            .is_none()
    {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0171",
            "the queued right-click's body has left the world"
        ));
    }

    let cast = cast::cast(scene, queued.at, now_ms, QUEUED_APPROACH, queued.target);

    if cast.answer() {
        scene.cc.target = queued.target;
    }

    Stopped::Replayed(cast)
}

fn tick_skill_auto_combat(scene: &mut FieldScene, player: usize, now_ms: u32) -> Stopped {
    if now_ms
        < scene
            .attack
            .last_attack_ms
            .wrapping_add(attack::ATTACK_INTERVAL_MS)
    {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0172",
            "the shared one-second floor has not run out (mode 2's jb)"
        ));
    }

    if cast::rooted(&scene.world.actors.actors[player]) {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0173",
            "an affect has the caster's casting-block flag set"
        ));
    }

    let Some(entity) = scene.cc.target else {
        return acquire_auto_skill_target(scene, now_ms);
    };

    let Some(index) = scene
        .world
        .actors
        .entity(entity)
        .filter(|&index| index != 0)
    else {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0174",
            "the target is no longer in the world"
        ));
    };

    let here = attack::cell_of(&scene.world.actors.actors[player]);

    let there = attack::cell_of(&scene.world.actors.actors[index]);

    let distance = path::tile_distance(here, there);

    let Some(&code) = scene
        .entered
        .quick
        .get(usize::from(scene.entered.quick_selected))
    else {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0055",
            "the selected quick slot is outside the bar"
        ));
    };

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|table| table.get(usize::from(code)))
        .copied()
    else {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0175",
            "SkillData.bin did not load, so no range can be read"
        ));
    };

    let mut range = skill.range();

    if scene.combat.movement != MOVE_STANDS_STILL {
        range += REACH_WIDEN;
    }

    if distance > range {
        scene.cc.target = None;

        return Stopped::Dropped(crate::internal_reason!(
            "NWYD-R0197",
            "the target is past the armed skill's range"
        ));
    }

    if scene.world.actors.actors[index].flags.invisible()
        && scene.world.actors.actors[index].class == attack::INVISIBLE_CLASS
    {
        scene.cc.target = None;

        return Stopped::Dropped(crate::internal_reason!(
            "NWYD-R0198",
            "the target is an invisible class-0x42 body"
        ));
    }

    let body = scene.world.actors.actors[index].position;

    let at = (body.x, body.y, body.z);

    let cast = cast::skill_attack(scene, entity, at, now_ms);

    if cast.answer() {
        return Stopped::Cast(cast);
    }

    if scene.world.actors.actors[index].is_dead {
        scene.cc.target = None;

        if away_from_home(scene).unwrap_or(0) > 0 {
            return Stopped::WalkedHome(walk_home(scene, now_ms));
        }

        return Stopped::Dropped(crate::internal_reason!(
            "NWYD-R0199",
            "the target died mid-cast"
        ));
    }

    if scene.combat.movement == MOVE_STANDS_STILL {
        scene.cc.target = None;

        return Stopped::Dropped(crate::internal_reason!(
            "NWYD-R0200",
            "movement mode 2 never chases"
        ));
    }

    let range = skill.range();

    let Some(marched) = march(scene, there, range, distance) else {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0176",
            "the target is already inside the range the march would close"
        ));
    };

    scene.cc.auto_move = marched.cell;

    let y = ground_y(scene, marched.cell);

    let walked = walk(
        scene,
        (marched.cell.0 as f32, y, marched.cell.1 as f32),
        now_ms,
        WALK_MACRO,
    );

    if !walked.answer() {
        scene.attack.stuck = scene.cc.target;
    }

    Stopped::Marched {
        tile: marched.cell,
        walked,
    }
}

fn acquire_auto_skill_target(scene: &mut FieldScene, now_ms: u32) -> Stopped {
    let Some(&code) = scene
        .entered
        .quick
        .get(usize::from(scene.entered.quick_selected))
    else {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0055",
            "the selected quick slot is outside the bar"
        ));
    };

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|table| table.get(usize::from(code)))
        .copied()
    else {
        return Stopped::Idle(crate::internal_reason!(
            "NWYD-R0175",
            "SkillData.bin did not load, so no range can be read"
        ));
    };

    let mut range = skill.range();

    if scene.combat.movement != MOVE_STANDS_STILL {
        range += REACH_WIDEN;
    }

    let acquired = acquire(scene, range, now_ms);

    scene.cc.target = acquired.entity();

    if scene.cc.target.is_some() || scene.combat.movement != MOVE_PLANS_A_PATH {
        return Stopped::Acquired(acquired);
    }

    if away_from_home(scene).unwrap_or(0) <= 0 {
        return Stopped::Acquired(acquired);
    }

    Stopped::WalkedHome(walk_home(scene, now_ms))
}

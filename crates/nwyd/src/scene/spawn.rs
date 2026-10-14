use super::actors::{AlternateHp, Doll};
use super::field::FieldScene;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::item_effect::Effects;
use formats::data::item_list::ItemList;
use formats::data::mount::{MountData, MountVisual};
use formats::mesh::appearance::Look;
use formats::net::affect;
use formats::net::spawn::{PLAYER_ID_LIMIT, Spawn};
use formats::world::HEIGHT_SCALE;
use formats::world::path::HeightField;
use glam::Vec3;

fn is_khepra(actor: &super::actors::Actor) -> bool {
    actor.class == 0x38 && actor.parts[0].mesh == 0
}

#[derive(Debug)]

pub struct Statics {
    pub items: ItemList,
    pub mounts: Vec<MountData>,
    pub visuals: Vec<MountVisual>,
    pub skills: Option<formats::data::skill_data::SkillData>,
    pub icons: formats::data::item_icon::IconTable,
}

impl Statics {
    pub fn effects(&self) -> Effects<'_> {
        Effects::new(&self.items, &self.mounts)
    }
}

pub const HEAL_ROW: i32 = 6;

pub fn heal_skill(statics: &Statics, skill_id: i16) -> bool {
    let Ok(id) = usize::try_from(skill_id) else {
        return false;
    };

    statics
        .skills
        .as_ref()
        .and_then(|s| s.get(id))
        .is_some_and(|row| row.damage_kind == HEAL_ROW)
}

pub const CON_DIVISOR_PLAYER: f32 = 4000.0;

pub const CON_DIVISOR_MOB: f32 = 2000.0;

pub const CON_CLAMP: f32 = 500.0;

pub const SHRUNK_BODIES: [i16; 2] = [0x16B, 0x4B];

pub const SMALL_BODY_LIMIT: u16 = 0x28;

pub const FRESH_SPAWN_SHRINK: f32 = 0.9;

pub const ALT_HP_CAP: u32 = 2_000_000_000;

pub fn alternate_hp(spawn: &Spawn) -> Option<AlternateHp> {
    let multiplier = spawn.alt_hp_multiplier()?;

    let current = spawn.score.hp.wrapping_mul(multiplier).min(ALT_HP_CAP);

    let max = spawn.score.max_hp.wrapping_mul(multiplier).min(ALT_HP_CAP);

    Some(AlternateHp {
        current,
        max,
        base_max: max,
    })
}

fn apply_alternate_hp(
    standing: &mut Option<AlternateHp>,
    incoming: Option<AlternateHp>,
    fresh: bool,
) {
    if fresh || incoming.is_some() {
        *standing = incoming;
    }
}

pub fn body_scale(con: u16, body: u16, id: u16, fresh: bool) -> f32 {
    let con = if id >= 1000 && con > 8000 { 0 } else { con };

    let (clamp, divisor) = match id < PLAYER_ID_LIMIT {
        true => (CON_CLAMP, CON_DIVISOR_PLAYER),
        false => (f32::INFINITY, CON_DIVISOR_MOB),
    };

    let con = match SHRUNK_BODIES.contains(&(body as i16)) {
        true => -1000.0,
        false => f32::from(con).min(clamp),
    };

    let mut scale = (con / divisor + 1.0) * 0.9;

    if fresh && body < SMALL_BODY_LIMIT {
        scale *= FRESH_SPAWN_SHRINK;
    }

    scale
}

pub fn facing(code: u8) -> f32 {
    use std::f32::consts::PI;

    match code {
        1 => 1.75 * PI,
        2 => 1.5 * PI,
        3 => 1.25 * PI,
        6 => PI,
        7 => 0.25 * PI,
        8 => 0.5 * PI,
        9 => 0.75 * PI,
        _ => 0.0,
    }
}

pub fn facing_of(spawn: &Spawn) -> f32 {
    facing(spawn.score.flags.to_le_bytes()[0] >> 4)
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Spawned {
    pub id: u16,
    pub index: Option<usize>,
    pub reused: bool,
    pub stale: bool,
    pub panels: super::vanish::PanelTeardown,
    pub shop: bool,
    pub undrawable: bool,
    pub was_armed: bool,
}

pub fn doll(
    spawn: &Spawn,
    statics: &Statics,
    ground: f32,
    fresh: bool,
    opcode: u16,
) -> Option<Doll> {
    let effects = statics.effects();

    let look = Look::for_spawn(
        &spawn.equip,
        &spawn.effect,
        &spawn.refine,
        spawn.level(),
        spawn.flags,
        spawn.extra,
        &effects,
        &statics.visuals,
    )?;

    Some(Doll {
        look,
        pet_item: spawn.equip[13],
        position: Vec3::new(f32::from(spawn.x) + 0.5, ground, f32::from(spawn.z) + 0.5),
        yaw: facing_of(spawn),
        scale: body_scale(spawn.score.stat[CON_STAT], spawn.equip[0], spawn.id, fresh),
        hands: spawn.hands(),
        name: spawn.name(),
        record: usize::from(spawn.id),
        entity: spawn.id,
        status: spawn.score,
        name_level: match spawn.is_player() {
            true => spawn.name_level,
            false => MOB_NAME_LEVEL,
        },
        level: spawn.level(),
        guild: spawn.guild,
        guild_name: spawn.guild_name(),
        face: i32::from(spawn.equip[0] as i16),
        affects: affect::Affect::all(&spawn.affect),
        pick_action: None,
        shop_title: match opcode == protocol::server::SPAWN_SHOP {
            true => spawn.motto(),
            false => String::new(),
        },
        green_name: spawn.id >= super::nameplate::PLAYER_ID_LIMIT && spawn.score.defense == 0,
    })
}

pub const CON_STAT: usize = super::select_char::CON_STAT;

pub const MOB_NAME_LEVEL: u8 = 0x4B;

pub fn apply(
    scene: &mut FieldScene,
    game: &GameDir,
    opcode: u16,
    mut packet: Spawn,
) -> Result<Spawned> {
    if packet.id >= PLAYER_ID_LIMIT {
        tracing::debug!(
            target : "nwyd::mob_evidence", event = "spawn", opcode, entity = packet.id,
            body = packet.equip[0], defense = packet.score.defense,
            green_name = packet.score.defense == 0, "mob green-name source"
        );
    }

    super::store::finish_panel_close(scene);

    let mut existing = scene.world.actors.entity(packet.id);

    let stale = existing.is_some_and(|index| {
        let actor = &scene.world.actors.actors[index];

        packet.id >= PLAYER_ID_LIMIT
            && i32::from(packet.equip[0] as i16) != actor.body
            && packet.name() != actor.name
    });

    let panels = match stale {
        true => super::vanish::teardown_panels(scene, packet.id),
        false => super::vanish::PanelTeardown::default(),
    };

    if stale && let Some(index) = existing {
        if scene.player != Some(index) {
            if scene.world.actors.actors[index].removal_mode == super::corpse::MODE_NONE {
                super::start::vanishing(scene, index, scene.time.now_ms, true);
            }

            super::action::remove(scene, packet.id);

            existing = scene.world.actors.entity(packet.id);
        }
    }

    if opcode == protocol::server::SPAWN {
        packet.apply_arena(
            &scene.world.actors.registries.arena_visual,
            &scene.statics.effects(),
        );
    }

    let shop = opcode == protocol::server::SPAWN_SHOP && packet.shop_rewrite();

    let packet_alternate_hp = alternate_hp(&packet);

    let was_armed = match existing {
        Some(old) => {
            let actor = &mut scene.world.actors.actors[old];

            let armed = actor.removal_mode == super::corpse::MODE_ARMED;

            actor.corpse_ms = None;

            actor.removal_mode = super::corpse::MODE_NONE;

            armed
        }
        None => false,
    };

    if let Some(old) = existing.filter(|_| was_armed) {
        if is_khepra(&scene.world.actors.actors[old]) {
            scene.khepra = Some(packet.id);
        }

        apply_alternate_hp(
            &mut scene.world.actors.actors[old].alternate_hp,
            packet_alternate_hp,
            false,
        );

        if packet_alternate_hp.is_some() {
            super::nameplate::refresh_backdrop(&mut scene.world.actors.actors[old]);
        }

        super::party::spawn_markers(scene, old, (packet.x, packet.z), false);

        return Ok(Spawned {
            id: packet.id,
            index: Some(old),
            reused: true,
            stale,
            panels,
            shop,
            undrawable: false,
            was_armed: true,
        });
    }

    let zone = scene.world.actors.zone;

    let ground = f32::from(
        HeightField::window(&scene.world.actors.window, zone)
            .byte(i32::from(packet.x), i32::from(packet.z)) as i8,
    ) * HEIGHT_SCALE;

    let fresh = existing.is_none();

    let Some(mut doll) = doll(&packet, &scene.statics, ground, fresh, opcode) else {
        return Ok(Spawned {
            id: packet.id,
            index: None,
            reused: !fresh,
            stale,
            panels,
            shop,
            undrawable: false,
            was_armed: false,
        });
    };

    crate::game_settings::Interface::project(scene, &mut doll);

    let placed = scene
        .world
        .actors
        .place_one(game, &doll)
        .with_context(|| format!("placing the entity {} spawned into", packet.id))?;

    if let Some(index) = placed {
        scene
            .world
            .objects
            .load_held_actor(game, &mut scene.world.actors, index)
            .with_context(|| format!("loading the weapons entity {} is holding", packet.id))?;

        let objects = &scene.world.objects;

        scene
            .world
            .actors
            .place_actor_swing_radii(index, |id| objects.mesh_max_z(id));
    }

    let index = match (placed, existing) {
        (Some(new), Some(old)) => {
            let record = scene.world.actors.actors[old].record;

            let incarnation = scene.world.actors.actors[old].incarnation;

            let name_backdrop = (
                scene.world.actors.actors[old].plate_outline,
                scene.world.actors.actors[old].green_name,
                scene.world.actors.actors[old].plate_name_color,
            );

            let body_cloud_ms = scene.world.actors.actors[old].body_cloud_ms;

            let impact_grid_ms = scene.world.actors.actors[old].impact_grid_ms;

            let affect_pair_ms = scene.world.actors.actors[old].affect_pair_ms;

            let body_glow_points = scene.world.actors.actors[old].body_glow.points;

            let footprint_kind = scene.world.actors.actors[old].footprint_kind;

            let appearance_epoch = scene.world.actors.actors[old]
                .appearance_epoch
                .wrapping_add(1);

            let details = scene.world.actors.actors[old].details;

            let target_hud = scene.world.actors.actors[old].target_hud;

            let alternate_hp = scene.world.actors.actors[old].alternate_hp;

            let mount_hp_bonus = scene.world.actors.actors[old].mount_hp_bonus;

            let balloon = scene.world.actors.actors[old].balloon.clone();

            scene.world.actors.actors.swap(old, new);

            scene.world.actors.actors.truncate(new);

            scene.world.actors.actors[old].record = record;

            scene.world.actors.actors[old].incarnation = incarnation;

            (
                scene.world.actors.actors[old].plate_outline,
                scene.world.actors.actors[old].green_name,
                scene.world.actors.actors[old].plate_name_color,
            ) = name_backdrop;

            scene.world.actors.actors[old].appearance_epoch = appearance_epoch;

            scene.world.actors.actors[old].body_cloud_ms = body_cloud_ms;

            scene.world.actors.actors[old].impact_grid_ms = impact_grid_ms;

            scene.world.actors.actors[old].affect_pair_ms = affect_pair_ms;

            scene.world.actors.actors[old].body_glow.points = body_glow_points;

            scene.world.actors.actors[old].footprint_kind = footprint_kind;

            super::footprints::refresh_type(&mut scene.world.actors.actors[old]);

            scene.world.actors.actors[old].details = details;

            scene.world.actors.actors[old].target_hud = target_hud;

            scene.world.actors.actors[old].alternate_hp = alternate_hp;

            scene.world.actors.actors[old].mount_hp_bonus = mount_hp_bonus;

            scene.world.actors.actors[old].balloon = balloon;

            let actor = &mut scene.world.actors.actors[old];

            let wearer = affect::Wearer {
                class: actor.class,
                part0_mesh: actor.parts[0].mesh as u16,
            };

            let (affects, details) = (actor.affects, actor.details);

            actor.flags.rebuild(&affects, &details, &wearer);

            let flags = actor.flags;

            scene.world.actors.apply_affects(old, &flags);

            Some(old)
        }
        (index, _) => index,
    };

    if let Some(index) = index {
        let actor = &mut scene.world.actors.actors[index];

        actor.guild_rank = u16::from(packet.guild_rank);

        super::nameplate::refresh_backdrop(actor);

        actor.plate_backdrop_color = 0x55AA_0000;

        if packet.id >= 1000 && packet.score.defense == 0 {
            actor.plate_backdrop_color = 0x5500_AA00;

            actor.plate_outline = true;

            actor.green_name = true;
        }

        scene.world.actors.actors[index].costume_item = packet.extra;

        if is_khepra(&scene.world.actors.actors[index]) {
            scene.khepra = Some(packet.id);
        }

        apply_alternate_hp(
            &mut scene.world.actors.actors[index].alternate_hp,
            packet_alternate_hp,
            fresh,
        );

        let speed = super::enter::move_speed(&packet.score);

        scene.world.actors.actors[index].speed = speed;

        if scene.player == Some(index) {
            scene.wire_speed = speed as i32;
        }

        if fresh {
            super::start::spawned(scene, index, packet.create_type, scene.time.now_ms);
        }

        super::party::spawn_markers(scene, index, (packet.x, packet.z), fresh);
    }

    let undrawable = index.is_some_and(|index| {
        let actors = &scene.world.actors;

        let model = &actors.models[actors.actors[index].model];

        model.parts.iter().all(|part| part.mesh.indices.len() < 3)
    });

    Ok(Spawned {
        id: packet.id,
        index,
        reused: !fresh,
        stale,
        panels,
        shop,
        undrawable,
        was_armed: false,
    })
}

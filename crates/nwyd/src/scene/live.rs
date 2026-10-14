use super::actors::Doll;
use super::attack::{TEXT_ANCHOR_LIFT, TEXT_DROP, project_text};
use super::damage::{self, Number};
use super::field::FieldScene;
use super::spawn::{CON_STAT, body_scale};
use anyhow::{Context, Result};
use formats::mesh::appearance;
use formats::mesh::appearance::Look;
use formats::net::affect::{AFFECTS, Wearer};
use formats::net::enter::{QUICK_CLASS_SPAN, QUICK_EMPTY, QUICK_SLOTS};
use formats::net::firework::Glyph;
use formats::net::live::{
    ActorStatus, Container, EquipChange, Gold, HpDelta, PlayerProgression, SlotPatch, Vitals,
};
use formats::net::spawn::PLAYER_ID_LIMIT;
use formats::scene::demo::APPEARANCE_EFFECT;

pub const ACTION_REVIVE: usize = 0xE;

pub const ACTION_DEAD: usize = appearance::ACTION_DEAD;

pub const DEAD_ACTION: usize = ACTION_DEAD;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vitalised {
    pub entity: u16,
    pub index: Option<usize>,
    pub local: bool,
    pub broadcast: bool,
    pub hp: u32,
    pub mp: u32,
    pub dead: bool,
    pub revived: bool,
    pub dropped_target: bool,
}

pub fn vitals(scene: &mut FieldScene, entity: u16, vitals: &Vitals) -> Vitalised {
    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    let dead = vitals.is_dead();

    let dropped_target =
        scene.attack.auto && dead && scene.attack.auto_target == Some(entity) && index.is_some();

    if dropped_target {
        scene.attack.auto_target = None;
    }

    let Some(index) = index else {
        return Vitalised {
            entity,
            index,
            local,
            broadcast: false,
            hp: 0,
            mp: 0,
            dead,
            revived: false,
            dropped_target,
        };
    };

    let actor = &mut scene.world.actors.actors[index];

    let was_dead = actor.is_dead || actor.action == DEAD_ACTION;

    actor.status.hp = u32::try_from(vitals.hp)
        .unwrap_or(0)
        .min(actor.status.max_hp);

    actor.status.mp = u32::try_from(vitals.mp).unwrap_or(0);

    let (hp, mp) = (actor.status.hp, actor.status.mp);

    let revived = !dead && was_dead;

    if revived {
        actor.is_dead = false;

        scene
            .world
            .actors
            .play_at(index, ACTION_REVIVE, scene.time.now_ms);
    }

    if local {
        scene.entered.status = scene.world.actors.actors[index].status;
    }

    Vitalised {
        entity,
        index: Some(index),
        local,
        broadcast: local,
        hp,
        mp,
        dead,
        revived,
        dropped_target,
    }
}

pub const DELTA_DELAY_MS: u32 = 0x1F4;

pub const DELTA_KIND_LOCAL: u8 = damage::KIND_RED;

pub const DELTA_KIND_REMOTE: u8 = damage::KIND_YELLOW;

pub const DELTA_KIND_HEAL: u8 = damage::KIND_BLUE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HpChanged {
    pub entity: u16,
    pub index: Option<usize>,
    pub local: bool,
    pub broadcast: bool,
    pub hp: u32,
    pub delta: i16,
    pub raised: Option<u8>,
}

pub fn hp_delta(scene: &mut FieldScene, entity: u16, delta: &HpDelta) -> HpChanged {
    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    let Some(index) = index else {
        return HpChanged {
            entity,
            index,
            local,
            broadcast: false,
            hp: 0,
            delta: delta.delta,
            raised: None,
        };
    };

    let body = &scene.world.actors.actors[index];

    let at = [
        body.position.x,
        body.position.y + TEXT_ANCHOR_LIFT,
        body.position.z,
    ];

    let actor = &mut scene.world.actors.actors[index];

    actor.status.hp = u32::try_from(delta.hp).unwrap_or(0);

    let hp = actor.status.hp;

    let raised = raise_delta(scene, at, local, delta);

    if local {
        scene.entered.status = scene.world.actors.actors[index].status;
    }

    HpChanged {
        entity,
        index: Some(index),
        local,
        broadcast: local,
        hp,
        delta: delta.delta,
        raised,
    }
}

fn raise_delta(scene: &mut FieldScene, at: [f32; 3], local: bool, delta: &HpDelta) -> Option<u8> {
    let text = delta.text()?;

    let screen = project_text(scene, at)?;

    let kind = match delta.is_heal() {
        true => DELTA_KIND_HEAL,
        false => match local {
            true => DELTA_KIND_LOCAL,
            false => DELTA_KIND_REMOTE,
        },
    };

    let drop = (scene.layout.sy * TEXT_DROP).trunc();

    let items = damage::set_of(kind)
        .and_then(|set| damage::set_items(&scene.sets, set))
        .unwrap_or(&[]);

    let number = Number::new(
        &text,
        (screen.0, (screen.1 + drop).trunc()),
        kind,
        DELTA_DELAY_MS,
        damage::LIFE_MS,
        scene.time.now_ms,
        items,
        &scene.layout,
    );

    scene.damage.push(number);

    Some(kind)
}

fn split(pair: u32) -> [i16; 2] {
    [pair as u16 as i16, (pair >> 16) as u16 as i16]
}

fn restate_sheet(sheet: &mut super::enter::Derived, score: &ActorStatus) {
    sheet.critical = score.critical;

    sheet.mana_saving = score.mana_saving;

    sheet.attack_speed = score.player_words[0] as i16;

    sheet.value8 = score.player_words[1] as i16;

    sheet.value9 = score.player_words[2] as i16;

    let [low, high] = split(score.unknown_7a);

    sheet.regen[0] = low;

    sheet.regen[1] = high;

    let [low, high] = split(score.unknown_76);

    sheet.regen[2] = low;

    sheet.regen[3] = high;

    sheet.magic_bonus = score.magic_bonus as i16;

    sheet.words = score.words;
}

fn restore_alternate_hp(entity: u16, flag: u32, hp: &mut Option<super::actors::AlternateHp>) {
    if entity >= PLAYER_ID_LIMIT
        && flag == 1
        && let Some(hp) = hp
    {
        hp.restore();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusApplied {
    pub entity: u16,
    pub index: Option<usize>,
    pub local: bool,
    pub level: u32,
    pub hp: u32,
    pub max_hp: u32,
    pub adopted: usize,
    pub invisible: bool,
    pub revived: bool,
    pub died: bool,
}

pub fn apply_actor_status(
    scene: &mut FieldScene,
    entity: u16,
    score: &ActorStatus,
) -> StatusApplied {
    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    let level = score.score.shown_level();

    let Some(index) = index else {
        return StatusApplied {
            entity,
            index,
            local,
            level,
            hp: score.score.hp,
            max_hp: score.score.max_hp,
            adopted: 0,
            invisible: false,
            revived: false,
            died: false,
        };
    };

    let mut adopted = 0;

    let now_ms = scene.time.now_ms;

    let standing = scene.world.actors.actors[index].affects;

    let mut affects = standing;

    for slot in 0..AFFECTS {
        let elapsed = now_ms.wrapping_sub(scene.affect_stamps[slot]);

        let take = !local || ActorStatus::reconcile(standing[slot], score.affects[slot], elapsed);

        if take {
            affects[slot] = score.affects[slot];

            adopted += 1;

            if local {
                scene.affect_stamps[slot] = now_ms;
            }
        }
    }

    let actor = &mut scene.world.actors.actors[index];

    restore_alternate_hp(
        actor.entity,
        score.restore_alternate_hp,
        &mut actor.alternate_hp,
    );

    actor.status = score.score;

    actor.guild = score.guild;

    actor.mount_hp_bonus = score.mount_hp_bonus as i16;

    actor.transformation.variant = score.transformation_variant;

    actor.affects = affects;

    actor.scale = body_scale(score.score.stat[CON_STAT], actor.body as u16, entity, false);

    if actor.class == 0x28 {
        actor.scale = super::actors::titan_scale(actor.scale, score.score.mastery[3]);
    }

    let wearer = Wearer {
        class: actor.class,
        part0_mesh: actor.parts[0].mesh as u16,
    };

    let details = actor.details;

    actor.flags.rebuild(&affects, &details, &wearer);

    super::nameplate::refresh_backdrop(actor);

    if !actor.mover.pathless {
        actor.speed = super::enter::move_speed(&actor.status);

        if local {
            scene.wire_speed = actor.speed as i32;
        }
    }

    let flags = actor.flags;

    let was_dead = actor.is_dead || actor.action == DEAD_ACTION;

    let alive = score.score.hp > 0;

    let died = !alive && actor.action != ACTION_DEAD && actor.action != appearance::ACTION_DIE;

    let revived = alive && was_dead;

    if revived {
        actor.is_dead = false;
    }

    scene.world.actors.apply_affects(index, &flags);

    if died {
        scene.world.actors.actors[index].removal_mode = super::corpse::MODE_SCORE_DEATH;

        scene.world.actors.play_at(index, ACTION_DEAD, now_ms);
    }

    if revived {
        scene.world.actors.play_at(index, ACTION_REVIVE, now_ms);
    }

    if local {
        if (score.score.level as i16) > (scene.entered.status.level as i16) {
            scene.analytics.level_up(&scene.entered);
        }

        scene.entered.status = score.score;

        scene.entered.guild = score.guild;

        restate_sheet(&mut scene.entered.derived, score);
    }

    StatusApplied {
        entity,
        index: Some(index),
        local,
        level,
        hp: score.score.hp,
        max_hp: score.score.max_hp,
        adopted,
        invisible: flags.invisible(),
        revived,
        died,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressionApplied {
    pub entity: u16,
    pub local: bool,
    pub exp: u64,
    pub gold: u32,
    pub cp: bool,
    pub mask_changed: bool,
    pub blanked: Vec<usize>,
}

pub const SWEEP_BLANKS_CLASS_0: u8 = QUICK_CLASS_SPAN;

fn sweep_quick_bar(bar: &mut [u8; QUICK_SLOTS], mask: u32, class: u8) -> Vec<usize> {
    let mut blanked = Vec::new();

    for (slot, cell) in bar.iter_mut().enumerate() {
        let held = *cell;

        let bit = match held < SEPHIRA_BASE {
            true => held.wrapping_sub(class.wrapping_mul(QUICK_CLASS_SPAN)),
            false => held.wrapping_sub(SEPHIRA_BIT_DELTA),
        };

        let probe = 1u32 << (u32::from(bit) & 0x1F);

        if mask & probe != probe && held < QUICK_CLASS_SPAN {
            *cell = QUICK_EMPTY;

            blanked.push(slot);
        }
    }

    blanked
}

const SEPHIRA_BASE: u8 = 0x60;

const SEPHIRA_BIT_DELTA: u8 = 0x48;

pub fn apply_player_progression(
    scene: &mut FieldScene,
    entity: u16,
    score: &PlayerProgression,
) -> ProgressionApplied {
    let local = scene
        .world
        .actors
        .entity(entity)
        .is_some_and(|i| Some(i) == scene.player);

    let mask_changed = local && scene.entered.skill_mask != score.mask;

    let mut blanked = Vec::new();

    if local {
        let class = scene.entered.class;

        blanked = sweep_quick_bar(&mut scene.entered.quick, score.mask[0], class);

        if !blanked.is_empty() {
            scene
                .outbox
                .push(protocol::client::quick_bar(&scene.entered.quick, class));
        }

        scene.entered.exp = score.exp;

        scene.entered.gold = score.gold;

        scene.entered.cp = score.cp;

        scene.entered.skill_mask = score.mask;

        scene.entered.derived.stat_points = score.words[0] as i16;

        scene.entered.derived.mastery_points = score.words[1] as i16;

        scene.entered.derived.skill_points = score.words[2] as i16;
    }

    ProgressionApplied {
        entity,
        local,
        exp: score.exp,
        gold: score.gold,
        cp: score.cp > 0,
        mask_changed,
        blanked,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedBar {
    pub bar: [u8; QUICK_SLOTS],
    pub changed: bool,
    pub repainted: bool,
}

pub fn quick_bar(scene: &mut FieldScene, pushed: &[u8; QUICK_SLOTS]) -> PushedBar {
    let bar = formats::net::enter::rebase_for_receive(pushed, scene.entered.class);

    let changed = scene.entered.quick != bar;

    scene.entered.quick = bar;

    let items = &scene.statics.items;

    let repainted = match scene.hud.as_mut() {
        Some(hud) => {
            hud.refresh_skills(&bar, items);

            true
        }
        None => false,
    };

    PushedBar {
        bar,
        changed,
        repainted,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Golden {
    pub entity: u16,
    pub local: bool,
    pub gold: u32,
}

pub fn gold(scene: &mut FieldScene, entity: u16, gold: &Gold) -> Golden {
    let local = scene
        .world
        .actors
        .entity(entity)
        .is_some_and(|i| Some(i) == scene.player);

    if local {
        scene.entered.gold = gold.gold;
    }

    Golden {
        entity,
        local,
        gold: gold.gold,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Redressed {
    pub entity: u16,
    pub index: Option<usize>,
    pub local: bool,
    pub silent: bool,
    pub raised: Option<super::burst::Raised>,
    pub redressed: bool,
    pub body: u16,
    pub reason: Option<String>,
}

pub fn equip(
    scene: &mut FieldScene,
    game: &assets::GameDir,
    entity: u16,
    change: &EquipChange,
) -> Result<Redressed> {
    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    let mut visual_change = *change;

    if local {
        tracing::debug!(
            target : "nwyd::body_state", event = "equip_change_36b", entity, slot = scene
            .entered.slot, live_body_before = scene.entered.equipment.body().index,
            retained_body_before = scene.entered.account_body, wire_equipment = ? change
            .equip, wire_flags = change.flags, previous_flags = scene.entered.wearer
            .flags, "Server appearance update before arena projection and body refresh"
        );
    }

    visual_change.apply_arena(
        &scene.world.actors.registries.arena_visual,
        &scene.statics.effects(),
    );

    let change = &visual_change;

    let body = change.equip[0];

    if local {
        scene.entered.equipment.items[0].index = body;

        scene.entered.wearer.flags = change.flags;

        scene.interface = scene.interface.with_costume_flags(change.flags);

        scene.entered.refresh_class();

        tracing::debug!(
            target : "nwyd::body_state", event = "equip_change_36b_applied", entity,
            live_body = body, retained_body = scene.entered.account_body, class = scene
            .entered.class, "Server appearance update applied"
        );
    }

    let Some(index) = index else {
        return Ok(Redressed {
            entity,
            index,
            local,
            silent: false,
            raised: None,
            redressed: false,
            body,
            reason: Some("no body in this world carries that id".to_string()),
        });
    };

    let standing = {
        let actor = &scene.world.actors.actors[index];

        (actor.parts[0].mesh, actor.class)
    };

    let incoming = scene
        .statics
        .items
        .get(usize::from(body))
        .map(|entry| (entry.mesh, entry.effect_sum(APPEARANCE_EFFECT)));

    let silent = incoming.is_some_and(|pair| change.silent(standing, pair));

    let raised = (!silent).then(|| super::burst::equip(scene, index));

    let Some(doll) = dressed(scene, index, change) else {
        return Ok(Redressed {
            entity,
            index: Some(index),
            local,
            silent,
            raised,
            redressed: false,
            body,
            reason: Some(format!("body item {body} resolved no appearance")),
        });
    };

    if local {
        scene.entered.kingdom = doll.look.kingdom;
    }

    let redressed = scene
        .world
        .actors
        .redress(game, index, &doll)
        .with_context(|| format!("re-dressing entity {entity} after a 0x36B"))?;

    if redressed {
        settle(scene, game, index, silent)?;
    }

    Ok(Redressed {
        entity,
        index: Some(index),
        local,
        silent,
        raised,
        redressed,
        body,
        reason: match redressed {
            true => None,
            false => Some("the appearance declined — see the unresolved list".to_string()),
        },
    })
}

fn dressed(scene: &FieldScene, index: usize, change: &EquipChange) -> Option<Doll> {
    let actor = &scene.world.actors.actors[index];

    let effects = scene.statics.effects();

    let look = Look::for_spawn(
        &change.equip,
        &change.effect,
        &change.refine,
        actor.level,
        change.flags,
        actor.costume_item,
        &effects,
        &scene.statics.visuals,
    )?;

    let mut doll = Doll {
        look,
        pet_item: change.equip[13],
        position: actor.position,
        yaw: actor.yaw,
        scale: body_scale(
            actor.status.stat[CON_STAT],
            change.equip[0],
            actor.entity,
            false,
        ),
        hands: change.hands(),
        record: actor.record,
        entity: actor.entity,
        name: actor.name.clone(),
        status: actor.status,
        name_level: actor.name_level,
        level: actor.level,
        guild: actor.guild,
        guild_name: actor.guild_name.clone(),
        face: i32::from(change.equip[0] as i16),
        affects: actor.affects,
        pick_action: None,
        shop_title: actor.shop_title.clone(),
        green_name: actor.green_name,
    };

    crate::game_settings::Interface::project(scene, &mut doll);

    Some(doll)
}

fn settle(
    scene: &mut FieldScene,
    game: &assets::GameDir,
    index: usize,
    silent: bool,
) -> Result<()> {
    scene
        .world
        .objects
        .load_held_actor(game, &mut scene.world.actors, index)
        .context("loading the weapons the re-dressed body is holding")?;

    let objects = &scene.world.objects;

    scene
        .world
        .actors
        .place_actor_swing_radii(index, |id| objects.mesh_max_z(id));

    let flags = scene.world.actors.actors[index].flags;

    scene.world.actors.apply_affects(index, &flags);

    if !silent {
        scene
            .world
            .actors
            .play_at(index, ACTION_REVIVE, scene.time.now_ms);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patched {
    pub entity: u16,
    pub index: Option<usize>,
    pub local: bool,
    pub container: Container,
    pub slot: i16,
    pub written: bool,
    pub redressed: bool,
}

pub fn slot_patch(
    scene: &mut FieldScene,
    game: &assets::GameDir,
    entity: u16,
    patch: &SlotPatch,
) -> Result<Patched> {
    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    let mut written = false;

    if local {
        tracing::debug!(
            target : "nwyd::body_state", event = "slot_patch_182", entity, container = ?
            patch.container, slot = patch.slot, item = ? patch.item, live_body_before =
            scene.entered.equipment.body().index, retained_body_before = scene.entered
            .account_body, "Server item-slot update"
        );
    }

    if local && let Some(slot) = patch.index() {
        match patch.container {
            Container::Equipment => scene.entered.equipment.items[slot] = patch.item,
            Container::Carry => {
                scene.entered.carry[slot] = patch.item;

                if !patch.item.is_empty()
                    && let Some(panels) = &scene.panels
                    && let Some(name) = panels.items.name(usize::from(patch.item.index))
                {
                    scene.analytics.item_collected(
                        &scene.entered,
                        &name,
                        formats::data::item_effect::stack_size(&patch.item),
                    );
                }
            }
            Container::Cargo => scene.entered.warehouse.cargo[slot] = patch.item,
            Container::Other(_) => {}
        }

        written = !matches!(patch.container, Container::Other(_));
    }

    if local {
        scene.entered.refresh_class();
    }

    let redressed = match (local, index) {
        (true, Some(index)) => redress_player(scene, game, index)?,
        _ => false,
    };

    if index.is_some()
        && let Some(panels) = scene.panels.as_mut()
    {
        super::panels::refine_item_changed(panels, &scene.entered);
    }

    Ok(Patched {
        entity,
        index,
        local,
        container: patch.container,
        slot: patch.slot,
        written,
        redressed,
    })
}

pub fn redress_player(
    scene: &mut FieldScene,
    game: &assets::GameDir,
    index: usize,
) -> Result<bool> {
    let costume_item = scene.world.actors.actors[index].costume_item;

    let Some(mut doll) = super::field::player_doll(
        &scene.entered,
        &scene.statics.effects(),
        &scene.statics.visuals,
        costume_item,
    ) else {
        return Ok(false);
    };

    let actor = &scene.world.actors.actors[index];

    let arena = actor
        .details
        .iter()
        .find(|d| d.kind == b'2')
        .filter(|d| d.level != 0 && d.level < 20)
        .and_then(|d| {
            scene
                .world
                .actors
                .registries
                .arena_visual
                .get(usize::from(d.level))
        });

    if let Some(row) = arena {
        let Some(look) = Look::for_field_with_arena(
            &scene.entered.equipment,
            &scene.statics.effects(),
            &scene.statics.visuals,
            scene.entered.wearer,
            costume_item,
            Some((row, actor.class)),
        ) else {
            return Ok(false);
        };

        doll.look = look;
    }

    crate::game_settings::Interface::project(scene, &mut doll);

    scene.entered.kingdom = doll.look.kingdom;

    let actor = &scene.world.actors.actors[index];

    doll.position = actor.position;

    doll.yaw = actor.yaw;

    doll.affects = actor.affects;

    doll.status = actor.status;

    doll.shop_title = actor.shop_title.clone();

    let redressed = scene
        .world
        .actors
        .redress(game, index, &doll)
        .context("re-dressing the local player")?;

    if redressed {
        settle(scene, game, index, true)?;
    }

    Ok(redressed)
}

pub fn describe_hp_delta(applied: &HpChanged) -> String {
    match applied.index {
        None => {
            format!(
                "NWYD: 0x18A for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        Some(_) => {
            format!(
                "NWYD: 0x18A — entity {} at {} hp, delta {}{}{}",
                applied.entity,
                applied.hp,
                applied.delta,
                match applied.local {
                    true => " (the player)",
                    false => "",
                },
                match applied.raised {
                    Some(kind) => format!(", number kind {kind}"),
                    None => ", no number".to_string(),
                },
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sparked {
    pub entity: u16,
    pub index: Option<usize>,
    pub lit: usize,
    pub forced: bool,
    pub raised: bool,
}

pub fn firework(scene: &mut FieldScene, entity: u16, glyph: &Glyph) -> Sparked {
    let (lit, forced) = (glyph.cells.iter().filter(|c| **c).count(), glyph.forced());

    let Some(index) = scene.world.actors.entity(entity) else {
        return Sparked {
            entity,
            index: None,
            lit,
            forced,
            raised: false,
        };
    };

    let now_ms = scene.time.now_ms;

    let body = scene.world.actors.actors[index].position;

    let at = glam::Vec3::new(body.x, body.y + super::firework::ANCHOR_LIFT, body.z);

    let mut shell = super::firework::Firework::new(
        at,
        super::firework::GLYPH_PATTERN,
        now_ms,
        &mut scene.world.actors.effect_rng,
    );

    shell.seed_glyph(glyph);

    scene.world.effects.spawn(Box::new(shell));

    scene.world.play_sound(super::firework::LAUNCH_SOUND);

    Sparked {
        entity,
        index: Some(index),
        lit,
        forced,
        raised: true,
    }
}

pub fn describe_firework(applied: &Sparked) -> String {
    match applied.index {
        None => {
            format!(
                "NWYD: 0x3CA for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        Some(_) => {
            format!(
                "NWYD: 0x3CA — a firework over entity {}, {} of {} cells lit{}",
                applied.entity,
                applied.lit,
                formats::net::firework::CELLS,
                match applied.forced {
                    true => " (an empty mask, so one cell is forced on)",
                    false => "",
                },
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct NameMarked {
    pub entity: u16,
    pub index: Option<usize>,
    pub mode: i32,
    pub set: bool,
}

pub fn name_mark(scene: &mut FieldScene, entity: u16, mode: i32) -> NameMarked {
    use formats::net::body_mark::NAME_MARK_GREEN;

    let Some(index) = scene.world.actors.entity(entity) else {
        return NameMarked {
            entity,
            index: None,
            mode,
            set: false,
        };
    };

    let actor = &mut scene.world.actors.actors[index];

    let before = actor.green_name;

    actor.green_name = mode == NAME_MARK_GREEN;

    if mode == NAME_MARK_GREEN {
        actor.plate_backdrop_color = 0x5500_AA00;

        actor.plate_outline = true;
    }

    NameMarked {
        entity,
        index: Some(index),
        mode,
        set: actor.green_name != before,
    }
}

pub fn describe_name_mark(applied: &NameMarked) -> String {
    match applied.index {
        None => {
            format!(
                "NWYD: 0x193 for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        Some(_) => {
            format!(
                "NWYD: 0x193 — entity {}, mode {}{}",
                applied.entity,
                applied.mode,
                match (
                    applied.mode == formats::net::body_mark::NAME_MARK_GREEN,
                    applied.set
                ) {
                    (true, true) => ", green name set",
                    (true, false) => ", green name was already set",
                    (false, true) => ", green name cleared",
                    (false, false) => ", green name was already clear",
                },
            )
        }
    }
}

pub fn describe_vitals(applied: &Vitalised) -> String {
    match applied.index {
        None => {
            format!(
                "NWYD: 0x181 for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        Some(_) => {
            format!(
                "NWYD: 0x181 — entity {} at {} hp, {} mp{}{}{}",
                applied.entity,
                applied.hp,
                applied.mp,
                match applied.local {
                    true => " (the player)",
                    false => "",
                },
                match applied.dead {
                    true => ", dead",
                    false => "",
                },
                match (applied.revived, applied.dropped_target) {
                    (true, true) => ", revived, target dropped",
                    (true, false) => ", revived",
                    (false, true) => ", target dropped",
                    (false, false) => "",
                },
            )
        }
    }
}

pub fn describe_actor_status(applied: &StatusApplied) -> String {
    match applied.index {
        None => {
            format!(
                "NWYD: 0x336 for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        Some(_) => {
            format!(
                "NWYD: 0x336 — entity {} level {}, {}/{} hp, {} affect pair(s) adopted{}{}{}",
                applied.entity,
                applied.level,
                applied.hp,
                applied.max_hp,
                applied.adopted,
                match applied.local {
                    true => " (the player)",
                    false => "",
                },
                match applied.invisible {
                    true => ", invisible",
                    false => "",
                },
                match (applied.revived, applied.died) {
                    (true, _) => ", revived",
                    (_, true) => ", died",
                    _ => "",
                },
            )
        }
    }
}

pub fn describe_player_progression(applied: &ProgressionApplied) -> String {
    match applied.local {
        false => {
            format!(
                "NWYD: 0x337 for entity {} — declined, the update applies to the local player only",
                applied.entity,
            )
        }
        true => {
            format!(
                "NWYD: 0x337 — {} exp, {} gold{}{}{}",
                applied.exp,
                applied.gold,
                match applied.cp {
                    true => ", and CP in hand",
                    false => "",
                },
                match applied.mask_changed {
                    true => ", and the skill mask moved",
                    false => "",
                },
                match applied.blanked.is_empty() {
                    true => String::new(),
                    false => format!(
                        ", sweep blanked quick slot(s) {:?} and sent a 0x378",
                        applied.blanked,
                    ),
                },
            )
        }
    }
}

pub fn describe_quick_bar(applied: &PushedBar) -> String {
    format!(
        "NWYD: 0x378 quick-slot bar — {} of {} filled, {}{}",
        applied.bar.iter().filter(|b| **b != QUICK_EMPTY).count(),
        QUICK_SLOTS,
        match applied.changed {
            true => "replaced",
            false => "restated unchanged",
        },
        match applied.repainted {
            true => "",
            false => ", with no HUD up to repaint",
        },
    )
}

pub fn describe_gold(applied: &Golden) -> String {
    match applied.local {
        false => {
            format!(
                "NWYD: 0x3AF for entity {} — declined, the update applies to the local player only",
                applied.entity,
            )
        }
        true => format!("NWYD: 0x3AF — {} gold", applied.gold),
    }
}

pub fn describe_equip(applied: &Redressed) -> String {
    match &applied.reason {
        Some(reason) => format!("NWYD: 0x36B for entity {} — {reason}", applied.entity),
        None => {
            format!(
                "NWYD: 0x36B — entity {} re-dressed, body {}{}{}",
                applied.entity,
                applied.body,
                match applied.local {
                    true => " (the player)",
                    false => "",
                },
                match (applied.silent, &applied.raised) {
                    (true, _) => ", silhouette unchanged so none of the four objects".to_string(),
                    (false, Some(raised)) => format!(
                        ", {} effect node(s), {} shade(s), sound {:#04X}{}",
                        raised.nodes,
                        raised.shades,
                        super::burst::EQUIP_SOUND,
                        match &raised.level_up {
                            Some(up) => format!(
                                ", of which a EffectLevelUp kind {} is {}",
                                up.kind, up.nodes
                            ),
                            None => ", and no EffectLevelUp — the body is GM-hidden".to_string(),
                        },
                    ),
                    (false, None) => String::new(),
                },
            )
        }
    }
}

pub fn describe_patch(applied: &Patched) -> String {
    let where_ = match applied.container {
        Container::Equipment => "equipment",
        Container::Carry => "the backpack",
        Container::Cargo => "the warehouse",
        Container::Other(_) => "a container with no arm",
    };

    match (applied.local, applied.written) {
        (false, _) => {
            format!(
                "NWYD: 0x182 for entity {} — declined, this port patches the local player only (D-180)",
                applied.entity,
            )
        }
        (true, false) => {
            format!(
                "NWYD: 0x182 — slot {} of {where_} is outside it, nothing written",
                applied.slot,
            )
        }
        (true, true) => {
            format!(
                "NWYD: 0x182 — slot {} of {where_} patched{}",
                applied.slot,
                match applied.redressed {
                    true => ", body re-dressed",
                    false => "",
                },
            )
        }
    }
}

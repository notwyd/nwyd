use super::attack::{TEXT_ANCHOR_LIFT, TEXT_DROP, project_text};
use super::damage::{self, Number};
use super::field::FieldScene;
use super::minipopup::{self, Notice};
use formats::mesh::appearance;
use formats::net::account::Item;
use formats::net::affect::Flags;
use formats::net::kill::Kill;

pub const DEATH_CLEARED: [(usize, usize); 5] =
    [(0xB2C, 4), (0xB31, 1), (0xB34, 4), (0xB38, 4), (0xB3C, 1)];

pub const KILLER_MEMO_ITEM: u16 = 0x0D87;

pub const KILLER_MEMO_SLOTS: usize = 60;

pub fn killer_memo_enabled(carry: &[Item]) -> bool {
    carry
        .iter()
        .take(KILLER_MEMO_SLOTS)
        .any(|item| item.index == KILLER_MEMO_ITEM)
}

pub fn killer_memo_line(calendar: &crate::clock::Calendar, killer: &str) -> String {
    format!(
        "[{:02}:{:02}:{:02}] Killer[{killer}] ",
        calendar.hour, calendar.minute, calendar.second,
    )
}

#[derive(Debug, Clone, PartialEq)]

pub struct Killed {
    pub victim: u16,
    pub victim_index: Option<usize>,
    pub killer: u16,
    pub killer_index: Option<usize>,
    pub mine: bool,
    pub credited: bool,
    pub fell: bool,
    pub exp_gained: u64,
    pub cp_lost: i64,
    pub raised: usize,
    pub dropped_target: bool,
    pub charged: bool,
    pub memo: Option<String>,
}

pub fn apply(scene: &mut FieldScene, kill: &Kill, now_ms: u32) -> Killed {
    let killer_index = scene.world.actors.entity(kill.killer);

    let victim_index = scene.world.actors.entity(kill.victim);

    let mine = killer_index.is_some() && killer_index == scene.player;

    let credited =
        mine || killer_index.is_some_and(|index| scene.world.actors.actors[index].tracked);

    let mut applied = Killed {
        victim: kill.victim,
        victim_index,
        killer: kill.killer,
        killer_index,
        mine,
        credited,
        fell: false,
        exp_gained: 0,
        cp_lost: 0,
        raised: 0,
        dropped_target: false,
        charged: false,
        memo: None,
    };

    if killer_index.is_some() {
        applied.charged = charge(scene, killer_index, now_ms);
    }

    if killer_index.is_some() && credited {
        paid(scene, kill, now_ms, &mut applied);
    }

    if killer_index.is_some() && mine {
        if scene.attack.auto && scene.attack.auto_target.is_some() {
            scene.attack.auto_target = None;

            applied.dropped_target = true;
        }
    }

    if let Some(index) = victim_index {
        scene.world.actors.actors[index].status.hp = 0;

        applied.fell = die(scene, index, now_ms);

        if scene.khepra == Some(kill.victim) {
            scene.khepra = None;
        }

        if killer_index.is_some()
            && scene.player == Some(index)
            && killer_memo_enabled(&scene.entered.carry)
        {
            let killer_name = scene.world.actors.actors[killer_index.expect("checked")]
                .name
                .clone();

            let line = killer_memo_line(&scene.time.calendar, &killer_name);

            if let Some(panels) = scene.panels.as_mut() {
                panels.append_memo(line.clone());

                minipopup::set(panels, Notice::Memo, true);
            }

            applied.memo = Some(line);
        }
    }

    applied
}

fn charge(scene: &mut FieldScene, killer: Option<usize>, now_ms: u32) -> bool {
    let Some(index) = killer else {
        return false;
    };

    let Some(body) = scene.world.actors.actors.get(index) else {
        return false;
    };

    let owner = super::motion::Owner {
        position: body.position,
        seat: body.mounted.then_some(body.skill_anchor).flatten(),
    };

    let mut charge =
        super::mesh_fx::Charge::new(Some(owner), KILL_CHARGE_MODE, KILL_CHARGE_ARGB, now_ms);

    charge.owner_key = Some(super::effect_context::ActorKey::of(body));

    scene.world.effects.spawn(Box::new(charge));

    true
}

pub const KILL_CHARGE_MODE: i32 = 0;

pub const KILL_CHARGE_ARGB: u32 = 0xFFFF_FFFF;

pub fn die(scene: &mut FieldScene, index: usize, now_ms: u32) -> bool {
    if scene.world.actors.actors[index].is_dead {
        return false;
    }

    if scene.player == Some(index) {
        scene.death_return_birth_ms = now_ms;
    }

    let actor = &mut scene.world.actors.actors[index];

    actor.green_name = false;

    actor.is_dead = true;

    actor.projectile_muzzle = actor.position + glam::Vec3::Y * 1.5;

    actor.body_points.points[0] = Some(actor.projectile_muzzle);

    actor.mover.flatten(actor.mover.cursor, 0.0);

    scene
        .world
        .actors
        .play_at(index, appearance::ACTION_DIE, now_ms);

    let raised = super::burst::death(scene, index);

    if raised.refused.is_none() {
        tracing::info!("{}", super::burst::describe(&raised));
    }

    let actor = &mut scene.world.actors.actors[index];

    for (offset, len) in DEATH_CLEARED {
        for byte in 0..len {
            actor.flags.set(offset + byte, 0);
        }
    }

    actor.flags.set(Flags::LAST, 0);

    let flags = actor.flags;

    scene.world.actors.apply_affects(index, &flags);

    super::siege_effects::death(scene, index, now_ms);

    true
}

fn paid(scene: &mut FieldScene, kill: &Kill, now_ms: u32, applied: &mut Killed) {
    let gained = i128::from(kill.exp) - i128::from(scene.entered.exp);

    let lost = i64::from(scene.entered.cp) - i64::from(kill.cp);

    if gained == 0 && lost == 0 {
        return;
    }

    scene.entered.exp = kill.exp;

    scene.entered.cp = u32::try_from(kill.cp).unwrap_or(0);

    if let Some(player) = scene.player {
        scene.world.actors.actors[player].status = scene.entered.status;
    }

    if gained > 0 {
        applied.exp_gained = u64::try_from(gained).unwrap_or(0);

        let raised = float(
            scene,
            &format!("Exp +{}", applied.exp_gained),
            damage::EXP_LIFE_MS,
            now_ms,
        );

        applied.raised += raised;

        if raised != 0 {
            reward_notice(scene, 148, applied.exp_gained as u32 as i32);
        }
    }

    if lost > 0 {
        applied.cp_lost = lost;

        let raised = float(scene, &format!("Cp -{lost}"), damage::LIFE_MS, now_ms);

        applied.raised += raised;

        if raised != 0 {
            reward_notice(scene, 305, lost as i32);
        }
    }
}

fn reward_notice(scene: &mut FieldScene, record: usize, value: i32) {
    let template = scene
        .message_strings
        .strdef
        .as_ref()
        .map_or("", |s| s.get(record));

    let text = template
        .replacen("%d", &value.to_string(), 1)
        .replace("%%", "%");

    scene.long_message.reward(text);
}

fn float(scene: &mut FieldScene, text: &str, life_ms: u32, now_ms: u32) -> usize {
    let Some(player) = scene.player else {
        return 0;
    };

    let body = &scene.world.actors.actors[player];

    let at = [
        body.position.x,
        body.position.y + TEXT_ANCHOR_LIFT,
        body.position.z,
    ];

    let Some(screen) = project_text(scene, at) else {
        return 0;
    };

    let drop = (scene.layout.sy * TEXT_DROP).trunc();

    let items = damage::set_of(damage::KIND_GREEN)
        .and_then(|set| damage::set_items(&scene.sets, set))
        .unwrap_or(&[]);

    let number = Number::new(
        text,
        (screen.0, (screen.1 + drop).trunc()),
        damage::KIND_GREEN,
        0,
        life_ms,
        now_ms,
        items,
        &scene.layout,
    );

    scene.damage.push(number);

    1
}

pub fn describe(applied: &Killed) -> String {
    let mut line = format!(
        "NWYD: 0x338 — entity {} killed entity {}",
        applied.killer, applied.victim
    );

    if applied.killer_index.is_none() {
        line.push_str(" (no body in this world carries the killer's id)");
    }

    match applied.victim_index {
        None => line.push_str(", and none carries the victim's"),
        Some(_) => line.push_str(match applied.fell {
            true => ", which fell over",
            false => ", which was already down",
        }),
    }

    if applied.credited {
        line.push_str(&format!(
            " — +{} exp, -{} cp, {} number(s)",
            applied.exp_gained, applied.cp_lost, applied.raised,
        ));
    }

    if applied.dropped_target {
        line.push_str(", and the auto-attack let go");
    }

    if applied.charged {}

    line
}

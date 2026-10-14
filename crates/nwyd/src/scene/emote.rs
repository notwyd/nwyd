use super::field::FieldScene;
use formats::mesh::appearance;
use formats::net::emote::{self, Emote};
use glam::Vec3;

pub const SILENT_CLASSES: [i32; 2] = [0x24, 0x25];

pub const COIN_CLASS: i32 = 0x27;

pub const BARE_CLASS: i32 = 0x38;

pub const BARE_MESH: u16 = 0;

pub const COIN_SUBSTITUTE: i16 = appearance::ACTION_CACKLE as i16;

pub const MODE1_MODELS: [u32; 4] = [0, 1, 0x15, 3];

pub const BANK_MODELS: [u32; 5] = [3, 8, 7, 0x19, 0x1C];

pub const BANK_ID_LIMIT: u16 = 1000;

pub const BANK_SHIFT: i16 = 0x0E;

pub const COIN_SOUND: i64 = 0x10B;

pub const TAIL_SOUND: i64 = 0x12C;

pub const TAIL_SOUND_CLASS: i32 = 4;

pub const TAIL_SOUND_MESH: u16 = 0xF;

pub const VISIBLE_IN_FIELD: bool = false;

pub const FIREWORK_LIFT: f32 = super::firework::ANCHOR_LIFT;

pub const LEVEL_UP_KIND: i32 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Emoted {
    pub entity: u16,
    pub index: Option<usize>,
    pub code: i16,
    pub mode: i16,
    pub outcome: Outcome,
    pub played: Option<usize>,
    pub substituted: bool,
    pub banked: Option<i16>,
    pub undied: bool,
    pub level_up: bool,
    pub latch_cleared: bool,
    pub sound: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Outcome {
    NoBody,
    Firework { pattern: i32 },
    ClassRefused,
    ModelRefused,
    CodeRefused,
    BankVoided,
    NegativeCode,
    Played,
}

pub fn coin_sound(visible: bool) -> Option<i64> {
    visible.then_some(COIN_SOUND)
}

pub fn tail_sound(class: i32, part0_mesh: i16, visible: bool) -> Option<i64> {
    let matched = class == TAIL_SOUND_CLASS && part0_mesh as u16 == TAIL_SOUND_MESH;

    (matched && visible).then_some(TAIL_SOUND)
}

pub fn bank(entity: u16, model_id: u32, code: i16) -> Option<i16> {
    let admitted = code == 0x0D || (0x0F..=0x18).contains(&code);

    let banked = entity < BANK_ID_LIMIT && BANK_MODELS.contains(&model_id) && admitted;

    banked.then(|| code - BANK_SHIFT)
}

pub fn emote(scene: &mut FieldScene, entity: u16, packet: &Emote) -> Emoted {
    let (code, mode) = (packet.code, packet.mode);

    let blank = |index, outcome| Emoted {
        entity,
        index,
        code,
        mode,
        outcome,
        played: None,
        substituted: false,
        banked: None,
        undied: false,
        level_up: false,
        latch_cleared: false,
        sound: None,
    };

    let Some(index) = scene.world.actors.entity(entity) else {
        return blank(None, Outcome::NoBody);
    };

    let mut applied = blank(Some(index), Outcome::Played);

    let now_ms = scene.time.now_ms;

    let body = scene.world.actors.actors[index].position;

    if code == emote::FIREWORK {
        let pattern = mode as i32;

        let at = Vec3::new(body.x, body.y + FIREWORK_LIFT, body.z);

        let shell =
            super::firework::Firework::new(at, pattern, now_ms, &mut scene.world.actors.effect_rng);

        scene.world.effects.spawn(Box::new(shell));

        scene.world.play_sound(super::firework::LAUNCH_SOUND);

        applied.outcome = Outcome::Firework { pattern };

        return applied;
    }

    if mode == emote::MODE_APPEARANCE {
        ladder(scene, index, &mut applied);

        return applied;
    }

    if mode == emote::MODE_UNDIE {
        scene.world.actors.actors[index].is_dead = false;

        applied.undied = true;
    }

    if mode == emote::MODE_LEVEL_UP {
        super::timers::spawn(scene, Vec3::new(body.x, body.y, body.z), LEVEL_UP_KIND);

        applied.level_up = true;
    }

    if code >= emote::CODE_LIMIT {
        applied.outcome = Outcome::CodeRefused;

        return applied;
    }

    if Some(index) == scene.player {
        scene.world.actors.actors[index].emote_latch = None;

        applied.latch_cleared = true;
    }

    let actor = &scene.world.actors.actors[index];

    let played = match bank(actor.entity, actor.model_id, code) {
        Some(banked) => {
            applied.banked = Some(banked);

            banked
        }
        None => code,
    };

    play(scene, index, played, &mut applied);

    applied
}

fn ladder(scene: &mut FieldScene, index: usize, applied: &mut Emoted) {
    let actor = &scene.world.actors.actors[index];

    let (class, part0) = (actor.class, actor.parts.first().map_or(0, |p| p.mesh));

    if SILENT_CLASSES.contains(&class) {
        applied.outcome = Outcome::ClassRefused;

        return;
    }

    if class == COIN_CLASS {
        let even = scene.world.actors.effect_rng.next() & 1 == 0;

        if even {
            applied.substituted = true;

            applied.code = COIN_SUBSTITUTE;

            applied.sound = coin_sound(VISIBLE_IN_FIELD);
        }

        let code = applied.code;

        play(scene, index, code, applied);

        return;
    }

    if class == BARE_CLASS && part0 as u16 == BARE_MESH {
        let code = applied.code;

        play(scene, index, code, applied);

        return;
    }

    if !MODE1_MODELS.contains(&scene.world.actors.actors[index].model_id) {
        applied.outcome = Outcome::ModelRefused;

        return;
    }

    let code = applied.code;

    play(scene, index, code, applied);

    applied.sound = tail_sound(class, part0, VISIBLE_IN_FIELD);
}

fn play(scene: &mut FieldScene, index: usize, code: i16, applied: &mut Emoted) {
    if code == -1 {
        applied.outcome = Outcome::BankVoided;

        return;
    }

    if code < 0 {
        applied.outcome = Outcome::NegativeCode;

        return;
    }

    let now_ms = scene.time.now_ms;

    scene.world.actors.play_at(index, code as usize, now_ms);

    applied.played = Some(code as usize);

    applied.outcome = Outcome::Played;
}

pub fn describe(applied: &Emoted) -> String {
    let Some(_) = applied.index else {
        return format!(
            "NWYD: 0x36A for entity {} — no body in this world carries that id",
            applied.entity,
        );
    };

    let what = match applied.outcome {
        Outcome::NoBody => unreachable!("index is Some"),
        Outcome::Firework { pattern } => format!("a firework on pattern {pattern}"),
        Outcome::ClassRefused => "nothing — the appearance class answers mode 1 in silence".into(),
        Outcome::ModelRefused => "nothing — the model is outside mode 1's whitelist".into(),
        Outcome::CodeRefused => {
            format!("nothing — code {} is at or past 0x100", applied.code)
        }
        Outcome::BankVoided => "nothing — the alternate bank turned the code into -1".into(),
        Outcome::NegativeCode => format!("nothing — code {} is negative", applied.code),
        Outcome::Played => match applied.played {
            Some(action) => format!("action {action}"),
            None => "nothing".into(),
        },
    };

    let mut line = format!("NWYD: 0x36A — entity {} plays {what}", applied.entity);

    if applied.substituted {
        line.push_str(", the class-0x27 coin came up even so the code became CACKLE");
    }

    if let Some(banked) = applied.banked {
        line.push_str(&format!(", through the alternate bank as {banked}"));
    }

    if applied.undied {
        line.push_str(", and stood back up");
    }

    if applied.level_up {
        line.push_str(", and a level-up burst");
    }

    if applied.latch_cleared {
        line.push_str(", releasing our own emote latch");
    }

    if let Some(slot) = applied.sound {
        line.push_str(&format!(", sound slot {slot}"));
    }

    line
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Keys {
    pub floor_ms: u32,
    pub swallow_next: bool,
}

pub const VK_NUMPAD0: u8 = 0x60;

pub const VK_NUMPAD9: u8 = 0x69;

pub const CODE_BIAS: i16 = 0x51;

pub const FLOOR_MS: u32 = 500;

pub const CARET_EDITABLE_TEXT: i32 = 0x0D;

pub const SWALLOWING_SCHEME: i16 = 0;

pub const HELD_ACTIONS: [i32; 4] = [0x0D, 0x0F, 0x19, 0x1B];

pub const ACTION_SEATED: i32 = 0x18;

pub const ACTION_PUNISHING: i32 = 0x1A;

pub const CODE_RELEASE: i16 = 0x1B;

pub const CODE_SEAT: i16 = 0x0D;

pub const CODE_PUNISH: i16 = 0x0F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Refusal {
    Editing,
    Mounted,
    Dead,
    TooSoon { remaining_ms: u32 },
    Latched(i16),
    Held(i32),
    Toggled(i32),
    NoPlayer,
    Locked,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Press {
    Unclaimed,
    Refused(Refusal),
    Sent { code: i16 },
}

pub fn caret_swallows(caret: Option<i32>, control_scheme: i16) -> bool {
    caret == Some(CARET_EDITABLE_TEXT) && control_scheme == SWALLOWING_SCHEME
}

pub fn too_soon(now_ms: u32, floor_ms: u32) -> bool {
    now_ms < floor_ms.wrapping_add(FLOOR_MS)
}

pub fn code_for(vk: u8, action: i32) -> Option<i16> {
    let provisional = vk as i16 - CODE_BIAS;

    match vk {
        VK_NUMPAD9 => match action {
            ACTION_SEATED => Some(CODE_RELEASE),
            ACTION_PUNISHING => None,
            _ => Some(CODE_SEAT),
        },
        VK_NUMPAD0 => match action {
            ACTION_PUNISHING => Some(CODE_RELEASE),
            ACTION_SEATED => None,
            _ => Some(CODE_PUNISH),
        },
        _ => Some(provisional),
    }
}

pub fn outbound(code: i16) -> Vec<u8> {
    let mut out = vec![0u8; emote::FRAME_LEN];

    protocol::header::set_opcode(&mut out, emote::OPCODE);

    out[protocol::header::LEN..].copy_from_slice(&emote::Emote::outbound(code).to_bytes());

    out
}

pub fn press(scene: &mut FieldScene, vk: u8, caret: Option<i32>, control_scheme: i16) -> Press {
    if !(VK_NUMPAD0..=VK_NUMPAD9).contains(&vk) {
        return Press::Unclaimed;
    }

    let now_ms = scene.time.now_ms;

    if caret_swallows(caret, control_scheme) {
        return Press::Refused(Refusal::Editing);
    }

    scene.emote_keys.swallow_next = true;

    let Some(index) = scene.player else {
        return Press::Refused(Refusal::NoPlayer);
    };

    let actor = &scene.world.actors.actors[index];

    if actor.mounted {
        return Press::Refused(Refusal::Mounted);
    }

    if actor.is_dead {
        return Press::Refused(Refusal::Dead);
    }

    if too_soon(now_ms, scene.emote_keys.floor_ms) {
        let remaining = scene
            .emote_keys
            .floor_ms
            .wrapping_add(FLOOR_MS)
            .wrapping_sub(now_ms);

        return Press::Refused(Refusal::TooSoon {
            remaining_ms: remaining,
        });
    }

    if let Some(pending) = actor.emote_latch {
        return Press::Refused(Refusal::Latched(pending));
    }

    let action = actor.action as i32;

    if HELD_ACTIONS.contains(&action) {
        return Press::Refused(Refusal::Held(action));
    }

    let Some(code) = code_for(vk, action) else {
        return Press::Refused(Refusal::Toggled(action));
    };

    scene.world.actors.actors[index].emote_latch = Some(code);

    scene.outbox.push(outbound(code));

    scene.emote_keys.floor_ms = now_ms;

    Press::Sent { code }
}

pub const SKILL_BAR_CHARS: [char; 20] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '!', '@', '#', '$', '%', '^', '&', '*', '(',
    ')',
];

pub fn describe_press(vk: u8, press: &Press) -> Option<String> {
    let what = match press {
        Press::Unclaimed => return None,
        Press::Sent { code } => format!("sent action {code} and latched it"),
        Press::Refused(Refusal::Editing) => {
            "swallowed — an editable control holds the caret".into()
        }
        Press::Refused(Refusal::Mounted) => "swallowed — mounted".into(),
        Press::Refused(Refusal::Dead) => "swallowed — dead".into(),
        Press::Refused(Refusal::TooSoon { remaining_ms }) => {
            format!("swallowed — {remaining_ms} ms left of the shared 500 ms floor")
        }
        Press::Refused(Refusal::Latched(code)) => {
            format!("swallowed — action {code} is still latched, awaiting its 0x36A back")
        }
        Press::Refused(Refusal::Held(action)) => {
            format!("swallowed — the body is holding action {action}")
        }
        Press::Refused(Refusal::Toggled(action)) => {
            format!("swallowed — the toggle refuses action {action} on this key")
        }
        Press::Refused(Refusal::NoPlayer) => "swallowed — no local player".into(),
        Press::Refused(Refusal::Locked) => "swallowed — the six-second input lock is up".into(),
    };

    Some(format!("NWYD: 0x36A — numpad {:#04x} {what}", vk))
}

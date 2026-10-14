use super::attack::{attribute_at, cell_of};
use super::field::FieldScene;

pub const ATTRIBUTE_BIT: u8 = formats::world::attribute_map::flags::TELEPORT_GATE;

pub const CONTEXT: u32 = 0x3C;

pub const ASK_NAMED_STRDEF: usize = 208;

pub const ASK_UNNAMED_STRDEF: usize = 43;

pub const FEE_STRDEF: usize = 207;

pub const OFF_GATE_STRDEF: usize = 38;

pub const WATER_SCROLL_STRDEF: usize = 160;

pub const CUBE_INVITE_STRDEF: usize = 258;

pub const TOAST_MS: u32 = 0xBB8;

pub const USE_TILE: u16 = 0x290;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Gate {
    pub x: i32,
    pub z: i32,
    pub fee: i32,
    pub name: usize,
}

pub const GATE_MASK: i32 = !3;

const SECTOR_SHIFT: u32 = 6;

pub const GATES: [Gate; 37] = [
    Gate {
        x: 2116,
        z: 2100,
        fee: 700,
        name: 209,
    },
    Gate {
        x: 2480,
        z: 1716,
        fee: 700,
        name: 209,
    },
    Gate {
        x: 2456,
        z: 2016,
        fee: 700,
        name: 209,
    },
    Gate {
        x: 3648,
        z: 3108,
        fee: 700,
        name: 209,
    },
    Gate {
        x: 1044,
        z: 1724,
        fee: 0,
        name: 125,
    },
    Gate {
        x: 1044,
        z: 1716,
        fee: 0,
        name: 126,
    },
    Gate {
        x: 1044,
        z: 1708,
        fee: 0,
        name: 173,
    },
    Gate {
        x: 1048,
        z: 1764,
        fee: 0,
        name: 210,
    },
    Gate {
        x: 2140,
        z: 2068,
        fee: 0,
        name: 213,
    },
    Gate {
        x: 2468,
        z: 1716,
        fee: 0,
        name: 211,
    },
    Gate {
        x: 2364,
        z: 2284,
        fee: 0,
        name: 212,
    },
    Gate {
        x: 144,
        z: 3788,
        fee: 0,
        name: 213,
    },
    Gate {
        x: 2668,
        z: 2156,
        fee: 0,
        name: 212,
    },
    Gate {
        x: 144,
        z: 3772,
        fee: 0,
        name: 213,
    },
    Gate {
        x: 148,
        z: 3780,
        fee: 0,
        name: 215,
    },
    Gate {
        x: 144,
        z: 3780,
        fee: 0,
        name: 215,
    },
    Gate {
        x: 1004,
        z: 4028,
        fee: 0,
        name: 214,
    },
    Gate {
        x: 408,
        z: 4072,
        fee: 0,
        name: 215,
    },
    Gate {
        x: 1004,
        z: 4064,
        fee: 0,
        name: 214,
    },
    Gate {
        x: 744,
        z: 3820,
        fee: 0,
        name: 215,
    },
    Gate {
        x: 1004,
        z: 3992,
        fee: 0,
        name: 214,
    },
    Gate {
        x: 680,
        z: 4076,
        fee: 0,
        name: 216,
    },
    Gate {
        x: 916,
        z: 3820,
        fee: 0,
        name: 215,
    },
    Gate {
        x: 876,
        z: 3872,
        fee: 0,
        name: 216,
    },
    Gate {
        x: 932,
        z: 3820,
        fee: 0,
        name: 215,
    },
    Gate {
        x: 188,
        z: 188,
        fee: 0,
        name: 126,
    },
    Gate {
        x: 2548,
        z: 1740,
        fee: 1000,
        name: 322,
    },
    Gate {
        x: 1824,
        z: 1772,
        fee: 0,
        name: 218,
    },
    Gate {
        x: 1172,
        z: 4080,
        fee: 0,
        name: 211,
    },
    Gate {
        x: 1516,
        z: 3996,
        fee: 0,
        name: 220,
    },
    Gate {
        x: 1304,
        z: 3816,
        fee: 0,
        name: 219,
    },
    Gate {
        x: 2452,
        z: 1716,
        fee: 0,
        name: 211,
    },
    Gate {
        x: 2452,
        z: 1988,
        fee: 0,
        name: 211,
    },
    Gate {
        x: 3648,
        z: 3140,
        fee: 700,
        name: 211,
    },
    Gate {
        x: 2480,
        z: 1648,
        fee: 700,
        name: 301,
    },
    Gate {
        x: 1052,
        z: 1708,
        fee: 700,
        name: 321,
    },
    Gate {
        x: 1056,
        z: 1724,
        fee: 0,
        name: 485,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct State {
    pub last_attribute: u8,
    pub last_gate: i32,
    pub prompted: bool,
    pub(super) answer_pressed: bool,
}

impl State {
    pub fn new() -> State {
        State {
            last_attribute: 0,
            last_gate: NO_GATE,
            prompted: false,
            answer_pressed: false,
        }
    }
}

pub const NO_GATE: i32 = -1;

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Step {
    Nothing,
    Asked {
        slot: usize,
        gate: Option<usize>,
        fee: i32,
    },
    Toast {
        record: usize,
    },
    Dropped,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Accepted {
    Sent,
    OffGate,
}

pub fn gate_at(cell: (i32, i32)) -> Option<usize> {
    let (x, z) = (cell.0 & GATE_MASK, cell.1 & GATE_MASK);

    GATES.iter().position(|gate| gate.x == x && gate.z == z)
}

pub fn rectangle_toast(cell: (i32, i32)) -> Option<usize> {
    let (x, z) = cell;

    if 0x7AB < x && x < 0x7B2 && 0x6EA < z && z < 0x6F1 {
        return Some(WATER_SCROLL_STRDEF);
    }

    if 0x941 < x && x < 0x96C && 0x6BF < z && z < 0x6E0 {
        return Some(CUBE_INVITE_STRDEF);
    }

    None
}

pub fn suppressed(cell: (i32, i32), armed: bool) -> bool {
    armed && (2357..=2376).contains(&cell.0) && (3917..=3936).contains(&cell.1)
}

pub fn clear_latch(state: &mut State, personal_shop_open: bool) {
    if std::mem::take(&mut state.answer_pressed) || personal_shop_open {
        return;
    }

    state.prompted = false;
}

pub fn sector_dedup(state: &State, gate: Option<usize>, attribute: u8) -> bool {
    let previous = usize::try_from(state.last_gate)
        .ok()
        .and_then(|i| GATES.get(i));

    let (Some(previous), Some(current)) = (previous, gate.and_then(|i| GATES.get(i))) else {
        return false;
    };

    let sector = |v: i32| v >> SECTOR_SHIFT;

    let moved = sector(current.x) != sector(previous.x) || sector(current.z) != sector(previous.z);

    moved && state.last_attribute == attribute
}

pub fn question(strings: &ui::Strings, gate: Option<usize>, fee: i32) -> String {
    let record = |n: usize| strings.translate(&format!("ss_{n}"));

    let Some(gate) = gate.and_then(|i| GATES.get(i)) else {
        return record(ASK_UNNAMED_STRDEF);
    };

    let mut text = record(ASK_NAMED_STRDEF).replacen("%s", &record(gate.name), 1);

    if fee > 0 {
        text.push_str(ui::text::FF_BREAK);

        text.push_str(&record(FEE_STRDEF).replacen("%d", &fee.to_string(), 1));
    }

    text
}

pub fn quotes_fee(zone_kingdom: u8, player_kingdom: u8) -> bool {
    match zone_kingdom {
        0 => player_kingdom != 0 && player_kingdom != 3,
        7 => player_kingdom != 1,
        8 => player_kingdom != 2,
        0xFF => true,
        _ => false,
    }
}

pub fn quoted_fee(fee: i32, level: u16, mask: u32) -> i32 {
    if fee != REPRICED_FEE {
        return fee;
    }

    if i32::from(level) % 10 <= 5 {
        return 1_000;
    }

    match mask & LEVEL_DIGIT_MASK_BIT != 0 {
        true => 3_000,
        false => 2_000,
    }
}

pub const REPRICED_FEE: i32 = 700;

pub const LEVEL_DIGIT_MASK_BIT: u32 = 0x4000_0000;

pub fn tick(scene: &mut FieldScene) -> Step {
    let Some(player) = scene.player else {
        return Step::Nothing;
    };

    let actor = &scene.world.actors.actors[player];

    let cell = cell_of(actor);

    let attribute = attribute_at(scene.world.actors.attributes(), cell).unwrap_or(0);

    let gate = gate_at(cell);

    if actor.action > 1 {
        close_prompt(scene, attribute);

        return Step::Nothing;
    }

    let previous = scene.gate.last_attribute;

    scene.gate.last_attribute = attribute;

    if attribute & ATTRIBUTE_BIT == 0 {
        close_prompt(scene, attribute);

        return Step::Nothing;
    }

    if scene.gate.prompted {
        return Step::Nothing;
    }

    scene.gate.prompted = true;

    let dedup = State {
        last_attribute: previous,
        ..scene.gate
    };

    if sector_dedup(&dedup, gate, attribute) {
        scene.gate.last_gate = gate.map_or(NO_GATE, |i| i as i32);

        return Step::Nothing;
    }

    if let Some(record) = rectangle_toast(cell) {
        let now_ms = scene.time.now_ms;

        if let Some(panels) = scene.panels.as_mut() {
            let text = panels.strings.translate(&format!("ss_{record}"));

            super::notice::post(panels, &text, TOAST_MS, true, now_ms);
        }

        return Step::Toast { record };
    }

    let already_open = scene
        .panels
        .as_ref()
        .is_some_and(|panels| panels.boxes.open().any(|(_, open)| open.context == CONTEXT));

    if already_open {
        return Step::Nothing;
    }

    if super::field::air_move_active(scene) {
        return Step::Nothing;
    }

    if suppressed(cell, scene.khepra.is_some()) {
        return Step::Nothing;
    }

    scene.gate.last_gate = gate.map_or(NO_GATE, |i| i as i32);

    let fee = match gate.and_then(|i| GATES.get(i)) {
        Some(row) if row.fee > 0 && quotes_fee(scene.siege.zone_kingdom, actor.kingdom) => {
            quoted_fee(
                row.fee,
                scene.entered.status.level,
                scene.entered.skill_mask[0],
            )
        }
        _ => 0,
    };

    if fee > 0 {
        let first = question(&scene.message_strings, gate, 0);

        let second = scene
            .message_strings
            .translate("ss_207")
            .replacen("%d", &fee.to_string(), 1);

        scene.legacy_boxes.set_text(
            super::legacy_message_box::Owner::Question,
            &first,
            0x10,
            Some(&second),
        );
    }

    let Some(panels) = scene.panels.as_mut() else {
        return Step::Dropped;
    };

    let body = super::msgbox::Body::Text(question(&panels.strings, gate, fee));

    let request = super::msgbox::Request {
        caption: Some(super::interact::TALK_BOX_CAPTION_STRDEF),
        kind: super::interact::TALK_BOX_KIND,
        context: CONTEXT,
        body,
        flags: super::msgbox::FLAGS,
    };

    match panels.raise_box(&request, None) {
        Some(slot) => Step::Asked { slot, gate, fee },
        None => Step::Dropped,
    }
}

fn close_prompt(scene: &mut FieldScene, attribute: u8) {
    if attribute & ATTRIBUTE_BIT != 0 {
        return;
    }

    let Some(panels) = scene.panels.as_mut() else {
        return;
    };

    while let Some(slot) = panels.boxes.find(CONTEXT) {
        panels.boxes.close(slot);
    }

    panels.msgbox.follow(&panels.boxes);
}

pub fn arrived(scene: &mut FieldScene) {
    close_prompt(scene, 0);

    if let Some(panels) = scene.panels.as_mut() {
        panels.gate_accepted = false;
    }

    let cell = scene.player.map(|i| cell_of(&scene.world.actors.actors[i]));

    scene.gate.last_attribute = cell
        .and_then(|at| attribute_at(scene.world.actors.attributes(), at))
        .unwrap_or(0);

    scene.gate.last_gate = cell.and_then(gate_at).map_or(NO_GATE, |i| i as i32);

    scene.gate.prompted = true;
}

pub fn accept(scene: &mut FieldScene) -> Accepted {
    scene.cc.last_action_ms = scene.time.now_ms;

    let on_gate = scene
        .player
        .map(|player| cell_of(&scene.world.actors.actors[player]))
        .and_then(|cell| attribute_at(scene.world.actors.attributes(), cell))
        .is_some_and(|attribute| attribute & ATTRIBUTE_BIT != 0);

    if !on_gate {
        let now_ms = scene.time.now_ms;

        if let Some(panels) = scene.panels.as_mut() {
            let text = panels.strings.translate(&format!("ss_{OFF_GATE_STRDEF}"));

            super::notice::post(panels, &text, TOAST_MS, true, now_ms);
        }

        return Accepted::OffGate;
    }

    scene.outbox.push(super::interact::frame(
        USE_TILE,
        super::interact::INTERACT_LEN,
        0,
    ));

    scene.gate.prompted = true;

    Accepted::Sent
}

pub fn panel_accepts(scene: &mut FieldScene) -> Option<Accepted> {
    let accepted = scene
        .panels
        .as_mut()
        .is_some_and(|panels| std::mem::take(&mut panels.gate_accepted));

    accepted.then(|| accept(scene))
}

pub fn describe(step: &Step) -> Option<String> {
    match step {
        Step::Nothing => None,
        Step::Asked { gate, fee, .. } => Some(match (gate, fee) {
            (Some(gate), 0) => {
                format!(
                    "NWYD: teleport gate {gate} — strdef[{}], no fee",
                    GATES[*gate].name,
                )
            }
            (Some(gate), fee) => {
                format!(
                    "NWYD: teleport gate {gate} — strdef[{}], {fee} gold",
                    GATES[*gate].name,
                )
            }
            (None, _) => "NWYD: teleport gate — bit 0x10 on a cell in no table row".to_string(),
        }),
        Step::Toast { record } => Some(format!("NWYD: teleport gate refused — strdef[{record}]")),
        Step::Dropped => {
            Some("NWYD: teleport gate — no message-box slot free, the question is dropped".into())
        }
    }
}

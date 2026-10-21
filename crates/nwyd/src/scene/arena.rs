use super::chat::format_tokens;
use super::event_hud::EventHud;
use super::kickout::MessagePanel;
use formats::net::arena::{self, Kill, List, Round};
use ui::Strings;

#[cfg(test)]
mod tooltip_tests;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Board {
    pub visible: bool,
    pub scores: [String; arena::SCORES],
    pub cost: i32,
    pub scroll: i32,
    pub minimap_hidden: bool,
    pub panel: Option<PanelFill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelFill {
    pub slot: usize,
    pub name: String,
    pub state: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rounded {
    pub visible: bool,
    pub banner: String,
    pub record: usize,
    pub life_ms: u32,
    pub chimed: bool,
    pub seeded: bool,
    pub hid_minimap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Killed {
    pub banner: Option<String>,
    pub record: Option<usize>,
    pub scores: [String; arena::SCORES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub slot: usize,
    pub cost: i32,
    pub fill: Option<PanelFill>,
    pub opened: bool,
}

pub fn score_text(value: i32, guarded: bool) -> String {
    match guarded && value == arena::BLANK {
        true => arena::BLANK_TEXT.to_string(),
        false => value.to_string(),
    }
}

fn record_text(strings: Option<&Strings>, record: usize) -> String {
    let Some(strings) = strings else {
        return record.to_string();
    };

    let text = strings.translate(&format!("ss_{record}"));

    match text.is_empty() || text == format!("ss_{record}") {
        true => record.to_string(),
        false => text,
    }
}

pub fn round(
    board: &mut Board,
    hud: &mut EventHud,
    message_panel: &mut MessagePanel,
    strings: Option<&Strings>,
    packet: &Round,
    now_ms: u32,
) -> Rounded {
    if !packet.is_active() {
        board.visible = false;

        let (record, banner) = match packet.draw() {
            true => (arena::DRAW_STRDEF, record_text(strings, arena::DRAW_STRDEF)),
            false => {
                let team = packet.winning_team().to_string();

                let format = record_text(strings, arena::WINS_STRDEF);

                (arena::WINS_STRDEF, format_tokens(&format, &[team.as_str()]))
            }
        };

        message_panel.post_formatted(record, banner.clone(), arena::ROUND_BANNER_MS, now_ms);

        return Rounded {
            visible: false,
            banner,
            record,
            life_ms: arena::ROUND_BANNER_MS,
            chimed: true,
            seeded: false,
            hid_minimap: false,
        };
    }

    let seeded = !board.visible;

    if seeded {
        super::event_hud::armed(
            hud,
            &formats::net::event_hud::Countdown {
                seconds: arena::SEED_SECONDS,
            },
            now_ms,
        );

        hud.clock.text = now_ms.to_string();
    }

    board.visible = true;

    board.scroll = 0;

    for (slot, value) in board.scores.iter_mut().zip(packet.scores) {
        *slot = score_text(value, true);
    }

    let hid_minimap = !board.minimap_hidden;

    board.minimap_hidden = true;

    let banner = record_text(strings, arena::OBJECTIVE_STRDEF);

    message_panel.replace_formatted(
        arena::OBJECTIVE_STRDEF,
        banner.clone(),
        arena::OBJECTIVE_BANNER_MS,
        now_ms,
    );

    Rounded {
        visible: true,
        banner,
        record: arena::OBJECTIVE_STRDEF,
        life_ms: arena::OBJECTIVE_BANNER_MS,
        chimed: false,
        seeded,
        hid_minimap,
    }
}

pub fn kill(
    board: &mut Board,
    message_panel: &mut MessagePanel,
    strings: Option<&Strings>,
    packet: &Kill,
    now_ms: u32,
) -> Killed {
    for (i, (slot, value)) in board.scores.iter_mut().zip(packet.scores).enumerate() {
        *slot = score_text(value, i != 0);
    }

    let scores = board.scores.clone();

    let Some(record) = packet.record() else {
        return Killed {
            banner: None,
            record: None,
            scores,
        };
    };

    let format = record_text(strings, record);

    let banner = match packet.tower() {
        true => format_tokens(
            &format,
            &[packet.killer_team.saturating_add(1).to_string().as_str()],
        ),
        false => {
            let killer_team = packet.killer_team.saturating_add(1).to_string();

            let victim_team = packet.victim_team.saturating_add(1).to_string();

            format_tokens(
                &format,
                &[
                    packet.killer.as_str(),
                    killer_team.as_str(),
                    packet.victim.as_str(),
                    victim_team.as_str(),
                ],
            )
        }
    };

    message_panel.post_formatted(record, banner.clone(), arena::KILL_BANNER_MS, now_ms);

    Killed {
        banner: Some(banner),
        record: Some(record),
        scores,
    }
}

pub fn list(board: &mut Board, strings: Option<&Strings>, packet: &List, created: bool) -> Listed {
    board.cost = 0;

    let slot = packet.chosen();

    if !created {
        return Listed {
            slot,
            cost: board.cost,
            fill: None,
            opened: false,
        };
    }

    board.cost = packet.cost[slot];

    let state = packet.state[slot];

    let (enabled, text) = match (
        state != 0 && board.cost != arena::BLANK,
        state == arena::LIST_OPEN,
    ) {
        (false, _) => (false, record_text(strings, arena::NO_ENTRY_STRDEF)),
        (true, true) => (true, slot_text(packet.current[slot], packet.max[slot])),
        (true, false) => (false, record_text(strings, arena::PROCEEDING_STRDEF)),
    };

    let fill = PanelFill {
        slot,
        name: record_text(strings, arena::NAME_STRDEF),
        state: text,
        enabled,
    };

    board.panel = Some(fill.clone());

    Listed {
        slot,
        cost: board.cost,
        fill: Some(fill),
        opened: false,
    }
}

pub const CLOSE_BUTTON: &str = "Close";

pub const ENTER_BUTTON: &str = "RoyalArena";

pub const BOX_KIND: crate::frame::PanelId = super::panels::CONTENT_ARENA;

pub const BOX_CONTEXT: u32 = arena::ACCEPT_BOX;

pub fn question(strings: Option<&Strings>, cost: i32) -> String {
    let ask = record_text(strings, arena::CONFIRM_STRDEF);

    if cost <= 0 {
        return ask;
    }

    let line = record_text(strings, arena::COST_STRDEF).replacen("%d", &cost.to_string(), 1);

    format!("{ask}{}{line}", ui::text::FF_BREAK)
}

pub fn confirm(strings: Option<&Strings>, cost: i32) -> super::msgbox::Request {
    super::msgbox::Request {
        caption: None,
        kind: BOX_KIND as i32,
        context: BOX_CONTEXT,
        body: super::msgbox::Body::Text(question(strings, cost)),
        flags: super::msgbox::FLAGS,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clicked {
    Closed,
    Asked(Box<super::msgbox::Request>),
    Ignored,
}

pub fn command(strings: Option<&Strings>, cost: i32, name: &str) -> Clicked {
    if name.eq_ignore_ascii_case(CLOSE_BUTTON) {
        return Clicked::Closed;
    }

    match name == ENTER_BUTTON {
        true => Clicked::Asked(Box::new(confirm(strings, cost))),
        false => Clicked::Ignored,
    }
}

pub fn accept() -> [u8; arena::ACCEPT_LEN] {
    arena::accept_frame(arena::ACCEPT_PAYLOAD)
}

pub fn slot_text(current: i32, max: i32) -> String {
    format!("( {current:2} / {max:2} )")
}

pub fn describe_round(rounded: &Rounded) -> String {
    format!(
        "NWYD: 0x2f1 arena round — board {}, strdef {} for {} ms{}{}{}",
        match rounded.visible {
            true => "up",
            false => "down",
        },
        rounded.record,
        rounded.life_ms,
        match rounded.chimed {
            true => ", chime",
            false => ", silent",
        },
        match rounded.seeded {
            true => format!(", clock seeded at {}s", arena::SEED_SECONDS),
            false => String::new(),
        },
        match rounded.hid_minimap {
            true => " (the minimap is down for good)",
            false => "",
        },
    )
}

pub fn describe_kill(killed: &Killed) -> String {
    match &killed.banner {
        Some(banner) => {
            format!(
                "NWYD: 0x2f2 arena kill — strdef {}, {banner:?}, scores {:?}",
                killed.record.unwrap_or_default(),
                killed.scores,
            )
        }
        None => {
            format!(
                "NWYD: 0x2f2 arena kill — no line (victim team {}), scores {:?}",
                arena::KILL_SILENT,
                killed.scores,
            )
        }
    }
}

pub fn describe_list(listed: &Listed) -> String {
    match &listed.fill {
        Some(fill) => {
            format!(
                "NWYD: 0x2f5 arena list — slot {}, {} gold, {:?} / {:?}, button {}{}",
                listed.slot,
                listed.cost,
                fill.name,
                fill.state,
                match fill.enabled {
                    true => "enabled",
                    false => "disabled",
                },
                match listed.opened {
                    true => ", panel raised",
                    false => "",
                },
            )
        }
        None => {
            format!(
                "NWYD: 0x2f5 arena list — slot {}, PanelArena was never created; the cost is {}",
                listed.slot, listed.cost,
            )
        }
    }
}

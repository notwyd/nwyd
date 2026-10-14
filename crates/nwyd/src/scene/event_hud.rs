use super::login_static::{Layout, Rect};
use formats::net::event_hud::{Countdown, Packed, Remaining, Score, Target};

pub const CLOCK_KEY: u32 = 0x1000_0000;

pub const COUNTER_KEY: u32 = 0x1000_0001;

pub const CLOCK_COLOUR: u32 = 0xFF00_FF00;

pub const COUNTER_COLOUR: u32 = 0xFFFF_AA00;

pub const CLOCK_BUILD_TEXT: &str = "00 : 00";

pub const COUNTER_BUILD_STRDEF: usize = 230;

pub const BOX_H: f32 = 16.0;

pub const BUILD_Y: f32 = 30.0;

pub const CLOCK_W: f32 = 200.0;

pub const COUNTER_W: f32 = 100.0;

pub const CLOCK_BUILD_X_HALF: f64 = 0.5;

pub const CLOCK_BUILD_X_INSET: f64 = 42.0;

pub const COUNTER_BUILD_X_INSET: f64 = 210.0;

pub const CLOCK_PLACE_X_HALF: f32 = 0.5;

pub const CLOCK_PLACE_X_INSET: f32 = 50.0;

pub const CLOCK_PLACE_Y_SCALE: f32 = 3.0;

pub const CLOCK_PLACE_Y_BASE: f32 = 55.0;

pub const COUNTER_PLACE_X_INSET: f32 = 200.0;

pub const COUNTER_PLACE_Y_SCALE: f32 = 30.0;

pub const COUNTER_PACKET_X_INSET: f32 = 130.0;

pub const COUNTER_TIMEOUT_MS: u32 = 6_000;

pub const COUNTER_TIMEOUT_INDOOR_MS: u32 = 900_000;

pub const COUNTER_NEVER_TYPE: i32 = 5;

pub const COUNTER_INDOOR_TYPE: i32 = 4;

pub const CLOCK_ZONES: [(u8, u8); 16] = [
    (1, 0x1F),
    (6, 0x1C),
    (9, 0x1C),
    (8, 0x1B),
    (10, 0x1B),
    (10, 0x0B),
    (0x0D, 0x1F),
    (0x0E, 0x1E),
    (0x0F, 0x1F),
    (8, 2),
    (9, 1),
    (10, 2),
    (0x0D, 0x1C),
    (0x0E, 0x1C),
    (0x1B, 0x0B),
    (1, 1),
];

pub const CLOCK_MINUTE_CAP: i32 = 100;

pub const CLOCK_SECOND_CAP: i32 = 100;

pub const ARENA_SEED_SECONDS: i32 = 900;

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub colour: u32,
    pub text: String,
    pub visible: bool,
}

impl Label {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventHud {
    pub clock: Label,
    pub counter: Label,
    pub total_seconds: i32,
    pub armed_at: u32,
    pub armed: bool,
    pub counter_at: u32,
    pub target_total: Option<i32>,
}

impl EventHud {
    pub fn new(layout: &Layout) -> EventHud {
        let clock_box = layout.control(
            build_x(layout, CLOCK_BUILD_X_HALF, CLOCK_BUILD_X_INSET),
            BUILD_Y,
            CLOCK_W,
            BOX_H,
        );

        let counter_box = layout.control(
            build_x(layout, 1.0, COUNTER_BUILD_X_INSET),
            BUILD_Y,
            COUNTER_W,
            BOX_H,
        );

        let mut hud = EventHud {
            clock: Label {
                x: clock_box.x,
                y: clock_box.y,
                w: clock_box.w,
                h: clock_box.h,
                colour: CLOCK_COLOUR,
                text: CLOCK_BUILD_TEXT.to_string(),
                visible: false,
            },
            counter: Label {
                x: counter_box.x,
                y: counter_box.y,
                w: counter_box.w,
                h: counter_box.h,
                colour: COUNTER_COLOUR,
                text: String::new(),
                visible: false,
            },
            total_seconds: 0,
            armed_at: 0,
            armed: false,
            counter_at: 0,
            target_total: None,
        };

        let (cx, cy) = clock_placement(layout);

        hud.clock.x = cx;

        hud.clock.y = cy;

        let (ux, uy) = counter_placement(layout);

        hud.counter.x = ux;

        hud.counter.y = uy;

        hud
    }
}

fn build_x(layout: &Layout, half: f64, inset: f64) -> f32 {
    ((f64::from(layout.width) / f64::from(layout.sx)) * half - inset) as f32
}

pub fn clock_placement(layout: &Layout) -> (f32, f32) {
    (
        layout.width as f32 * CLOCK_PLACE_X_HALF - CLOCK_PLACE_X_INSET,
        layout.sy * CLOCK_PLACE_Y_SCALE + CLOCK_PLACE_Y_BASE,
    )
}

pub fn counter_placement(layout: &Layout) -> (f32, f32) {
    (
        layout.width as f32 - layout.sx * COUNTER_PLACE_X_INSET,
        layout.sy * COUNTER_PLACE_Y_SCALE,
    )
}

pub fn packet_placement(layout: &Layout) -> (f32, f32) {
    (
        layout.width as f32 - COUNTER_PACKET_X_INSET,
        layout.sy * COUNTER_PLACE_Y_SCALE,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Armed {
    pub seconds: i32,
    pub at_ms: u32,
    pub text: String,
    pub visible: bool,
}

pub fn armed(hud: &mut EventHud, countdown: &Countdown, now_ms: u32) -> Armed {
    hud.total_seconds = countdown.seconds;

    hud.armed_at = now_ms;

    hud.armed = true;

    Armed {
        seconds: hud.total_seconds,
        at_ms: hud.armed_at,
        text: hud.clock.text.clone(),
        visible: hud.clock.visible,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wrote {
    pub text: Option<String>,
    pub visible: bool,
    pub stamped: bool,
    pub moved: bool,
    pub at: (f32, f32),
}

pub fn remaining(hud: &mut EventHud, remaining: &Remaining, now_ms: u32) -> Wrote {
    hud.counter_at = now_ms;

    hud.counter.text = remaining.text();

    hud.counter.visible = true;

    report(hud, Some(hud.counter.text.clone()), true, false)
}

pub fn packed(hud: &mut EventHud, packed: &Packed, now_ms: u32) -> Wrote {
    hud.counter_at = now_ms;

    hud.counter.text = packed.text();

    hud.counter.visible = true;

    report(hud, Some(hud.counter.text.clone()), true, false)
}

pub fn target(hud: &mut EventHud, target: &Target, layout: &Layout) -> Wrote {
    if !target.shows() {
        hud.counter.visible = false;

        return report(hud, None, false, false);
    }

    hud.target_total = Some(target.total);

    let (x, y) = packet_placement(layout);

    hud.counter.x = x;

    hud.counter.y = y;

    hud.counter.text = target.text().expect("the drawing arm formats a line");

    hud.counter.visible = true;

    report(hud, Some(hud.counter.text.clone()), false, true)
}

pub fn score(hud: &mut EventHud, score: &Score, layout: &Layout) -> Wrote {
    let (x, y) = packet_placement(layout);

    hud.counter.x = x;

    hud.counter.y = y;

    hud.counter.text = score.text();

    report(hud, Some(hud.counter.text.clone()), false, true)
}

fn report(hud: &EventHud, text: Option<String>, stamped: bool, moved: bool) -> Wrote {
    Wrote {
        text,
        visible: hud.counter.visible,
        stamped,
        moved,
        at: (hud.counter.x, hud.counter.y),
    }
}

pub fn counter_timeout(env_global: i32) -> Option<u32> {
    match env_global {
        COUNTER_NEVER_TYPE => None,
        COUNTER_INDOOR_TYPE => Some(COUNTER_TIMEOUT_INDOOR_MS),
        _ => Some(COUNTER_TIMEOUT_MS),
    }
}

pub fn clock_admitted(zone: (u8, u8), arena_board_visible: bool) -> bool {
    CLOCK_ZONES.contains(&zone) || arena_board_visible
}

pub fn clock_text(remaining: i32) -> String {
    format!("{:02} : {:02}", remaining / 60, remaining % 60)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ticked {
    pub remaining: Option<i32>,
    pub clock_text: String,
    pub clock_visible: bool,
    pub armed: bool,
    pub counter_visible: bool,
    pub counter_expired: bool,
    pub counter_hidden: bool,
}

pub fn tick(
    hud: &mut EventHud,
    now_ms: u32,
    env_global: i32,
    _zone: (u8, u8),
    _arena_board_visible: bool,
) -> Ticked {
    let counter_elapsed = now_ms.wrapping_sub(hud.counter_at);

    let counter_expired = match counter_timeout(env_global) {
        Some(limit) => counter_elapsed > limit,
        None => false,
    };

    let counter_hidden = counter_expired && hud.counter.visible;

    if counter_expired {
        hud.counter.visible = false;
    }

    let mut remaining = None;

    if hud.armed {
        let elapsed_s = (clock_elapsed(now_ms, hud.armed_at) / 1000) as i32;

        let left = hud.total_seconds.wrapping_sub(elapsed_s);

        remaining = Some(left);

        if left < 1 {
            hud.clock.visible = false;
        } else {
            hud.clock.text = clock_text(left);

            if left / 60 < CLOCK_MINUTE_CAP && left % 60 < CLOCK_SECOND_CAP {
                hud.clock.visible = true;
            }
        }
    }

    Ticked {
        remaining,
        clock_text: hud.clock.text.clone(),
        clock_visible: hud.clock.visible,
        armed: hud.armed,
        counter_visible: hud.counter.visible,
        counter_expired,
        counter_hidden,
    }
}

fn clock_elapsed(now_ms: u32, armed_at: u32) -> u32 {
    now_ms.wrapping_sub(armed_at)
}

pub fn describe_armed(armed: &Armed) -> String {
    format!(
        "0x3A1 event clock — {} s from {} ms, hidden until the tick shows it",
        armed.seconds, armed.at_ms
    )
}

pub fn describe_wrote(opcode: u16, wrote: &Wrote) -> String {
    match &wrote.text {
        Some(text) => {
            format!(
                "{opcode:#06X} event counter — {:?}{}{}",
                text,
                if wrote.stamped {
                    ", stamped"
                } else {
                    ", NOT stamped — the tick will hide it"
                },
                if wrote.moved { ", moved" } else { "" },
            )
        }
        None => format!("{opcode:#06X} event counter — hidden"),
    }
}

pub fn describe_tick(ticked: &Ticked) -> Option<String> {
    if ticked.counter_hidden {
        return Some("event counter — timed out, hidden".to_string());
    }

    None
}

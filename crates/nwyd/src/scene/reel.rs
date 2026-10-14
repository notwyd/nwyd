use formats::net::reel::{self, ResultMessage};

pub const PANEL: u32 = 0x1900;

pub const START: u16 = 0x1901;

pub const LESS_10K: u16 = 0x1902;

pub const LESS_1K: u16 = 0x1903;

pub const MORE_10K: u16 = 0x1904;

pub const MORE_1K: u16 = 0x1905;

pub const MONEY: u32 = 0x1908;

pub const BET: u32 = 0x1910;

pub const QUIT: u16 = 0x1915;

pub const TOOLTIP_TEXT_BASE: u32 = 0x0430_0000;

pub const TALK_B: i16 = 0x1006;

pub const TALK_A: i16 = 0x1007;

pub const SOUND_NO_ROOM: i64 = 0x21;

pub const SOUND_RESULT: i64 = 0x151;

pub const SOUND_SPIN: i64 = 0x152;

pub const SOUND_STOP: i64 = 0x153;

pub const SOUND_JACKPOT: i64 = 0x154;

pub const START_WAGER: i32 = 1_000;

pub const MAX_WAGER: i32 = 100_000;

pub const DURATION_MS: u32 = 3_000;

pub const BANNER_MS: u32 = 4_500;

pub const AUTO_MS: u32 = 2_000;

pub const AUTO_DELAY_MS: u32 = 10_000;

pub const BLINK_MS: u32 = 500;

pub const BUTTON_MS: u32 = 500;

pub const CELL: i32 = 62;

pub const CELLS: i32 = 22;

pub const CONTROLLER_AT: (f32, f32) = (21.0, 79.0);

pub const CONTROLLER_SIZE: (f32, f32) = (186.0, 180.0);

pub const BASE_SETS: [usize; 2] = [0x14A, 0x154];

pub const MARKER_SETS: [usize; 2] = [0x15E, 0x15F];

pub const DIGIT_SET: usize = 0x163;

pub const COUNTER_FRAME_SET: usize = 0x164;

pub const REEL_OFFSETS: [[usize; 22]; 3] = [
    [
        1, 6, 2, 3, 1, 4, 1, 8, 2, 5, 1, 9, 1, 6, 1, 4, 3, 0, 2, 5, 1, 7,
    ],
    [
        0, 1, 3, 5, 0, 3, 2, 4, 0, 1, 3, 6, 0, 3, 9, 2, 0, 3, 7, 3, 0, 8,
    ],
    [
        3, 2, 0, 4, 2, 6, 1, 0, 2, 0, 1, 8, 2, 3, 0, 5, 2, 0, 7, 0, 2, 9,
    ],
];

const SPEED_DIVISOR: f64 = 1.299_999_952_316_284_2;

pub fn money_text(money: i32) -> String {
    if money <= 0 {
        return format!("{money:10}");
    }

    let digits = money.to_string();

    let mut text = " ".repeat(10usize.saturating_sub(digits.len()));

    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index).is_multiple_of(3) {
            text.push(',');
        }

        text.push(digit);
    }

    text
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Idle,
    Spin,
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controller {
    pub mode: Mode,
    pub scroll: [i32; 3],
    pub strip_scroll: [Option<i32>; 3],
    pub stops: [i8; 3],
    pub last_tick_ms: u32,
    pub remaining_ms: u32,
    pub lines: [bool; 9],
    pub primary: [bool; 9],
    pub alternate: [bool; 9],
    pub target: u32,
    pub displayed: u32,
    pub auto_ms: u32,
    pub blink_ms: u32,
    pub payout: i32,
    pub remembered_wager: i32,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            mode: Mode::Idle,
            scroll: [0; 3],
            strip_scroll: [None; 3],
            stops: [0; 3],
            last_tick_ms: 0,
            remaining_ms: 0,
            lines: [false; 9],
            primary: [false; 9],
            alternate: [false; 9],
            target: 0,
            displayed: 0,
            auto_ms: 0,
            blink_ms: 0,
            payout: 0,
            remembered_wager: 0,
        }
    }
}

impl Controller {
    pub fn cell_y(&self, strip: usize, cell: usize) -> Option<i32> {
        if cell >= CELLS as usize {
            return None;
        }

        match *self.strip_scroll.get(strip)? {
            None => (cell < 4).then_some(cell as i32 * CELL - CELL),
            Some(scroll) => {
                let y = scroll.wrapping_add(cell as i32 * CELL) % (CELL * CELLS) - CELL;

                (y <= CELL * 3).then_some(y)
            }
        }
    }

    fn set_visible(&mut self, visible: bool) {
        if visible {
            self.strip_scroll = [None; 3];
        } else {
            self.lines = [false; 9];

            self.primary = [false; 9];

            self.alternate = [false; 9];
        }
    }

    pub fn active(&self) -> bool {
        self.mode == Mode::Spin || self.remaining_ms != 0
    }

    fn mark_line(&mut self, line: usize) {
        self.lines = match line {
            0..=2 => {
                let mut lines = self.lines;

                lines[line * 3..line * 3 + 3].fill(true);

                lines
            }
            3 => {
                let mut lines = self.lines;

                for index in [0, 4, 8] {
                    lines[index] = true;
                }

                lines
            }
            4 => {
                let mut lines = self.lines;

                for index in [6, 4, 2] {
                    lines[index] = true;
                }

                lines
            }
            _ => self.lines,
        };
    }

    fn arm(&mut self, visible: bool, mode: Mode, stops: [i8; 3], _now_ms: u32) {
        if !visible {
            return;
        }

        self.stops = stops;

        self.mode = mode;

        self.remaining_ms = DURATION_MS;

        if mode == Mode::Spin {
            self.lines = [false; 9];

            self.primary = [false; 9];

            self.alternate = [false; 9];

            self.target = 0;

            self.payout = 0;
        }
    }

    fn before_start(&mut self, now_ms: u32, auto: bool) -> bool {
        if self.mode == Mode::Idle && self.remaining_ms == 0 {
            if self.blink_ms.wrapping_add(BLINK_MS) < now_ms {
                self.blink_ms = now_ms;

                for index in 0..self.lines.len() {
                    if self.primary[index] {
                        self.primary[index] = false;

                        self.alternate[index] = true;
                    } else if self.alternate[index] {
                        self.alternate[index] = false;

                        self.primary[index] = true;
                    }
                }
            }

            if self.auto_ms.wrapping_add(AUTO_MS) < now_ms && auto {
                return true;
            }
        }

        false
    }

    fn after_start(&mut self, now_ms: u32, elapsed: u32, auto_start: bool) -> ControllerTick {
        let mut out = ControllerTick {
            auto_start,
            ..ControllerTick::default()
        };

        if auto_start {
            self.auto_ms = now_ms.wrapping_add(AUTO_DELAY_MS);
        }

        if self.mode == Mode::Spin || self.remaining_ms != 0 {
            if elapsed < self.remaining_ms {
                self.remaining_ms -= elapsed;
            } else {
                self.primary = self.lines;

                out.gold_delta = self.payout % 100_000_000;

                out.settled = true;

                self.auto_ms = now_ms;

                self.mode = Mode::Idle;

                self.remaining_ms = 0;

                if self.payout > 0 {
                    out.banner = Some(Banner {
                        payout: self.payout,
                        wager: self.remembered_wager,
                    });

                    self.target = 0;

                    self.payout = 0;
                }
            }

            let advance = (elapsed as f64 / SPEED_DIVISOR).trunc() as i32;

            for reel in 0..3 {
                if self.scroll[reel] > 2_000_000_000 || self.scroll[reel] < 0 {
                    self.scroll[reel] = 0;
                }

                let stops_now = self.mode != Mode::Spin
                    && match reel {
                        0 => self.remaining_ms < 1_500,
                        1 => self.remaining_ms < 900,
                        _ => self.remaining_ms < 300,
                    };

                if stops_now {
                    let destination = (23 - i32::from(self.stops[reel])) * CELL;

                    if self.scroll[reel] != destination {
                        self.scroll[reel] = destination;

                        self.strip_scroll[reel] = Some(destination);

                        out.stop_sounds += 1;
                    }
                } else {
                    self.scroll[reel] = self.scroll[reel].wrapping_add(advance);

                    self.strip_scroll[reel] = Some(self.scroll[reel]);
                }
            }
        }

        if (self.target == 0 || self.displayed < self.target) && self.displayed != 0 {
            if self.target == 0 {
                self.displayed = self.displayed.wrapping_add(1);
            } else {
                let step = (self.target - self.displayed) / 500;

                let next = self
                    .displayed
                    .wrapping_add(step.wrapping_mul(step))
                    .wrapping_add(1);

                self.displayed = next.min(self.target);
            }
        } else {
            self.displayed = self.target;
        }

        self.last_tick_ms = now_ms;

        out
    }

    pub fn digits(&self) -> [usize; 10] {
        let mut digits = [10usize; 10];

        let mut divisor = 1_000_000_000u32;

        for digit in &mut digits {
            *digit = ((self.displayed / divisor) % 10) as usize;

            divisor = (divisor / 10).max(1);
        }

        let first = digits.iter().position(|digit| *digit != 0).unwrap_or(9);

        digits[..first].fill(10);

        digits
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Banner {
    pub payout: i32,
    pub wager: i32,
}

impl Banner {
    pub fn text(self, suffix: &str) -> String {
        let ratio = self.payout as f32 / self.wager as f32;

        format!("X {ratio:.1}  {}{suffix}", self.payout)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ControllerTick {
    pub auto_start: bool,
    pub settled: bool,
    pub gold_delta: i32,
    pub stop_sounds: usize,
    pub banner: Option<Banner>,
}

#[must_use]
pub(crate) struct PendingTick {
    selected: usize,
    now_ms: u32,
    elapsed: u32,
    pub auto_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub visible: bool,
    pub hovered: Option<u16>,
    pub selected: usize,
    pub controller_visible: [bool; 2],
    pub wager: i32,
    pub controllers: [Controller; 2],
    pub banner: Option<PostedBanner>,
    buttons: [Button; 6],
}

impl Default for Game {
    fn default() -> Self {
        Self {
            visible: false,
            hovered: None,
            selected: 0,
            controller_visible: [true, true],
            wager: START_WAGER,
            controllers: [Controller::default(), Controller::default()],
            banner: None,
            buttons: [Button::default(); 6],
        }
    }
}

impl Game {
    pub fn button_frame(&self, command: u16) -> Option<usize> {
        let index = button_index(command)?;

        Some(if self.hovered == Some(command) {
            if self.buttons[index].armed { 2 } else { 1 }
        } else {
            0
        })
    }

    pub fn new(now_ms: u32) -> Self {
        let mut game = Self::default();

        for controller in &mut game.controllers {
            controller.last_tick_ms = now_ms;
        }

        game
    }

    pub fn open_talk(&mut self, item: i16) -> bool {
        let selected = match item {
            TALK_A => 0,
            TALK_B => 1,
            _ => return false,
        };

        if self.visible {
            return false;
        }

        self.visible = true;

        self.selected = selected;

        self.controller_visible = [selected == 0, selected == 1];

        for (index, controller) in self.controllers.iter_mut().enumerate() {
            controller.set_visible(index == selected);
        }

        true
    }

    pub fn close(&mut self) -> bool {
        if self.visible && self.controllers[1].active() {
            return false;
        }

        let changed = self.visible;

        self.visible = false;

        changed
    }

    pub fn apply_result(&mut self, message: &ResultMessage, now_ms: u32) -> AppliedResult {
        let jackpot = message.lines[1] == 14;

        if jackpot {
            for controller in &mut self.controllers {
                controller.target = 0;
            }
        } else {
            for controller in &mut self.controllers {
                controller.target = message.counter;
            }
        }

        for line in 0..message.lines.len() {
            if message.lines[line] > 0 {
                self.controllers[0].mark_line(line);

                self.controllers[1].mark_line(line);
            }
        }

        if message.payout > 0 {
            self.controllers[0].payout = message.payout;

            self.controllers[1].payout = message.payout;
        }

        for controller in 0..2 {
            self.controllers[controller].arm(
                self.controller_visible[controller],
                Mode::Result,
                message.stops,
                now_ms,
            );
        }

        AppliedResult {
            jackpot_gold: jackpot.then_some(message.counter % 100_000_000),
            sound: jackpot.then_some(SOUND_JACKPOT),
        }
    }

    pub fn adjust(&mut self, command: u16, gold: u32) -> bool {
        let gold = gold as i32;

        match command {
            LESS_10K => self.wager = self.wager.wrapping_sub(10_000).max(START_WAGER),
            LESS_1K => self.wager = self.wager.wrapping_sub(1_000).max(START_WAGER),
            MORE_10K => {
                if gold.wrapping_sub(self.wager) < 10_000 {
                    return false;
                }

                self.wager = self.wager.wrapping_add(10_000).min(MAX_WAGER);
            }
            MORE_1K => {
                if gold.wrapping_sub(self.wager) < 1_000 {
                    return false;
                }

                self.wager = self.wager.wrapping_add(1_000).min(MAX_WAGER);
            }
            _ => return false,
        }

        true
    }

    pub fn start(&mut self, client_id: u16, gold: u32, has_room: bool, now_ms: u32) -> Start {
        if self.controllers.iter().any(Controller::active) {
            return Start::Busy;
        }

        if !has_room {
            return Start::NoRoom;
        }

        if (gold as i32).wrapping_sub(self.wager) < 0 {
            return Start::NoGold;
        }

        let frame = reel::spin_frame(client_id, self.wager).to_vec();

        for controller in &mut self.controllers {
            controller.remembered_wager = self.wager;
        }

        for controller in 0..2 {
            self.controllers[controller].arm(
                self.controller_visible[controller],
                Mode::Spin,
                [0; 3],
                now_ms,
            );
        }

        Start::Sent {
            frame,
            debit: self.wager as u32,
        }
    }

    pub fn tick(&mut self, now_ms: u32, auto: bool) -> Ticked {
        let Some(pending) = self.begin_tick(now_ms, auto) else {
            return Ticked::default();
        };

        self.finish_tick(pending)
    }

    pub(crate) fn begin_tick(&mut self, now_ms: u32, auto: bool) -> Option<PendingTick> {
        if self
            .banner
            .as_ref()
            .is_some_and(|banner| now_ms.wrapping_sub(banner.shown_at_ms) > BANNER_MS)
        {
            self.banner = None;
        }

        if !self.visible || !self.controller_visible[self.selected] {
            return None;
        }

        let selected = self.selected;

        let controller = &mut self.controllers[selected];

        let elapsed = now_ms.wrapping_sub(controller.last_tick_ms);

        let auto_start = controller.before_start(now_ms, auto);

        Some(PendingTick {
            selected,
            now_ms,
            elapsed,
            auto_start,
        })
    }

    pub(crate) fn finish_tick(&mut self, pending: PendingTick) -> Ticked {
        let PendingTick {
            selected,
            now_ms,
            elapsed,
            auto_start,
        } = pending;

        let controller = self.controllers[selected].after_start(now_ms, elapsed, auto_start);

        Ticked {
            selected,
            controller,
        }
    }

    pub fn post_banner(&mut self, text: String, now_ms: u32) {
        self.banner = Some(PostedBanner {
            text,
            shown_at_ms: now_ms,
        });
    }

    pub fn input(
        &mut self,
        event: Mouse,
        hit: Option<u16>,
        inside_parent: bool,
        now_ms: u32,
    ) -> Input {
        if !self.visible {
            return Input::IGNORED;
        }

        let hit_index = hit.and_then(button_index);

        match event {
            Mouse::Down => {
                let Some(index) = hit_index else {
                    return Input {
                        consumed: inside_parent,
                        command: None,
                    };
                };

                self.buttons[index].armed = true;

                Input::CONSUMED
            }
            Mouse::Up => {
                let Some(index) = self.buttons.iter().position(|button| button.armed) else {
                    return Input {
                        consumed: hit_index.is_some() || inside_parent,
                        command: None,
                    };
                };

                if hit_index != Some(index) {
                    self.buttons[index].armed = false;

                    return Input {
                        consumed: hit_index.is_some() || inside_parent,
                        command: None,
                    };
                }

                if self.buttons[index].last_ms.wrapping_add(BUTTON_MS) >= now_ms {
                    return Input::CONSUMED;
                }

                self.buttons[index].armed = false;

                self.buttons[index].last_ms = now_ms;

                Input {
                    consumed: true,
                    command: Some(BUTTONS[index]),
                }
            }
        }
    }
}

pub const BUTTONS: [u16; 6] = [START, LESS_10K, LESS_1K, MORE_10K, MORE_1K, QUIT];

fn button_index(command: u16) -> Option<usize> {
    BUTTONS.iter().position(|candidate| *candidate == command)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Button {
    armed: bool,
    last_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostedBanner {
    pub text: String,
    pub shown_at_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouse {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    pub consumed: bool,
    pub command: Option<u16>,
}

impl Input {
    pub const IGNORED: Self = Self {
        consumed: false,
        command: None,
    };

    pub const CONSUMED: Self = Self {
        consumed: true,
        command: None,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppliedResult {
    pub jackpot_gold: Option<u32>,
    pub sound: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    Busy,
    NoRoom,
    NoGold,
    Sent { frame: Vec<u8>, debit: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ticked {
    pub selected: usize,
    pub controller: ControllerTick,
}

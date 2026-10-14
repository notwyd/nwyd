use formats::net::long_message::LongMessage;
use std::collections::VecDeque;

pub const PANEL_MS: u32 = 5_000;

pub const EVENT_MS: u32 = 180_000;

pub const OVERLAY_CAPACITY: usize = 100;

pub const OVERLAY_VISIBLE: usize = 5;

pub const NOTICE_COLOUR: u32 = 0xFFCC_AAFF;

pub const EVENT_COLOUR: u32 = 0xFFFF_F6A6;

pub const QUIZ_CAPTION: usize = 260;

pub const GUILD_CAPTION: usize = 259;

pub const CHIME: i64 = 0x21;

pub const PANEL_ID: u32 = 0x370;

pub const CAPTION_ID: u32 = 0x17DC;

pub const LINE_IDS: [u32; 4] = [0x371, 0x372, 0x373, 0x374];

pub const PANEL_HALF_WIDTH: f32 = 240.0;

pub const PANEL_VERTICAL_OFFSET: f32 = 200.0;

pub const OVERLAY_AT: (f32, f32) = (490.0, 440.0);

pub const OVERLAY_ROW_HEIGHT: f32 = 14.0;

pub const INITIAL_CAPTION: usize = 58;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct OverlayRow {
    pub text: String,
    pub colour: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Panel {
    pub available: bool,
    pub visible: bool,
    pub caption: String,
    pub lines: [String; 4],
    pub shown_at_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Applied {
    pub changed: bool,
    pub chime: bool,
}

impl Applied {
    pub const NO_CHANGE: Self = Self {
        changed: false,
        chime: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Ticked {
    pub panel_hidden: bool,
    pub event_rows: usize,
}

impl Ticked {
    pub fn changed(self) -> bool {
        self.panel_hidden || self.event_rows != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct State {
    pub panel: Panel,
    overlay: VecDeque<OverlayRow>,
    pub rewards_hidden: bool,
    pub event_lines: [String; 4],
    pub event_stamp_ms: u32,
    pub event_index: u32,
}

impl State {
    pub fn new(available: bool, initial_caption: String, now_ms: u32) -> Self {
        Self {
            panel: Panel {
                available,
                visible: false,
                caption: super::stext::display_text(&initial_caption),
                lines: std::array::from_fn(|_| String::new()),
                shown_at_ms: 0,
            },
            overlay: VecDeque::new(),
            rewards_hidden: false,
            event_lines: std::array::from_fn(|_| String::new()),
            event_stamp_ms: now_ms,
            event_index: 0,
        }
    }

    pub fn apply(
        &mut self,
        message: &LongMessage,
        now_ms: u32,
        quiz_caption: &str,
        guild_caption: &str,
    ) -> Applied {
        if message.mode == 10 {
            self.event_lines = message.lines.clone();

            self.event_stamp_ms = 0;

            return Applied::NO_CHANGE;
        }

        if !self.panel.available {
            return Applied::NO_CHANGE;
        }

        match message.mode {
            0 => self.panel.caption = super::stext::display_text(quiz_caption),
            1 => self.panel.caption = super::stext::display_text(guild_caption),
            _ => {}
        }

        for line in &message.lines {
            self.push_overlay(line.clone(), NOTICE_COLOUR);
        }

        self.panel.lines =
            std::array::from_fn(|index| super::stext::display_text(&message.lines[index]));

        self.panel.shown_at_ms = now_ms;

        self.panel.visible = true;

        Applied {
            changed: true,
            chime: true,
        }
    }

    pub fn tick(&mut self, now_ms: u32) -> Ticked {
        let panel_hidden =
            self.panel.visible && now_ms.wrapping_sub(self.panel.shown_at_ms) > PANEL_MS;

        if panel_hidden {
            self.panel.visible = false;
        }

        let mut event_rows = 0;

        if now_ms.wrapping_sub(self.event_stamp_ms) > EVENT_MS {
            self.event_index = (self.event_index + 1) % 4;

            self.event_stamp_ms = now_ms;

            for index in (0..self.event_lines.len()).rev() {
                if !self.event_lines[index].is_empty() {
                    self.event_lines[index] = self.event_lines[index].replace('_', " ");

                    self.push_overlay(self.event_lines[index].clone(), EVENT_COLOUR);

                    event_rows += 1;
                }
            }
        }

        Ticked {
            panel_hidden,
            event_rows,
        }
    }

    pub fn overlay(&self) -> impl DoubleEndedIterator<Item = &OverlayRow> {
        self.overlay.iter()
    }

    pub fn local_reply(&mut self, text: impl Into<String>) {
        self.push_overlay(text.into(), 0xFFFF_AAAA);
    }

    pub fn reward(&mut self, text: String) {
        if !self.rewards_hidden {
            self.push_overlay(text, 0xFFCC_AAFF);
        }
    }

    pub fn visible_overlay(&self) -> impl Iterator<Item = &OverlayRow> {
        self.overlay
            .iter()
            .skip(self.overlay.len().saturating_sub(OVERLAY_VISIBLE))
    }

    fn push_overlay(&mut self, text: String, colour: u32) {
        if self.overlay.len() == OVERLAY_CAPACITY {
            self.overlay.pop_front();
        }

        self.overlay.push_back(OverlayRow {
            text: super::stext::display_text(&text),
            colour,
        });
    }
}

pub const DESIGN_SIZE: (f32, f32) = (800.0, 600.0);

pub const PANEL_ID: u16 = 0x380;

pub const BUTTON_IDS: [u16; 4] = [0x381, 0x382, 899, 900];

pub const DEBOUNCE_MS: u32 = 500;

pub const QUESTION_COLOR: u32 = 0xFFFF_FFFF;

pub const BUTTON_COLOR: u32 = 0x7777_7777;

pub const BUTTON_TEXT_COLOR: u32 = 0xFF00_0000;

pub const PANEL_COLOR: u32 = 0x44FF_FFFF;

const PANEL_SIZE: (f32, f32) = (525.0, 54.0);

const QUESTION_RECT: (f32, f32, f32, f32) = (5.0, 104.0, 510.0, 16.0);

const BUTTON_RECTS: [(f32, f32, f32, f32); 4] = [
    (5.0, 128.0, 125.0, 20.0),
    (135.0, 128.0, 125.0, 20.0),
    (265.0, 128.0, 125.0, 20.0),
    (395.0, 128.0, 125.0, 20.0),
];

const INITIAL_QUESTION: &str = "Question";

const INITIAL_ANSWERS: [&str; 4] = ["Answer1", "Answer2", "Answer3", "Answer4"];

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    pub fn contains(self, point: (f32, f32)) -> bool {
        self.left <= point.0 && self.top <= point.1 && point.0 < self.right && point.1 < self.bottom
    }

    pub fn width(self) -> f32 {
        self.right - self.left
    }

    pub fn height(self) -> f32 {
        self.bottom - self.top
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Geometry {
    pub panel: Rect,
    pub question: Rect,
    pub buttons: [Rect; 4],
}

impl Geometry {
    pub fn new(width: u32, height: u32) -> Self {
        let width = width as f32;

        let height = height as f32;

        let sx = width / DESIGN_SIZE.0;

        let sy = height / DESIGN_SIZE.1;

        let panel_width = PANEL_SIZE.0 * sx;

        let panel_height = PANEL_SIZE.1 * sy;

        let panel_x = width * 0.5 - panel_width * 0.5;

        let panel_y = height * 0.016;

        let panel = Rect::from_xywh(panel_x, panel_y, panel_width, panel_height);

        let child = |(x, y, w, h): (f32, f32, f32, f32)| {
            Rect::from_xywh(panel_x + x * sx, panel_y + y * sy, w * sx, h * sy)
        };

        let question = child(QUESTION_RECT);

        let buttons = BUTTON_RECTS.map(|(x, y, w, h)| child((x + 1.0, y - 1.0, w, h)));

        Self {
            panel,
            question,
            buttons,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct TextSeed {
    pub control: Option<u16>,
    pub rect: Rect,
    pub text: String,
    pub color: u32,
}

#[derive(Debug, Clone, PartialEq)]

pub struct DrawSeed {
    pub visible: bool,
    pub panel: Rect,
    pub panel_color: u32,
    pub question: TextSeed,
    pub answers: [TextSeed; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Press {
    pub enabled: bool,
    pub armed: bool,
    pub last_accepted_ms: u32,
}

impl Default for Press {
    fn default() -> Self {
        Self {
            enabled: true,
            armed: false,
            last_accepted_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Mouse {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Choice {
    pub index: u32,
    pub control: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Input {
    pub consumed: bool,
    pub choice: Option<Choice>,
}

impl Input {
    pub const IGNORED: Self = Self {
        consumed: false,
        choice: None,
    };

    pub const CONSUMED: Self = Self {
        consumed: true,
        choice: None,
    };

    fn chosen(index: usize) -> Self {
        Self {
            consumed: true,
            choice: Some(Choice {
                index: index as u32,
                control: BUTTON_IDS[index],
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct State {
    pub visible: bool,
    pub question: String,
    pub answers: [String; 4],
    pub geometry: Geometry,
    presses: [Press; 4],
}

impl State {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            visible: false,
            question: INITIAL_QUESTION.to_string(),
            answers: INITIAL_ANSWERS.map(str::to_string),
            geometry: Geometry::new(width, height),
            presses: [Press::default(); 4],
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.geometry = Geometry::new(width, height);
    }

    pub fn replace(&mut self, question: String, answers: [String; 4]) {
        self.question = question;

        self.answers = answers;

        self.visible = true;
    }

    pub fn press(&self, choice: usize) -> Option<&Press> {
        self.presses.get(choice)
    }

    pub fn set_enabled(&mut self, choice: usize, enabled: bool) -> bool {
        let Some(press) = self.presses.get_mut(choice) else {
            return false;
        };

        press.enabled = enabled;

        true
    }

    fn button_at(&self, point: (f32, f32)) -> Option<usize> {
        self.geometry
            .buttons
            .iter()
            .position(|rect| rect.contains(point))
    }

    pub fn input(&mut self, event: Mouse, point: (f32, f32), now_ms: u32) -> Input {
        if !self.visible {
            return Input::IGNORED;
        }

        let hit = self.button_at(point);

        match event {
            Mouse::Down => {
                let Some(choice) = hit else {
                    return Input::IGNORED;
                };

                if !self.presses[choice].enabled {
                    return Input::CONSUMED;
                }

                self.presses[choice].armed = true;

                Input::CONSUMED
            }
            Mouse::Up => {
                let Some(choice) = hit else {
                    for press in &mut self.presses {
                        press.armed = false;
                    }

                    return Input::IGNORED;
                };

                if !self.presses[choice].enabled {
                    for (index, press) in self.presses.iter_mut().enumerate() {
                        if index != choice {
                            press.armed = false;
                        }
                    }

                    return Input::CONSUMED;
                }

                if !self.presses[choice].armed {
                    for press in &mut self.presses {
                        press.armed = false;
                    }

                    return Input::IGNORED;
                }

                let press = &mut self.presses[choice];

                if press.last_accepted_ms.wrapping_add(DEBOUNCE_MS) >= now_ms {
                    for (index, press) in self.presses.iter_mut().enumerate() {
                        if index != choice {
                            press.armed = false;
                        }
                    }

                    return Input::CONSUMED;
                }

                press.last_accepted_ms = now_ms;

                for press in &mut self.presses {
                    press.armed = false;
                }

                self.visible = false;

                Input::chosen(choice)
            }
        }
    }

    pub fn draw_seed(&self) -> DrawSeed {
        DrawSeed {
            visible: self.visible,
            panel: self.geometry.panel,
            panel_color: PANEL_COLOR,
            question: TextSeed {
                control: None,
                rect: self.geometry.question,
                text: self.question.clone(),
                color: QUESTION_COLOR,
            },
            answers: std::array::from_fn(|choice| TextSeed {
                control: Some(BUTTON_IDS[choice]),
                rect: self.geometry.buttons[choice],
                text: self.answers[choice].clone(),
                color: BUTTON_TEXT_COLOR,
            }),
        }
    }
}

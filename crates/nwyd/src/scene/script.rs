use formats::mesh::appearance;

pub const SOUND_ORC: i64 = 0x0F;

pub const SOUND_HUMAN: i64 = 0x10;

pub const SLOTS: usize = 50;

pub const CULLED: [std::ops::Range<usize>; 2] = [0..18, 26..40];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Cue {
    Order(usize),
    Play(usize, usize),
    Sound(i64),
    Cull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct DemoScript {
    pub variant: u32,
    pub stage: u8,
    pub culled: bool,
}

impl DemoScript {
    pub fn new(variant: u32) -> DemoScript {
        DemoScript {
            variant,
            stage: 0,
            culled: false,
        }
    }

    pub fn advance(&mut self, now_ms: u32) -> Vec<(u32, Cue)> {
        let mut cues = Vec::new();

        if now_ms > 5_000 && self.stage == 0 {
            match self.variant {
                0 => {
                    cues.push((5_000, Cue::Order(0)));

                    self.stage = 1;
                }
                1 => {
                    cues.extend((0..SLOTS).map(|i| (5_000, Cue::Order(i))));

                    cues.push((5_000, Cue::Sound(SOUND_ORC)));

                    self.stage = 1;
                }
                _ => {}
            }
        }

        if now_ms > 7_000 && self.stage == 1 {
            match self.variant {
                0 => {
                    cues.push((7_000, Cue::Order(1)));

                    self.stage = 2;
                }
                1 => {
                    cues.push((7_000, Cue::Sound(SOUND_HUMAN)));

                    self.stage = 2;
                }
                _ => {}
            }
        }

        if self.variant == 1 && now_ms > 14_000 && !self.culled {
            cues.push((14_000, Cue::Cull));

            self.culled = true;
        }

        if self.variant == 3 {
            if now_ms > 12_000 && self.stage == 0 {
                cues.push((12_000, Cue::Play(0, LEVELUP)));

                self.stage = 1;
            }

            if now_ms > 20_000 && self.stage == 1 {
                cues.push((20_000, Cue::Play(0, LEVELUP)));

                self.stage = 2;
            }
        }

        cues
    }

    pub fn alive(&self, record: usize) -> bool {
        !self.culled || !CULLED.iter().any(|r| r.contains(&record))
    }
}

pub const LEVELUP: usize = 14;

const _: () = assert!(LEVELUP != appearance::ACTION_WALK && LEVELUP != appearance::ACTION_RUN);

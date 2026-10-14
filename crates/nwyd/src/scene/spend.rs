use super::enter::Entered;
use super::panels::Panels;

pub const SPEND: u16 = 0x277;

pub const SPEND_LEN: usize = 0x14;

pub const REPEAT_DELAY_MS: u32 = 0x5DC;

pub const REPEAT_PERIOD_MS: u32 = 0x64;

pub const KIND_STAT: u16 = 0;

pub const KIND_MASTERY: u16 = 1;

pub const KIND_SKILL: u16 = 2;

pub const STAT_BUTTONS: [&str; 4] = ["StatBtn0", "StatBtn1", "StatBtn2", "StatBtn3"];

pub const MASTERY_BUTTONS: [&str; 4] = ["MasterBtn0", "MasterBtn1", "MasterBtn2", "MasterBtn3"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Repeat {
    pub left_down: bool,
    button: u8,
    armed: bool,
    stamp: u32,
}

impl Repeat {
    pub fn reset(&mut self) {
        self.button = 0;

        self.armed = false;
    }

    pub fn cancel(&mut self) {
        *self = Self::default();
    }

    fn press(&mut self, button: u8) {
        self.button = button;

        self.armed = false;

        self.stamp = 0;
    }

    fn due(&mut self, now: u32, open: bool) -> Option<u8> {
        if self.button == 0 {
            return None;
        }

        if !self.left_down || !open {
            self.reset();

            self.stamp = 0;

            return None;
        }

        if !self.armed {
            if self.stamp == 0 {
                self.stamp = now;
            } else if self.stamp.wrapping_add(REPEAT_DELAY_MS) < now {
                self.stamp = now;

                self.armed = true;
            }

            return None;
        }

        if self.stamp.wrapping_add(REPEAT_PERIOD_MS) >= now {
            return None;
        }

        self.stamp = now;

        Some(self.button)
    }
}

pub fn is_button(name: &str) -> bool {
    STAT_BUTTONS
        .iter()
        .chain(MASTERY_BUTTONS.iter())
        .any(|b| b.eq_ignore_ascii_case(name))
}

pub fn press(panels: &mut Panels, entered: &Entered, name: &str, at: Raise) -> Option<Spent> {
    let button = STAT_BUTTONS
        .iter()
        .chain(MASTERY_BUTTONS.iter())
        .position(|b| b.eq_ignore_ascii_case(name))?;

    panels.spend_repeat.press(button as u8 + 1);

    command(panels, entered, name, at)
}

pub fn tick(panels: &mut Panels, entered: &Entered, body: Option<Raise>, now: u32) {
    panels.now_ms = now;

    let open = panels.is_open(super::panels::CONTENT_CHARACTER);

    let Some(button) = panels.spend_repeat.due(now, open) else {
        return;
    };

    let Some(at) = body else {
        panels.spend_repeat.reset();

        return;
    };

    let result = if button <= 4 {
        raise_stat(panels, entered, usize::from(button - 1), at)
    } else {
        raise_mastery(panels, entered, usize::from(button - 5), at)
    };

    if matches!(result, Spent::Refused(Refused::NoPoints | Refused::NoBody)) {
        panels.spend_repeat.reset();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spent {
    Sent { kind: u16, index: u16 },
    Refused(Refused),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NoBody,
    Dead,
    NoPoints,
    AtCap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Raise {
    pub dead: bool,
    pub absent: bool,
}

pub fn frame(kind: u16, which: u16, extra: u32) -> Vec<u8> {
    let mut out = vec![0u8; SPEND_LEN];

    protocol::header::set_opcode(&mut out, SPEND);

    let at = protocol::header::LEN;

    out[at..at + 2].copy_from_slice(&kind.to_le_bytes());

    out[at + 2..at + 4].copy_from_slice(&which.to_le_bytes());

    out[at + 4..at + 8].copy_from_slice(&extra.to_le_bytes());

    out
}

pub fn raise_stat(panels: &mut Panels, entered: &Entered, index: usize, at: Raise) -> Spent {
    if at.absent {
        return Spent::Refused(Refused::NoBody);
    }

    if at.dead {
        return Spent::Refused(Refused::Dead);
    }

    if entered.derived.stat_points < 1 {
        return Spent::Refused(Refused::NoPoints);
    }

    let which = index as u16;

    panels.wire.push(frame(KIND_STAT, which, 0));

    Spent::Sent {
        kind: KIND_STAT,
        index: which,
    }
}

pub fn raise_mastery(panels: &mut Panels, entered: &Entered, index: usize, at: Raise) -> Spent {
    if at.absent {
        return Spent::Refused(Refused::NoBody);
    }

    if at.dead {
        return Spent::Refused(Refused::Dead);
    }

    if entered.derived.mastery_points < 1 {
        return Spent::Refused(Refused::NoPoints);
    }

    if let Some(&current) = entered.status.mastery.get(index)
        && i32::from(current)
            >= mastery_caps(
                entered.status.level,
                entered.class,
                entered.equipment.evolved(),
                entered.skill_mask,
            )[index]
    {
        let text = panels
            .strings
            .strdef
            .as_ref()
            .map(|table| table.get(39).to_owned())
            .unwrap_or_default();

        super::notice::post(panels, &text, 2000, true, panels.now_ms);

        return Spent::Refused(Refused::AtCap);
    }

    let which = index as u16;

    panels.wire.push(frame(KIND_MASTERY, which, 0));

    Spent::Sent {
        kind: KIND_MASTERY,
        index: which,
    }
}

pub fn mastery_caps(level: u16, class: u8, evolved: bool, skills: [u32; 2]) -> [i32; 4] {
    let tier_two = evolved && skills[0] & (1 << 30) != 0;

    let base = if tier_two {
        200
    } else {
        (i32::from(level as i16) * 3 + 3) / 2
    };

    let mut caps = [base, base.min(200), base.min(200), base.min(200)];

    if tier_two {
        if class == 0 && skills[1] & (1 << 5) != 0 {
            caps[0] = 280;
        } else if class == 2 && skills[1] & (1 << 9) != 0 {
            caps[0] = 230;
        }
    }

    if class == 3 && skills[1] & (1 << 2) != 0 {
        caps[1] = 400;
    } else if skills[1] & 1 != 0 {
        caps[1] = 320;
    } else if skills[0] & (1 << 7) != 0 {
        caps[1] = 255;
    }

    for (row, high, low) in [(2, 4, 15), (3, 8, 23)] {
        if skills[1] & (1 << high) != 0 {
            caps[row] = 320;
        } else if skills[0] & (1 << low) != 0 {
            caps[row] = 255;
        }
    }

    caps
}

pub fn command(panels: &mut Panels, entered: &Entered, name: &str, at: Raise) -> Option<Spent> {
    if let Some(index) = STAT_BUTTONS
        .iter()
        .position(|b| b.eq_ignore_ascii_case(name))
    {
        return Some(raise_stat(panels, entered, index, at));
    }

    if let Some(index) = MASTERY_BUTTONS
        .iter()
        .position(|b| b.eq_ignore_ascii_case(name))
    {
        return Some(raise_mastery(panels, entered, index, at));
    }

    None
}

pub fn describe(spent: &Spent, name: &str) -> Option<String> {
    match spent {
        Spent::Sent { .. } => None,
        Spent::Refused(Refused::Dead) => {
            Some(format!("NWYD: {name} — the dead do not spend points"))
        }
        Spent::Refused(Refused::NoPoints) => Some(format!("NWYD: {name} — no points to spend")),
        Spent::Refused(Refused::AtCap | Refused::NoBody) => None,
    }
}

pub const SKILL_SLOTS: usize = 20;

pub const PAGE_SLOTS: usize = 10;

pub const DEFAULT_AUTO_COUNT: i32 = 4;

pub const MAX_AUTO_COUNT: i32 = 10;

pub const DIGITS: [char; PAGE_SLOTS] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

pub const SYMBOLS: [char; PAGE_SLOTS] = ['!', '@', '#', '$', '%', '^', '&', '*', '(', ')'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub auto_count: i32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            auto_count: DEFAULT_AUTO_COUNT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Select {
    Ignored,
    Swallowed,
    Selected(usize),
    InvalidPage(i32),
}

impl Select {
    pub fn handled(self) -> bool {
        !matches!(self, Select::Ignored)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flip {
    pub page: Option<i32>,
    pub select: Select,
}

pub fn key_slot(ch: char) -> Option<usize> {
    DIGITS
        .iter()
        .position(|&candidate| candidate == ch)
        .or_else(|| symbol_slot(ch))
}

pub fn symbol_slot(ch: char) -> Option<usize> {
    SYMBOLS.iter().position(|&candidate| candidate == ch)
}

pub fn select(swallow_next: &mut bool, ch: char, page: Option<i32>) -> Select {
    let Some(slot) = key_slot(ch) else {
        return Select::Ignored;
    };

    if std::mem::take(swallow_next) {
        return Select::Swallowed;
    }

    match page {
        None | Some(0) => Select::Selected(slot),
        Some(1) => Select::Selected(PAGE_SLOTS + slot),
        Some(page) => Select::InvalidPage(page),
    }
}

pub fn flip_and_select_one(swallow_next: &mut bool, page: Option<i32>) -> Flip {
    let page = page.map(|page| i32::from(page == 0));

    let select = select(swallow_next, '1', page);

    Flip { page, select }
}

impl State {
    pub fn adjust_auto_count(&mut self, ch: char, armed: bool) -> bool {
        if ch != '-' && ch != '_' {
            return false;
        }

        if armed {
            let next = self.auto_count.wrapping_add(1);

            self.auto_count = if next > MAX_AUTO_COUNT { 1 } else { next };
        }

        true
    }
}

pub fn entry_selection(quick: &[u8; SKILL_SLOTS]) -> Option<usize> {
    quick[..PAGE_SLOTS - 1]
        .iter()
        .rposition(|&code| code < 0xF8)
}

pub fn next_auto_slot(
    selected: u8,
    page: i32,
    count: i32,
    armed: bool,
    present: &[bool; SKILL_SLOTS],
) -> Option<usize> {
    if !armed {
        return None;
    }

    let end = if page == 1 {
        SKILL_SLOTS as i32
    } else {
        PAGE_SLOTS as i32
    };

    let start = end.wrapping_sub(count);

    let current = i32::from(selected as i8);

    let mut candidate = if current < start {
        start
    } else {
        current.wrapping_add(1)
    };

    if candidate >= end {
        candidate = start;
    }

    let mut slot = candidate;

    while slot < end {
        let Ok(index) = usize::try_from(slot) else {
            break;
        };

        if index >= SKILL_SLOTS {
            break;
        }

        if present[index] {
            return Some(index);
        }

        slot = slot.wrapping_add(1);
    }

    if start < candidate {
        let mut slot = start;

        while slot < candidate {
            let Ok(index) = usize::try_from(slot) else {
                return None;
            };

            if index >= SKILL_SLOTS {
                return None;
            }

            if present[index] {
                return Some(index);
            }

            slot = slot.wrapping_add(1);
        }
    }

    None
}

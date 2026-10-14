use super::hud::InputLock;
use protocol::{client, server};

pub const STAMP_SLOT: usize = 6;

pub const FIRE_MS: u32 = 5_000;

pub const EXPIRE_MS: u32 = 6_000;

pub const TICK_MS: u32 = 1_000;

pub const COLUMN_KIND: i32 = 1;

pub const COLUMN_LIFT: f32 = 0.05;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub requester: String,
    pub armed: bool,
    pub command: bool,
    pub command_requester: String,
    pub shown: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Raise,
    Clear,
}

pub fn requested(state: &mut State, request: &server::SummonRequest) -> Notice {
    state.requester.clone_from(&request.requester);

    Notice::Raise
}

pub fn cancelled(_state: &mut State) -> Notice {
    Notice::Clear
}

pub fn answer(state: &mut State, lock: &mut InputLock, yes: bool, now_ms: u32) -> bool {
    if !yes {
        return false;
    }

    state.armed = true;

    state.command = false;

    lock.stamp(STAMP_SLOT, now_ms);

    true
}

pub fn command(state: &mut State, lock: &mut InputLock, requester: Option<&str>, now_ms: u32) {
    if let Some(requester) = requester {
        state.command_requester.clear();

        state.command_requester.push_str(requester);
    }

    state.armed = true;

    state.command = true;

    lock.stamp(STAMP_SLOT, now_ms);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Nothing,
    Expired,
    Tick(u32),
    Sent { requester: String },
}

impl State {
    pub fn seconds(stamp_ms: u32, now_ms: u32) -> u32 {
        stamp_ms.wrapping_sub(now_ms).wrapping_add(FIRE_MS) / TICK_MS
    }

    pub fn tick(&mut self, lock: &mut InputLock, outbox: &mut Vec<Vec<u8>>, now_ms: u32) -> Step {
        let stamp = lock.stamps[STAMP_SLOT];

        if stamp == 0 {
            return Step::Nothing;
        }

        if !self.armed && stamp.wrapping_add(EXPIRE_MS) < now_ms {
            lock.stamps[STAMP_SLOT] = 0;

            return Step::Expired;
        }

        if stamp.wrapping_add(FIRE_MS) < now_ms && self.armed {
            let requester = match self.command {
                true => self.command_requester.clone(),
                false => self.requester.clone(),
            };

            outbox.push(client::summon_relocation(&requester));

            self.armed = false;

            return Step::Sent { requester };
        }

        let seconds = Self::seconds(stamp, now_ms);

        if self.shown == Some(seconds) {
            return Step::Nothing;
        }

        self.shown = Some(seconds);

        Step::Tick(seconds)
    }
}

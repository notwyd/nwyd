use formats::net::action::Order;

pub const ACTIVE_ENVIRONMENT: i32 = 5;

pub const TOTAL_SECONDS: u32 = 900;

pub const SOURCE_ZONE: (i32, i32) = (25, 13);

pub const DESTINATION_X: std::ops::RangeInclusive<i32> = 26..=30;

pub const DESTINATION_Z: std::ops::RangeInclusive<i32> = 8..=12;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    pub started_at: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Update {
    pub visible: Option<bool>,
    pub text: Option<String>,
}

pub fn starts(local: bool, order: Order, here: (i32, i32), destination: (i32, i32)) -> bool {
    local
        && matches!(order, Order::Act(_))
        && ((here.0 & !0x7F) >> 7, (here.1 & !0x7F) >> 7) == SOURCE_ZONE
        && DESTINATION_X.contains(&(destination.0 >> 7))
        && DESTINATION_Z.contains(&(destination.1 >> 7))
}

pub fn frame_environment(env_global: i32, pending_zone: Option<(u8, u8)>) -> i32 {
    pending_zone.map_or(env_global, |zone| {
        super::world::environment_type(zone.0, zone.1)
    })
}

pub fn start(state: &mut State, now_ms: u32) -> Update {
    state.started_at = now_ms;

    Update {
        visible: Some(true),
        text: (state.started_at != 0).then(|| text(state.started_at, now_ms)),
    }
}

pub fn tick(state: &mut State, now_ms: u32, env_global: i32) -> Update {
    if env_global != ACTIVE_ENVIRONMENT {
        state.started_at = 0;

        return Update {
            visible: Some(false),
            text: None,
        };
    }

    if state.started_at == 0 {
        return Update::default();
    }

    Update {
        visible: Some(true),
        text: Some(text(state.started_at, now_ms)),
    }
}

pub fn text(started_at: u32, now_ms: u32) -> String {
    let elapsed_seconds = now_ms.wrapping_sub(started_at) / 1_000;

    let remaining = TOTAL_SECONDS.wrapping_sub(elapsed_seconds) as i32;

    let minutes = remaining / 60;

    let seconds = remaining - minutes * 60;

    format!("[{minutes:02}:{seconds:02}]")
}

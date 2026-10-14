use crate::clock::GameTime;
use std::ops::ControlFlow;

pub mod actor_packets;
pub mod combat_packets;
pub mod commerce_packets;
pub mod departure;
pub mod dispatch;
pub mod entry;
pub mod environment_packets;
pub mod field_packets;
pub mod identity;
pub mod interaction_packets;
pub mod inventory_packets;
pub mod lifecycle;
pub mod live_packets;
pub mod message_packets;
pub mod object_packets;
pub mod outbound;
pub mod screens;
pub mod selection;
pub mod session_packets;
pub mod social_packets;
pub mod state;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum MusicOrder {
    BeforeOutbox,
    AfterOutbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum UpdateOutcome {
    Ready { game_time_ms: GameTime },
    Exit,
}

pub trait UpdateHost {
    type Error;

    fn sample_time(&mut self) -> GameTime;

    fn camera_input(&mut self);

    fn poll_status(&mut self);

    fn poll_network(&mut self) -> ControlFlow<()>;

    fn tick_scene(&mut self) -> ControlFlow<()>;

    fn music_order(&mut self) -> MusicOrder;

    fn flush_music(&mut self);

    fn stream_zones(&mut self);

    fn flush_outbox(&mut self);

    fn flush_browser(&mut self);

    fn save_quick_slots(&mut self);

    fn sync_ime(&mut self);
}

pub fn advance<H: UpdateHost>(host: &mut H) -> Result<UpdateOutcome, H::Error> {
    let game_time_ms = host.sample_time();

    host.camera_input();

    host.poll_status();

    if host.poll_network().is_break() {
        return Ok(UpdateOutcome::Exit);
    }

    if host.tick_scene().is_break() {
        return Ok(UpdateOutcome::Exit);
    }

    let music = host.music_order();

    if music == MusicOrder::BeforeOutbox {
        host.flush_music();
    }

    host.stream_zones();

    host.flush_outbox();

    host.flush_browser();

    if music == MusicOrder::AfterOutbox {
        host.flush_music();
    }

    host.save_quick_slots();

    host.sync_ime();

    Ok(UpdateOutcome::Ready { game_time_ms })
}

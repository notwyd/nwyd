use crate::scene::Scene;
use formats::net::{arena, event_hud, weather};

pub enum Packet {
    Weather(weather::Forecast),
    EventCountdown(event_hud::Countdown),
    EventRemaining(event_hud::Remaining),
    EventPacked(event_hud::Packed),
    EventTarget(event_hud::Target),
    EventScore(event_hud::Score),
    ArenaRound(arena::Round),
    ArenaKill(Box<arena::Kill>),
    ArenaList(Box<arena::List>),
}

pub trait Host {
    fn scene(&mut self) -> &mut Scene;

    fn game_time(&mut self) -> crate::clock::GameTime;

    fn log_message(&mut self, record: usize);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet) {
    match packet {
        Packet::Weather(forecast) => {
            let now = host.game_time().as_millis();

            match host.scene().weather(&forecast, now) {
                Some(applied) => {
                    tracing::info!("{}", applied.describe());

                    host.redraw();
                }
                None => {
                    tracing::info!(
                        "NWYD: 0x18B weather {} arrived with no world up",
                        forecast.code,
                    )
                }
            }
        }
        Packet::EventCountdown(countdown) => {
            let now = host.game_time().as_millis();

            match host.scene().event_countdown(&countdown, now) {
                Some(armed) => {
                    tracing::info!("{}", crate::scene::event_hud::describe_armed(&armed));

                    host.redraw();
                }
                None => {
                    tracing::info!(
                        "NWYD: 0x3A1 event clock ({} s) arrived with no world up",
                        countdown.seconds,
                    )
                }
            }
        }
        Packet::EventRemaining(remaining) => {
            let now = host.game_time().as_millis();

            match host.scene().event_remaining(&remaining, now) {
                Some(wrote) => {
                    tracing::info!(
                        "{}",
                        crate::scene::event_hud::describe_wrote(
                            protocol::server::EVENT_REMAINING,
                            &wrote,
                        ),
                    );

                    host.redraw();
                }
                None => {
                    tracing::info!("NWYD: 0x3B0 event counter arrived with no world up")
                }
            }
        }
        Packet::EventPacked(packed) => {
            let now = host.game_time().as_millis();

            match host.scene().event_packed(&packed, now) {
                Some(wrote) => {
                    tracing::info!(
                        "{}",
                        crate::scene::event_hud::describe_wrote(
                            protocol::server::EVENT_PACKED,
                            &wrote,
                        ),
                    );

                    host.redraw();
                }
                None => {
                    tracing::info!("NWYD: 0x3BB event counter arrived with no world up")
                }
            }
        }
        Packet::EventTarget(target) => match host.scene().event_target(&target) {
            Some(wrote) => {
                tracing::info!(
                    "{}",
                    crate::scene::event_hud::describe_wrote(protocol::server::EVENT_TARGET, &wrote,),
                );

                host.redraw();
            }
            None => {
                tracing::info!("NWYD: 0x3CF event counter arrived with no world up")
            }
        },
        Packet::EventScore(score) => match host.scene().event_score(&score) {
            Some(wrote) => {
                tracing::info!(
                    "{}",
                    crate::scene::event_hud::describe_wrote(protocol::server::EVENT_SCORE, &wrote,),
                );

                host.redraw();
            }
            None => {
                tracing::info!("NWYD: 0x3D0 event counter arrived with no world up")
            }
        },
        Packet::ArenaRound(packet) => {
            let now = host.game_time().as_millis();

            match host.scene().arena_round(&packet, now) {
                Some(rounded) => {
                    tracing::info!("{}", crate::scene::arena::describe_round(&rounded));

                    host.log_message(rounded.record);

                    host.redraw();
                }
                None => tracing::info!("NWYD: 0x2f1 arena round arrived with no world up"),
            }
        }
        Packet::ArenaKill(packet) => {
            let now = host.game_time().as_millis();

            match host.scene().arena_kill(&packet, now) {
                Some(killed) => {
                    tracing::info!("{}", crate::scene::arena::describe_kill(&killed));

                    if let Some(record) = killed.record {
                        host.log_message(record);
                    }

                    if killed.record.is_some()
                        || host.scene().arena().is_some_and(|board| board.visible)
                    {
                        host.redraw();
                    }
                }
                None => tracing::info!("NWYD: 0x2f2 arena kill arrived with no world up"),
            }
        }
        Packet::ArenaList(packet) => match host.scene().arena_list(&packet) {
            Some(listed) => {
                tracing::info!("{}", crate::scene::arena::describe_list(&listed));

                if listed.fill.is_some() {
                    host.redraw();
                }
            }
            None => tracing::info!("NWYD: 0x2f5 arena list arrived with no world up"),
        },
    }
}

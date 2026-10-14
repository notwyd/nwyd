use crate::scene::{Scene, crafting, panels};
use formats::net::{question, reel};

pub enum Packet {
    Question(Box<question::Question>),
    ReelResult(Box<reel::ResultMessage>),
    CloseCargo,
    CraftingClosed,
}

pub trait Host {
    fn scene(&mut self) -> &mut Scene;

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet) {
    match packet {
        Packet::Question(question) => {
            if host.scene().question(&question) {
                tracing::info!("NWYD: 0x1C6 — four-choice field question shown");

                host.redraw();
            } else {
                tracing::info!("NWYD: 0x1C6 arrived with no world up");
            }
        }
        Packet::ReelResult(result) => match host.scene().reel_result(&result) {
            Some(applied) => {
                tracing::info!(
                    "NWYD: 0x1BF — paired reel result applied{}",
                    if applied.jackpot_gold.is_some() {
                        " with an immediate jackpot credit"
                    } else {
                        ""
                    },
                );

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x1BF arrived with no world up"),
        },
        Packet::CloseCargo => {
            if let Scene::Field(scene) = host.scene()
                && let Some(panels) = scene.panels.as_mut()
            {
                panels::apply(
                    panels,
                    &[panels::Action::SetOpen(panels::CONTENT_CARGO, false)],
                );

                tracing::info!("NWYD: 0x19C — warehouse close handled");

                host.redraw();
            }
        }
        Packet::CraftingClosed => match host.scene().crafting_closed() {
            Some(closed) => {
                tracing::info!("{}", crafting::describe(&closed));

                if closed.changed() {
                    host.redraw();
                }
            }
            None => tracing::info!("NWYD: 0x3A7 arrived with no world up"),
        },
    }
}

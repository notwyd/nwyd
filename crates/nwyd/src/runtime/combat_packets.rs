use crate::scene::{Scene, affect, attack, kill};
use formats::net::{affect::Affects, attack::Attack, kill::Kill};

pub enum Packet {
    Attack { opcode: u16, attack: Box<Attack> },
    Kill(Kill),
    Affects { entity: u16, affects: Box<Affects> },
}

pub trait Host {
    fn scene(&mut self) -> &mut Scene;

    fn refresh_screens(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet) {
    match packet {
        Packet::Attack {
            opcode,
            attack: swing,
        } => match host.scene().attacked(opcode, &swing) {
            Some(applied) => {
                if let Some(line) = attack::describe_applied(&applied) {
                    tracing::info!("{line}");
                }

                if applied.redraws() {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: {opcode:#05x} attack arrived with no world up")
            }
        },
        Packet::Kill(death) => match host.scene().killed(&death) {
            Some(applied) => {
                tracing::info!("{}", kill::describe(&applied));

                if applied.credited {
                    host.refresh_screens();
                }

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x338 arrived with no world up"),
        },
        Packet::Affects { entity, affects } => match host.scene().affects(entity, &affects) {
            Some(applied) => {
                tracing::info!("{}", affect::describe(&applied));

                if applied.local {
                    host.refresh_screens();

                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x3B9 for entity {entity} arrived with no world up")
            }
        },
    }
}

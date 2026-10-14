use crate::scene::{Scene, minipopup, summon};
use formats::net::{field_fx, kept, siege};
use protocol::server::SummonRequest;

pub enum Packet {
    SummonRequest(SummonRequest),
    SummonCancel,
    SiegeState(siege::Siege),
    WarFlag(siege::WarFlag),
    FieldCarpet(field_fx::Carpet),
    FieldSound(field_fx::Sound),
    FieldLine(Box<kept::Line>),
    KeyedRecord(Box<kept::Record>),
}

pub trait Host {
    fn scene(&mut self) -> &mut Scene;

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet) {
    match packet {
        Packet::SummonRequest(request) => match host.scene().summon_requested(&request) {
            Some(notice) => {
                if summon_notice(host.scene(), notice) {
                    host.redraw();
                }

                tracing::info!(
                    "NWYD: 0x3B2 summon request retained from {:?}",
                    request.requester
                );
            }
            None => {
                tracing::info!("NWYD: 0x3B2 summon request arrived with no world up")
            }
        },
        Packet::SummonCancel => match host.scene().summon_cancelled() {
            Some(notice) => {
                if summon_notice(host.scene(), notice) {
                    host.redraw();
                }

                tracing::info!("NWYD: 0x3B3 summon notice cancelled");
            }
            None => {
                tracing::info!("NWYD: 0x3B3 summon cancel arrived with no world up")
            }
        },
        Packet::SiegeState(frame) => match host.scene().siege_state(&frame) {
            Some(applied) => {
                tracing::info!("{}", crate::scene::siege::describe(&applied))
            }
            None => {
                tracing::info!("NWYD: 0x3A8 castle siege arrived with no world up")
            }
        },
        Packet::WarFlag(flag) => match host.scene().war_flag(flag) {
            Some(applied) => {
                tracing::info!("{}", crate::scene::siege::describe_war(&applied))
            }
            None => {
                tracing::info!(
                    "NWYD: 0x3AC war flag {:#010x} arrived with no world up",
                    flag.flag,
                )
            }
        },
        Packet::FieldCarpet(packet) => match host.scene().field_carpet(&packet) {
            Some(carried) => {
                tracing::info!("{}", crate::scene::field_fx::describe(&carried));

                if carried.raised() > 0 {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x3a2 explosion carpet arrived with no world up")
            }
        },
        Packet::FieldSound(packet) => match host.scene().field_sound(&packet) {
            Some(carried) => {
                tracing::info!("{}", crate::scene::field_fx::describe(&carried))
            }
            None => tracing::info!("NWYD: 0x3a3 field sound arrived with no world up"),
        },
        Packet::FieldLine(line) => match host.scene().field_line(&line) {
            Some(slot) => {
                if let Some(lines) = host.scene().lines() {
                    tracing::info!("{}", crate::scene::kept::describe_line(lines, slot));
                }
            }
            None => {
                tracing::info!("NWYD: 0x2c8 field text line arrived with no world up")
            }
        },
        Packet::KeyedRecord(record) => match host.scene().keyed_record(*record) {
            Some(slot) => {
                if let Some(records) = host.scene().records() {
                    tracing::info!("{}", crate::scene::kept::describe_record(records, slot),);
                }
            }
            None => {
                tracing::info!("NWYD: 0xdc3 keyed record arrived with no world up")
            }
        },
    }
}

fn summon_notice(scene: &mut Scene, notice: summon::Notice) -> bool {
    let Scene::Field(field) = scene else {
        return false;
    };

    let Some(panels) = field.panels.as_mut() else {
        return false;
    };

    if panels.screen(minipopup::CONTENT).is_none() {
        return false;
    }

    let on = match notice {
        summon::Notice::Raise => {
            panels.summoner.clone_from(&field.summon.requester);

            true
        }
        summon::Notice::Clear => false,
    };

    minipopup::set(panels, minipopup::Notice::Summon, on);

    true
}

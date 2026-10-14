use crate::scene::{Scene, body_plate, challenge, guild, guild_invite, interact, live, party};
use formats::net::{
    challenge::Challenge,
    guild::Membership,
    guild_invite::Invite,
    party::{Leave, Marks, Member},
    plate::{GuildLine, LevelChange},
};

pub enum Packet {
    Member { opcode: u16, member: Member },
    Left(Leave),
    Marks(Marks),
    Guild(Membership),
    Invite { entity: u16, invite: Invite },
    Application,
    Challenge { entity: u16, challenge: Challenge },
    Level(LevelChange),
    GuildLine(GuildLine),
    NameMark { entity: u16, mode: i32 },
}

pub trait Host {
    fn scene(&mut self) -> &mut Scene;

    fn refresh_party(&mut self);

    fn refresh_screens(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet) {
    match packet {
        Packet::Member { opcode, member } => match host.scene().party_member(opcode, &member) {
            Some(applied) => {
                tracing::info!("{}", applied.describe());

                host.refresh_party();

                host.refresh_screens();

                host.redraw();
            }
            None => {
                tracing::info!("NWYD: {opcode:#06X} party row arrived with no world up")
            }
        },
        Packet::Left(leave) => match host.scene().party_left(&leave) {
            Some(applied) => {
                tracing::info!("{}", party::describe_leaving(&applied));

                host.refresh_party();

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x37E arrived with no world up"),
        },
        Packet::Marks(marks) => match host.scene().party_marks(&marks) {
            Some(applied) => {
                tracing::info!("{}", party::describe_marked(&applied));

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x3EA arrived with no world up"),
        },
        Packet::Guild(membership) => match host.scene().guild(&membership) {
            Some(applied) => {
                tracing::info!("{}", guild::describe(&applied));

                if applied.effect {
                    host.redraw();
                }
            }
            None => {
                tracing::info!(
                    "NWYD: 0x3E1C guild {} arrived with no local player in the world",
                    membership.guild
                )
            }
        },
        Packet::Invite { entity, invite } => match host.scene().guild_invite(entity, &invite) {
            Some(applied) => {
                tracing::info!("{}", guild_invite::describe(&applied));

                if applied.slot.is_some() {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x3D5 for entity {entity} arrived with no world up")
            }
        },
        Packet::Application => match host.scene().guild_application() {
            Some(raised) => {
                tracing::info!("{}", interact::describe_guild_application(&raised));

                if raised.slot.is_some() {
                    host.redraw();
                }
            }
            None => tracing::info!("NWYD: 0x18D arrived with no world up"),
        },
        Packet::Challenge { entity, challenge } => {
            match host.scene().challenge(entity, &challenge) {
                Some(applied) => {
                    tracing::info!("{}", challenge::describe(&applied));

                    if applied.slot.is_some() {
                        host.redraw();
                    }
                }
                None => {
                    tracing::info!("NWYD: 0x39F for entity {entity} arrived with no world up")
                }
            }
        }
        Packet::Level(change) => match host.scene().plate_level(&change) {
            Some(applied) => {
                tracing::info!("{}", body_plate::describe_level(&applied));

                if applied.refreshed_screens {
                    host.refresh_screens();
                }

                if applied.index.is_some() && !applied.refused_removal {
                    host.redraw();
                }
            }
            None => {
                tracing::info!(
                    "NWYD: 0x2F7 for entity {} arrived with no world up",
                    change.entity
                )
            }
        },
        Packet::GuildLine(line) => match host.scene().plate_guild(&line) {
            Some(applied) => {
                tracing::info!("{}", body_plate::describe_guild(&applied));

                if applied.index.is_some() && !applied.refused_removal {
                    host.redraw();
                }
            }
            None => {
                tracing::info!(
                    "NWYD: 0x2F8 for entity {} arrived with no world up",
                    line.entity
                )
            }
        },
        Packet::NameMark { entity, mode } => match host.scene().name_mark(entity, mode) {
            Some(applied) => {
                tracing::info!("{}", live::describe_name_mark(&applied));

                if applied.set || (applied.index.is_some() && mode == 4) {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x193 for entity {entity} arrived with no world up")
            }
        },
    }
}

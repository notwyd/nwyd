use super::field::FieldScene;
use formats::net::guild::{Command, EFFECT_KIND, Membership, PANEL_CONTENT, SOUND_SLOT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Ping {
    Guildless,
    UpdateConn {
        charidx: u32,
        report_channel: bool,
        conn: i32,
    },
}

impl Ping {
    pub fn connection_report(self) -> Option<super::guild_panel::ConnectionReport> {
        match self {
            Ping::Guildless => None,
            Ping::UpdateConn {
                charidx,
                report_channel,
                conn,
            } => Some(super::guild_panel::ConnectionReport {
                charidx,
                report_channel,
                channel: conn,
            }),
        }
    }
}

pub fn offline_connection_report(
    guild_panel_materialized: bool,
    guild: u16,
    charidx: Option<u32>,
) -> Option<super::guild_panel::ConnectionReport> {
    if !guild_panel_materialized || guild == 0 {
        return None;
    }

    Some(super::guild_panel::ConnectionReport {
        charidx: charidx?,
        report_channel: false,
        channel: 0,
    })
}

pub fn report_offline(scene: &mut FieldScene) -> Option<super::guild_panel::ConnectionReport> {
    refresh_session(scene);

    let materialised = scene
        .panels
        .as_ref()
        .is_some_and(|panels| panels.screen(super::guild_panel::CONTENT).is_some());

    let charidx = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .and_then(|actor| actor.charidx);

    let report = offline_connection_report(materialised, scene.entered.mob_guild, charidx)?;

    let panels = scene.panels.as_mut()?;

    panels.guild_update_conn(report);

    panels.guild_pump(&scene.entered);

    Some(report)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum PanelArm {
    NotReached,
    Refreshed,
    Closed,
    PanelShut { wanted: Command },
    Dropped,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Applied {
    pub command: Command,
    pub raw_command: i32,
    pub guild: u16,
    pub rank: u16,
    pub charidx: u32,
    pub mob_guild: u16,
    pub mob_guild_rank: u8,
    pub ping: Ping,
    pub effect: bool,
    pub sound: Option<i32>,
    pub panel: PanelArm,
}

pub fn changed(scene: &mut FieldScene, m: &Membership) -> Option<Applied> {
    let index = scene.player?;

    let actor = &mut scene.world.actors.actors[index];

    actor.guild = m.guild;

    actor.guild_rank = m.rank;

    actor.charidx = Some(m.charidx);

    let mob_guild = actor.guild;

    let mob_guild_rank = actor.guild_rank as u8;

    let position = actor.position;

    scene.entered.mob_guild = mob_guild;

    scene.entered.mob_guild_rank = mob_guild_rank;

    refresh_session(scene);

    let ping = match (
        m.pings_the_web_service(),
        scene.world.actors.actors[index].charidx,
    ) {
        (false, _) => Ping::Guildless,
        (true, None) => Ping::Guildless,
        (true, Some(charidx)) => Ping::UpdateConn {
            charidx,
            report_channel: true,
            conn: scene.session.channel,
        },
    };

    if let Some(report) = ping.connection_report()
        && let Some(panels) = scene.panels.as_mut()
    {
        panels.guild_update_conn(report);
    }

    let effect = m.raises_effect();

    let sound = match effect {
        true => {
            super::timers::spawn(scene, position, EFFECT_KIND);

            scene.world.play_sound(i64::from(SOUND_SLOT));

            Some(SOUND_SLOT)
        }
        false => None,
    };

    let command = m.command();

    let open = scene
        .panels
        .as_ref()
        .is_some_and(|p| p.is_open(PANEL_CONTENT));

    let panel = match command {
        Command::None => PanelArm::NotReached,
        Command::Other(_) => PanelArm::Dropped,
        Command::Refresh if open => PanelArm::Refreshed,
        Command::Close if open => PanelArm::Closed,
        wanted => PanelArm::PanelShut { wanted },
    };

    if let Some(panels) = scene.panels.as_mut() {
        match panel {
            PanelArm::Refreshed => panels.guild_membership_refresh(),
            PanelArm::Closed => {
                panels.set_open(PANEL_CONTENT, false);
            }
            _ => {}
        }
    }

    Some(Applied {
        command,
        raw_command: m.command,
        guild: m.guild,
        rank: m.rank,
        charidx: m.charidx,
        mob_guild,
        mob_guild_rank,
        ping,
        effect,
        sound,
        panel,
    })
}

pub(crate) fn refresh_session(scene: &mut FieldScene) {
    refresh_inputs(scene);

    if let Some(p) = scene.panels.as_mut() {
        p.guild_refresh_session();
    }
}

pub(crate) fn refresh_inputs(scene: &mut FieldScene) {
    let actor = scene.player.and_then(|i| scene.world.actors.actors.get(i));

    if let Some(p) = scene.panels.as_mut() {
        p.guild_identity.present = actor.is_some();

        p.field_body_guild = actor.map(|a| a.guild);

        if actor.is_none() {
            p.guild_live_session = None;
        }
    }

    let Some(actor) = actor else {
        return;
    };

    if let Some(panels) = scene.panels.as_mut() {
        panels.guild_live_session = Some(super::guild_panel::Session {
            guild: u32::from(scene.entered.mob_guild),
            rank: i32::from(actor.guild_rank as i16),
            charidx: actor.charidx,
            ally: u32::from(scene.siege.ally),
        });

        let identity = &mut panels.guild_identity;

        identity.character = actor.name.clone();

        identity.entity = actor.entity;

        identity.rank = i32::from(actor.guild_rank as i16);

        identity.slot = scene.entered.slot as i8;

        identity.server = scene.session.server as i32;

        identity.channel = scene.session.channel;
    }
}

pub fn describe(applied: &Applied) -> String {
    let mut line = format!(
        "NWYD: 0x3E1C — guild {}, rank {}, charidx {}, sub-command {}",
        applied.guild, applied.rank, applied.charidx, applied.raw_command,
    );

    if u16::from(applied.mob_guild_rank) != applied.rank {
        line.push_str(&format!(
            " (the mob record mirrors only the low byte, {})",
            applied.mob_guild_rank,
        ));
    }

    line.push_str(match applied.ping {
        Ping::Guildless => ", guildless so nothing is reported to the guild service",
        Ping::UpdateConn { .. } => ", connection reported",
    });

    if let Some(slot) = applied.sound {
        line.push_str(&format!(", with a level-up burst and sound {slot}"));
    }

    line.push_str(match applied.panel {
        PanelArm::NotReached => "",
        PanelArm::Refreshed => "; the guild window is refreshed",
        PanelArm::Closed => "; the guild window is closed",
        PanelArm::PanelShut {
            wanted: Command::Close,
        } => "; the guild window would have been closed, but it is not up",
        PanelArm::PanelShut { .. } => {
            "; the guild window would have been refreshed, but it is not up"
        }
        PanelArm::Dropped => "; and the sub-command reaches no panel arm",
    });

    line
}

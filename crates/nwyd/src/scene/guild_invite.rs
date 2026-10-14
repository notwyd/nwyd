use super::field::FieldScene;
use super::msgbox;
use formats::net::guild_invite::{self, Invite};
use ui::Strings;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Raised {
    pub entity: u16,
    pub routed: bool,
    pub subject: u32,
    pub known: bool,
    pub slot: Option<usize>,
    pub body: String,
}

pub fn question(strings: &Strings, guild: &str) -> String {
    let format = strings.translate(&format!("ss_{}", guild_invite::BODY_STRDEF));

    match format.find("%s") {
        Some(at) => format!("{}{guild}{}", &format[..at], &format[at + 2..]),
        None => format,
    }
}

pub fn request(strings: &Strings, guild: &str) -> msgbox::Request {
    msgbox::Request {
        caption: Some(guild_invite::CAPTION_STRDEF),
        kind: guild_invite::KIND,
        context: guild_invite::CONTEXT,
        body: msgbox::Body::Text(question(strings, guild)),
        flags: guild_invite::FLAGS,
    }
}

pub fn raise(scene: &mut FieldScene, entity: u16, invite: &Invite) -> Raised {
    let routed = scene.world.actors.entity(entity).is_some();

    let subject = invite.subject as u16;

    let known =
        u32::from(subject) == invite.subject && scene.world.actors.entity(subject).is_some();

    let mut out = Raised {
        entity,
        routed,
        subject: invite.subject,
        known,
        slot: None,
        body: String::new(),
    };

    if !routed || !known {
        return out;
    }

    let Some(panels) = scene.panels.as_mut() else {
        return out;
    };

    let request = request(&panels.strings, &invite.guild);

    out.body = match &request.body {
        msgbox::Body::Text(text) => text.clone(),
        other => other.text(&panels.strings),
    };

    out.slot = panels.raise_entity_box(&request, subject);

    out
}

pub fn reply_frame(subject: u16) -> Vec<u8> {
    let mut out = super::interact::frame(
        protocol::server::GUILD_INVITE,
        guild_invite::REPLY_LEN,
        u32::from(subject),
    );

    let at = protocol::header::LEN + 4;

    out[at..at + 4].copy_from_slice(&guild_invite::ACCEPT.to_le_bytes());

    out
}

pub fn describe(applied: &Raised) -> String {
    if !applied.routed {
        return format!(
            "NWYD: 0x3D5 for entity {} — no body in this world carries that id, so the entity callback dispatched nothing",
            applied.entity,
        );
    }

    if !applied.known {
        return format!(
            "NWYD: 0x3D5 — entity {} named subject {}, which no body carries",
            applied.entity, applied.subject,
        );
    }

    match applied.slot {
        None => {
            format!(
                "NWYD: 0x3D5 — a guild invitation from {} with nowhere to put it",
                applied.subject,
            )
        }
        Some(slot) => {
            format!(
                "NWYD: 0x3D5 — guild invitation from {} in box slot {slot}: {:?}",
                applied.subject, applied.body,
            )
        }
    }
}

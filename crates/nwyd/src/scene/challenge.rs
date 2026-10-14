use super::field::FieldScene;
use super::msgbox;
use formats::net::challenge::{self, Challenge};
use ui::Strings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raised {
    pub entity: u16,
    pub routed: bool,
    pub subject: u32,
    pub known: bool,
    pub stake: i32,
    pub ratio: Option<&'static str>,
    pub slot: Option<usize>,
    pub body: String,
}

pub fn question(strings: &Strings, name: &str, ratio: &str) -> String {
    let format = strings.translate(&format!("ss_{}", challenge::BODY_STRDEF));

    let tail = strings.translate(&format!("ss_{}", challenge::TAIL_STRDEF));

    format!(
        "{}{}{tail}",
        substitute(&format, [name, ratio]),
        ui::text::FF_BREAK
    )
}

pub fn substitute(format: &str, arguments: [&str; 2]) -> String {
    let room = format.len() + arguments.iter().map(|a| a.len()).sum::<usize>();

    let mut out = String::with_capacity(room);

    let mut rest = format;

    for argument in arguments {
        let Some(at) = rest.find("%s") else {
            break;
        };

        out.push_str(&rest[..at]);

        out.push_str(argument);

        rest = &rest[at + 2..];
    }

    out.push_str(rest);

    out
}

pub fn request(strings: &Strings, name: &str, ratio: &str) -> msgbox::Request {
    msgbox::Request {
        caption: Some(challenge::CAPTION_STRDEF),
        kind: challenge::KIND,
        context: challenge::CONTEXT,
        body: msgbox::Body::Text(question(strings, name, ratio)),
        flags: challenge::FLAGS,
    }
}

pub fn raise(scene: &mut FieldScene, entity: u16, challenged: &Challenge) -> Raised {
    let routed = scene.world.actors.entity(entity).is_some();

    let subject = challenged.subject as u16;

    let index = (u32::from(subject) == challenged.subject)
        .then(|| scene.world.actors.entity(subject))
        .flatten();

    let mut out = Raised {
        entity,
        routed,
        subject: challenged.subject,
        known: index.is_some(),
        stake: challenged.stake,
        ratio: challenged.ratio(),
        slot: None,
        body: String::new(),
    };

    let Some(index) = index else {
        return out;
    };

    if !routed {
        return out;
    }

    let Some(ratio) = out.ratio else {
        return out;
    };

    let name = scene.world.actors.actors[index].name.clone();

    let Some(panels) = scene.panels.as_mut() else {
        return out;
    };

    let request = request(&panels.strings, &name, ratio);

    out.body = match &request.body {
        msgbox::Body::Text(text) => text.clone(),
        other => other.text(&panels.strings),
    };

    out.slot = panels.raise_entity_box(&request, subject);

    out
}

pub fn reply_frame(subject: u16) -> Vec<u8> {
    let mut out = super::interact::frame(
        protocol::server::CHALLENGE,
        challenge::REPLY_LEN,
        u32::from(subject),
    );

    let at = protocol::header::LEN + 4;

    out[at..at + 4].copy_from_slice(&challenge::ACCEPT.to_le_bytes());

    out
}

pub fn describe(applied: &Raised) -> String {
    if !applied.routed {
        return format!(
            "NWYD: 0x39F for entity {} — no body in this world carries that id, so the entity callback dispatched nothing",
            applied.entity,
        );
    }

    if !applied.known {
        return format!(
            "NWYD: 0x39F — entity {} named subject {}, which no body carries",
            applied.entity, applied.subject,
        );
    }

    let Some(ratio) = applied.ratio else {
        return format!(
            "NWYD: 0x39F — subject {} offered stake {}, which indexes off the front of the \
             four-row table (D-471), so no box was raised",
            applied.subject, applied.stake,
        );
    };

    match applied.slot {
        None => {
            format!(
                "NWYD: 0x39F — a {ratio} challenge from {} with nowhere to put it",
                applied.subject,
            )
        }
        Some(slot) => {
            format!(
                "NWYD: 0x39F — {ratio} challenge from {} in box slot {slot}: {:?}",
                applied.subject, applied.body,
            )
        }
    }
}

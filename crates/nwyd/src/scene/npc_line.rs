use super::field::FieldScene;
use formats::net::npc_line::{self, NpcLine};
use formats::text::string_table::StringTable;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Applied {
    pub entity: u16,
    pub index: Option<usize>,
    pub kind: u8,
    pub quest_index: i16,
    pub raw: String,
    pub posted: Option<String>,
    pub balloon: Option<(String, u32)>,
}

fn de_underscore(s: &str) -> String {
    s.replace('_', " ")
}

fn resolve(table: &StringTable, index: i16) -> String {
    let text = match index >= 0 {
        true => table.get(index as usize),
        false => "",
    };

    match text.is_empty() {
        true => index.to_string(),
        false => text.to_string(),
    }
}

pub fn apply(scene: &mut FieldScene, entity: u16, line: &NpcLine) -> Applied {
    let miss = |raw: String| Applied {
        entity,
        index: None,
        kind: line.kind,
        quest_index: line.index,
        raw,
        posted: None,
        balloon: None,
    };

    let Some(index) = scene.world.actors.entity(entity) else {
        return miss(String::new());
    };

    let raw = match &scene.quest_strings {
        Some(table) => resolve(table, line.index),
        None => line.index.to_string(),
    };

    if line.kind != NpcLine::CHAT {
        let now_ms = scene.time.now_ms;

        let Some(panels) = scene.panels.as_mut() else {
            return Applied {
                index: Some(index),
                ..miss(raw)
            };
        };

        super::notice::post(
            panels,
            &raw,
            npc_line::NOTICE_LIFE_MS,
            npc_line::NOTICE_SOUND,
            now_ms,
        );

        return Applied {
            entity,
            index: Some(index),
            kind: line.kind,
            quest_index: line.index,
            posted: Some(raw.clone()),
            raw,
            balloon: None,
        };
    }

    let long = raw.as_bytes().first() == Some(&npc_line::BALLOON_LONG_PREFIX);

    let suppress = raw
        .as_bytes()
        .first()
        .is_some_and(|b| npc_line::BALLOON_SUPPRESS.contains(b));

    let body = match long {
        true => raw[1..].to_string(),
        false => raw.clone(),
    };

    let life_ms = match long {
        true => npc_line::BALLOON_LONG_MS,
        false => npc_line::BALLOON_SHORT_MS,
    };

    let name = scene.world.actors.actors[index].name.clone();

    let echo = de_underscore(&format!("[{name}]> {body}"));

    let Some(panels) = scene.panels.as_mut() else {
        return Applied {
            index: Some(index),
            ..miss(raw)
        };
    };

    super::chat::system(panels, &echo, super::chat::SAY_COLOR);

    let balloon = match suppress {
        true => None,
        false => {
            super::balloon::raise(scene, entity, &body, life_ms).then(|| (body.clone(), life_ms))
        }
    };

    Applied {
        entity,
        index: Some(index),
        kind: line.kind,
        quest_index: line.index,
        raw,
        posted: Some(echo),
        balloon,
    }
}

pub fn describe(applied: &Applied) -> String {
    match (applied.index, &applied.posted) {
        (None, _) => {
            format!(
                "NWYD: 0x107 for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        (Some(_), None) => {
            format!(
                "NWYD: 0x107 — entity {}, quest string {}, and no HUD to put it on",
                applied.entity, applied.quest_index,
            )
        }
        (Some(_), Some(posted)) => {
            format!(
                "NWYD: 0x107 — entity {}, kind {}, quest string {}, {posted:?}{}",
                applied.entity,
                applied.kind,
                applied.quest_index,
                match &applied.balloon {
                    Some((_, life_ms)) => format!(" + a {life_ms}ms balloon"),
                    None => String::new(),
                },
            )
        }
    }
}

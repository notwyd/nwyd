use assets::GameDir;
use formats::text::{Codepage, strdef};
use protocol::server::{self, ChatLine};

const RECORDS: [usize; 3] = [1462, 1562, 1584];

pub(crate) fn is_record(record: usize) -> bool {
    RECORDS.contains(&record)
}

#[derive(Default)]
pub(crate) struct Filter {
    templates: Vec<Template>,
}

impl Filter {
    pub(crate) fn load(game: &GameDir) -> Self {
        let mut templates = Vec::new();
        for language in crate::game_settings::available_languages(game) {
            let path = format!("Lang/{}/strdef.bin", language.dir());
            let Ok(bytes) = game.read(path.as_bytes()) else {
                continue;
            };
            let table = strdef::parse(&bytes, Codepage::for_language(language));
            for record in RECORDS {
                if let Some(template) = Template::new(record, table.get(record))
                    && !templates.contains(&template)
                {
                    templates.push(template);
                }
            }
        }
        Self { templates }
    }

    pub(crate) fn matches(&self, line: &ChatLine) -> bool {
        match line.opcode {
            server::SYSTEM_MESSAGE | server::SYSTEM_MESSAGE_ARG => {
                line.strdef().is_some_and(is_record)
            }
            server::CHAT_CONSOLE if line.speaker == 0 => self
                .templates
                .iter()
                .any(|template| template.matches(&line.text)),
            _ => false,
        }
    }
}

#[derive(PartialEq, Eq)]
struct Template {
    parts: Vec<String>,
    optional_npc: bool,
}

impl Template {
    fn new(record: usize, text: &str) -> Option<Self> {
        let parts: Vec<_> = text.trim().trim_end_matches('!').split("%s").collect();
        if !(2..=7).contains(&parts.len())
            || parts[1..parts.len() - 1].iter().any(|part| part.is_empty())
            || parts.iter().any(|part| part.contains('%'))
            || !parts
                .iter()
                .any(|part| part.chars().any(char::is_alphabetic))
        {
            return None;
        }
        Some(Self {
            optional_npc: record == 1584 && parts.len() == 4,
            parts: parts.into_iter().map(str::to_owned).collect(),
        })
    }

    fn matches(&self, text: &str) -> bool {
        let text = text.trim().trim_end_matches('!');
        let Some(rest) = text.strip_prefix(&self.parts[0]) else {
            return false;
        };
        let Some(mut rest) = rest.strip_suffix(self.parts.last().unwrap()) else {
            return false;
        };
        let literals = &self.parts[1..self.parts.len() - 1];
        for (index, literal) in literals.iter().enumerate() {
            let Some((at, _)) = rest
                .match_indices(literal)
                .find(|(at, _)| !rest[..*at].trim().is_empty())
            else {
                return self.optional_npc
                    && index + 1 == literals.len()
                    && rest
                        .strip_suffix(literal.trim_end())
                        .is_some_and(|field| !field.trim().is_empty());
            };
            rest = &rest[at + literal.len()..];
        }
        self.optional_npc || !rest.trim().is_empty()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scene::login_panels::language_tests::translated;

    pub(crate) fn game() -> GameDir {
        GameDir::from_files(
            "synthetic-announcements",
            [
                (
                    "Lang/EN/strdef.bin".into(),
                    translated(
                        &[
                            (1462, "%s refined an item to +%s!!"),
                            (1562, "%s reinforced %s to +%s"),
                            (1584, "%s obtained the item %s at the %s!!"),
                            (1176, "Your refinement succeeded"),
                        ],
                        "",
                    )
                    .strdef
                    .unwrap()
                    .to_bytes(),
                ),
                (
                    "Lang/PT/strdef.bin".into(),
                    translated(&[(1584, "%s obteve o item %s no %s")], "")
                        .strdef
                        .unwrap()
                        .to_bytes(),
                ),
            ],
        )
        .unwrap()
    }

    fn console(text: &str) -> ChatLine {
        ChatLine {
            opcode: server::CHAT_CONSOLE,
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn literal_announcements_match_all_installed_languages_and_punctuation() {
        let filter = Filter::load(&game());
        for text in [
            "Synthetic obtained the item Synthetic Sword at the Synthetic NPC!",
            "Synthetic obtained the item Synthetic Sword at the Synthetic NPC!!",
            "Synthetic obteve o item Espada Sintética no NPC Sintético",
            "Synthetic refined an item to +15!!",
            "Synthetic reinforced Synthetic Sword to +15",
        ] {
            assert!(filter.matches(&console(text)), "{text}");
        }
    }

    #[test]
    fn item_obtained_announcements_allow_a_missing_npc() {
        let filter = Filter::load(&game());
        for text in [
            "Synthetic obtained the item Synthetic Sword at the ",
            "Synthetic obteve o item Espada Sintética no ",
        ] {
            for punctuation in ["", "!", "!!"] {
                let text = format!("{text}{punctuation}");
                assert!(filter.matches(&console(&text)), "{text}");
            }
        }
    }

    #[test]
    fn literal_filter_preserves_other_messages_and_player_chat() {
        let filter = Filter::load(&game());
        for text in [
            "Your refinement succeeded",
            "Server maintenance in five minutes",
            "Synthetic obtained the item at the Synthetic NPC!",
            "Synthetic obtained the item  at the !!",
            " obtained the item Synthetic Sword at the !!",
            "Synthetic refined an item to +!!",
            "Synthetic reinforced Synthetic Sword to +!!",
            "obtained the item Synthetic Sword at the Synthetic NPC!",
            "!!120000",
            "!#26120",
            "Guild Chatting : Off",
        ] {
            assert!(!filter.matches(&console(text)), "{text}");
        }
        for text in [
            "Synthetic obtained the item Synthetic Sword at the Synthetic NPC!!",
            "Synthetic obtained the item Synthetic Sword at the !!",
        ] {
            for opcode in [server::CHAT_SAY, server::CHAT_SHOUT, server::CHAT_CHANNEL] {
                assert!(!filter.matches(&ChatLine {
                    opcode,
                    ..console(text)
                }));
            }
            assert!(!filter.matches(&ChatLine {
                speaker: 37,
                ..console(text)
            }));
        }
    }

    #[test]
    fn canned_chat_filters_only_announcement_records() {
        for opcode in [server::SYSTEM_MESSAGE, server::SYSTEM_MESSAGE_ARG] {
            for record in [1462, 1562, 1584, 1176, 1563, 1614] {
                let line = ChatLine {
                    opcode,
                    speaker: 37,
                    strdef_code: Some(record - 1000),
                    ..Default::default()
                };
                assert_eq!(
                    Filter::default().matches(&line),
                    matches!(record, 1462 | 1562 | 1584)
                );
            }
        }
    }

    #[test]
    fn templates_require_literals_nonempty_fields_and_the_whole_sentence() {
        for text in [
            "",
            "%s",
            "%s %s",
            "%s%s earned %s",
            "%d earned %s",
            "Synthetic reward",
        ] {
            assert!(Template::new(1584, text).is_none(), "{text}");
        }
        let template = Template::new(1584, "[Reward] %s obtained %s.").unwrap();
        assert!(template.matches("[Reward] Jogador Sintético obtained Espada Épica."));
        for text in [
            "prefix [Reward] Synthetic obtained Sword.",
            "[Reward] Synthetic obtained Sword. suffix",
            "[Reward]  obtained Sword.",
            "[Reward] Synthetic obtained .",
        ] {
            assert!(!template.matches(text), "{text}");
        }
    }
}

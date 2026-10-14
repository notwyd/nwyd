use super::*;

pub(crate) struct LanguageUpdate {
    game: GameDir,
    strings: ui::Strings,
    panels: Option<panels::LanguageUpdate>,
    effects: hud::EffectStrings,
    labels: formats::world::minimap::Minimap,
    label_codepage: Codepage,
    quest_strings: Option<formats::text::string_table::StringTable>,
    text: super::super::stext::SceneText,
}

impl LanguageUpdate {
    pub(crate) fn prepare(
        scene: &FieldScene,
        game: &GameDir,
        strings: &ui::Strings,
    ) -> Result<Self> {
        let panels = scene
            .panels
            .as_ref()
            .map(|panels| panels::LanguageUpdate::prepare(panels, game, strings))
            .transpose()?;

        let (labels, label_codepage) = minimap::read_labels(game)?;

        let chains = crate::fonts::load(game, crate::fonts::system_font_dir().as_deref());

        Ok(Self {
            game: game.clone(),
            strings: strings.clone(),
            panels,
            effects: hud::EffectStrings::load(game),
            labels,
            label_codepage,
            quest_strings: crate::ui::read_language(game, "QuestStringTable.txt")
                .ok()
                .map(|(raw, cp)| formats::text::string_table::quest::parse(&raw, cp)),
            text: super::super::stext::SceneText::new(&chains, scene.layout.width),
        })
    }
}

impl FieldScene {
    pub(crate) fn apply_language(&mut self, update: LanguageUpdate) {
        let LanguageUpdate {
            game,
            strings,
            panels,
            effects,
            labels,
            label_codepage,
            quest_strings,
            text,
        } = update;

        self.map_title_text = self
            .ui
            .records
            .iter()
            .find(|record| {
                record.control == ControlType::Text
                    && record.header.name_id == landmarks::TITLE_TEXT_NAME_ID
            })
            .and_then(|record| record.fields.get(10))
            .and_then(|index| usize::try_from(*index).ok())
            .map(|index| {
                super::super::stext::display_text(
                    strings.ui.as_ref().map_or("", |table| table.get(index)),
                )
            });

        let captions = [
            super::super::long_message::QUIZ_CAPTION,
            super::super::long_message::GUILD_CAPTION,
        ]
        .map(|index| {
            strings
                .strdef
                .as_ref()
                .map_or("", |table| table.get(index))
                .to_owned()
        });

        for (old, new) in self.long_message_captions.iter().zip(&captions) {
            if self.long_message.panel.caption == super::super::stext::display_text(old) {
                self.long_message.panel.caption = super::super::stext::display_text(new);

                break;
            }
        }

        if !self.long_message.panel.visible {
            self.long_message.panel.caption =
                super::super::stext::display_text(strings.ui.as_ref().map_or("", |table| {
                    table.get(super::super::long_message::INITIAL_CAPTION)
                }));
        }

        self.long_message_captions = captions;

        self.drop_gold_caption = strings
            .strdef
            .as_ref()
            .map_or("", |table| table.get(65))
            .to_owned();

        self.quest_strings = quest_strings;

        let prefix = strings.translate(&format!("ss_{}", formats::data::guild_mark::URL_RECORD));

        self.marks.prefix = if prefix.starts_with("ss_") {
            String::new()
        } else {
            prefix
        };

        self.minimap.labels = labels;

        self.minimap.label_codepage = label_codepage;

        self.text = text;

        self.legacy_boxes.relocalize(&game, &strings);

        if let Some(hud) = self.hud.as_mut() {
            hud.relocalize(&game, &strings, effects);
        }

        if let (Some(panels), Some(update)) = (self.panels.as_mut(), panels) {
            panels.apply_language(update);

            screens::refresh_language(
                panels,
                &self.entered,
                self.mission.as_ref(),
                &self.collection,
            );
        }

        self.message_strings = strings;

        let original = self
            .panels
            .as_ref()
            .and_then(|panels| panels.screen(panels::CONTENT_MOVE_SERVER))
            .map(|screen| screen.placed.ui.panel.clone());

        if let Some(original) = original {
            super::refresh_channel_popup(self);

            if let Some(screen) = self
                .panels
                .as_mut()
                .and_then(|panels| panels.screen_mut(panels::CONTENT_MOVE_SERVER))
            {
                let translated = std::mem::replace(&mut screen.placed.ui.panel, original);

                screens::copy_language_text(
                    &mut screen.placed.ui.panel.elements,
                    &translated.elements,
                );
            }
        }
    }
}

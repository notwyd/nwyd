use super::*;

pub(in crate::scene) struct LanguageUpdate {
    game: GameDir,
    strings: Strings,
    help: Vec<Vec<screens::ColorLine>>,
    quests: Vec<Vec<screens::Quest>>,
    items: ItemList,
    missions: Option<mission::Tables>,
    mix_help: super::super::mix::HelpTables,
    item_help: Vec<formats::data::item_help::Help>,
    skill_master_desc: String,
    potal_pos: Vec<formats::text::potal_pos::Row>,
}

impl LanguageUpdate {
    pub(in crate::scene) fn prepare(
        panels: &Panels,
        game: &GameDir,
        strings: &Strings,
    ) -> Result<Self> {
        let base = super::super::item_table::load(game)?;

        Ok(Self::prepare_with_base(panels, game, strings, &base))
    }

    fn prepare_with_base(
        panels: &Panels,
        game: &GameDir,
        strings: &Strings,
        base: &ItemList,
    ) -> Self {
        let mut items = panels.items.clone();

        items.reset_names_from(base);

        if let Ok((raw, cp)) = crate::ui::read_language(game, ITEM_NAME) {
            items.apply_names_with_codepage(&formats::data::item_name::parse(&raw), cp);
        }

        let missions = panels.missions.as_ref().map(|previous| {
            let (help, names) = crate::ui::read_language(game, MISSION_HELP)
                .map(|(raw, cp)| formats::data::item_help::parse_mission(&raw, cp))
                .unwrap_or_else(|_| {
                    (
                        vec![Default::default(); formats::data::item_help::MISSION_RECORDS],
                        vec![String::new(); formats::data::item_help::MISSION_RECORDS],
                    )
                });

            let mut overrides = Default::default();

            if let Ok(raw) = game.read(b"MItem.txt") {
                formats::data::mitem::overlay(&mut overrides, &raw);
            }

            if let Ok((raw, cp)) = crate::ui::read_language(game, "MItem.txt") {
                formats::data::mitem::overlay_with_codepage(&mut overrides, &raw, cp);
            }

            let mut tables = mission::Tables {
                items: previous.items.clone(),
                help,
                names,
                overrides,
            };

            tables.fill_missing_help(&items, &panels.icons, strings);

            tables
        });

        let (raw, cp) =
            crate::ui::read_language(game, super::super::mix::HELP_SCRIPT).unwrap_or_default();

        let (help, names) = formats::data::item_help::parse_mix(&raw, cp);

        Self {
            game: game.clone(),
            strings: strings.clone(),
            help: screens::load_help(game),
            quests: screens::load_quests(game),
            items,
            missions,
            mix_help: super::super::mix::HelpTables { help, names },
            item_help: crate::ui::load_item_help(game).unwrap_or_default(),
            skill_master_desc: crate::ui::read_language(game, skill_master::DESC_FILE)
                .map(|(raw, cp)| ui::text::decode_colored_text_records(&raw, cp))
                .unwrap_or_default(),
            potal_pos: crate::ui::read_language(game, "PotalPos.txt")
                .map(|(raw, cp)| formats::text::potal_pos::parse(&raw, cp))
                .unwrap_or_default(),
        }
    }
}

impl Panels {
    pub(in crate::scene) fn apply_language(&mut self, update: LanguageUpdate) {
        let LanguageUpdate {
            game,
            strings,
            help,
            quests,
            items,
            missions,
            mix_help,
            item_help,
            skill_master_desc,
            potal_pos,
        } = update;

        for screen in &mut self.screens {
            screen.placed.ui.relocalize(&game, &strings);

            if let Some(base) = screen.guild_popup_base.as_mut() {
                base.relocalize(&strings);
            }
        }

        self.msgbox.relocalize(&game, &strings);

        self.tooltip.relocalize(&game, &strings, item_help);

        self.hover_tip
            .for_each_ui(&mut |ui| ui.relocalize(&game, &strings));

        self.hover_tip.hide();

        self.carry
            .for_each_ui(&mut |ui| ui.relocalize(&game, &strings));

        self.hover_source = None;

        self.icon_hover_stamp = None;

        self.help = help;

        self.quests = quests;

        self.items = items;

        self.missions = missions;

        self.mix_help = mix_help;

        self.skill_master_desc = skill_master_desc;

        self.potal_pos = potal_pos;

        self.quest_game = Some(game.clone());

        self.tooltip_portuguese = game.language() == formats::config::Language::Portuguese;

        self.guild_host = super::super::guild_panel::web_host(&strings);

        self.strings = strings.clone();

        let coin_caption = match self.coin_action {
            4 if self.store_price.is_some() => Some(super::super::store::PRICE_STRDEF),
            6 if self.seal_name_slot.is_some() => Some(349),
            12 if self.split_stack.is_some() => Some(408),
            _ => None,
        };

        if let Some(record) = coin_caption
            && let Some(screen) = self.screen_mut(CONTENT_COIN_INPUT)
        {
            screens::text(screen, "msg", &strings.translate(&format!("ss_{record}")));
        }

        chat::relocalize(self, &strings);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use formats::{config::Language, text::Codepage};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);

            loop {
                let path = std::env::temp_dir().join(format!(
                    "nwyd-panel-language-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));

                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("creating synthetic test directory: {error}"),
                }
            }
        }

        fn write(&self, path: &str, data: &[u8]) {
            let path = self.0.join(path);

            fs::create_dir_all(path.parent().unwrap()).unwrap();

            fs::write(path, data).unwrap();
        }

        fn game(&self, language: Language) -> GameDir {
            let mut game = GameDir::open(&self.0).unwrap();

            game.set_language(language);

            game
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn language_refresh_preserves_live_panels_and_resets_missing_translated_item_names() {
        let root = Directory::new();

        root.write("Lang/PT/descskillm.txt", b"FFFFFF Descricao sintetica");

        root.write("Lang/EN/descskillm.txt", b"FFFFFF Synthetic description");

        root.write(
            "Lang/PT/itemname.bin",
            &formats::data::item_name::to_bytes(&[formats::data::item_name::Record {
                id: 3,
                name: b"Nome sintetico".to_vec(),
            }]),
        );

        let mut panels = super::super::language_tests::populated();

        let mut base = ItemList::from_plain(
            vec![0; formats::data::item_list::BODY_LEN],
            Codepage::Windows1252,
        );

        base.set_name(3, b"Synthetic base name");

        base.set_name(4, b"Synthetic fallback");

        panels.items.set_name(4, b"Previous language");

        let identity = panels.screens[0].placed.ui.identity();

        let origin = panels.screens[0].placed.origin;

        for language in [
            Language::Portuguese,
            Language::English,
            Language::Portuguese,
        ] {
            let game = root.game(language);

            let strings = crate::ui::strings(&game);

            let update = LanguageUpdate::prepare_with_base(&panels, &game, &strings, &base);

            panels.apply_language(update);

            super::super::language_tests::assert_preserved(&panels);

            assert_eq!(panels.screens[0].placed.ui.identity(), identity);

            assert_eq!(panels.screens[0].placed.origin, origin);

            assert_eq!(panels.items.name(4).as_deref(), Some("Synthetic fallback"));

            assert_eq!(panels.quest_game.as_ref().unwrap().language(), language);

            assert_eq!(panels.tooltip_portuguese, language == Language::Portuguese);

            match language {
                Language::Portuguese => {
                    assert_eq!(panels.items.name(3).as_deref(), Some("Nome sintetico"));

                    assert_eq!(panels.skill_master_desc, "Descricao sintetica<n>");
                }
                _ => {
                    assert_eq!(panels.items.name(3).as_deref(), Some("Synthetic base name"));

                    assert_eq!(panels.skill_master_desc, "Synthetic description<n>");
                }
            }
        }
    }

    #[test]
    fn language_refresh_preparation_failure_leaves_live_panels_untouched() {
        let root = Directory::new();

        let panels = super::super::language_tests::populated();

        let game = root.game(Language::English);

        assert!(LanguageUpdate::prepare(&panels, &game, &Strings::default()).is_err());

        super::super::language_tests::assert_preserved(&panels);

        assert!(panels.quest_game.is_none());
    }

    #[test]
    fn language_refresh_preserves_unhandled_chat_scroll_sentinel_and_history() {
        let mut panels = super::super::language_tests::populated();

        let resource = ui::Resource::default();

        let mut element = ui::Element::new(ElementType::ListView, "ListView", "chatLog", &resource);

        let list = element.widget.list_mut().unwrap();

        list.add_item("Synthetic previous message");

        list.selected = 0;

        list.items[0].selected = true;

        list.scroll = 0x00FF_FFFF;

        panels.screens[0].placed.ui.panel.elements.push(element);

        let packet =
            formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();

        let entered = crate::scene::enter::Entered::from_packet(&packet, 0, None);

        screens::refresh_language(&mut panels, &entered, None, &Default::default());

        super::super::language_tests::assert_preserved(&panels);

        let list = panels.screens[0]
            .element("chatLog", ElementType::ListView)
            .unwrap()
            .widget
            .list()
            .unwrap();

        assert_eq!(list.scroll, 0x00FF_FFFF);

        assert_eq!(list.selected, 0);

        assert!(list.items[0].selected);

        assert_eq!(list.items[0].text, "Synthetic previous message");
    }

    #[test]
    fn language_refresh_updates_coin_prompt_and_combine_cost_without_resetting_draft() {
        let root = Directory::new();

        let mut panels = Panels::for_test();

        let resource = ui::Resource::default();

        let mut coin = super::super::language_tests::input_screen();

        coin.content = CONTENT_COIN_INPUT;

        coin.script = b"synthetic_coin";

        coin.placed.ui.panel.elements = vec![
            ui::Element::new(ElementType::Static, "Static", "msg", &resource),
            ui::Element::new(ElementType::ImeEditBox, "ImeEditBox", "edit", &resource),
        ];

        let mut combine = super::super::language_tests::input_screen();

        combine.content = CONTENT_ITEM_MIX4;

        combine.script = b"synthetic_combine";

        combine.placed.ui.panel.elements = vec![ui::Element::new(
            ElementType::Static,
            "Static",
            "Money",
            &resource,
        )];

        panels.screens = vec![coin, combine];

        panels.strings = crate::scene::login_panels::language_tests::translated(
            &[(137, "Old amount prompt")],
            "278 Old_cost\n",
        );

        assert!(panels.begin_coin_money(2, 137));

        let edit = panels.screens[0]
            .element_mut("edit", ElementType::ImeEditBox)
            .unwrap()
            .widget
            .edit_mut()
            .unwrap();

        edit.text = "123456".into();

        edit.caret = 4;

        edit.anchor = 1;

        let identity = panels.screens[0].placed.ui.identity();

        let strings = crate::scene::login_panels::language_tests::translated(
            &[(137, "New amount prompt")],
            "278 New_cost\n",
        );

        let base = panels.items.clone();

        let update = LanguageUpdate::prepare_with_base(
            &panels,
            &root.game(Language::English),
            &strings,
            &base,
        );

        panels.apply_language(update);

        let packet =
            formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();

        let entered = crate::scene::enter::Entered::from_packet(&packet, 0, None);

        screens::refresh_language(&mut panels, &entered, None, &Default::default());

        assert_eq!(panels.coin_action, 2);

        assert_eq!(panels.screens[0].placed.ui.identity(), identity);

        assert_eq!(
            panels.screens[0]
                .element("msg", ElementType::Static)
                .unwrap()
                .display,
            "New amount prompt"
        );

        let edit = panels.screens[0]
            .element("edit", ElementType::ImeEditBox)
            .unwrap()
            .widget
            .edit()
            .unwrap();

        assert_eq!(edit.text, "123456");

        assert_eq!((edit.caret, edit.anchor), (4, 1));

        assert_eq!(
            panels.screens[1]
                .element("Money", ElementType::Static)
                .unwrap()
                .display,
            "New cost 30,000,000"
        );
    }
}

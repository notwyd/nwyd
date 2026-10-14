use super::*;
use crate::scene::login_panels::language_tests::{LanguageGame, translated};
use formats::config::Language;
use formats::data::{item_list, item_name};
use formats::text::{Codepage, codepage};

#[test]

fn runtime_language_notice_item_names_preserve_metadata_and_queued_messages() {
    let fixture = LanguageGame::new();

    let root = fixture.game.root();

    for (folder, name, codepage) in [
        ("KR", "합성 검", Codepage::Windows949),
        ("EN", "Synthetic sword", Codepage::Windows1252),
    ] {
        let path = root.join(format!("Lang/{folder}"));

        std::fs::create_dir_all(&path).unwrap();

        std::fs::write(
            path.join("itemname.bin"),
            item_name::to_bytes(&[item_name::Record {
                id: 3,
                name: codepage::encode(name, codepage),
            }]),
        )
        .unwrap();
    }

    let mut game = assets::GameDir::open(root).unwrap();

    let mut overlay = Overlay::for_test();

    overlay.base_items.set_name(3, b"Root item");

    overlay.base_items.set_name(4, b"Root fallback");

    overlay.items = overlay.base_items.clone();

    overlay.items.set_name(4, b"Old translation");

    overlay.items.set_price(3, 76543);

    let original = overlay.items.get(3).unwrap();

    overlay.post("Existing notice", 4000, true, 5000);

    overlay.post("Existing queued notice", 4000, false, 6000);

    let notice = overlay.state.clone();

    let old_display = overlay.placed.ui.panel.elements[0].display.clone();

    for (language, expected) in [
        (Language::Korean, "합성 검"),
        (Language::Portuguese, "Synthetic sword"),
        (Language::English, "Synthetic sword"),
    ] {
        game.set_language(language);

        let update = LanguageUpdate::prepare(&overlay, &game).unwrap();

        let reads = game.reads();

        overlay.apply_language(update);

        assert_eq!(game.reads(), reads);

        assert_eq!(overlay.items.name(3).as_deref(), Some(expected));

        assert_eq!(overlay.items.name(4).as_deref(), Some("Root fallback"));

        let mut item = overlay.items.get(3).unwrap();

        item.name = original.name.clone();

        assert_eq!(item, original);

        assert_eq!(overlay.state, notice);

        assert_eq!(overlay.placed.ui.panel.elements[0].display, old_display);
    }

    game.set_language(Language::Korean);

    let initial = localized_items(&overlay.base_items, &overlay.base_items, &game).unwrap();

    assert_eq!(initial.name(3).as_deref(), Some("합성 검"));

    overlay.apply_language(LanguageUpdate { items: initial });

    let record = super::super::chat::HANDLED_ELSEWHERE[0];

    let strings = translated(&[(record, "%s obtained %s level %s")], "");

    let result = overlay.announce(
        &strings,
        &protocol::server::SystemMessage {
            kind: super::super::chat::PLAIN,
            code: (record - protocol::server::STRDEF_BASE) as i16,
            argument: "3 9 Synthetic".into(),
            has_argument: true,
        },
        7000,
    );

    assert_eq!(result.text, "Synthetic obtained 합성 검 level 9");

    assert_eq!(overlay.base_items.name(3).as_deref(), Some("Root item"));

    assert_eq!(overlay.base_items.get(3).unwrap().price, 0);

    assert_eq!(overlay.base_items.name(4).as_deref(), Some("Root fallback"));
}

#[test]

fn runtime_language_notice_resets_to_root_names_without_localized_resources() {
    let fixture = LanguageGame::new();

    let mut overlay = Overlay::for_test();

    overlay.base_items.set_name(7, b"Synthetic base");

    overlay.items =
        item_list::ItemList::from_plain(vec![0; item_list::BODY_LEN], Codepage::Windows1252);

    overlay.items.set_name(7, b"Previous language");

    overlay.items.set_price(7, 777);

    let update = LanguageUpdate::prepare(&overlay, &fixture.game).unwrap();

    overlay.apply_language(update);

    assert_eq!(overlay.items.name(7).as_deref(), Some("Synthetic base"));

    assert_eq!(overlay.items.get(7).unwrap().price, 777);
}

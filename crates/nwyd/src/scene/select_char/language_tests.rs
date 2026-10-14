use super::*;
use crate::scene::login_panels::language_tests::placed;

pub(super) fn panels() -> Panels {
    let panes = [
        placed(&[("Delete", ElementType::Button)]),
        placed(&[]),
        placed(&[(CREATE_INPUT, ElementType::ImeEditBox)]),
        placed(&[(keypad::TITLE, ElementType::Static)]),
    ];

    let delete_owner = panes[0].ui.panel.elements[0].instance;

    Panels {
        panes,
        delete_owner,
        msgbox: msgbox::BoxPanel::for_test(),
        guimat_textures: Default::default(),
        missing: Vec::new(),
        notes: Vec::new(),
        manager: Manager::default(),
        strings: Strings::default(),
        clipboard: Default::default(),
    }
}

#[test]
fn characterization_character_caption_updates_preserve_draft_keypad_and_visibility() {
    let mut panels = panels();

    let input = panels.panes[PANE_CREATE].ui.panel.elements[0]
        .widget
        .edit_mut()
        .unwrap();

    input.text = "SyntheticHero".into();

    input.caret = 7;

    input.anchor = 3;

    input.scroll = 2;

    let draft = input.clone();

    panels.panes[PANE_CREATE].ui.panel.visible = true;

    panels.panes[PANE_SEL_INFO].ui.panel.visible = false;

    let pad = keypad::Pad {
        labels: [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
        typed: vec![1, 3],
        saved: vec![2, 4],
        next_press_ms: 4100,
    };

    let saved_pad = pad.clone();

    let pane = create::Pane {
        class: "Synthetic class".into(),
        desc: "Synthetic description".into(),
        stat: std::array::from_fn(|i| (i + 1).to_string()),
        gauge_width: [1, 2, 3, 4],
        gauge_height: create::GaugeHeight(8),
        scale: 1.0,
        action: 0,
        sounds: Vec::new(),
    };

    create::apply(&pane, &mut panels.panes[PANE_CREATE].ui);

    pad.label(&mut panels.panes[PANE_PASSWORD].ui);

    pad.title(&mut panels.panes[PANE_PASSWORD].ui, &panels.strings);

    panels.reanchor((800, 600));

    assert_eq!(panels.create_name(), "SyntheticHero");

    assert_eq!(
        panels.panes[PANE_CREATE].ui.panel.elements[0]
            .widget
            .edit()
            .unwrap(),
        &draft
    );

    assert_eq!(pad, saved_pad);

    assert!(panels.panes[PANE_CREATE].ui.panel.visible);

    assert!(!panels.panes[PANE_SEL_INFO].ui.panel.visible);
}

#[test]
fn characterization_selection_info_caption_updates_preserve_account_data() {
    let mut panels = panels();

    let mut block = Block::default();

    block.slots[1].name[..9].copy_from_slice(b"Synthetic");

    block.slots[1].equipment.items[0].index = 1;

    block.slots[1].status.level = 12;

    block.slots[1].gold = 12345;

    block.slots[1].exp = 67890;

    let original = block;

    let items = ItemList::from_plain(
        vec![0; formats::data::item_list::BODY_LEN],
        Codepage::Windows1252,
    );

    let info = sel_info::fill(&block, 1, &items, &panels.strings).unwrap();

    panels.panes[PANE_SEL_INFO].ui.panel.visible = true;

    sel_info::apply(&info, &mut panels.panes[PANE_SEL_INFO].ui);

    assert_eq!(block, original);

    assert_eq!(info.name, "Synthetic");

    assert_eq!(info.level, "13");

    assert_eq!(info.gold, "12,345");

    assert!(panels.panes[PANE_SEL_INFO].ui.panel.visible);

    assert_eq!(
        panels.panes[PANE_SEL_INFO]
            .ui
            .panel
            .element("Name", ElementType::Static)
            .unwrap()
            .display,
        "Synthetic"
    );
}

#[test]
fn characterization_shared_dialog_text_reflow_preserves_pending_inputs_and_messages() {
    use crate::scene::{character_name, legacy_message_box, message_panel};

    let layout = Layout::new(800, 600);

    let chains = Default::default();

    let mut dialog = character_name::Dialog::for_test();

    dialog.open();

    dialog.pending = true;

    dialog.prompt_required = true;

    dialog.requested_at_ms = 3200;

    dialog.edit.text = "NameDraft".into();

    dialog.edit.caret = 5;

    dialog.edit.anchor = 2;

    let edit = dialog.edit.clone();

    let mut legacy = legacy_message_box::Dialogs::for_test();

    legacy.show(
        legacy_message_box::Owner::Question,
        "Existing question",
        123,
        None,
    );

    let states = legacy.states.clone();

    let mut message = message_panel::Overlay::for_test();

    message
        .state
        .post_text("Existing message".into(), 4000, 3000);

    let message_state = message.state.clone();

    dialog.resize(&chains, &layout, &layout);

    legacy.resize(&chains, &layout, &layout);

    message.resize(&chains, 800);

    assert_eq!(dialog.edit, edit);

    assert!(dialog.visible && dialog.pending && dialog.prompt_required && dialog.has_focus());

    assert_eq!(dialog.requested_at_ms, 3200);

    assert_eq!(legacy.states, states);

    assert_eq!(legacy.active(), Some(legacy_message_box::Owner::Question));

    assert_eq!(message.state, message_state);
}

#[test]
fn runtime_language_preserves_character_draft_and_info_values() {
    use crate::scene::login_panels::language_tests::{LanguageGame, translated};

    let fixture = LanguageGame::new();

    let mut panels = panels();

    panels.panes[PANE_CREATE].ui.panel.elements[0]
        .widget
        .edit_mut()
        .unwrap()
        .text = "SyntheticHero".into();

    let draft = panels.panes[PANE_CREATE].ui.panel.elements[0]
        .widget
        .edit()
        .unwrap()
        .clone();

    panels.panes[PANE_CREATE].ui.panel.visible = true;

    panels.panes[PANE_SEL_INFO].ui.panel.visible = false;

    let mut block = Block::default();

    block.slots[1].name[..9].copy_from_slice(b"Synthetic");

    block.slots[1].equipment.items[0].index = 1;

    block.slots[1].status.level = 12;

    block.slots[1].gold = 12345;

    block.slots[1].exp = 67890;

    block.slots[1].x = 2100;

    block.slots[1].z = 2100;

    let items = ItemList::from_plain(
        vec![0; formats::data::item_list::BODY_LEN],
        Codepage::Windows1252,
    );

    let mut info = sel_info::fill(&block, 1, &items, &panels.strings).unwrap();

    let original = info.clone();

    let strings = translated(
        &[(121, "New class"), (125, "New town"), (242, "New mastery")],
        "",
    );

    panels.relocalize(&fixture.game, &strings);

    sel_info::relocalize(&mut info, &block, 1, &strings);

    sel_info::apply(&info, &mut panels.panes[PANE_SEL_INFO].ui);

    assert_eq!(info.class, "New class");

    assert_eq!(info.start, "New town");

    assert_eq!(info.mtext[0], "New mastery");

    assert_eq!(info.name, original.name);

    assert_eq!(info.level, original.level);

    assert_eq!(info.gold, original.gold);

    assert_eq!(info.exp, original.exp);

    assert_eq!(info.stat, original.stat);

    assert_eq!(info.master, original.master);

    assert_eq!(info.flag, original.flag);

    assert_eq!(info.gm_body_substitute, original.gm_body_substitute);

    assert_eq!(
        panels.panes[PANE_CREATE].ui.panel.elements[0]
            .widget
            .edit()
            .unwrap(),
        &draft
    );

    assert!(panels.panes[PANE_CREATE].ui.panel.visible);

    assert!(!panels.panes[PANE_SEL_INFO].ui.panel.visible);
}

#[test]
fn runtime_language_preserves_pending_dialogs_and_historical_messages() {
    use crate::scene::login_panels::language_tests::{LanguageGame, translated};
    use crate::scene::{character_name, legacy_message_box, message_panel};

    let fixture = LanguageGame::new();

    let strings = translated(&[(1, "New text")], "");

    let mut dialog = character_name::Dialog::for_test();

    dialog.open();

    dialog.pending = true;

    dialog.requested_at_ms = 3200;

    dialog.edit.text = "NameDraft".into();

    dialog.edit.caret = 5;

    dialog.edit.anchor = 2;

    let edit = dialog.edit.clone();

    let mut legacy = legacy_message_box::Dialogs::for_test();

    legacy.show(
        legacy_message_box::Owner::Question,
        "Existing question",
        123,
        None,
    );

    let states = legacy.states.clone();

    let mut message = message_panel::Overlay::for_test();

    message
        .state
        .post_text("Existing message".into(), 4000, 3000);

    let message_state = message.state.clone();

    dialog.relocalize(&fixture.game, &strings);

    legacy.relocalize(&fixture.game, &strings);

    message.relocalize(&fixture.game, &strings);

    assert_eq!(dialog.edit, edit);

    assert!(dialog.visible && dialog.pending && dialog.has_focus());

    assert_eq!(dialog.requested_at_ms, 3200);

    assert_eq!(legacy.states, states);

    assert_eq!(legacy.active(), Some(legacy_message_box::Owner::Question));

    assert_eq!(message.state, message_state);
}

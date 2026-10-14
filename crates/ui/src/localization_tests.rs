use crate::{Element, Panel, Resource, Strings};
use formats::{
    text::Codepage,
    ui::pane::{Block, ElementType, Pane, Property},
};

fn strings() -> Strings {
    Strings {
        ui: Some(crate::text::ui_strings(
            b"1 Original_title\n2 Original_caption\n3 Original_tooltip\n",
            Codepage::Windows1252,
        )),
        ..Default::default()
    }
}

fn translated_strings() -> Strings {
    Strings {
        ui: Some(crate::text::ui_strings(
            b"1 New_title\n2 New_caption\n3 New_tooltip\n",
            Codepage::Windows1252,
        )),
        ..Default::default()
    }
}

fn property(key: &str, value: &str) -> Property {
    Property {
        key: key.into(),
        value: value.into(),
    }
}

fn translated(kind: ElementType, name: &str) -> Element {
    let mut element = Element::new(kind, "synthetic", name, &Resource::default());

    element.id = 31;

    element.x = 12;

    element.y = 23;

    element.width = 150;

    element.height = 30;

    element.apply(
        &property("Caption", "uu_2"),
        &Resource::default(),
        &strings(),
    );

    element.set_tooltip("uu_3", &strings());

    element
}

fn panel() -> Panel {
    let pane = Pane {
        panel: Block {
            properties: vec![property("Caption", "uu_1"), property("CloseButton", "true")],
        },
        ..Default::default()
    };

    Panel::instantiate(&pane, &Resource::default(), &strings(), true)
}

fn retained_panel() -> Panel {
    let mut panel = panel();

    panel.left = 101;

    panel.top = 202;

    panel.right = 600;

    panel.bottom = 550;

    panel.visible = false;

    panel.focus = Some(1);

    panel
        .elements
        .push(translated(ElementType::CheckBox, "check"));

    panel.elements[0].widget.button_mut().unwrap().checked = true;

    let mut edit = Element::new(
        ElementType::ImeEditBox,
        "ImeEditBox",
        "draft",
        &Resource::default(),
    );

    let state = edit.widget.edit_mut().unwrap();

    state.text = "unfinished draft".into();

    state.caret = 8;

    state.anchor = 2;

    state.scroll = 3;

    panel.elements.push(edit);

    let mut nested = Element::new(
        ElementType::ListView,
        "ListView",
        "nested",
        &Resource::default(),
    );

    let inner = nested.widget.list_mut().unwrap();

    inner
        .template
        .push(translated(ElementType::Static, "template"));

    inner.add_item("live row payload");

    inner.items[0].children[0].set_display("live child value");

    inner.items[0].payload = 987;

    inner.items[0].selected = true;

    inner.selected = 0;

    inner.scroll = 4;

    let mut outer = Element::new(
        ElementType::ListView,
        "ListView",
        "outer",
        &Resource::default(),
    );

    let state = outer.widget.list_mut().unwrap();

    state
        .template
        .push(translated(ElementType::Static, "outer template"));

    state.add_item("outer live row");

    state.items[0].children.push(nested);

    state.selected = 0;

    panel.elements.push(outer);

    panel
}

#[test]
fn characterization_translation_keys_survive_resolution_and_dynamic_text_updates() {
    let panel = panel();

    assert_eq!(panel.caption_key, "uu_1");

    assert_eq!(panel.caption, "Original title");

    let mut element = translated(ElementType::Static, "label");

    assert_eq!(element.caption_key, "uu_2");

    assert_eq!(element.caption, "Original caption");

    assert_eq!(element.display, "Original caption");

    assert_eq!(element.tooltip_key, "uu_3");

    assert_eq!(element.tooltip, "Original tooltip");

    element.set_display("runtime_value");

    element.set_tooltip_text("runtime tooltip");

    assert_eq!(element.display, "runtime value");

    assert_eq!(element.caption, "Original caption");

    assert_eq!(element.caption_key, "uu_2");

    assert_eq!(element.tooltip_key, "uu_3");

    assert_eq!(element.tooltip, "runtime tooltip");
}

#[test]
fn characterization_explicit_text_property_overrides_a_resolved_caption() {
    let mut element = translated(ElementType::Button, "button");

    element.apply(
        &property("Text", "explicit_value"),
        &Resource::default(),
        &strings(),
    );

    assert_eq!(element.display, "explicit value");

    assert_eq!(element.text, "explicit_value");

    assert_eq!(element.caption_key, "uu_2");

    assert_eq!(element.caption, "Original caption");
}

#[test]
fn characterization_template_clones_keep_text_keys_but_have_new_instance_identity() {
    for kind in [
        ElementType::Static,
        ElementType::Button,
        ElementType::CheckBox,
    ] {
        let original = translated(kind, "source");

        let mut cloned = original.clone_template();

        assert_ne!(cloned.instance, original.instance);

        assert_eq!(cloned.caption_key, original.caption_key);

        assert_eq!(cloned.tooltip_key, original.tooltip_key);

        assert_eq!(cloned.display, original.display);

        assert_eq!(cloned.tooltip, original.tooltip);

        cloned.set_display("custom row label");

        assert_eq!(original.display, "Original caption");

        let second = cloned.clone_template();

        assert_eq!(second.display, "custom row label");

        assert_eq!(second.caption_key, "uu_2");
    }
}

#[test]
fn characterization_layout_preparation_preserves_drafts_controls_and_nested_rows() {
    let mut panel = retained_panel();

    let before = panel.clone();

    panel.prepare_text_layouts();

    assert_eq!(panel, before);

    let list = panel.elements[2].widget.list().unwrap();

    let inner = list.items[0].children[1].widget.list().unwrap();

    assert_eq!(inner.template[0].caption_key, "uu_2");

    assert_eq!(inner.items[0].children[0].caption_key, "uu_2");

    assert_eq!(inner.items[0].children[0].display, "live child value");

    assert_eq!(inner.items[0].payload, 987);

    assert_eq!(
        panel.elements[1].widget.edit().unwrap().text,
        "unfinished draft"
    );
}

#[test]
fn relocalization_updates_bound_text_in_controls_close_buttons_and_clones() {
    let mut panel = panel();

    panel
        .elements
        .push(translated(ElementType::Button, "button"));

    panel.elements.push(panel.elements[0].clone_template());

    let mut image = Element::new(ElementType::Image, "Image", "image", &Resource::default());

    image.set_tooltip("uu_3", &strings());

    panel.elements.push(image.clone_template());

    let close = panel.close_button.as_mut().unwrap();

    close.apply(
        &property("Caption", "uu_2"),
        &Resource::default(),
        &strings(),
    );

    close.set_tooltip("uu_3", &strings());

    let before = panel.clone();

    panel.relocalize(&translated_strings());

    assert_eq!(panel.caption, "New title");

    for element in panel.elements.iter().take(2) {
        assert_eq!(element.caption, "New caption");

        assert_eq!(element.display, "New caption");

        assert_eq!(element.tooltip, "New tooltip");

        assert_eq!(element.caption_key, "uu_2");
    }

    assert_eq!(panel.elements[2].tooltip, "New tooltip");

    let close = panel.close_button.as_ref().unwrap();

    assert_eq!(close.display, "New caption");

    assert_eq!(close.tooltip, "New tooltip");

    let cloned = panel.elements[0].clone_template();

    assert_eq!(cloned.display, "New caption");

    panel.relocalize(&strings());

    assert_eq!(panel, before);
}

#[test]
fn relocalization_keeps_dynamic_text_even_when_it_matches_the_old_translation() {
    let mut panel = panel();

    panel.caption = "live panel title".into();

    let mut dynamic = translated(ElementType::Static, "dynamic");

    dynamic.set_display("Original caption");

    dynamic.set_tooltip_text("Original tooltip");

    panel.elements.push(dynamic.clone_template());

    let mut explicit = translated(ElementType::Button, "explicit");

    explicit.apply(
        &property("Text", "Original caption"),
        &Resource::default(),
        &strings(),
    );

    panel.elements.push(explicit);

    let mut direct = translated(ElementType::Static, "direct");

    direct.display = "direct value".into();

    direct.caption = "direct caption".into();

    direct.tooltip = "direct tooltip".into();

    panel.elements.push(direct);

    panel.relocalize(&translated_strings());

    assert_eq!(panel.caption, "live panel title");

    assert_eq!(panel.elements[0].display, "Original caption");

    assert_eq!(panel.elements[0].tooltip, "Original tooltip");

    assert_eq!(panel.elements[0].caption, "New caption");

    assert_eq!(panel.elements[1].display, "Original caption");

    assert_eq!(panel.elements[2].display, "direct value");

    assert_eq!(panel.elements[2].caption, "direct caption");

    assert_eq!(panel.elements[2].tooltip, "direct tooltip");

    panel.relocalize(&strings());

    panel.relocalize(&translated_strings());

    assert_eq!(panel.elements[0].display, "Original caption");

    assert_eq!(panel.elements[2].display, "direct value");
}

#[test]
fn relocalization_preserves_runtime_state_and_recurses_through_nested_templates_and_rows() {
    let mut panel = retained_panel();

    let before = panel.clone();

    panel.relocalize(&translated_strings());

    let list = panel.elements[2].widget.list().unwrap();

    assert_eq!(list.template[0].display, "New caption");

    assert_eq!(list.items[0].text, "outer live row");

    assert_eq!(list.items[0].children[0].display, "New caption");

    let nested = list.items[0].children[1].widget.list().unwrap();

    assert_eq!(nested.template[0].display, "New caption");

    assert_eq!(nested.items[0].children[0].display, "live child value");

    assert_eq!(nested.items[0].children[0].tooltip, "New tooltip");

    assert_eq!(nested.items[0].payload, 987);

    assert_eq!(nested.scroll, 4);

    assert_eq!(panel.focus, Some(1));

    assert_eq!(
        panel.elements[1].widget.edit(),
        before.elements[1].widget.edit()
    );

    panel.relocalize(&strings());

    assert_eq!(panel, before);
}

#[test]
fn clearing_an_icon_tooltip_does_not_rebind_it_on_language_change() {
    let mut icon = translated(ElementType::IconButton, "icon");

    assert_eq!(icon.tooltip, "Original tooltip");

    icon.set_icon_contents(None);

    assert!(icon.tooltip.is_empty());

    icon.relocalize(&translated_strings());

    assert!(icon.tooltip.is_empty());
}

#[test]
fn relocalization_keeps_nested_editor_composition_combo_popup_and_populated_icon_state() {
    let resource = Resource::default();

    let mut children = Vec::new();

    for kind in [
        ElementType::EditBox,
        ElementType::ImeEditBox,
        ElementType::ImeNotePadEditBox,
        ElementType::ImeNotePad,
    ] {
        let mut element = Element::new(kind, "editor", "draft", &resource);

        let edit = element.widget.edit_mut().unwrap();

        edit.text = "unfinished synthetic draft".into();

        edit.caret = 8;

        edit.anchor = 3;

        edit.scroll = 2;

        edit.selection_held = true;

        edit.ime_status = crate::widget::ImeStatus::Composing;

        crate::text_entry::preedit(edit, "한국", Some((0, 3)));

        if let Some(pad) = edit.pad.as_mut() {
            pad.scroll = 3;

            pad.scroll_metrics = Some([12, 4]);

            pad.rows_cleared = false;

            pad.font_heights.insert(0, 16);
        }

        children.push(element);
    }

    let mut combo = translated(ElementType::ComboBox, "choices");

    let state = combo.widget.combo_box_mut().unwrap();

    state.items = vec![
        crate::combo_box::Row {
            text: "live first choice".into(),
            payload: 71,
            color: 0xFFFF_FFFF,
        },
        crate::combo_box::Row {
            text: "live second choice".into(),
            payload: 82,
            color: 0xFFFF_FF00,
        },
    ];

    state.selected = 0;

    state.hovered = 1;

    state.open = true;

    state.scroll = 1;

    state.drag_offset = Some(4);

    children.push(combo);

    let mut icon = translated(ElementType::IconButton, "cell");

    let button = icon
        .set_icon_contents(crate::icon::set_picture_icon(123))
        .unwrap();

    button.checked = true;

    button.icon_locked = true;

    button.cell_group = crate::widget::cell_group::INVENTORY;

    children.push(icon);

    let mut list = Element::new(ElementType::ListView, "ListView", "container", &resource);

    let state = list.widget.list_mut().unwrap();

    let row = state.add_item("live row");

    state.items[row].children = children;

    let mut panel = panel();

    panel.elements.push(list);

    let before = panel.clone();

    panel.relocalize(&translated_strings());

    let children = &panel.elements[0].widget.list().unwrap().items[0].children;

    let previous = &before.elements[0].widget.list().unwrap().items[0].children;

    for (current, previous) in children.iter().zip(previous) {
        assert_eq!(current.instance, previous.instance);

        assert_eq!(current.widget, previous.widget);
    }

    assert_eq!(children[4].display, "New caption");

    assert_eq!(children[5].display, "New caption");

    panel.relocalize(&strings());

    assert_eq!(panel, before);
}

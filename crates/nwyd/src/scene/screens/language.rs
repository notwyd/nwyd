use super::*;

pub(in crate::scene) fn refresh_language(
    panels: &mut Panels,
    entered: &Entered,
    mission: Option<&super::super::mission::State>,
    collection: &super::super::collection::Book,
) {
    let Panels {
        screens,
        strings,
        items,
        icons,
        help,
        quests,
        missions,
        mixes,
        mix_help,
        mix,
        refine,
        portal,
        potal_pos,
        skill_master_desc,
        guild,
        ..
    } = panels;

    for screen in screens {
        let original = screen.placed.ui.panel.clone();

        match screen.content {
            panels::CONTENT_ITEM_MIX4 => text(
                screen,
                "Money",
                &format!("{} {}", strings.translate("uu_278"), thousands(30_000_000)),
            ),
            CONTENT_CHARACTER => sheet_refresh(screen, strings, entered),
            CONTENT_INVENTORY => {
                for bag in 0..2 {
                    let label = entered
                        .bag_extension(bag)
                        .map(|item| bag_expiry(&item, strings))
                        .unwrap_or_default();

                    if let Some(bound) = screen.inventory_widgets.days[bag]
                        && let Some(element) = screen
                            .placed
                            .ui
                            .panel
                            .elements
                            .iter_mut()
                            .find(|element| element.instance == bound)
                    {
                        element.set_display(&label);
                    }
                }
            }
            CONTENT_SKILL => skill_refresh(screen, entered, items, strings),
            CONTENT_SKILL_MASTER => {
                text(screen, skill_master::DESC, skill_master_desc);

                let first = screen
                    .element(&skill_master::button_name(0), ElementType::IconButton)
                    .and_then(|element| element.widget.button())
                    .and_then(|button| button.icon_contents)
                    .map_or(0, |icon| icon.index as i16);

                for (group, index) in skill_master::GROUP_STRDEF[skill_master::class_of(first)]
                    .iter()
                    .enumerate()
                {
                    text(
                        screen,
                        &format!("SkillName{}", group + 1),
                        &strings.translate(&format!("ss_{index}")),
                    );
                }
            }
            CONTENT_HELP if screen.page != super::super::minipopup::MEMO_HELP_TAB => {
                if let Some(lines) = usize::try_from(screen.page)
                    .ok()
                    .and_then(|index| help.get(index))
                {
                    help_page(screen, screen.page, lines);
                }
            }
            CONTENT_QUEST => {
                let selected = screen
                    .element("TitleList", ElementType::ListView)
                    .and_then(|element| element.widget.list())
                    .map_or(-1, |list| list.selected);

                quest_page(screen, screen.page, quests, entered);

                if let Ok(selected) = usize::try_from(selected) {
                    quest_select(screen, selected, quests);
                }
            }
            CONTENT_MISSION_MIX => {
                if let (Some(state), Some(tables)) = (mission, missions.as_ref()) {
                    let details = state.details(tables, &entered.carry, items);

                    mission_details(screen, &details, items, icons);
                }
            }
            super::super::mix::CONTENT => {
                if let Some(list) = mixes.as_ref() {
                    mix_refresh(screen, mix, list, mix_help, items, icons, strings);
                }
            }
            super::super::refine::CONTENT => {
                refine_stop_caption(screen, refine.auto, strings);

                text_any(
                    screen,
                    "Stop",
                    &strings.translate(super::super::refine::AUTO_CAPTION),
                );

                refine_piece_name(screen, refine, items);

                if let Some(list) = mixes.as_ref() {
                    refine_recipe_labels(screen, refine, list, items, strings);
                }
            }
            panels::CONTENT_PORTAL => {
                if let Some(page) = portal.page {
                    portal_scroll(
                        screen,
                        formats::text::potal_pos::page(potal_pos, page as usize),
                        strings,
                        page,
                    );
                } else if portal.mode == Some(portal::Mode::AirMove) {
                    portal_air_move(screen, quests, strings);
                }
            }
            panels::CONTENT_GUILD_MEMBER => guild_member_refresh(screen, guild, strings),
            super::super::guild_panel::MEMBER_POPUP => {
                guild_popup_refresh(screen, guild, strings);
            }
            super::super::collection::CONTENT => {
                if let Some(list) = screen
                    .element_mut(super::super::collection::LIST_VIEW, ElementType::ListView)
                    .and_then(|element| element.widget.list_mut())
                {
                    for row in &mut list.items {
                        let Some(entry) = usize::try_from(row.payload)
                            .ok()
                            .and_then(|index| collection.table.get(index))
                        else {
                            continue;
                        };

                        for child in &mut row.children {
                            for (index, name) in
                                super::super::collection::REWARD_LABELS.iter().enumerate()
                            {
                                if child.name.eq_ignore_ascii_case(name) {
                                    let (effect, value) = if index == 0 {
                                        (entry.primary_reward_effect, entry.primary_reward_value)
                                    } else {
                                        (
                                            entry.secondary_reward_effect,
                                            entry.secondary_reward_value,
                                        )
                                    };

                                    child.set_display(&super::super::collection::reward_label(
                                        effect, value, strings,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            _ => continue,
        }

        let translated = std::mem::replace(&mut screen.placed.ui.panel, original);

        copy_text(&mut screen.placed.ui.panel.elements, &translated.elements);
    }
}

pub(in crate::scene) fn copy_text(current: &mut [ui::Element], translated: &[ui::Element]) {
    for element in current {
        let Some(source) = translated
            .iter()
            .find(|source| source.instance == element.instance)
            .or_else(|| {
                translated
                    .iter()
                    .find(|source| source.matches(&element.name, element.element_type))
            })
        else {
            continue;
        };

        if element.widget.edit().is_none() && element.display != source.display {
            element.set_display(&source.display);
        }

        if element.element_type == ElementType::Static {
            element.visible = source.visible;

            for (layer, source) in element.layers.iter_mut().zip(&source.layers) {
                layer.font_color = source.font_color.clone();
            }
        }

        let rect = element.rect();

        if let (Some(list), Some(source)) = (element.widget.list_mut(), source.widget.list()) {
            copy_list_text(list, source, rect);
        }
    }
}

fn contains_editor(elements: &[ui::Element]) -> bool {
    elements.iter().any(|element| {
        element.widget.edit().is_some()
            || element
                .widget
                .list()
                .is_some_and(|list| list.items.iter().any(|row| contains_editor(&row.children)))
    })
}

fn unique_payloads(rows: &[ui::widget::ListRow]) -> bool {
    let mut seen = std::collections::BTreeSet::new();

    rows.iter().all(|row| seen.insert(row.payload))
}

fn copy_list_text(
    list: &mut ui::widget::ListState,
    source: &ui::widget::ListState,
    rect: [i32; 4],
) {
    let by_payload = unique_payloads(&list.items) && unique_payloads(&source.items);

    if list.items.iter().any(|row| contains_editor(&row.children)) {
        for (index, current) in list.items.iter_mut().enumerate() {
            let source = if by_payload {
                source
                    .items
                    .iter()
                    .find(|row| row.payload == current.payload)
            } else {
                source.items.get(index)
            };

            if let Some(source) = source {
                current.text.clone_from(&source.text);

                current.rich.clone_from(&source.rich);

                copy_text(&mut current.children, &source.children);
            }
        }

        return;
    }

    let translated_index = |index: i32| -> i32 {
        let Some(row) = usize::try_from(index)
            .ok()
            .and_then(|index| list.items.get(index))
        else {
            return -1;
        };

        if by_payload {
            source
                .items
                .iter()
                .position(|source| source.payload == row.payload)
                .map_or(-1, |index| index as i32)
        } else if (index as usize) < source.items.len() {
            index
        } else {
            -1
        }
    };

    let selected = translated_index(list.selected);

    let anchor = translated_index(list.selection_anchor);

    let mut previous: Vec<_> = std::mem::take(&mut list.items)
        .into_iter()
        .map(Some)
        .collect();

    for (index, source) in source.items.iter().enumerate() {
        let index = if by_payload {
            previous.iter().position(|row| {
                row.as_ref()
                    .is_some_and(|row| row.payload == source.payload)
            })
        } else {
            Some(index)
        };

        let mut row = index
            .and_then(|index| previous.get_mut(index))
            .and_then(Option::take)
            .unwrap_or_else(|| source.clone());

        row.text.clone_from(&source.text);

        row.rich.clone_from(&source.rich);

        copy_text(&mut row.children, &source.children);

        list.items.push(row);
    }

    list.selected = selected;

    list.selection_anchor = anchor;

    if selected < 0 {
        list.selection_held = false;
    }

    list.scroll = list.clamped_scroll(list.content_rect(rect));
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn language_refresh_preserves_list_selection_scroll_and_existing_row_identity() {
        let resource = ui::Resource::default();

        let mut element = ui::Element::new(ElementType::ListView, "ListView", "rows", &resource);

        element.width = 100;

        element.height = 18;

        let list = element.widget.list_mut().unwrap();

        list.item_size = [100, 18];

        list.template = vec![ui::Element::new(
            ElementType::Static,
            "Static",
            "label",
            &resource,
        )];

        for text in ["old first", "old second"] {
            let index = list.add_item("");

            list.items[index].children[0].set_display(text);
        }

        list.selected = 1;

        list.selection_anchor = 1;

        list.items[1].selected = true;

        list.scroll = 1;

        let identity = list.items[1].children[0].instance;

        let mut translated = element.clone();

        let list = translated.widget.list_mut().unwrap();

        list.clear_items();

        for text in ["new first", "new second", "new third"] {
            let index = list.add_item("");

            list.items[index].children[0].set_display(text);
        }

        let mut current = [element];

        copy_text(&mut current, &[translated]);

        let list = current[0].widget.list().unwrap();

        assert_eq!(
            (list.selected, list.selection_anchor, list.scroll),
            (1, 1, 1)
        );

        assert!(list.items[1].selected);

        assert_eq!(list.items[1].children[0].instance, identity);

        assert_eq!(list.items[1].children[0].display, "new second");

        assert_eq!(list.items[2].children[0].display, "new third");
    }

    #[test]
    fn language_refresh_reconciles_payloads_and_clears_removed_action_selection() {
        let resource = ui::Resource::default();

        let mut element = ui::Element::new(ElementType::ListView, "ListView", "actions", &resource);

        element.width = 100;

        element.height = 18;

        let list = element.widget.list_mut().unwrap();

        list.item_size = [100, 18];

        list.template = vec![ui::Element::new(
            ElementType::Static,
            "Static",
            "label",
            &resource,
        )];

        for payload in [10, 20, 30] {
            let index = list.add_item("");

            list.items[index].payload = payload;

            list.items[index].children[0].set_display(&format!("old {payload}"));
        }

        list.selected = 0;

        list.selection_anchor = 0;

        list.items[0].selected = true;

        list.scroll = 999;

        let survivor = list.items[2].children[0].instance;

        let mut translated = element.clone();

        let list = translated.widget.list_mut().unwrap();

        list.clear_items();

        for payload in [30, 20] {
            let index = list.add_item("");

            list.items[index].payload = payload;

            list.items[index].children[0].set_display(&format!("new {payload}"));
        }

        let mut current = [element];

        copy_text(&mut current, &[translated]);

        let list = current[0].widget.list().unwrap();

        assert_eq!(
            list.items.iter().map(|row| row.payload).collect::<Vec<_>>(),
            [30, 20]
        );

        assert_eq!(list.items[0].children[0].instance, survivor);

        assert_eq!(list.items[0].children[0].display, "new 30");

        assert_eq!(list.selected, -1);

        assert!(list.items.iter().all(|row| !row.selected));

        assert_eq!(
            list.scroll,
            list.clamped_scroll(list.content_rect(current[0].rect()))
        );

        let list = current[0].widget.list_mut().unwrap();

        list.selected = 1;

        list.selection_anchor = 1;

        list.items[1].selected = true;

        let survivor = list.items[1].children[0].instance;

        let mut translated = current[0].clone();

        translated.widget.list_mut().unwrap().items.remove(0);

        copy_text(&mut current, &[translated]);

        let list = current[0].widget.list().unwrap();

        assert_eq!((list.selected, list.selection_anchor), (0, 0));

        assert_eq!(list.items[0].payload, 20);

        assert!(list.items[0].selected);

        assert_eq!(list.items[0].children[0].instance, survivor);
    }

    #[test]
    fn language_refresh_never_discards_unexpected_list_editor_drafts() {
        let resource = ui::Resource::default();

        let mut element = ui::Element::new(ElementType::ListView, "ListView", "drafts", &resource);

        let list = element.widget.list_mut().unwrap();

        list.template = vec![ui::Element::new(
            ElementType::EditBox,
            "EditBox",
            "draft",
            &resource,
        )];

        for payload in [10, 20] {
            let index = list.add_item("");

            list.items[index].payload = payload;

            list.items[index].children[0]
                .widget
                .edit_mut()
                .unwrap()
                .text = format!("draft {payload}");
        }

        let identity = list.items[1].children[0].instance;

        let mut translated = element.clone();

        translated.widget.list_mut().unwrap().items.pop();

        let mut current = [element];

        copy_text(&mut current, &[translated]);

        let list = current[0].widget.list().unwrap();

        assert_eq!(list.items.len(), 2);

        assert_eq!(list.items[1].children[0].instance, identity);

        assert_eq!(
            list.items[1].children[0].widget.edit().unwrap().text,
            "draft 20"
        );
    }

    #[test]
    fn language_refresh_copies_labels_without_changing_editor_or_button_state() {
        let resource = ui::Resource::default();

        let mut label = ui::Element::new(ElementType::Static, "Static", "label", &resource);

        label.set_display("old label");

        let mut edit = ui::Element::new(ElementType::EditBox, "EditBox", "draft", &resource);

        let state = edit.widget.edit_mut().unwrap();

        state.text = "unsent synthetic draft".into();

        state.caret = 7;

        state.anchor = 2;

        let mut button = ui::Element::new(ElementType::Button, "Button", "locked", &resource);

        button.enabled = false;

        button.widget.button_mut().unwrap().checked = true;

        let mut current = vec![label, edit, button];

        let mut changed = current.clone();

        changed[0].set_display("new label");

        changed[1].set_display("replacement");

        changed[1].widget.edit_mut().unwrap().text = "replacement".into();

        changed[2].enabled = true;

        changed[2].widget.button_mut().unwrap().checked = false;

        let ids: Vec<_> = current.iter().map(|element| element.instance).collect();

        copy_text(&mut current, &changed);

        assert_eq!(current[0].display, "new label");

        let edit = current[1].widget.edit().unwrap();

        assert_eq!(edit.text, "unsent synthetic draft");

        assert_eq!((edit.caret, edit.anchor), (7, 2));

        assert!(!current[2].enabled);

        assert!(current[2].widget.button().unwrap().checked);

        assert_eq!(
            current
                .iter()
                .map(|element| element.instance)
                .collect::<Vec<_>>(),
            ids
        );
    }
}

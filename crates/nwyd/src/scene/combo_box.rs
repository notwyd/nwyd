use super::{
    Scene,
    login_panels::{LoginPanels, PlacedPanel},
    panels,
};
use ui::{
    Element,
    draw::Owner,
    input::{Event, EventId, Manager, Target},
};

fn owner(manager: &Manager, wheel: bool) -> Option<Target> {
    if wheel {
        manager.hovered
    } else {
        manager.focused
    }
}

pub fn key_prefix(scene: &mut Scene, vk: u32) {
    if scene.legacy_modal() && !matches!(scene, Scene::Field(_)) && !scene.server_modern_keyboard()
    {
        return;
    }

    match scene {
        Scene::LoginStatic(_) => {}
        Scene::LoginPanels(s) => s.key_prefix(vk),
        Scene::ServerSelect(s) => s.panels.key_prefix(vk),
        Scene::SelectChar(s) => {
            if s.name_dialog.visible {
                return;
            }

            s.panels.key_prefix(vk);
        }
        Scene::Field(s) => {
            if super::field::air_move_active(s) {
                return;
            }

            let Some(p) = s.panels.as_mut() else {
                return;
            };

            if p.msgbox.placed.ui.panel.visible {
                ui::key_prefix::visit(&mut [&mut p.msgbox.placed.ui.panel], &[0], None, vk);

                return;
            }

            if p.is_open(panels::CONTENT_COIN_INPUT) {
                if let Some(screen) = p.screen_mut(panels::CONTENT_COIN_INPUT) {
                    ui::key_prefix::visit(&mut [&mut screen.placed.ui.panel], &[0], None, vk);
                }

                return;
            }

            let Some(hud) = s.hud.as_mut() else {
                return;
            };

            let order = panels::walk_order(hud, p);

            let mut refs = vec![
                &mut hud.bar.ui.panel,
                &mut hud.plate.ui.panel,
                &mut hud.target.ui.panel,
            ];

            refs.extend(p.screens.iter_mut().map(|s| &mut s.placed.ui.panel));

            ui::key_prefix::visit_manager(&mut refs, &order, &hud.manager, vk);
        }
    }
}

pub fn edit_virtual_key(key: ui::text_entry::Key) -> u32 {
    use ui::text_entry::Key;

    match key {
        Key::Tab => 9,
        Key::Return => 13,
        Key::Escape => 27,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::End => 0x23,
        Key::Home => 0x24,
        Key::Left => 0x25,
        Key::Up => 0x26,
        Key::Right => 0x27,
        Key::Down => 0x28,
        Key::Insert => 0x2d,
        Key::Delete => 0x2e,
    }
}

fn element(
    placed: &mut PlacedPanel,
    target: Target,
    wheel: bool,
) -> Option<(Owner, &mut Element, [i32; 4])> {
    if wheel {
        placed.ui.refresh_input_layout();
    }

    if matches!(target.owner, Owner::Descendant { item: None, .. }) || wheel {
        let panel = &mut placed.ui.panel;

        let mut owner = match target.owner {
            Owner::ListRow {
                element,
                child: None,
                ..
            } => Owner::Element(element),
            owner => owner,
        };

        loop {
            let (e, _) = ui::input::owner_element(panel, owner)?;

            if !e.msg_receive {
                return None;
            }

            if !wheel
                || e.widget.combo_box().is_some()
                || e.widget.list().is_some()
                || e.widget.edit().is_some_and(|edit| edit.note_pad)
            {
                let (e, rect) = ui::input::owner_element_mut(panel, owner)?;

                return Some((owner, e, rect));
            }

            owner = ui::input::parent_owner(panel, owner)?;
        }
    }

    if !placed.ui.panel.visible {
        return None;
    }

    let page = placed.ui.panel.page_index;

    let index = match target.owner {
        Owner::Element(i) | Owner::ListRow { element: i, .. } => i,
        _ => return None,
    };

    let e = placed.ui.panel.elements.get_mut(index)?;

    if !e.visible
        || !e.enabled
        || !e.msg_receive
        || (page != -1 && e.page_index != 0xff && page != i32::from(e.page_index))
    {
        return None;
    }

    if let Owner::ListRow {
        row,
        child: Some(child),
        ..
    } = target.owner
    {
        let rect = ui::input::container_rows(e)
            .into_iter()
            .find(|(i, _)| *i == row)?
            .1;

        if wheel
            && e.widget
                .list()?
                .items
                .get(row)?
                .children
                .get(child)?
                .widget
                .combo_box()
                .is_none()
        {
            let rect = e.rect();

            return Some((Owner::Element(index), e, rect));
        }

        let child = e
            .widget
            .list_mut()?
            .items
            .get_mut(row)?
            .children
            .get_mut(child)?;

        if !child.visible || !child.enabled || !child.msg_receive {
            return None;
        }

        let rect = [
            child.x + rect[0],
            child.y + rect[1],
            child.x + rect[0] + child.width,
            child.y + rect[1] + child.height,
        ];

        return Some((target.owner, child, rect));
    }

    let rect = e.rect();

    Some((target.owner, e, rect))
}

fn login(scene: &mut LoginPanels, wheel: bool) -> Option<(Target, &mut Element, [i32; 4])> {
    if scene.msgbox.placed.ui.panel.visible {
        return None;
    }

    let target = owner(&scene.manager, wheel)?;

    let placed = match target.panel {
        0 => &mut scene.login,
        1 => &mut scene.login2,
        2 => &mut scene.copyright,
        _ => return None,
    };

    scene
        .manager
        .reconcile_panel(target.panel, &placed.ui.panel);

    let target = owner(&scene.manager, wheel)?;

    let (owner, e, rect) = element(placed, target, wheel)?;

    Some((Target { owner, ..target }, e, rect))
}

fn target(scene: &mut Scene, wheel: bool) -> Option<(Target, &mut Element, [i32; 4])> {
    match scene {
        Scene::LoginStatic(_) => None,
        Scene::LoginPanels(scene) => login(scene, wheel),
        Scene::ServerSelect(scene) => login(&mut scene.panels, wheel),
        Scene::SelectChar(scene) => {
            if scene.name_dialog.visible || scene.panels.msgbox.placed.ui.panel.visible {
                return None;
            }

            let target = owner(&scene.panels.manager, wheel)?;

            let pane = scene.panels.layers().get(target.panel).copied().flatten()?;

            scene
                .panels
                .manager
                .reconcile_panel(target.panel, &scene.panels.panes[pane].ui.panel);

            let target = owner(&scene.panels.manager, wheel)?;

            let (owner, e, rect) = element(&mut scene.panels.panes[pane], target, wheel)?;

            Some((Target { owner, ..target }, e, rect))
        }
        Scene::Field(scene) => {
            if super::field::air_move_active(scene) {
                return None;
            }

            let p = scene.panels.as_mut()?;

            if p.msgbox.placed.ui.panel.visible || p.is_open(panels::CONTENT_COIN_INPUT) {
                return None;
            }

            let hud = scene.hud.as_mut()?;

            let target = owner(&hud.manager, wheel)?;

            let placed = match target.panel {
                0 => &mut hud.bar,
                1 => &mut hud.plate,
                2 => &mut hud.target,
                index => {
                    &mut p
                        .screens
                        .get_mut(index.checked_sub(panels::HUD_PANELS)?)?
                        .placed
                }
            };

            hud.manager.reconcile_panel(target.panel, &placed.ui.panel);

            let target = owner(&hud.manager, wheel)?;

            let (owner, e, rect) = element(placed, target, wheel)?;

            Some((Target { owner, ..target }, e, rect))
        }
    }
}

fn notification(target: Target, e: &Element, changed: bool) -> Vec<Event> {
    if !changed {
        return Vec::new();
    }

    vec![Event {
        panel: target.panel,
        id: EventId::ComboChanged,
        owner: Some(target.owner),
        name: e.name.clone(),
        extra: i64::from(
            e.widget
                .combo_box()
                .expect("ComboBox notification owner")
                .selected,
        ),
    }]
}

fn focused_parts(scene: &mut Scene) -> Option<(&mut Manager, &mut PlacedPanel)> {
    fn login_parts(scene: &mut LoginPanels) -> Option<(&mut Manager, &mut PlacedPanel)> {
        if scene.msgbox.placed.ui.panel.visible {
            return None;
        }

        let placed = match scene.manager.focused?.panel {
            0 => &mut scene.login,
            1 => &mut scene.login2,
            2 => &mut scene.copyright,
            _ => return None,
        };

        Some((&mut scene.manager, placed))
    }

    let (manager, placed) = match scene {
        Scene::LoginStatic(_) => return None,
        Scene::LoginPanels(scene) => login_parts(scene)?,
        Scene::ServerSelect(scene) => login_parts(&mut scene.panels)?,
        Scene::SelectChar(scene) => {
            if scene.name_dialog.visible || scene.panels.msgbox.placed.ui.panel.visible {
                return None;
            }

            let index = scene.panels.manager.focused?.panel;

            let pane = scene.panels.layers().get(index).copied().flatten()?;

            (&mut scene.panels.manager, &mut scene.panels.panes[pane])
        }
        Scene::Field(scene) => {
            if super::field::air_move_active(scene) {
                return None;
            }

            let p = scene.panels.as_mut()?;

            if p.msgbox.placed.ui.panel.visible || p.is_open(panels::CONTENT_COIN_INPUT) {
                return None;
            }

            let hud = scene.hud.as_mut()?;

            let placed = match hud.manager.focused?.panel {
                0 => &mut hud.bar,
                1 => &mut hud.plate,
                2 => &mut hud.target,
                index => {
                    &mut p
                        .screens
                        .get_mut(index.checked_sub(panels::HUD_PANELS)?)?
                        .placed
                }
            };

            (&mut hud.manager, placed)
        }
    };

    manager.reconcile_panel(manager.focused?.panel, &placed.ui.panel);

    Some((manager, placed))
}

pub fn checkable_key(scene: &mut Scene, down: bool) -> Option<Vec<Event>> {
    let (manager, placed) = focused_parts(scene)?;

    manager.checkable_key(manager.focused?.panel, &mut placed.ui.panel, down)
}

fn button_key(scene: &mut Scene, vk: u32) -> Option<Vec<Event>> {
    let (manager, placed) = focused_parts(scene)?;

    manager.button_key(manager.focused?.panel, &placed.ui.panel, vk)
}

fn nested_notepad_key(scene: &mut Scene, vk: u32) -> Option<Vec<Event>> {
    use ui::text_entry::Key;

    let key = match vk {
        0x0d => Key::Return,
        0x21 => Key::PageUp,
        0x22 => Key::PageDown,
        0x23 => Key::End,
        0x24 => Key::Home,
        0x25 => Key::Left,
        0x26 => Key::Up,
        0x27 => Key::Right,
        0x28 => Key::Down,
        _ => return None,
    };

    let (manager, placed) = focused_parts(scene)?;

    let target = manager.focused?;

    if !matches!(
        target.owner,
        Owner::ListRow { child: Some(_), .. } | Owner::Descendant { item: None, .. }
    ) {
        return None;
    }

    placed
        .ui
        .notepad_owner_key(target.owner, key, manager.keys)
        .then(Vec::new)
}

pub fn key(scene: &mut Scene, vk: u32, repeat: bool) -> Option<Vec<Event>> {
    if let Some(events) = nested_notepad_key(scene, vk) {
        return Some(events);
    }

    if let Some(events) = button_key(scene, vk) {
        return Some(events);
    }

    let shift = match &*scene {
        Scene::LoginStatic(_) => false,
        Scene::LoginPanels(scene) => scene.manager.keys.shift,
        Scene::ServerSelect(scene) => scene.panels.manager.keys.shift,
        Scene::SelectChar(scene) => scene.panels.manager.keys.shift,
        Scene::Field(scene) => scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.shift),
    };

    let (target, e, rect) = target(scene, false)?;

    if e.element_type == formats::ui::pane::ElementType::CustomListView
        && matches!(vk, 0x21..=0x24 | 0x26 | 0x28)
    {
        return Some(Vec::new());
    }

    if matches!(
        e.element_type,
        formats::ui::pane::ElementType::ListBox
            | formats::ui::pane::ElementType::ListView
            | formats::ui::pane::ElementType::TreeView
    ) {
        let state = e.widget.list_mut()?;

        let changed = state.navigate_with_shift(vk, rect, shift)?;

        let selected = state.selection_notification_index();

        return Some(if changed {
            vec![Event {
                panel: target.panel,
                id: EventId::ListSelect,
                owner: Some(target.owner),
                name: e.name.clone(),
                extra: i64::from(selected),
            }]
        } else {
            Vec::new()
        });
    }

    let changed = e.widget.combo_box_mut()?.key(vk, repeat)?;

    Some(notification(target, e, changed))
}

pub fn wheel(scene: &mut Scene, delta: i32, lines: u32) -> Option<Vec<Event>> {
    let (target, e, rect) = target(scene, true)?;

    if let Some(pad) = e.widget.edit_mut().and_then(|edit| edit.pad.as_mut()) {
        pad.wheel_scroll(delta);

        return Some(Vec::new());
    }

    if let Some(state) = e.widget.list_mut() {
        state.wheel_scroll(rect, delta);

        let id = if state.custom.is_some() {
            Some(EventId::CustomListScrolled)
        } else if !state.plain && state.tree.is_none() {
            Some(EventId::ListScrolled)
        } else {
            None
        };

        return Some(
            id.map(|id| Event {
                panel: target.panel,
                id,
                owner: Some(target.owner),
                name: e.name.clone(),
                extra: i64::from(state.scroll),
            })
            .into_iter()
            .collect(),
        );
    }

    let layout = e.widget.combo_box()?.stored_layout(rect, &e.layers)?;

    let changed = e
        .widget
        .combo_box_mut()?
        .wheel(delta / 120, lines, &layout)?;

    Some(notification(target, e, changed))
}

pub fn virtual_key(key: ui::text_entry::Key) -> Option<u32> {
    use ui::text_entry::Key;

    Some(match key {
        Key::Return => 0x0d,
        Key::Escape => 0x1b,
        Key::Left => 0x25,
        Key::Up => 0x26,
        Key::Right => 0x27,
        Key::Down => 0x28,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::End => 0x23,
        Key::Home => 0x24,
        _ => return None,
    })
}

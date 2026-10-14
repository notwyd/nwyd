use crate::{Element, Panel, input::Target};
use formats::ui::pane::ElementType;

pub fn visit_manager(
    panels: &mut [&mut Panel],
    order: &[usize],
    manager: &crate::input::Manager,
    vk: u32,
) -> Vec<usize> {
    if vk == 27 && manager.carried.is_some() {
        return Vec::new();
    }

    let focused = manager.focused.and_then(|t| {
        panels
            .get(t.panel)
            .and_then(|p| manager.retained_target(t, p))
    });

    if let Some(drag) = manager.drag {
        visit(panels, &[drag.panel], focused, vk)
    } else {
        visit(panels, order, focused, vk)
    }
}

fn widget_consumes(e: &Element, vk: u32) -> bool {
    if !e.visible || !e.enabled {
        return false;
    }

    match e.element_type {
        ElementType::EditBox | ElementType::ImeEditBox => {
            !matches!(vk, 0x0d | 0x1b | 0x21 | 0x22 | 0x2d | 0x70..=0x73)
        }
        ElementType::ImeNotePadEditBox => !matches!(vk, 9 | 0x1b | 0x2d),
        ElementType::ImeNotePad => {
            let child = e
                .widget
                .edit()
                .and_then(|e| e.pad.as_ref())
                .is_some_and(|p| !p.read_only);

            child && !matches!(vk, 9 | 0x1b | 0x2d) || matches!(vk, 0x21..=0x24 | 0x26 | 0x28)
        }
        ElementType::Button | ElementType::IconButton => e.widget.button().is_some_and(|b| {
            b.group.is_empty() && ((vk == 13 && b.default_key) || (vk == 27 && b.cancel_key))
        }),
        ElementType::CheckBox | ElementType::RadioButton => vk == 32,
        ElementType::ComboBox => {
            matches!(vk, 0x25..=0x28 | 0x73)
                || vk == 13 && e.widget.combo_box().is_some_and(|c| c.open)
        }
        ElementType::ListBox
        | ElementType::ListView
        | ElementType::TreeView
        | ElementType::CustomListView => matches!(vk, 0x21..=0x24 | 0x26 | 0x28),
        _ => false,
    }
}

pub fn visit(
    panels: &mut [&mut Panel],
    order: &[usize],
    focused: Option<Target>,
    vk: u32,
) -> Vec<usize> {
    let focused_edit = focused
        .and_then(|t| {
            panels
                .get(t.panel)
                .and_then(|p| crate::input::retained_element(p, t.owner))
        })
        .is_some_and(|e| {
            matches!(
                e.element_type,
                ElementType::EditBox | ElementType::ImeEditBox
            )
        });

    let mut visited = Vec::new();

    for &index in order {
        let Some(panel) = panels.get_mut(index) else {
            continue;
        };

        if !panel.visible {
            if panel.modal {
                break;
            }

            continue;
        }

        panel.input_focus_latched = true;

        visited.push(index);

        let owns_focus = focused.is_some_and(|t| t.panel == index);

        let focused_consumes = focused
            .filter(|t| t.panel == index)
            .and_then(|t| crate::input::retained_element(panel, t.owner))
            .is_some_and(|e| widget_consumes(e, vk));

        let default = vk == 13
            && panel.enter_defaults
            && panel
                .elements
                .iter()
                .any(|e| e.element_type == ElementType::Button && e.is_default);

        let hotkey = !focused_edit && panel.elements.iter().any(|e| e.accel_key == vk as i32);

        let fallback = match vk {
            9 => focused.is_some() && panel.tab_navigation,
            13 => {
                owns_focus
                    && panel.elements.iter().any(|e| {
                        e.element_type == ElementType::Button
                            && e.widget.button().is_some_and(|b| b.default_key)
                    })
            }
            27 => {
                panel.close_button.is_some()
                    || panel.elements.iter().any(|e| {
                        e.element_type == ElementType::Button
                            && e.visible
                            && e.enabled
                            && e.widget.button().is_some_and(|b| b.cancel_key)
                    })
            }
            0x25..=0x28 => focused.is_some(),
            _ => false,
        };

        if focused_consumes || default || hotkey || fallback || panel.modal {
            break;
        }
    }

    visited
}

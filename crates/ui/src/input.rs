use crate::carry::Carried;
use crate::draw::Owner;
use crate::element::Element;
use crate::panel::Panel;
use formats::ui::pane::ElementType;

pub const POPULATED_PREFIX_CELLS: usize = 7;

pub const POPULATED_PREFIX_FADE_MS: u32 = 1_800;

pub const POPULATED_PREFIX_HIGHLIGHT_MS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrefixSweepCell {
    pub index: usize,
    pub fade: Option<f32>,
    pub highlight: bool,
}

pub fn populated_prefix_sweep(
    populated: &[bool],
    stamp_ms: u32,
    now_ms: u32,
) -> Vec<PrefixSweepCell> {
    if stamp_ms == 0 {
        return (!populated.is_empty())
            .then_some(PrefixSweepCell {
                index: 0,
                fade: None,
                highlight: false,
            })
            .into_iter()
            .collect();
    }

    let elapsed = now_ms.wrapping_sub(stamp_ms);

    if elapsed == 0 {
        return Vec::new();
    }

    let fade = (elapsed < POPULATED_PREFIX_FADE_MS)
        .then_some(elapsed as f32 / POPULATED_PREFIX_FADE_MS as f32);

    let highlight = elapsed <= POPULATED_PREFIX_HIGHLIGHT_MS;

    populated
        .iter()
        .take(POPULATED_PREFIX_CELLS)
        .take_while(|&&filled| filled)
        .enumerate()
        .map(|(index, _)| PrefixSweepCell {
            index,
            fade,
            highlight,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouse {
    Move,
    LeftDown,
    LeftDouble,
    LeftUp,
    RightDown,
    RightDouble,
    RightUp,
}

impl Mouse {
    pub fn is_button_up(self) -> bool {
        matches!(self, Mouse::LeftUp | Mouse::RightUp)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub panel: usize,
    pub owner: Owner,
}

#[derive(Debug, Clone, Copy)]
pub struct PanelRef<'a> {
    pub panel: &'a Panel,
    pub origin: (i32, i32),
}

impl PanelRef<'_> {
    pub fn screen_rect(&self) -> [i32; 4] {
        let (x, y) = self.origin;

        [x, y, x + self.panel.width(), y + self.panel.height()]
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (lx, ly) = self.local(x, y);

        in_rect(self.screen_rect(), x, y) || combo_popup_hit(self.panel, lx, ly).is_some()
    }

    pub fn local(&self, x: i32, y: i32) -> (i32, i32) {
        (x - self.origin.0, y - self.origin.1)
    }
}

#[derive(Debug)]
pub struct PanelMut<'a> {
    pub panel: &'a mut Panel,
    pub origin: &'a mut (i32, i32),
}

impl PanelMut<'_> {
    fn screen_rect(&self) -> [i32; 4] {
        let (x, y) = *self.origin;

        [x, y, x + self.panel.width(), y + self.panel.height()]
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        let (lx, ly) = self.local(x, y);

        in_rect(self.screen_rect(), x, y) || combo_popup_hit(self.panel, lx, ly).is_some()
    }

    fn local(&self, x: i32, y: i32) -> (i32, i32) {
        (x - self.origin.0, y - self.origin.1)
    }

    fn titlebar_rect(&self) -> [i32; 4] {
        let (x, y) = *self.origin;

        [x, y, x + self.panel.width(), y + self.panel.titlebar_height]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    Move,
    ResizeTop,
    ResizeBottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelDrag {
    pub panel: usize,
    pub mode: DragMode,
    pub grab: (i32, i32),
    pub grabbed: (i32, i32),
    pub moved: bool,
}

pub const DEAD_ZONE: i32 = 4;

pub const MIN_HEIGHT: i32 = 100;

#[cfg(test)]
mod tests {

    use super::*;

    fn control(kind: ElementType, name: &str) -> Element {
        let mut element = Element::new(kind, "", name, &crate::Resource::default());

        element.width = 40;

        element.height = 20;

        element
    }

    #[test]
    fn hit_rectangles_exclude_right_bottom_and_touching_intersections() {
        let rect = [-10, -20, 30, 40];

        for point in [(-10, -20), (29, 39)] {
            assert!(in_rect(rect, point.0, point.1));
        }

        for point in [(-11, 0), (0, -21), (30, 0), (0, 40)] {
            assert!(!in_rect(rect, point.0, point.1));
        }

        assert!(!in_rect([0, 0, 0, 10], 0, 0));

        assert_eq!(intersect(rect, [30, 0, 50, 20]), None);

        assert_eq!(intersect(rect, [0, 10, 50, 60]), Some([0, 10, 30, 40]));
    }

    #[test]
    fn selecting_radio_clears_only_sibling_radios_in_the_same_group() {
        let mut panel = Panel::default();

        for (kind, group, name) in [
            (ElementType::RadioButton, "mode", "first"),
            (ElementType::RadioButton, "mode", "second"),
            (ElementType::RadioButton, "other", "unrelated"),
            (ElementType::CheckBox, "mode", "checkbox"),
        ] {
            let mut element = control(kind, name);

            let button = element.widget.button_mut().unwrap();

            button.group = group.into();

            button.checked = true;

            panel.elements.push(element);
        }

        let events = set_radio_checked(2, &mut panel, Owner::Element(1), true, true, true).unwrap();

        assert_eq!(
            panel
                .elements
                .iter()
                .map(|e| e.widget.button().unwrap().checked)
                .collect::<Vec<_>>(),
            [false, true, true, true]
        );

        assert_eq!(
            events,
            vec![Event {
                panel: 2,
                id: EventId::RadioChanged,
                owner: Some(Owner::Element(1)),
                name: "second".into(),
                extra: 0
            }]
        );
    }

    #[test]
    fn checkbox_keyboard_activation_requires_a_matching_press_and_release() {
        let mut panel = Panel {
            elements: vec![control(ElementType::CheckBox, "choice")],
            ..Panel::default()
        };

        let target = Target {
            panel: 0,
            owner: Owner::Element(0),
        };

        let mut manager = Manager {
            focused: Some(target),
            ..Manager::default()
        };

        assert_eq!(manager.checkable_key(0, &mut panel, false), Some(vec![]));

        for _ in 0..2 {
            assert_eq!(manager.checkable_key(0, &mut panel, true), Some(vec![]));
        }

        assert_eq!(manager.pressed, Some(target));

        assert!(!panel.elements[0].widget.button().unwrap().checked);

        let events = manager.checkable_key(0, &mut panel, false).unwrap();

        assert_eq!(
            events,
            vec![Event {
                panel: 0,
                id: EventId::CheckChanged,
                owner: Some(target.owner),
                name: "choice".into(),
                extra: 1
            }]
        );

        assert_eq!(manager.pressed, None);

        assert!(panel.elements[0].widget.button().unwrap().checked);

        assert_eq!(manager.checkable_key(0, &mut panel, false), Some(vec![]));

        assert!(panel.elements[0].widget.button().unwrap().checked);

        manager.checkable_key(0, &mut panel, true);

        let events = manager.checkable_key(0, &mut panel, false).unwrap();

        assert_eq!(events[0].extra, 0);

        assert!(!panel.elements[0].widget.button().unwrap().checked);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub panel: usize,
    pub id: EventId,
    pub owner: Option<Owner>,
    pub name: String,
    pub extra: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EventId {
    Click = 0x101,
    MouseMove = 0x110,
    LeftDown = 0x111,
    RightDown = 0x112,
    LeftUp = 0x113,
    RightReleased = 0x114,
    DoubleClick = 0x115,
    RightUp = 0x102,
    RadioChanged = 0x301,
    CheckChanged = 0x401,
    ComboChanged = 0x201,
    SliderChanged = 0x501,
    ListSelect = 0x702,
    TreeRightSelect = 0x705,
    ListDoubleSelect = 0x706,
    ListScrolled = 0x708,
    CustomListScrolled = 0x709,
    Resized = 0x704,
}

impl EventId {
    pub fn command_id(self) -> Self {
        if self == Self::RadioChanged {
            Self::Click
        } else {
            self
        }
    }
}

pub fn set_checkbox_checked(
    index: usize,
    panel: &mut Panel,
    owner: Owner,
    checked: bool,
    notify: bool,
) -> Option<Event> {
    let notify = notify || panel.force_notifications;

    let element = retained_element_mut(panel, owner)?;

    if !matches!(
        element.element_type,
        ElementType::CheckBox | ElementType::RadioButton
    ) {
        return None;
    }

    element.widget.button_mut()?.checked = checked;

    notify.then(|| Event {
        panel: index,
        owner: Some(owner),
        name: element.name.clone(),
        extra: i64::from(checked),
        id: EventId::CheckChanged,
    })
}

pub fn set_radio_checked(
    index: usize,
    panel: &mut Panel,
    owner: Owner,
    checked: bool,
    clear_group: bool,
    notify: bool,
) -> Option<Vec<Event>> {
    let element = retained_element(panel, owner)?;

    if element.element_type != ElementType::RadioButton {
        return None;
    }

    let node = element.instance;

    let group = element.widget.button()?.group.clone();

    let force = panel.force_notifications;

    let mut events = Vec::new();

    if checked && clear_group {
        fn clear(
            elements: &mut [Element],
            node: crate::element::Instance,
            group: &str,
            target: Target,
            force: bool,
            events: &mut Vec<Event>,
        ) -> bool {
            if elements.iter().any(|e| e.instance == node) {
                for (i, e) in elements.iter_mut().enumerate() {
                    if e.element_type != ElementType::RadioButton {
                        continue;
                    }

                    let Some(button) = e.widget.button_mut().filter(|b| b.group == group) else {
                        continue;
                    };

                    button.checked = false;

                    if force {
                        let owner = match target.owner {
                            Owner::Element(_) => Owner::Element(i),
                            Owner::ListRow {
                                element,
                                row,
                                child: Some(_),
                            } => Owner::ListRow {
                                element,
                                row,
                                child: Some(i),
                            },
                            Owner::Descendant { element, .. } => Owner::Descendant {
                                element,
                                node: e.instance,
                                item: None,
                            },
                            _ => continue,
                        };

                        events.push(Event {
                            panel: target.panel,
                            owner: Some(owner),
                            name: e.name.clone(),
                            extra: 0,
                            id: EventId::RadioChanged,
                        });
                    }
                }

                return true;
            }

            for e in elements {
                if let Some(list) = e.widget.list_mut() {
                    for row in &mut list.items {
                        if clear(&mut row.children, node, group, target, force, events) {
                            return true;
                        }
                    }
                }
            }

            false
        }

        clear(
            &mut panel.elements,
            node,
            &group,
            Target {
                panel: index,
                owner,
            },
            force,
            &mut events,
        );
    }

    let element = retained_element_mut(panel, owner)?;

    element.widget.button_mut()?.checked = checked;

    if notify || force {
        events.push(Event {
            panel: index,
            owner: Some(owner),
            name: element.name.clone(),
            extra: 0,
            id: EventId::RadioChanged,
        });
    }

    Some(events)
}

pub fn in_rect(rect: [i32; 4], x: i32, y: i32) -> bool {
    x >= rect[0] && x < rect[2] && y >= rect[1] && y < rect[3]
}

fn element_contains(element: &Element, rect: [i32; 4], x: i32, y: i32) -> bool {
    if matches!(
        element.element_type,
        ElementType::Static
            | ElementType::BalloonBox
            | ElementType::TooltipBox
            | ElementType::GaugeBar
    ) && !element.static_hit_enabled
    {
        return false;
    }

    in_rect(rect, x, y)
        || matches!(
            element.element_type,
            ElementType::CheckBox | ElementType::RadioButton
        ) && in_rect(
            [
                rect[0],
                rect[1],
                rect[0].saturating_add(rect[3].saturating_sub(rect[1])),
                rect[3],
            ],
            x,
            y,
        )
        || element
            .widget
            .slider()
            .is_some_and(|s| in_rect(s.thumb(rect), x, y))
}

pub fn row_at(element: &Element, x: i32, y: i32) -> Option<usize> {
    let list = element.widget.list()?;

    if let Some(tree) = &list.tree {
        return tree
            .layout(list, element.rect())
            .rows
            .iter()
            .find(|(_, r)| in_rect(*r, x, y))
            .map(|(i, _)| *i);
    }

    let content = list.content_rect(element.rect());

    if !in_rect(content, x, y) {
        return None;
    }

    (0..list.items.len()).find(|&i| {
        let (rect, on_page) = list.item_rect(i, content);

        on_page && in_rect(rect, x, y)
    })
}

pub fn container_rows(element: &Element) -> Vec<(usize, [i32; 4])> {
    let Some(list) = element.widget.list() else {
        return Vec::new();
    };

    if let Some(tree) = &list.tree {
        tree.layout(list, element.rect()).rows
    } else if list.custom.is_some() {
        crate::custom_list::layout(list, element.rect()).rows
    } else {
        let content = list.content_rect(element.rect());

        let first = if list.render_method == 0 {
            list.scroll.saturating_mul(list.columns(content)).max(0) as usize
        } else {
            0
        };

        (first..list.items.len())
            .filter_map(|i| {
                let (rect, visible) = list.item_rect(i, content);

                visible.then_some((i, rect))
            })
            .collect()
    }
}

fn find_instance(element: &Element, node: crate::element::Instance) -> Option<&Element> {
    if element.instance == node {
        return Some(element);
    }

    element
        .widget
        .list()?
        .items
        .iter()
        .flat_map(|r| &r.children)
        .find_map(|e| find_instance(e, node))
}

fn find_instance_mut(
    element: &mut Element,
    node: crate::element::Instance,
) -> Option<&mut Element> {
    if element.instance == node {
        return Some(element);
    }

    element
        .widget
        .list_mut()?
        .items
        .iter_mut()
        .flat_map(|r| &mut r.children)
        .find_map(|e| find_instance_mut(e, node))
}

pub fn retained_element(panel: &Panel, owner: Owner) -> Option<&Element> {
    match owner {
        Owner::Close => panel.close_button.as_ref(),
        Owner::Element(i) => panel.elements.get(i),
        Owner::ListRow {
            element,
            row,
            child: Some(child),
        } => panel
            .elements
            .get(element)?
            .widget
            .list()?
            .items
            .get(row)?
            .children
            .get(child),
        Owner::Descendant {
            element,
            node,
            item: None,
        } => find_instance(panel.elements.get(element)?, node),
        _ => None,
    }
}

pub fn retained_element_mut(panel: &mut Panel, owner: Owner) -> Option<&mut Element> {
    match owner {
        Owner::Close => panel.close_button.as_mut(),
        Owner::Element(i) => panel.elements.get_mut(i),
        Owner::ListRow {
            element,
            row,
            child: Some(child),
        } => panel
            .elements
            .get_mut(element)?
            .widget
            .list_mut()?
            .items
            .get_mut(row)?
            .children
            .get_mut(child),
        Owner::Descendant {
            element,
            node,
            item: None,
        } => find_instance_mut(panel.elements.get_mut(element)?, node),
        _ => None,
    }
}

pub fn parent_owner(panel: &Panel, owner: Owner) -> Option<Owner> {
    fn find(e: &Element, owner: Owner, node: crate::element::Instance) -> Option<Owner> {
        for (row, item) in e.widget.list()?.items.iter().enumerate() {
            for (child, e) in item.children.iter().enumerate() {
                if e.instance == node {
                    return Some(owner);
                }

                if let Some(parent) = find(e, crate::draw::child_owner(owner, row, child, e), node)
                {
                    return Some(parent);
                }
            }
        }

        None
    }

    match owner {
        Owner::ListRow {
            element,
            child: Some(_),
            ..
        } => Some(Owner::Element(element)),
        Owner::Descendant {
            element,
            node,
            item: None,
        } => find(panel.elements.get(element)?, Owner::Element(element), node),
        _ => None,
    }
}

fn admitted_instance(
    element: &Element,
    node: crate::element::Instance,
    offset: [i32; 2],
) -> Option<(&Element, [i32; 4])> {
    if !interactive(element) {
        return None;
    }

    let x = element.x.saturating_add(offset[0]);

    let y = element.y.saturating_add(offset[1]);

    if element.instance == node {
        return Some((
            element,
            [
                x,
                y,
                x.saturating_add(element.width),
                y.saturating_add(element.height),
            ],
        ));
    }

    let state = element.widget.list()?;

    for (row, rect) in container_rows(element) {
        let offset = [
            rect[0].saturating_add(offset[0]),
            rect[1].saturating_add(offset[1]),
        ];

        for child in &state.items.get(row)?.children {
            if let Some(found) = admitted_instance(child, node, offset) {
                return Some(found);
            }
        }
    }

    None
}

pub fn owner_element(panel: &Panel, owner: Owner) -> Option<(&Element, [i32; 4])> {
    if !panel.visible {
        return None;
    }

    if owner == Owner::Close {
        return panel
            .close_button
            .as_ref()
            .filter(|e| interactive(e))
            .map(|e| (e, panel.close_button_rect()));
    }

    let index = match owner {
        Owner::Element(i)
        | Owner::ListRow {
            element: i,
            child: Some(_),
            ..
        }
        | Owner::Descendant {
            element: i,
            item: None,
            ..
        } => i,
        _ => return None,
    };

    let parent = panel.elements.get(index)?;

    if !interactive(parent)
        || (panel.page_index != -1
            && parent.page_index != 0xff
            && panel.page_index != i32::from(parent.page_index))
    {
        return None;
    }

    if let Owner::Descendant { node, .. } = owner {
        return admitted_instance(parent, node, [0, 0]);
    }

    if let Owner::ListRow {
        row,
        child: Some(child),
        ..
    } = owner
    {
        let state = parent.widget.list()?;

        if state.tree.is_none()
            && state.custom.is_none()
            && state.render_method == 0
            && row
                < state
                    .scroll
                    .saturating_mul(state.columns(state.content_rect(parent.rect())))
                    .max(0) as usize
        {
            return None;
        }

        let rect = container_rows(parent)
            .into_iter()
            .find(|(i, _)| *i == row)?
            .1;

        let e = state.items.get(row)?.children.get(child)?;

        if !interactive(e) {
            return None;
        }

        let x = e.x.saturating_add(rect[0]);

        let y = e.y.saturating_add(rect[1]);

        return Some((
            e,
            [x, y, x.saturating_add(e.width), y.saturating_add(e.height)],
        ));
    }

    Some((parent, parent.rect()))
}

pub fn control_geometry(panel: &Panel, owner: Owner) -> Option<(&Element, [i32; 4])> {
    if owner == Owner::Close {
        return panel
            .close_button
            .as_ref()
            .map(|e| (e, panel.close_button_rect()));
    }

    fn placed(
        e: &Element,
        node: crate::element::Instance,
        origin: [i32; 2],
    ) -> Option<(&Element, [i32; 4])> {
        let [x, y] = origin;

        let rect = [
            x.saturating_add(e.x),
            y.saturating_add(e.y),
            x.saturating_add(e.x).saturating_add(e.width),
            y.saturating_add(e.y).saturating_add(e.height),
        ];

        if e.instance == node {
            return Some((e, rect));
        }

        let list = e.widget.list()?;

        for (row, r) in container_rows(e) {
            for child in &list.items.get(row)?.children {
                if let Some(found) = placed(
                    child,
                    node,
                    [x.saturating_add(r[0]), y.saturating_add(r[1])],
                ) {
                    return Some(found);
                }
            }
        }

        None
    }

    let index = match owner {
        Owner::Element(i)
        | Owner::ListRow {
            element: i,
            child: Some(_),
            ..
        }
        | Owner::Descendant {
            element: i,
            item: None,
            ..
        } => i,
        _ => return None,
    };

    let parent = panel.elements.get(index)?;

    let node = retained_element(panel, owner)?.instance;

    placed(parent, node, [0, 0])
}

pub fn owner_element_mut(panel: &mut Panel, owner: Owner) -> Option<(&mut Element, [i32; 4])> {
    let rect = owner_element(panel, owner)?.1;

    let element = retained_element_mut(panel, owner)?;

    Some((element, rect))
}

fn container_popup_at(element: &Element, x: i32, y: i32) -> Option<(usize, usize)> {
    let list = element.widget.list()?;

    for (row, rect) in container_rows(element) {
        for (child, e) in list.items.get(row)?.children.iter().enumerate().rev() {
            if !interactive(e) || !e.msg_receive {
                continue;
            }

            if container_popup_at(e, x.saturating_sub(rect[0]), y.saturating_sub(rect[1])).is_some()
            {
                return Some((row, child));
            }

            if list
                .custom
                .as_ref()
                .is_some_and(|c| c.focused != Some((row, child)))
            {
                continue;
            }

            let Some(combo) = e.widget.combo_box().filter(|c| c.open) else {
                continue;
            };

            let mut placed = e.clone();

            placed.x += rect[0];

            placed.y += rect[1];

            let Some(layout) = combo.stored_layout(placed.rect(), &e.layers) else {
                continue;
            };

            if in_rect(layout.popup, x, y)
                || (layout.page < combo.items.len() && layout.bar.contains(x, y))
            {
                return Some((row, child));
            }
        }
    }

    None
}

pub fn list_child_at(element: &Element, x: i32, y: i32) -> Option<(usize, usize, [i32; 4])> {
    list_child_hit(element, x, y, false)
}

fn list_child_hit(
    element: &Element,
    x: i32,
    y: i32,
    mouse: bool,
) -> Option<(usize, usize, [i32; 4])> {
    let row_unbounded_hover = !mouse
        && matches!(
            element.element_type,
            ElementType::ListView | ElementType::TreeView
        );

    let list = element.widget.list()?;

    let content = if mouse && list.custom.is_some() {
        element.rect()
    } else {
        list.content_rect(element.rect())
    };

    if !in_rect(content, x, y) {
        return None;
    }

    let rows = container_rows(element);

    for (row, row_rect) in rows {
        let Some(item) = list.items.get(row) else {
            continue;
        };

        if !row_unbounded_hover && list.custom.is_none() && !in_rect(row_rect, x, y) {
            continue;
        }

        for ordinal in 0..item.children.len() {
            let child = if mouse && list.tree.is_some() {
                ordinal
            } else {
                item.children.len() - 1 - ordinal
            };

            let element = &item.children[child];

            if !element.msg_receive
                || !interactive(element)
                || (mouse && !container_mouse_child(element))
            {
                continue;
            }

            let rect = [
                row_rect[0] + element.x,
                row_rect[1] + element.y,
                row_rect[0] + element.x + element.width,
                row_rect[1] + element.y + element.height,
            ];

            if element_contains(element, rect, x, y) {
                return Some((row, child, rect));
            }
        }

        if !row_unbounded_hover {
            list.custom.as_ref()?;
        }
    }

    None
}

fn container_mouse_child(element: &Element) -> bool {
    matches!(
        element.element_type,
        ElementType::Button
            | ElementType::IconButton
            | ElementType::CheckBox
            | ElementType::RadioButton
            | ElementType::ComboBox
            | ElementType::Slider
            | ElementType::EditBox
            | ElementType::ImeEditBox
            | ElementType::ListBox
            | ElementType::ListView
            | ElementType::TreeView
            | ElementType::CustomListView
            | ElementType::ImeNotePad
    )
}

pub fn intersect(a: [i32; 4], b: [i32; 4]) -> Option<[i32; 4]> {
    let rect = [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ];

    (rect[0] < rect[2] && rect[1] < rect[3]).then_some(rect)
}

pub fn overlap_hit_test(panel: &Panel, rect: [i32; 4]) -> Option<(usize, [i32; 4])> {
    let mut best: Option<(usize, [i32; 4])> = None;

    let mut score = 0;

    for (index, element) in panel.elements.iter().enumerate() {
        if !element.msg_receive {
            continue;
        }

        let Some(hit) = intersect(element.rect(), rect) else {
            continue;
        };

        if !interactive(element) {
            continue;
        }

        if element.element_type != ElementType::IconButton {
            continue;
        }

        let semi_perimeter = (hit[2] - hit[0]) + (hit[3] - hit[1]);

        if score < semi_perimeter {
            score = semi_perimeter;

            best = Some((index, hit));
        }
    }

    best
}

fn interactive(element: &Element) -> bool {
    element.visible && element.enabled
}

pub fn can_focus(element: &Element) -> bool {
    interactive(element)
        && matches!(
            element.element_type,
            ElementType::Button | ElementType::EditBox | ElementType::ImeEditBox
        )
}

pub fn tab_stop(element: &Element) -> bool {
    if let Some(button) = element.widget.button() {
        return interactive(element) && button.focus_enabled;
    }

    interactive(element)
        && matches!(
            element.element_type,
            ElementType::EditBox
                | ElementType::ImeEditBox
                | ElementType::ImeNotePad
                | ElementType::Slider
                | ElementType::ComboBox
                | ElementType::TreeView
                | ElementType::CustomListView
                | ElementType::ListBox
                | ElementType::ListView
        )
}

pub fn hit_test(panel: &Panel, x: i32, y: i32) -> Option<Owner> {
    if let Some(owner) = combo_popup_hit(panel, x, y) {
        return Some(owner);
    }

    if let Some(close) = panel.close_button.as_ref()
        && in_rect(panel.close_button_rect(), x, y)
        && interactive(close)
    {
        return Some(Owner::Close);
    }

    for (index, element) in panel.elements.iter().enumerate().rev() {
        if !element.msg_receive {
            continue;
        }

        if panel.page_index != -1
            && element.page_index != 0xFF
            && panel.page_index != element.page_index as i32
        {
            continue;
        }

        if element_contains(element, element.rect(), x, y) && interactive(element) {
            return Some(Owner::Element(index));
        }
    }

    None
}

fn drop_hit_test(panel: &Panel, x: i32, y: i32) -> Option<Owner> {
    if let Some(close) = panel.close_button.as_ref()
        && in_rect(panel.close_button_rect(), x, y)
        && interactive(close)
    {
        return Some(Owner::Close);
    }

    panel
        .elements
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, e)| {
            (e.msg_receive
                && e.visible
                && (panel.page_index == -1
                    || e.page_index == 0xff
                    || panel.page_index == i32::from(e.page_index))
                && element_contains(e, e.rect(), x, y))
            .then_some(Owner::Element(index))
        })
}

fn combo_popup_hit(panel: &Panel, x: i32, y: i32) -> Option<Owner> {
    panel
        .elements
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, e)| {
            if !interactive(e)
                || !e.msg_receive
                || (panel.page_index != -1
                    && e.page_index != 0xff
                    && panel.page_index != i32::from(e.page_index))
            {
                return None;
            }

            if container_popup_at(e, x, y).is_some() {
                return Some(Owner::Element(index));
            }

            let state = e
                .widget
                .combo_box()
                .filter(|s| s.open && !s.items.is_empty())?;

            let layout = state.stored_layout(e.rect(), &e.layers)?;

            (in_rect(layout.popup, x, y)
                || (layout.page < state.items.len() && layout.bar.contains(x, y)))
            .then_some(Owner::Element(index))
        })
}

fn refine_hover(panel: &Panel, mut owner: Owner, x: i32, y: i32) -> Owner {
    if let Some((mut element, rect)) = owner_element(panel, owner) {
        let mut x = x.saturating_sub(rect[0]).saturating_add(element.x);

        let mut y = y.saturating_sub(rect[1]).saturating_add(element.y);

        while matches!(
            element.element_type,
            ElementType::ListView | ElementType::TreeView | ElementType::CustomListView
        ) {
            let Some((row, child, placed)) = list_child_at(element, x, y) else {
                break;
            };

            let next = &element.widget.list().unwrap().items[row].children[child];

            owner = crate::draw::child_owner(owner, row, child, next);

            x = x.saturating_sub(placed[0]).saturating_add(next.x);

            y = y.saturating_sub(placed[1]).saturating_add(next.y);

            element = next;
        }
    }

    owner
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetainedTargets(Vec<(Target, crate::element::Instance)>);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manager {
    pub retained_targets: RetainedTargets,
    pub button_sounds: Vec<i64>,
    pub focused: Option<Target>,
    pub pressed: Option<Target>,
    pub hovered: Option<Target>,
    pub capture: Option<Target>,
    pub drag: Option<PanelDrag>,
    pub screen: [i32; 4],
    pub carried: Option<Carried>,
    pub drop: Option<DropPanel>,
    pub cursor: (i32, i32),
    pub keys: crate::text_entry::Mods,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropPanel {
    pub panel: usize,
    pub element: usize,
    pub rect: [i32; 4],
}

impl DropPanel {
    fn area(&self) -> i32 {
        (self.rect[2] - self.rect[0]) * (self.rect[3] - self.rect[1])
    }
}

struct MouseWalk {
    panel: usize,
    event: Mouse,
    point: [i32; 2],
    screen_y: Option<[i32; 2]>,
}

#[derive(Clone, Copy)]
struct MouseAdmission {
    parent_callback: bool,
    focus_allowed: bool,
}

impl Manager {
    pub fn remove_tree_item(
        &mut self,
        index: usize,
        panel: &mut Panel,
        owner: Owner,
        row: usize,
    ) -> Option<Event> {
        let element = retained_element(panel, owner)?;

        if element.element_type != ElementType::TreeView {
            return None;
        }

        let state = element.widget.list()?;

        if row >= state.items.len() || state.tree.as_ref()?.node(row).is_none() {
            return None;
        }

        self.reconcile_panel(index, panel);

        let element = retained_element_mut(panel, owner)?;

        let state = element.widget.list_mut()?;

        let (selected, anchor) = (state.selected, state.selection_anchor);

        if !state.remove_tree_item(row) {
            return None;
        }

        state.selected = if i64::from(selected) >= state.items.len() as i64 {
            -1
        } else {
            selected
        };

        state.selection_anchor = anchor;

        let event = Event {
            panel: index,
            owner: Some(owner),
            name: element.name.clone(),
            extra: i64::from(state.selection_notification_index()),
            id: EventId::ListSelect,
        };

        self.reconcile_panel(index, panel);

        Some(event)
    }

    pub fn retained_target(&self, target: Target, panel: &Panel) -> Option<Target> {
        match target.owner {
            Owner::Element(_) if self.retained_targets.0.iter().any(|(t, _)| *t == target) => {
                let (_, instance) = self
                    .retained_targets
                    .0
                    .iter()
                    .find(|(t, _)| *t == target)
                    .unwrap();

                panel
                    .elements
                    .iter()
                    .position(|e| e.instance == *instance)
                    .map(|index| Target {
                        panel: target.panel,
                        owner: Owner::Element(index),
                    })
            }
            Owner::ListRow {
                element,
                child: Some(_),
                ..
            } => {
                let Some((_, node)) = self.retained_targets.0.iter().find(|(t, _)| *t == target)
                else {
                    return retained_element(panel, target.owner).map(|_| target);
                };

                if retained_element(panel, target.owner).is_some_and(|e| e.instance == *node) {
                    return Some(target);
                }

                let list = panel.elements.get(element)?.widget.list()?;

                list.items.iter().enumerate().find_map(|(row, item)| {
                    item.children
                        .iter()
                        .position(|e| e.instance == *node)
                        .map(|child| Target {
                            panel: target.panel,
                            owner: Owner::ListRow {
                                element,
                                row,
                                child: Some(child),
                            },
                        })
                })
            }
            Owner::Descendant { item: None, .. } => {
                retained_element(panel, target.owner).map(|_| target)
            }
            _ => Some(target),
        }
    }

    fn remember_panel(&mut self, index: usize, panel: &Panel) {
        let targets = [self.focused, self.pressed, self.hovered, self.capture];

        self.retained_targets
            .0
            .retain(|(t, _)| t.panel != index && targets.contains(&Some(*t)));

        for target in targets.into_iter().flatten() {
            if target.panel == index
                && !self.retained_targets.0.iter().any(|(t, _)| *t == target)
                && let Some(e) = retained_element(panel, target.owner)
                && (matches!(target.owner, Owner::ListRow { child: Some(_), .. })
                    || matches!(target.owner, Owner::Element(_)) && e.widget.edit().is_some())
            {
                self.retained_targets.0.push((target, e.instance));
            }
        }
    }

    pub fn reconcile_panel(&mut self, index: usize, panel: &Panel) {
        let resolve = |target: Option<Target>| {
            target.and_then(|t| {
                if t.panel == index {
                    self.retained_target(t, panel)
                } else {
                    Some(t)
                }
            })
        };

        let next = [
            resolve(self.focused),
            resolve(self.pressed),
            resolve(self.hovered),
            resolve(self.capture),
        ];

        [self.focused, self.pressed, self.hovered, self.capture] = next;

        self.remember_panel(index, panel);
    }

    pub fn leave_hovered_panel(&mut self, index: usize, panel: &mut Panel) {
        self.reconcile_panel(index, panel);

        let Some(hovered) = self.hovered.filter(|t| t.panel == index) else {
            return;
        };

        self.hovered = None;

        if let Some(pressed) = self.pressed.filter(|t| t.panel == index) {
            let mut owner = Some(pressed.owner);

            while let Some(current) = owner {
                if current == hovered.owner {
                    if retained_element(panel, pressed.owner)
                        .is_some_and(button_leave_cancels_press)
                    {
                        self.pressed = None;
                    }

                    break;
                }

                owner = parent_owner(panel, current);
            }
        }

        fn clear(e: &mut Element) {
            if let Some(list) = e.widget.list_mut() {
                if let Some(custom) = &mut list.custom {
                    custom.hovered = None;
                }

                if let Some(tree) = &mut list.tree {
                    tree.hovered = None;
                }

                for row in &mut list.items {
                    for child in &mut row.children {
                        clear(child);
                    }
                }
            }
        }

        if let Some(e) = retained_element_mut(panel, hovered.owner) {
            clear(e);
        }

        let mut parent = parent_owner(panel, hovered.owner);

        while let Some(owner) = parent {
            if let Some(list) = retained_element_mut(panel, owner).and_then(|e| e.widget.list_mut())
            {
                if let Some(custom) = &mut list.custom {
                    custom.hovered = None;
                }

                if let Some(tree) = &mut list.tree {
                    tree.hovered = None;
                }
            }

            parent = parent_owner(panel, owner);
        }

        self.remember_panel(index, panel);
    }

    pub fn activate_button<'a>(
        &mut self,
        target: Target,
        panels: impl IntoIterator<Item = (usize, &'a mut Panel)>,
    ) -> Option<Vec<Event>> {
        let mut panels: Vec<_> = panels.into_iter().collect();

        let panel = &panels.iter().find(|(index, _)| *index == target.panel)?.1;

        let element = retained_element(panel, target.owner)?;

        if !matches!(
            element.element_type,
            ElementType::Button | ElementType::IconButton
        ) {
            return None;
        }

        let button = element.widget.button()?;

        let admits =
            panel.allow_focus && element.visible && element.enabled && button.focus_enabled;

        let event = Event {
            panel: target.panel,
            owner: Some(target.owner),
            name: element.name.clone(),
            extra: 0,
            id: EventId::Click,
        };

        if admits {
            self.request_focus(Some(target), panels.iter_mut().map(|(i, p)| (*i, &mut **p)));
        }

        Some(vec![event])
    }

    pub fn activate_checkable<'a>(
        &mut self,
        target: Target,
        panels: impl IntoIterator<Item = (usize, &'a mut Panel)>,
    ) -> Option<Vec<Event>> {
        let mut panels: Vec<_> = panels.into_iter().collect();

        let panel = &panels.iter().find(|(index, _)| *index == target.panel)?.1;

        let element = retained_element(panel, target.owner)?;

        let kind = element.element_type;

        if !matches!(kind, ElementType::CheckBox | ElementType::RadioButton) {
            return None;
        }

        let button = element.widget.button()?;

        let checked = !button.checked;

        let admits =
            panel.allow_focus && element.visible && element.enabled && button.focus_enabled;

        if admits {
            self.request_focus(Some(target), panels.iter_mut().map(|(i, p)| (*i, &mut **p)));
        }

        let panel = &mut panels
            .iter_mut()
            .find(|(index, _)| *index == target.panel)?
            .1;

        if kind == ElementType::CheckBox {
            Some(
                set_checkbox_checked(target.panel, panel, target.owner, checked, true)
                    .into_iter()
                    .collect(),
            )
        } else {
            set_radio_checked(target.panel, panel, target.owner, true, true, true)
        }
    }

    pub fn activate_combo_box<'a>(
        &mut self,
        target: Target,
        panels: impl IntoIterator<Item = (usize, &'a mut Panel)>,
    ) -> Option<Vec<Event>> {
        let mut panels: Vec<_> = panels.into_iter().collect();

        let panel = &panels.iter().find(|(index, _)| *index == target.panel)?.1;

        let element = retained_element(panel, target.owner)?;

        let combo = element.widget.combo_box()?;

        if combo.open || combo.selected == -1 {
            return Some(Vec::new());
        }

        let admits = panel.allow_focus && element.visible && element.enabled;

        if admits {
            self.request_focus(Some(target), panels.iter_mut().map(|(i, p)| (*i, &mut **p)));
        }

        let panel = &mut panels
            .iter_mut()
            .find(|(index, _)| *index == target.panel)?
            .1;

        let element = retained_element_mut(panel, target.owner)?;

        let combo = element.widget.combo_box_mut()?;

        combo.selected = combo.selected.saturating_add(1);

        if i64::from(combo.selected) >= i64::try_from(combo.items.len()).unwrap_or(i64::MAX) {
            combo.selected = 0;
        }

        combo.hovered = combo.selected;

        Some(vec![Event {
            panel: target.panel,
            owner: Some(target.owner),
            name: element.name.clone(),
            id: EventId::ComboChanged,
            extra: i64::from(combo.selected),
        }])
    }

    pub fn button_key(&mut self, index: usize, panel: &Panel, vk: u32) -> Option<Vec<Event>> {
        self.reconcile_panel(index, panel);

        if !panel.visible {
            return None;
        }

        let target = self.focused.filter(|target| target.panel == index)?;

        let element = retained_element(panel, target.owner)?;

        if !matches!(
            element.element_type,
            ElementType::Button | ElementType::IconButton
        ) || !element.visible
            || !element.enabled
        {
            return None;
        }

        let button = element.widget.button()?;

        if !button.group.is_empty()
            || !match vk {
                0x0d => button.default_key,
                0x1b => button.cancel_key,
                _ => false,
            }
        {
            return None;
        }

        if element.element_type == ElementType::Button {
            self.button_sounds.push(53);
        }

        Some(vec![Event {
            panel: index,
            id: EventId::Click,
            owner: Some(target.owner),
            name: element.name.clone(),
            extra: 0,
        }])
    }

    pub fn checkable_key(
        &mut self,
        index: usize,
        panel: &mut Panel,
        down: bool,
    ) -> Option<Vec<Event>> {
        self.reconcile_panel(index, panel);

        let target = self.focused.filter(|target| target.panel == index)?;

        let (element, _) = owner_element(panel, target.owner)?;

        if !checkable(element) || !element.msg_receive {
            return None;
        }

        let mut parent = parent_owner(panel, target.owner);

        while let Some(owner) = parent {
            if !retained_element(panel, owner)?.msg_receive {
                return None;
            }

            parent = parent_owner(panel, owner);
        }

        if down {
            self.pressed = Some(target);

            return Some(Vec::new());
        }

        if self.pressed != Some(target) {
            return Some(Vec::new());
        }

        let kind = element.element_type;

        self.pressed = None;

        if kind == ElementType::CheckBox {
            let checked = !retained_element(panel, target.owner)?
                .widget
                .button()?
                .checked;

            return Some(
                set_checkbox_checked(index, panel, target.owner, checked, true)
                    .into_iter()
                    .collect(),
            );
        }

        set_radio_checked(index, panel, target.owner, true, true, true)
    }

    fn prepare_element(
        &mut self,
        e: &mut Element,
        owner: Owner,
        offset: [i32; 2],
        active: bool,
        walk: &MouseWalk,
        events: &mut Vec<Event>,
    ) {
        let active = active && interactive(e);

        let target = Target {
            panel: walk.panel,
            owner,
        };

        let x = e.x.saturating_add(offset[0]);

        let y = e.y.saturating_add(offset[1]);

        let rect = [x, y, x.saturating_add(e.width), y.saturating_add(e.height)];

        let rows = container_rows(e);

        if let Some(list) = e.widget.list_mut() {
            if !active {
                list.scrollbar.drag_offset = None;

                list.scrollbar.press = 0;

                list.scrollbar.hover = [false, false];

                list.selection_held = false;
            }

            if matches!(e.element_type, ElementType::ListBox | ElementType::ListView)
                && list.scrollbar.drag_offset.is_none()
                && !list.selection_held
                && self.capture == Some(target)
            {
                self.capture = None;

                self.pressed = None;
            }

            if let Some(custom) = &mut list.custom {
                custom.hovered = None;

                if !active {
                    custom.drag_offset = None;
                }

                if !active {
                    custom.focused = None;
                }
            }

            if let Some(tree) = &mut list.tree {
                tree.hovered = None;

                if !active {
                    tree.drag_offset = None;

                    if self.capture == Some(target) {
                        self.capture = None;

                        self.pressed = None;
                    }
                }
            }

            for (row, item) in list.items.iter_mut().enumerate() {
                let placement = rows.iter().find(|(id, _)| *id == row).map(|(_, r)| *r);

                for (child, e) in item.children.iter_mut().enumerate() {
                    let child_owner = crate::draw::child_owner(owner, row, child, e);

                    let child_offset = placement.map_or(offset, |r| {
                        [
                            r[0].saturating_add(offset[0]),
                            r[1].saturating_add(offset[1]),
                        ]
                    });

                    self.prepare_element(
                        e,
                        child_owner,
                        child_offset,
                        active && placement.is_some() && e.msg_receive,
                        walk,
                        events,
                    );
                }
            }
        }

        if !active && self.capture == Some(target) {
            self.capture = None;

            self.pressed = None;
        }

        if let Some(slider) = e.widget.slider_mut() {
            if !active {
                slider.drag_offset = None;
            }

            if slider.drag_offset.is_none() && self.capture == Some(target) {
                self.capture = None;

                self.pressed = None;
            }
        }

        if let Some(pad) = e.widget.edit_mut().and_then(|edit| edit.pad.as_mut()) {
            if !active {
                pad.scrollbar.drag_offset = None;

                pad.scrollbar.press = 0;

                pad.scrollbar.hover = [false, false];

                pad.selection_held = false;
            }

            if pad.scrollbar.drag_offset.is_none()
                && !pad.selection_held
                && self.capture == Some(target)
            {
                self.capture = None;

                self.pressed = None;
            }
        }

        if let Some(edit) = e.widget.edit_mut().filter(|edit| !edit.note_pad) {
            if !active {
                edit.selection_held = false;
            }

            if !edit.selection_held && self.capture == Some(target) {
                self.capture = None;

                self.pressed = None;
            }
        }

        if walk.event == Mouse::Move
            && self.pressed == Some(target)
            && (self.capture == Some(target) || self.hovered == Some(target))
            && button_leave_cancels_press(e)
            && !element_contains(e, rect, walk.point[0], walk.point[1])
        {
            self.pressed = None;
        }

        let Some(combo) = e.widget.combo_box_mut() else {
            return;
        };

        if let Some(screen) = walk.screen_y {
            combo.screen = Some(screen);
        }

        if !active || self.focused.is_some_and(|t| t != target) {
            combo.open = false;

            combo.drag_offset = None;

            if self.capture == Some(target) {
                self.capture = None;

                self.pressed = None;
            }
        } else if combo.open && matches!(walk.event, Mouse::LeftDown | Mouse::LeftDouble) {
            let [lx, ly] = walk.point;

            let inside = combo.stored_layout(rect, &e.layers).is_some_and(|l| {
                in_rect(l.popup, lx, ly) || (l.page < combo.items.len() && l.bar.contains(lx, ly))
            });

            if !in_rect(rect, lx, ly) && !inside {
                combo.open = false;

                combo.hovered = combo.selected;

                combo.drag_offset = None;

                events.push(Event {
                    panel: walk.panel,
                    id: EventId::ComboChanged,
                    owner: Some(owner),
                    name: e.name.clone(),
                    extra: i64::from(combo.selected),
                });
            }
        }
    }

    pub fn request_focus<'a>(
        &mut self,
        next: Option<Target>,
        panels: impl IntoIterator<Item = (usize, &'a mut Panel)>,
    ) -> bool {
        if self.focused == next {
            return false;
        }

        let panels: Vec<_> = panels.into_iter().collect();

        if self.focused.is_some_and(|previous| {
            panels
                .iter()
                .any(|(index, panel)| *index == previous.panel && panel.focus_protected)
        }) {
            return false;
        }

        self.set_focus(next, panels);

        true
    }

    pub fn set_focus<'a>(
        &mut self,
        next: Option<Target>,
        panels: impl IntoIterator<Item = (usize, &'a mut Panel)>,
    ) {
        let panels: Vec<_> = panels.into_iter().collect();

        for (index, panel) in &panels {
            self.reconcile_panel(*index, panel);
        }

        let previous = self.focused;

        self.focused = next;

        for (index, panel) in panels {
            if let Some(previous) = previous.filter(|t| t.panel == index && Some(*t) != next) {
                self.kill_combo_focus(previous, std::iter::once((index, &mut *panel)));
            }

            if let Some(next) = next.filter(|t| t.panel == index && Some(*t) != previous)
                && let Some(edit) =
                    retained_element_mut(panel, next.owner).and_then(|e| e.widget.edit_mut())
            {
                edit.reset_caret();
            }

            self.remember_panel(index, panel);
        }
    }

    fn kill_combo_focus<'a>(
        &mut self,
        previous: Target,
        panels: impl IntoIterator<Item = (usize, &'a mut Panel)>,
    ) {
        let Some((_, panel)) = panels
            .into_iter()
            .find(|(index, _)| *index == previous.panel)
        else {
            return;
        };

        let Some(previous) = self.retained_target(previous, panel) else {
            return;
        };

        let element = retained_element_mut(panel, previous.owner);

        if let Some(combo) = element.and_then(|e| e.widget.combo_box_mut()) {
            combo.open = false;

            combo.drag_offset = None;

            if self.capture == Some(previous) {
                self.capture = None;
            }

            if self.pressed == Some(previous) {
                self.pressed = None;
            }

            match previous.owner {
                Owner::Element(i) if panel.focus == Some(i) => panel.focus = None,
                Owner::ListRow {
                    element,
                    row,
                    child: Some(child),
                } => {
                    if let Some(custom) = panel.elements[element]
                        .widget
                        .list_mut()
                        .and_then(|l| l.custom.as_mut())
                        && custom.focused == Some((row, child))
                    {
                        custom.focused = None;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn mouse(
        &mut self,
        panels: &mut [PanelMut<'_>],
        event: Mouse,
        x: i32,
        y: i32,
    ) -> Vec<Event> {
        let order: Vec<usize> = (0..panels.len()).collect();

        self.mouse_in_order(panels, &order, event, x, y)
    }

    pub fn mouse_in_order(
        &mut self,
        panels: &mut [PanelMut<'_>],
        order: &[usize],
        event: Mouse,
        x: i32,
        y: i32,
    ) -> Vec<Event> {
        for (index, panel) in panels.iter().enumerate() {
            self.reconcile_panel(index, panel.panel);
        }

        let previous_hover = self.hovered.and_then(|target| {
            let p = panels.get(target.panel)?;

            let (x, y) = p.local(self.cursor.0, self.cursor.1);

            Some(Target {
                owner: refine_hover(p.panel, target.owner, x, y),
                ..target
            })
        });

        self.cursor = (x, y);

        let previous_focus = self.focused;

        let focus_protected = previous_focus
            .is_some_and(|t| panels.get(t.panel).is_some_and(|p| p.panel.focus_protected));

        let mut events = Vec::new();

        if let Some(target) = self.capture
            && matches!(target.owner, Owner::Descendant { item: None, .. })
            && panels
                .get(target.panel)
                .is_none_or(|p| owner_element(p.panel, target.owner).is_none())
        {
            self.capture = None;

            self.pressed = None;
        }

        for (panel_index, p) in panels.iter_mut().enumerate() {
            let (lx, ly) = p.local(x, y);

            let walk = MouseWalk {
                panel: panel_index,
                event,
                point: [lx, ly],
                screen_y: (self.screen[3] > self.screen[1]).then_some([
                    self.screen[1].saturating_sub(p.origin.1),
                    self.screen[3].saturating_sub(p.origin.1),
                ]),
            };

            let visible = p.panel.visible;

            let page = p.panel.page_index;

            let [left, top, right, bottom] = p.panel.close_button_rect();

            if let Some(close) = &mut p.panel.close_button {
                close.x = left;

                close.y = top;

                close.width = right.saturating_sub(left);

                close.height = bottom.saturating_sub(top);

                self.prepare_element(close, Owner::Close, [0, 0], visible, &walk, &mut events);
            }

            for (i, e) in p.panel.elements.iter_mut().enumerate() {
                let on_page = page == -1 || e.page_index == 0xff || page == i32::from(e.page_index);

                self.prepare_element(
                    e,
                    Owner::Element(i),
                    [0, 0],
                    visible && on_page,
                    &walk,
                    &mut events,
                );
            }
        }

        let right_release_disarms = self.pressed.is_some_and(|target| {
            target.owner != Owner::Close
                && panels
                    .get(target.panel)
                    .and_then(|panel| retained_element(panel.panel, target.owner))
                    .is_some_and(|element| ordinary_button(element) || grouped_button(element))
        });

        let released_scrollbar = self.capture.filter(|target| {
            self.drag.is_none()
                && event.is_button_up()
                && panels
                    .get(target.panel)
                    .and_then(|p| retained_element(p.panel, target.owner))
                    .is_some_and(crate::scrollbar::thumb_dragging)
        });

        let slider_capture = self.capture.filter(|t| {
            panels
                .get(t.panel)
                .and_then(|p| retained_element(p.panel, t.owner))
                .is_some_and(|e| {
                    e.widget.slider().is_some()
                        || e.widget.combo_box().is_some()
                        || e.widget.list().is_some()
                        || e.widget.edit().is_some()
                })
        });

        let canceled_button_capture = self.capture.is_some_and(|t| {
            self.pressed.is_none()
                && panels
                    .get(t.panel)
                    .and_then(|p| retained_element(p.panel, t.owner))
                    .is_some_and(|e| {
                        ordinary_button(e) || checkable(e) || e.widget.combo_box().is_some()
                    })
        });

        let releases_capture = event.is_button_up()
            && self.capture.is_some()
            && (self.capture == self.pressed || canceled_button_capture)
            && (slider_capture.is_none() || event == Mouse::LeftUp);

        if self.carried.is_some() && event == Mouse::RightDown {
            self.cancel_carry();

            return events;
        }

        let candidates: Vec<usize> = match (self.drag, self.capture) {
            (Some(drag), _) => vec![drag.panel],
            (None, Some(target)) => vec![target.panel],
            (None, None) => order.to_vec(),
        };

        let held = self.drag.is_some() || self.capture.is_some();

        let mut point_hover = None;

        for index in candidates {
            let Some(panel_ref) = panels.get_mut(index) else {
                continue;
            };

            if !panel_ref.panel.visible {
                continue;
            }

            if matches!(event, Mouse::LeftDown | Mouse::LeftDouble) {
                panel_ref.panel.input_focus_latched = true;
            }

            let focused_slider = self.focused_drag_motion(index, panel_ref.panel, event);

            if !held && !panel_ref.contains(x, y) && focused_slider.is_none() {
                continue;
            }

            if self.panel_drag(index, panel_ref, event, x, y, &mut events) {
                break;
            }

            let (lx, ly) = panel_ref.local(x, y);

            let hit = hit_test(panel_ref.panel, lx, ly);

            point_hover = hit.map(|owner| Target {
                panel: index,
                owner: refine_hover(panel_ref.panel, owner, lx, ly),
            });

            let owner = self
                .capture
                .filter(|capture| {
                    capture.panel == index
                        && (releases_capture
                            || matches!(
                                capture.owner,
                                Owner::ListRow { child: Some(_), .. }
                                    | Owner::Descendant { item: None, .. }
                            )
                            || matches!(
                                capture.owner, Owner::Element(i) if panel_ref.panel.elements
                                .get(i).is_some_and(| e | e.widget.slider().is_some() || e
                                .widget.combo_box().is_some() || e.widget.list().is_some()
                                || e.widget.edit().is_some())
                            ))
                })
                .map(|capture| capture.owner)
                .or(focused_slider.map(|target| target.owner))
                .or(hit);

            let focus_allowed = panel_ref.panel.allow_focus && !focus_protected;

            let release_clears_focus = !focus_protected
                && !panel_ref.panel.input_focus_latched
                && matches!(event, Mouse::LeftUp | Mouse::RightUp)
                && self.pressed.is_some_and(|t| {
                    t.panel == index
                        && Some(t.owner) == owner
                        && t.owner != Owner::Close
                        && retained_element(panel_ref.panel, t.owner)
                            .is_some_and(|e| ordinary_button(e) && interactive(e) && e.msg_receive)
                });

            events.extend(self.deliver(
                index,
                panel_ref.panel,
                owner,
                event,
                (lx, ly),
                MouseAdmission {
                    parent_callback: false,
                    focus_allowed,
                },
            ));

            if release_clears_focus {
                self.focused = None;
            }

            if event == Mouse::Move
                && self.carried.is_none()
                && let Some(capture) = self.capture.filter(|t| t.panel == index)
                && matches!(
                    capture.owner,
                    Owner::ListRow { child: Some(_), .. } | Owner::Descendant { item: None, .. }
                )
                && retained_element(panel_ref.panel, capture.owner)
                    .is_some_and(capturing_left_button)
            {
                self.hovered = hit.map(|owner| Target {
                    panel: index,
                    owner: if matches!(
                        owner, Owner::Element(i) if panel_ref.panel.elements.get(i)
                        .is_some_and(| e | e.element_type == ElementType::ListView)
                    ) {
                        owner
                    } else {
                        refine_hover(panel_ref.panel, owner, lx, ly)
                    },
                });
            }

            break;
        }

        if event == Mouse::Move
            && self.carried.is_none()
            && self.drag.is_none()
            && let Some(target) = self.pressed
            && (self.capture == Some(target) || previous_hover == Some(target))
            && point_hover != Some(target)
            && panels
                .get(target.panel)
                .and_then(|p| retained_element(p.panel, target.owner))
                .is_some_and(button_leave_cancels_press)
        {
            self.pressed = None;

            if self.hovered == Some(target) {
                self.hovered = point_hover;
            }
        }

        if previous_focus != self.focused
            && let Some(next) = self.focused
        {
            let refused = panels.get(next.panel).is_none_or(|p| {
                !p.panel.allow_focus
                    || retained_element(p.panel, next.owner).is_some_and(|e| {
                        matches!(
                            e.element_type,
                            ElementType::Button
                                | ElementType::IconButton
                                | ElementType::CheckBox
                                | ElementType::RadioButton
                        ) && e.widget.button().is_some_and(|b| !b.focus_enabled)
                    })
            }) || previous_focus.is_some_and(|old| {
                panels
                    .get(old.panel)
                    .is_some_and(|p| p.panel.focus_protected)
            });

            if refused {
                self.focused = previous_focus;
            }
        }

        if previous_focus != self.focused
            && let Some(previous) = previous_focus
        {
            self.kill_combo_focus(
                previous,
                panels
                    .iter_mut()
                    .enumerate()
                    .map(|(i, p)| (i, &mut *p.panel)),
            );
        }

        if self.carried.is_some() {
            self.update_drop(panels, order);
        }

        if event == Mouse::LeftUp || (event == Mouse::RightUp && right_release_disarms) {
            self.pressed = None;
        }

        if releases_capture {
            self.capture = None;
        }

        if let Some(target) = released_scrollbar {
            if let Some(element) = panels
                .get_mut(target.panel)
                .and_then(|p| retained_element_mut(p.panel, target.owner))
            {
                crate::scrollbar::release_thumb(element);
            }

            if self.capture == Some(target) {
                self.capture = None;
            }

            if self.pressed == Some(target) {
                self.pressed = None;
            }
        }

        if event == Mouse::LeftUp
            && let Some(target) = slider_capture
            && let Owner::Element(i) = target.owner
            && let Some(slider) = panels
                .get_mut(target.panel)
                .and_then(|p| p.panel.elements.get_mut(i))
                .and_then(|e| e.widget.slider_mut())
        {
            slider.drag_offset = None;
        }

        if event == Mouse::LeftUp {
            for panel in panels.iter_mut() {
                for element in &mut panel.panel.elements {
                    crate::notepad::release_pointer(element);
                }
            }
        }

        for (index, panel) in panels.iter().enumerate() {
            self.remember_panel(index, panel.panel);
        }

        events
    }

    pub fn pick_up(&mut self, carried: Carried) -> bool {
        if self.carried.is_some() {
            return false;
        }

        self.carried = Some(carried);

        true
    }

    pub fn cancel_carry(&mut self) -> Option<Carried> {
        self.drop = None;

        self.carried.take()
    }

    fn update_drop(&mut self, panels: &[PanelMut<'_>], order: &[usize]) {
        let Some(carried) = self.carried else {
            return;
        };

        let (x, y) = self.cursor;

        let (half_w, half_h) = (carried.size.0 / 2, carried.size.1 / 2);

        self.drop = None;

        for &index in order {
            let Some(panel) = panels.get(index) else {
                continue;
            };

            if !panel.panel.visible {
                continue;
            }

            if !panel.contains(x, y) {
                continue;
            }

            let (lx, ly) = panel.local(x, y);

            let local = [lx - half_w, ly - half_h, lx + half_w, ly + half_h];

            let Some((element, hit)) = overlap_hit_test(panel.panel, local) else {
                continue;
            };

            let candidate = DropPanel {
                panel: index,
                element,
                rect: hit,
            };

            if self.drop.is_none_or(|best| best.area() <= candidate.area()) {
                self.drop = Some(candidate);
            }
        }

        self.hovered = self.drop.map(|d| Target {
            panel: d.panel,
            owner: Owner::Element(d.element),
        });
    }

    pub fn drop_at(&self, panels: &[PanelRef<'_>]) -> Option<Target> {
        self.carried?;

        let drop = self.drop?;

        let panel = panels.get(drop.panel)?;

        let (lx, ly) = panel.local(self.cursor.0, self.cursor.1);

        let owner = drop_hit_test(panel.panel, lx, ly)?;

        Some(Target {
            panel: drop.panel,
            owner,
        })
    }

    fn panel_drag(
        &mut self,
        index: usize,
        panel: &mut PanelMut<'_>,
        event: Mouse,
        x: i32,
        y: i32,
        events: &mut Vec<Event>,
    ) -> bool {
        if !panel.panel.titlebar && panel.panel.resize_tip == 0 {
            return false;
        }

        match event {
            Mouse::LeftDown => {
                self.arm_drag(index, panel, x, y);

                false
            }
            Mouse::LeftUp => {
                self.drag = None;

                false
            }
            Mouse::Move => self.drag_to(index, panel, x, y, events),
            Mouse::LeftDouble | Mouse::RightDown | Mouse::RightDouble | Mouse::RightUp => false,
        }
    }

    fn arm_drag(&mut self, index: usize, panel: &mut PanelMut<'_>, x: i32, y: i32) {
        let (left, top) = *panel.origin;

        let (w, h) = (panel.panel.width(), panel.panel.height());

        if panel.panel.resize_tip != 0 {
            let edge = if panel.panel.resize_tip & 1 != 0
                && in_rect([left, top, left + w, top + 1], x, y)
            {
                Some(DragMode::ResizeTop)
            } else if panel.panel.resize_tip & 2 != 0
                && in_rect([left, top + h - 1, left + w, top + h], x, y)
            {
                Some(DragMode::ResizeBottom)
            } else {
                None
            };

            if let Some(mode) = edge {
                self.drag = Some(PanelDrag {
                    panel: index,
                    mode,
                    grab: (x - left, y - top),
                    grabbed: (w, h),
                    moved: true,
                });

                return;
            }
        }

        if self.drag.is_some() || !panel.panel.titlebar {
            return;
        }

        if !in_rect(panel.titlebar_rect(), x, y) {
            return;
        }

        let (lx, ly) = (x - left, y - top);

        if panel.panel.close_button.is_some() && in_rect(panel.panel.close_button_rect(), lx, ly) {
            return;
        }

        self.drag = Some(PanelDrag {
            panel: index,
            mode: DragMode::Move,
            grab: (lx, ly),
            grabbed: (w, h),
            moved: false,
        });
    }

    fn drag_to(
        &mut self,
        index: usize,
        panel: &mut PanelMut<'_>,
        x: i32,
        y: i32,
        events: &mut Vec<Event>,
    ) -> bool {
        let Some(mut drag) = self.drag.filter(|d| d.panel == index) else {
            return false;
        };

        let (lx, ly) = panel.local(x, y);

        if drag.mode == DragMode::Move && !drag.moved {
            if (drag.grab.0 - lx).abs() < DEAD_ZONE && (drag.grab.1 - ly).abs() < DEAD_ZONE {
                return true;
            }

            drag.moved = true;

            self.drag = Some(drag);
        }

        if !panel.panel.movable {
            return true;
        }

        let [vl, _vt, vr, vb] = self.screen;

        let (w, h) = (panel.panel.width(), panel.panel.height());

        match drag.mode {
            DragMode::Move => {
                let (mut left, mut top) = (x - drag.grab.0, y - drag.grab.1);

                let (mut right, mut bottom) = (left + w, top + h);

                if left < 0 {
                    right -= left;

                    left = 0;
                }

                if top < 0 {
                    bottom -= top;

                    top = 0;
                }

                if left < vl {
                    right += vl - left;

                    left = vl;
                }

                if right >= vr {
                    left += vr - right;
                }

                if bottom >= vb {
                    top += vb - bottom;
                }

                *panel.origin = (left, top);
            }
            DragMode::ResizeTop => {
                let new_top = (y - drag.grab.1)
                    .min(panel.origin.1 + h - MIN_HEIGHT)
                    .max(0);

                panel.panel.bottom += panel.origin.1 - new_top;

                panel.origin.1 = new_top;
            }
            DragMode::ResizeBottom => {
                let new_bottom = (y - drag.grab.1 + drag.grabbed.1)
                    .max(panel.origin.1 + MIN_HEIGHT)
                    .min(vb);

                panel.panel.bottom = panel.panel.top + (new_bottom - panel.origin.1);
            }
        }

        if drag.mode != DragMode::Move {
            events.push(Event {
                panel: index,
                id: EventId::Resized,
                owner: None,
                name: String::new(),
                extra: -1,
            });
        }

        true
    }

    fn focused_drag_motion(&self, index: usize, panel: &Panel, event: Mouse) -> Option<Target> {
        if !matches!(event, Mouse::Move | Mouse::LeftUp) {
            return None;
        }

        self.focused.filter(|target| {
            target.panel == index
                && owner_element(panel, target.owner).is_some_and(|(element, _)| {
                    element.widget.slider().is_some_and(|slider| {
                        slider.drag_offset.is_some() && !slider.mouse_move_lock
                    }) || crate::scrollbar::state(element).is_some_and(|bar| bar.press != 0)
                })
        })
    }

    fn deliver_container_child(
        &mut self,
        index: usize,
        panel: &mut Panel,
        owner: Option<Owner>,
        event: Mouse,
        point: (i32, i32),
        focus_allowed: bool,
    ) -> Option<Vec<Event>> {
        let (lx, ly) = point;

        let element_index = match owner? {
            Owner::Element(i)
            | Owner::ListRow { element: i, .. }
            | Owner::Descendant { element: i, .. } => i,
            _ => return None,
        };

        let element = panel.elements.get(element_index)?;

        if !interactive(element)
            || !matches!(
                element.element_type,
                ElementType::ListView | ElementType::TreeView | ElementType::CustomListView
            )
        {
            return None;
        }

        let ordinary_list = element.element_type == ElementType::ListView;

        let rows = container_rows(element);

        let state = element.widget.list()?;

        if self.focused
            == Some(Target {
                panel: index,
                owner: Owner::Element(element_index),
            })
            && state.scrollbar.press != 0
            && matches!(event, Mouse::Move | Mouse::LeftUp)
        {
            return None;
        }

        if self.capture
            == Some(Target {
                panel: index,
                owner: Owner::Element(element_index),
            })
            && (state.scrollbar.drag_offset.is_some()
                || state
                    .tree
                    .as_ref()
                    .is_some_and(|tree| tree.drag_offset.is_some())
                || state
                    .custom
                    .as_ref()
                    .is_some_and(|custom| custom.drag_offset.is_some()))
        {
            return None;
        }

        let captured = self
            .capture
            .or_else(|| self.focused_drag_motion(index, panel, event))
            .filter(|t| t.panel == index)
            .and_then(|t| match t.owner {
                Owner::ListRow {
                    element,
                    row,
                    child: Some(child),
                } if element == element_index => Some((row, child)),
                Owner::Descendant {
                    element,
                    node,
                    item: None,
                } if element == element_index => {
                    state.items.iter().enumerate().find_map(|(row, item)| {
                        item.children
                            .iter()
                            .position(|e| find_instance(e, node).is_some())
                            .map(|child| (row, child))
                    })
                }
                _ => None,
            });

        let bar = crate::scrollbar::layout(element).map(|layout| layout.rects);

        if captured.is_none()
            && bar.is_some_and(|b| {
                in_rect(b.track, lx, ly) || in_rect(b.up, lx, ly) || in_rect(b.down, lx, ly)
            })
        {
            return None;
        }

        let hit = captured
            .or_else(|| container_popup_at(element, lx, ly))
            .or_else(|| {
                list_child_hit(element, lx, ly, event != Mouse::Move)
                    .map(|(row, child, _)| (row, child))
            })?;

        let (row, child) = hit;

        let rect = rows.iter().find(|(i, _)| *i == row)?.1;

        let item = state.items.get(row)?;

        let target = item.children.get(child)?;

        if !interactive(target) || !target.msg_receive || !container_mouse_child(target) {
            return None;
        }

        let group = matches!(event, Mouse::LeftDown | Mouse::LeftDouble)
            .then_some(target)
            .filter(|e| grouped_button(e))
            .map(|e| (e.instance, e.widget.button().unwrap().group.clone()));

        if event == Mouse::LeftDown && focus_allowed {
            self.focused = Some(Target {
                panel: index,
                owner: Owner::Element(element_index),
            });

            panel.focus = Some(element_index);
        }

        if let Some((instance, group)) = group {
            clear_button_group(panel, instance, &group);
        }

        let mut children = std::mem::take(
            &mut panel.elements[element_index].widget.list_mut()?.items[row].children,
        );

        let offsets: Vec<_> = children.iter().map(|e| (e.x, e.y)).collect();

        for e in &mut children {
            e.x = e.x.saturating_add(rect[0]);

            e.y = e.y.saturating_add(rect[1]);
        }

        let mut local_panel = Panel {
            visible: true,
            elements: children,
            force_notifications: panel.force_notifications,
            ..Panel::default()
        };

        let local = |p: &Panel, target: Option<Target>| {
            target.and_then(|t| match t.owner {
                Owner::ListRow {
                    element,
                    row: r,
                    child: Some(c),
                } if t.panel == index && element == element_index && r == row => Some(Target {
                    panel: index,
                    owner: Owner::Element(c),
                }),
                Owner::Descendant {
                    element,
                    node,
                    item,
                } if t.panel == index && element == element_index => {
                    let child = p
                        .elements
                        .iter()
                        .position(|e| find_instance(e, node).is_some())?;

                    let owner = if p.elements[child].instance == node && item.is_none() {
                        Owner::Element(child)
                    } else {
                        Owner::Descendant {
                            element: child,
                            node,
                            item,
                        }
                    };

                    Some(Target {
                        panel: index,
                        owner,
                    })
                }
                _ => None,
            })
        };

        let mut routing = self.clone();

        routing.focused = local(&local_panel, self.focused);

        routing.hovered = local(&local_panel, self.hovered);

        routing.button_sounds.clear();

        routing.pressed = local(&local_panel, self.pressed);

        routing.capture = local(&local_panel, self.capture);

        let mut events = routing.deliver(
            index,
            &mut local_panel,
            Some(Owner::Element(child)),
            event,
            (lx, ly),
            MouseAdmission {
                parent_callback: true,
                focus_allowed,
            },
        );

        self.button_sounds.append(&mut routing.button_sounds);

        let callbacks: Vec<_> = events
            .iter()
            .map(|e| {
                e.owner == Some(Owner::Element(child))
                    && ((ordinary_button(&local_panel.elements[child])
                        && matches!(
                            e.id,
                            EventId::Click | EventId::RightUp | EventId::DoubleClick
                        ))
                        || (grouped_button(&local_panel.elements[child])
                            && e.id == EventId::RightUp))
            })
            .collect();

        let outer = |target: Option<Target>| {
            target.and_then(|t| {
                let owner = match t.owner {
                    Owner::Element(child) => Owner::ListRow {
                        element: element_index,
                        row,
                        child: Some(child),
                    },
                    Owner::ListRow {
                        element: child,
                        row: item,
                        child: None,
                    } => Owner::ListChildRow {
                        element: element_index,
                        row,
                        child,
                        item,
                    },
                    Owner::Descendant { node, item, .. } => Owner::Descendant {
                        element: element_index,
                        node,
                        item,
                    },
                    other => Owner::Descendant {
                        element: element_index,
                        node: retained_element(&local_panel, other)?.instance,
                        item: None,
                    },
                };

                Some(Target {
                    panel: index,
                    owner,
                })
            })
        };

        let restore = |next, previous| {
            outer(next).or_else(|| {
                if local(&local_panel, previous).is_none() {
                    previous
                } else {
                    None
                }
            })
        };

        self.focused = restore(routing.focused, self.focused);

        self.hovered = if ordinary_list {
            routing.hovered.map(|_| Target {
                panel: index,
                owner: Owner::Element(element_index),
            })
        } else {
            outer(routing.hovered)
        };

        self.pressed = restore(routing.pressed, self.pressed);

        self.capture = restore(routing.capture, self.capture);

        for e in &mut events {
            e.owner = outer(e.owner.map(|owner| Target {
                panel: index,
                owner,
            }))
            .map(|t| t.owner);
        }

        for (e, (x, y)) in local_panel.elements.iter_mut().zip(offsets) {
            e.x = x;

            e.y = y;
        }

        let element_rect = panel.elements[element_index].rect();

        let state = panel.elements[element_index].widget.list_mut()?;

        state.items[row].children = local_panel.elements;

        if let Some(custom) = &mut state.custom {
            custom.hovered = Some(hit);

            if let Some(target) = routing.focused {
                let child = match target.owner {
                    Owner::Element(i)
                    | Owner::ListRow { element: i, .. }
                    | Owner::Descendant { element: i, .. }
                    | Owner::ListChildRow { element: i, .. } => Some(i),
                    _ => None,
                };

                if let Some(child) = child {
                    custom.focused = Some((row, child));
                }
            }
        }

        if matches!(event, Mouse::LeftDown | Mouse::LeftDouble)
            && (focus_allowed || routing.focused.is_some())
        {
            panel.focus = Some(element_index);
        }

        if state.custom.is_none() && callbacks.iter().any(|&v| v) {
            state.select(row as i32, element_rect);

            let selected = state.selection_notification_index();

            let name = panel.elements[element_index].name.clone();

            events = events
                .into_iter()
                .zip(callbacks)
                .flat_map(|(event, callback)| {
                    callback
                        .then(|| Event {
                            panel: index,
                            owner: Some(Owner::Element(element_index)),
                            name: name.clone(),
                            extra: i64::from(selected),
                            id: EventId::ListSelect,
                        })
                        .into_iter()
                        .chain([event])
                })
                .collect();
        }

        Some(events)
    }

    fn deliver(
        &mut self,
        index: usize,
        panel: &mut Panel,
        owner: Option<Owner>,
        event: Mouse,
        point: (i32, i32),
        admission: MouseAdmission,
    ) -> Vec<Event> {
        let (lx, ly) = point;

        let MouseAdmission {
            parent_callback,
            focus_allowed,
        } = admission;

        if owner == Some(Owner::Close) {
            let Some(close) = panel
                .close_button
                .as_ref()
                .filter(|e| interactive(e) && e.msg_receive)
            else {
                return Vec::new();
            };

            let target = Some(Target {
                panel: index,
                owner: Owner::Close,
            });

            let hit = in_rect(close.rect(), lx, ly);

            match event {
                Mouse::LeftDown | Mouse::LeftDouble if hit => {
                    self.pressed = target;

                    self.capture = target;

                    self.hovered = target;
                }
                Mouse::LeftUp if self.pressed == target => {
                    self.pressed = None;

                    self.capture = None;

                    if hit {
                        if close.element_type == ElementType::Button {
                            self.button_sounds.push(53);
                        }

                        return vec![Event {
                            panel: index,
                            owner,
                            name: close.name.clone(),
                            id: EventId::Click,
                            extra: 0,
                        }];
                    }
                }
                _ => {}
            }

            return Vec::new();
        }

        if let Some(events) =
            self.deliver_container_child(index, panel, owner, event, point, focus_allowed)
        {
            return events;
        }

        let name = match owner {
            Some(Owner::Element(i)) => panel
                .elements
                .get(i)
                .map(|e| e.name.clone())
                .unwrap_or_default(),
            _ => String::new(),
        };

        let raise = |id: EventId, extra: i64| Event {
            panel: index,
            id,
            owner,
            name: name.clone(),
            extra,
        };

        let target = owner.map(|owner| Target {
            panel: index,
            owner,
        });

        let mut events = Vec::new();

        if let Some(Owner::Element(i)) = owner
            && let Some(element) = panel.elements.get_mut(i)
            && interactive(element)
            && let Some(layout) = crate::scrollbar::layout(element)
        {
            let (bar, position) = crate::scrollbar::state_mut(element).unwrap();

            bar.pointer = [self.cursor.0, self.cursor.1];

            let up = in_rect(layout.rects.up, lx, ly);

            let down = in_rect(layout.rects.down, lx, ly);

            if event == Mouse::Move {
                bar.hover = [up, down];
            }

            if matches!(event, Mouse::LeftDown | Mouse::LeftDouble) && (up || down) {
                *position = layout.clamp(layout.position.saturating_add(if up { -1 } else { 1 }));

                bar.press = if up { 1 } else { 2 };

                self.focused = target;

                panel.focus = Some(i);

                self.hovered = target;

                return events;
            }

            if bar.press != 0 && matches!(event, Mouse::Move | Mouse::LeftUp) {
                if event == Mouse::LeftUp {
                    bar.press = 0;
                }

                return events;
            }
        }

        if let Some(Owner::Element(i)) = owner
            && let Some(element) = panel.elements.get_mut(i)
            && interactive(element)
            && element.widget.edit().is_some_and(|edit| !edit.note_pad)
        {
            let hit = in_rect(element.rect(), lx, ly);

            let edit = element.widget.edit_mut().unwrap();

            if edit.selection_held && self.capture == target {
                if event == Mouse::LeftUp {
                    edit.selection_held = false;
                }

                self.hovered = target;

                return events;
            }

            if hit && matches!(event, Mouse::LeftDown | Mouse::LeftDouble) {
                edit.selection_held = true;

                self.capture = target;

                self.pressed = target;

                self.hovered = target;

                if focus_allowed {
                    self.focused = target;

                    panel.focus = Some(i);
                }

                return events;
            }
        }

        if let Some(Owner::Element(i)) = owner
            && let Some(element) = panel.elements.get_mut(i)
            && element.element_type == ElementType::ImeNotePad
            && interactive(element)
        {
            let [l, t, r, b] = element.rect();

            let edit = element.widget.edit_mut().unwrap();

            let border = edit.border_width;

            let inset = border.saturating_add(edit.spacing);

            let pad = edit.pad.as_mut().unwrap();

            if pad.selection_held && self.capture == target {
                self.hovered = target;

                if event == Mouse::LeftUp {
                    pad.selection_held = false;
                }

                return events;
            }

            if let Some([count, page]) = pad.scroll_metrics {
                let width = pad.scrollbar_width.max(0);

                let left = if edit.left_scrollbar {
                    l + border
                } else {
                    r - border - width
                };

                let rect = [left, t + border, left + width, b - border];

                let max = count.saturating_sub(page).max(0);

                let scroll = pad.scroll.clamp(0, max);

                let bar = pad.scrollbar.rects(rect, [0, count], page, scroll);

                let dragging = pad.scrollbar.drag_offset.is_some() && self.capture == target;

                if in_rect(rect, lx, ly) || dragging {
                    self.hovered = target;

                    pad.scroll = scroll;

                    match event {
                        Mouse::Move => {
                            if let Some(grab) = pad.scrollbar.drag_offset
                                && let Some(position) =
                                    bar.drag_position([0, count], page, grab, ly)
                            {
                                pad.scroll = position;
                            }
                        }
                        Mouse::LeftDown | Mouse::LeftDouble => {
                            self.focused = target;

                            panel.focus = Some(i);

                            if let Some(thumb) = bar.thumb {
                                if in_rect(thumb, lx, ly) {
                                    pad.scrollbar.drag_offset = Some(ly.saturating_sub(thumb[1]));

                                    self.capture = target;

                                    self.pressed = target;
                                } else if let Some(delta) = bar.page_delta(page, ly) {
                                    pad.scroll = scroll.saturating_add(delta).clamp(0, max);
                                }
                            }
                        }
                        Mouse::LeftUp => {
                            pad.scrollbar.drag_offset = None;

                            pad.scrollbar.press = 0;

                            if self.capture == target {
                                self.capture = None;

                                self.pressed = None;
                            }
                        }
                        _ => {}
                    }

                    return events;
                }
            }

            let width = pad.scrollbar_width.max(0);

            let content = [
                l + inset + if edit.left_scrollbar { width } else { 0 },
                t + inset,
                r - inset - if edit.left_scrollbar { 0 } else { width },
                b - inset,
            ];

            if matches!(event, Mouse::LeftDown | Mouse::LeftDouble) && in_rect(content, lx, ly) {
                pad.selection_held = true;

                self.capture = target;

                self.pressed = target;

                self.focused = target;

                panel.focus = Some(i);

                self.hovered = target;

                return events;
            }
        }

        if let Some(Owner::Element(i)) = owner
            && let Some(element) = panel.elements.get_mut(i)
            && matches!(
                element.element_type,
                ElementType::ListBox | ElementType::ListView
            )
        {
            let rect = element.rect();

            let admitted = interactive(element);

            let state = element.widget.list_mut().unwrap();

            let content = state.content_rect(rect);

            let page = state.rows_per_page(content);

            let (min, max) = state.scroll_range(content);

            let bar = state.scrollbar_rects(rect);

            let on_bar = in_rect(state.scrollbar_rect(rect), lx, ly);

            let dragging = state.scrollbar.drag_offset.is_some() && self.capture == target;

            let available = admitted && (!state.plain || state.text_row_height.is_some());

            if available
                && (on_bar || dragging)
                && !(state.selection_held
                    && (event == Mouse::LeftUp || state.plain && event == Mouse::RightUp))
            {
                self.hovered = target;

                state.scroll = state.clamped_scroll(content);

                match event {
                    Mouse::Move => {
                        state.scrollbar.hover =
                            [in_rect(bar.up, lx, ly), in_rect(bar.down, lx, ly)];

                        if let Some(grab) = state.scrollbar.drag_offset
                            && let Some(position) = bar.drag_position([min, max], page, grab, ly)
                        {
                            state.scroll = position;
                        }
                    }
                    Mouse::LeftDown | Mouse::LeftDouble => {
                        self.focused = target;

                        panel.focus = Some(i);

                        if let Some(thumb) = bar.thumb {
                            if in_rect(thumb, lx, ly) {
                                state.scrollbar.drag_offset = Some(ly.saturating_sub(thumb[1]));

                                self.capture = target;

                                self.pressed = target;
                            } else {
                                let delta = if in_rect(bar.up, lx, ly) {
                                    Some(-1)
                                } else if in_rect(bar.down, lx, ly) {
                                    Some(1)
                                } else {
                                    bar.page_delta(page, ly)
                                };

                                if let Some(delta) = delta {
                                    state.scroll = state.scroll.saturating_add(delta);

                                    state.scroll = state.clamped_scroll(content);
                                }
                            }
                        }
                    }
                    Mouse::LeftUp => {
                        state.scrollbar.drag_offset = None;

                        state.scrollbar.press = 0;

                        if self.capture == target {
                            self.capture = None;

                            self.pressed = None;
                        }
                    }
                    _ => {}
                }

                return events;
            }
        }

        if let Some(Owner::Element(i)) = owner
            && let Some(element) = panel.elements.get_mut(i)
            && matches!(
                element.element_type,
                ElementType::TreeView | ElementType::CustomListView
            )
        {
            if !interactive(element) {
                return events;
            }

            let rect = element.rect();

            let Some(state) = element.widget.list_mut() else {
                return events;
            };

            let (rows, bar, scroll, max, page) = if let Some(tree) = &state.tree {
                let layout = tree.layout(state, rect);

                let max = layout.range[1].saturating_sub(layout.page as i32).max(0);

                (
                    layout.rows,
                    layout.bar,
                    layout.first as i32,
                    max,
                    layout.page as i32,
                )
            } else {
                let layout = crate::custom_list::layout(state, rect);

                (
                    layout.rows,
                    layout.bar,
                    layout.scroll,
                    layout.max_scroll,
                    layout.page,
                )
            };

            let row = rows
                .iter()
                .find(|(_, r)| in_rect(*r, lx, ly))
                .map(|(i, _)| *i);

            let mut drag = state
                .tree
                .as_ref()
                .and_then(|t| t.drag_offset)
                .or_else(|| state.custom.as_ref().and_then(|c| c.drag_offset));

            if let Some(tree) = &mut state.tree {
                tree.hovered = row;
            }

            self.hovered = target;

            state.scroll = scroll;

            match event {
                Mouse::Move => {
                    state.scrollbar.hover = [in_rect(bar.up, lx, ly), in_rect(bar.down, lx, ly)];

                    if let Some(offset) = drag
                        && let Some(position) =
                            bar.drag_position([0, max.saturating_add(page)], page, offset, ly)
                    {
                        state.scroll = position;
                    }
                }
                Mouse::LeftDown | Mouse::LeftDouble => {
                    if event == Mouse::LeftDown || (state.custom.is_some() && bar.contains(lx, ly))
                    {
                        self.focused = target;

                        panel.focus = Some(i);
                    }

                    if bar.contains(lx, ly) {
                        if let Some(thumb) = bar.thumb {
                            if in_rect(thumb, lx, ly) {
                                drag = Some(ly - thumb[1]);

                                self.capture = target;

                                self.pressed = target;
                            } else {
                                let step = if in_rect(bar.up, lx, ly) {
                                    -1
                                } else if in_rect(bar.down, lx, ly) {
                                    1
                                } else {
                                    bar.page_delta(page, ly).unwrap_or(0)
                                };

                                state.scroll = state.scroll.saturating_add(step).clamp(0, max);
                            }
                        }
                    } else if let Some(row) = state.select_tree_mouse(rect, lx, ly) {
                        let tree = state.tree.as_mut().unwrap();

                        if tree.toggle(row) {
                            let end = i32::try_from(tree.visible().len()).unwrap_or(i32::MAX);

                            state.scroll = state.scroll.clamp(0, end.saturating_sub(page).max(0));
                        }

                        self.capture = target;

                        self.pressed = target;

                        events.push(raise(
                            EventId::ListSelect,
                            i64::from(state.selection_notification_index()),
                        ));
                    }
                }
                Mouse::RightUp => {
                    if state.select_tree_mouse(rect, lx, ly).is_some() {
                        events.push(raise(EventId::TreeRightSelect, 0));
                    }
                }
                Mouse::LeftUp => {
                    drag = None;

                    self.capture = None;

                    self.pressed = None;

                    state.scrollbar.press = 0;
                }
                _ => {}
            }

            if let Some(tree) = &mut state.tree {
                tree.drag_offset = drag;
            }

            if let Some(custom) = &mut state.custom {
                custom.drag_offset = drag;
            }

            return events;
        }

        if let Some(Owner::Element(i)) = owner
            && let Some(element) = panel.elements.get_mut(i)
            && element.widget.combo_box().is_some()
        {
            if !interactive(element) {
                return events;
            }

            let rect = element.rect();

            let layout = element
                .widget
                .combo_box()
                .and_then(|c| c.stored_layout(rect, &element.layers));

            let combo = element.widget.combo_box_mut().unwrap();

            let row = layout.as_ref().and_then(|l| {
                l.rows
                    .iter()
                    .find(|(_, r)| in_rect(*r, lx, ly))
                    .map(|(i, _)| *i)
            });

            match event {
                Mouse::Move => {
                    self.hovered = target.filter(|_| in_rect(rect, lx, ly));

                    if combo.open {
                        if let Some(row) = row {
                            combo.hovered = row as i32;
                        }

                        if let Some(offset) = combo.drag_offset
                            && let Some(layout) = &layout
                            && let Some(position) = layout.bar.drag_position(
                                [0, combo.items.len() as i32],
                                layout.page as i32,
                                offset,
                                ly,
                            )
                        {
                            combo.scroll = position;
                        }
                    }
                }
                Mouse::LeftDown | Mouse::LeftDouble => {
                    let admitted = self.focused == target || focus_allowed;

                    if admitted {
                        self.focused = target;

                        panel.focus = Some(i);
                    }

                    self.hovered = target.filter(|_| in_rect(rect, lx, ly));

                    if combo.open
                        && let Some(layout) = &layout
                        && layout.page < combo.items.len()
                        && layout.bar.contains(lx, ly)
                    {
                        if let Some(thumb) = layout.bar.thumb {
                            if in_rect(thumb, lx, ly) {
                                combo.drag_offset = Some(ly - thumb[1]);

                                self.capture = target;

                                self.pressed = target;
                            } else {
                                let step =
                                    layout.bar.page_delta(layout.page as i32, ly).unwrap_or(0);

                                combo.scroll = (layout.first as i32)
                                    .saturating_add(step)
                                    .clamp(0, combo.items.len().saturating_sub(layout.page) as i32);
                            }
                        }
                    } else if combo.open
                        && let Some(row) = row
                    {
                        combo.select(row);

                        combo.open = false;

                        events.push(raise(EventId::ComboChanged, i64::from(combo.selected)));
                    } else if in_rect(rect, lx, ly) {
                        if admitted {
                            combo.open = !combo.open;
                        }

                        self.capture = target;

                        self.pressed = target;
                    }
                }
                Mouse::LeftUp => {
                    combo.drag_offset = None;

                    self.capture = None;

                    self.pressed = None;
                }
                _ => {}
            }

            return events;
        }

        if let Some(Owner::Element(i)) = owner
            && let Some(element) = panel.elements.get_mut(i)
            && element.widget.slider().is_some()
        {
            let rect = element.rect();

            let enabled = interactive(element);

            let slider = element.widget.slider_mut().unwrap();

            if !enabled || slider.mouse_move_lock {
                if event == Mouse::LeftUp {
                    slider.drag_offset = None;

                    self.capture = None;

                    self.pressed = None;
                }

                return events;
            }

            match event {
                Mouse::Move => {
                    self.hovered = target;

                    if let Some(offset) = slider.drag_offset {
                        let value = slider.value_at(rect, lx.saturating_add(offset));

                        if slider.set_value(value) {
                            events.push(raise(EventId::SliderChanged, i64::from(slider.value)));
                        }
                    }
                }
                Mouse::LeftDown | Mouse::LeftDouble => {
                    self.hovered = target;

                    self.focused = target;

                    panel.focus = Some(i);

                    let center = rect[0].saturating_add(slider.offset(rect));

                    if in_rect(slider.thumb(rect), lx, ly) {
                        slider.drag_offset = Some(center.saturating_sub(lx));

                        self.capture = target;

                        self.pressed = target;
                    } else if in_rect(rect, lx, ly) {
                        slider.drag_offset = Some(rect[0]);

                        let step = if lx > center {
                            1
                        } else if lx < center {
                            -1
                        } else {
                            0
                        };

                        if slider.set_value(slider.value.saturating_add(step)) {
                            events.push(raise(EventId::SliderChanged, i64::from(slider.value)));
                        }
                    }
                }
                Mouse::LeftUp if slider.drag_offset.take().is_some() => {
                    self.capture = None;

                    self.pressed = None;

                    events.push(raise(EventId::SliderChanged, i64::from(slider.value)));
                }
                _ => {}
            }

            return events;
        }

        match event {
            Mouse::Move => {
                self.hovered = target.filter(|_| {
                    !matches!(
                        owner, Some(Owner::Element(i)) if panel.elements.get(i)
                        .is_some_and(| e | capturing_left_button(e) &&!
                        element_contains(e, e.rect(), lx, ly))
                    )
                });

                events.push(raise(EventId::MouseMove, -1));
            }
            Mouse::LeftDown | Mouse::LeftDouble => {
                self.hovered = target;

                let double = event == Mouse::LeftDouble;

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(toggle_button)
                {
                    let state = panel.elements[i]
                        .widget
                        .button_mut()
                        .expect("a Button or IconButton carries button state");

                    state.checked = !state.checked;

                    if state.checked && panel.elements[i].element_type == ElementType::Button {
                        self.button_sounds.push(53);
                    }

                    events.push(raise(EventId::Click, 0));

                    return events;
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(ordinary_button)
                    && self.pressed == target
                {
                    self.focused = target;

                    return events;
                }

                if !double {
                    self.pressed = target;
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(capturing_left_button)
                {
                    self.pressed = target;

                    self.capture = target;

                    self.focused = target;
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(|e| {
                        can_focus(e)
                            || !double
                                && interactive(e)
                                && matches!(
                                    e.element_type,
                                    ElementType::ListBox | ElementType::ListView
                                )
                    })
                {
                    self.focused = target;
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(grouped_button)
                {
                    let group = panel.elements[i]
                        .widget
                        .button()
                        .expect("a grouped Button or IconButton carries button state")
                        .group
                        .clone();

                    clear_button_group(panel, panel.elements[i].instance, &group);

                    panel.elements[i]
                        .widget
                        .button_mut()
                        .expect("a grouped Button or IconButton carries button state")
                        .checked = true;

                    if panel.elements[i].element_type == ElementType::Button {
                        self.button_sounds.push(53);
                    }

                    if double {
                        events.push(raise(EventId::DoubleClick, 0));
                    } else {
                        events.push(raise(EventId::Click, 0));

                        events.push(raise(EventId::LeftDown, 0));
                    }

                    return events;
                }

                if double {
                    if let Some(Owner::Element(i)) = owner
                        && let Some(element) = panel.elements.get_mut(i)
                        && element
                            .widget
                            .list()
                            .and_then(|s| s.mouse_row_at(element.rect(), lx, ly))
                            .is_some()
                        && let Some(list) = element.widget.list_mut()
                        && list.custom.is_none()
                        && list.tree.is_none()
                    {
                        list.selection_held = true;

                        self.capture = target;

                        self.pressed = target;

                        events.push(raise(
                            EventId::ListDoubleSelect,
                            i64::from(list.selection_notification_index()),
                        ));

                        return events;
                    }

                    if let Some(Owner::Element(i)) = owner
                        && panel.elements.get(i).is_some_and(ordinary_button)
                    {
                        if panel.elements[i].element_type == ElementType::Button {
                            self.button_sounds.push(53);
                        }

                        events.push(raise(EventId::Click, 0));

                        events.push(raise(EventId::DoubleClick, 0));

                        self.pressed = None;

                        self.capture = None;
                    }

                    return events;
                }

                if owner.is_some()
                    && !matches!(
                        owner, Some(Owner::Element(i)) if panel.elements.get(i)
                        .is_some_and(checkable)
                    )
                {
                    let extra = match owner {
                        Some(Owner::Element(i))
                            if panel.elements.get(i).is_some_and(ordinary_button) =>
                        {
                            0
                        }
                        _ => -1,
                    };

                    events.push(raise(EventId::LeftDown, extra));
                }

                if let Some(Owner::Element(i)) = owner
                    && let Some(element) = panel.elements.get_mut(i)
                    && let Some(row) = element
                        .widget
                        .list()
                        .and_then(|s| s.mouse_row_at(element.rect(), lx, ly))
                    && let Some(list) = element.widget.list_mut()
                    && list.select_mouse(row, self.keys.shift, self.keys.ctrl)
                {
                    self.capture = target;

                    self.pressed = target;

                    events.push(raise(
                        EventId::ListSelect,
                        i64::from(list.selection_notification_index()),
                    ));
                }
            }
            Mouse::LeftUp => {
                self.hovered = target;

                if let Some(Owner::Element(i)) = owner
                    && let Some(list) = panel.elements.get_mut(i).and_then(|e| e.widget.list_mut())
                    && list.finish_mouse_selection()
                {
                    events.push(raise(
                        EventId::ListSelect,
                        i64::from(list.selection_notification_index()),
                    ));
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(toggle_button)
                {
                    return events;
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(grouped_button)
                {
                    return events;
                }

                if let (Some(pressed), Some(target)) = (self.pressed, target)
                    && pressed == target
                    && let Owner::Element(i) = target.owner
                    && panel.elements.get(i).is_some_and(checkable)
                {
                    if !element_contains(&panel.elements[i], panel.elements[i].rect(), lx, ly) {
                        return events;
                    }

                    let kind = panel.elements[i].element_type;

                    if kind == ElementType::RadioButton {
                        events.extend(
                            set_radio_checked(index, panel, target.owner, true, true, true)
                                .unwrap_or_default(),
                        );

                        return events;
                    }

                    let checked = !panel.elements[i]
                        .widget
                        .button()
                        .expect("checkbox state")
                        .checked;

                    events.extend(set_checkbox_checked(
                        index,
                        panel,
                        target.owner,
                        checked,
                        true,
                    ));

                    return events;
                }

                if let (Some(pressed), Some(target)) = (self.pressed, target)
                    && pressed == target
                    && let Owner::Element(i) = target.owner
                    && panel.elements.get(i).is_some_and(ordinary_button)
                {
                    if parent_callback || in_rect(panel.elements[i].rect(), lx, ly) {
                        if panel.elements[i].element_type == ElementType::Button {
                            self.button_sounds.push(53);
                        }

                        events.push(raise(EventId::Click, 0));

                        if !parent_callback {
                            events.push(raise(EventId::LeftUp, 0));
                        }
                    }

                    return events;
                }

                if owner.is_some() {
                    events.push(raise(EventId::LeftUp, -1));
                }
            }
            Mouse::RightDown | Mouse::RightDouble => {
                self.hovered = target;

                let double = event == Mouse::RightDouble;

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(toggle_button)
                {
                    if double {
                        return events;
                    }

                    let state = panel.elements[i]
                        .widget
                        .button_mut()
                        .expect("a Button or IconButton carries button state");

                    state.checked = !state.checked;

                    if state.checked && panel.elements[i].element_type == ElementType::Button {
                        self.button_sounds.push(53);
                    }

                    events.push(raise(EventId::Click, 0));

                    return events;
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(ordinary_button)
                {
                    self.pressed = target;

                    self.capture = target;

                    self.focused = target;

                    if !double {
                        events.push(raise(EventId::RightDown, 0));
                    }
                }
            }
            Mouse::RightUp => {
                self.hovered = target;

                if let Some(Owner::Element(i)) = owner
                    && let Some(element) = panel.elements.get_mut(i)
                {
                    let row = element
                        .widget
                        .list()
                        .and_then(|s| s.mouse_row_at(element.rect(), lx, ly));

                    if let Some(list) = element.widget.list_mut() {
                        if list.plain {
                            if list.finish_mouse_selection() {
                                events.push(raise(
                                    EventId::ListSelect,
                                    i64::from(list.selection_notification_index()),
                                ));
                            }

                            if self.capture == target {
                                self.capture = None;

                                self.pressed = None;
                            }
                        } else if list.tree.is_none()
                            && list.custom.is_none()
                            && let Some(row) = row
                        {
                            events.push(raise(EventId::TreeRightSelect, row as i64));
                        }
                    }
                }

                if let Some(Owner::Element(i)) = owner
                    && panel.elements.get(i).is_some_and(toggle_button)
                {
                    return events;
                }

                if let Some(target) = target
                    && let Owner::Element(i) = target.owner
                    && panel
                        .elements
                        .get(i)
                        .is_some_and(|element| ordinary_button(element) || grouped_button(element))
                    && (self.pressed == Some(target)
                        || panel.elements[i]
                            .widget
                            .button()
                            .is_some_and(|state| !state.group.is_empty() && state.checked))
                {
                    if parent_callback || in_rect(panel.elements[i].rect(), lx, ly) {
                        events.push(raise(EventId::RightUp, 0));

                        if !parent_callback {
                            events.push(raise(EventId::RightReleased, 0));
                        }
                    }

                    return events;
                }
            }
        }

        events
    }

    pub fn interaction(&self, panel: usize) -> crate::draw::Interaction {
        let mine = |t: Option<Target>| t.filter(|t| t.panel == panel).map(|t| t.owner);

        crate::draw::Interaction {
            hovered: mine(self.hovered),
            pressed: mine(self.pressed),
            press_lifts: false,
            focused: Some(mine(self.focused).unwrap_or(Owner::Background)),
            now_ms: None,
        }
    }

    pub fn interaction_for(
        &self,
        index: usize,
        panel: &Panel,
        origin: (i32, i32),
    ) -> crate::draw::Interaction {
        let mut interaction = self.interaction(index);

        let retained = |owner: Option<Owner>| {
            owner.and_then(|owner| {
                self.retained_target(
                    Target {
                        panel: index,
                        owner,
                    },
                    panel,
                )
                .map(|t| t.owner)
            })
        };

        interaction.pressed = retained(interaction.pressed);

        interaction.hovered = retained(interaction.hovered);

        interaction.focused = Some(retained(interaction.focused).unwrap_or(Owner::Background));

        if self.carried.is_none()
            && let Some(owner) = interaction.hovered
        {
            interaction.hovered = Some(refine_hover(
                panel,
                owner,
                self.cursor.0.saturating_sub(origin.0),
                self.cursor.1.saturating_sub(origin.1),
            ));
        }

        interaction
    }
}

fn clear_button_group(panel: &mut Panel, instance: crate::element::Instance, group: &str) {
    let clear = |element: &mut Element| {
        if element.instance != instance
            && matches!(
                element.element_type,
                ElementType::Button | ElementType::IconButton
            )
            && let Some(button) = element.widget.button_mut()
            && !button.group.is_empty()
            && button.group == group
        {
            button.checked = false;
        }
    };

    for element in &mut panel.elements {
        if matches!(
            element.element_type,
            ElementType::ListView | ElementType::CustomListView
        ) {
            if let Some(list) = element.widget.list_mut() {
                for row in &mut list.items {
                    for child in &mut row.children {
                        clear(child);
                    }
                }
            }
        } else {
            clear(element);
        }
    }
}

fn button_leave_cancels_press(element: &Element) -> bool {
    matches!(
        element.element_type,
        ElementType::Button
            | ElementType::IconButton
            | ElementType::CheckBox
            | ElementType::RadioButton
            | ElementType::ComboBox
    ) && element
        .widget
        .button()
        .or_else(|| element.widget.combo_box().map(|c| &c.button))
        .is_some_and(|b| !b.toggle && b.group.is_empty())
        && element
            .widget
            .combo_box()
            .is_none_or(|c| c.drag_offset.is_none())
}

fn toggle_button(element: &Element) -> bool {
    matches!(
        element.element_type,
        ElementType::Button | ElementType::IconButton
    ) && element.widget.button().is_some_and(|state| state.toggle)
}

fn ordinary_button(element: &Element) -> bool {
    matches!(
        element.element_type,
        ElementType::Button | ElementType::IconButton
    ) && element
        .widget
        .button()
        .is_some_and(|state| !state.toggle && state.group.is_empty())
}

fn grouped_button(element: &Element) -> bool {
    matches!(
        element.element_type,
        ElementType::Button | ElementType::IconButton
    ) && element
        .widget
        .button()
        .is_some_and(|state| !state.toggle && !state.group.is_empty())
}

fn capturing_left_button(element: &Element) -> bool {
    ordinary_button(element) || grouped_button(element) || checkable(element)
}

fn checkable(element: &Element) -> bool {
    matches!(
        element.element_type,
        ElementType::CheckBox | ElementType::RadioButton
    )
}

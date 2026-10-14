use super::{
    Scene,
    login_panels::{LoginPanels, PlacedPanel},
    panels,
};
use ui::{EditState, draw::Owner, widget::ImeStatus};

struct Target<'a> {
    edit: &'a mut EditState,
    area: [f64; 4],
}

impl Target<'_> {
    fn identity(&self) -> usize {
        self.edit as *const EditState as usize
    }
}

fn element(placed: &mut PlacedPanel, owner: Owner, scale: f32) -> Option<Target<'_>> {
    let (e, rect) = ui::input::owner_element_mut(&mut placed.ui.panel, owner)?;

    let area = [
        rect[0] + placed.origin.0,
        rect[1] + placed.origin.1,
        e.width,
        e.height,
    ]
    .map(|v| v as f64 * scale as f64);

    let edit = e.widget.edit_mut()?;

    if edit.pad.as_ref().is_some_and(|p| p.read_only) {
        return None;
    }

    Some(Target { edit, area })
}

fn named<'a>(placed: &'a mut PlacedPanel, name: &str, scale: f32) -> Option<Target<'a>> {
    let index = placed
        .ui
        .panel
        .elements
        .iter()
        .position(|e| e.name == name && e.widget.edit().is_some())?;

    element(placed, Owner::Element(index), scale)
}

fn login(scene: &mut LoginPanels) -> Option<Target<'_>> {
    if scene.msgbox.placed.ui.panel.visible {
        scene.msgbox.edit_mut()?;

        return named(
            &mut scene.msgbox.placed,
            super::msgbox::ELEMENT_EDIT,
            scene.layout.ui_scale(),
        );
    }

    let focus = scene.manager.focused?;

    let scale = scene.layout.ui_scale();

    let placed = match focus.panel {
        0 => &mut scene.login,
        1 => &mut scene.login2,
        2 => &mut scene.copyright,
        _ => return None,
    };

    scene.manager.reconcile_panel(focus.panel, &placed.ui.panel);

    let focus = scene.manager.focused?;

    element(placed, focus.owner, scale)
}

fn target(scene: &mut Scene) -> Option<Target<'_>> {
    if let Scene::SelectChar(s) = scene {
        s.sync_name_focus();
    }

    let legacy_edit = matches!(scene, Scene::SelectChar(s) if s.name_dialog.has_focus());

    if scene.legacy_modal()
        && !matches!(scene, Scene::Field(_))
        && !legacy_edit
        && !scene.server_modern_keyboard()
    {
        return None;
    }

    match scene {
        Scene::LoginStatic(_) => None,
        Scene::LoginPanels(scene) => login(scene),
        Scene::ServerSelect(scene) => login(&mut scene.panels),
        Scene::SelectChar(scene) => {
            if scene.name_dialog.visible {
                if !scene.name_dialog.has_focus() {
                    return None;
                }

                let area = scene.name_dialog.ime_area(&scene.layout);

                return Some(Target {
                    edit: &mut scene.name_dialog.edit,
                    area,
                });
            }

            let p = &mut scene.panels;

            if p.msgbox.placed.ui.panel.visible {
                p.msgbox.edit_mut()?;

                return named(
                    &mut p.msgbox.placed,
                    super::msgbox::ELEMENT_EDIT,
                    scene.layout.ui_scale(),
                );
            }

            let focus = p.manager.focused?;

            let pane = p.layers().get(focus.panel).copied().flatten()?;

            p.manager
                .reconcile_panel(focus.panel, &p.panes[pane].ui.panel);

            let focus = p.manager.focused?;

            element(&mut p.panes[pane], focus.owner, scene.layout.ui_scale())
        }
        Scene::Field(scene) => {
            if super::field::air_move_active(scene) {
                return None;
            }

            let p = scene.panels.as_mut()?;

            if let Some(hud) = scene.hud.as_mut() {
                panels::sync_coin_focus(hud, p);
            }

            let scale = p.layout.ui_scale();

            if p.msgbox.placed.ui.panel.visible {
                return named(&mut p.msgbox.placed, super::msgbox::ELEMENT_EDIT, scale);
            }

            if p.is_open(panels::CONTENT_COIN_INPUT) {
                let focus = scene.hud.as_ref()?.manager.focused?;

                let index = p
                    .screens
                    .iter()
                    .position(|s| s.content == panels::CONTENT_COIN_INPUT)?;

                if focus.panel != panels::input_slot(index) {
                    return None;
                }

                let placed = &mut p.screens[index].placed;

                let editor = placed
                    .ui
                    .panel
                    .elements
                    .iter()
                    .position(|e| e.name == "edit" && e.widget.edit().is_some())?;

                if focus.owner != Owner::Element(editor) {
                    return None;
                }

                return element(placed, focus.owner, scale);
            }

            let hud = scene.hud.as_mut()?;

            let focus = hud.manager.focused?;

            let placed = match focus.panel {
                0 => &mut hud.bar,
                1 => &mut hud.plate,
                2 => &mut hud.target,
                index => {
                    let screen = p.screens.get_mut(index.checked_sub(panels::HUD_PANELS)?)?;

                    if !screen.open {
                        return None;
                    }

                    &mut screen.placed
                }
            };

            hud.manager.reconcile_panel(focus.panel, &placed.ui.panel);

            let focus = hud.manager.focused?;

            element(placed, focus.owner, scale)
        }
    }
}

fn clear_retained_preedit(scene: &mut Scene, owner: Option<usize>) {
    let Some(owner) = owner else {
        return;
    };

    fn clear(edit: &mut EditState, owner: usize) {
        if edit as *const EditState as usize == owner {
            edit.preedit = None;

            edit.ime_status = ImeStatus::Disabled;
        }
    }

    fn visit(element: &mut ui::Element, owner: usize) {
        if let Some(edit) = element.widget.edit_mut() {
            clear(edit, owner);
        }

        if let Some(list) = element.widget.list_mut() {
            for row in &mut list.items {
                for child in &mut row.children {
                    visit(child, owner);
                }
            }
        }
    }

    scene.for_each_ui(|ui, _| {
        for element in &mut ui.panel.elements {
            visit(element, owner);
        }
    });

    if let Scene::SelectChar(scene) = scene {
        clear(&mut scene.name_dialog.edit, owner);
    }
}

#[derive(Debug)]
pub struct Bridge {
    owner: Option<usize>,
    enabled: bool,
    baseline: Option<(String, i32, i32)>,
    pub focused: bool,
    pub composing: bool,
}

impl Default for Bridge {
    fn default() -> Self {
        Self {
            owner: None,
            enabled: false,
            baseline: None,
            focused: true,
            composing: false,
        }
    }
}

impl Bridge {
    pub fn synchronize(&mut self, scene: &mut Scene) -> Option<bool> {
        let next = if self.focused {
            target(scene).map(|t| t.identity())
        } else {
            None
        };

        let replaced = self.baseline.as_ref().is_some_and(|(text, caret, anchor)| {
            target(scene).is_none_or(|t| {
                t.edit.text != *text || t.edit.caret != *caret || t.edit.anchor != *anchor
            })
        });

        if next == self.owner && !replaced {
            return None;
        }

        clear_retained_preedit(scene, self.owner);

        if let Some(t) = target(scene) {
            t.edit.preedit = None;

            t.edit.ime_status = ImeStatus::Disabled;
        }

        self.owner = next;

        self.enabled = false;

        self.composing = false;

        self.baseline = None;

        Some(next.is_some())
    }

    pub fn cancel(&mut self, scene: &mut Scene) {
        clear_retained_preedit(scene, self.owner);

        if let Some(t) = target(scene) {
            t.edit.preedit = None;

            t.edit.ime_status = ImeStatus::Disabled;
        }

        self.owner = None;

        self.enabled = false;

        self.composing = false;

        self.baseline = None;
    }

    pub fn area(&mut self, scene: &mut Scene) -> Option<[f64; 4]> {
        if !self.enabled {
            return None;
        }

        let t = target(scene)?;

        (Some(t.identity()) == self.owner).then_some(t.area)
    }

    pub fn event(&mut self, scene: &mut Scene, event: winit::event::Ime) {
        use winit::event::Ime;

        match event {
            Ime::Enabled => {
                self.enabled = false;

                if self.focused
                    && let Some(t) = target(scene)
                    && Some(t.identity()) == self.owner
                {
                    self.enabled = true;

                    t.edit.ime_status = ImeStatus::Ready;
                }
            }
            Ime::Disabled => {
                self.enabled = false;

                self.composing = false;

                self.baseline = None;

                clear_retained_preedit(scene, self.owner);

                if let Some(t) = target(scene) {
                    t.edit.preedit = None;

                    t.edit.ime_status = ImeStatus::Disabled;
                }
            }
            Ime::Preedit(text, cursor) => {
                if !self.enabled {
                    return;
                }

                let Some(t) = target(scene) else {
                    return;
                };

                if Some(t.identity()) != self.owner {
                    return;
                }

                if !text.is_empty() && self.baseline.is_none() {
                    self.baseline = Some((t.edit.text.clone(), t.edit.caret, t.edit.anchor));
                }

                ui::text_entry::preedit(t.edit, &text, cursor);

                self.composing = !text.is_empty();

                t.edit.ime_status = if self.composing {
                    ImeStatus::Composing
                } else {
                    ImeStatus::Ready
                };
            }
            Ime::Commit(text) => {
                if !self.enabled {
                    return;
                }

                let Some(t) = target(scene) else {
                    return;
                };

                if Some(t.identity()) != self.owner {
                    return;
                }

                t.edit.preedit = None;

                t.edit.ime_status = ImeStatus::Ready;

                self.composing = false;

                self.baseline = None;

                for ch in text.chars().filter(|ch| !ch.is_control()) {
                    scene.character(ch);
                }
            }
        }
    }
}

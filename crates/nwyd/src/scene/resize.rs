use super::{Scene, field, login_static::Layout};

impl Scene {
    pub(crate) fn resize(&mut self, chains: &crate::fonts::Chains, size: (u32, u32)) {
        match self {
            Scene::LoginStatic(scene) => scene.resize(size),
            Scene::LoginPanels(scene) => scene.resize(chains, size),
            Scene::ServerSelect(scene) => {
                scene.panels.resize(chains, size);

                scene.layout = Layout::new(size.0, size.1);
            }
            Scene::SelectChar(scene) => scene.resize(chains, size),
            Scene::Field(scene) => scene.resize(chains, size),
        }

        self.cancel_pointer_input();
    }

    pub(crate) fn cancel_pointer_input(&mut self) {
        self.for_each_ui(|ui, _| ui::input::cancel_panel_pointer(&mut ui.panel));

        match self {
            Scene::LoginStatic(_) => {}
            Scene::LoginPanels(scene) => cancel_login_pointer(scene),
            Scene::ServerSelect(scene) => cancel_login_pointer(&mut scene.panels),
            Scene::SelectChar(scene) => {
                cancel_manager_pointer(&mut scene.panels.manager);

                scene.legacy_boxes.cancel_pointer_input();

                scene.name_dialog.cancel_pointer_input();

                scene.pointer(None);
            }
            Scene::Field(scene) => {
                field::cancel_pointer_hold(scene);

                if let Some(hud) = &mut scene.hud {
                    cancel_manager_pointer(&mut hud.manager);
                }

                scene.legacy_boxes.cancel_pointer_input();
            }
        }
    }
}

fn cancel_login_pointer(scene: &mut super::login_panels::LoginPanels) {
    cancel_manager_pointer(&mut scene.manager);

    cancel_manager_pointer(&mut scene.box_manager);

    scene.legacy_boxes.cancel_pointer_input();
}

fn cancel_manager_pointer(manager: &mut ui::input::Manager) {
    manager.pressed = None;

    manager.capture = None;

    manager.drag = None;

    manager.hovered = None;

    manager.cancel_carry();
}

#[cfg(test)]
mod tests {

    use super::*;
    use ui::draw::Owner;
    use ui::input::{DragMode, Manager, PanelDrag, Target};

    #[test]
    fn pointer_cancellation_preserves_keyboard_focus_and_modifier_state() {
        let target = Target {
            panel: 1,
            owner: Owner::Element(2),
        };

        let mut manager = Manager {
            focused: Some(target),
            pressed: Some(target),
            capture: Some(target),
            hovered: Some(target),
            drag: Some(PanelDrag {
                panel: 1,
                mode: DragMode::Move,
                grab: (10, 20),
                grabbed: (30, 40),
                moved: true,
            }),
            screen: [0, 0, 800, 600],
            keys: ui::text_entry::Mods {
                ctrl: true,
                shift: true,
                alt: false,
            },
            button_sounds: vec![1],
            ..Default::default()
        };

        let keys = manager.keys;

        cancel_manager_pointer(&mut manager);

        assert_eq!(manager.focused, Some(target));

        assert_eq!(manager.keys, keys);

        assert_eq!(manager.screen, [0, 0, 800, 600]);

        assert_eq!(manager.button_sounds, [1]);

        assert_eq!(manager.pressed, None);

        assert_eq!(manager.capture, None);

        assert_eq!(manager.drag, None);

        assert_eq!(manager.hovered, None);
    }
}

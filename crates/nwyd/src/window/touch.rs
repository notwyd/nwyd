use super::Client;
use crate::scene::{Scene, field};
use winit::event::{ElementState, MouseButton, Touch, TouchPhase};

impl Client {
    pub(super) fn pointer_touch(&mut self, touch: Touch) {
        let identity = (touch.device_id, touch.id);

        match touch.phase {
            TouchPhase::Started if self.primary_touch.is_none() => {
                self.primary_touch = Some(identity);

                // Mouse releases are ignored while touch owns the pointer.
                self.runtime.input.mouse = crate::scene::camera::MouseInput {
                    alt: self.runtime.input.mods.alt,
                    ..Default::default()
                };

                if let Scene::Field(scene) = &mut self.runtime.scene {
                    field::cancel_pointer_hold(scene);
                }

                // Winit already supplies physical canvas coordinates, including device scale.
                self.pointer_moved((touch.location.x, touch.location.y));

                self.pointer_button(ElementState::Pressed, MouseButton::Left);
            }
            TouchPhase::Moved if self.primary_touch == Some(identity) => {
                self.pointer_moved((touch.location.x, touch.location.y));
            }
            TouchPhase::Ended if self.primary_touch == Some(identity) => {
                self.pointer_moved((touch.location.x, touch.location.y));

                if self.pointer_in_backbuffer().is_none() {
                    // Normal mouse routing skips releases outside the fitted game content.
                    self.cancel_touch();

                    return;
                }

                self.pointer_button(ElementState::Released, MouseButton::Left);

                self.primary_touch = None;

                self.pointer_left();
            }
            TouchPhase::Cancelled if self.primary_touch == Some(identity) => {
                self.cancel_touch();
            }
            _ => {}
        }
    }

    pub(super) fn cancel_touch(&mut self) {
        if self.primary_touch.take().is_none() {
            return;
        }

        // A cancellation must disarm controls without activating a pending click.
        self.runtime.scene.cancel_pointer_input();

        if let Some(panel) = &mut self.runtime.game_settings {
            panel.cancel_input();
        }

        self.pointer_left();
    }
}

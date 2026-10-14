use super::Scene;
use formats::net::item_prices::Update;

#[derive(Debug, Default, Clone, Copy)]
pub struct Retained {
    latest: Option<Update>,
}

impl Retained {
    pub fn receive(&mut self, scene: &mut Scene, update: Update) -> bool {
        if !matches!(scene, Scene::Field(_)) {
            return false;
        }

        self.latest = Some(update);

        self.apply(scene);

        true
    }

    pub fn apply(&self, scene: &mut Scene) {
        if let (Some(update), Scene::Field(field)) = (self.latest, scene) {
            update.apply(&mut field.statics.items);

            if let Some(panels) = field.panels.as_mut() {
                update.apply(&mut panels.items);
            }
        }
    }
}

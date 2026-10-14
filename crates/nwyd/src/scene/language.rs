use super::Scene;
use anyhow::Result;
use assets::GameDir;

pub(crate) enum Update {
    Static,
    Login(Box<super::login_panels::LanguageUpdate>),
    Character(Box<super::select_char::LanguageUpdate>),
    Field(Box<super::field::LanguageUpdate>),
}

impl Scene {
    pub(crate) fn prepare_language(&self, game: &GameDir, strings: &ui::Strings) -> Result<Update> {
        Ok(match self {
            Scene::LoginStatic(_) => Update::Static,
            Scene::LoginPanels(scene) => Update::Login(Box::new(
                super::login_panels::LanguageUpdate::prepare(scene, game, strings)?,
            )),
            Scene::ServerSelect(scene) => Update::Login(Box::new(
                super::login_panels::LanguageUpdate::prepare(&scene.panels, game, strings)?,
            )),
            Scene::SelectChar(scene) => Update::Character(Box::new(
                super::select_char::LanguageUpdate::prepare(scene, game, strings)?,
            )),
            Scene::Field(scene) => Update::Field(Box::new(super::field::LanguageUpdate::prepare(
                scene, game, strings,
            )?)),
        })
    }

    pub(crate) fn apply_language(&mut self, update: Update) {
        match (self, update) {
            (Scene::LoginStatic(_), Update::Static) => {}
            (Scene::LoginPanels(scene), Update::Login(update)) => scene.apply_language(*update),
            (Scene::ServerSelect(scene), Update::Login(update)) => {
                scene.panels.apply_language(*update)
            }
            (Scene::SelectChar(scene), Update::Character(update)) => scene.apply_language(*update),
            (Scene::Field(scene), Update::Field(update)) => scene.apply_language(*update),
            _ => unreachable!("scene changed during language preparation"),
        }
    }
}

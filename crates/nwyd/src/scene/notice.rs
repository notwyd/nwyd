use crate::frame::PanelId;
use crate::scene::panels::Panels;
use formats::ui::pane::ElementType;
use std::collections::VecDeque;

#[derive(Debug)]
pub struct Overlay {
    items: formats::data::item_list::ItemList,
    base_items: formats::data::item_list::ItemList,
    pub state: Notice,
    pub placed: super::login_panels::PlacedPanel,
    textures: std::collections::BTreeMap<i32, super::login_panels::GuiTexture>,
}

#[derive(Debug)]
pub struct LanguageUpdate {
    items: formats::data::item_list::ItemList,
}

impl LanguageUpdate {
    pub fn prepare(overlay: &Overlay, game: &assets::GameDir) -> anyhow::Result<Self> {
        Ok(Self {
            items: localized_items(&overlay.items, &overlay.base_items, game)?,
        })
    }
}

fn localized_items(
    current: &formats::data::item_list::ItemList,
    base: &formats::data::item_list::ItemList,
    game: &assets::GameDir,
) -> anyhow::Result<formats::data::item_list::ItemList> {
    let mut items = current.clone();

    items.reset_names_from(base);

    match crate::ui::read_language(game, super::panels::ITEM_NAME) {
        Ok((raw, codepage)) => {
            items.apply_names_with_codepage(&formats::data::item_name::parse(&raw), codepage);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    Ok(items)
}

impl Overlay {
    pub fn apply_language(&mut self, update: LanguageUpdate) {
        self.items = update.items;
    }

    pub fn relocalize(&mut self, game: &assets::GameDir, strings: &ui::Strings) {
        self.placed.ui.relocalize(game, strings);
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        use formats::data::item_list::{BODY_LEN, ItemList};

        Self {
            items: ItemList::from_plain(vec![0; BODY_LEN], Default::default()),
            base_items: ItemList::from_plain(vec![0; BODY_LEN], Default::default()),
            state: Default::default(),
            placed: super::login_panels::language_tests::placed(&[(MESSAGE, ElementType::Static)]),
            textures: Default::default(),
        }
    }

    pub(super) fn resize(&mut self, layout: &super::login_static::Layout) {
        self.placed.ui.scale = layout.ui_scale();

        self.placed.origin = self.placed.ui.reposition(layout.virtual_screen());
    }

    pub fn build(
        game: &assets::GameDir,
        backbuffer: (u32, u32),
        strings: &ui::Strings,
    ) -> anyhow::Result<Self> {
        let layout = super::login_static::Layout::new(backbuffer.0, backbuffer.1);

        let chains = crate::fonts::load(game, crate::fonts::system_font_dir().as_deref());

        let mut ui = crate::ui::PanelUi::build(game, SCRIPT, strings, &chains, layout.ui_scale())?;

        let origin = ui.origin(layout.virtual_screen());

        ui.panel.visible = true;

        let mut textures = std::collections::BTreeMap::new();

        for slot in ui.referenced_textures(&Default::default()) {
            if let Some(texture) = super::login_panels::load_guimat_texture(game, &ui, slot)? {
                textures.insert(slot, texture);
            }
        }

        ui.panel.visible = false;

        let base_items = super::item_table::load(game)?;

        let items = localized_items(&base_items, &base_items, game)?;

        Ok(Self {
            items,
            base_items,
            state: Notice::default(),
            placed: super::login_panels::PlacedPanel { ui, origin },
            textures,
        })
    }

    pub fn announce(
        &mut self,
        strings: &ui::Strings,
        message: &protocol::server::SystemMessage,
        now_ms: u32,
    ) -> super::chat::Announced {
        let announced = super::chat::prepare_announcement(strings, &self.items, message, false);

        for (text, life) in &announced.banners {
            self.post(text, *life, true, now_ms);
        }

        announced
    }

    fn show(&mut self, text: &str, sound: bool) {
        if let Some(element) = self
            .placed
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.name == MESSAGE && e.element_type == ElementType::Static)
        {
            element.set_display(text);
        }

        self.placed.ui.panel.visible = true;

        if sound {
            self.state.sounds.push(SOUND_SLOT);
        }
    }

    pub fn post(&mut self, text: &str, life_ms: u32, sound: bool, now_ms: u32) -> bool {
        if !self.state.post(text, life_ms, sound, now_ms) {
            return false;
        }

        self.show(text, sound);

        true
    }

    pub fn tick(&mut self, now_ms: u32) {
        if let Some(next) = self.state.tick(now_ms) {
            self.show(&next.text, next.sound);
        }

        self.placed.ui.panel.visible = self.state.showing;
    }

    pub fn upload(&self, gpu: &render::device::Gpu, renderer: &mut crate::frame::SceneRenderer) {
        for (slot, loaded) in &self.textures {
            renderer.upload_once(
                gpu,
                crate::frame::TextureKey::GuiMat(*slot),
                &loaded.texture,
                render::texture::AlphaPolicy::Stored,
                &loaded.path,
            );
        }
    }

    pub fn sync(&self, gpu: &render::device::Gpu, renderer: &mut crate::frame::SceneRenderer) {
        if self.state.showing {
            renderer.sync_named_icons(gpu, CONTENT, &self.placed.ui);

            renderer.sync_captions(gpu, CONTENT, &self.placed.ui);
        }
    }

    pub fn frame_draws(&mut self, captions: bool) -> Vec<crate::frame::PlannedQuad> {
        use crate::{
            frame::{PlannedQuad, TextureKey},
            ui::Source,
        };

        if !self.state.showing {
            return Vec::new();
        }

        self.placed
            .ui
            .frame(&Default::default(), self.placed.origin)
            .into_iter()
            .filter_map(|draw| {
                if !captions && matches!(draw.source, Source::Caption(..)) {
                    return None;
                }

                let texture = match draw.source {
                    Source::Texture(slot) => TextureKey::GuiMat(slot),
                    Source::Caption(owner, run) => TextureKey::Caption(CONTENT, owner, run),
                    Source::NamedIcon(owner) => TextureKey::NamedIcon(CONTENT, owner),
                    Source::ItemIcon(sheet) => TextureKey::ItemIcon(sheet),
                    Source::Amul(sheet) => TextureKey::Amul(sheet),
                    Source::Solid => TextureKey::White,
                };

                Some(PlannedQuad {
                    mode: draw.mode(),
                    texture,
                    quad: draw.quad,
                    uv: draw.uv,
                })
            })
            .collect()
    }
}

pub const CONTENT: PanelId = 0x06;

pub const SCRIPT: &[u8] = b"ui/Panel/notice.pane";

pub const MESSAGE: &str = "msg";

pub const LIFE_MS: u32 = 0xFA0;

pub const LIFE_LONG_MS: u32 = 0x927C0;

pub const QUEUED_LIFE_MS: u32 = 2_000;

pub const CUT_MS: u32 = 0x15E;

pub const SOUND_SLOT: i64 = 0x21;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub text: String,
    pub sound: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notice {
    pub showing: bool,
    pub deadline_ms: u32,
    pub queue: VecDeque<Queued>,
    pub sounds: Vec<i64>,
}

impl Notice {
    pub fn waiting(&self) -> bool {
        !self.queue.is_empty()
    }

    pub fn post(&mut self, text: &str, life_ms: u32, sound: bool, now_ms: u32) -> bool {
        if self.showing {
            self.queue.clear();

            self.queue.push_back(Queued {
                text: text.into(),
                sound,
            });

            return false;
        }

        self.showing = true;

        self.deadline_ms = now_ms.wrapping_add(life_ms);

        true
    }

    pub fn tick(&mut self, now_ms: u32) -> Option<Queued> {
        if self.showing {
            if self.deadline_ms < now_ms {
                self.deadline_ms = now_ms;

                self.showing = false;
            } else if self.waiting() && now_ms.wrapping_add(CUT_MS) < self.deadline_ms {
                self.deadline_ms = now_ms.wrapping_add(CUT_MS);
            }

            if self.showing {
                return None;
            }
        }

        let next = self.queue.pop_front()?;

        self.deadline_ms = now_ms.wrapping_add(QUEUED_LIFE_MS);

        self.showing = true;

        Some(next)
    }
}

pub fn post(panels: &mut Panels, text: &str, life_ms: u32, sound: bool, now_ms: u32) -> bool {
    if panels.screen(CONTENT).is_none() {
        return false;
    }

    if !panels.notice.post(text, life_ms, sound, now_ms) {
        return false;
    }

    show(panels, text, sound);

    true
}

pub fn tick(panels: &mut Panels, now_ms: u32) {
    if panels.screen(CONTENT).is_none() {
        return;
    }

    if let Some(next) = panels.notice.tick(now_ms) {
        show(panels, &next.text, next.sound);
    } else if !panels.notice.showing {
        panels.set_open(CONTENT, false);
    }
}

pub fn clear(panels: &mut Panels) {
    panels.set_open(CONTENT, false);

    panels.notice.deadline_ms = 0;

    panels.notice.showing = false;

    panels.notice.queue.clear();
}

fn show(panels: &mut Panels, text: &str, sound: bool) {
    if let Some(screen) = panels.screen_mut(CONTENT)
        && let Some(element) = screen.element_mut(MESSAGE, ElementType::Static)
    {
        element.set_display(text);
    }

    panels.set_open(CONTENT, true);

    if sound {
        panels.notice.sounds.push(SOUND_SLOT);
    }
}

pub fn text(panels: &Panels) -> Option<&str> {
    let screen = panels.screen(CONTENT)?;

    let element = screen.element(MESSAGE, ElementType::Static)?;

    Some(element.display.as_str())
}

#[cfg(test)]
mod language_tests;

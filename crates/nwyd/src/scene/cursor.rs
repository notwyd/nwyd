use super::login_static::{self, Layout, LoadedTexture, LoginPresentation};
use crate::frame::{PlannedQuad, SceneRenderer, TextureKey};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::config::CursorMode;
use formats::exe;
use render::device::Gpu;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default)]

pub enum Location {
    #[default]
    Initial,
    Inside,
    Outside,
}

impl Location {
    pub fn resolve(self, viewport: (u32, u32), mapped: Option<(i32, i32)>) -> Option<(i32, i32)> {
        match self {
            Self::Initial => Some((viewport.0 as i32 / 2, viewport.1 as i32 / 2)),
            Self::Inside => mapped,
            Self::Outside => None,
        }
    }
}

#[derive(Debug, Clone)]

pub struct EngineSprite {
    sets: formats::texture::texture_set_list::TextureSetList,
    textures: BTreeMap<u32, LoadedTexture>,
}

impl EngineSprite {
    pub fn load(game: &GameDir) -> Result<Self> {
        use formats::texture::{texture_list, texture_set_list};

        let sets = texture_set_list::parse(&game.read(login_static::TEXTURE_SET_LIST)?)?;

        let list = texture_list::parse(&game.read(login_static::TEXTURE_LIST)?)?;

        let count = sets
            .get(0)
            .context("missing Cursor atlas set0")?
            .items
            .len();

        anyhow::ensure!(count > 0, "empty Cursor atlas set0");

        let mut textures = BTreeMap::new();

        for frame in 0..count {
            let draw = login_static::cursor_draw(
                Layout::new(800, 600),
                &sets,
                &LoginPresentation {
                    cursor: CursorMode::Engine,
                    cursor_frame: frame,
                    ..LoginPresentation::default()
                },
            )?;

            if let std::collections::btree_map::Entry::Vacant(entry) = textures.entry(draw.texture)
            {
                entry.insert(
                    login_static::load_texture(game, &list, draw.texture)?.with_context(|| {
                        format!("missing engine cursor texture{}", draw.texture)
                    })?,
                );
            }
        }

        Ok(Self { sets, textures })
    }

    pub fn draw(
        &self,
        viewport: (u32, u32),
        frame: u32,
        position: (i32, i32),
    ) -> Option<PlannedQuad> {
        let draw = login_static::cursor_draw(
            Layout::new(viewport.0, viewport.1),
            &self.sets,
            &LoginPresentation {
                cursor: CursorMode::Engine,
                cursor_frame: frame as usize,
                mouse: Some((position.0 as f32, position.1 as f32)),
                ..LoginPresentation::default()
            },
        )
        .ok()?;

        Some(PlannedQuad {
            mode: login_static::MODE,
            texture: TextureKey::EngineCursor(draw.texture),
            quad: draw.quad,
            uv: render::sprite::UvRule::TexelCentre,
        })
    }

    fn upload(&self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        for (slot, loaded) in &self.textures {
            renderer.upload_once(
                gpu,
                TextureKey::EngineCursor(*slot),
                &loaded.texture,
                loaded.alpha,
                &loaded.stored_path,
            );
        }
    }
}

use super::actors::HIGHLIGHT_EMISSIVE;
use super::field::FieldScene;

pub const EMISSIVE_LIT: f32 = 0.999;

pub const EMISSIVE_DARK: f32 = 0.001;

pub const FRAME_NORMAL: u32 = 0;

pub const FRAME_ALTERNATE: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Pointer {
    Hidden,
    Normal,
    Alternate,
    Unchanged,
}

pub fn select(mode: CursorMode, frame: u32) -> Pointer {
    match mode {
        CursorMode::Engine => Pointer::Hidden,
        CursorMode::System => match frame {
            FRAME_NORMAL => Pointer::Normal,
            FRAME_ALTERNATE => Pointer::Alternate,
            _ => Pointer::Unchanged,
        },
    }
}

pub fn frame_after_move(frame: u32, emissive: Option<[f32; 4]>, inside: bool) -> u32 {
    let lit = inside
        && emissive
            .is_some_and(|e| e[0] > EMISSIVE_LIT && e[1] <= EMISSIVE_DARK && e[2] <= EMISSIVE_DARK);

    if lit {
        return FRAME_ALTERNATE;
    }

    if frame == FRAME_ALTERNATE {
        FRAME_NORMAL
    } else {
        frame
    }
}

pub fn inside_client_rect(at: (i32, i32), backbuffer: (u32, u32), inset: (u32, u32)) -> bool {
    let (x, y) = at;

    x > 0 && y > 0 && x < (backbuffer.0 - inset.0) as i32 && y < (backbuffer.1 - inset.1) as i32
}

pub fn hovered_emissive(scene: &FieldScene) -> Option<[f32; 4]> {
    scene.hovered.map(|_| HIGHLIGHT_EMISSIVE)
}

pub const NO_HOVER: Option<[f32; 4]> = None;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct PointerState {
    pub frame: u32,
    pub shown: Option<Pointer>,
}

impl Default for PointerState {
    fn default() -> PointerState {
        PointerState {
            frame: FRAME_NORMAL,
            shown: None,
        }
    }
}

impl PointerState {
    pub fn scene_changed(&mut self) {
        self.frame = FRAME_NORMAL;
    }

    pub fn on_move(
        &mut self,
        mode: CursorMode,
        emissive: Option<[f32; 4]>,
        inside: bool,
    ) -> Option<Pointer> {
        let show = self.choose(mode);

        self.frame = frame_after_move(self.frame, emissive, inside);

        show
    }

    pub fn on_select_only(&mut self, mode: CursorMode) -> Option<Pointer> {
        self.choose(mode)
    }

    fn choose(&mut self, mode: CursorMode) -> Option<Pointer> {
        let choice = select(mode, self.frame);

        if choice == Pointer::Unchanged || self.shown == Some(choice) {
            return None;
        }

        self.shown = Some(choice);

        Some(choice)
    }
}

#[derive(Debug, Clone)]

pub struct Pointers {
    pub mode: CursorMode,
    pub normal: Option<exe::Cursor>,
    pub alternate: Option<exe::Cursor>,
    pub engine: Option<EngineSprite>,
    pub notes: Vec<String>,
}

impl Pointers {
    pub fn none() -> Pointers {
        Pointers {
            mode: CursorMode::System,
            normal: None,
            alternate: None,
            engine: None,
            notes: Vec::new(),
        }
    }

    pub fn load(game: &GameDir, mode: CursorMode) -> Pointers {
        let mut pointers = Pointers {
            mode,
            ..Pointers::none()
        };

        if mode == CursorMode::Engine {
            match EngineSprite::load(game) {
                Ok(sprite) => pointers.engine = Some(sprite),
                Err(error) => pointers
                    .notes
                    .push(format!("engine cursor did not load: {error:#}")),
            }
        }

        let image = match game.read(b"WYD.exe") {
            Ok(image) => image,
            Err(error) => {
                pointers.notes.push(format!(
                    "no pointer artwork: WYD.exe could not be read: {error}"
                ));

                return pointers;
            }
        };

        let resources = match exe::Resources::parse(&image) {
            Ok(resources) => resources,
            Err(error) => {
                pointers.notes.push(format!(
                    "no pointer artwork: WYD.exe has no readable resources: {error}"
                ));

                return pointers;
            }
        };

        for (id, slot) in [
            (exe::CURSOR_NORMAL, &mut pointers.normal),
            (exe::CURSOR_ALTERNATE, &mut pointers.alternate),
        ] {
            match exe::group_cursor(&resources, id) {
                Ok(cursor) => *slot = Some(cursor),
                Err(error) => pointers
                    .notes
                    .push(format!("cursor {id:#04X} did not decode: {error}")),
            }
        }

        pointers
    }

    pub fn queue(
        &self,
        gpu: &Gpu,
        renderer: &mut SceneRenderer,
        viewport: (u32, u32),
        frame: u32,
        position: Option<(i32, i32)>,
    ) {
        if self.mode != CursorMode::Engine {
            return;
        }

        let (Some(sprite), Some(position)) = (&self.engine, position) else {
            return;
        };

        if let Some(draw) = sprite.draw(viewport, frame, position) {
            sprite.upload(gpu, renderer);

            renderer.push(&draw);
        }
    }
}

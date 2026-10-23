use crate::scene::Scene;
use crate::scene::actors::SkinAlpha;
use crate::scene::world::World;
use render::billboard::{BillboardArray, BillboardRenderer};
use render::device::Gpu;
use render::mesh::{MeshPass, MeshRenderer, MeshTexture};
use render::preset::RenderMode;
use render::quirk::VendorClass;
use render::renderer::{SpriteRenderer, TextureSet};
use render::sea::{SeaRenderer, SeaTexture};
use render::skin::{SkinFrame, SkinPass, SkinRenderer};
use render::sprite::{Quad, SpriteBatcher, UvRule};
use render::target::{DEPTH_FORMAT, DepthBuffer};
use render::texture::{self, AlphaPolicy};
use render::world::{FrameUniforms, TerrainRenderer, TileCache, TileUploadStats};
use std::collections::HashMap;
use ui::draw::Owner;

mod antialias;
mod light_shafts;
mod world_textures;
mod zone_upload;

pub use world_textures::MaterialUploadStats;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod timing_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod ui_model_tests;

#[must_use = "a prepared frame must be submitted or explicitly discarded"]
pub struct PreparedFrame<'a> {
    renderer: &'a mut SceneRenderer,
    gpu: &'a Gpu,
}

impl PreparedFrame<'_> {
    pub fn submit(self, view: &wgpu::TextureView) {
        self.renderer.render_frame(self.gpu, view);
    }

    pub fn submit_timed(
        self,
        view: &wgpu::TextureView,
        timing: Option<&render::timing::GpuTimingFrame>,
    ) {
        self.renderer.render_frame_timed(self.gpu, view, timing);
    }
}

pub const CLEAR: wgpu::Color = wgpu::Color::TRANSPARENT;

pub type PanelId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AmulSheet {
    Plain,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MiniPopupPhase {
    Lit,
    Dark,
}

impl AmulSheet {
    pub fn of(sheet: i32) -> Option<AmulSheet> {
        match sheet {
            ui::icon::SKILL_SHEET => Some(AmulSheet::Plain),
            ui::icon::AFFECT_SHEET => Some(AmulSheet::On),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TextureKey {
    Scene(u32),
    EngineCursor(u32),
    ExternalUi(u32),
    GuiMat(i32),
    HoverGuiMat(i32),
    MiniPopupIcon(MiniPopupPhase),
    Caption(PanelId, Owner, u32),
    ItemIcon(i32),
    NamedIcon(PanelId, Owner),
    Amul(AmulSheet),
    SceneText(u32),
    White,
    Effect(usize),
    GuildMark(usize),
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedQuad {
    pub mode: RenderMode,
    pub texture: TextureKey,
    pub quad: Quad,
    pub uv: UvRule,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct WorldResidency {
    identity: u64,
    terrain_vertices: usize,
    terrain_batches: usize,
    terrain_revision: u64,
    tiles: usize,
    mesh_allocs: usize,
    mesh_textures: usize,
    mesh_texture_updates: usize,
    skin_allocs: usize,
    skin_textures: usize,
    skin_texture_updates: usize,
    skin_glow_pairs: usize,
    effect_textures: usize,
    effect_arrays: usize,
    sea_textures: usize,
    sea_bound: bool,
}

impl WorldResidency {
    fn of(world: &World) -> Self {
        Self {
            identity: world.identity,
            terrain_vertices: world.mesh.vertices.len(),
            terrain_batches: world.mesh.batches.len(),
            terrain_revision: world.terrain_revision,
            tiles: world.tiles.len(),
            mesh_allocs: world.objects.arena.allocs.len(),
            mesh_textures: world.objects.textures.len(),
            mesh_texture_updates: world.objects.texture_updates.len(),
            skin_allocs: world.actors.arena.allocs.len(),
            skin_textures: world.actors.textures.len(),
            skin_texture_updates: world.actors.texture_updates.len(),
            skin_glow_pairs: world.actors.glow_pairs.len(),
            effect_textures: world.effects.textures.len(),
            effect_arrays: world.effects.arrays.len(),
            sea_textures: world.sea.textures.len(),
            sea_bound: world.sea.bound(),
        }
    }
}

pub struct SceneRenderer {
    pub(crate) quad_scratch: Vec<PlannedQuad>,
    target_format: wgpu::TextureFormat,
    sprites: SpriteRenderer,
    batcher: SpriteBatcher<TextureKey>,
    textures: TextureSet<TextureKey>,
    ui_models: Option<UiModels>,
    caption_serials: HashMap<(PanelId, Owner, u32), (u64, usize)>,
    caption_owners: HashMap<u64, std::sync::Weak<()>>,
    icon_serials: HashMap<(PanelId, Owner), (u64, usize)>,
    scene_text_serials: HashMap<u32, (u64, usize)>,
    scene_text_owners: HashMap<u64, std::sync::Weak<()>>,
    guild_mark_serials: HashMap<usize, (u64, u64)>,
    terrain: TerrainRenderer,
    world_resident: Option<WorldResidency>,
    tiles: TileCache,
    mesh: Option<Vec<render::world::TerrainBatch>>,
    meshes: MeshRenderer,
    world_textures: world_textures::WorldTextures,
    zone_staging: zone_upload::Staging,
    upload_budget: zone_upload::Budget,
    mesh_queued: bool,
    skins: SkinRenderer,
    skin_queued: bool,
    skin_drops: render::skin::SkinDrops,
    seas: SeaRenderer,
    sea_textures: Vec<SeaTexture>,
    sea_queued: bool,
    billboards: BillboardRenderer,
    billboard_arrays: Vec<BillboardArray>,
    billboard_queued: bool,
    vendor: VendorClass,
    world_frame: Option<(FrameUniforms, bool)>,
    temporal: Option<crate::temporal::TemporalFrame>,
    antialias: Option<antialias::AntialiasFrame>,
    light_shafts: Option<light_shafts::LightShaftFrame>,
    queued_light_shafts: Vec<render::light_shafts::LightShaft>,
    neural_failure: Option<String>,
    terrain_clock: Option<u32>,
    terrain_queued: bool,
    depth: Option<DepthBuffer>,
    clear: wgpu::Color,
    world_uploads: usize,
    uploads: usize,
}

struct UiModels {
    renderer: MeshRenderer,
    depth: Option<DepthBuffer>,
    textures: Vec<MeshTexture>,
    resident: Option<(u64, usize, usize)>,
    spans: Vec<(u32, std::ops::Range<usize>)>,
    prepared: bool,
}

impl SceneRenderer {
    pub fn new(gpu: &Gpu, target_format: wgpu::TextureFormat) -> SceneRenderer {
        Self::with_temporal(gpu, target_format, None)
    }

    pub fn new_with_motion(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> SceneRenderer {
        Self::with_temporal(
            gpu,
            target_format,
            crate::temporal::TemporalFrame::new(gpu, size, target_format),
        )
    }

    pub fn motion_vectors(&self) -> Option<&wgpu::TextureView> {
        self.temporal.as_ref().map(|state| &state.motion)
    }

    pub fn new_with_neural(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> SceneRenderer {
        #[cfg(feature = "neural-rendering")]
        {
            match crate::temporal::TemporalFrame::new_neural(gpu, size, target_format) {
                Ok(frame) => Self::with_temporal(gpu, target_format, Some(frame)),
                Err(error) => {
                    let mut renderer = Self::new(gpu, target_format);
                    renderer.neural_failure = Some(error);
                    renderer
                }
            }
        }

        #[cfg(not(feature = "neural-rendering"))]
        {
            let _ = size;
            Self::new(gpu, target_format)
        }
    }

    pub fn release_neural(&mut self) {
        if let Some(temporal) = &mut self.temporal {
            temporal.release_neural();
        }
    }

    pub fn take_neural_failure(&mut self) -> Option<String> {
        self.neural_failure.take().or_else(|| {
            self.temporal
                .as_mut()
                .and_then(|temporal| temporal.take_failure())
        })
    }

    pub fn reactive_mask(&self) -> Option<&wgpu::TextureView> {
        self.temporal.as_ref().map(|state| &state.bias)
    }

    fn with_temporal(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        temporal: Option<crate::temporal::TemporalFrame>,
    ) -> SceneRenderer {
        let camera = temporal.as_ref().map(|state| &state.camera);

        let depth_format = if camera.is_some() {
            wgpu::TextureFormat::Depth32Float
        } else {
            DEPTH_FORMAT
        };

        let sprites = SpriteRenderer::new(gpu, target_format);

        let mut renderer = SceneRenderer {
            quad_scratch: Vec::new(),
            target_format,
            sprites,
            batcher: SpriteBatcher::new(),
            textures: TextureSet::new(),
            ui_models: None,
            caption_serials: HashMap::new(),
            caption_owners: HashMap::new(),
            icon_serials: HashMap::new(),
            scene_text_serials: HashMap::new(),
            scene_text_owners: HashMap::new(),
            guild_mark_serials: HashMap::new(),
            terrain: TerrainRenderer::with_temporal(gpu, target_format, depth_format, camera),
            world_resident: None,
            world_uploads: 0,
            uploads: 0,
            tiles: TileCache::default(),
            mesh: None,
            meshes: MeshRenderer::with_temporal(
                gpu,
                target_format,
                depth_format,
                VendorClass::DEFAULT,
                camera,
            ),
            world_textures: world_textures::WorldTextures::default(),
            zone_staging: zone_upload::Staging::default(),
            upload_budget: zone_upload::Budget::new(),
            mesh_queued: false,
            skins: SkinRenderer::with_temporal(
                gpu,
                target_format,
                depth_format,
                VendorClass::DEFAULT,
                camera,
            ),
            skin_queued: false,
            skin_drops: render::skin::SkinDrops::default(),
            seas: SeaRenderer::with_temporal(gpu, target_format, depth_format, camera),
            sea_textures: Vec::new(),
            sea_queued: false,
            billboards: BillboardRenderer::with_temporal(gpu, target_format, depth_format, camera),
            billboard_arrays: Vec::new(),
            billboard_queued: false,
            vendor: VendorClass::DEFAULT,
            world_frame: None,
            terrain_clock: None,
            terrain_queued: false,
            depth: None,
            clear: CLEAR,
            temporal,
            antialias: antialias::AntialiasFrame::new(gpu, target_format),
            light_shafts: None,
            queued_light_shafts: Vec::new(),
            neural_failure: None,
        };

        let white = formats::texture::Texture {
            width: 1,
            height: 1,
            format: formats::texture::PixelFormat::Rgba8Unorm,
            mips: vec![formats::texture::MipLevel {
                width: 1,
                height: 1,
                offset: 0,
                len: 4,
            }],
            data: vec![0xFF, 0xFF, 0xFF, 0xFF],
            source_len: 4,
        };

        renderer.upload(
            gpu,
            TextureKey::White,
            &white,
            AlphaPolicy::Stored,
            "1x1 white",
        );

        renderer
    }

    pub fn upload(
        &mut self,
        gpu: &Gpu,
        key: TextureKey,
        decoded: &formats::texture::Texture,
        alpha: AlphaPolicy,
        label: &str,
    ) {
        let mode = match key {
            TextureKey::Scene(_)
            | TextureKey::EngineCursor(_)
            | TextureKey::SceneText(_)
            | TextureKey::GuildMark(_) => RenderMode::SControl2d,
            TextureKey::Effect(_) => RenderMode::LensFlare2d,
            TextureKey::GuiMat(_)
            | TextureKey::ExternalUi(_)
            | TextureKey::HoverGuiMat(_)
            | TextureKey::MiniPopupIcon(_)
            | TextureKey::ItemIcon(_)
            | TextureKey::NamedIcon(..)
            | TextureKey::Amul(_)
            | TextureKey::Caption(..)
            | TextureKey::White
            | TextureKey::Overlay => RenderMode::NuiPanel2d,
        };

        let uploaded = texture::upload(gpu, decoded, alpha, label);

        let bound = self.sprites.bind(gpu, &uploaded, mode);

        self.textures.insert(key, bound);

        self.uploads += 1;
    }

    pub fn upload_once(
        &mut self,
        gpu: &Gpu,
        key: TextureKey,
        decoded: &formats::texture::Texture,
        alpha: AlphaPolicy,
        label: &str,
    ) {
        let global = matches!(
            key,
            TextureKey::GuiMat(_)
                | TextureKey::EngineCursor(_)
                | TextureKey::ExternalUi(_)
                | TextureKey::HoverGuiMat(_)
                | TextureKey::MiniPopupIcon(_)
                | TextureKey::ItemIcon(_)
                | TextureKey::Amul(_)
                | TextureKey::Effect(_)
        );

        if global && self.textures.get(&key).is_some() {
            return;
        }

        self.upload(gpu, key, decoded, alpha, label);
    }

    pub fn sync_captions(&mut self, gpu: &Gpu, panel: PanelId, ui: &crate::ui::PanelUi) {
        let identity = ui.identity();

        self.caption_owners.insert(identity, ui.lifetime());

        self.caption_serials
            .retain(|&(id, owner, run), &mut (cache, _)| {
                if id != panel || cache != identity || ui.cache.get(&(owner, run)).is_some() {
                    return true;
                }

                self.textures.remove(&TextureKey::Caption(id, owner, run));

                false
            });

        for (key, cached) in ui.cache.iter() {
            let key = (panel, key.0, key.1);

            let revision = (ui.identity(), cached.serial);

            if self.caption_serials.get(&key) == Some(&revision) {
                continue;
            }

            let decoded = formats::texture::Texture {
                width: cached.bitmap.width,
                height: cached.bitmap.height,
                format: formats::texture::PixelFormat::Rgba8Unorm,
                mips: vec![formats::texture::MipLevel {
                    width: cached.bitmap.width,
                    height: cached.bitmap.height,
                    offset: 0,
                    len: cached.bitmap.pixels.len(),
                }],
                data: cached.bitmap.pixels.clone(),
                source_len: cached.bitmap.pixels.len(),
            };

            self.upload(
                gpu,
                TextureKey::Caption(key.0, key.1, key.2),
                &decoded,
                AlphaPolicy::Stored,
                "caption bitmap",
            );

            self.caption_serials.insert(key, revision);
        }
    }

    pub fn sync_named_icons(&mut self, gpu: &Gpu, panel: PanelId, ui: &crate::ui::PanelUi) {
        for (&owner, image) in &ui.icon_images {
            let Some(texture) = &image.texture else {
                continue;
            };

            let key = (panel, owner);

            let revision = (ui.identity(), image.serial);

            if self.icon_serials.get(&key) == Some(&revision) {
                continue;
            }

            let texture_key = TextureKey::NamedIcon(panel, owner);

            match texture.as_ref() {
                formats::texture::TextureResource::Image(texture) => {
                    self.upload(gpu, texture_key, texture, AlphaPolicy::Stored, &image.path)
                }
                formats::texture::TextureResource::Cube(cube) => {
                    let Some(texture) =
                        render::texture::upload_cube(gpu, cube, AlphaPolicy::Stored, &image.path)
                    else {
                        tracing::warn!(
                            path =% image.path, "invalid cube image resource"
                        );

                        continue;
                    };

                    let bound = self
                        .sprites
                        .bind_cube(gpu, &texture, RenderMode::NuiPanel2d);

                    self.textures.insert(texture_key, bound);

                    self.uploads += 1;
                }
            }

            self.icon_serials.insert(key, revision);
        }
    }

    pub fn sync_scene_text(&mut self, gpu: &Gpu, text: &crate::scene::stext::SceneText) {
        let identity = text.identity();

        self.scene_text_owners
            .entry(identity)
            .or_insert_with(|| text.lifetime());

        self.scene_text_serials.retain(|id, (owner, _)| {
            if *owner != identity || text.get(*id).is_some() {
                return true;
            }

            self.textures.remove(&TextureKey::SceneText(*id));

            false
        });

        for (name_id, line) in text.iter() {
            let revision = (text.identity(), line.serial);

            if self.scene_text_serials.get(name_id) == Some(&revision) {
                continue;
            }

            let decoded = formats::texture::Texture {
                width: line.bitmap.width,
                height: line.bitmap.height,
                format: formats::texture::PixelFormat::Rgba8Unorm,
                mips: vec![formats::texture::MipLevel {
                    width: line.bitmap.width,
                    height: line.bitmap.height,
                    offset: 0,
                    len: line.bitmap.pixels.len(),
                }],
                data: line.bitmap.pixels.clone(),
                source_len: line.bitmap.pixels.len(),
            };

            self.upload(
                gpu,
                TextureKey::SceneText(*name_id),
                &decoded,
                AlphaPolicy::Stored,
                "S* text line",
            );

            self.scene_text_serials.insert(*name_id, revision);
        }
    }

    pub fn sync_guild_marks(&mut self, gpu: &Gpu, marks: &crate::scene::guild_mark::Marks) {
        use formats::data::guild_mark::{BMP_HEIGHT, BMP_WIDTH};

        let (width, height) = (BMP_WIDTH as u32, BMP_HEIGHT as u32);

        for (slot, record) in marks.bank.ready() {
            let revision = (marks.bank.identity(), record.serial);

            if self.guild_mark_serials.get(&slot) == Some(&revision) {
                continue;
            }

            let Some(pixels) = record.pixels.as_ref() else {
                continue;
            };

            let decoded = formats::texture::Texture {
                width,
                height,
                format: formats::texture::PixelFormat::Rgba8Unorm,
                mips: vec![formats::texture::MipLevel {
                    width,
                    height,
                    offset: 0,
                    len: pixels.len(),
                }],
                data: pixels.clone(),
                source_len: pixels.len(),
            };

            self.upload(
                gpu,
                TextureKey::GuildMark(slot),
                &decoded,
                AlphaPolicy::Stored,
                "guild badge",
            );

            self.guild_mark_serials.insert(slot, revision);
        }
    }

    pub fn upload_world(&mut self, gpu: &Gpu, world: &World) {
        let want = WorldResidency::of(world);

        let have = self.world_resident.filter(|r| r.identity == want.identity);

        let fresh = |same: bool| have.is_none() || !same;

        if have.is_none() {
            self.world_textures.clear();

            self.light_shafts = None;

            self.queued_light_shafts.clear();

            self.tiles.clear();

            self.zone_staging = zone_upload::Staging::default();
        }

        gpu.breadcrumb.set(format!(
            "upload_world — {} actors, {} resident zones, {} skin textures, {} mesh textures, \
             {} terrain verts, {} world uploads so far",
            world.actors.actors.len(),
            world.streamed.len() + 1,
            world.actors.textures.len(),
            world.objects.textures.len(),
            world.mesh.vertices.len(),
            self.world_uploads,
        ));

        if fresh(have.is_some_and(|r| {
            (r.terrain_vertices, r.terrain_batches, r.terrain_revision)
                == (
                    want.terrain_vertices,
                    want.terrain_batches,
                    want.terrain_revision,
                )
        })) {
            if let Some(old) = have {
                self.terrain.update_mesh(
                    gpu,
                    &world.mesh,
                    world.terrain_changes_since(old.terrain_revision, old.terrain_vertices),
                );
            } else {
                self.terrain.upload_mesh(gpu, &world.mesh);
            }

            self.world_uploads += 1;

            self.mesh = Some(world.mesh.batches.clone());
        }

        if fresh(have.is_some_and(|r| r.tiles == want.tiles)) {
            let tiles: Vec<_> = world.tiles.iter().map(|t| (t.id, &t.texture)).collect();

            if !self.tiles.sync(gpu, &self.terrain, &tiles) {
                tracing::warn!(
                    tiles = tiles.len(),
                    "terrain tile data or array capacity is invalid"
                );
            }

            self.world_uploads += 1;
        }

        self.clear = world.clear_color();

        if fresh(have.is_some_and(|r| r.mesh_allocs == want.mesh_allocs)) {
            if have.is_some() {
                self.meshes.append_staged_arena(
                    gpu,
                    &world.objects.arena,
                    &self.zone_staging.mesh_copies(world),
                );
            } else {
                self.meshes.upload_arena(gpu, &world.objects.arena);
            }

            self.world_uploads += 1;
        }

        if fresh(have.is_some_and(|r| {
            (r.mesh_textures, r.mesh_texture_updates)
                == (want.mesh_textures, want.mesh_texture_updates)
        })) {
            self.world_textures
                .sync_meshes(gpu, &self.meshes, &world.objects.textures);

            if let Some(have) = have {
                self.world_textures.refresh_meshes(
                    gpu,
                    &self.meshes,
                    &world.objects.textures,
                    &world.objects.texture_updates[have.mesh_texture_updates..],
                    have.mesh_textures,
                );
            }

            self.world_uploads += 1;
        }

        if fresh(have.is_some_and(|r| r.skin_allocs == want.skin_allocs)) {
            if have.is_some() {
                self.skins.append_staged_arena(
                    gpu,
                    &world.actors.arena,
                    &self.zone_staging.skin_copies(world),
                );
            } else {
                self.skins.upload_arena(gpu, &world.actors.arena);
            }

            self.world_uploads += 1;
        }

        if fresh(have.is_some_and(|r| {
            (r.skin_textures, r.skin_glow_pairs, r.skin_texture_updates)
                == (
                    want.skin_textures,
                    want.skin_glow_pairs,
                    want.skin_texture_updates,
                )
        })) {
            self.world_textures.sync_skins(
                gpu,
                &self.skins,
                &world.actors.textures,
                &world.actors.glow_pairs,
            );

            if let Some(have) = have {
                self.world_textures.refresh_skins(
                    gpu,
                    &self.skins,
                    &world.actors.textures,
                    &world.actors.glow_pairs,
                    &world.actors.texture_updates[have.skin_texture_updates..],
                    have.skin_textures,
                );
            }

            self.world_uploads += 1;
        }

        if fresh(have.is_some_and(|r| {
            (r.effect_textures, r.effect_arrays) == (want.effect_textures, want.effect_arrays)
        })) {
            self.billboard_arrays = world.effects.upload_arrays(gpu, &self.billboards);

            self.world_uploads += 1;
        }

        if fresh(
            have.is_some_and(|r| {
                (r.sea_textures, r.sea_bound) == (want.sea_textures, want.sea_bound)
            }),
        ) {
            self.sea_textures = world.sea.upload_textures(gpu, &self.seas);

            self.world_uploads += 1;
        }

        self.world_textures.flush(
            gpu,
            &self.meshes,
            &world.objects.textures,
            &self.skins,
            &world.actors.textures,
            &world.actors.glow_pairs,
            &mut self.upload_budget,
        );

        self.world_resident = Some(want);

        if self.zone_staging.complete && self.zone_staging.matches(world.identity) {
            self.zone_staging = zone_upload::Staging::default();

            self.world_textures.clear_staged();
        }
    }

    pub fn stream_zones(
        &mut self,
        gpu: &Gpu,
        scene: &mut Scene,
        game: Option<&assets::GameDir>,
    ) -> bool {
        match scene {
            Scene::Field(scene) => {
                crate::scene::field::stream_zones_staged(scene, game, &mut |identity, prepared| {
                    self.zone_staging.advance(
                        gpu,
                        identity,
                        prepared,
                        &mut self.world_textures,
                        &mut self.upload_budget,
                    )
                })
            }
            _ => false,
        }
    }

    pub fn begin_upload_frame(&mut self) {
        self.upload_budget = zone_upload::Budget::new();
    }

    pub fn finish_upload_frame(&self) {
        self.upload_budget.report();
    }

    pub fn queue_world(
        &mut self,
        gpu: &Gpu,
        uniforms: FrameUniforms,
        stage1_enabled: bool,
        objects: &crate::scene::objects::Frame,
    ) {
        self.world_frame = Some((uniforms, stage1_enabled));

        self.queued_light_shafts.clone_from(&objects.light_shafts);

        self.terrain_clock = None;

        self.terrain_queued = true;

        self.meshes
            .prepare(gpu, &uniforms, &objects.objects, &objects.draws);

        self.mesh_queued = !objects.draws.is_empty();
    }

    pub fn queue_world_at(
        &mut self,
        gpu: &Gpu,
        uniforms: FrameUniforms,
        objects: &crate::scene::objects::Frame,
        now_ms: u32,
    ) {
        self.queue_world_at_with_history(gpu, uniforms, objects, now_ms, &[]);
    }

    pub fn queue_world_at_with_history(
        &mut self,
        gpu: &Gpu,
        uniforms: FrameUniforms,
        objects: &crate::scene::objects::Frame,
        now_ms: u32,
        held: &[crate::scene::actors::HeldDraw],
    ) {
        self.world_frame = Some((uniforms, false));

        self.queued_light_shafts.clone_from(&objects.light_shafts);

        self.terrain_clock = Some(now_ms);

        self.terrain_queued = true;

        if self.temporal.is_some() {
            let keys: Vec<_> = objects
                .objects
                .iter()
                .map(|object| {
                    let mut matching = held
                        .iter()
                        .filter(|held| held.world.shader_constants() == object.world);

                    let first = matching.next().map(|held| held.motion_key);

                    if matching.next().is_none() {
                        first
                    } else {
                        None
                    }
                })
                .collect();

            self.meshes.prepare_with_history(
                gpu,
                &uniforms,
                &objects.objects,
                &objects.draws,
                &keys,
            );
        } else {
            self.meshes
                .prepare(gpu, &uniforms, &objects.objects, &objects.draws);
        }

        self.mesh_queued = !objects.draws.is_empty();
    }

    pub fn set_terrain_enabled(&mut self, enabled: bool) {
        self.terrain_queued = enabled;
    }

    pub fn queue_skins(
        &mut self,
        gpu: &Gpu,
        frame: &SkinFrame,
        actors: &crate::scene::actors::Frame,
    ) {
        self.skins.prepare_with_history(
            gpu,
            frame,
            &actors.objects,
            &actors.palettes,
            &actors.draws,
            self.world_textures.skins.len(),
            &actors.motion_keys,
        );

        self.skin_queued = !actors.draws.is_empty();

        let drops = self.skins.drops();

        if drops.worse_than(&self.skin_drops) {
            self.skin_drops = drops;

            tracing::info!(
                "NWYD: skinned pass dropped {} draw(s) with invalid palette/object references \
                 ({} palettes, capacity {}), \
                 {} for an unbound part and {} for an unbound texture — \
                 those bodies draw nothing and keep their nameplates",
                drops.over_capacity,
                drops.palette_demand,
                self.skins.palette_capacity(),
                drops.unbound_part,
                drops.unbound_texture,
            );
        }
    }

    pub fn queue_seas(
        &mut self,
        gpu: &Gpu,
        uniforms: &FrameUniforms,
        seas: &crate::scene::sea::Frame,
    ) {
        self.seas.prepare(
            gpu,
            uniforms,
            &seas.objects,
            &seas.vertices,
            &seas.indices,
            &seas.draws,
        );

        self.sea_queued = !seas.draws.is_empty();
    }

    pub fn queue_effects(
        &mut self,
        gpu: &Gpu,
        uniforms: &FrameUniforms,
        effects: &crate::scene::effects::Frame,
    ) {
        self.billboards.prepare(
            gpu,
            uniforms,
            &effects.objects,
            &effects.vertices,
            &effects.indices,
            &effects.draws,
        );

        self.billboard_queued = !effects.draws.is_empty();
    }

    pub fn upload_count(&self) -> usize {
        self.uploads
    }

    pub fn world_upload_count(&self) -> usize {
        self.world_uploads
    }

    pub fn world_texture_upload_counts(&self) -> (usize, usize) {
        self.world_textures.uploads
    }

    pub fn material_upload_stats(&self) -> MaterialUploadStats {
        self.world_textures.stats()
    }

    pub fn tile_upload_stats(&self) -> TileUploadStats {
        self.tiles.stats()
    }

    pub fn terrain_vertex_upload_stats(&self) -> render::geometry::GeometryUploadStats {
        self.terrain.vertex_upload_stats()
    }

    pub fn geometry_upload_stats(
        &self,
    ) -> (
        render::geometry::GeometryUploadStats,
        render::geometry::GeometryUploadStats,
    ) {
        (
            self.meshes.geometry_upload_stats(),
            self.skins.geometry_upload_stats(),
        )
    }

    pub fn skin_alpha_reference(&self) -> SkinAlpha {
        SkinAlpha {
            opaque: SkinPass::Opaque.alpha_reference(self.vendor),
            blended: SkinPass::Blended.alpha_reference(self.vendor),
        }
    }

    pub fn vendor(&self) -> VendorClass {
        self.vendor
    }

    pub fn begin(&mut self) {
        let captions = self.caption_owners.len();

        self.caption_owners
            .retain(|_, owner| owner.strong_count() != 0);

        if self.caption_owners.len() != captions {
            self.caption_serials
                .retain(|&(panel, owner, run), &mut (cache, _)| {
                    if self.caption_owners.contains_key(&cache) {
                        return true;
                    }

                    self.textures
                        .remove(&TextureKey::Caption(panel, owner, run));

                    false
                });
        }

        let owners = self.scene_text_owners.len();

        self.scene_text_owners
            .retain(|_, owner| owner.strong_count() != 0);

        if self.scene_text_owners.len() != owners {
            self.scene_text_serials.retain(|id, (owner, _)| {
                if self.scene_text_owners.contains_key(owner) {
                    return true;
                }

                self.textures.remove(&TextureKey::SceneText(*id));

                false
            });
        }

        self.batcher.clear();

        if let Some(models) = &mut self.ui_models {
            models.spans.clear();

            models.prepared = false;
        }

        self.world_frame = None;

        self.queued_light_shafts.clear();

        if let Some(shafts) = &mut self.light_shafts {
            shafts.clear();
        }

        self.terrain_clock = None;

        self.terrain_queued = false;

        self.mesh_queued = false;

        self.skin_queued = false;

        self.sea_queued = false;

        self.billboard_queued = false;
    }

    pub fn push(&mut self, planned: &PlannedQuad) {
        let (w, h) = self
            .textures
            .get(&planned.texture)
            .map(|t| (t.width, t.height))
            .unwrap_or((1, 1));

        self.batcher.push_with(
            planned.mode,
            planned.texture,
            &planned.quad,
            w,
            h,
            planned.uv,
        );
    }

    pub fn queue_ui_meshes(
        &mut self,
        gpu: &Gpu,
        identity: u64,
        resources: &crate::scene::objects::ObjectScene,
        uniforms: &FrameUniforms,
        frame: &crate::scene::objects::Frame,
    ) {
        if frame.draws.is_empty() {
            return;
        }

        let models = self.ui_models.get_or_insert_with(|| UiModels {
            renderer: MeshRenderer::new_ui(gpu, self.target_format, self.vendor),
            depth: None,
            textures: Vec::new(),
            resident: None,
            spans: Vec::new(),
            prepared: false,
        });

        let want = (
            identity,
            resources.arena.allocs.len(),
            resources.textures.len(),
        );

        let have = models.resident.filter(|previous| previous.0 == identity);

        if !have.is_some_and(|previous| previous.1 == want.1) {
            models.renderer.upload_arena(gpu, &resources.arena);
        }

        if !have.is_some_and(|previous| previous.2 == want.2) {
            models.textures = resources.upload_textures(gpu, &models.renderer);
        }

        models.resident = Some(want);

        models
            .renderer
            .prepare(gpu, uniforms, &frame.objects, &frame.draws);

        models.prepared = true;
    }

    pub fn push_ui_model(&mut self, range: std::ops::Range<usize>) {
        if let Some(models) = &mut self.ui_models
            && models.prepared
            && range.start < range.end
            && range.end <= models.renderer.draw_count()
        {
            models.spans.push((self.batcher.quad_count(), range));
        }
    }

    pub fn bind_overlay(&mut self, gpu: &Gpu, atlas: &crate::overlay::Atlas) {
        self.upload(
            gpu,
            TextureKey::Overlay,
            &atlas.texture,
            AlphaPolicy::Stored,
            "debug overlay glyph atlas",
        );
    }

    pub fn push_overlay(&mut self, draws: &[crate::overlay::Draw]) {
        for draw in draws {
            self.push(&PlannedQuad {
                mode: RenderMode::NuiPanel2d,
                texture: if draw.glyph {
                    TextureKey::Overlay
                } else {
                    TextureKey::White
                },
                quad: draw.quad,
                uv: UvRule::TexelEdge,
            });
        }
    }

    pub fn prepare(&mut self, gpu: &Gpu, viewport: (u32, u32)) {
        let size = if let Some(state) = &mut self.temporal {
            state.resize(gpu, viewport);

            if let Some((uniforms, _)) = self.world_frame {
                state.prepare(gpu, &uniforms);
            } else {
                state.reset();
            }

            (state.scene.width, state.scene.height)
        } else {
            viewport
        };

        self.prepare_for_targets(gpu, size, viewport);
    }

    pub fn prepare_frame<'a>(
        &'a mut self,
        gpu: &'a Gpu,
        viewport: (u32, u32),
    ) -> PreparedFrame<'a> {
        self.prepare(gpu, viewport);

        PreparedFrame {
            renderer: self,
            gpu,
        }
    }

    pub fn prepare_for_targets(
        &mut self,
        gpu: &Gpu,
        scene_size: (u32, u32),
        output_size: (u32, u32),
    ) {
        self.sprites
            .prepare(gpu, &self.batcher, output_size, &self.textures);

        if let Some(models) = &mut self.ui_models
            && models.prepared
            && !models.spans.is_empty()
            && !models
                .depth
                .as_ref()
                .is_some_and(|depth| depth.fits(output_size.0, output_size.1))
        {
            models.depth = Some(DepthBuffer::new(gpu, output_size.0, output_size.1));
        }

        let Some((uniforms, stage1_enabled)) = self.world_frame else {
            return;
        };

        if !self
            .depth
            .as_ref()
            .is_some_and(|d| d.fits(scene_size.0, scene_size.1))
        {
            let format = if self.temporal.is_some() {
                wgpu::TextureFormat::Depth32Float
            } else {
                DEPTH_FORMAT
            };

            self.depth = Some(DepthBuffer::with_format(
                gpu,
                scene_size.0,
                scene_size.1,
                format,
            ));
        }

        if self.terrain_queued
            && let (Some(mesh), Some(tiles)) = (&self.mesh, self.tiles.bound())
        {
            if let Some(now_ms) = self.terrain_clock {
                self.terrain.prepare_at(gpu, mesh, tiles, &uniforms, now_ms);
            } else {
                self.terrain
                    .prepare(gpu, mesh, tiles, &uniforms, stage1_enabled);
            }
        }

        if !self.queued_light_shafts.is_empty() || self.light_shafts.is_some() {
            let shafts = self
                .light_shafts
                .get_or_insert_with(|| light_shafts::LightShaftFrame::new(gpu, self.target_format));
            shafts.prepare(
                gpu,
                &uniforms,
                &self.queued_light_shafts,
                &self.depth.as_ref().unwrap().view,
                &self.world_textures.mesh_views,
            );
        }
    }

    pub fn render(&self, gpu: &Gpu, view: &wgpu::TextureView) {
        assert!(
            self.temporal.is_none(),
            "motion-vector renderers must use render_frame"
        );

        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: render::gpu_label("frame"),
            });

        let has_world = self.has_world();

        self.encode_scene(&mut enc, view, has_world);

        self.encode_2d(&mut enc, view, has_world);

        gpu.queue.submit([enc.finish()]);
    }

    pub fn render_frame(&mut self, gpu: &Gpu, view: &wgpu::TextureView) {
        self.render_frame_timed(gpu, view, None);
    }

    fn render_frame_timed(
        &mut self,
        gpu: &Gpu,
        view: &wgpu::TextureView,
        timing: Option<&render::timing::GpuTimingFrame>,
    ) {
        let has_world = self.has_world();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: render::gpu_label("scene composition and interface"),
            });
        if (!has_world || self.temporal.is_none())
            && let Some(timing) = timing
        {
            timing.write(&mut encoder, render::timing::GpuTimingPoint::SceneStart);
        }
        if has_world {
            if let Some(antialias) = &mut self.antialias {
                antialias.resize(
                    gpu,
                    view.texture().width(),
                    view.texture().height(),
                    self.temporal.as_ref().map(|state| &state.bias),
                );
            }
            let scene_target = self
                .antialias
                .as_ref()
                .map_or(view, |antialias| antialias.view());
            if let Some(state) = &self.temporal {
                self.render_scene_timed(gpu, &state.scene.view, timing);
                self.temporal.as_mut().unwrap().finish(
                    gpu,
                    scene_target,
                    self.depth.as_ref().map(|depth| &depth.view),
                );
            } else {
                self.encode_scene(&mut encoder, scene_target, true);
            }
            if let Some(antialias) = &self.antialias {
                antialias.encode(&mut encoder, view);
            }
        } else {
            self.reset_temporal_history();
        }

        self.encode_2d(&mut encoder, view, has_world);
        if let Some(timing) = timing {
            timing.write(&mut encoder, render::timing::GpuTimingPoint::SceneEnd);
        }
        gpu.queue.submit([encoder.finish()]);
    }

    pub fn reset_temporal_history(&mut self) {
        if let Some(state) = &mut self.temporal {
            state.reset();
        }

        self.skins.reset_motion_history();

        self.meshes.reset_motion_history();
    }

    pub fn temporal_scene_stamp(&mut self, identity: u64, clock: u32) {
        if self
            .temporal
            .as_mut()
            .is_some_and(|state| state.scene_stamp(identity, clock))
        {
            self.skins.reset_motion_history();

            self.meshes.reset_motion_history();
        }
    }

    pub fn render_scene(&self, gpu: &Gpu, view: &wgpu::TextureView) {
        self.render_scene_timed(gpu, view, None);
    }

    fn render_scene_timed(
        &self,
        gpu: &Gpu,
        view: &wgpu::TextureView,
        timing: Option<&render::timing::GpuTimingFrame>,
    ) {
        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: render::gpu_label("scene frame"),
            });

        if let Some(timing) = timing {
            timing.write(&mut enc, render::timing::GpuTimingPoint::SceneStart);
        }
        self.encode_scene(&mut enc, view, self.has_world());

        gpu.queue.submit([enc.finish()]);
    }

    pub fn render_2d(&self, gpu: &Gpu, view: &wgpu::TextureView) {
        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: render::gpu_label("2D frame"),
            });

        self.encode_2d(&mut enc, view, self.has_world());

        gpu.queue.submit([enc.finish()]);
    }

    fn has_world(&self) -> bool {
        self.world_frame.is_some()
            && ((self.terrain_queued && self.tiles.bound().is_some())
                || self.mesh_queued
                || self.skin_queued
                || self.sea_queued
                || self.billboard_queued)
    }

    fn encode_scene(
        &self,
        enc: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        has_world: bool,
    ) {
        if has_world {
            let depth = self
                .depth
                .as_ref()
                .expect("prepare allocates the depth attachment");

            let mut colors = vec![Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear),
                    store: wgpu::StoreOp::Store,
                },
            })];

            if let Some(state) = &self.temporal {
                for view in [&state.motion, &state.bias] {
                    colors.push(Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }));
                }
            }

            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: render::gpu_label("3D pass (SetRenderMode 1)"),
                color_attachments: &colors,
                depth_stencil_attachment: Some(depth.attachment()),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if let Some(state) = &self.temporal {
                pass.set_bind_group(4, &state.camera.binding, &[]);
            }

            if self.terrain_queued
                && let Some(tiles) = self.tiles.bound()
            {
                self.terrain.draw(&mut pass, tiles);
            }

            if self.mesh_queued {
                self.meshes.draw_passes(
                    &mut pass,
                    &self.world_textures.meshes,
                    &[MeshPass::Opaque, MeshPass::Cutout, MeshPass::Sky],
                );
            }

            if self.skin_queued {
                self.skins.draw_passes(
                    &mut pass,
                    &self.world_textures.skins,
                    &[SkinPass::Opaque, SkinPass::Highlight],
                );
            }

            if self.mesh_queued {
                self.meshes.draw_passes(
                    &mut pass,
                    &self.world_textures.meshes,
                    &[
                        MeshPass::SolidEffect,
                        MeshPass::Silhouette,
                        MeshPass::AdditiveSolid,
                        MeshPass::Reflective,
                    ],
                );
            }

            if let Some(shafts) = self.light_shafts.as_ref().filter(|shafts| shafts.ready()) {
                drop(pass);

                shafts.encode(enc, view, self.temporal.as_ref().map(|state| &state.bias));

                for attachment in colors.iter_mut().flatten() {
                    attachment.ops.load = wgpu::LoadOp::Load;
                }

                pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: render::gpu_label("transparent scene effects"),
                    color_attachments: &colors,
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

                if let Some(state) = &self.temporal {
                    pass.set_bind_group(4, &state.camera.binding, &[]);
                }
            }

            if self.skin_queued {
                self.skins.draw_passes(
                    &mut pass,
                    &self.world_textures.skins,
                    &[SkinPass::Invisible, SkinPass::EffectAlpha],
                );
            }

            if self.sea_queued {
                self.seas.draw(&mut pass, &self.sea_textures);
            }

            if self.mesh_queued {
                self.meshes.draw_passes(
                    &mut pass,
                    &self.world_textures.meshes,
                    &[MeshPass::Blended, MeshPass::Additive],
                );
            }

            if self.skin_queued {
                self.skins.draw_passes(
                    &mut pass,
                    &self.world_textures.skins,
                    &[SkinPass::Blended, SkinPass::EffectAdditive],
                );
            }

            if self.billboard_queued {
                self.billboards.draw(&mut pass, &self.billboard_arrays);
            }
        }
    }

    fn encode_2d(&self, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, has_world: bool) {
        let mut load = if has_world {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(if self.world_frame.is_some() {
                self.clear
            } else {
                CLEAR
            })
        };
        let mut previous = 0;

        if let Some(models) = &self.ui_models
            && models.prepared
            && !models.spans.is_empty()
        {
            let depth = models.depth.as_ref().expect("prepare allocates UI depth");

            for (before, range) in &models.spans {
                self.encode_sprites(enc, view, load, previous..*before);
                load = wgpu::LoadOp::Load;

                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: render::gpu_label("UI model pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(depth.attachment()),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

                models
                    .renderer
                    .draw_range(&mut pass, &models.textures, range.clone());

                previous = *before;
            }
        }

        self.encode_sprites(enc, view, load, previous..self.batcher.quad_count());
    }

    fn encode_sprites(
        &self,
        enc: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
        range: std::ops::Range<u32>,
    ) {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: render::gpu_label("2D pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        self.sprites
            .draw_range(&mut pass, &self.batcher, &self.textures, range);
    }

    pub fn batch_stats(&self) -> (u32, usize) {
        (self.batcher.quad_count(), self.batcher.batches().len())
    }

    pub fn terrain_draw_count(&self) -> usize {
        if self.terrain_queued {
            self.terrain.draw_count()
        } else {
            0
        }
    }

    pub fn mesh_draw_count(&self) -> usize {
        self.meshes.draw_count()
    }

    pub fn mesh_texture_count(&self) -> usize {
        self.world_textures.meshes.len()
    }

    pub fn skin_draw_count(&self) -> usize {
        self.skins.draw_count()
    }

    pub fn skin_palette_bytes(&self) -> u64 {
        self.skins.palette_bytes()
    }

    pub fn sea_draw_count(&self) -> usize {
        self.seas.draw_count()
    }

    pub fn billboard_draw_count(&self) -> usize {
        self.billboards.draw_count()
    }

    pub fn billboard_array_count(&self) -> usize {
        self.billboard_arrays.len()
    }

    pub fn depth(&self) -> Option<&DepthBuffer> {
        self.depth.as_ref()
    }

    pub fn texture_keys(&self) -> impl Iterator<Item = &TextureKey> {
        self.textures.keys()
    }
}

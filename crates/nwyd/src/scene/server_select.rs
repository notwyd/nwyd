use crate::frame::{PlannedQuad, SceneRenderer};
use crate::scene::login_panels::{self, LoginPanels};
use crate::scene::login_static::{Layout, LoginPresentation, SceneTime};
use crate::scene::world::{self, World, WorldFrame};
use anyhow::{Context, Result};
use assets::GameDir;
use render::device::Gpu;

#[derive(Debug)]
pub struct ServerSelect {
    pub layout: Layout,
    pub time: SceneTime,
    pub world: World,
    pub panels: Box<LoginPanels>,
}

#[derive(Debug)]
pub struct LanguageUpdate(login_panels::LanguageUpdate);

impl LanguageUpdate {
    pub fn prepare(scene: &ServerSelect, game: &GameDir, strings: &ui::Strings) -> Result<Self> {
        Ok(Self(login_panels::LanguageUpdate::prepare(
            &scene.panels,
            game,
            strings,
        )?))
    }
}

pub fn build(
    game: &GameDir,
    backbuffer: (u32, u32),
    presentation: &LoginPresentation,
    time: &SceneTime,
    captions: bool,
    variant: Option<u32>,
) -> Result<ServerSelect> {
    let panels = login_panels::build(game, backbuffer, presentation, time, captions)
        .context("building the 2D half of the scene")?;

    let mut world = world::build(
        game,
        variant.unwrap_or_else(|| time.calendar.bg_variant()),
        &time.calendar,
    )
    .context("building the background world")?;

    world.effects.spawn(Box::new(entrance_quad()));

    Ok(ServerSelect {
        layout: Layout::new(backbuffer.0, backbuffer.1),
        time: *time,
        world,
        panels: Box::new(panels),
    })
}

fn entrance_quad() -> super::timers::TimedQuad {
    super::timers::TimedQuad::new(
        super::quads::GroundQuad {
            owner: 0,
            slot: super::quads::Slot::Placed,
            texture: 93,
            pass: render::billboard::BillboardPass::Additive,
            anchor: glam::Vec3::new(10.0, 10.1, 10.0),
            base_scale: glam::Vec3::splat(f32::from_bits(0x447A00A4)),
            grow: 0.002,
            spin_period: 0,
            ordinal: 0,
            argb: 0xFFFFFFFF,
        },
        20000,
    )
}

pub fn upload(scene: &ServerSelect, gpu: &Gpu, renderer: &mut SceneRenderer) {
    renderer.upload_world(gpu, &scene.world);

    crate::scene::sun::upload(&scene.world.effects, gpu, renderer);

    login_panels::upload(&scene.panels, gpu, renderer);
}

pub fn sync(scene: &mut ServerSelect, gpu: &Gpu, renderer: &mut SceneRenderer) {
    login_panels::sync(&mut scene.panels, gpu, renderer);
}

pub fn frame_draws(scene: &mut ServerSelect) -> Vec<PlannedQuad> {
    frame_draws_with_cursor(scene, true)
}

pub(crate) fn frame_draws_with_cursor(
    scene: &mut ServerSelect,
    include_cursor: bool,
) -> Vec<PlannedQuad> {
    let mut out = Vec::new();

    append_frame_draws(scene, include_cursor, &mut out);

    out
}

pub(crate) fn append_frame_draws(
    scene: &mut ServerSelect,
    include_cursor: bool,
    out: &mut Vec<PlannedQuad>,
) {
    let backbuffer = (scene.layout.width, scene.layout.height);

    out.extend(crate::scene::sun::frame_draws(&mut scene.world, backbuffer));

    login_panels::append_frame_draws(&mut scene.panels, include_cursor, out);
}

pub fn world_frame(scene: &mut ServerSelect, backbuffer: (u32, u32)) -> WorldFrame {
    scene.world.frame(scene.time.now_ms, backbuffer)
}

impl ServerSelect {
    pub fn apply_language(&mut self, update: LanguageUpdate) {
        self.panels.apply_language(update.0);
    }

    pub fn clear_color(&self) -> wgpu::Color {
        self.world.clear_color()
    }
}

use crate::clock::{Calendar, Clock};
use anyhow::{Context, Result, bail};
use assets::GameDir;
use formats::config::{Config, CursorMode};
use formats::scene::scene_ui::{self, ControlType, SceneUi};
use formats::texture::texture_list::{self, FormatFlag, TextureList};
use formats::texture::texture_set_list::{self, TextureSetList};
use formats::texture::{self, Texture};
use render::preset::RenderMode;
use render::sprite::Quad;
use render::texture::{AlphaPolicy, ColorKey};
use std::collections::BTreeMap;

pub const SCENE_FILE: &[u8] = br"UI\SelServerScene3.bin";

pub const TEXTURE_SET_LIST: &[u8] = br"UI\UITextureSetList.txt";

pub const TEXTURE_LIST: &[u8] = br"UI\UITextureListN.bin";

pub const LOGO1_NAME_ID: u32 = 0x137;

pub const LOGO2_NAME_ID: u32 = 0x138;

pub const SPLASH_CONTROL_ID: i32 = 0x232;

pub const CURSOR_CONTROL_ID: i32 = 0;

pub const AUTHORED_WIDTH: f32 = 800.0;

pub const AUTHORED_HEIGHT: f32 = 600.0;

pub const UI_SCALE_FLOOR: f32 = 1e-5;

pub const SPLASH_DURATION_MS: u32 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct SceneTime {
    pub now_ms: u32,
    pub raw_ms: u32,
    pub calendar: Calendar,
}

impl SceneTime {
    pub fn live(clock: &Clock, calendar: &Calendar) -> SceneTime {
        SceneTime {
            now_ms: clock.game_time(),
            raw_ms: clock.tick_ms(),
            calendar: *calendar,
        }
    }

    pub const fn pinned(now_ms: u32, calendar: Calendar) -> SceneTime {
        SceneTime {
            now_ms,
            raw_ms: now_ms,
            calendar,
        }
    }

    pub fn day_quarter(&self) -> u32 {
        self.calendar.day_quarter()
    }

    pub fn bg_variant(&self) -> u32 {
        self.calendar.bg_variant()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum DrawPath {
    TlQuad,
    Sprite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Control {
    ScenePanel(u32),
    Splash,
    Cursor,
}

#[derive(Debug, Clone, PartialEq)]

pub struct Draw {
    pub control: Control,
    pub bucket: u8,
    pub texture: u32,
    pub path: DrawPath,
    pub quad: Quad,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub sx: f32,
    pub sy: f32,
}

impl Layout {
    pub fn extended_pixel_scale(width: u32) -> f32 {
        (width as f32 / 1680.0).max(1.0)
    }

    pub fn pixel_scale(&self) -> f32 {
        Self::extended_pixel_scale(self.width)
    }

    pub fn new(width: u32, height: u32) -> Layout {
        Layout {
            width,
            height,
            sx: width as f32 / AUTHORED_WIDTH,
            sy: height as f32 / AUTHORED_HEIGHT,
        }
    }

    pub fn ui_scale(&self) -> f32 {
        self.sx.min(self.sy).max(UI_SCALE_FLOOR)
    }

    pub fn virtual_extent(&self) -> (u32, u32) {
        let [_, _, w, h] = self.virtual_screen();

        (w as u32, h as u32)
    }

    pub fn virtual_screen(&self) -> [i32; 4] {
        let scale = self.ui_scale();

        [
            0,
            0,
            (self.width as f32 / scale) as i32,
            (self.height as f32 / scale) as i32,
        ]
    }

    pub fn from_config(config: &Config) -> Layout {
        let p = config.preset();

        Layout::new(p.width, p.height)
    }

    pub fn control(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x: self.sx * x,
            y: self.sy * y,
            w: w * self.sx,
            h: self.sy * h,
        }
    }

    pub fn logo_y_offset(&self) -> f32 {
        match self.width {
            1024 => 20.0,
            1280 => 40.0,
            _ => 0.0,
        }
    }

    pub fn logos_are_pinned(&self) -> bool {
        self.width == 1600
    }

    pub fn logo_positions(&self, logo1_width: f32) -> (Rect, Rect) {
        let (x1, x2, y) = if self.logos_are_pinned() {
            (self.sx * 144.0, self.sx * 400.0, self.sy * 10.0)
        } else {
            let half = self.width as f32 * 0.5;

            (
                half - logo1_width,
                half,
                self.sy * 10.0 + self.logo_y_offset(),
            )
        };

        let (w, h) = (self.sx * 256.0, self.sy * 256.0);

        (Rect { x: x1, y, w, h }, Rect { x: x2, y, w, h })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct LoginPresentation {
    pub splash: bool,
    pub cursor: CursorMode,
    pub cursor_frame: usize,
    pub mouse: Option<(f32, f32)>,
}

impl Default for LoginPresentation {
    fn default() -> LoginPresentation {
        LoginPresentation {
            splash: false,
            cursor: CursorMode::System,
            cursor_frame: 0,
            mouse: None,
        }
    }
}

impl LoginPresentation {
    pub fn from_config(config: &Config) -> LoginPresentation {
        LoginPresentation {
            cursor: config.cursor_mode(),
            ..LoginPresentation::default()
        }
    }

    fn cursor_position(&self, layout: &Layout) -> (f32, f32) {
        match self.cursor {
            CursorMode::System => (-100.0, -100.0),
            CursorMode::Engine => self
                .mouse
                .unwrap_or((layout.width as f32 * 0.5, layout.height as f32 * 0.5)),
        }
    }
}

#[derive(Debug, Clone)]

pub struct LoadedTexture {
    pub slot: u32,
    pub stored_path: String,
    pub flag: FormatFlag,
    pub alpha: AlphaPolicy,
    pub color_key: ColorKey,
    pub keyed_texels: usize,
    pub texture: Texture,
}

#[derive(Debug)]

pub struct LoginScene {
    pub layout: Layout,
    pub time: SceneTime,
    pub draws: Vec<Draw>,
    pub(crate) logo_width: Option<f32>,
    pub textures: BTreeMap<u32, LoadedTexture>,
    pub missing: Vec<u32>,
}

impl LoginScene {
    pub(super) fn resize(&mut self, size: (u32, u32)) {
        let next = Layout::new(size.0, size.1);

        let scale_x = next.sx / self.layout.sx;

        let scale_y = next.sy / self.layout.sy;

        for draw in &mut self.draws {
            draw.quad.dst_x *= scale_x;

            draw.quad.dst_y *= scale_y;

            draw.quad.dst_w *= scale_x;

            draw.quad.dst_h *= scale_y;
        }

        if let Some(width) = self.logo_width {
            let (first, second) = next.logo_positions(width * next.sx);

            for draw in &mut self.draws {
                let rect = match draw.control {
                    Control::ScenePanel(LOGO1_NAME_ID) => first,
                    Control::ScenePanel(LOGO2_NAME_ID) => second,
                    _ => continue,
                };

                draw.quad.dst_x = rect.x;

                draw.quad.dst_y = rect.y;

                draw.quad.dst_w = rect.w;

                draw.quad.dst_h = rect.h;
            }
        }

        self.layout = next;
    }

    pub(crate) fn set_now_ms(&mut self, now_ms: u32) {
        self.time.now_ms = now_ms;

        if now_ms >= SPLASH_DURATION_MS {
            self.draws.retain(|draw| draw.control != Control::Splash);
        }
    }

    pub fn referenced_slots(&self) -> Vec<u32> {
        let mut seen = Vec::new();

        for d in &self.draws {
            if !seen.contains(&d.texture) {
                seen.push(d.texture);
            }
        }

        seen
    }

    pub fn find(&self, control: Control) -> Option<&Draw> {
        self.draws.iter().find(|d| d.control == control)
    }
}

pub fn draw_list(
    layout: Layout,
    scene: &SceneUi,
    sets: &TextureSetList,
    presentation: &LoginPresentation,
    time: &SceneTime,
) -> Result<Vec<Draw>> {
    let mut panels: Vec<(u32, i32, u32, Rect)> = Vec::new();

    for record in &scene.records {
        if record.control != ControlType::Panel || !record.header.is_root_child() {
            continue;
        }

        let (Some(control_id), Some([x, y, w, h])) = (record.control_id(), record.rect()) else {
            continue;
        };

        if control_id >= -2 {
            continue;
        }

        let colour = record.fields.get(5).copied().unwrap_or(-1) as u32;

        panels.push((
            record.header.name_id,
            control_id,
            colour,
            layout.control(x as f32, y as f32, w as f32, h as f32),
        ));
    }

    if let Some(logo1_width) = panels
        .iter()
        .find(|(name, ..)| *name == LOGO1_NAME_ID)
        .map(|(.., rect)| rect.w)
    {
        let (one, two) = layout.logo_positions(logo1_width);

        for (name, _, _, rect) in &mut panels {
            match *name {
                LOGO1_NAME_ID => *rect = one,
                LOGO2_NAME_ID => *rect = two,
                _ => {}
            }
        }
    }

    panels.reverse();

    let mut draws = Vec::with_capacity(panels.len() + 2);

    for (name_id, control_id, colour, rect) in panels {
        draws.push(quad_draw(
            Control::ScenePanel(name_id),
            1,
            sets,
            set_index(control_id)?,
            0,
            rect,
            colour,
            DrawPath::TlQuad,
        )?);
    }

    if presentation.splash && time.now_ms < SPLASH_DURATION_MS {
        let w = layout.width as f32 * (AUTHORED_WIDTH / layout.width as f32);

        let h = layout.height as f32 * (AUTHORED_HEIGHT / layout.height as f32);

        draws.push(quad_draw(
            Control::Splash,
            1,
            sets,
            set_index(SPLASH_CONTROL_ID)?,
            0,
            layout.control(0.0, 0.0, w, h),
            0xFFFF_FFFF,
            DrawPath::Sprite,
        )?);
    }

    draws.push(cursor_draw(layout, sets, presentation)?);

    Ok(draws)
}

pub fn cursor_draw(
    layout: Layout,
    sets: &TextureSetList,
    presentation: &LoginPresentation,
) -> Result<Draw> {
    let (cx, cy) = presentation.cursor_position(&layout);

    let size = layout.control(0.0, 0.0, 32.0, 32.0);

    quad_draw(
        Control::Cursor,
        29,
        sets,
        set_index(CURSOR_CONTROL_ID)?,
        presentation.cursor_frame,
        Rect {
            x: cx,
            y: cy,
            w: size.w,
            h: size.h,
        },
        0xFFFF_FFFF,
        DrawPath::Sprite,
    )
}

fn set_index(control_id: i32) -> Result<usize> {
    let index = if control_id < -2 {
        -control_id
    } else {
        control_id
    };

    usize::try_from(index)
        .with_context(|| format!("control id {control_id} selects no texture set"))
}

#[allow(clippy::too_many_arguments)]

fn quad_draw(
    control: Control,
    bucket: u8,
    sets: &TextureSetList,
    set: usize,
    item: usize,
    dst: Rect,
    colour: u32,
    path: DrawPath,
) -> Result<Draw> {
    let entry = sets
        .item(set, item)
        .with_context(|| format!("{control:?}: UITextureSetList has no set {set} item {item}"))?;

    if entry.is_degenerate() {
        bail!("{control:?}: atlas set {set} item {item} has a zero-area source rectangle");
    }

    let texture = u32::try_from(entry.texture_index).with_context(|| {
        format!(
            "{control:?}: negative texture index {}",
            entry.texture_index
        )
    })?;

    Ok(Draw {
        control,
        bucket,
        texture,
        path,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: dst.x,
            dst_y: dst.y,
            dst_w: dst.w,
            dst_h: dst.h,
            src_x: entry.x as f32,
            src_y: entry.y as f32,
            src_w: entry.w as f32,
            src_h: entry.h as f32,
            color: colour,
            rotation: 0.0,
        },
    })
}

pub const MODE: RenderMode = RenderMode::SControl2d;

pub fn upload(
    scene: &LoginScene,
    gpu: &render::device::Gpu,
    renderer: &mut crate::frame::SceneRenderer,
) {
    for (slot, loaded) in &scene.textures {
        renderer.upload(
            gpu,
            crate::frame::TextureKey::Scene(*slot),
            &loaded.texture,
            loaded.alpha,
            &format!("{} (slot {slot})", loaded.stored_path),
        );
    }
}

pub fn frame_draws(scene: &LoginScene) -> Vec<crate::frame::PlannedQuad> {
    frame_draws_with_cursor(scene, true)
}

pub(crate) fn frame_draws_with_cursor(
    scene: &LoginScene,
    include_cursor: bool,
) -> Vec<crate::frame::PlannedQuad> {
    let mut out = Vec::new();

    append_frame_draws(scene, include_cursor, &mut out);

    out
}

pub(crate) fn append_frame_draws(
    scene: &LoginScene,
    include_cursor: bool,
    out: &mut Vec<crate::frame::PlannedQuad>,
) {
    out.extend(
        scene
            .draws
            .iter()
            .filter(|draw| include_cursor || draw.control != Control::Cursor)
            .map(|draw| crate::frame::PlannedQuad {
                mode: MODE,
                texture: crate::frame::TextureKey::Scene(draw.texture),
                quad: draw.quad,
                uv: render::sprite::UvRule::TexelCentre,
            }),
    );
}

pub fn build(
    game: &GameDir,
    backbuffer: (u32, u32),
    presentation: &LoginPresentation,
    time: &SceneTime,
) -> Result<LoginScene> {
    let layout = Layout::new(backbuffer.0, backbuffer.1);

    let scene = scene_ui::parse(&game.read(SCENE_FILE).context("UI/SelServerScene3.bin")?)
        .context("parsing UI/SelServerScene3.bin")?;

    let sets = texture_set_list::parse(
        &game
            .read(TEXTURE_SET_LIST)
            .context("UI/UITextureSetList.txt")?,
    )
    .context("parsing UI/UITextureSetList.txt")?;

    let list = texture_list::parse(&game.read(TEXTURE_LIST).context("UI/UITextureListN.bin")?)
        .context("parsing UI/UITextureListN.bin")?;

    let draws = draw_list(layout, &scene, &sets, presentation, time)?;

    let logo_width = scene.records.iter().find_map(|record| {
        (record.control == ControlType::Panel
            && record.header.is_root_child()
            && record.header.name_id == LOGO1_NAME_ID
            && record.control_id().is_some_and(|id| id < -2))
        .then(|| record.rect().map(|rect| rect[2] as f32))
        .flatten()
    });

    let mut textures = BTreeMap::new();

    let mut missing = Vec::new();

    for slot in draws.iter().map(|d| d.texture) {
        if textures.contains_key(&slot) || missing.contains(&slot) {
            continue;
        }

        match load_texture(game, &list, slot)? {
            Some(loaded) => {
                textures.insert(slot, loaded);
            }
            None => missing.push(slot),
        }
    }

    Ok(LoginScene {
        layout,
        time: *time,
        draws,
        logo_width,
        textures,
        missing,
    })
}

pub fn load_texture(
    game: &GameDir,
    list: &TextureList,
    slot: u32,
) -> Result<Option<LoadedTexture>> {
    let entry = list
        .get(slot as usize)
        .with_context(|| format!("UITextureListN.bin slot {slot} is empty"))?;

    let Ok(bytes) = game.read(&entry.path) else {
        return Ok(None);
    };

    let color_key = if entry.is_dds {
        ColorKey::CannotFire
    } else {
        let header = formats::texture::wyt::header(&bytes)
            .with_context(|| format!("slot {slot}: {}", entry.path_str()))?;

        ColorKey::for_wyt(entry.format_flag, header.bits_per_pixel)
    };

    let mut decoded = texture::decode(&bytes)
        .with_context(|| format!("decoding slot {slot}: {}", entry.path_str()))?;

    let keyed_texels = match color_key {
        ColorKey::Fires => render::texture::apply_color_key(&mut decoded),
        ColorKey::CannotFire | ColorKey::Unresolved => 0,
    };

    Ok(Some(LoadedTexture {
        slot,
        stored_path: entry.path_str(),
        flag: entry.format_flag,
        alpha: AlphaPolicy::for_flag(entry.format_flag),
        color_key,
        keyed_texels,
        texture: decoded,
    }))
}

#[cfg(test)]

mod tests {

    use super::*;

    fn draw(control: Control, rect: [f32; 4]) -> Draw {
        Draw {
            control,
            bucket: 1,
            texture: 7,
            path: DrawPath::TlQuad,
            quad: Quad {
                normalized_corners: None,
                clip: None,
                dst_x: rect[0],
                dst_y: rect[1],
                dst_w: rect[2],
                dst_h: rect[3],
                src_x: 3.0,
                src_y: 4.0,
                src_w: 100.0,
                src_h: 60.0,
                color: 0xFFFF_FFFF,
                rotation: 0.0,
            },
        }
    }

    #[test]

    fn resizing_login_controls_updates_viewport_and_preserves_texture_coordinates() {
        let mut scene = crate::scene::Scene::LoginStatic(LoginScene {
            layout: Layout::new(800, 600),
            time: SceneTime::pinned(100, Calendar::GOLDEN),
            draws: vec![draw(Control::ScenePanel(1), [10.0, 20.0, 30.0, 40.0])],
            logo_width: None,
            textures: BTreeMap::new(),
            missing: vec![7],
        });

        scene.resize(&crate::fonts::Chains::default(), (1600, 900));

        assert_eq!(scene.viewport(), (1600, 900));

        let crate::scene::Scene::LoginStatic(scene) = scene else {
            unreachable!()
        };

        let quad = scene.draws[0].quad;

        assert_eq!(
            [quad.dst_x, quad.dst_y, quad.dst_w, quad.dst_h],
            [20.0, 30.0, 60.0, 60.0]
        );

        assert_eq!(
            [quad.src_x, quad.src_y, quad.src_w, quad.src_h],
            [3.0, 4.0, 100.0, 60.0]
        );

        assert_eq!(scene.time.now_ms, 100);

        assert_eq!(scene.missing, [7]);
    }

    #[test]

    fn resizing_logos_obeys_resolution_offsets_without_restoring_expired_splash() {
        let mut scene = LoginScene {
            layout: Layout::new(800, 600),
            time: SceneTime::pinned(0, Calendar::GOLDEN),
            draws: vec![
                draw(
                    Control::ScenePanel(LOGO1_NAME_ID),
                    [80.0, 10.0, 256.0, 256.0],
                ),
                draw(
                    Control::ScenePanel(LOGO2_NAME_ID),
                    [400.0, 10.0, 256.0, 256.0],
                ),
                draw(Control::Splash, [0.0, 0.0, 800.0, 600.0]),
            ],
            logo_width: Some(320.0),
            textures: BTreeMap::new(),
            missing: Vec::new(),
        };

        for (size, first, second) in [
            (
                (1600, 900),
                [288.0, 15.0, 512.0, 384.0],
                [800.0, 15.0, 512.0, 384.0],
            ),
            (
                (1024, 768),
                [102.4, 32.8, 327.68, 327.68],
                [512.0, 32.8, 327.68, 327.68],
            ),
            (
                (1280, 720),
                [128.0, 52.0, 409.6, 307.2],
                [640.0, 52.0, 409.6, 307.2],
            ),
            (
                (800, 600),
                [80.0, 10.0, 256.0, 256.0],
                [400.0, 10.0, 256.0, 256.0],
            ),
        ] {
            scene.resize(size);

            for (control, expected) in [
                (Control::ScenePanel(LOGO1_NAME_ID), first),
                (Control::ScenePanel(LOGO2_NAME_ID), second),
            ] {
                let quad = scene.find(control).unwrap().quad;

                for (actual, expected) in [quad.dst_x, quad.dst_y, quad.dst_w, quad.dst_h]
                    .into_iter()
                    .zip(expected)
                {
                    assert!((actual - expected).abs() < 0.001);
                }
            }

            let splash = scene.find(Control::Splash).unwrap().quad;

            assert!((splash.dst_w - size.0 as f32).abs() < 0.001);

            assert!((splash.dst_h - size.1 as f32).abs() < 0.001);
        }

        scene.set_now_ms(SPLASH_DURATION_MS);

        scene.resize((1920, 1080));

        assert!(scene.find(Control::Splash).is_none());

        assert_eq!(scene.time.now_ms, SPLASH_DURATION_MS);
    }
}

use super::actors::Actor;
use super::field::{FieldScene, S_MODE};
use super::login_static::{Layout, Rect};
use crate::frame::{PlannedQuad, TextureKey};
use formats::data::item_list::item_type;
use glam::Vec3;
use render::camera::Matrix4;
use render::preset::RenderMode;
use render::sprite::{Quad, UvRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Display {
    pub shown: bool,
    pub blocked: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            shown: true,
            blocked: false,
        }
    }
}

impl Display {
    pub fn refresh(&mut self, at: Vec3) {
        let blocked = super::sel_info::town_at(at.x as i32, at.z as i32) == 1
            && (2604.0..=2651.0).contains(&at.x)
            && (1708.0..=1744.0).contains(&at.z);

        if blocked != self.blocked {
            self.blocked = blocked;

            self.shown = !blocked;
        }
    }

    pub fn toggle(&mut self) {
        if !self.blocked {
            self.shown = !self.shown;
        }
    }

    fn admits(self, actor: &Actor, hovered: Option<u16>, clicked: Option<u16>) -> bool {
        !self.blocked
            && (self.shown
                || hovered == Some(actor.entity)
                || clicked == Some(actor.entity) && !actor.is_dead)
    }
}

pub const NAME_RAMP: [[u8; 4]; 9] = [
    [0x00, 0x00, 0x00, 0xFF],
    [0x00, 0x00, 0xFF, 0xFF],
    [0x66, 0xAA, 0xFF, 0xFF],
    [0x00, 0xFF, 0xFF, 0xFF],
    [0x99, 0xFF, 0xFF, 0xFF],
    [0x99, 0xFF, 0x99, 0xFF],
    [0xFF, 0xFF, 0x99, 0xFF],
    [0xFF, 0xFF, 0xFF, 0xFF],
    [0xFF, 0xF6, 0xFF, 0xFF],
];

pub const OUTLINE_BELOW: u8 = 10;

pub const NAME_LEVEL_CLAMP: u8 = 0x96;

pub const RAMP_STEP: u8 = 20;

pub const RAMP_BIAS: u8 = 5;

pub fn name_colour(name_level: u8) -> u32 {
    if name_level < OUTLINE_BELOW {
        return 0xFF00_0000;
    }

    let level = name_level.min(NAME_LEVEL_CLAMP);

    let row = usize::from((level - RAMP_BIAS) / RAMP_STEP);

    let blend = f32::from((level - RAMP_BIAS) % RAMP_STEP) / f32::from(RAMP_STEP);

    let keep = 1.0 - f64::from(blend);

    let lerp = |channel: usize| {
        let lo = f64::from(NAME_RAMP[row][channel]);

        let hi = f64::from(NAME_RAMP[row + 1][channel]);

        (lo * keep + hi * f64::from(blend)).trunc() as u32
    };

    0xFF00_0000 | (lerp(2) << 16) | (lerp(1) << 8) | lerp(0)
}

pub fn animated_name_colour(name_level: u8, base: u32, now_ms: u32) -> u32 {
    if name_level != 0 {
        return base;
    }

    let phase = (f64::from(now_ms % 2000) * f64::from(std::f32::consts::PI) / 2000.0) as f32;

    let red = (f64::from(phase.sin()) * 255.0).trunc() as u32;

    0xFF00_0000 | (red << 16)
}

pub const HP_FILL_BUILT: u32 = 0xFF9E_F048;

pub const GAUGE_HOSTILE: u32 = 0xFFFF_0000;

pub const GAUGE_FRIENDLY: u32 = 0xFF1E_821E;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct GaugeLocal {
    pub same_body: bool,
    pub kingdom: u8,
    pub guild: u16,
}

pub fn gauge_fill(
    actor: &Actor,
    local: Option<GaugeLocal>,
    allied_guild: u16,
    war_state: u32,
    pk: bool,
) -> u32 {
    let mut color = GAUGE_HOSTILE;

    let Some(local) = local else {
        return color;
    };

    let same_positive_kingdom =
        (local.kingdom as i8) > 0 && (actor.kingdom as i8) > 0 && local.kingdom == actor.kingdom;

    if same_positive_kingdom && (war_state != 0 || actor.entity >= 1000 && !pk) {
        color = GAUGE_FRIENDLY;
    }

    if war_state == 0 && target_red_zone(actor.position) {
        color = GAUGE_HOSTILE;
    }

    if local.same_body
        || actor.tracked
        || actor.guild != 0 && (actor.guild == local.guild || actor.guild == allied_guild)
    {
        color = GAUGE_FRIENDLY;
    }

    color
}

pub const GAUGE_BACK: u32 = 0xFF33_3333;

pub const MOUNT_FILL: u32 = 0xFFFF_AA00;

pub const GAUGE_WIDTH: f32 = 72.0;

pub const GAUGE_HALF_WIDTH: f32 = 35.0;

pub const GAUGE_HEIGHT_SCALED: f32 = 2.0;

pub const HP_HEIGHT_FIXED: f32 = 6.0;

pub const HP_RISE: f32 = 10.0;

pub const HP_MOUNTED_RISE: f32 = 15.0;

pub const MOUNT_HEIGHT_FIXED: f32 = 5.0;

pub const MOUNT_RISE: f32 = 16.0;

pub const MOUNT_RISE_FIXED: f32 = 14.0;

pub const GAUGE_FILL_SHRINK: f32 = 4.0;

pub fn anchor(actor: &Actor) -> Vec3 {
    let height = super::pick::PICK_VOLUME
        .get(actor.model_id as usize)
        .map_or(0.0, |volume| volume.1);

    Vec3::new(
        actor.position.x,
        height * actor.presentation_scale() + actor.position.y + HEAD_LIFT,
        actor.position.z,
    )
}

pub const HEAD_LIFT: f32 = 0.2;

pub fn project(
    view: &Matrix4,
    projection: &Matrix4,
    at: Vec3,
    layout: &Layout,
) -> Option<(f32, f32)> {
    let ndc = projection.transform_coord(view.transform_coord(at));

    if !(0.0..1.0).contains(&ndc.z) {
        return None;
    }

    let (w, h) = (layout.width as f32, layout.height as f32);

    let sx = (ndc.x + 1.0) * 0.5 * w;

    let sy = (1.0 - ndc.y) * 0.5 * h;

    let (ix, iy) = (sx.trunc(), sy.trunc());

    (ix >= 1.0 && ix < w && iy >= 1.0 && iy < h).then_some((sx, sy))
}

pub fn project_actor(
    view: &Matrix4,
    projection: &Matrix4,
    actor: &Actor,
    layout: &Layout,
    camera_distance: f32,
) -> Option<(f32, f32)> {
    project_body_ndc(
        projection.transform_coord(view.transform_coord(anchor(actor))),
        layout,
        actor.mounted,
        camera_distance,
    )
}

fn project_body_ndc(
    ndc: Vec3,
    layout: &Layout,
    mounted: bool,
    camera_distance: f32,
) -> Option<(f32, f32)> {
    if !(0.0..1.0).contains(&ndc.z) {
        return None;
    }

    let (w, h) = (f64::from(layout.width), f64::from(layout.height));

    let pixels = f64::from(layout.pixel_scale());

    let x = ((f64::from(ndc.x) + 1.0) * w * 0.5).trunc();

    let c = if mounted { 0.76 } else { 1.16 };

    let projected_y = ((c - f64::from(ndc.y)) * (h - pixels) * -0.5).trunc();

    let camera_y = if mounted {
        (f64::from(camera_distance) * 3.5).trunc()
    } else {
        -3.0 * f64::from(camera_distance).trunc()
    };

    let raw_y = camera_y * pixels - projected_y;

    if x < 1.0 || x >= w || raw_y < 1.0 || raw_y >= h {
        return None;
    }

    let sy = f64::from(layout.sy);

    let mut y = (raw_y + 13.0 * sy).trunc();

    y += (if mounted { 8.0 * sy } else { -32.0 * sy }).trunc();

    if sy / pixels == 1.0 {
        y += if mounted {
            -8.0 * pixels
        } else {
            10.0 * pixels
        };
    } else if sy / pixels >= 1.7 {
        y += (if mounted { 15.0 * sy } else { -6.0 * sy }).trunc();
    }

    Some((x as f32, y as f32))
}

fn caption_origin(at: (f32, f32), layout: &Layout) -> (f32, f32) {
    super::stext::origin(
        layout,
        at,
        Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 16.0 * layout.pixel_scale(),
        },
    )
}

pub fn wanted(
    actor: &Actor,
    hovered: Option<u16>,
    pursuing: Option<u16>,
    stuck: Option<u16>,
    clicked: Option<u16>,
) -> bool {
    let names = |id: Option<u16>| id.is_some_and(|id| id == actor.entity);

    names(hovered)
        || names(pursuing)
        || names(stuck)
        || actor.entity != 0 && actor.entity < PLAYER_ID_LIMIT
        || super::cc::npc_body(actor)
        || actor.tracked
        || actor.guild != 0
        || actor.body == NPC_GRADED_BODY && actor.status.flags & 0xF != 0
        || names(clicked) && !actor.is_dead
}

pub const NPC_GRADED_BODY: i32 = 0x10F;

pub use formats::net::spawn::PLAYER_ID_LIMIT;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Plate {
    pub name: (f32, f32),
    pub hp: [f32; 4],
    pub mount: [f32; 4],
    pub guild: (f32, f32),
    pub mark: (f32, f32),
}

struct Placed {
    entity: u16,
    name: String,
    zero_kind: u32,
    name_scale: f32,
    name_anchor: (f32, f32),
    guild: String,
    name_color: u32,
    pulse: bool,
    backdrop: Option<u32>,
    hp_shown: bool,
    fill: f32,
    fill_color: u32,
    mount_fill: Option<f32>,
    guild_id: u16,
    guild_rank: u16,
    plate: Plate,
}

pub(crate) fn displayed_name(name: &str) -> &str {
    name.split('\0')
        .next()
        .unwrap_or_default()
        .split('^')
        .next()
        .unwrap_or_default()
}

pub(crate) fn tagged_name(name: &str) -> bool {
    name.split('\0').next().unwrap_or_default().contains('^')
}

pub(crate) fn refresh_backdrop(actor: &mut Actor) {
    if tagged_name(&actor.name) {
        actor.green_name = true;

        return;
    }

    actor.plate_name_color = name_colour(actor.name_level);

    if actor.name_level < 10 {
        actor.plate_outline = true;
    } else if actor.entity < 1000 {
        actor.plate_outline = false;
    }

    if !actor.plate_outline {
        actor.green_name = false;
    }
}

pub const GUILD_RISE: f32 = 28.0;

pub const GUILD_COLOUR: u32 = 0xFFCC_CCCC;

pub const GUILD_COLOUR_BUILT: u32 = 0xFFFF_FFAA;

pub fn place(
    at: (f32, f32),
    layout: &Layout,
    name_len: usize,
    guild_len: usize,
    mounted: bool,
) -> Plate {
    place_with_shop(at, layout, name_len, guild_len, mounted, false)
}

fn place_with_shop(
    at: (f32, f32),
    layout: &Layout,
    name_len: usize,
    guild_len: usize,
    mounted: bool,
    shop_shown: bool,
) -> Plate {
    let (sx, sy) = (layout.sx, layout.sy);

    let (px, py) = at;

    let pixels = layout.pixel_scale();

    let hp_y = py
        - sy * HP_RISE
        - if mounted {
            HP_MOUNTED_RISE * pixels
        } else {
            0.0
        };

    let name_y = py
        - if mounted && !shop_shown {
            20.0 * pixels
        } else {
            0.0
        };

    Plate {
        name: (
            px - ((name_len as i32 * 3 + 6) * 2) as f32 * sx * 0.5,
            name_y,
        ),
        hp: [
            px - sx * GAUGE_HALF_WIDTH,
            hp_y,
            sx * GAUGE_WIDTH,
            sy * GAUGE_HEIGHT_SCALED + HP_HEIGHT_FIXED * pixels,
        ],
        mount: [
            px - sx * GAUGE_HALF_WIDTH,
            py - sy * MOUNT_RISE - MOUNT_RISE_FIXED * pixels,
            sx * GAUGE_WIDTH,
            sy * GAUGE_HEIGHT_SCALED + MOUNT_HEIGHT_FIXED * pixels,
        ],
        guild: (
            px - ((guild_len as i32 * 3 + 6) * 2) as f32 * sx * 0.5,
            name_y - GUILD_RISE * sx,
        ),
        mark: (
            px - ((name_len as i32 * 3 + 6) * 2) as f32 * sx * 0.5 - MARK_DX * pixels,
            name_y + MARK_DY * pixels,
        ),
    }
}

pub const MARK_DX: f32 = 10.0;

pub const MARK_DY: f32 = 2.0;

pub const MARK_SIZE: (f32, f32) = (16.0, 12.0);

pub const FRAME_SIZE: (f32, f32) = (20.0, 16.0);

pub const FRAME_SIZE_SCALED: (f32, f32) = (18.5, 14.5);

pub const FRAME_INSET: f32 = 2.0;

pub fn mark_rects(at: (f32, f32), layout: &Layout) -> ([f32; 4], [f32; 4]) {
    let (sx, sy) = (layout.sx, layout.sy);

    let pixels = layout.pixel_scale();

    let (mut x, mut y) = (at.0 - FRAME_INSET * pixels, at.1 - FRAME_INSET * pixels);

    let (fw, fh) = match sx == 1.0 {
        true => FRAME_SIZE,
        false => (sx * FRAME_SIZE_SCALED.0, sy * FRAME_SIZE_SCALED.1),
    };

    if sy == 0.8 {
        y += 1.0;
    } else if sy != 1.0 && (sx == 1.6 || sx == 2.0) {
        x -= 1.0;
    }

    let (dx, dy) = (
        sx * MARK_SIZE.0 - MARK_SIZE.0 * pixels,
        sy * MARK_SIZE.1 - MARK_SIZE.1 * pixels,
    );

    (
        [x - dx, y - dy, fw, fh],
        [at.0 - dx, at.1 - dy, sx * MARK_SIZE.0, sy * MARK_SIZE.1],
    )
}

pub fn mount_gauge_shown(
    local: bool,
    mounted: bool,
    mount_kind: i16,
    mount_item_type: i16,
) -> bool {
    local && mounted && !matches!(mount_kind, 0x1B..=0x1E) && mount_item_type != item_type::MOUNT
}

pub const SIGN_SET: usize = 0x200;

pub const SIGN_SIZE: (f32, f32) = (141.0, 43.0);

pub const CAPTION_SIZE: (f32, f32) = (143.0, 50.0);

pub const SIGN_COLOR: u32 = 0xFFFF_FFFF;

pub const CAPTION_COLOR: u32 = 0xFFFF_FFFF;

pub const SIGN_HALF_WIDTH: f32 = 150.0 * 0.5;

pub const SIGN_RISE: f32 = 3.0;

pub fn signboard(at: (f32, f32), layout: &Layout) -> (Rect, Rect) {
    let (sx, sy) = (layout.sx, layout.sy);

    let (x, y) = (at.0 - sx * SIGN_HALF_WIDTH, at.1 - sy * SIGN_RISE);

    (
        Rect {
            x,
            y,
            w: SIGN_SIZE.0 * sx,
            h: SIGN_SIZE.1 * sy,
        },
        Rect {
            x,
            y,
            w: CAPTION_SIZE.0 * sx,
            h: CAPTION_SIZE.1 * sy,
        },
    )
}

pub const SIGN_KEY: u32 = 0x2000_0000;

pub fn fill_width(width: f32, current: u32, max: u32) -> f32 {
    let current = current.min(max);

    let max = max.max(1);

    width * current as f32 / max as f32
}

pub fn hp_pair(actor: &Actor) -> (u32, u32) {
    match actor.alternate_hp.filter(|hp| hp.active()) {
        Some(hp) => (hp.current, hp.max),
        None => (actor.status.hp, actor.status.max_hp),
    }
}

pub const TARGET_HP_RED: u32 = 0xFFFF_0000;

pub const TARGET_HP_GREEN: u32 = 0xFF00_6400;

fn target_hidden(actor: &Actor, viewer: Option<u16>) -> bool {
    super::actors::ActorScene::hidden(actor, viewer)
}

fn target_red_zone(position: Vec3) -> bool {
    let x = position.x.trunc() as i32;

    let z = position.z.trunc() as i32;

    matches!(
        (x >> 7, z & !0x7F),
        (8, 0x680 | 0x780 | 0x800) | (9, 0x780 | 0x800)
    )
}

fn target_reset_appearance(actor: &Actor) -> bool {
    matches!(actor.class, 1 | 2 | 4 | 8 | 0x1A)
        || actor.class == 0x21 && actor.parts[0].mesh == 0
        || actor.body as i16 == NPC_GRADED_BODY as i16 && actor.status.flags & 0xF != 0
}

fn target_reset_before_hover(actor: &Actor) -> bool {
    let x = actor.position.x.trunc() as i32;

    let z = actor.position.z.trunc() as i32;

    if (2..=10).contains(&(x >> 7)) && (z & !0x7F) < 0x280 {
        return true;
    }

    if !target_reset_appearance(actor) {
        return false;
    }

    let cell = |value: f32| (f64::from(value) * (1.0 / 128.0)).trunc() as i32;

    actor.body as i16 != 0x33 || cell(actor.position.x) != 0x0E || cell(actor.position.z) != 0x1C
}

fn hp_gauge_shown(actor: &Actor, local: Option<bool>, hovered: Option<u16>, safe: bool) -> bool {
    let special = actor.class == 0x38 && actor.parts[0].mesh == 0;

    let mut shown = !special;

    let (x, z) = super::attack::cell_of(actor);

    if (2..=10).contains(&(x >> 7)) && (z & !0x7F) < 0x280 {
        shown = true;
    } else if target_reset_appearance(actor) {
        shown = !target_reset_before_hover(actor);
    }

    if local.is_some()
        && !super::cc::npc_body(actor)
        && actor.body as i16 != 0x39
        && actor.shop_title.is_empty()
    {
        shown = !(local == Some(false) && hovered == Some(actor.entity) && special);
    }

    shown && !(safe && actor.class == 0x10) && actor.shop_title.is_empty()
}

fn target_arena_clears(actor: &Actor, _arena_control: &super::arena::Board) -> bool {
    let x = actor.position.x.trunc() as i32;

    let z = actor.position.z.trunc() as i32;

    (x & !0x7F) == 0x80 && (z & !0x7F) == 0x200 && !actor.tracked
}

fn apply_target_body_return(
    actor: &mut Actor,
    hovered: Option<u16>,
    pursuing: Option<u16>,
    stuck: Option<u16>,
    clicked: Option<u16>,
    has_local: bool,
) -> bool {
    let entity = actor.entity;

    let points_at = |id: Option<u16>| id.is_some_and(|id| id == entity);

    let returns = !points_at(hovered)
        && !points_at(pursuing)
        && !points_at(stuck)
        && matches!(actor.body as i16, 0xD8 | 0xE2 | 0x12A);

    if returns {
        actor.target_hud.eligible = has_local && points_at(clicked) && !actor.is_dead;
    }

    returns
}

fn target_eligible(
    actor: &Actor,
    local: Option<bool>,
    hovered: Option<u16>,
    _pursuing: Option<u16>,
    _stuck: Option<u16>,
    clicked: Option<u16>,
    placed: bool,
    hidden: bool,
) -> bool {
    if hidden {
        return false;
    }

    let points_at = |id: Option<u16>| id.is_some_and(|id| id == actor.entity);

    let clicked = local.is_some() && points_at(clicked) && !actor.is_dead;

    let special = actor.class == 0x38 && actor.parts[0].mesh == 0;

    if !placed {
        return special && !actor.is_dead;
    }

    let mut eligible = clicked || special;

    if target_reset_before_hover(actor) {
        eligible = false;
    }

    if local == Some(false)
        && points_at(hovered)
        && !super::cc::npc_body(actor)
        && actor.body as i16 != 0x39
        && actor.shop_title.is_empty()
    {
        eligible = true;
    }

    eligible
}

fn target_hp_color(
    previous: u32,
    actor: &Actor,
    local: Option<(bool, u16)>,
    allied_guild: u16,
    war_state: u32,
) -> u32 {
    let mut color = previous;

    if actor.body as i16 > 0x28 {
        color = TARGET_HP_RED;
    }

    if local.is_some() && war_state == 0 && target_red_zone(actor.position) {
        color = TARGET_HP_RED;
    }

    if let Some((is_local, local_guild)) = local {
        if is_local
            || actor.tracked
            || actor.guild != 0 && (actor.guild == local_guild || actor.guild == allied_guild)
        {
            color = TARGET_HP_GREEN;
        }
    }

    color
}

pub fn consumes_gauge_mouse(scene: &FieldScene, event: ui::input::Mouse, x: i32, y: i32) -> bool {
    use ui::input::Mouse;

    if !matches!(
        event,
        Mouse::Move | Mouse::LeftDown | Mouse::LeftUp | Mouse::RightDown
    ) {
        return false;
    }

    let Some((view, projection)) = scene.projected else {
        return false;
    };

    let air_move = super::field::air_move_active(scene);

    let mount_item_type = super::mount_hp::worn_item_type(&scene.statics, &scene.entered.equipment);

    scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .any(|(index, actor)| {
            if super::actors::ActorScene::hidden(actor, scene.world.actors.viewer)
                || !scene
                    .name_display
                    .admits(actor, scene.hovered, scene.attack.clicked)
                || air_move && Some(index) == scene.player
                || !wanted(
                    actor,
                    scene.hovered,
                    scene.cc.target,
                    scene.attack.stuck,
                    scene.attack.clicked,
                )
            {
                return false;
            }

            let Some(at) =
                project_actor(&view, &projection, actor, &scene.layout, scene.camera.dist)
            else {
                return false;
            };

            let plate = place_with_shop(
                at,
                &scene.layout,
                displayed_name(&actor.name).chars().count(),
                actor.guild_name.chars().count(),
                actor.mounted,
                !actor.shop_title.is_empty(),
            );

            let inside = |r: [f32; 4]| {
                x as f32 >= r[0]
                    && y as f32 >= r[1]
                    && (x as f32) < r[0] + r[2]
                    && (y as f32) < r[1] + r[3]
            };

            (hp_gauge_shown(
                actor,
                scene.player.map(|player| player == index),
                scene.hovered,
                super::attack::safe_here(
                    scene.world.actors.attributes(),
                    super::attack::cell_of(actor),
                ),
            ) && inside(plate.hp))
                || (mount_gauge_shown(
                    Some(index) == scene.player,
                    actor.mounted,
                    actor.mount_kind,
                    mount_item_type,
                ) && inside(plate.mount))
        })
}

#[derive(Debug, Default)]

pub(crate) struct NameHover {
    controls: std::collections::BTreeMap<u64, NameInput>,
}

#[derive(Debug, Default)]

struct NameInput {
    rect: [f32; 4],
    shown: bool,
    hit: bool,
}

fn scale_name_rect(rect: [f32; 4], anchor: (f32, f32), scale: f32) -> [f32; 4] {
    if scale == 1.0 {
        return rect;
    }

    [
        anchor.0 + (rect[0] - anchor.0) * scale,
        anchor.1 + (rect[1] - anchor.1) * scale,
        rect[2] * scale,
        rect[3] * scale,
    ]
}

fn scale_name_quad(mut quad: Quad, anchor: (f32, f32), scale: f32) -> Quad {
    [quad.dst_x, quad.dst_y, quad.dst_w, quad.dst_h] = scale_name_rect(
        [quad.dst_x, quad.dst_y, quad.dst_w, quad.dst_h],
        anchor,
        scale,
    );

    quad
}

impl NameHover {
    pub(crate) fn contains(&self, actor: &Actor) -> bool {
        self.controls
            .get(&actor.incarnation)
            .is_some_and(|control| control.hit)
    }

    pub(crate) fn prune(&mut self, actors: &[Actor]) {
        self.controls
            .retain(|key, _| actors.iter().any(|actor| actor.incarnation == *key));
    }

    fn place(&mut self, actor: &Actor, at: (f32, f32), anchor: (f32, f32), layout: &Layout) {
        let control = self.controls.entry(actor.incarnation).or_default();

        let len = displayed_name(&actor.name).chars().count();

        control.rect = scale_name_rect(
            name_control_rect(at, len, layout),
            anchor,
            actor.evok_factor(),
        );

        control.shown = !actor.is_dead;
    }
}

fn name_control_rect(at: (f32, f32), len: usize, layout: &Layout) -> [f32; 4] {
    let pixels = layout.pixel_scale();

    [
        at.0,
        at.1,
        (len as f32 * 6.0 + 18.0) * pixels,
        16.0 * pixels,
    ]
}

pub(crate) fn mouse_names(scene: &mut FieldScene, x: i32, y: i32) {
    scene.name_hover.prune(&scene.world.actors.actors);

    for control in scene
        .name_hover
        .controls
        .values_mut()
        .filter(|control| control.shown)
    {
        let [left, top, width, height] = control.rect;

        control.hit = x as f32 >= left
            && y as f32 >= top
            && (x as f32) < left + width
            && (y as f32) < top + height;
    }
}

pub fn consumes_badge_mouse(scene: &FieldScene, event: ui::input::Mouse, x: i32, y: i32) -> bool {
    use ui::input::Mouse;

    if !matches!(
        event,
        Mouse::Move | Mouse::LeftDown | Mouse::LeftUp | Mouse::RightDown
    ) {
        return false;
    }

    let Some((view, projection)) = scene.projected else {
        return false;
    };

    let air_move = super::field::air_move_active(scene);

    let rect = scene.layout.control(0.0, 0.0, 12.0, 16.0);

    scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .any(|(index, actor)| {
            if actor.guild == 0
                || !actor.shop_title.is_empty()
                || !scene
                    .name_display
                    .admits(actor, scene.hovered, scene.attack.clicked)
                || super::actors::ActorScene::hidden(actor, scene.world.actors.viewer)
                || air_move && Some(index) == scene.player
                || !wanted(
                    actor,
                    scene.hovered,
                    scene.cc.target,
                    scene.attack.stuck,
                    scene.attack.clicked,
                )
            {
                return false;
            }

            let Some(at) =
                project_actor(&view, &projection, actor, &scene.layout, scene.camera.dist)
            else {
                return false;
            };

            let plate = place_with_shop(
                at,
                &scene.layout,
                displayed_name(&actor.name).chars().count(),
                actor.guild_name.chars().count(),
                actor.mounted,
                !actor.shop_title.is_empty(),
            );

            x as f32 >= plate.mark.0
                && y as f32 >= plate.mark.1
                && (x as f32) < plate.mark.0 + rect.w
                && (y as f32) < plate.mark.1 + rect.h
        })
}

pub fn frame_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    let Some((view, projection)) = scene.projected else {
        return Vec::new();
    };

    let layout = scene.layout;

    let captions = scene.captions;

    let now_ms = scene.time.now_ms;

    let hovered = scene.hovered;

    let (pursuing, stuck, clicked) = (scene.cc.target, scene.attack.stuck, scene.attack.clicked);

    let player = scene.player;

    let air_move = super::field::air_move_active(scene);

    let viewer = scene.world.actors.viewer;

    let (mount_hp, mount_max) = super::mount_hp::pair(scene);

    let mount_item_type = super::mount_hp::worn_item_type(&scene.statics, &scene.entered.equipment);

    let local_relation = player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map(|actor| (actor.kingdom, actor.guild));

    let allied_guild = scene.siege.ally;

    let war_state = scene.siege.war_state;

    let pk = scene.pk;

    for (index, actor) in scene.world.actors.actors.iter_mut().enumerate() {
        let hidden = target_hidden(actor, viewer);

        let local = local_relation.map(|(kingdom, guild)| GaugeLocal {
            same_body: Some(index) == player,
            kingdom,
            guild,
        });

        if !hidden
            && apply_target_body_return(actor, hovered, pursuing, stuck, clicked, local.is_some())
        {
            continue;
        }

        let placed = !hidden
            && wanted(actor, hovered, pursuing, stuck, clicked)
            && project_actor(&view, &projection, actor, &layout, scene.camera.dist).is_some();

        let eligible = target_eligible(
            actor,
            local.map(|local| local.same_body),
            hovered,
            pursuing,
            stuck,
            clicked,
            placed,
            hidden,
        );

        let hp_color = match placed {
            true => target_hp_color(
                actor.target_hud.hp_color,
                actor,
                local.map(|local| (local.same_body, local.guild)),
                allied_guild,
                war_state,
            ),
            false => actor.target_hud.hp_color,
        };

        actor.target_hud.eligible = eligible && !target_arena_clears(actor, &scene.arena);

        actor.target_hud.hp_color = hp_color;
    }

    scene.name_hover.prune(&scene.world.actors.actors);

    for control in scene.name_hover.controls.values_mut() {
        control.shown = false;
    }

    let plates: Vec<Placed> = scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .filter(|(_, actor)| {
            actor.camera_visible && !super::actors::ActorScene::hidden(actor, viewer)
        })
        .filter(|(index, _)| !(air_move && Some(*index) == player))
        .filter(|(_, actor)| wanted(actor, hovered, pursuing, stuck, clicked))
        .filter(|(_, actor)| scene.name_display.admits(actor, hovered, clicked))
        .filter_map(|(index, actor)| {
            let at = project_actor(&view, &projection, actor, &layout, scene.camera.dist)?;

            let plate = place_with_shop(
                at,
                &layout,
                displayed_name(&actor.name).chars().count(),
                actor.guild_name.chars().count(),
                actor.mounted,
                !actor.shop_title.is_empty(),
            );

            let (hp, max_hp) = hp_pair(actor);

            scene.name_hover.place(actor, plate.name, at, &layout);

            Some(Placed {
                zero_kind: u32::from(!actor.name.contains('^')),
                entity: actor.entity,
                name: displayed_name(&actor.name).to_owned(),
                name_scale: actor.evok_factor(),
                name_anchor: at,
                guild: actor.guild_name.clone(),
                name_color: animated_name_colour(
                    actor.name_level,
                    if tagged_name(&actor.name) {
                        actor.plate_name_color
                    } else {
                        name_colour(actor.name_level)
                    },
                    now_ms,
                ),
                pulse: actor.name_level == 0,
                backdrop: actor.plate_outline.then_some(actor.plate_backdrop_color),
                hp_shown: hp_gauge_shown(
                    actor,
                    local_relation.map(|_| Some(index) == player),
                    hovered,
                    super::attack::safe_here(
                        scene.world.actors.attributes(),
                        super::attack::cell_of(actor),
                    ),
                ),
                fill: fill_width(plate.hp[2], hp, max_hp),
                fill_color: gauge_fill(
                    actor,
                    local_relation.map(|(kingdom, guild)| GaugeLocal {
                        same_body: Some(index) == player,
                        kingdom,
                        guild,
                    }),
                    allied_guild,
                    war_state,
                    pk,
                ),
                mount_fill: mount_gauge_shown(
                    Some(index) == player,
                    actor.mounted,
                    actor.mount_kind,
                    mount_item_type,
                )
                .then(|| {
                    fill_width(
                        plate.mount[2],
                        mount_hp.max(0) as u32,
                        mount_max.max(0) as u32,
                    )
                }),
                guild_id: if actor.shop_title.is_empty() {
                    actor.guild
                } else {
                    0
                },
                guild_rank: actor.guild_rank,
                plate,
            })
        })
        .collect();

    let speaking: Vec<(u16, super::balloon::Balloon, (f32, f32))> = match captions {
        false => Vec::new(),
        true => scene
            .world
            .actors
            .actors
            .iter()
            .filter(|actor| !super::actors::ActorScene::hidden(actor, viewer))
            .filter_map(|actor| {
                let balloon = actor.balloon.as_ref()?;

                if !super::balloon::visible(balloon, now_ms) {
                    return None;
                }

                let at = project_actor(&view, &projection, actor, &layout, scene.camera.dist)?;

                Some((actor.entity, balloon.clone(), at))
            })
            .collect(),
    };

    let selling: Vec<(u16, String, (f32, f32))> = scene
        .world
        .actors
        .actors
        .iter()
        .filter(|actor| actor.camera_visible && !super::actors::ActorScene::hidden(actor, viewer))
        .filter(|actor| scene.name_display.admits(actor, hovered, clicked))
        .filter(|actor| !actor.shop_title.is_empty())
        .filter_map(|actor| {
            let at = project_actor(&view, &projection, actor, &layout, scene.camera.dist)?;

            Some((actor.entity, actor.shop_title.clone(), at))
        })
        .collect();

    let mut out = Vec::with_capacity(plates.len() * 8 + speaking.len() * 3 + selling.len() * 3);

    for (entity, title, at) in selling {
        let (plate, caption) = signboard(at, &layout);

        if let Some(item) = scene.sets.item(SIGN_SET, 0)
            && let Ok(slot) = u32::try_from(item.texture_index)
            && scene.chrome.contains_key(&slot)
        {
            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::Scene(slot),
                quad: Quad {
                    normalized_corners: None,
                    clip: None,
                    dst_x: plate.x + item.dst_offset[0] as f32,
                    dst_y: plate.y + item.dst_offset[1] as f32,
                    dst_w: plate.w,
                    dst_h: plate.h,
                    src_x: item.x as f32,
                    src_y: item.y as f32,
                    src_w: item.w as f32,
                    src_h: item.h as f32,
                    color: SIGN_COLOR,
                    rotation: 0.0,
                },
                uv: UvRule::TexelCentre,
            });
        }

        if !captions {
            continue;
        }

        let key = SIGN_KEY | u32::from(entity);

        let title = super::stext::display_text(&title);

        if scene.text.resolve(key, &title, CAPTION_COLOR)
            && let Some(line) = scene.text.get(key)
        {
            let at = super::stext::origin(&layout, (0.0, 0.0), caption);

            out.push(PlannedQuad {
                mode: RenderMode::Ui2dPoint,
                texture: TextureKey::SceneText(key),
                quad: super::stext::shadow_quad(line, at),
                uv: UvRule::TexelCentre,
            });

            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::SceneText(key),
                quad: super::stext::quad(line, at),
                uv: UvRule::TexelCentre,
            });
        }
    }

    for (entity, balloon, at) in speaking {
        out.extend(super::balloon::draws(
            &mut scene.text,
            entity,
            &balloon,
            at,
            &layout,
        ));
    }

    for Placed {
        entity,
        name,
        zero_kind,
        name_scale,
        name_anchor,
        guild,
        name_color,
        pulse,
        backdrop,
        hp_shown,
        fill,
        fill_color,
        mount_fill,
        guild_id,
        guild_rank,
        plate,
    } in plates
    {
        let (hp, name_at) = (plate.hp, plate.name);

        if let Some(mount) = mount_fill {
            let rect = plate.mount;

            out.push(gauge_quad(rect, GAUGE_BACK));

            if mount > 0.0 {
                out.push(gauge_quad(
                    [
                        rect[0],
                        rect[1] + GAUGE_FILL_SHRINK * layout.pixel_scale() * 0.5,
                        mount,
                        rect[3] - GAUGE_FILL_SHRINK * layout.pixel_scale(),
                    ],
                    MOUNT_FILL,
                ));
            }
        }

        if hp_shown {
            out.push(gauge_quad(hp, GAUGE_BACK));

            if fill > 0.0 {
                out.push(gauge_quad(
                    [
                        hp[0],
                        hp[1] + GAUGE_FILL_SHRINK * layout.pixel_scale() * 0.5,
                        fill,
                        hp[3] - GAUGE_FILL_SHRINK * layout.pixel_scale(),
                    ],
                    fill_color,
                ));
            }
        }

        if let Some(slot) = scene.marks.resolve(guild_id)
            && scene
                .marks
                .bank
                .slot(slot)
                .is_some_and(|record| record.pixels.is_some())
        {
            let (frame, badge) = mark_rects(plate.mark, &layout);

            if let Some(colour) = frame_colour(guild_rank) {
                out.push(gauge_quad(frame, colour));
            }

            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::GuildMark(slot),
                quad: Quad {
                    normalized_corners: None,
                    clip: None,
                    dst_x: badge[0],
                    dst_y: badge[1],
                    dst_w: badge[2],
                    dst_h: badge[3],
                    src_x: 0.0,
                    src_y: 0.0,
                    src_w: MARK_SIZE.0,
                    src_h: MARK_SIZE.1,
                    color: 0xFFFF_FFFF,
                    rotation: 0.0,
                },
                uv: UvRule::TexelCentre,
            });
        }

        if let Some(color) = backdrop {
            out.push(gauge_quad(
                scale_name_rect(
                    name_control_rect(name_at, name.chars().count(), &layout),
                    name_anchor,
                    name_scale,
                ),
                color,
            ));
        }

        if !captions {
            continue;
        }

        let name_at = caption_origin(name_at, &layout);

        let key = NAME_KEY | u32::from(entity);

        let name = super::stext::display_text(&name);

        if scene.text.resolve_kind(
            key,
            &name,
            if pulse { 0xFFFF_FFFF } else { name_color },
            zero_kind,
        ) && let Some(line) = scene.text.get(key)
        {
            out.push(PlannedQuad {
                mode: RenderMode::Ui2dPoint,
                texture: TextureKey::SceneText(key),
                quad: scale_name_quad(
                    super::stext::shadow_quad(line, name_at),
                    name_anchor,
                    name_scale,
                ),
                uv: UvRule::TexelCentre,
            });

            let mut quad =
                scale_name_quad(super::stext::quad(line, name_at), name_anchor, name_scale);

            if pulse {
                quad.color = name_color;
            }

            out.push(PlannedQuad {
                mode: if pulse {
                    RenderMode::NuiPanel2d
                } else {
                    S_MODE
                },
                texture: TextureKey::SceneText(key),
                quad,
                uv: UvRule::TexelCentre,
            });
        }

        let key = GUILD_KEY | u32::from(entity);

        let guild = super::stext::display_text(&guild);

        if scene
            .text
            .resolve_kind(key, &guild, GUILD_COLOUR, zero_kind)
            && let Some(line) = scene.text.get(key)
        {
            out.push(PlannedQuad {
                mode: RenderMode::Ui2dPoint,
                texture: TextureKey::SceneText(key),
                quad: super::stext::shadow_quad(line, caption_origin(plate.guild, &layout)),
                uv: UvRule::TexelCentre,
            });

            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::SceneText(key),
                quad: super::stext::quad(line, caption_origin(plate.guild, &layout)),
                uv: UvRule::TexelCentre,
            });
        }
    }

    out
}

pub const NAME_KEY: u32 = 0x8000_0000;

pub const GUILD_KEY: u32 = 0x6000_0000;

pub fn forget(scene: &mut FieldScene, entity: u16) -> bool {
    let name = scene.text.forget(NAME_KEY | u32::from(entity));

    let sign = scene.text.forget(SIGN_KEY | u32::from(entity));

    let guild = scene.text.forget(GUILD_KEY | u32::from(entity));

    name || sign || guild
}

pub fn frame_colour(guild_rank: u16) -> Option<u32> {
    use formats::data::guild_mark::{Frame, frame, kind};

    match frame(kind(guild_rank as i16)) {
        Frame::Gold => Some(formats::data::guild_mark::FRAME_GOLD),
        Frame::Silver => Some(formats::data::guild_mark::FRAME_SILVER),
        Frame::None => None,
    }
}

fn gauge_quad(rect: [f32; 4], color: u32) -> PlannedQuad {
    PlannedQuad {
        mode: RenderMode::NuiPanel2d,
        texture: TextureKey::White,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: rect[0],
            dst_y: rect[1],
            dst_w: rect[2],
            dst_h: rect[3],
            src_x: 0.0,
            src_y: 0.0,
            src_w: 1.0,
            src_h: 1.0,
            color,
            rotation: 0.0,
        },
        uv: UvRule::TexelEdge,
    }
}

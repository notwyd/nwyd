use crate::clock::Calendar;
use crate::scene::actors::{self, ActorScene};
use crate::scene::burst;
use crate::scene::drip;
use crate::scene::dust;
use crate::scene::effect_mesh;
use crate::scene::effects;
use crate::scene::emitters;
use crate::scene::lights;
use crate::scene::objects::{self, ObjectScene};
use crate::scene::rain;
use crate::scene::registries::Registries;
use crate::scene::rng::Lcg;
use crate::scene::script;
use crate::scene::sea;
use crate::scene::shade;
use crate::scene::skill_anim;
use crate::scene::sky;
use crate::scene::snow;
use crate::scene::sun;
use crate::scene::view_distance::ViewDistance;
use crate::scene::weather;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::scene::cam_action::{self, CamAction, PLAYBACK_LEAD_MS};
use formats::texture::Texture;
use formats::world::env_texture_list::{self, EnvTextureList};
use formats::world::path::HeightField;
use formats::world::trn::{self, Terrain};
use render::camera::{self, Matrix4, Projection, View};
use render::device::Gpu;
use render::texture::AlphaPolicy;
use render::world::{self, Fog, FrameUniforms, Lighting, Material, TerrainMesh};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) mod loading;
mod tiles;
pub(crate) mod view_stream;

#[cfg(test)]
mod terrain_tests;

#[cfg(test)]
pub(crate) mod fog_tests;

#[cfg(test)]
mod marker_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod material_upload_tests;

pub const CAMERA_PAIR_UNBUILT: (f32, f32) = (0.0, 0.0);

pub const WEATHER_SEED: u32 = 1;

pub const ENV_TEXTURE_LIST: &[u8] = br"Env\EnvTextureList3.bin";

pub const BACKGROUND_WORLDS: [&str; 3] = ["Field2723", "Character", "Field0813"];

pub const CAMERA_PATHS: [&str; 3] = [
    r"UI\DemoCamAction.bin",
    r"UI\DemoCamAction2.bin",
    r"UI\DemoCamAction3.bin",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldSpec<'a> {
    pub variant: u32,
    pub zone: &'a str,
    pub camera: Option<&'a str>,
    pub roster: Option<&'static [u8]>,
    pub day: Option<u32>,
    pub neighbour: bool,
    pub script: bool,
    pub scene: u32,
}

pub const SCENE_LOGIN: u32 = 0x7534;

pub const SCENE_SELECT_CHAR: u32 = 0x7531;

pub const SCENE_BUDGET: u32 = 0x7535;

pub const SCENE_FIELD: u32 = 30_000;

pub const FIELD_DAY: u32 = 0;

pub const SELECT_CHAR_ZONE: &str = "Character";

pub const SELECT_CHAR_CAMERA: &str = r"UI\SelCamAction.bin";

pub const SELECT_CHAR_DAY: u32 = 3;

pub const SELECT_CHAR_VARIANT: u32 = 5;

impl WorldSpec<'static> {
    pub fn server_select(variant: u32) -> WorldSpec<'static> {
        let variant = variant % BACKGROUND_WORLDS.len() as u32;

        WorldSpec {
            variant,
            zone: BACKGROUND_WORLDS[variant as usize],
            camera: Some(CAMERA_PATHS[variant as usize]),
            roster: actors::roster_for(variant),
            day: None,
            neighbour: variant == NEIGHBOUR_VARIANT,
            script: true,
            scene: SCENE_LOGIN,
        }
    }

    pub const SELECT_CHAR: WorldSpec<'static> = WorldSpec {
        variant: SELECT_CHAR_VARIANT,
        zone: SELECT_CHAR_ZONE,
        camera: Some(SELECT_CHAR_CAMERA),
        roster: None,
        day: Some(SELECT_CHAR_DAY),
        neighbour: false,
        script: false,
        scene: SCENE_SELECT_CHAR,
    };
}

pub const FIELD_VARIANT: u32 = 6;

impl<'a> WorldSpec<'a> {
    pub fn field(zone: &'a str) -> WorldSpec<'a> {
        WorldSpec {
            variant: FIELD_VARIANT,
            zone,
            camera: None,
            roster: None,
            day: Some(FIELD_DAY),
            neighbour: false,
            script: false,
            scene: SCENE_FIELD,
        }
    }
}

pub const LIGHT0_BY_DAY: [[f32; 4]; 4] = [
    [0.7, 0.7, 0.7, 0.1],
    [0.3, 0.3, 0.3, 1.0],
    [0.5, 0.4, 0.2, 1.0],
    [0.26, 0.34, 0.34, 1.0],
];

pub const SKY_COLOR_BY_DAY: [u32; 4] = [0x001C_78BD, 0x0033_362A, 0x0062_2F04, 0x0013_2E33];

pub const LIGHT1_DEFAULT: f32 = 0.4;

pub const LIGHT1_SPECIAL_ZONE: f32 = 0.6;

pub const FOG_TABLE: [(f32, f32); 13] = [
    (14.0, 34.0),
    (25.0, 34.0),
    (55.0, 70.0),
    (25.0, 34.0),
    (14.0, 23.0),
    (14.0, 22.0),
    (16.0, 34.0),
    (16.0, 34.0),
    (39.0, 70.0),
    (19.0, 30.0),
    (19.0, 30.0),
    (19.0, 70.0),
    (29.0, 40.0),
];

pub const FOG_GENERIC: (f32, f32) = FOG_TABLE[7];

pub const FOG_SPECIAL_ZONE: (f32, f32) = FOG_TABLE[8];

pub const FOG_BIAS: f32 = 5.0;

pub const FOG_PITCH_GATE: f32 = -0.5;

pub const FOG_END_PITCHED: f32 = 24.0;

pub const FOG_END_LEVEL: f32 = 34.0;

pub const FOG_EASE: f32 = 0.9;

pub fn fog_end_for_pitch(pitch: f32) -> f32 {
    let target = if pitch <= FOG_PITCH_GATE {
        FOG_END_PITCHED
    } else {
        FOG_END_LEVEL
    };

    let current = FOG_GENERIC.1;

    if current as i32 == target as i32 {
        return target;
    }

    target - (target - current) * FOG_EASE
}

pub fn fog_for_scene(scene: u32) -> Option<((f32, f32), bool)> {
    match scene {
        SCENE_BUDGET => Some((FOG_TABLE[10], true)),
        SCENE_SELECT_CHAR => Some((FOG_TABLE[11], false)),
        _ => None,
    }
}

fn environment_type_one(base: Environment, zone_x: u8, zone_y: u8) -> Environment {
    let (zx, zy) = (i32::from(zone_x), i32::from(zone_y));

    if (17..=19).contains(&zx) && zy >= 30 {
        return Environment {
            light0: [1.0, 0.8, 0.8, 1.0],
            light1: [0.3, 0.0, 0.0, 1.0],
            sky_color: 0x0016_0000,
            fog: if (zx, zy) == (18, 30) {
                FOG_TABLE[1]
            } else {
                FOG_TABLE[0]
            },
            ..base
        };
    }

    if zx == 31 && zy == 31 {
        return Environment {
            light0: [1.0, 1.0, 1.0, 1.0],
            light1: [0.6, 0.6, 0.6, 1.0],
            fog: FOG_TABLE[2],
            ..base
        };
    }

    if (28..=30).contains(&zx) && (27..=28).contains(&zy) {
        return Environment {
            light0: [0.9, 0.9, 0.9, 1.0],
            light1: [0.0, 0.0, 0.0, 1.0],
            sky_color: 0,
            fog: FOG_TABLE[3],
            ..base
        };
    }

    if (27..=29).contains(&zx) && (30..=31).contains(&zy) {
        return Environment {
            light0: [1.0, 1.0, 1.0, 1.0],
            light1: [0.4, 0.4, 0.4, 1.0],
            sky_color: 0,
            fog: (25.0, 34.0),
            ..base
        };
    }

    Environment {
        light0: [1.0, 1.0, 1.0, 1.0],
        light1: [0.4, 0.4, 0.4, 1.0],
        sky_color: 0,
        fog: FOG_TABLE[4],
        ..base
    }
}

fn environment_type_two(base: Environment, zone_x: u8, zone_y: u8) -> Environment {
    let inner = matches!(zone_x, 13 | 14) && zone_y == 28;

    Environment {
        light0: if inner {
            [0.8, 0.8, 0.8, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        },
        light1: if inner {
            [0.6, 0.6, 0.6, 1.0]
        } else {
            [0.2, 0.3, 0.2, 1.0]
        },
        sky_color: 0,
        fog: if inner { FOG_TABLE[6] } else { FOG_TABLE[5] },
        ..base
    }
}

pub fn draws_sky(scene: u32, env_type: i32) -> bool {
    matches!(env_type, 0 | 3 | 4) && scene != SCENE_SELECT_CHAR
}

pub const CUTSCENE_DISTANCE: f32 = cam_action::CUTSCENE_DISTANCE;

pub fn tile_of(x: f32, z: f32) -> (i32, i32) {
    ((x as i32) >> 7, (z as i32) >> 7)
}

pub fn is_special_tile(tile: (i32, i32)) -> bool {
    (27..=30).contains(&tile.0) && (21..=24).contains(&tile.1)
}

pub fn is_special_zone(zone_x: u8, zone_y: u8) -> bool {
    is_special_tile((i32::from(zone_x), i32::from(zone_y)))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Environment {
    pub day_quarter: u32,
    pub env_type: i32,
    pub light0: [f32; 4],
    pub light1: [f32; 4],
    pub sky_color: u32,
    pub fog: (f32, f32),
    pub fog_enabled: bool,
    pub far_plane: f32,
    pub special_zone: bool,
    pub snow: bool,
    pub suppresses_flare: bool,
}

impl Environment {
    pub fn new(calendar: &Calendar, zone_x: u8, zone_y: u8) -> Environment {
        let day = calendar.day_quarter() & 3;

        let mut env = Environment::for_day(day, zone_x, zone_y);

        env.snow = day == 1;

        env
    }

    pub fn for_day(day: u32, zone_x: u8, zone_y: u8) -> Environment {
        let day = (day & 3) as usize;

        let env_type = environment_type(zone_x, zone_y);

        let base = Environment {
            day_quarter: day as u32,
            env_type,
            light0: LIGHT0_BY_DAY[day],
            light1: [LIGHT1_DEFAULT, LIGHT1_DEFAULT, LIGHT1_DEFAULT, 1.0],
            sky_color: SKY_COLOR_BY_DAY[day],
            fog: FOG_GENERIC,
            fog_enabled: true,
            far_plane: camera::FAR_SKY,
            special_zone: is_special_zone(zone_x, zone_y),
            snow: false,
            suppresses_flare: matches!(env_type, 1 | 2),
        };

        match env_type {
            1 => environment_type_one(base, zone_x, zone_y),
            2 => environment_type_two(base, zone_x, zone_y),
            5 => Environment {
                light0: [1.0, 1.0, 1.0, 1.0],
                light1: [0.4, 0.4, 0.4, 1.0],
                sky_color: 0,
                fog: FOG_TABLE[4],
                ..base
            },
            _ => {
                let special = base.special_zone;

                let light1 = if special {
                    LIGHT1_SPECIAL_ZONE
                } else {
                    LIGHT1_DEFAULT
                };

                let (start, end) = if special {
                    FOG_SPECIAL_ZONE
                } else {
                    FOG_GENERIC
                };

                Environment {
                    light1: [light1, light1, light1, 1.0],
                    fog: (start + FOG_BIAS, end + FOG_BIAS),
                    far_plane: if special {
                        camera::FAR_SKY_SPECIAL_ZONE
                    } else {
                        camera::FAR_SKY
                    },
                    ..base
                }
            }
        }
    }

    pub fn apply_pitch(&mut self, scene: u32, pitch: f32) {
        let refused = fog_for_scene(scene).is_some() || self.special_zone;

        if refused || !matches!(self.env_type, 0 | 3 | 4) {
            return;
        }

        self.fog.1 = fog_end_for_pitch(pitch) + FOG_BIAS;
    }

    pub fn for_scene(mut self, scene: u32) -> Environment {
        if !matches!(self.env_type, 0 | 3 | 4) {
            return self;
        }

        if let Some(((start, end), enabled)) = fog_for_scene(scene) {
            self.fog = (start + FOG_BIAS, end + FOG_BIAS);

            self.fog_enabled = enabled;
        }

        self
    }

    pub fn apply_day(&mut self, blend: sky::Blend) {
        if !self.uses_day_cycle() {
            return;
        }

        self.sky_color = blend.sky_color;

        self.light0 = blend.light0;
    }

    pub fn uses_day_cycle(&self) -> bool {
        matches!(self.env_type, 0 | 3 | 4)
    }

    pub fn clear_color(&self) -> wgpu::Color {
        let [r, g, b, _] = world::argb_to_rgba_f32(self.sky_color);

        wgpu::Color {
            r: f64::from(r),
            g: f64::from(g),
            b: f64::from(b),
            a: 1.0,
        }
    }

    pub fn lighting(&self) -> Lighting {
        Lighting {
            light0: self.light0,
            light1: self.light1,
            material: Material::TERRAIN,
            global_ambient: world::GLOBAL_AMBIENT,
            point_lights: render::world::PointLights::default(),
        }
    }

    pub fn fog(&self) -> Fog {
        self.fog_at(CUTSCENE_DISTANCE)
    }

    pub fn fog_at(&self, camera_distance: f32) -> Fog {
        Fog {
            enabled: self.fog_enabled,
            ..Fog::from_sky(self.sky_color, self.fog.0, self.fog.1, camera_distance)
        }
    }

    fn visual_fog(
        &self,
        scene: u32,
        camera_distance: f32,
        underwater: bool,
        view_distance: ViewDistance,
    ) -> Fog {
        let mut fog = self.fog_at(camera_distance);

        if scene == SCENE_FIELD && !underwater {
            fog.start *= view_distance.multiplier();

            fog.end *= view_distance.multiplier();
        }

        fog
    }

    fn view_far_plane(&self, scene: u32, view_distance: ViewDistance) -> f32 {
        self.far_plane
            * if scene == SCENE_FIELD {
                view_distance.multiplier()
            } else {
                1.0
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub eye: glam::Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Pose {
    pub fn view(&self) -> View {
        View::from_pose(self.eye, self.yaw, self.pitch)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraPath {
    pub action: CamAction,
    pub start_ms: u32,
}

impl CameraPath {
    pub fn pose(&self, now_ms: u32) -> Option<Pose> {
        let t = now_ms
            .wrapping_sub(self.start_ms)
            .wrapping_add(PLAYBACK_LEAD_MS);

        if let Some(s) = self.action.sample(t) {
            return Some(Pose {
                eye: glam::Vec3::new(s.x, s.y, s.z),
                yaw: s.yaw,
                pitch: s.pitch,
            });
        }

        let last = self.action.keyframes.last()?;

        (t > last.time_ms).then(|| Pose {
            eye: glam::Vec3::new(last.x, last.y, last.z),
            yaw: last.yaw,
            pitch: last.pitch,
        })
    }

    pub fn pose_or(&self, now_ms: u32, previous: Pose) -> Pose {
        self.pose(now_ms).unwrap_or(previous)
    }
}

#[derive(Debug)]
pub struct Tile {
    pub id: i32,
    pub path: String,
    pub texture: Texture,
}

fn next_world_identity() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);

    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug)]
pub struct World {
    pub identity: u64,
    pub terrain_revision: u64,
    terrain_pages: Vec<u64>,
    background_loader: Option<loading::Loader>,
    background_retry: Option<web_time::Instant>,
    visual_zone: Option<(u8, u8)>,
    visual_prefetch: view_stream::Prefetch,
    visual_collision_dirty: bool,
    pub variant: u32,
    pub scene: u32,
    pub name: String,
    pub terrain: Terrain,
    pub streamed: Vec<Streamed>,
    pub lamps: lights::Lamps,
    pub current: (u8, u8),
    pub force_load: Option<(u8, u8)>,
    pub force_load_restore_yaw: Option<f32>,
    pub mesh: TerrainMesh,
    pub tiles: Vec<Tile>,
    tile_list: EnvTextureList,
    pub missing: Vec<i32>,
    pub path: CameraPath,
    pub environment: Environment,
    pub view_distance: ViewDistance,
    pub env_type: i32,
    pub env_global: i32,
    pub sky: sky::Sky,
    pub sky_ornaments: sky::Ornaments,
    pub sun: Option<sun::Sun>,
    pub projected: Option<(Matrix4, Matrix4)>,
    pub weather: weather::Weather,
    pub rain: rain::Rain,
    pub snow: snow::Snow,
    pub snow_fast: snow::Snow,
    pub weather_rng: super::rng::Lcg,
    pub weather_target: Option<glam::Vec3>,
    pub weather_anchor: weather::Anchor,
    pub pose: Pose,
    pub camera_distance: f32,
    pub objects: ObjectScene,
    pub actors: ActorScene,
    pub lights: lights::Lights,
    pub effects: effects::BillboardScene,
    pub effect_scratch: effects::Frame,
    pub(super) actor_scratch: actors::Frame,
    effect_owners: super::effect_context::OwnerIndex,
    pub sea: sea::SeaScene,
    pub ambient: Vec<dust::Source>,
    pub terrain_ambient: super::terrain_ambient::State,
    pub drips: Vec<drip::Column>,
    pub drip_rng: Lcg,
    pub script: Option<script::DemoScript>,
    sounds: Vec<i64>,
    damage_sounds: Vec<skill_anim::DamageSoundRequest>,
}

#[derive(Debug)]
pub struct Streamed {
    pub zone: (u8, u8),
    pub name: String,
    pub terrain: Terrain,
    pub(crate) relief: Vec<i8>,
}

pub const STREAM_MARGIN: f32 = 16.0;

pub const STREAM_MARGIN_WIDE: f32 = 20.0;

pub const WIDE_MARGIN_ZONE: (u8, u8) = (17, 10);

pub const WIDE_MARGIN_X: std::ops::Range<i32> = 0xD00..0xF80;

pub const WIDE_MARGIN_ROWS: std::ops::Range<i32> = 21..25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proximity {
    Load(Direction),
    Cross(Direction),
}

pub fn proximity(offset: glam::Vec2, margin: f32) -> [Option<Proximity>; 2] {
    let cells = formats::world::ZONE_CELLS as f32;

    let axis = |v: f32, low: Direction, high: Direction| -> Option<Proximity> {
        if !v.is_finite() {
            return None;
        }

        if (0.0..margin).contains(&v) {
            return Some(Proximity::Load(low));
        }

        if v < 0.0 {
            return Some(Proximity::Cross(low));
        }

        if v > cells - margin && v < cells {
            return Some(Proximity::Load(high));
        }

        (v >= cells).then_some(Proximity::Cross(high))
    };

    [
        axis(offset.x, Direction::MinusX, Direction::PlusX),
        axis(offset.y, Direction::MinusZ, Direction::PlusZ),
    ]
}

pub fn window_anchor(current: (u8, u8), dir: Direction) -> (u8, u8) {
    match dir {
        Direction::MinusX => (current.0.saturating_sub(1), current.1),
        Direction::MinusZ => (current.0, current.1.saturating_sub(1)),
        Direction::PlusX | Direction::PlusZ => current,
    }
}

pub const YAW_RESET: f32 = std::f32::consts::FRAC_PI_4;

pub fn stream_margin(zone: (u8, u8), player: glam::Vec2) -> f32 {
    let x = player.x as i32 & !0x7F;

    let row = (player.y as i32) >> 7;

    if WIDE_MARGIN_X.contains(&x) && WIDE_MARGIN_ROWS.contains(&row) {
        return STREAM_MARGIN_WIDE;
    }

    if zone == WIDE_MARGIN_ZONE {
        return STREAM_MARGIN_WIDE;
    }

    STREAM_MARGIN
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldFrame {
    pub pose: Pose,
    pub view: View,
    pub projection: Projection,
    pub uniforms: FrameUniforms,
    pub stage1_enabled: bool,
}

impl WorldFrame {
    pub fn object_frustum(&self) -> render::cull::Frustum {
        render::cull::Frustum::from_clip(&render::camera::Matrix4::multiply(
            &self.view.matrix(),
            &self.projection.matrix(),
        ))
    }
}

pub const NEIGHBOUR_VARIANT: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    MinusX,
    PlusX,
    MinusZ,
    PlusZ,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::MinusX,
        Direction::PlusX,
        Direction::MinusZ,
        Direction::PlusZ,
    ];

    pub fn step(self) -> (i32, i32) {
        match self {
            Direction::MinusX => (-1, 0),
            Direction::PlusX => (1, 0),
            Direction::MinusZ => (0, -1),
            Direction::PlusZ => (0, 1),
        }
    }
}

pub fn neighbour_of(zone: (u8, u8), dir: Direction) -> Option<((u8, u8), String)> {
    let (dx, dz) = dir.step();

    let x = u8::try_from(i32::from(zone.0) + dx).ok()?;

    let z = u8::try_from(i32::from(zone.1) + dz).ok()?;

    Some(((x, z), format!("Field{x:02}{z:02}")))
}

pub fn neighbour_zone(terrain: &Terrain) -> Option<((u8, u8), String)> {
    neighbour_of((terrain.zone_x, terrain.zone_y), Direction::PlusX)
}

pub fn build(game: &GameDir, variant: u32, calendar: &Calendar) -> Result<World> {
    build_spec(game, &WorldSpec::server_select(variant), calendar)
}

pub fn build_spec(game: &GameDir, spec: &WorldSpec, calendar: &Calendar) -> Result<World> {
    let WorldSpec {
        variant,
        zone: name,
        ..
    } = *spec;

    let trn_path = format!(r"Env\{name}.trn");

    let bytes = game
        .read(trn_path.as_bytes())
        .with_context(|| format!("reading {trn_path}"))?;

    let mut terrain = trn::parse(&bytes).with_context(|| format!("parsing {trn_path}"))?;

    let extra = spec.neighbour.then(|| neighbour_zone(&terrain)).flatten();

    let mut neighbour = match &extra {
        Some((_, name)) => {
            let path = format!(r"Env\{name}.trn");

            let bytes = game
                .read(path.as_bytes())
                .with_context(|| format!("reading {path}"))?;

            Some(trn::parse(&bytes).with_context(|| format!("parsing {path}"))?)
        }
        None => None,
    };

    let list_bytes = game
        .read(ENV_TEXTURE_LIST)
        .context("reading Env\\EnvTextureList3.bin")?;

    let list = env_texture_list::parse(&list_bytes).context("parsing EnvTextureList3.bin")?;

    let action = match spec.camera {
        Some(path) => load_camera(game, path)?,
        None => CamAction {
            keyframes: Vec::new(),
        },
    };

    let day = spec.day.unwrap_or_else(|| calendar.day_quarter() & 3);

    let environment = match spec.day {
        Some(day) => Environment::for_day(day, terrain.zone_x, terrain.zone_y),
        None => Environment::new(calendar, terrain.zone_x, terrain.zone_y),
    }
    .for_scene(spec.scene);

    let env_type = environment_type(terrain.zone_x, terrain.zone_y);

    let seed = action.keyframes.first().map(|k| Pose {
        eye: glam::Vec3::new(k.x, k.y, k.z),
        yaw: k.yaw,
        pitch: k.pitch,
    });

    let registries =
        Arc::new(Registries::load(game).with_context(|| format!("loading {name}'s registries"))?);

    let mut objects = objects::build(
        game,
        name,
        (terrain.zone_x, terrain.zone_y),
        environment.day_quarter,
        Arc::clone(&registries),
    )
    .with_context(|| format!("building {name}'s objects"))?;

    if let Some((zone, neighbour_name)) = &extra {
        objects
            .load_neighbour(game, neighbour_name, *zone)
            .with_context(|| format!("building {neighbour_name}'s objects"))?;
    }

    let zone = (terrain.zone_x, terrain.zone_y);

    let mut lamps = lights::Lamps::new(spec.scene);

    let lights = {
        let mut list: Vec<&mut Terrain> = vec![&mut terrain];

        list.extend(neighbour.as_mut());

        let mut zones = lights::Zones::new(list);

        let mut lights = lights::build(&mut zones, &mut lamps, &objects.records, zone, spec.scene);

        if let Some((neighbour_zone, _)) = &extra {
            let more = lights::build(
                &mut zones,
                &mut lamps,
                &objects.neighbour_records,
                *neighbour_zone,
                spec.scene,
            );

            lights.merge(more);
        }

        lights
    };

    let mut mesh = world::build_terrain(&terrain);

    if let Some(neighbour) = &neighbour {
        mesh.append_neighbour(
            &world::build_terrain(neighbour),
            formats::world::TRN_DIM as i32,
            0,
        );
    }

    mesh.stitch_boundaries();

    let mut tiles = Vec::new();

    let missing = tiles::load(game, &list, &mesh, &mut tiles)?;

    let mut actors = actors::build(
        game,
        spec.roster,
        &terrain,
        &objects.records,
        environment.light0,
        environment.light1,
        render::skin::grass_enabled(render::quirk::VendorClass::DEFAULT),
        spec.scene,
        registries,
    )
    .with_context(|| format!("building {name}'s actors"))?;

    if let Some(terrain) = &neighbour {
        actors
            .add_neighbour(
                game,
                terrain,
                &objects.neighbour_records,
                environment.light0,
                environment.light1,
            )
            .with_context(|| format!("building {name}'s neighbour trees"))?;
    }

    objects
        .load_held(game, &mut actors)
        .with_context(|| format!("loading {name}'s weapons"))?;

    objects
        .load_by_id(game, &dust::WATERFALL_MESH_IDS)
        .with_context(|| format!("loading {name}'s waterfall lanes"))?;

    objects.load_mesh_texture_slots(game, &[6]);

    if spec.scene == SCENE_FIELD {
        let rotor_meshes = super::mesh_fx::HUMAN_ROTOR_MESHES.map(|id| id as u32);

        objects
            .load_by_id(game, &rotor_meshes)
            .with_context(|| format!("loading {name}'s human rotor meshes"))?;

        objects
            .load_by_id(game, &[skill_anim::PULSE_MESH as u32])
            .with_context(|| format!("loading {name}'s weapon-pulse mesh"))?;

        objects
            .load_by_id(game, &super::skill::static_mesh_ids())
            .with_context(|| format!("loading {name}'s generated skill meshes"))?;

        objects.load_effect_textures(game, &[19, 91]);

        objects.load_effect_textures(game, &super::skill::mesh_effect_textures());

        objects.load_effect_textures(game, &[20, 51, 52, 54, 152]);

        objects
            .load_by_id(game, &[704, 705])
            .context("loading killer Charge meshes")?;

        objects
            .load_by_id(game, &[10, 19, 20])
            .context("loading special weapon swing meshes")?;

        objects.load_mesh_texture_slots(game, &[201]);

        objects.load_effect_textures(game, &[57]);

        objects.load_effect_textures(game, &[0, 45]);

        let arrow_textures: Vec<usize> =
            (20..26).chain(61..67).chain(101..107).chain([97]).collect();

        objects.load_effect_textures(game, &arrow_textures);

        objects.load_mesh_texture_overrides(game, &["mesh\\stone01.wys"]);
    }

    if spec.scene == SCENE_SELECT_CHAR {
        objects
            .load_by_id(game, &[10])
            .context("loading character-slot effect mesh")?;
    }

    if !lights.glows.is_empty() {
        let ids: Vec<u32> = lights.glows.iter().map(|g| g.mesh_id as u32).collect();

        objects
            .load_by_id(game, &ids)
            .with_context(|| format!("loading {name}'s glow mesh"))?;
    }

    let mut effects = effects::BillboardScene::build(game)
        .with_context(|| format!("loading {name}'s effect textures"))?;

    let sky_ornaments = sky::Ornaments::new(&mut Lcg::new(1));

    effects.billboards = emitters::build(&objects.records, zone);

    if let Some((neighbour_zone, _)) = &extra {
        effects
            .billboards
            .extend(emitters::build(&objects.neighbour_records, *neighbour_zone));
    }

    let mut ambient = dust::build_sources(&objects.records, zone);

    if let Some((neighbour_zone, _)) = &extra {
        ambient.extend(dust::build_sources(
            &objects.neighbour_records,
            *neighbour_zone,
        ));
    }

    let mut drip_rng = Lcg::new(drip::COLUMN_SEED);

    let mut drips = drip::build_columns(&objects.records, zone, &mut drip_rng);

    if let Some((neighbour_zone, _)) = &extra {
        drips.extend(drip::build_columns(
            &objects.neighbour_records,
            *neighbour_zone,
            &mut drip_rng,
        ));
    }

    let mut water =
        sea::SeaScene::build(game).with_context(|| format!("loading {name}'s water textures"))?;

    water.seas = sea::build(&objects.records, zone, spec.scene);

    if let Some((neighbour_zone, _)) = &extra {
        water.seas.extend(sea::build(
            &objects.neighbour_records,
            *neighbour_zone,
            spec.scene,
        ));
    }

    effects.shades = lights.shades.clone();

    let heights = terrain.build_height_grid();

    let ground = shade::Ground::new(&terrain, &heights, neighbour.iter().collect());

    for decal in &mut effects.shades {
        decal.place(&ground);
    }

    let mut ordinal = 0;

    for float in &mut actors.raft.floats {
        let at = float.position;

        let owner = std::iter::once(&terrain)
            .chain(neighbour.iter())
            .find(|t| t.contains(at.x, at.z));

        let height = owner.and_then(|t| {
            super::floats::water_height(&water.seas, (t.zone_x, t.zone_y), at.x, at.z, 0)
        });

        let light = owner.map_or_else(
            || super::lights::sample::<&Terrain>(&[], at.x, at.z),
            |t| super::lights::sample(&[t], at.x, at.z),
        );

        float.refresh(height, light);

        let quads = float.quads(ordinal);

        ordinal += quads.len();

        effects.quads.extend(quads);
    }

    effects.make_resident(game);

    actors.place_swing_radii(|id| objects.mesh_max_z(id));

    for texture in &mut objects.textures {
        texture.prepare_material(false);
    }

    for texture in &mut actors.textures {
        texture.prepare_material(true);
    }

    let mut rng = super::rng::Lcg::new(WEATHER_SEED);

    let (rain, snow, snow_fast) = (
        rain::Rain::new(&mut rng),
        snow::Snow::new(1.0, &mut rng),
        snow::Snow::new(2.0, &mut rng),
    );

    Ok(World {
        identity: next_world_identity(),
        terrain_revision: 0,
        terrain_pages: Vec::new(),
        background_loader: None,
        background_retry: None,
        visual_zone: None,
        visual_prefetch: Default::default(),
        visual_collision_dirty: false,
        variant,
        scene: spec.scene,
        name: name.to_string(),
        lamps,
        current: (terrain.zone_x, terrain.zone_y),
        force_load: None,
        force_load_restore_yaw: None,
        terrain,
        streamed: match (extra, neighbour) {
            (Some((zone, name)), Some(terrain)) => {
                vec![Streamed {
                    zone,
                    name,
                    relief: terrain.build_height_grid(),
                    terrain,
                }]
            }
            _ => Vec::new(),
        },
        mesh,
        tiles,
        tile_list: list,
        missing,
        path: CameraPath {
            action,
            start_ms: 0,
        },
        camera_distance: CUTSCENE_DISTANCE,
        environment,
        view_distance: ViewDistance::Retail,
        env_type,
        env_global: env_type,
        sky: sky::Sky::at(day),
        sky_ornaments,
        sun: (spec.scene != SCENE_SELECT_CHAR).then(sun::Sun::new),
        projected: None,
        weather: weather::Weather {
            emitters: weather::Emitters {
                snow: spec.scene == SCENE_LOGIN && weather::login_snow(calendar.day_quarter()),
                ..weather::Emitters::NONE
            },
            ..weather::Weather::new(0)
        },
        rain,
        snow,
        snow_fast,
        weather_rng: rng,
        weather_target: None,
        weather_anchor: weather::Anchor::adrift(CAMERA_PAIR_UNBUILT.0, CAMERA_PAIR_UNBUILT.1),
        pose: seed.unwrap_or(Pose {
            eye: glam::Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        }),
        objects,
        actors,
        effects,
        effect_scratch: effects::Frame::default(),
        actor_scratch: actors::Frame::default(),
        effect_owners: super::effect_context::OwnerIndex::default(),
        lights,
        sea: water,
        ambient,
        terrain_ambient: super::terrain_ambient::State::default(),
        drips,
        drip_rng,
        script: spec.script.then(|| script::DemoScript::new(variant)),
        sounds: Vec::new(),
        damage_sounds: Vec::new(),
    })
}

pub fn load_camera(game: &GameDir, path: &str) -> Result<CamAction> {
    let bytes = game
        .read(path.as_bytes())
        .with_context(|| format!("reading {path}"))?;

    cam_action::parse(&bytes).with_context(|| format!("parsing {path}"))
}

pub fn environment_type(zone_x: u8, zone_y: u8) -> i32 {
    render::world::terrain_environment(zone_x, zone_y)
}

fn stitch_terrain_boundaries(
    mesh: &mut TerrainMesh,
    terrain_revision: &mut u64,
    terrain_pages: &mut Vec<u64>,
    ranges: &[std::ops::Range<usize>],
) {
    let changed = mesh.stitch_boundaries_in(ranges);

    if changed.is_empty() {
        return;
    }

    let revision = terrain_revision
        .checked_add(1)
        .expect("terrain revision exhausted");

    terrain_pages.resize(mesh.vertices.len().div_ceil(256), 0);

    for range in changed {
        terrain_pages[range.start / 256..range.end.div_ceil(256)].fill(revision);
    }

    *terrain_revision = revision;
}

fn refresh_terrain_colours(
    mesh: &mut TerrainMesh,
    terrains: &[&Terrain],
    terrain_revision: &mut u64,
    terrain_pages: &mut Vec<u64>,
    registered: &[usize],
) {
    let revision = terrain_revision
        .checked_add(1)
        .expect("terrain revision exhausted");

    terrain_pages.resize(mesh.vertices.len().div_ceil(256), 0);

    let mut changed = false;

    for &zone_index in registered {
        let terrain = terrains[zone_index];
        let dx = (i32::from(terrain.zone_x) - i32::from(mesh.zone.0)) * 64;

        let dz = (i32::from(terrain.zone_y) - i32::from(mesh.zone.1)) * 64;

        let start = zone_index * 64 * 64 * 4;

        for (local, vertex) in mesh.vertices[start..start + 64 * 64 * 4]
            .iter_mut()
            .enumerate()
        {
            let x = (vertex.pos[0] as i32 - dx).clamp(0, 63) as usize;

            let z = (vertex.pos[2] as i32 - dz).clamp(0, 63) as usize;

            let colour = u32::from_le_bytes(terrain.cells[z * 64 + x].color);

            if vertex.diffuse != colour {
                vertex.diffuse = colour;

                terrain_pages[(start + local) / 256] = revision;

                changed = true;
            }
        }
    }

    if changed {
        *terrain_revision = revision;
    }

    let ranges = terrain_boundary_ranges(terrains, registered);

    stitch_terrain_boundaries(mesh, terrain_revision, terrain_pages, &ranges);
}

fn terrain_boundary_ranges(
    terrains: &[&Terrain],
    registered: &[usize],
) -> Vec<std::ops::Range<usize>> {
    terrains
        .iter()
        .enumerate()
        .filter(|(_, terrain)| {
            registered.iter().any(|&index| {
                terrain.zone_x.abs_diff(terrains[index].zone_x) <= 1
                    && terrain.zone_y.abs_diff(terrains[index].zone_y) <= 1
            })
        })
        .map(|(index, _)| index * 64 * 64 * 4..(index + 1) * 64 * 64 * 4)
        .collect()
}

pub fn detail_layer_enabled(env_type: i32) -> bool {
    matches!(env_type, 0 | 3 | 4)
}

pub const WATER_TILE_IDS: [i32; 2] = [38, 39];

pub const LAVA_TILE_IDS: [i32; 2] = [170, 171];

pub const SCROLLED_TILE0: std::ops::RangeInclusive<i16> = 0x34..=0x37;

pub const WATER_BASE_SHIFT: i32 = 102;

pub const WATER_DETAIL_TEXTURE: i32 = 0x158;

pub const TILE_SCROLL_PERIOD_MS: u32 = 10_000;

pub fn animated_tiles_enabled(zone_x: u8, zone_y: u8) -> bool {
    let env_type = environment_type(zone_x, zone_y);

    let (zx, zy) = (i32::from(zone_x), i32::from(zone_y));

    !matches!(env_type, 0 | 3 | 4) && !((26..=30).contains(&zx) && (8..=12).contains(&zy))
}

fn resolve_mesh_submissions(
    submissions: Vec<super::effect_tree::MeshSubmission>,
    sample: impl Fn(f32, f32) -> [f32; 4],
    owner_visible: impl Fn(u16) -> bool,
    light0: [f32; 4],
    fog: bool,
) -> (
    Vec<lights::Glow>,
    Vec<objects::RotateLitDraw>,
    Vec<super::effect_tree::TexturedMesh>,
    Vec<super::effect_tree::AuthoredMesh>,
) {
    let mut glows = Vec::new();

    let mut rotate_lit = Vec::new();

    let mut textured = Vec::new();

    let mut authored = Vec::new();

    for submission in submissions {
        match submission {
            super::effect_tree::MeshSubmission::Glow(glow) => glows.push(glow),
            super::effect_tree::MeshSubmission::Textured(effect) => textured.push(effect),
            super::effect_tree::MeshSubmission::Authored(effect) => authored.push(effect),
            super::effect_tree::MeshSubmission::RotateLit(effect) => {
                if effect.owner.is_some_and(|owner| !owner_visible(owner)) {
                    continue;
                }

                let terrain = sample(effect.position.x, effect.position.z);

                rotate_lit.push(objects::RotateLitDraw {
                    mesh_id: effect.mesh_id,
                    world: effect.world,
                    material: super::mesh_fx::rotate_lit_material(terrain, light0),
                    fog,
                });
            }
        }
    }

    (glows, rotate_lit, textured, authored)
}

fn spawn_pulse_plan(
    scene: &mut effects::BillboardScene,
    rng: &mut Lcg,
    ground: &shade::Ground<'_>,
    plan: skill_anim::PulsePlan,
    now_ms: u32,
) -> usize {
    let mut raised = 0;

    for step in plan.steps {
        match step {
            skill_anim::PulseStep::Mesh(recipe) => {
                let mut mesh = effect_mesh::EffectMesh::new(
                    recipe.mesh,
                    recipe.argb,
                    recipe.yaw,
                    recipe.mode,
                    now_ms,
                );

                mesh.position = recipe.at;

                mesh.scale_xz = recipe.scale_xz;

                mesh.scale_y = recipe.scale_y;

                mesh.texture_bias = recipe.texture;

                mesh.lifetime_ms = recipe.lifetime_ms;

                mesh.period_ms = recipe.period_ms;

                mesh.pulse = recipe.pulse;

                mesh.blend = if recipe.additive {
                    render::mesh::EFFECT_ADDITIVE
                } else {
                    render::mesh::EFFECT_BLENDED
                };

                scene.spawn(Box::new(mesh));

                raised += 1;
            }
            skill_anim::PulseStep::Shade(recipe) => {
                let mut shade = shade::Shade::new(
                    recipe.grid,
                    recipe.texture as usize,
                    recipe.argb,
                    glam::Vec3::new(recipe.at.x, 0.0, recipe.at.y),
                    -1,
                )
                .with_lifetime(recipe.lifetime_ms);

                shade.radius = recipe.radius;

                shade.pass = if recipe.additive {
                    render::billboard::BillboardPass::Additive
                } else {
                    render::billboard::BillboardPass::Blended
                };

                shade.place(ground);

                scene.spawn(Box::new(shade));

                raised += 1;
            }
            skill_anim::PulseStep::TouchChild { .. } => {}
            skill_anim::PulseStep::Particle(recipe) => {
                raised += burst::spawn(
                    scene,
                    &burst::Recipe {
                        at: recipe.at,
                        kind: recipe.kind,
                        count: recipe.count,
                        spread: recipe.size_step,
                        argb: recipe.argb,
                        random_colour: recipe.random_colour,
                        texture: recipe.texture as usize,
                        scatter: recipe.spread,
                        additive: recipe.additive,
                        axis: recipe.axis,
                        lifetime_ms: recipe.lifetime_ms,
                    },
                    now_ms,
                    rng,
                );
            }
            skill_anim::PulseStep::Blade(recipe) => {
                let mut blade = effects::Billboard::new(
                    recipe.texture as usize,
                    recipe.lifetime_ms,
                    recipe.scale,
                    recipe.constructor_growth,
                    recipe.frame_count as u32,
                    recipe.frame_ms as u32,
                );

                blade.position = recipe.at;

                blade.origin = recipe.at;

                blade.axis = recipe.axis;

                blade.roll = recipe.yaw;

                blade.growth = recipe.growth;

                blade.motion = effects::Motion::from_field(recipe.motion as u32);

                blade.facing = recipe.billboard;

                blade.pass = if recipe.additive {
                    render::billboard::BillboardPass::Additive
                } else {
                    render::billboard::BillboardPass::Blended
                };

                blade.set_color(recipe.argb);

                scene.spawn(Box::new(blade));

                raised += 1;
            }
        }
    }

    raised
}

impl World {
    pub fn terrain_draw_enabled(&self) -> bool {
        let raised = self.pose.pitch > 0.4;

        self.scene != SCENE_SELECT_CHAR && !raised
    }

    pub fn play_camera(&mut self, action: CamAction, now_ms: u32) {
        self.path = CameraPath {
            action,
            start_ms: now_ms,
        };
    }

    pub fn restart_camera(&mut self, now_ms: u32) {
        self.path.start_ms = now_ms;
    }

    pub fn frame(&mut self, now_ms: u32, backbuffer: (u32, u32)) -> WorldFrame {
        let pose = self.path.pose_or(now_ms, self.pose);

        self.frame_at(pose, now_ms, backbuffer)
    }

    fn resolve_weather_anchor(&self) -> weather::Anchor {
        if self.scene == SCENE_LOGIN {
            let eye = self.pose.eye;

            return weather::Anchor::on_login_camera(
                [eye.x, eye.y, eye.z],
                self.weather_target.map(|t| t.y),
            );
        }

        match self.weather_target {
            Some(t) => weather::Anchor::on_target(t.x, t.y, t.z),
            None => weather::Anchor::adrift(CAMERA_PAIR_UNBUILT.0, CAMERA_PAIR_UNBUILT.1),
        }
    }

    pub fn tick_weather(&mut self, now_ms: u32) {
        let (anchor, emitters) = (self.weather_anchor, self.weather.emitters);

        if emitters.snow_fast {
            self.snow_fast.tick(now_ms, anchor, &mut self.weather_rng);
        }

        if emitters.snow {
            self.snow.tick(now_ms, anchor, &mut self.weather_rng);
        }

        if emitters.rain {
            let field = HeightField::window(&self.actors.window, self.actors.zone);

            let ground = drip::window_sampler(&field);

            let seas = &self.sea.seas;

            let splashes = self.rain.tick_on_surface(
                now_ms,
                anchor,
                &mut self.weather_rng,
                self.actors.attributes(),
                |x, z| drip::splash_surface_y(ground(x, z), seas, x, z),
            );

            for splash in splashes {
                self.effects.spawn(Box::new(splash));
            }
        }
    }

    pub fn tick_ambient(&mut self, now_ms: u32) {
        let target = self.weather_target;

        for i in 0..self.ambient.len() {
            let mut source = self.ambient[i];

            let out = source.tick(now_ms, &mut self.weather_rng, target);

            self.ambient[i] = source;

            self.effects.billboards.extend(out.billboards);

            for dust in out.dust {
                self.effects.spawn(Box::new(dust));
            }

            for bubble in out.bubbles {
                self.effects.spawn(Box::new(bubble));
            }

            self.sounds.extend(out.sounds);
        }

        self.tick_drips(now_ms, target);
    }

    fn tick_drips(&mut self, now_ms: u32, target: Option<glam::Vec3>) {
        let field = HeightField::window(&self.actors.window, self.actors.zone);

        let ground = drip::window_sampler(&field);

        let seas = &self.sea.seas;

        for i in 0..self.drips.len() {
            let mut column = self.drips[i];

            let out = column.tick_on_surface(now_ms, &mut self.weather_rng, target, |x, z| {
                drip::splash_surface_y(ground(x, z), seas, x, z)
            });

            self.drips[i] = column;

            for splash in out.splashes {
                self.effects.spawn(Box::new(splash));
            }

            self.sounds.extend(out.sounds);
        }
    }

    pub fn set_view_distance(&mut self, view_distance: ViewDistance) {
        if self.view_distance != view_distance {
            self.view_distance = view_distance;

            self.projected = None;
        }
    }

    pub fn view_far_plane(&self) -> f32 {
        self.environment
            .view_far_plane(self.scene, self.view_distance)
    }

    pub fn fog(&self) -> render::world::Fog {
        self.camera_environment().visual_fog(
            self.scene,
            self.camera_distance,
            self.camera_under_water(),
            self.view_distance,
        )
    }

    pub fn camera_under_water(&self) -> bool {
        let eye = self.pose.eye;

        if !eye.is_finite() {
            return false;
        }

        let owner = std::iter::once(&self.terrain)
            .chain(self.streamed.iter().map(|z| &z.terrain))
            .find(|t| t.contains(eye.x, eye.z));

        let Some(owner) = owner else {
            return false;
        };

        self.sea
            .seas
            .iter()
            .filter(|s| s.zone == (owner.zone_x, owner.zone_y))
            .take(10)
            .find(|s| s.contains(eye.x, eye.z))
            .is_some_and(|s| s.position.y > eye.y)
    }

    pub fn camera_environment(&self) -> Environment {
        let mut environment = self.environment;
        let underwater = self.camera_under_water();

        if !underwater
            && draws_sky(self.scene, environment.env_type)
            && let Some(color) = self
                .objects
                .sky_horizon
                .as_ref()
                .and_then(|horizon| horizon.current)
        {
            environment.sky_color = color;
        }

        if underwater {
            environment.fog = (1.0, 10.0 + (self.camera_distance - 10.0).max(0.0));

            environment.sky_color = if matches!(environment.env_type, 1 | 2) {
                0xFF335599
            } else {
                match self.sky.day {
                    0 => 0xFF335599,
                    1 => 0xFF333333,
                    2 => 0xFF441100,
                    3 => 0xFF222222,
                    _ => environment.sky_color,
                }
            };
        }

        environment
    }

    pub fn clear_color(&self) -> wgpu::Color {
        self.camera_environment().clear_color()
    }

    pub fn frame_at(&mut self, pose: Pose, now_ms: u32, backbuffer: (u32, u32)) -> WorldFrame {
        self.actors.body_light.begin(self.scene, now_ms);

        self.pose = pose;

        if let Some(horizon) = &mut self.objects.sky_horizon {
            horizon.update(
                self.sky,
                now_ms,
                self.environment.uses_day_cycle(),
                pose.eye.y,
            );
        }

        self.objects.update_motion(now_ms);

        for glow in &mut self.lights.glows {
            glow.advance_uv(now_ms);
        }

        let heights =
            formats::world::path::HeightField::window(&self.actors.window, self.actors.zone);

        self.objects
            .update_fade(self.scene, self.weather_target, pose.eye, now_ms, &heights);

        self.sky_ornaments
            .update(self.sky, pose, now_ms, self.environment.uses_day_cycle());

        self.refresh_floats(now_ms);

        self.flush_death_particles();

        self.objects.environment = self.environment.env_type;

        self.effects.environment = self.environment.env_type;

        self.effects.scene_id = self.scene;

        self.effects.view_distance = self.view_distance;

        self.actors.grass.field.view_distance = if self.scene == SCENE_FIELD {
            self.view_distance
        } else {
            ViewDistance::Retail
        };

        super::transformation::update(self, now_ms);

        super::hit_reaction::update(self, now_ms);

        super::actor_effects::update(&mut self.actors, &mut self.effects, now_ms);

        super::actor_effects::hand_sprites(&self.actors, &mut self.effects, now_ms, false);

        for (entity, rotors) in self.actors.take_rotor_refreshes() {
            self.effects
                .tree
                .remove_owned(super::effect_tree::OwnedGroup::HumanRotors(entity));

            for mut rotor in rotors {
                if self.actors.actors.iter().any(|actor| {
                    actor.entity == entity
                        && (actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1)
                }) {
                    super::effect_tree::Effect::human_owner_hidden(&mut rotor);
                }

                self.effects.spawn(Box::new(rotor));
            }
        }

        let ground = self.effects.needs_ground().then(|| {
            shade::Ground::new(
                &self.terrain,
                &self.actors.heights,
                self.streamed
                    .iter()
                    .map(|streamed| &streamed.terrain)
                    .collect(),
            )
            .with_water(&self.sea.seas)
        });

        let roster = &self.actors.actors;

        let models = &self.actors.models;

        let owners = { self.effect_owners.prepare(roster, models) };

        let position_of = |entity| owners.entity(entity).map(|actor| actor.position);

        let body = |key| owners.snapshot(key);

        let bone = |key, bone| owners.bone(key, bone);

        let window = &self.actors.window;

        let zone = self.actors.zone;

        let height = |x, z| super::effect_context::height(window, zone, x, z);

        let viewer = self
            .actors
            .viewer
            .and_then(|id| owners.entity(id))
            .map(|actor| super::effect_context::snapshot(actor, models));

        let mesh =
            |id: i32| u32::try_from(id).is_ok_and(|id| self.objects.by_id_meshes.contains_key(&id));

        let mut live = super::effect_context::Live {
            mesh: Some(&mesh),
            body: &body,
            bone: &bone,
            height: &height,
            ground: ground.as_ref(),
            camera: self.pose,
            viewer,
            scene: self.scene,
            world: self.identity,
            zone,
            detail: if self.actors.effects_detail.effects {
                0
            } else {
                1
            },
            rate: self.actors.render_lod.rate,
            rng: &mut self.actors.effect_rng,
        };

        {
            self.effects.update_live(now_ms, &mut live, position_of);
        }

        for (entity, stamp) in self.effects.tree.take_hits() {
            if let Some(actor) = self
                .actors
                .actors
                .iter_mut()
                .find(|actor| actor.entity == entity)
            {
                actor.recent_hit_ms = Some(stamp);
            }
        }

        let started = self.effects.take_sounds();

        self.sounds.extend(started);

        self.weather_anchor = self.resolve_weather_anchor();

        self.tick_weather(now_ms);

        self.tick_ambient(now_ms);

        self.sea
            .update_environment(now_ms, self.environment.env_type);

        self.environment.apply_pitch(self.scene, self.pose.pitch);

        let view = self.pose.view();

        let projection = Projection::new(backbuffer, (0, 0), self.view_far_plane());

        let uniforms = FrameUniforms::new(
            self.mesh.world,
            self.mesh.normal_matrix,
            view.matrix(),
            projection.matrix(),
            &self.lighting(),
            &self.fog(),
        );

        self.projected = Some((view.matrix(), projection.matrix()));

        if self.scene == SCENE_FIELD {
            let camera = super::human_visibility::Camera {
                eye: view.eye,
                forward: view.at - view.eye,
                clip: Matrix4::multiply(&view.matrix(), &projection.matrix()),
                viewport: backbuffer,
                fog_end: self.fog().end,
            };

            for actor in &mut self.actors.actors {
                actor.camera_visible = camera.admits(
                    actor.position,
                    actor.presentation_scale(),
                    self.actors.human_visibility_radius,
                );
            }
        }

        self.terrain_ambient.include(&self.terrain);

        for zone in &self.streamed {
            self.terrain_ambient.include(&zone.terrain);
        }

        let view_frustum = render::cull::Frustum::from_clip(&Matrix4::multiply(
            &view.matrix(),
            &projection.matrix(),
        ));

        if self.weather_target.is_some() {
            for source in &mut self.objects.house_emitters {
                let visible = view_frustum.admits(&render::cull::Aabb {
                    min: (source.position - glam::Vec3::splat(6.0)).to_array(),
                    max: (source.position + glam::Vec3::splat(6.0)).to_array(),
                });

                let births = source.tick(
                    now_ms,
                    visible,
                    self.actors.effects_detail.effects,
                    self.environment.env_type,
                    self.current,
                    &mut self.actors.effect_rng,
                );

                for b in births.billboards {
                    self.effects.spawn(Box::new(b));
                }

                if let Some(recipe) = births.particle {
                    let (parent, children) =
                        super::burst::build(&recipe, now_ms, &mut self.actors.effect_rng);

                    for b in children {
                        self.effects.spawn(Box::new(b));
                    }

                    self.effects.spawn(Box::new(parent));
                }
            }
        }

        let lava = if self.terrain_draw_enabled() {
            self.terrain_ambient.tick(
                now_ms,
                self.actors.effects_detail.effects,
                &view_frustum,
                &mut self.actors.effect_rng,
            )
        } else {
            super::terrain_ambient::Emissions::default()
        };

        for sprite in lava.billboards {
            self.effects.spawn(Box::new(sprite));
        }

        if !lava.fires.is_empty() {
            let ground = shade::Ground::new(
                &self.terrain,
                &self.actors.heights,
                self.streamed.iter().map(|zone| &zone.terrain).collect(),
            )
            .with_water(&self.sea.seas);

            for at in lava.fires {
                self.effects.spawn(Box::new(super::fire::Fire::new(
                    at,
                    1,
                    None,
                    -1,
                    0x22331100,
                    now_ms,
                    Some(&ground),
                )));
            }
        }

        WorldFrame {
            pose: self.pose,
            view,
            projection,
            uniforms,
            stage1_enabled: detail_layer_enabled(self.env_type),
        }
    }

    pub fn object_frame(
        &self,
        vendor: render::quirk::VendorClass,
        held: &[actors::HeldDraw],
        now_ms: u32,
        frustum: &render::cull::Frustum,
    ) -> objects::Frame {
        self.object_frame_with(vendor, held, &[], now_ms, &[], frustum)
    }

    pub fn object_frame_with(
        &self,
        vendor: render::quirk::VendorClass,
        held: &[actors::HeldDraw],
        drops: &[super::drops::DropDraw],
        now_ms: u32,
        extra_meshes: &[super::effect_tree::AuthoredMesh],
        frustum: &render::cull::Frustum,
    ) -> objects::Frame {
        let mut spawned = self.effects.mesh_frame(self.pose.eye, now_ms);

        for swing in self.actors.actors.iter().flat_map(|actor| &actor.swings) {
            spawned.extend(
                swing
                    .special_meshes(now_ms, self.actors.effects_detail)
                    .into_iter()
                    .map(super::effect_tree::MeshSubmission::Authored),
            );
        }

        let mut ground = vec![&self.terrain];

        ground.extend(self.streamed.iter().map(|zone| &zone.terrain));

        let (spawned_glows, rotate_lit, textured, authored) = resolve_mesh_submissions(
            spawned,
            |x, z| lights::sample(&ground, x, z),
            |entity| {
                self.actors
                    .entity(entity)
                    .and_then(|index| self.actors.actors.get(index))
                    .is_some_and(|actor| !actors::ActorScene::hidden(actor, self.actors.viewer))
            },
            self.environment.light0,
            self.environment.fog_enabled,
        );

        let glows = match spawned_glows.is_empty() {
            true => std::borrow::Cow::Borrowed(&self.lights.glows[..]),
            false => {
                let mut all = self.lights.glows.clone();

                all.extend_from_slice(&spawned_glows);

                std::borrow::Cow::Owned(all)
            }
        };

        let sky = draws_sky(self.scene, self.env_type);

        let mut frame = self.objects.frame(
            self.pose.eye,
            vendor,
            held,
            drops,
            &glows,
            &rotate_lit,
            now_ms,
            sky,
            super::light_shafts::enabled(self.scene, &self.name),
            frustum,
        );

        self.objects.apply_sky(
            &mut frame,
            self.sky,
            now_ms,
            self.environment.uses_day_cycle(),
        );

        self.objects
            .append_authored_effects(&mut frame, extra_meshes, vendor, frustum);

        self.objects
            .append_textured_effects(&mut frame, &textured, vendor, frustum);

        self.objects
            .append_authored_effects(&mut frame, &authored, vendor, frustum);

        frame
    }

    pub fn pose_reach(&self) -> actors::Reach {
        let reach = actors::Reach {
            eye: self.pose.eye,
            far: match self.scene {
                SCENE_SELECT_CHAR | SCENE_BUDGET | SCENE_LOGIN => None,
                _ => Some(self.view_far_plane()),
            },
        };

        self.projected
            .as_ref()
            .map_or(reach, |(_, projection)| reach.with_projection(projection))
    }

    fn flush_death_particles(&mut self) {
        for (born, recipe) in std::mem::take(&mut self.actors.death_particles) {
            super::burst::spawn(
                &mut self.effects,
                &recipe,
                born,
                &mut self.actors.effect_rng,
            );
        }
    }

    fn rebuild_terrain_colours(&mut self, registered: &[usize]) {
        let terrains: Vec<_> = std::iter::once(&self.terrain)
            .chain(self.streamed.iter().map(|s| &s.terrain))
            .collect();

        refresh_terrain_colours(
            &mut self.mesh,
            &terrains,
            &mut self.terrain_revision,
            &mut self.terrain_pages,
            registered,
        );
    }

    pub fn terrain_changes_since(
        &self,
        revision: u64,
        resident: usize,
    ) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
        let limit = resident.min(self.mesh.vertices.len());

        let mut page = 0;

        std::iter::from_fn(move || {
            while page < self.terrain_pages.len() && self.terrain_pages[page] <= revision {
                page += 1;
            }

            let start = page * 256;

            if start >= limit || page == self.terrain_pages.len() {
                return None;
            }

            while page < self.terrain_pages.len() && self.terrain_pages[page] > revision {
                page += 1;
            }

            Some(start..(page * 256).min(limit))
        })
    }

    fn flush_tree_shadow_stamps(&mut self) {
        let stamps = std::mem::take(&mut self.actors.tree_shadow_stamps);

        if stamps.is_empty() {
            return;
        }

        let mut list = vec![&mut self.terrain];

        list.extend(self.streamed.iter_mut().map(|s| &mut s.terrain));

        let mut zones = lights::Zones::new(list);

        for at in stamps {
            zones.register(at.x + 2.0, at.z - 2.0, 0);
        }

        let registered = zones.registered_zones().to_vec();

        if !registered.is_empty() {
            self.rebuild_terrain_colours(&registered);
        }
    }

    fn refresh_floats(&mut self, now_ms: u32) {
        let reach = self.pose_reach();

        for float in &mut self.actors.raft.floats {
            let at = float.position;

            if !float.has_mesh() || reach.admits(at) {
                let terrain = std::iter::once(&self.terrain)
                    .chain(self.streamed.iter().map(|s| &s.terrain))
                    .find(|t| t.contains(at.x, at.z));

                let height = terrain.and_then(|t| {
                    super::floats::water_height(
                        &self.sea.seas,
                        (t.zone_x, t.zone_y),
                        at.x,
                        at.z,
                        now_ms,
                    )
                });

                let light = terrain.map_or_else(
                    || super::lights::sample::<&Terrain>(&[], at.x, at.z),
                    |t| super::lights::sample(&[t], at.x, at.z),
                );

                float.refresh(height, light);
            }

            for quad in &mut self.effects.quads {
                if quad.slot != super::quads::Slot::Placed
                    && quad.owner == float.record
                    && quad.anchor.x == at.x
                    && quad.anchor.z == at.z
                {
                    if matches!(
                        quad.slot,
                        super::quads::Slot::Glow | super::quads::Slot::FollowingGlow
                    ) {
                        if now_ms <= 500 {
                            continue;
                        }

                        quad.slot = super::quads::Slot::FollowingGlow;
                    }

                    quad.anchor = float.position;
                }
            }
        }
    }

    pub fn actor_frame(
        &mut self,
        now_ms: u32,
        view: &render::camera::Matrix4,
        vendor: render::quirk::VendorClass,
        alpha: actors::SkinAlpha,
    ) -> actors::Frame {
        self.actor_frame_reusing(now_ms, view, vendor, alpha, actors::Frame::default())
    }

    pub(crate) fn actor_frame_reusing(
        &mut self,
        now_ms: u32,
        view: &render::camera::Matrix4,
        vendor: render::quirk::VendorClass,
        alpha: actors::SkinAlpha,
        scratch: actors::Frame,
    ) -> actors::Frame {
        self.refresh_floats(now_ms);

        let fog = self.fog();

        self.actors.grass.field.enabled =
            self.actors.grass_effects_enabled() && render::skin::grass_enabled(vendor);

        self.actors
            .grass
            .field
            .refresh_light(self.environment.light0);

        self.actors.light0 = self.environment.light0;

        self.actors.light1 = self.environment.light1;

        self.actors.effect_environment = self.env_global;

        let script_epoch = self.path.start_ms;

        let cues = self
            .script
            .as_mut()
            .map(|s| s.advance(now_ms.wrapping_sub(script_epoch)))
            .unwrap_or_default();

        for (at, cue) in cues {
            let at = script_epoch.wrapping_add(at);

            match cue {
                script::Cue::Order(record) => {
                    self.actors.order(record, at);
                }
                script::Cue::Play(record, action) => self.actors.play(record, action, at),
                script::Cue::Sound(slot) => self.sounds.push(slot),
                script::Cue::Cull => self.actors.destroy(&script::CULLED),
            }
        }

        let mut ground: Vec<&Terrain> = vec![&self.terrain];

        ground.extend(self.streamed.iter().map(|s| &s.terrain));

        let skin_fog = render::skin::SkinFog {
            start: fog.start,
            end: fog.end,
            enabled: fog.enabled,
        };

        let reach = self.pose_reach();

        let mut frame = self.actors.frame_reusing(
            now_ms, view, reach.eye, reach.far, skin_fog, alpha, &ground, scratch,
        );

        self.sounds.append(&mut self.actors.tick_sounds);

        self.flush_tree_shadow_stamps();

        self.flush_death_particles();

        self.damage_sounds.extend(self.actors.take_damage_sounds());

        for actor in &mut self.actors.actors {
            for sprite in super::body_glow::emit(actor, now_ms) {
                self.effects.spawn(Box::new(sprite));
            }

            let visible = (if self.scene == SCENE_FIELD {
                actor.camera_visible
            } else {
                reach.admits(actor.position)
            }) && !actors::ActorScene::hidden(actor, self.actors.viewer);

            super::body_sprites::update_in_scene(
                actor,
                visible && actor.class != 45,
                self.env_global,
                now_ms,
                self.pose.yaw,
                self.pose.pitch,
                self.scene,
            );
        }

        self.effects
            .tree
            .control_humans(self.actors.actors.iter().filter_map(|actor| {
                let visible = (if self.scene == SCENE_FIELD {
                    actor.camera_visible
                } else {
                    reach.admits(actor.position)
                }) && !actors::ActorScene::hidden(actor, self.actors.viewer);

                let siege = super::siege_mesh::mesh(actor.parts[7].mesh)
                    .is_some_and(|id| self.actors.siege_meshes[(id - 2888) as usize]);

                (!visible || (actor.class != 45 && !siege)).then_some((
                    super::effect_context::ActorKey::of(actor),
                    super::effect_tree::HumanControl::DrawVisibility(visible),
                ))
            }));

        if !self.actors.actors.is_empty() {
            let ground = shade::Ground::new(
                &self.terrain,
                &self.actors.heights,
                self.streamed
                    .iter()
                    .map(|streamed| &streamed.terrain)
                    .collect(),
            );

            super::body_shadows::update(
                &mut self.actors.actors,
                &self.actors.footprint_foliage,
                &ground,
                self.actors.effects_detail.effects,
                self.scene,
                now_ms,
            );

            for sprite in super::movement_dust::emit(
                &mut self.actors.actors,
                &self.actors.footprint_foliage,
                &ground,
                self.scene,
                self.weather.code,
                now_ms,
            ) {
                self.effects.spawn(Box::new(sprite));
            }
        }

        if self.actors.actors.iter().any(|a| matches!(a.action, 2 | 3)) {
            let ground = shade::Ground::new(
                &self.terrain,
                &self.actors.heights,
                self.streamed
                    .iter()
                    .map(|streamed| &streamed.terrain)
                    .collect(),
            );

            for mark in super::footprints::emit(
                &mut self.actors.actors,
                &self.actors.footprint_foliage,
                &ground,
                self.scene,
                now_ms,
            ) {
                self.effects.spawn(Box::new(mark));
            }
        }

        if frame.afterimage_sound {
            self.sounds.push(160);
        }

        if !frame.ground_marks.is_empty() {
            let ground = shade::Ground::new(
                &self.terrain,
                &self.actors.heights,
                self.streamed
                    .iter()
                    .map(|streamed| &streamed.terrain)
                    .collect(),
            );

            for mut mark in frame.ground_marks.drain(..) {
                mark.place(&ground);

                self.effects.spawn(Box::new(mark));
            }
        }

        if !frame.skin_fires.is_empty() {
            let ground = shade::Ground::new(
                &self.terrain,
                &self.actors.heights,
                self.streamed
                    .iter()
                    .map(|streamed| &streamed.terrain)
                    .collect(),
            );

            for at in frame.skin_fires.drain(..) {
                self.effects.spawn(Box::new(super::fire::Fire::new(
                    at,
                    0,
                    None,
                    -1,
                    0x22331100,
                    now_ms,
                    Some(&ground),
                )));
            }
        }

        self.effects.billboards.append(&mut frame.spawned);

        super::actor_effects::haste_trails(self, now_ms);

        super::actor_effects::water_ripples(self, now_ms);

        super::actor_effects::hand_sprites(&self.actors, &mut self.effects, now_ms, true);

        for spark in frame.sparks.drain(..) {
            self.effects.spawn(Box::new(spark));
        }

        for built in frame.poison.drain(..) {
            if let Some(sound) = built.sound {
                self.sounds.push(sound);
            }

            let mut nodes: Vec<Box<dyn super::effect_tree::Effect>> = Vec::new();

            super::skill::poison::push(built, &mut nodes);

            for node in nodes {
                self.effects.tree.spawn(node);
            }
        }

        let pulse_plans = self.actors.take_pulse_plans();

        if !pulse_plans.is_empty() {
            let ground = shade::Ground::new(
                &self.terrain,
                &self.actors.heights,
                self.streamed
                    .iter()
                    .map(|streamed| &streamed.terrain)
                    .collect(),
            );

            for plan in pulse_plans {
                spawn_pulse_plan(
                    &mut self.effects,
                    &mut self.actors.effect_rng,
                    &ground,
                    plan,
                    now_ms,
                );
            }
        }

        frame
    }

    pub fn terrains(&self) -> Vec<&Terrain> {
        let mut all: Vec<&Terrain> = vec![&self.terrain];

        all.extend(self.streamed.iter().map(|s| &s.terrain));

        all
    }

    pub fn stream(&mut self, game: &GameDir, dir: Direction) -> Result<bool> {
        let Some((zone, zone_name)) = neighbour_of(self.current, dir) else {
            return Ok(false);
        };

        let loaded = self.load_zone(game, zone, zone_name);

        self.restore_stream_collision(zone, loaded.as_ref().is_ok_and(|&loaded| loaded));

        loaded
    }

    pub fn stream_background(&mut self, game: &GameDir, dir: Direction) -> Result<bool> {
        self.stream_background_staged(game, dir, &mut |_, _| true)
    }

    pub(crate) fn stream_background_staged(
        &mut self,
        game: &GameDir,
        dir: Direction,
        stage: &mut impl FnMut(u64, &loading::Prepared) -> bool,
    ) -> Result<bool> {
        let Some((zone, _)) = neighbour_of(self.current, dir) else {
            return Ok(false);
        };

        let loaded = self.load_zone_background(game, zone, stage);

        self.restore_stream_collision(zone, loaded.as_ref().is_ok_and(|&loaded| loaded));

        loaded
    }

    fn restore_stream_collision(&mut self, zone: (u8, u8), loaded: bool) {
        let old = self.actors.zone;

        let axis = |anchor: u8, current: u8, neighbour: u8| {
            let low = current.min(neighbour);

            let high = current.max(neighbour);

            if low < anchor || high > anchor.saturating_add(1) {
                low
            } else {
                anchor
            }
        };

        let anchor = (
            axis(old.0, self.current.0, zone.0),
            axis(old.1, self.current.1, zone.1),
        );

        if anchor != old || loaded || self.visual_collision_dirty {
            self.actors.rewindow(anchor);

            self.visual_collision_dirty = false;
        }
    }

    pub(crate) fn load_zone_background(
        &mut self,
        game: &GameDir,
        zone: (u8, u8),
        stage: &mut impl FnMut(u64, &loading::Prepared) -> bool,
    ) -> Result<bool> {
        if self.visual_zone.is_some() {
            return Ok(false);
        }

        let loaded = self.load_visual_zone_background(game, zone, stage)?;

        if loaded && self.force_load == Some(zone) {
            self.actors.rewindow(self.actors.zone);

            self.visual_collision_dirty = false;
        }

        Ok(loaded)
    }

    fn load_visual_zone_background(
        &mut self,
        game: &GameDir,
        zone: (u8, u8),
        stage: &mut impl FnMut(u64, &loading::Prepared) -> bool,
    ) -> Result<bool> {
        if self.holds(zone) {
            return Ok(false);
        }

        if self
            .background_retry
            .is_some_and(|until| web_time::Instant::now() < until)
        {
            return Ok(false);
        }

        if self.background_loader.is_none() {
            match loading::Loader::new(game.clone(), self.tile_list.clone()) {
                Ok(loader) => self.background_loader = Some(loader),
                Err(error) => {
                    tracing::warn!(% error, "cannot start zone loader; will retry");

                    self.background_retry =
                        Some(web_time::Instant::now() + std::time::Duration::from_secs(1));

                    return Ok(false);
                }
            }
        }

        let result = self.background_loader.as_mut().unwrap().take_or_stage(
            zone,
            &self.tiles,
            &self.missing,
            || {
                Some(loading::Inputs {
                    registries: self.objects.registries.clone(),
                    actors: self.actors.resource_snapshot(),
                    meshes: self.objects.meshes.iter().map(|m| m.id).collect(),
                    scene: self.lamps.scene(),
                })
            },
            &mut |prepared| stage(self.identity, prepared),
        );

        let prepared = match result {
            Ok(Some(prepared)) => prepared,
            Ok(None) => return Ok(false),
            Err(error) => {
                tracing::warn!(% error, "zone loader stopped; will retry");

                self.background_loader = None;

                self.background_retry =
                    Some(web_time::Instant::now() + std::time::Duration::from_secs(1));

                return Ok(false);
            }
        };

        let start = web_time::Instant::now();

        self.commit_zone(
            game,
            zone,
            format!("Field{:02}{:02}", zone.0, zone.1),
            prepared,
        )?;

        tracing::debug!(
            ?zone,
            elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
            "prepared zone committed on main thread"
        );

        Ok(true)
    }

    fn load_zone(&mut self, game: &GameDir, zone: (u8, u8), zone_name: String) -> Result<bool> {
        if self.holds(zone) {
            return Ok(false);
        }

        let resident: Vec<_> = self.tiles.iter().map(|t| t.id).collect();

        let Some(prepared) =
            loading::prepare(game, &self.tile_list, zone, &resident, &self.missing)?
        else {
            return Ok(false);
        };

        self.commit_zone(game, zone, zone_name, prepared)
    }

    fn commit_zone(
        &mut self,
        game: &GameDir,
        zone: (u8, u8),
        zone_name: String,
        prepared: loading::Prepared,
    ) -> Result<bool> {
        if self.holds(zone) {
            return Ok(false);
        }

        let commit_start = web_time::Instant::now();

        let loading::Prepared {
            mut terrain,
            mesh,
            records,
            tiles,
            resources,
            ..
        } = prepared;

        let prepared_resources = resources.is_some();

        let grids = resources.map(|r| {
            self.objects.install_resources(r.objects);

            self.actors.install_resources(r.actors);

            (r.heights, r.relief)
        });

        let first = self.objects.neighbour_records.len();

        self.objects
            .append_neighbour_resources(game, records, zone, !prepared_resources)
            .with_context(|| format!("building {zone_name}'s objects"))?;

        let objects_done = commit_start.elapsed();

        let (lights, mut registered) = {
            let scene = self.lamps.scene();

            let mut list: Vec<&mut Terrain> = vec![&mut self.terrain];

            list.extend(self.streamed.iter_mut().map(|s| &mut s.terrain));

            list.push(&mut terrain);

            let mut zones = lights::Zones::new(list);

            let lights = lights::build(
                &mut zones,
                &mut self.lamps,
                &self.objects.neighbour_records[first..],
                zone,
                scene,
            );

            (lights, zones.registered_zones().to_vec())
        };

        self.lights.registrations.extend(lights.registrations);

        self.lights.glows.extend(lights.glows.iter().cloned());

        let dx =
            (i32::from(zone.0) - i32::from(self.terrain.zone_x)) * formats::world::TRN_DIM as i32;

        let dz =
            (i32::from(zone.1) - i32::from(self.terrain.zone_y)) * formats::world::TRN_DIM as i32;

        self.mesh.append_neighbour(&mesh, dx, dz);

        let appended = self.streamed.len() + 1;

        if !registered.contains(&appended) {
            registered.push(appended);
        }

        let terrains: Vec<_> = std::iter::once(&self.terrain)
            .chain(self.streamed.iter().map(|s| &s.terrain))
            .chain(std::iter::once(&terrain))
            .collect();

        refresh_terrain_colours(
            &mut self.mesh,
            &terrains,
            &mut self.terrain_revision,
            &mut self.terrain_pages,
            &registered,
        );

        for tile in tiles {
            if self.tiles.iter().all(|old| old.id != tile.id) {
                self.tiles.push(tile);
            }
        }

        self.tiles.sort_unstable_by_key(|t| t.id);

        self.missing = self
            .mesh
            .batches
            .iter()
            .flat_map(|b| {
                std::iter::once(b.base_texture_id)
                    .chain(b.material.needs_detail().then_some(b.detail_texture_id))
            })
            .filter(|id| self.tiles.binary_search_by_key(id, |t| t.id).is_err())
            .collect();

        self.missing.sort_unstable();

        self.missing.dedup();

        let terrain_done = commit_start.elapsed();

        let first_float = self.actors.raft.floats.len();

        self.actors
            .add_prepared_neighbour(
                game,
                &terrain,
                &self.objects.neighbour_records[first..],
                self.environment.light0,
                self.environment.light1,
                grids,
            )
            .with_context(|| format!("building {zone_name}'s trees"))?;

        let actors_done = commit_start.elapsed();

        for float in &self.actors.raft.floats[first_float..] {
            self.effects
                .quads
                .extend(float.quads(self.effects.quads.len()));
        }

        if !prepared_resources && !lights.glows.is_empty() {
            let ids: Vec<u32> = lights.glows.iter().map(|g| g.mesh_id as u32).collect();

            self.objects
                .load_by_id(game, &ids)
                .with_context(|| format!("loading {zone_name}'s glow mesh"))?;
        }

        self.effects.billboards.extend(emitters::build(
            &self.objects.neighbour_records[first..],
            zone,
        ));

        self.ambient.extend(dust::build_sources(
            &self.objects.neighbour_records[first..],
            zone,
        ));

        self.drips.extend(drip::build_columns(
            &self.objects.neighbour_records[first..],
            zone,
            &mut self.drip_rng,
        ));

        self.sea.seas.extend(sea::build(
            &self.objects.neighbour_records[first..],
            zone,
            self.scene,
        ));

        let mut decals = lights.shades;

        let heights = self.terrain.build_height_grid();

        {
            let mut ground: Vec<&Terrain> = vec![&self.terrain];

            ground.extend(self.streamed.iter().map(|s| &s.terrain));

            ground.push(&terrain);

            let ground = shade::Ground::new(&self.terrain, &heights, ground);

            for decal in &mut decals {
                decal.place(&ground);
            }
        }

        self.lights.shades.extend(decals.iter().cloned());

        self.effects.shades.extend(decals);

        self.streamed.push(Streamed {
            zone,
            name: zone_name,
            relief: terrain.build_height_grid(),
            terrain,
        });

        self.visual_prefetch.invalidate_residency();

        tracing::debug!(
            ?zone,
            objects_ms = objects_done.as_secs_f64() * 1000.0,
            lighting_terrain_ms = (terrain_done - objects_done).as_secs_f64() * 1000.0,
            actors_collision_ms = (actors_done - terrain_done).as_secs_f64() * 1000.0,
            effects_ms = (commit_start.elapsed() - actors_done).as_secs_f64() * 1000.0,
            "zone commit phases"
        );

        Ok(true)
    }

    pub fn reanchor(
        &mut self,
        game: &GameDir,
        zone: (u8, u8),
        here: (i32, i32),
        now_ms: u32,
    ) -> Result<bool> {
        if self.current == zone {
            return Ok(false);
        }

        let loaded = self.load_zone(game, zone, format!("Field{:02}{:02}", zone.0, zone.1))?;

        if !self.holds(zone) {
            return Ok(false);
        }

        self.current = zone;

        let anchor = self.actors.zone;

        let step = (
            i32::from(zone.0) - i32::from(anchor.0),
            i32::from(zone.1) - i32::from(anchor.1),
        );

        let inside = matches!(step, (0, 0) | (1, 0) | (0, 1));

        if !inside {
            self.actors.rewindow(zone);
        } else if loaded || self.visual_collision_dirty {
            self.actors.rewindow(anchor);
        }

        self.visual_collision_dirty = false;

        self.environment = Environment::for_day(FIELD_DAY, zone.0, zone.1).for_scene(self.scene);

        self.env_type = environment_type(zone.0, zone.1);

        self.env_global = self.env_type;

        let _yaw_reset_applies = self
            .weather
            .warp(&mut self.sky, self.env_global, here, now_ms);

        let blend = self.sky.update_blend(now_ms);

        self.apply_day(blend);

        Ok(true)
    }

    pub fn apply_day(&mut self, blend: sky::Blend) {
        if let (Some(sun), Some(scale)) = (self.sun.as_mut(), blend.flare_scale)
            && self.environment.uses_day_cycle()
        {
            sun.scale = scale;
        }

        self.environment.apply_day(blend);
    }

    pub fn flare_suppressed(&self) -> bool {
        self.sun.is_none() || self.environment.suppresses_flare || self.sky.flare_suppressed()
    }

    pub fn rebase(&mut self, dir: Direction) -> bool {
        let Some((zone, _)) = neighbour_of(self.current, dir) else {
            return false;
        };

        if !self.holds(zone) {
            return false;
        }

        self.restore_stream_collision(zone, false);

        self.current = zone;

        true
    }

    pub fn relief_height(&self, x: f32, z: f32) -> Option<f32> {
        if self.terrain.contains(x, z) {
            return Some(actors::ground_height(
                &self.actors.relief,
                &self.terrain,
                x,
                z,
            ));
        }

        self.streamed
            .iter()
            .find(|zone| zone.terrain.contains(x, z))
            .map(|zone| actors::ground_height(&zone.relief, &zone.terrain, x, z))
    }

    pub(crate) fn gate_height(&mut self, game: &GameDir, x: f32, z: f32) -> Result<Option<f32>> {
        if !x.is_finite() || !z.is_finite() || x < 0.0 || z < 0.0 || x >= 4096.0 || z >= 4096.0 {
            return Ok(None);
        }

        let zone = ((x / 128.0) as u8, (z / 128.0) as u8);

        if !self.holds(zone) {
            if let Some(dir) = Direction::ALL
                .into_iter()
                .find(|dir| neighbour_of(self.current, *dir).is_some_and(|(at, _)| at == zone))
            {
                self.stream(game, dir)?;
            }
        }

        if zone == (self.terrain.zone_x, self.terrain.zone_y) {
            return Ok(Some(actors::ground_height(
                &self.actors.relief,
                &self.terrain,
                x,
                z,
            )));
        }

        Ok(self
            .streamed
            .iter()
            .find(|s| s.zone == zone)
            .map(|s| actors::ground_height(&s.terrain.build_height_grid(), &s.terrain, x, z)))
    }

    pub fn neighbour(&self) -> Option<&Terrain> {
        self.streamed.first().map(|s| &s.terrain)
    }

    pub fn holds(&self, zone: (u8, u8)) -> bool {
        (self.terrain.zone_x, self.terrain.zone_y) == zone
            || self.streamed.iter().any(|s| s.zone == zone)
    }

    pub fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    pub fn take_damage_sounds(&mut self) -> Vec<skill_anim::DamageSoundRequest> {
        std::mem::take(&mut self.damage_sounds)
    }

    pub fn finish_damage_sound(
        &mut self,
        request: skill_anim::DamageSoundRequest,
        played: bool,
    ) -> bool {
        self.actors.finish_damage_sound(request, played)
    }

    pub fn play_sound(&mut self, slot: i64) {
        self.sounds.push(slot);
    }

    pub fn effect_frame(&self, now_ms: u32) -> effects::Frame {
        let mut frame = self.effects.frame(self.pose_reach(), now_ms);

        self.sky_ornaments.submit(
            self.sky,
            self.scene,
            self.environment.env_type,
            &self.effects,
            self.pose.eye,
            now_ms,
            &mut frame,
        );

        super::body_shadows::submit(
            &self.actors,
            &self.effects,
            self.pose.eye,
            now_ms,
            &mut frame,
        );

        super::body_sprites::submit(
            &self.actors,
            &self.effects,
            self.pose.eye,
            now_ms,
            &mut frame,
        );

        frame
    }

    pub fn fall<'a>(&'a self, view: &'a render::camera::Matrix4) -> weather::Fall<'a> {
        weather::Fall {
            emitters: self.weather.emitters,
            rain: &self.rain,
            snow: [&self.snow, &self.snow_fast],
            anchor: self.weather_anchor,
            map: self.actors.attributes(),
            view,
        }
    }

    pub fn effect_frame_with_weather(
        &self,
        now_ms: u32,
        view: &render::camera::Matrix4,
    ) -> effects::Frame {
        let mut frame = self.effect_frame(now_ms);

        self.effects.append_weather_of(&mut frame, &self.fall(view));

        self.effects.append_drips(&mut frame, &self.drips);

        frame
    }

    pub fn lighting(&self) -> Lighting {
        Lighting {
            point_lights: if matches!(self.sky.day, 0 | 2 | 12) {
                render::world::PointLights::default()
            } else {
                self.lights.point_lights_at(
                    self.pose.eye,
                    self.camera_distance,
                    self.pose.pitch,
                    self.scene,
                )
            },
            light0: self
                .actors
                .body_light
                .apply_primary_light_suppression(self.scene, self.environment.light0),
            ..self.environment.lighting()
        }
    }

    pub fn sea_frame(&self) -> sea::Frame {
        self.sea.frame(self.pose_reach())
    }

    pub fn sea_uniforms(&self, frame: &WorldFrame) -> FrameUniforms {
        let mut lighting = self.lighting();

        lighting.material = render::sea::SeaMaterial::MATERIAL;

        let mut fog = self.fog();

        fog.enabled = false;

        FrameUniforms::new(
            render::camera::Matrix4::IDENTITY,
            render::camera::Matrix4::IDENTITY,
            frame.view.matrix(),
            frame.projection.matrix(),
            &lighting,
            &fog,
        )
    }

    pub fn skin_frame(&self, frame: &WorldFrame) -> render::skin::SkinFrame {
        let fog = self.fog();
        let original = self.camera_environment().fog_at(self.camera_distance);

        let [r, g, b, _] = render::world::argb_to_rgba_f32(fog.color);

        render::skin::SkinFrame::new(
            &frame.projection.matrix(),
            &frame.view.matrix(),
            [r, g, b, 1.0],
        )
        .with_fog_limit((fog.end > original.end).then_some(fog.end))
    }

    pub fn tile_layers(&self) -> HashMap<i32, u32> {
        self.tiles
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i as u32))
            .collect()
    }

    pub fn upload_tiles(&self, gpu: &Gpu) -> Option<render::texture::GpuTexture> {
        let textures: Vec<&Texture> = self.tiles.iter().map(|t| &t.texture).collect();

        render::texture::upload_material_array(
            gpu,
            &textures,
            AlphaPolicy::Opaque,
            render::texture::MipPolicy::Opaque,
            "Env tiles",
        )
    }

    pub fn world_matrix(&self) -> Matrix4 {
        self.mesh.world
    }
}

use super::effect_mesh::EffectMesh;
use super::field::FieldScene;
use super::shade::Shade;
use glam::Vec3;

pub const ACTIVE_MODE: i32 = 1;

pub const MESH_ID: i32 = 0x1FA;

pub const MESH_ARGB: u32 = 0xFFFF_FFFF;

pub const MESH_YAW: f32 = 0.0;

pub const MESH_MODE: i32 = 0;

pub const BLEND: u32 = render::mesh::EFFECT_ADDITIVE;

pub const STEP: f32 = 0.2;

pub const STEP_QWORD: f64 = 0.200_000_002_980_232_24;

pub const SCALE_XZ: f32 = 1.0;

pub const SCALE_Y: f32 = 2.5;

pub const PULSE: bool = true;

pub const PERIOD_MS: u32 = 3_000;

pub const LIFETIME_MS: u32 = 8_000;

pub const COLUMNS: [i32; 3] = [-1, 0, 1];

pub const SHADE_GRID: i32 = 3;

pub const SHADE_SLOT: usize = 0x76;

pub const SHADE_ARGB: u32 = 0xFFFF_FFFF;

pub const SHADE_MS: u32 = 8_000;

pub const SOUND: i64 = 1;

pub fn column_at(body: Vec3, i: i32) -> Vec3 {
    let offset = i as f32 * STEP;

    Vec3::new(body.x + offset, body.y, body.z + offset)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Burst {
    pub entity: u16,
    pub index: Option<usize>,
    pub mode: i32,
    pub raised: bool,
    pub columns: Vec<Vec3>,
    pub shade: Option<Vec3>,
}

pub fn apply(scene: &mut FieldScene, entity: u16, mode: i32) -> Burst {
    let Some(index) = scene.world.actors.entity(entity) else {
        return Burst {
            entity,
            index: None,
            mode,
            raised: false,
            columns: Vec::new(),
            shade: None,
        };
    };

    if mode != ACTIVE_MODE {
        return Burst {
            entity,
            index: Some(index),
            mode,
            raised: false,
            columns: Vec::new(),
            shade: None,
        };
    }

    let now_ms = scene.time.now_ms;

    let body = scene.world.actors.actors[index].position;

    let mut columns = Vec::with_capacity(COLUMNS.len());

    for i in COLUMNS {
        let at = column_at(body, i);

        let mut mesh = EffectMesh::new(MESH_ID, MESH_ARGB, MESH_YAW, MESH_MODE, now_ms);

        mesh.blend = BLEND;

        mesh.scale_xz = SCALE_XZ;

        mesh.scale_y = SCALE_Y;

        mesh.pulse = PULSE;

        mesh.period_ms = PERIOD_MS;

        mesh.lifetime_ms = LIFETIME_MS;

        mesh.position = at;

        scene.world.effects.spawn(Box::new(mesh));

        columns.push(at);
    }

    let shade = ground_shade(scene, SHADE_GRID, SHADE_SLOT, SHADE_ARGB, body, SHADE_MS);

    let centre = shade.centre;

    scene.world.effects.spawn(Box::new(shade));

    scene.world.play_sound(SOUND);

    Burst {
        entity,
        index: Some(index),
        mode,
        raised: true,
        columns,
        shade: Some(centre),
    }
}

fn ground_shade(
    scene: &FieldScene,
    grid: i32,
    texture: usize,
    argb: u32,
    centre: Vec3,
    lifetime_ms: u32,
) -> Shade {
    let mut shade = Shade::new(
        grid,
        texture,
        argb,
        centre,
        super::effects::SECTOR_UNASSIGNED,
    )
    .with_lifetime(lifetime_ms);

    let heights = &scene.world.actors.heights;

    let mut ground: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

    ground.extend(scene.world.streamed.iter().map(|s| &s.terrain));

    shade.place(&super::shade::Ground::new(
        &scene.world.terrain,
        heights,
        ground,
    ));

    shade
}

pub fn describe(applied: &Burst) -> String {
    match (applied.index, applied.raised) {
        (None, _) => {
            format!(
                "NWYD: 0x3AD for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        (Some(_), false) => {
            format!(
                "NWYD: 0x3AD — entity {}, mode {} is not {ACTIVE_MODE}, nothing raised",
                applied.entity, applied.mode,
            )
        }
        (Some(_), true) => {
            format!(
                "NWYD: 0x3AD — entity {}, {} glow columns + a decal + amb01.wav for {}ms",
                applied.entity,
                applied.columns.len(),
                LIFETIME_MS,
            )
        }
    }
}

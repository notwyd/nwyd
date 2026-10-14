use super::{
    actors::Actor,
    shade::{Ground, Shade},
};
use formats::world::{objects::ObjRecord, trn::Terrain};
use glam::Vec3;

pub(crate) fn foliage(records: &[ObjRecord]) -> Vec<bool> {
    let mut flags = vec![false; 128 * 128];

    for r in records.iter().filter(|r| (0x137..=0x142).contains(&r.kind)) {
        let [x, _, z] = r.world_position(0, 0);

        if !x.is_finite() || !z.is_finite() {
            continue;
        }

        for zz in [z, z + 1.0] {
            for xx in [x, x + 1.0] {
                let (col, row) = ((xx as i32) % 128, (zz as i32) % 128);

                if col >= 0 && row >= 0 {
                    flags[row as usize * 128 + col as usize] = true;
                }
            }
        }
    }

    flags
}

fn foot_type(model: u32, mount_part: i16) -> Option<u8> {
    Some(match model {
        0 | 1 | 2 | 3 | 6 | 10 | 22 => 1,
        4 | 7 | 21 | 25 | 28 | 29 | 30 | 31 | 38 | 39 => 2,
        26 | 27 | 34 | 36 => 3,
        11 | 42 => 4,
        35 => 5,
        8 | 12 | 20 | 23 | 24 | 32 | 40 | 43 | 44 | 69 => 0,
        5 => u8::from(mount_part == 1),
        _ => return None,
    })
}

fn period(scale: f32, speed: f32, kind: u8, model: u32) -> u32 {
    let mut ms = (scale * 1000.0 / speed) as u32;

    if ms.wrapping_sub(231) < 119 {
        ms = 230;
    } else if ms > 340 {
        ms = 340;
    }

    if kind == 3 {
        ms = 200;
    }

    if matches!(model, 11 | 39) {
        ms *= 2;
    }

    ms
}

pub(crate) fn refresh_type(actor: &mut Actor) {
    let model = if actor.mounted {
        actor.mount_model.unwrap_or(u32::MAX)
    } else {
        actor.model_id
    };

    let mount_part = actor.mount.as_ref().map_or(-1, |m| m.parts[0].mesh);

    if let Some(kind) = foot_type(model, mount_part) {
        actor.footprint_kind = kind;
    }
}

fn recipe(actor: &mut Actor, now: u32, terrain_kind: u8) -> Option<Shade> {
    refresh_type(actor);

    let kind = actor.footprint_kind;

    if actor.class == 45
        || actor.flags.get(0xB55) != 0
        || kind == 0
        || !matches!(actor.action, 2 | 3)
        || now.wrapping_sub(actor.footprint_ms)
            <= period(actor.scale, actor.speed, kind, actor.model_id)
    {
        return None;
    }

    if !matches!(terrain_kind, 8 | 9) {
        return None;
    }

    let region = (
        (actor.position.x as i32) >> 7,
        (actor.position.z as i32) >> 7,
    );

    if terrain_kind == 8 && (27..=30).contains(&region.0) && (21..=24).contains(&region.1) {
        return None;
    }

    let mut colour = if terrain_kind == 8 {
        0xFFFF8866
    } else {
        0x990000FF
    };

    let (texture, mut grid) = match kind {
        2 => (195, if actor.scale > 1.3 { 3 } else { 2 }),
        3 => {
            colour = 0xFFFFAA88;

            (196, 3)
        }
        4 => (197, 4),
        5 => (198, 4),
        _ if actor.mounted => (195, 2),
        _ => (194, if actor.scale > 1.5 { 3 } else { 2 }),
    };

    if actor.model_id == 39 {
        grid = 4;
    }

    let mut mark = Shade::new(grid, texture, colour, actor.position, -1).with_lifetime(3000);

    mark.pass = render::billboard::BillboardPass::Blended;

    mark.cosine_fade = true;

    mark.alpha_only = true;

    mark.render_with_effects_disabled = true;

    mark.rotation = actor.yaw - std::f32::consts::FRAC_PI_2;

    mark.birth_ms = Some(now);

    actor.footprint_ms = now;

    Some(mark)
}

pub(crate) fn terrain_kind(zone: &Terrain, flags: Option<&Vec<bool>>, at: Vec3) -> u8 {
    if !at.is_finite() || !zone.contains(at.x, at.z) {
        return 0;
    }

    let (ox, oz) = zone.origin();

    let (x, z) = ((at.x - ox) as usize, (at.z - oz) as usize);

    if flags.and_then(|f| f.get(z * 128 + x)).copied() == Some(true) {
        return 0;
    }

    match zone.cells.get((z / 2) * 64 + x / 2).map(|c| c.tile0) {
        Some(192..=195) => 8,
        Some(220..=221) => 9,
        _ => 0,
    }
}

pub(crate) fn geometry_height(zone: &Terrain, at: Vec3) -> Option<f32> {
    if !at.is_finite() || !zone.contains(at.x, at.z) {
        return None;
    }

    let (ox, oz) = zone.origin();

    let (x, z) = ((at.x - ox) * 0.5, (at.z - oz) * 0.5);

    let (col, row) = (x as usize, z as usize);

    let (u, v) = (x.fract(), z.fract());

    let h = |r: usize, c: usize| {
        zone.cells
            .get(r.clamp(1, 63) * 64 + c.clamp(1, 63))
            .map(|c| c.height as f32 * 0.1)
    };

    let (a, b, c, d) = (
        h(row, col)?,
        h(row, col + 1)?,
        h(row + 1, col)?,
        h(row + 1, col + 1)?,
    );

    Some(if u + v <= 1.0 {
        a + (b - a) * u + (c - a) * v
    } else {
        d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
    })
}

pub(crate) fn emit(
    actors: &mut [Actor],
    foliage: &std::collections::BTreeMap<(u8, u8), Vec<bool>>,
    ground: &Ground<'_>,
    scene: u32,
    now: u32,
) -> Vec<Shade> {
    let mut marks = Vec::new();

    for actor in actors {
        let Some(zone) = std::iter::once(ground.zone)
            .chain(ground.neighbours.iter().copied())
            .find(|zone| zone.contains(actor.position.x, actor.position.z))
        else {
            continue;
        };

        let kind = terrain_kind(
            zone,
            foliage.get(&(zone.zone_x, zone.zone_y)),
            actor.position,
        );

        if kind == 9
            && scene == 30000
            && geometry_height(ground.zone, actor.position).unwrap_or(-10000.0) as i32
                != actor.position.y as i32
        {
            continue;
        }

        if let Some(mut mark) = recipe(actor, now, kind) {
            mark.place(ground);

            marks.push(mark);
        }
    }

    marks
}

use super::{
    actors::{Actor, ActorScene},
    effect_tree::{Effect, Sink, Submit},
    effects::{Billboard, BillboardScene},
};
use glam::Vec3;

#[derive(Debug, Clone, Default)]
pub struct State {
    epoch: Option<u32>,
    pub sprites: Vec<Attachment>,
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub slot: u16,
    pub billboard: Billboard,
    body_slot: usize,
    lift: f32,
    retail_size: Vec3,
    published: bool,
    disabled: bool,
}

fn add(
    out: &mut Vec<Attachment>,
    a: &Actor,
    slot: u16,
    body_slot: usize,
    lift: f32,
    texture: usize,
    frames: u32,
    size: Vec3,
    colour: u32,
    mode: u32,
) {
    let mut b = Billboard::new(texture, 0, size, 0.0, frames, 80);

    b.pass = render::billboard::BillboardPass::Additive;

    b.colour_mode = mode;

    b.set_color(colour);

    if let Some(point) = a.body_points.points[body_slot] {
        b.position = point;
    }

    b.phase_seed = u32::from(slot);

    out.push(Attachment {
        slot,
        billboard: b,
        body_slot,
        lift,
        retail_size: size,
        published: a.body_points.points[body_slot].is_some(),
        disabled: false,
    });
}

pub(crate) fn refresh(a: &mut Actor, environment: i32) {
    if a.body_sprites.epoch == Some(a.appearance_epoch) {
        return;
    }

    let mut out = Vec::new();

    let p = a.parts[0];

    let scale = a.scale;

    match a.class {
        36 | 37 => {
            let variant = match a.parts[1].mesh {
                11 => 1,
                10 => 2,
                _ => {
                    if a.class == 37 {
                        3
                    } else {
                        0
                    }
                }
            };

            let colour = [0xFF005555, 0xFF885500, 0xFF550000, 0xFF005500][variant];

            for (i, body_slot) in [8, 9, 1, 6, 7, 2, 3].into_iter().enumerate() {
                if i == 2 && matches!(variant, 1 | 2) {
                    continue;
                }

                if i > 4 && variant != 2 {
                    break;
                }

                let x = [0.2, 0.2, 0.8, 0.6, 0.6, 0.8, 0.8][i] * scale;

                let y = [0.3, 0.3, 1.0, 0.8, 0.8, 1.0, 1.0][i] * scale;

                let lift = if i < 3 {
                    0.0
                } else if i < 5 {
                    0.1
                } else {
                    0.3
                };

                add(
                    &mut out,
                    a,
                    0x870 + i as u16 * 4,
                    body_slot,
                    lift,
                    101,
                    8,
                    Vec3::new(x, y, x),
                    colour,
                    0,
                );
            }
        }
        34 | 23 | 21 if a.class != 21 || p.mesh == 10 => {
            let (count, texture, colour) = match a.class {
                23 => (2, 71, 0xFF33FF66),
                21 => (1, 60, 0xFFEE8800),
                _ => (1, 71, 0xFFFFAAFF),
            };

            for i in 0..count {
                add(
                    &mut out,
                    a,
                    0x870 + i as u16 * 4,
                    i,
                    0.0,
                    texture,
                    1,
                    Vec3::ONE,
                    colour,
                    0,
                );
            }
        }
        29 if p.mesh == 1 => add(&mut out, a, 0x870, 0, 0.0, 71, 1, Vec3::ONE, 0xFFFF00FF, 0),
        32 if p.mesh == 2 => {
            for i in 0..6 {
                add(
                    &mut out,
                    a,
                    0x870 + i as u16 * 4,
                    i,
                    0.5,
                    11,
                    8,
                    Vec3::new(scale * 1.2, scale * 1.8, scale * 1.2),
                    0xFFAA8800,
                    0,
                );
            }
        }
        16 if p.mesh == 0 && p.tex == 1 && a.body_item_tier != 4 => {
            for i in 0..2 {
                add(
                    &mut out,
                    a,
                    0x890 + i as u16 * 4,
                    i + 8,
                    0.0,
                    101,
                    8,
                    Vec3::splat(scale * 0.4),
                    0xFF88FFAA,
                    0,
                );
            }
        }
        39 => {
            for (i, body_slot) in [6, 7, 2, 3].into_iter().enumerate() {
                add(
                    &mut out,
                    a,
                    0x870 + i as u16 * 4,
                    body_slot,
                    0.0,
                    if i < 2 { 56 } else { 101 },
                    if i < 2 { 1 } else { 8 },
                    Vec3::splat(scale),
                    0xFFFF5500,
                    0,
                );
            }
        }
        38 if (a.kingdom as i8) > 0 => {
            for i in 0..6 {
                let x = [0.2, 0.8, 0.6, 0.6, 0.8, 0.8][i] * scale;

                let y = [0.3, 1.0, 0.8, 0.8, 1.0, 1.0][i] * scale;

                let colour = if a.parts[7].mesh == 930 {
                    0xFF008855
                } else {
                    0xFF005588
                };

                add(
                    &mut out,
                    a,
                    0x874 + i as u16 * 4,
                    i + 1,
                    0.0,
                    123,
                    1,
                    Vec3::new(x, y, x),
                    colour,
                    2,
                );
            }
        }
        33 if p.mesh == 1 && environment == 2 => {
            for i in 0..7 {
                add(
                    &mut out,
                    a,
                    0x870 + i as u16 * 4,
                    i,
                    0.3,
                    101,
                    8,
                    Vec3::new(scale * 2.0, scale * 3.0, scale * 2.0),
                    0xFFFF5500,
                    0,
                );
            }
        }
        _ => {}
    }

    a.body_sprites = State {
        epoch: Some(a.appearance_epoch),
        sprites: out,
    };
}

pub(crate) fn has_primary(a: &Actor) -> bool {
    a.body_sprites.sprites.iter().any(|s| s.slot == 0x870)
}

pub(crate) fn owner_tick(a: &mut Actor, scene: u32) {
    if a.flags.get(0xB55) != 0 || a.flags.get(0xB3E) == 1 {
        for child in &mut a.body_sprites.sprites {
            child.disabled = true;

            if !super::effects::visible_in_scene(scene, false) {
                child.billboard.ready = false;
            }
        }
    }
}

pub(crate) fn update_in_scene(
    a: &mut Actor,
    visible: bool,
    environment: i32,
    now: u32,
    yaw: f32,
    pitch: f32,
    scene: u32,
) {
    refresh(a, environment);

    owner_tick(a, scene);

    let special = has_primary(a) && (matches!(a.class, 36 | 37) || a.flags.get(0xB56) == 1);

    let follow = visible && (a.class != 33 || a.parts[0].mesh != 1 || environment == 2 || special);

    let factor = a.evok_factor();

    let scale = a.presentation_scale();

    for s in &mut a.body_sprites.sprites {
        s.billboard.scale = s.retail_size * factor;

        let (body_slot, lift) = if special && (0x870..=0x888).contains(&s.slot) {
            let i = usize::from((s.slot - 0x870) / 4);

            (
                [8, 9, 1, 6, 7, 2, 3][i],
                if i < 3 {
                    0.0
                } else if i < 5 {
                    0.1
                } else {
                    0.3
                },
            )
        } else {
            (s.body_slot, s.lift)
        };

        if follow && let Some(point) = a.body_points.points[body_slot] {
            s.billboard.position = point + Vec3::Y * (lift * scale);

            s.published = true;
        }

        s.billboard.update(now, yaw, pitch);

        s.billboard.ready =
            visible && s.published && super::effects::visible_in_scene(scene, !s.disabled);
    }
}

pub(crate) fn submit(
    actors: &ActorScene,
    scene: &BillboardScene,
    eye: Vec3,
    now: u32,
    frame: &mut super::effects::Frame,
) {
    let cx = Submit {
        scene,
        eye,
        now_ms: now,
    };

    let mut meshes = Vec::new();

    let mut sink = Sink {
        effect: frame,
        mesh: &mut meshes,
    };

    for a in &actors.actors {
        for s in &a.body_sprites.sprites {
            s.billboard.submit(&cx, &mut sink);
        }
    }
}

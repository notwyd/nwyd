use super::{
    field::{FieldScene, S_MODE},
    quads::{GroundQuad, Slot},
};
use crate::frame::{PlannedQuad, TextureKey};
use formats::{data::item_effect::Effects, net::account::Item};
use glam::Vec3;
use render::{
    billboard::BillboardPass,
    sprite::{Quad, UvRule},
};

pub const NAME_KEY: u32 = 0xA000_0000;

pub const FESTIVE_LIGHTS: [[f32; 3]; 28] = [
    [0.398, 5.762, 0.706],
    [0.706, 4.932, 0.157],
    [1.117, 4.072, 1.220],
    [-0.664, 3.804, 0.220],
    [-0.056, 32.779, 1.841],
    [0.035, 2.605, -0.469],
    [-1.081, 2.215, 0.134],
    [1.182, 3.147, 1.736],
    [1.520, 2.340, 1.736],
    [1.924, 2.284, 0.109],
    [1.486, 1.878, -0.500],
    [-0.164, 1.425, 1.908],
    [1.536, 0.922, 1.192],
    [-1.081, 1.114, -0.185],
    [-0.217, 4.563, 1.277],
    [0.844, 4.058, -0.265],
    [0.844, 4.058, -0.265],
    [1.712, 3.225, 0.499],
    [-0.779, 3.326, 0.917],
    [-0.859, 2.284, 1.835],
    [0.612, 1.287, 2.295],
    [0.079, 1.467, -1.028],
    [0.790, 1.467, -1.028],
    [1.679, 3.817, 0.563],
    [-0.115, 3.283, 1.824],
    [-0.578, 2.770, -0.041],
    [-0.848, 1.306, 2.230],
    [0.769, 0.509, -1.143],
];

pub fn bind(
    scene: &mut super::effects::BillboardScene,
    game: &assets::GameDir,
    item: &Item,
    effects: &Effects<'_>,
) {
    if style(item, effects).glow.is_some() {
        scene.bind_slot(game, 2);
    }

    if item.index == 0x6BF {
        scene.bind_slot(game, 56);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glow {
    pub colour: u32,
    pub scale: f32,
    pub lift: f32,
    pub wide: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub label: u32,
    pub guild: u16,
    pub glow: Option<Glow>,
}

pub fn style(item: &Item, effects: &Effects<'_>) -> Style {
    let id = item.index;

    let kind = effects.total_scaled(item, 0x26);

    let quality = effects.name_color(item);

    let guild = (effects.total_scaled(item, 0x38) as u16).wrapping_shl(8)
        | effects.total_scaled(item, 0x39) as u16;

    let initial = if kind == 2 || guild == 0 {
        0xFFAA_AAFF
    } else if (0xC49..=0xC4D).contains(&id) {
        0xFFFF_FFAA
    } else {
        0xFFAA_AAAA
    };

    let mut s = Style {
        label: initial,
        guild,
        glow: Some(Glow {
            colour: 0,
            scale: 0.8,
            lift: 0.1,
            wide: false,
        }),
    };

    let g = s.glow.as_mut().unwrap();

    g.colour = match kind {
        1 => {
            if effects.total_scaled(item, 4) < 1 {
                0xFF00_0066
            } else if id == 0x6CB {
                0xFFAA_8888
            } else {
                0xFF66_0000
            }
        }
        2 | 3 => 0xFF66_6600,
        4 => {
            s.label = 0xFFFF_FFAA;

            0xFF77_4400
        }
        5 => {
            s.label = 0xFFFF_FFAA;

            0xFF44_0077
        }
        10 => 0xFF00_6600,
        _ => match id {
            0x2EB => {
                s.label = 0xFFFF_AA00;

                0xFF88_4400
            }
            0x309..=0x310 | 0xC62..=0xC7F => {
                s.label = 0xFFFF_FFAA;

                0xFF66_0000
            }
            0xF4A | 0xF4B | 0x6D0..=0x6D7 => {
                s.glow = None;

                return s;
            }
            0xCEE..=0xDDE | 0x140F => {
                s.label = 0xFFFF_FFAA;

                0xFF66_4466
            }
            0xFAC..=0xFD2 => {
                s.label = 0xFFFF_FFAA;

                0xFF66_6644
            }
            0x14C4 | 0x1005..=0x102E | 0x1030..=0x1035 => {
                s.label = 0xFFFF_FFAA;

                0xFF44_4488
            }
            0xFAA..=0xFAB => {
                s.label = 0xFFFF_FFAA;

                0xFF66_6666
            }
            0x311 | 0x314..=0x31A => 0xFF55_5533,
            0x295 => 0xFF66_0000,
            0x296 => 0xFF00_0066,
            0x297 => 0xFF00_6600,
            0x29B => {
                g.scale = 1.4;

                0xFF00_0066
            }
            0x1B4..=0x1B9 => {
                g.scale = 1.4;

                if id < 0x1B7 { 0xFF00_0066 } else { 0xFF66_0000 }
            }
            0x13F6..=0x140D => {
                g.scale = 1.0;

                g.lift = -0.03;

                g.wide = true;

                0xFF77_4400
            }
            _ => {
                s.label = quality;

                let marked = quality != 0xFFAA_AAFF
                    || matches!(
                        id,
                        0x19C | 0x19D | 0x102D | 0x1A3 | 0x1A4 | 0x2F1 | 0x301 | 0x6BE
                    );

                if !marked {
                    s.glow = None;

                    return s;
                }

                match id {
                    0x1A3 => 0xFF77_4400,
                    0x1A4 => 0xFF44_0077,
                    0x2F1 => 0xFF66_0000,
                    _ => {
                        g.lift = 0.05;

                        match quality {
                            0xFFFF_FFAA => 0xFF55_5533,
                            0xFFFF_FF00 => 0xFF88_4400,
                            _ => 0xFF00_3300,
                        }
                    }
                }
            }
        },
    };

    s
}

pub fn caption(item: &Item, effects: &Effects<'_>, name: &str, gold_format: &str) -> String {
    let text = if effects.total_scaled(item, 0x26) == 2 {
        let amount = (u16::from(effects.total_scaled(item, 0x24) as u8) << 8)
            | u16::from(effects.total_scaled(item, 0x25) as u8);

        gold_format.replacen("%d", &amount.to_string(), 1)
    } else {
        name.to_string()
    };

    super::stext::display_text(&text)
}

fn owners(scene: &FieldScene) -> impl Iterator<Item = (u16, &Item, Vec3, bool)> {
    scene
        .drops
        .items
        .iter()
        .rev()
        .filter(|d| d.drawable())
        .map(|d| (d.entity, &d.item, d.position, d.hovered))
        .chain(
            scene
                .gates
                .items
                .iter()
                .rev()
                .filter(|g| g.item_id > 0)
                .map(|g| (g.entity, &g.item, g.position, g.hovered)),
        )
}

fn label_origin(scene: &FieldScene, position: Vec3, hovered: bool) -> Option<(f32, f32)> {
    let (view, projection) = scene.projected?;

    let ctrl = scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.ctrl);

    if !(ctrl || hovered) || !scene.world.pose_reach().admits(position) {
        return None;
    }

    let (x, y) =
        super::nameplate::project(&view, &projection, position + Vec3::Y * 0.3, &scene.layout)?;

    Some((x.trunc() - 64.0, y.trunc()))
}

fn badge_admitted(item: &Item, effects: &Effects<'_>) -> bool {
    effects.total_scaled(item, 0x26) != 2 && style(item, effects).guild != 0
}

pub fn consumes_badge_mouse(scene: &FieldScene, event: ui::input::Mouse, x: i32, y: i32) -> bool {
    use ui::input::Mouse;

    if !matches!(
        event,
        Mouse::Move | Mouse::LeftDown | Mouse::LeftUp | Mouse::RightDown
    ) {
        return false;
    }

    let rect = scene.layout.control(0.0, 0.0, 10.0, 10.0);

    let effects = scene.statics.effects();

    owners(scene).any(|(_, item, position, hovered)| {
        if scene.statics.items.get(item.index as usize).is_none() {
            return false;
        }

        if !badge_admitted(item, &effects) {
            return false;
        }

        let Some(at) = label_origin(scene, position, hovered) else {
            return false;
        };

        let (left, top) = (at.0 + 10.0, at.1 + 3.0);

        x as f32 >= left
            && y as f32 >= top
            && (x as f32) < left + rect.w
            && (y as f32) < top + rect.h
    })
}

pub fn frame_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    let stale: Vec<_> = scene
        .text
        .iter()
        .filter_map(|(&key, _)| {
            (key & 0xFFFF_0000 == NAME_KEY
                && scene.drops.get(key as u16).is_none()
                && scene.gates.get(key as u16).is_none())
            .then_some(key)
        })
        .collect();

    for key in stale {
        scene.text.forget(key);
    }

    if scene.projected.is_none() {
        return Vec::new();
    }

    let ctrl = scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.ctrl);

    let effects = scene.statics.effects();

    let mut out = Vec::new();

    let owners: Vec<_> = owners(scene)
        .map(|(id, item, at, hover)| (id, *item, at, hover))
        .collect();

    for (entity, item, position, hovered) in owners {
        let Some(at) = label_origin(scene, position, hovered) else {
            continue;
        };

        let Some(template) = scene.statics.items.get(item.index as usize) else {
            continue;
        };

        let style = style(&item, &effects);

        if badge_admitted(&item, &effects)
            && let Some(slot) = scene.marks.resolve(style.guild)
            && scene
                .marks
                .bank
                .slot(slot)
                .is_some_and(|s| s.pixels.is_some())
        {
            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::GuildMark(slot),
                quad: Quad {
                    normalized_corners: None,
                    clip: None,
                    dst_x: at.0 + 10.0,
                    dst_y: at.1 + 3.0,
                    dst_w: 16.0,
                    dst_h: 12.0,
                    src_x: 0.0,
                    src_y: 0.0,
                    src_w: 16.0,
                    src_h: 12.0,
                    color: 0xFFFF_FFFF,
                    rotation: 0.0,
                },
                uv: UvRule::TexelCentre,
            });
        }

        if !scene.captions {
            continue;
        }

        let text = caption(&item, &effects, &template.name, &scene.drop_gold_caption);

        let key = NAME_KEY | u32::from(entity);

        let colour = if ctrl && hovered {
            0xFFFF_FFFF
        } else {
            style.label
        };

        if scene.text.resolve(key, &text, colour)
            && let Some(line) = scene.text.get(key)
        {
            out.push(PlannedQuad {
                mode: render::preset::RenderMode::Ui2dPoint,
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

    out
}

#[derive(Debug, Clone, Copy)]
pub struct GroundGlow {
    pub quad: GroundQuad,
    pub wide: bool,
}

pub fn ground_glows(scene: &FieldScene) -> Vec<GroundGlow> {
    let effects = scene.statics.effects();

    let mut out: Vec<_> = owners(scene)
        .filter_map(|(entity, item, position, _)| {
            let g = style(item, &effects).glow?;

            Some(GroundGlow {
                wide: g.wide,
                quad: GroundQuad {
                    owner: usize::from(entity),
                    slot: Slot::Placed,
                    texture: 2,
                    pass: BillboardPass::Additive,
                    anchor: position + Vec3::Y * g.lift,
                    base_scale: Vec3::splat(g.scale),
                    grow: 0.0,
                    spin_period: 0,
                    ordinal: usize::from(entity),
                    argb: g.colour,
                },
            })
        })
        .collect();

    for gate in &scene.gates.items {
        if let Some(colour) = gate.table.glow {
            out.push(GroundGlow {
                wide: false,
                quad: GroundQuad {
                    owner: usize::from(gate.entity),
                    slot: Slot::Placed,
                    texture: 2,
                    pass: BillboardPass::Additive,
                    anchor: gate.position + Vec3::Y * super::placed::glow_alpha(gate.swing),
                    base_scale: Vec3::splat(1.8),
                    grow: 0.0,
                    spin_period: 0,
                    ordinal: usize::from(gate.entity),
                    argb: colour,
                },
            });
        }
    }

    out
}

pub fn festive_lights(scene: &FieldScene) -> Vec<super::effects::Billboard> {
    let mut out = Vec::new();

    for drop in scene
        .drops
        .items
        .iter()
        .rev()
        .filter(|d| d.drawable() && d.item.index == 0x6BF)
    {
        for (i, offset) in FESTIVE_LIGHTS.into_iter().enumerate() {
            let mut b = super::effects::Billboard::new(56, 0, Vec3::splat(1.5), 0.0, 1, 80);

            b.pass = BillboardPass::Additive;

            b.colour_mode = 2;

            b.phase_seed = u32::from(drop.entity).wrapping_add(i as u32);

            b.position = drop.position + Vec3::from_array(offset);

            b.set_color(if i < 14 {
                0xFFAA_AA00
            } else if i < 23 {
                0xFF00_88FF
            } else {
                0xFFFF_5500
            });

            b.update(
                scene.time.now_ms,
                scene.world.pose.yaw,
                scene.world.pose.pitch,
            );

            out.push(b);
        }
    }

    out
}

pub fn append_lights(
    scene: &super::effects::BillboardScene,
    frame: &mut super::effects::Frame,
    lights: &[super::effects::Billboard],
    reach: super::actors::Reach,
    now: u32,
) {
    use super::effect_tree::Effect;

    let cx = super::effect_tree::Submit {
        scene,
        eye: reach.eye,
        now_ms: now,
    };

    let mut mesh = Vec::new();

    let mut sink = super::effect_tree::Sink {
        effect: frame,
        mesh: &mut mesh,
    };

    for b in lights {
        b.submit(&cx, &mut sink);
    }
}

pub fn pulse(argb: u32, ordinal: usize, now: u32) -> u32 {
    let t = ((ordinal as u32 % 100) * 200).wrapping_add(now) % 5000;

    let k = 0.8 + 0.2 * (std::f32::consts::TAU * t as f32 / 5000.0).sin().abs();

    [0, 8, 16, 24]
        .into_iter()
        .map(|shift| ((((argb >> shift) & 255) as f32 * k) as u32) << shift)
        .sum()
}

pub fn append_glows(
    scene: &super::effects::BillboardScene,
    frame: &mut super::effects::Frame,
    glows: &[GroundGlow],
    reach: super::actors::Reach,
    now: u32,
) {
    if scene.detail == 1 {
        return;
    }

    let cx = super::effect_tree::Submit {
        scene,
        eye: reach.eye,
        now_ms: now,
    };

    let mut mesh = Vec::new();

    let mut sink = super::effect_tree::Sink {
        effect: frame,
        mesh: &mut mesh,
    };

    for glow in glows {
        if !reach.admits(glow.quad.anchor) {
            sink.effect.culled += 1;

            continue;
        }

        let start = sink.effect.vertices.len();

        glow.quad.submit_coloured(
            &cx,
            &mut sink,
            None,
            Some(pulse(glow.quad.argb, glow.quad.ordinal, now)),
        );

        if glow.wide && sink.effect.vertices.len() == start + 4 {
            for (v, p) in sink.effect.vertices[start..].iter_mut().zip([
                [-0.5, 0.0, -0.8],
                [0.9, 0.0, -0.8],
                [0.9, 0.0, 0.5],
                [-0.5, 0.0, 0.5],
            ]) {
                v.position = p;
            }
        }
    }
}

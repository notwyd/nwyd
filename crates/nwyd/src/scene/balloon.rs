use super::field::{FieldScene, S_MODE};
use super::login_static::{Layout, Rect};
use super::stext::{self, SceneText};
use crate::frame::{PlannedQuad, TextureKey};
use render::preset::RenderMode;
use render::sprite::{Quad, UvRule};

pub const BOX_COLOR: u32 = 0x7700_0000;

pub const TEXT_COLOR: u32 = 0xFFFF_FFFF;

pub const PAD: f32 = 20.0;

pub const RISE: f32 = 60.0;

pub const RISE_CACHED: f32 = 64.0;

pub const DEFAULT_LIFE_MS: u32 = 3000;

pub const BUILT_BOX: [f32; 4] = [0.0, 650.0, 256.0, 64.0];

pub const BALLOON_KEY: u32 = 0x4000_0000;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Balloon {
    pub text: String,
    pub start_ms: u32,
    pub life_ms: u32,
}

pub fn size(measured: (i32, i32), layout: &Layout) -> (f32, f32) {
    (
        layout.sx * PAD + measured.0 as f32,
        measured.1 as f32 + layout.sy * PAD,
    )
}

pub fn place(at: (f32, f32), size: (f32, f32)) -> Rect {
    Rect {
        x: at.0 - size.0 * 0.5,
        y: at.1 - RISE,
        w: size.0,
        h: size.1,
    }
}

pub fn visible(balloon: &Balloon, now_ms: u32) -> bool {
    now_ms.wrapping_sub(balloon.start_ms) < balloon.life_ms
}

pub fn expired(balloon: &Balloon, now_ms: u32) -> bool {
    balloon.life_ms < now_ms.wrapping_sub(balloon.start_ms)
}

pub fn raise(scene: &mut FieldScene, entity: u16, text: &str, life_ms: u32) -> bool {
    if text.is_empty() {
        return false;
    }

    let Some(index) = scene.world.actors.entity(entity) else {
        return false;
    };

    let start_ms = scene.time.now_ms;

    scene.world.actors.actors[index].balloon = Some(Balloon {
        text: text.to_string(),
        start_ms,
        life_ms,
    });

    true
}

pub fn sweep(scene: &mut FieldScene, now_ms: u32) -> usize {
    let mut retired = 0;

    for actor in &mut scene.world.actors.actors {
        if actor
            .balloon
            .as_ref()
            .is_some_and(|balloon| expired(balloon, now_ms))
        {
            actor.balloon = None;

            retired += 1;
        }
    }

    let live: Vec<u32> = scene
        .world
        .actors
        .actors
        .iter()
        .filter(|actor| actor.balloon.is_some())
        .map(|actor| key(actor.entity))
        .collect();

    let stale: Vec<u32> = scene
        .text
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| id & 0xFFFF_0000 == BALLOON_KEY && !live.contains(id))
        .collect();

    for id in stale {
        forget(scene, (id & !BALLOON_KEY) as u16);
    }

    retired
}

pub fn key(entity: u16) -> u32 {
    BALLOON_KEY | u32::from(entity)
}

pub fn forget(scene: &mut FieldScene, entity: u16) -> bool {
    scene.text.forget(key(entity))
}

pub fn draws(
    text: &mut SceneText,
    entity: u16,
    balloon: &Balloon,
    at: (f32, f32),
    layout: &Layout,
) -> Vec<PlannedQuad> {
    let id = key(entity);

    let raw = balloon.text.split('\0').next().unwrap_or_default();

    if raw.is_empty() {
        text.forget(id);

        return Vec::new();
    }

    let Some(measured) = text.measure(raw) else {
        return Vec::new();
    };

    text.resolve(id, &stext::display_text(raw), TEXT_COLOR);

    let box_size = size(measured, layout);

    let rect = place(at, box_size);

    let origin = stext::origin(layout, (0.0, 0.0), rect);

    let mut out = vec![PlannedQuad {
        mode: RenderMode::NuiPanel2d,
        texture: TextureKey::White,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: rect.x,
            dst_y: rect.y,
            dst_w: rect.w,
            dst_h: rect.h,
            src_x: 0.0,
            src_y: 0.0,
            src_w: 1.0,
            src_h: 1.0,
            color: BOX_COLOR,
            rotation: 0.0,
        },
        uv: UvRule::TexelEdge,
    }];

    if let Some(line) = text.get(id) {
        out.push(PlannedQuad {
            mode: RenderMode::Ui2dPoint,
            texture: TextureKey::SceneText(id),
            quad: stext::shadow_quad(line, origin),
            uv: UvRule::TexelCentre,
        });

        out.push(PlannedQuad {
            mode: S_MODE,
            texture: TextureKey::SceneText(id),
            quad: stext::quad(line, origin),
            uv: UvRule::TexelCentre,
        });
    }

    out
}

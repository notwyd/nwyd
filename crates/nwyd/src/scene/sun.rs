use crate::frame::{PlannedQuad, SceneRenderer, TextureKey};
use glam::Vec3;
use render::camera::Matrix4;
use render::device::Gpu;
use render::preset::RenderMode;
use render::sprite::{Quad, UvRule};
use render::texture::AlphaPolicy;

pub const ANCHOR: Vec3 = Vec3::new(-1.0, 0.7, 0.3);

pub const SCALE_SEED: f32 = 1.0;

pub const SIZE_BASE: f64 = 50.0;

pub const Z_LIMIT: f32 = 1.0;

pub const SCALE_TO_Y: f32 = 0.7;

pub const Z_FIRST: f32 = 0.8;

pub const Z_STEP: f32 = 0.04;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Flare {
    pub texture: usize,
    pub axis: f32,
    pub size: f64,
    pub color: u32,
}

pub const FLARES: [Flare; 12] = [
    Flare {
        texture: 0xCE,
        axis: -0.6,
        size: 0.40,
        color: 0x00AA_8888,
    },
    Flare {
        texture: 0xD4,
        axis: -0.4,
        size: 0.23,
        color: 0x00AA_AA22,
    },
    Flare {
        texture: 0xD4,
        axis: -0.3,
        size: 0.09,
        color: 0x00AA_22AA,
    },
    Flare {
        texture: 0xD3,
        axis: 0.0,
        size: 1.60,
        color: 0x00FF_FFFF,
    },
    Flare {
        texture: 0xD2,
        axis: 0.0,
        size: 0.93,
        color: 0x00AA_AAAA,
    },
    Flare {
        texture: 0xD3,
        axis: 0.0,
        size: 2.81,
        color: 0x00AA_AAAA,
    },
    Flare {
        texture: 0xD4,
        axis: 0.0,
        size: 9.43,
        color: 0x00AA_AAAA,
    },
    Flare {
        texture: 0xD5,
        axis: 0.0,
        size: 10.60,
        color: 0x00FF_FFFF,
    },
    Flare {
        texture: 0xD2,
        axis: 0.1,
        size: 0.42,
        color: 0x00AA_AAAA,
    },
    Flare {
        texture: 0xD1,
        axis: 0.2,
        size: 0.64,
        color: 0x00AA_AAAA,
    },
    Flare {
        texture: 0xCE,
        axis: 0.3,
        size: 0.73,
        color: 0x00AA_AAAA,
    },
    Flare {
        texture: 0xD0,
        axis: 0.6,
        size: 1.56,
        color: 0x00AA_AAAA,
    },
];

pub fn sizes(width: u32) -> [f32; 12] {
    let base = (f64::from(super::login_static::Layout::new(width, 1).sx) * SIZE_BASE) as f32;

    FLARES.map(|flare| (f64::from(base) * flare.size) as f32)
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Element {
    pub texture: usize,
    pub centre: (f32, f32),
    pub half: f32,
    pub z: f32,
    pub color: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Sun {
    pub offset: Vec3,
    pub scale: f32,
}

impl Default for Sun {
    fn default() -> Sun {
        Sun::new()
    }
}

impl Sun {
    pub const fn new() -> Sun {
        Sun {
            offset: ANCHOR,
            scale: SCALE_SEED,
        }
    }

    pub fn project(
        at: Vec3,
        view: &Matrix4,
        projection: &Matrix4,
        (width, height): (u32, u32),
    ) -> Option<(f32, f32, f32)> {
        let ndc = projection.transform_coord(view.transform_coord(at));

        if ndc.z > Z_LIMIT {
            return None;
        }

        let (w, h) = (width as f32, height as f32);

        Some(((1.0 + ndc.x) * 0.5 * w, (1.0 - ndc.y) * 0.5 * h, ndc.z))
    }

    pub fn draw(
        &mut self,
        eye: Vec3,
        view: &Matrix4,
        projection: &Matrix4,
        backbuffer: (u32, u32),
        sizes: &[f32; 12],
        suppressed: bool,
    ) -> Vec<Element> {
        if suppressed {
            return Vec::new();
        }

        let Some((sx, sy, _)) = Sun::project(eye + self.offset, view, projection, backbuffer)
        else {
            return Vec::new();
        };

        self.offset.y = self.scale * SCALE_TO_Y;

        let axis = (
            (backbuffer.0 >> 1) as f32 - sx,
            (backbuffer.1 >> 1) as f32 - sy,
        );

        FLARES
            .iter()
            .zip(sizes)
            .enumerate()
            .map(|(i, (flare, &size))| Element {
                texture: flare.texture,
                centre: (sx + axis.0 * flare.axis, sy + axis.1 * flare.axis),
                half: self.scale * size,
                z: Z_FIRST - Z_STEP * i as f32,
                color: flare.color,
            })
            .collect()
    }
}

pub const TEXTURE_SLOTS: [usize; 7] = [0xCE, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5];

pub fn upload(effects: &super::effects::BillboardScene, gpu: &Gpu, renderer: &mut SceneRenderer) {
    for slot in TEXTURE_SLOTS {
        let Some(index) = effects.texture_index(slot) else {
            continue;
        };

        let bound = &effects.textures[index];

        renderer.upload_once(
            gpu,
            TextureKey::Effect(slot),
            &bound.texture,
            AlphaPolicy::for_flag(bound.flag),
            &bound.path,
        );
    }
}

pub fn quads(elements: &[Element], dims: impl Fn(usize) -> Option<(u32, u32)>) -> Vec<PlannedQuad> {
    elements
        .iter()
        .filter_map(|element| {
            let (w, h) = dims(element.texture)?;

            let (cx, cy) = element.centre;

            Some(PlannedQuad {
                mode: RenderMode::LensFlare2d,
                texture: TextureKey::Effect(element.texture),
                quad: Quad {
                    normalized_corners: None,
                    clip: None,
                    dst_x: cx - element.half,
                    dst_y: cy - element.half,
                    dst_w: element.half * 2.0,
                    dst_h: element.half * 2.0,
                    src_x: -(w as f32),
                    src_y: -(h as f32),
                    src_w: w as f32,
                    src_h: h as f32,
                    color: element.color,
                    rotation: 0.0,
                },
                uv: UvRule::TexelEdge,
            })
        })
        .collect()
}

pub fn frame_draws(world: &mut super::world::World, backbuffer: (u32, u32)) -> Vec<PlannedQuad> {
    let Some((view, projection)) = world.projected else {
        return Vec::new();
    };

    let suppressed = world.flare_suppressed();

    let eye = world.pose.eye;

    let sizes = sizes(backbuffer.0);

    let Some(sun) = world.sun.as_mut() else {
        return Vec::new();
    };

    let elements = sun.draw(eye, &view, &projection, backbuffer, &sizes, suppressed);

    quads(&elements, |slot| {
        let index = world.effects.texture_index(slot)?;

        let texture = &world.effects.textures[index].texture;

        Some((texture.width, texture.height))
    })
}

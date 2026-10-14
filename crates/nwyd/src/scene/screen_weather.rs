use super::{effects::BillboardScene, login_static::Layout, rng::Lcg};
use crate::frame::{PlannedQuad, SceneRenderer, TextureKey};
use render::{
    device::Gpu,
    preset::RenderMode,
    sprite::{Quad, UvRule},
    texture::AlphaPolicy,
};

pub const SNOW_TEXTURE: usize = 422;

#[derive(Debug, Clone)]

struct Gust {
    birth: Option<u32>,
    elapsed: u32,
    life: u32,
    size: f32,
    origin: (i32, i32),
}

#[derive(Debug, Clone)]

pub struct Gusts {
    last: u32,
    interval: u32,
    cycle: u32,
    live: Vec<Gust>,
}

impl Default for Gusts {
    fn default() -> Self {
        Self {
            last: 0,
            interval: 2000,
            cycle: 0,
            live: Vec::new(),
        }
    }
}

impl Gusts {
    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    pub fn update(&mut self, now: u32) {
        self.live.retain_mut(|gust| {
            let birth = *gust.birth.get_or_insert(now);

            gust.elapsed = now.wrapping_sub(birth);

            gust.elapsed < gust.life
        });
    }

    pub fn emit(&mut self, now: u32, eligible: bool, rng: &mut Lcg) -> bool {
        if !eligible || now.wrapping_sub(self.last) <= self.interval {
            return false;
        }

        for i in 0..5 {
            self.live.push(Gust {
                birth: None,
                elapsed: 0,
                life: 3000 + 800 * i,
                size: 300.0 + 50.0 * rng.next_mod(5) as f32,
                origin: (-900 + 100 * i as i32, 100 * i as i32),
            });
        }

        self.cycle = (self.cycle + 1) % 10;

        self.interval = if self.cycle == 0 {
            (rng.next_mod(10) + 5) * 3000
        } else {
            1000
        };

        self.last = now;

        true
    }

    pub fn quads(
        &self,
        layout: &Layout,
        detail: bool,
        effects: &BillboardScene,
    ) -> Vec<PlannedQuad> {
        let Some(index) = effects.texture_index(SNOW_TEXTURE).filter(|_| detail) else {
            return Vec::new();
        };

        let texture = &effects.textures[index].texture;

        self.quads_with_dims(layout, (texture.width, texture.height))
    }

    fn quads_with_dims(&self, layout: &Layout, dims: (u32, u32)) -> Vec<PlannedQuad> {
        self.live
            .iter()
            .rev()
            .filter_map(|gust| {
                let age = gust.elapsed as f32 / gust.life as f32;

                if gust.birth.is_none() || age < 0.05 {
                    return None;
                }

                let half_x = gust.size * layout.sx * age;

                let half_y = gust.size * layout.sy * age;

                let channel = ((std::f32::consts::PI * age).sin() * 255.0) as u32;

                Some(PlannedQuad {
                    mode: RenderMode::NuiPanel2d,
                    texture: TextureKey::Effect(SNOW_TEXTURE),
                    quad: Quad {
                        normalized_corners: None,
                        clip: None,
                        dst_x: (gust.origin.0 as f32 * layout.sx).trunc() - half_x
                            + 1600.0 * layout.sx * age,
                        dst_y: (gust.origin.1 as f32 * layout.sy).trunc() - half_y,
                        dst_w: half_x * 2.0,
                        dst_h: half_y * 2.0,
                        src_x: dims.0 as f32 * 0.01,
                        src_y: dims.1 as f32 * 0.01,
                        src_w: dims.0 as f32 * 0.98,
                        src_h: dims.1 as f32 * 0.98,
                        color: channel * 0x01010101,
                        rotation: 0.0,
                    },
                    uv: UvRule::TexelEdge,
                })
            })
            .collect()
    }
}

pub fn upload(effects: &BillboardScene, gpu: &Gpu, renderer: &mut SceneRenderer) {
    if let Some(index) = effects.texture_index(SNOW_TEXTURE) {
        let bound = &effects.textures[index];

        renderer.upload_once(
            gpu,
            TextureKey::Effect(SNOW_TEXTURE),
            &bound.texture,
            AlphaPolicy::for_flag(bound.flag),
            &bound.path,
        );
    }
}

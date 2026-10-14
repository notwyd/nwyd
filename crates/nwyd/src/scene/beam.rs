use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use glam::Vec3;
use render::billboard::{BillboardDraw, BillboardPass, BillboardUniforms, BillboardVertex};
use render::camera::Matrix4;

pub const BEAM_VERTICES: usize = 8;

pub const FAN_INDICES: [u16; 12] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];

pub const UV_LOW: f32 = 0.02;

pub const UV_HIGH: f32 = 0.98;

pub const SRC_BLEND: u32 = render::preset::d3d::BLEND_SRCALPHA;

pub const DEST_BLEND_ADDITIVE: u32 = render::preset::d3d::BLEND_ONE;

pub const DEST_BLEND_BLENDED: u32 = render::preset::d3d::BLEND_INVSRCALPHA;

pub const BLEND_ARM_OFFSET: u16 = 0x6C;

pub fn pass_from_blend_mode(blend_mode: u32) -> BillboardPass {
    BillboardPass::from_additive(blend_mode == 1)
}

pub fn blend_pair(pass: BillboardPass) -> (u32, u32) {
    match pass {
        BillboardPass::Additive => (SRC_BLEND, DEST_BLEND_ADDITIVE),
        BillboardPass::Blended => (SRC_BLEND, DEST_BLEND_BLENDED),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Beam {
    pub texture: usize,
    pub lifetime_ms: u32,
    pub birth_ms: u32,
    pub half_width: f32,
    pub inert_scale: f32,
    pub argb: u32,
    pub channels: [u32; 4],
    pub pass: BillboardPass,
    pub fade: bool,
    pub visible: bool,
    pub collapse_from_ms: u32,
    pub near: Vec3,
    pub far: Vec3,
    pub node: Vec3,
    pub vertices: [BillboardVertex; BEAM_VERTICES],
    pub gone: bool,
}

impl Beam {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        a: Vec3,
        b: Vec3,
        texture: usize,
        argb: u32,
        lifetime_ms: u32,
        inert_scale: f32,
        half_width: f32,
        now_ms: u32,
    ) -> Beam {
        let vertex = BillboardVertex {
            position: [0.0; 3],
            diffuse: 0,
            uv: [0.0, 0.0],
        };

        let mut beam = Beam {
            texture,
            lifetime_ms,
            birth_ms: now_ms,
            half_width,
            inert_scale,
            argb: 0,
            channels: [0; 4],
            pass: BillboardPass::Blended,
            fade: false,
            visible: true,
            collapse_from_ms: 0,
            near: a,
            far: b,
            node: Vec3::ZERO,
            vertices: [vertex; BEAM_VERTICES],
            gone: false,
        };

        beam.place(a, b);

        let uv = [
            [UV_LOW, UV_HIGH],
            [UV_HIGH, UV_HIGH],
            [UV_HIGH, UV_LOW],
            [UV_LOW, UV_LOW],
            [UV_LOW, UV_HIGH],
            [UV_HIGH, UV_HIGH],
            [UV_HIGH, UV_LOW],
            [UV_LOW, UV_LOW],
        ];

        for (v, uv) in beam.vertices.iter_mut().zip(uv) {
            v.uv = uv;
        }

        beam.set_color(argb);

        beam
    }

    pub fn additive(mut self) -> Beam {
        self.pass = BillboardPass::Additive;

        self.fade = true;

        self
    }

    pub fn set_color(&mut self, argb: u32) {
        self.argb = argb;

        for v in &mut self.vertices {
            v.diffuse = argb;
        }

        self.channels = [
            argb >> 24,
            (argb >> 16) & 0xFF,
            (argb >> 8) & 0xFF,
            argb & 0xFF,
        ];
    }

    pub fn place(&mut self, a: Vec3, b: Vec3) {
        let w = self.half_width;

        self.near = a;

        self.far = b;

        let positions = [
            [a.x, a.y - w, a.z],
            [b.x, b.y - w, b.z],
            [b.x, b.y + w, b.z],
            [a.x, a.y + w, a.z],
            [a.x - w, a.y, a.z],
            [b.x - w, b.y, b.z],
            [b.x + w, b.y, b.z],
            [a.x + w, a.y, a.z],
        ];

        for (v, p) in self.vertices.iter_mut().zip(positions) {
            v.position = p;
        }
    }

    pub fn age(&self, now_ms: u32) -> f32 {
        match self.lifetime_ms {
            0 => (now_ms % 1000) as f32 / 1000.0,
            lifetime => now_ms.wrapping_sub(self.birth_ms) as f32 / lifetime as f32,
        }
    }

    pub fn tick(&mut self, now_ms: u32) {
        self.node = Vec3::from(self.vertices[0].position);

        let age = self.age(now_ms);

        if self.lifetime_ms != 0 && self.lifetime_ms < now_ms.wrapping_sub(self.birth_ms) {
            self.visible = false;

            self.gone = true;
        }

        if self.fade && age < 1.0 {
            let argument = (f64::from(age) * std::f64::consts::PI) as f32;

            let scale = f64::from(argument).sin() as f32;

            let scaled = self
                .channels
                .map(|c| (c as f32 * scale).round().max(0.0) as u32 & 0xFF);

            let argb = scaled.iter().fold(0u32, |acc, c| (acc << 8) | c);

            for v in &mut self.vertices {
                v.diffuse = argb;
            }
        }

        if self.collapse_from_ms != 0 {
            let duration = self
                .birth_ms
                .wrapping_sub(self.collapse_from_ms)
                .wrapping_add(self.lifetime_ms);

            if duration != 0 {
                let t = now_ms.wrapping_sub(self.collapse_from_ms) as f32 / duration as f32;

                self.place(self.near, self.far * (1.0 - t) + self.near * t);
            }
        }
    }

    pub fn drawn(&self) -> bool {
        self.visible
    }
}

pub const CLASS: Class = Class {
    name: "EffectBillboard3",
    method_count: 16,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

impl Effect for Beam {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        self.tick(tick.now_ms);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if cx.scene.detail == 1 {
            return;
        }

        if !self.drawn() {
            out.effect.culled += 1;

            return;
        }

        let Some((array, layer)) = cx.scene.placement(self.texture) else {
            out.effect.unbound += 1;

            return;
        };

        let object = out.effect.objects.len() as u32;

        out.effect.objects.push(
            BillboardUniforms::new(Matrix4::IDENTITY, self.pass)
                .with_layer(layer)
                .with_opaque(cx.scene.alpha_at(self.texture)),
        );

        let base_vertex = out.effect.vertices.len() as u32;

        let first = out.effect.indices.len() as u32;

        out.effect.vertices.extend_from_slice(&self.vertices);

        out.effect.indices.extend(FAN_INDICES);

        out.effect.draws.push(BillboardDraw {
            pass: self.pass,
            object,
            texture: array,
            base_vertex,
            indices: first..out.effect.indices.len() as u32,
        });

        out.effect.submitted(&CLASS);
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        match self.visible {
            true => Cull::Kept,
            false => Cull::Rejected,
        }
    }
}

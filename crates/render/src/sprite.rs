use crate::preset::RenderMode;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]

pub struct TlVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rhw: f32,
    pub color: u32,
    pub u: f32,
    pub v: f32,
}

pub const DEFAULT_Z: f32 = 0.01;

pub const DEFAULT_RHW: f32 = 1.0;

impl TlVertex {
    pub const STRIDE: u64 = 28;

    pub fn new(x: f32, y: f32, u: f32, v: f32, color: u32) -> TlVertex {
        TlVertex {
            x,
            y,
            z: DEFAULT_Z,
            rhw: DEFAULT_RHW,
            color,
            u,
            v,
        }
    }

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: TlVertex::STRIDE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Unorm8x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 20,
                shader_location: 2,
            },
        ],
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Quad {
    pub normalized_corners: Option<[[f32; 2]; 4]>,
    pub clip: Option<[f32; 4]>,
    pub dst_x: f32,
    pub dst_y: f32,
    pub dst_w: f32,
    pub dst_h: f32,
    pub src_x: f32,
    pub src_y: f32,
    pub src_w: f32,
    pub src_h: f32,
    pub color: u32,
    pub rotation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum UvRule {
    #[default]
    TexelCentre,
    TexelEdge,
}

impl Quad {
    pub fn whole(x: f32, y: f32, tex_w: u32, tex_h: u32, color: u32) -> Quad {
        Quad {
            normalized_corners: None,
            clip: None,
            dst_x: x,
            dst_y: y,
            dst_w: tex_w as f32,
            dst_h: tex_h as f32,
            src_x: 0.0,
            src_y: 0.0,
            src_w: tex_w as f32,
            src_h: tex_h as f32,
            color,
            rotation: 0.0,
        }
    }

    pub fn vertices(&self, tex_w: u32, tex_h: u32) -> [TlVertex; 4] {
        self.vertices_with(tex_w, tex_h, UvRule::TexelCentre)
    }

    pub fn vertices_with(&self, tex_w: u32, tex_h: u32, rule: UvRule) -> [TlVertex; 4] {
        let (tw, th) = (tex_w as f32, tex_h as f32);

        let inset = match rule {
            UvRule::TexelCentre => 0.5,
            UvRule::TexelEdge => 0.0,
        };

        let u0 = self.src_x / tw + inset / tw;

        let u1 = (self.src_x + self.src_w) / tw - inset / tw;

        let v0 = self.src_y / th + inset / th;

        let v1 = (self.src_y + self.src_h) / th - inset / th;

        let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = self.corners_with(rule);

        [
            TlVertex::new(x0, y0, u0, v0, self.color),
            TlVertex::new(x1, y1, u1, v0, self.color),
            TlVertex::new(x2, y2, u1, v1, self.color),
            TlVertex::new(x3, y3, u0, v1, self.color),
        ]
    }

    pub fn is_rotated(&self) -> bool {
        self.rotation >= 0.1 || self.rotation < -0.1
    }

    pub fn corners(&self) -> [(f32, f32); 4] {
        self.corners_with(UvRule::TexelCentre)
    }

    pub fn corners_with(&self, rule: UvRule) -> [(f32, f32); 4] {
        if let Some(corners) = self.normalized_corners {
            let (s, c) = self.rotation.sin_cos();

            return corners.map(|[x, y]| {
                let (x, y) = ((x - 0.5) * self.dst_w, (y - 0.5) * self.dst_h);

                (
                    self.dst_x + self.dst_w * 0.5 + x * c - y * s,
                    self.dst_y + self.dst_h * 0.5 + x * s + y * c,
                )
            });
        }

        let rotated = if rule == UvRule::TexelEdge {
            self.rotation != 0.0
        } else {
            self.is_rotated()
        };

        if !rotated {
            let (x1, y1) = (self.dst_x + self.dst_w, self.dst_y + self.dst_h);

            return [
                (self.dst_x, self.dst_y),
                (x1, self.dst_y),
                (x1, y1),
                (self.dst_x, y1),
            ];
        }

        let (hw, hh) = (self.dst_w * 0.5, self.dst_h * 0.5);

        let cx = (self.dst_x + self.dst_x + self.dst_w) * 0.5;

        let cy = (self.dst_y + self.dst_y + self.dst_h) * 0.5;

        let (s, c) = (self.rotation.sin(), self.rotation.cos());

        let at = |u: f32, v: f32| (cx + u * c - v * s, cy + u * s + v * c);

        [at(-hw, -hh), at(hw, -hh), at(hw, hh), at(-hw, hh)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Batch<K = u32> {
    pub mode: RenderMode,
    pub clip: Option<[f32; 4]>,
    pub texture: K,
    pub first_quad: u32,
    pub quad_count: u32,
}

impl<K> Batch<K> {
    pub fn index_range(&self) -> std::ops::Range<u32> {
        let first = self.first_quad * 6;

        first..first + self.quad_count * 6
    }
}

#[derive(Debug)]

pub struct SpriteBatcher<K = u32> {
    vertices: Vec<TlVertex>,
    batches: Vec<Batch<K>>,
    quads: u32,
}

impl<K> Default for SpriteBatcher<K> {
    fn default() -> SpriteBatcher<K> {
        SpriteBatcher {
            vertices: Vec::new(),
            batches: Vec::new(),
            quads: 0,
        }
    }
}

impl<K: Copy + Eq> SpriteBatcher<K> {
    pub fn new() -> SpriteBatcher<K> {
        SpriteBatcher::default()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();

        self.batches.clear();

        self.quads = 0;
    }

    pub fn push(&mut self, mode: RenderMode, texture: K, quad: &Quad, tex_w: u32, tex_h: u32) {
        self.push_with(mode, texture, quad, tex_w, tex_h, UvRule::TexelCentre);
    }

    pub fn push_with(
        &mut self,
        mode: RenderMode,
        texture: K,
        quad: &Quad,
        tex_w: u32,
        tex_h: u32,
        rule: UvRule,
    ) {
        self.vertices
            .extend_from_slice(&quad.vertices_with(tex_w, tex_h, rule));

        match self.batches.last_mut() {
            Some(b) if b.mode == mode && b.texture == texture && b.clip == quad.clip => {
                b.quad_count += 1;
            }
            _ => self.batches.push(Batch {
                mode,
                clip: quad.clip,
                texture,
                first_quad: self.quads,
                quad_count: 1,
            }),
        }

        self.quads += 1;
    }

    pub fn vertices(&self) -> &[TlVertex] {
        &self.vertices
    }

    pub fn batches(&self) -> &[Batch<K>] {
        &self.batches
    }

    pub fn quad_count(&self) -> u32 {
        self.quads
    }

    pub fn is_empty(&self) -> bool {
        self.quads == 0
    }
}

pub fn indices(quads: u32) -> Vec<u16> {
    let mut out = Vec::with_capacity(quads as usize * 6);

    for q in 0..quads as u16 {
        let b = q * 4;

        out.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    out
}

pub const SPRITE_WGSL: &str = include_str!("shaders/sprite.wgsl");

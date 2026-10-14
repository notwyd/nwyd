use super::objects::{BoundTexture, MESH_TEXTURE_LIST};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::texture::texture_list;
use formats::world::objects::{ObjRecord, ObjectClass};
use formats::world::{QUAD_SIZE, zone_origin};
use glam::Vec3;
use render::camera::Matrix4;
use render::device::Gpu;
use render::sea::{SeaDraw, SeaPass, SeaRenderer, SeaTexture, SeaUniforms, SeaVertex};

pub const BASE_TEXTURE: usize = 2;

pub const DETAIL_TEXTURE: usize = 3;

pub const INDOOR_TEXTURES: [usize; 2] = [8, 9];

pub const LAVA_TEXTURE: usize = 406;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    Outdoor,
    Lava,
    Indoor,
}

impl Branch {
    fn textures(self) -> [(bool, usize); 2] {
        match self {
            Self::Outdoor => [(false, 2), (false, 3)],
            Self::Lava => [(true, 406), (true, 406)],
            Self::Indoor => [(false, 8), (false, 9)],
        }
    }
}

pub const SCROLL_PERIOD_MS: u32 = 12000;

pub const SCROLL_DIVISOR: f32 = 12000.0;

pub const WAVE_DIVISOR: f32 = 6000.0;

pub const SCROLL_ROWS: f32 = 3.0;

pub const WAVE_AMPLITUDE: f32 = 0.05;

pub const WAVE_OFFSET: f32 = 0.1;

pub const WAVE_COLUMNS: f32 = 0.5;

pub const BASE_UV_SCALE: f32 = 0.5;

pub const DETAIL_UV_DIVISOR: f32 = 12.0;

pub const SCENE_LOGIN: u32 = 0x7534;

pub const SCENE_IN_WORLD: u32 = 0x7530;

pub fn scroll_v(row: i32, now_ms: u32) -> f32 {
    row as f32 / SCROLL_ROWS + (now_ms % SCROLL_PERIOD_MS) as f32 / SCROLL_DIVISOR
}

pub fn wave_height(col: i32, now_ms: u32) -> f32 {
    let phase = (now_ms % SCROLL_PERIOD_MS) as f32 / WAVE_DIVISOR;

    let angle = std::f32::consts::PI * (WAVE_COLUMNS * col as f32 + 2.0 * phase);

    angle.sin() * WAVE_AMPLITUDE - WAVE_OFFSET
}

pub fn in_lava_box(x: f32, z: f32) -> bool {
    let (col, row) = (x as i32 >> 7, z as i32 >> 7);

    (28..=29).contains(&col) && (22..=23).contains(&row)
}

pub fn blend_enabled(scene_state: u32, x: f32, z: f32) -> bool {
    let (col, row) = (x as i32 >> 7, z as i32 >> 7);

    let opaque =
        scene_state == SCENE_IN_WORLD && (27..=30).contains(&col) && (21..=24).contains(&row);

    !opaque
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sea {
    pub zone: (u8, u8),
    pub environment: i32,
    pub position: Vec3,
    pub cols: i32,
    pub rows: i32,
    pub blended: bool,
    pub vertices: Vec<SeaVertex>,
    pub indices: Vec<u16>,
}

impl Sea {
    pub fn branch(&self) -> Branch {
        if in_lava_box(self.position.x, self.position.z) {
            Branch::Lava
        } else if matches!(self.environment, 0 | 3 | 4) {
            Branch::Outdoor
        } else {
            Branch::Indoor
        }
    }

    pub fn pass(&self) -> SeaPass {
        match (self.branch(), self.blended) {
            (Branch::Lava, _) => SeaPass::Lava,
            (Branch::Outdoor, true) => SeaPass::Outdoor,
            (Branch::Outdoor, false) => SeaPass::OutdoorOpaque,
            (Branch::Indoor, true) => SeaPass::Indoor,
            (Branch::Indoor, false) => SeaPass::IndoorOpaque,
        }
    }

    pub fn contains(&self, x: f32, z: f32) -> bool {
        if !x.is_finite() || !z.is_finite() {
            return false;
        }

        let (x, z) = (x as i32, z as i32);

        x >= (self.position.x - self.cols as f32) as i32
            && x < (self.position.x + self.cols as f32) as i32
            && z >= (self.position.z - self.rows as f32) as i32
            && z < (self.position.z + self.rows as f32) as i32
    }

    pub fn new(record: &ObjRecord, zone: (u8, u8), scene_state: u32) -> Option<Sea> {
        let (ox, oz) = zone_origin(u32::from(zone.0), u32::from(zone.1));

        let position = Vec3::new(record.x + ox, record.y, record.z + oz);

        let (cols, rows) = (record.p1 / 2, record.p0 / 2);

        if cols <= 0 || rows <= 0 {
            return None;
        }

        let vertex_count = i64::from(cols + 1) * i64::from(rows + 1);

        if vertex_count > i64::from(u16::MAX) {
            return None;
        }

        let mut sea = Sea {
            zone,
            environment: super::world::environment_type(zone.0, zone.1),
            position,
            cols,
            rows,
            blended: blend_enabled(scene_state, position.x, position.z),
            vertices: Vec::with_capacity(vertex_count as usize),
            indices: Vec::with_capacity((rows * cols * 6) as usize),
        };

        sea.generate();

        Some(sea)
    }

    fn generate(&mut self) {
        let (cols, rows) = (self.cols, self.rows);

        for row in 0..=rows {
            for col in 0..=cols {
                self.vertices.push(SeaVertex {
                    position: [
                        (col - cols / 2) as f32 * QUAD_SIZE,
                        0.0,
                        (row - rows / 2) as f32 * QUAD_SIZE,
                    ],
                    diffuse: 0,
                    uv0: [col as f32 * BASE_UV_SCALE, row as f32 * BASE_UV_SCALE],
                    uv1: [
                        col as f32 / DETAIL_UV_DIVISOR,
                        row as f32 / DETAIL_UV_DIVISOR,
                    ],
                });
            }
        }

        let stride = cols + 1;

        for row in 0..rows {
            for col in 0..cols {
                let a = (stride * row + col) as u16;

                let b = (stride * (row + 1) + col) as u16;

                self.indices.extend([a, b, a + 1, b, b + 1, a + 1]);
            }
        }
    }

    pub fn update(&mut self, now_ms: u32) {
        let stride = self.cols + 1;

        let branch = self.branch();

        for row in 0..=self.rows {
            let v = scroll_v(row, now_ms);

            for col in 0..=self.cols {
                let vertex = &mut self.vertices[(stride * row + col) as usize];

                match branch {
                    Branch::Outdoor => {
                        vertex.uv0 = [col as f32 * 0.5, row as f32 * 0.5];

                        vertex.uv1 = [col as f32 / 12.0, v];

                        vertex.position[1] = wave_height(col, now_ms);
                    }
                    Branch::Indoor => {
                        vertex.uv0 = [
                            col as f32 / 1.8,
                            row as f32 / 1.2 + (now_ms % 12000) as f32 / 12000.0,
                        ];

                        vertex.uv1 = [col as f32 / 12.0, row as f32 / 12.0];

                        vertex.position[1] = wave_height(col, now_ms)
                            + if self.environment == 2 { 0.05 } else { 0.0 };
                    }
                    Branch::Lava => {
                        let phase = (now_ms % 18000) as f32 / 18000.0;

                        vertex.uv0 = [col as f32 / 9.5, row as f32 / 9.5 + phase];

                        vertex.uv1 = [col as f32 / 3.8 + phase, row as f32 / 3.8];

                        vertex.position[1] = (std::f32::consts::PI * col as f32 / 4.5
                            + 2.0 * (now_ms % 12000) as f32 / 6000.0)
                            .sin()
                            * 0.9
                            - 0.1;
                    }
                }
            }
        }
    }

    pub fn radius(&self) -> f32 {
        let half = |n: i32| (n - n / 2).max(n / 2) as f32 * QUAD_SIZE;

        half(self.cols).hypot(half(self.rows))
    }

    pub fn world(&self) -> Matrix4 {
        Matrix4::multiply(
            &super::objects::DEVICE_WORLD,
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        )
    }

    pub fn triangles(&self) -> usize {
        (self.rows * self.cols * 2) as usize
    }
}

pub fn build(records: &[ObjRecord], zone: (u8, u8), scene_state: u32) -> Vec<Sea> {
    records
        .iter()
        .filter(|r| r.class() == ObjectClass::Sea)
        .filter_map(|r| Sea::new(r, zone, scene_state))
        .collect()
}

#[derive(Debug)]
pub struct SeaScene {
    pub seas: Vec<Sea>,
    pub textures: Vec<BoundTexture>,
    pub missing: Vec<usize>,
}

impl SeaScene {
    pub fn build(game: &GameDir) -> Result<SeaScene> {
        let list = texture_list::parse(
            &game
                .read(MESH_TEXTURE_LIST)
                .context("reading mesh\\MeshTextureList.bin")?,
        )
        .context("parsing MeshTextureList.bin")?;

        let mut scene = SeaScene {
            seas: Vec::new(),
            textures: Vec::new(),
            missing: Vec::new(),
        };

        let effects = texture_list::parse(&game.read(super::objects::EFFECT_TEXTURE_LIST)?)?;

        for (effect_table, slot) in [(false, 2), (false, 3), (false, 8), (false, 9), (true, 406)] {
            let decoded = (if effect_table { &effects } else { &list })
                .get(slot)
                .and_then(|e| Some((e, game.read(&e.path).ok()?)))
                .and_then(|(e, bytes)| Some((e, formats::texture::decode(&bytes).ok()?)));

            match decoded {
                Some((entry, texture)) => scene.textures.push(BoundTexture {
                    pending: false,
                    effect_table,
                    slot,
                    path: entry.path_str(),
                    flag: entry.format_flag,
                    texture,
                }),
                None => scene.missing.push(slot),
            }
        }

        Ok(scene)
    }

    pub fn empty() -> SeaScene {
        SeaScene {
            seas: Vec::new(),
            textures: Vec::new(),
            missing: Vec::new(),
        }
    }

    pub fn bound(&self) -> bool {
        [Branch::Outdoor, Branch::Indoor, Branch::Lava]
            .into_iter()
            .any(|branch| self.texture_pair(branch).is_some())
    }

    fn texture_pair(&self, branch: Branch) -> Option<[usize; 2]> {
        let [a, b] = branch.textures().map(|(effect, slot)| {
            self.textures
                .iter()
                .position(|t| t.effect_table == effect && t.slot == slot)
        });

        Some([a?, b?])
    }

    fn pairs(&self) -> Vec<(Branch, [usize; 2])> {
        [Branch::Outdoor, Branch::Indoor, Branch::Lava]
            .into_iter()
            .filter_map(|branch| self.texture_pair(branch).map(|pair| (branch, pair)))
            .collect()
    }

    pub fn update_environment(&mut self, now_ms: u32, environment: i32) {
        for sea in &mut self.seas {
            sea.environment = environment;

            sea.update(now_ms);
        }
    }

    pub fn update(&mut self, now_ms: u32) {
        for sea in &mut self.seas {
            sea.update(now_ms);
        }
    }

    pub fn seeded(&self, now_ms: u32) -> Vec<Sea> {
        let mut seeded = self.seas.clone();

        for sea in &mut seeded {
            sea.update(now_ms);
        }

        seeded
    }

    pub fn frame(&self, reach: super::actors::Reach) -> Frame {
        self.frame_of(&self.seas, reach)
    }

    pub fn frame_of(&self, seas: &[Sea], reach: super::actors::Reach) -> Frame {
        let mut frame = Frame::default();

        let pairs = self.pairs();

        for sea in seas {
            if !reach.admits_within(sea.position, sea.radius()) {
                continue;
            }

            let Some(texture) = pairs.iter().position(|(branch, _)| *branch == sea.branch()) else {
                frame.unbound += 1;

                continue;
            };

            let object = frame.objects.len() as u32;

            let base_vertex = frame.vertices.len() as u32;

            let first_index = frame.indices.len() as u32;

            let mut object_uniforms = SeaUniforms::new(sea.world());

            object_uniforms.material = match sea.branch() {
                Branch::Outdoor => [0.5, 0.1, 2.0, 1.0],
                Branch::Lava => [0.9, 0.9, 1.0, 1.0],
                Branch::Indoor => [0.5, 0.0, 1.0, 0.0],
            };

            frame.objects.push(object_uniforms);

            frame.vertices.extend_from_slice(&sea.vertices);

            frame.indices.extend_from_slice(&sea.indices);

            frame.draws.push(SeaDraw {
                pass: sea.pass(),
                object,
                texture,
                base_vertex,
                indices: first_index..frame.indices.len() as u32,
            });

            frame.triangles += sea.triangles();
        }

        frame
    }

    pub fn upload_textures(&self, gpu: &Gpu, renderer: &SeaRenderer) -> Vec<SeaTexture> {
        if !self.bound() {
            return Vec::new();
        }

        let uploaded: Vec<_> = self
            .textures
            .iter()
            .map(|t| {
                render::texture::upload(
                    gpu,
                    &t.texture,
                    t.alpha_policy(),
                    &format!("sea texture {}", t.path),
                )
            })
            .collect();

        self.pairs()
            .into_iter()
            .map(|(_, [a, b])| renderer.bind_textures(gpu, &uploaded[a].view, &uploaded[b].view))
            .collect()
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Frame {
    pub objects: Vec<SeaUniforms>,
    pub vertices: Vec<SeaVertex>,
    pub indices: Vec<u16>,
    pub draws: Vec<SeaDraw>,
    pub triangles: usize,
    pub unbound: usize,
}

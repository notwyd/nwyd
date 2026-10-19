use super::effect_mesh::EffectMesh;
use super::shade::Shade;
use formats::world::objects::ObjRecord;
use formats::world::trn::Terrain;
use glam::Vec3;

pub fn particle_tint(kind: u32) -> Option<u32> {
    match kind {
        0x1F5 | 0x1F6 => Some(0x00FF_AA00),
        0x1F7 => Some(0x00AA_00FF),
        _ => None,
    }
}

pub fn particle_shade_colour(kind: u32) -> Option<u32> {
    match kind {
        0x1F5 | 0x1F6 => Some(0x3333_1100),
        0x1F7 => Some(0x0001_1033),
        _ => None,
    }
}

pub const PARTICLE_SHADE_GRID: i32 = 5;

pub const PARTICLE_SHADE_GRID_SELECT_CHAR: i32 = 9;

pub const PARTICLE_SHADE_TEXTURE: usize = 7;

pub const GLOW_LIGHT: u32 = 0xFFFF_DDDD;

pub const GLOW_SHADE_GRID: i32 = 3;

pub const GLOW_SHADE_GRID_SELECT_CHAR: i32 = 5;

pub const GLOW_SHADE_TEXTURE: usize = 0x76;

pub const GLOW_SHADE_COLOUR: u32 = 0x8899_8888;

pub const GLOW_MESH_ID: i32 = 0x1FA;

pub const GLOW_COLOUR: u32 = 0x4455_4444;

pub const GLOW_COLOUR_SELECT_CHAR: u32 = 0xFF88_7777;

pub const GLOW_COLOUR_FIELD: u32 = 0xFF22_2288;

pub const GLOW_PERIOD_MS: u32 = 5000;

pub const GLOW_PERIOD_MS_SELECT_CHAR: u32 = 10_000;

pub const FLARE_TINT: u32 = 0x0022_3333;

pub const FLARE_KINDS: std::ops::RangeInclusive<u32> = 0x208..=0x212;

pub const DUST_KIND: u32 = 0x213;

pub const DUST_MODE: i32 = 0;

pub const AURORA_MESH_ID: i32 = 0x214;

pub const AURORA_TINT: u32 = 0xAAAA_AAAA;

pub const AURORA_PERIOD_MS: u32 = 5000;

pub const PULSE_AND_SCROLL: u16 = 0x0101;

fn apply_pulse_and_scroll(mesh: &mut EffectMesh) {
    let [pulse, scroll] = PULSE_AND_SCROLL.to_le_bytes();

    mesh.pulse = pulse != 0;

    mesh.uv_scroll = match scroll {
        1 => super::effect_mesh::UvScroll::U,
        2 => super::effect_mesh::UvScroll::V,
        _ => super::effect_mesh::UvScroll::None,
    };
}

pub const WISP_TRUNK_MESH_ID: i32 = 0x7BC;

pub const WISP_GLOW_MESH_ID: i32 = 0x7BB;

pub const WISP_CAP_MESH_ID: i32 = 0x7BD;

pub const WISP_CAP_TINT: u32 = 0x8888_8888;

pub const WISP_OFFSET: (f32, f32) = (4.58, 4.5);

pub const WISP_Y: f32 = 0.3;

pub const WISP_GLOW_SCALE_XZ: f32 = 0.98;

pub const WISP_GLOW_SCALE_Y_DIVISOR: f32 = 15.4;

pub const WISP_GLOW_SCALE_Y_BIAS: f32 = 0.72;

pub const REMAP: &[(u32, i32)] = &[
    (0x5F8, 0x613),
    (0x604, 0x614),
    (0x605, 0x615),
    (0x606, 0x616),
    (0x607, 0x617),
    (0x63D, 0x63E),
];

pub const LOAD_CLOCK_MS: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    Particle,
    Glow,
    Lamp,
    Flare,
    Dust,
    Aurora,
    Remap(i32),
    Wisp,
}

pub fn arm(kind: u32) -> Option<Arm> {
    match kind {
        0x1F5..=0x1F7 => Some(Arm::Particle),
        0x1FA => Some(Arm::Glow),
        0x1FF..=0x206 => Some(Arm::Lamp),
        k if FLARE_KINDS.contains(&k) => Some(Arm::Flare),
        DUST_KIND => Some(Arm::Dust),
        0x214 => Some(Arm::Aurora),
        0x7BC => Some(Arm::Wisp),
        k => REMAP
            .iter()
            .find(|(from, _)| *from == k)
            .map(|(_, to)| Arm::Remap(*to)),
    }
}

pub fn particle_shade_grid(scene: u32) -> i32 {
    match scene == super::world::SCENE_SELECT_CHAR {
        true => PARTICLE_SHADE_GRID_SELECT_CHAR,
        false => PARTICLE_SHADE_GRID,
    }
}

pub fn glow_shade_grid(scene: u32) -> i32 {
    match scene == super::world::SCENE_SELECT_CHAR {
        true => GLOW_SHADE_GRID_SELECT_CHAR,
        false => GLOW_SHADE_GRID,
    }
}

pub fn glow_shade_colour(scene: u32, world_x: f32, world_z: f32) -> u32 {
    let column = (world_x as i32) >> 7;

    if scene == super::world::SCENE_FIELD
        && (0x10 < column && column < 0x14)
        && 0xE80 < ((world_z as i32) & !0x7F)
    {
        return GLOW_COLOUR_FIELD;
    }

    GLOW_SHADE_COLOUR
}

pub fn glow_period_ms(scene: u32) -> u32 {
    match scene == super::world::SCENE_SELECT_CHAR {
        true => GLOW_PERIOD_MS_SELECT_CHAR,
        false => GLOW_PERIOD_MS,
    }
}

pub fn glow_colour(scene: u32, world_x: f32, world_z: f32) -> u32 {
    if scene == super::world::SCENE_SELECT_CHAR {
        return GLOW_COLOUR_SELECT_CHAR;
    }

    let column = (world_x as i32) >> 7;

    if scene == super::world::SCENE_FIELD
        && (0x10 < column && column < 0x14)
        && 0xE80 < ((world_z as i32) & !0x7F)
    {
        return GLOW_COLOUR_FIELD;
    }

    GLOW_COLOUR
}

pub fn glow_sector(scene: u32, cell: i32) -> i32 {
    match scene == super::world::SCENE_SELECT_CHAR {
        true => super::effect_mesh::UNFILED_SECTOR,
        false => cell,
    }
}

pub const LAMP_STEPS: i32 = 5;

pub const LAMP_ORIGIN: (f32, f32) = (-1.0, -3.0);

pub const LAMP_INDEX_START: u32 = 2;

pub const LAMP_GREEN: f32 = 0.6;

pub const LAMP_RANGE: f32 = 5.0;

pub const LAMP_RANGE_LONG: f32 = 7.0;

pub const LAMP_ATTENUATION: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lamp {
    pub index: u32,
    pub position: Vec3,
    pub diffuse: [f32; 4],
    pub range: f32,
}

impl Lamp {
    pub fn visible(&self, eye: Vec3, distance: f32, pitch: f32, scene: u32) -> bool {
        if scene != super::world::SCENE_FIELD {
            return true;
        }

        let high = pitch >= -std::f32::consts::FRAC_PI_8;

        let (size, width, length, shift) = if high {
            (7.2, 1.6, 1.4, 2.0)
        } else {
            (distance.max(5.0) * 0.8 * 2.0, 1.0, 1.0, 0.0)
        };

        let a = eye.z - eye.x - shift + self.position.x;

        let b = eye.x + eye.z + shift - self.position.x;

        let inside = |z: f32| {
            z > a + size / 1.414 - size * 1.414
                && z < a + size * width / 1.414
                && z > b
                && z < b + width * 1.414 * size * length
        };

        inside(self.position.z)
            || inside(self.position.z - self.range)
            || !high && inside(self.position.z + self.range)
    }

    pub fn light(&self) -> render::world::PointLight {
        render::world::PointLight {
            position: [
                self.position.x,
                self.position.y,
                self.position.z,
                self.range,
            ],
            diffuse: self.diffuse,
            attenuation: [LAMP_ATTENUATION, LAMP_ATTENUATION, LAMP_ATTENUATION, 1.0],
        }
    }
}

pub const SAMPLE_SCALE: f32 = 256.0;

pub fn blend(sample: [f32; 4], tint: u32) -> u32 {
    let [tb, tg, tr, ta] = tint.to_le_bytes();

    let byte = |v: f32| ((v * SAMPLE_SCALE).round_ties_even() as i64 as u32) & 0xFF;

    let half = |a: u32, b: u8| (a + u32::from(b)) >> 1;

    (half(byte(sample[3]), ta) << 24)
        | (half(byte(sample[0]), tr) << 16)
        | (half(byte(sample[1]), tg) << 8)
        | half(byte(sample[2]), tb)
}

pub struct Zones<'a> {
    zones: Vec<&'a mut Terrain>,
    registered: Vec<usize>,
}

impl<'a> Zones<'a> {
    pub fn new(zones: Vec<&'a mut Terrain>) -> Zones<'a> {
        Zones {
            zones,
            registered: Vec::new(),
        }
    }

    pub fn registered_zones(&self) -> &[usize] {
        &self.registered
    }

    pub fn owner(&self, x: f32, z: f32) -> Option<usize> {
        self.zones.iter().position(|t| t.contains(x, z))
    }

    pub fn sample(&self, x: f32, z: f32) -> [f32; 4] {
        sample(&self.zones, x, z)
    }

    pub fn register(&mut self, x: f32, z: f32, argb: u32) -> bool {
        let Some(i) = self.owner(x, z) else {
            return false;
        };

        let (ox, oz) = self.zones[i].origin();

        let landed = self.zones[i].set_light(x - ox, z - oz, argb);

        if landed && !self.registered.contains(&i) {
            self.registered.push(i);
        }

        landed
    }
}

pub fn sample<T: std::ops::Deref<Target = Terrain>>(zones: &[T], x: f32, z: f32) -> [f32; 4] {
    match zones.iter().position(|t| t.contains(x, z)) {
        Some(i) => {
            let (ox, oz) = zones[i].origin();

            zones[i].color_at(x - ox, z - oz)
        }
        None => [1.0, 1.0, 1.0, 1.0],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration {
    pub kind: u32,
    pub at: (i32, i32),
    pub colour: u32,
    pub landed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DustSeed {
    pub kind: u32,
    pub position: Vec3,
    pub plume_base_height: f32,
    pub mode: i32,
}

#[derive(Debug, Default, Clone)]
pub struct Lights {
    pub registrations: Vec<Registration>,
    pub shades: Vec<Shade>,
    pub meshes: Vec<EffectMesh>,
    pub glows: Vec<Glow>,
    pub lamps: Vec<Lamp>,
    pub dusts: Vec<DustSeed>,
}

impl Lights {
    pub fn point_lights(&self) -> render::world::PointLights {
        let mut out = render::world::PointLights::default();

        for lamp in &self.lamps {
            out.set(lamp.index, lamp.light());
        }

        out
    }

    pub fn point_lights_at(
        &self,
        eye: Vec3,
        distance: f32,
        pitch: f32,
        scene: u32,
    ) -> render::world::PointLights {
        let mut out = render::world::PointLights::default();

        for lamp in self
            .lamps
            .iter()
            .filter(|lamp| lamp.visible(eye, distance, pitch, scene))
        {
            out.set(lamp.index, lamp.light());
        }

        out
    }

    pub fn push(&mut self, mesh: EffectMesh) {
        if let Some(glow) = glow_of(&mesh) {
            self.glows.push(glow);
        }

        self.meshes.push(mesh);
    }

    pub fn merge(&mut self, other: Lights) {
        self.registrations.extend(other.registrations);

        self.shades.extend(other.shades);

        self.meshes.extend(other.meshes);

        self.glows.extend(other.glows);

        self.lamps.extend(other.lamps);

        self.dusts.extend(other.dusts);
    }
}

pub fn glow_of(mesh: &EffectMesh) -> Option<Glow> {
    if !mesh.draws() {
        return None;
    }

    Some(Glow {
        mesh_id: mesh.mesh_id,
        position: mesh.position,
        yaw: mesh.yaw,
        scale: (mesh.scale_xz, mesh.scale_y),
        colour: mesh.colour,
        pulse: mesh.pulse,
        period_ms: mesh.period_ms,
        sector: mesh.sector,
        uv_scroll: mesh.uv_scroll,
        uv_offset: mesh.uv_offset,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glow {
    pub mesh_id: i32,
    pub position: Vec3,
    pub yaw: f32,
    pub scale: (f32, f32),
    pub colour: u32,
    pub pulse: bool,
    pub period_ms: u32,
    pub sector: i32,
    pub uv_scroll: super::effect_mesh::UvScroll,
    pub uv_offset: [f32; 2],
}

impl Glow {
    pub fn advance_uv(&mut self, now_ms: u32) {
        let phase = if self.period_ms == 0 {
            0.0
        } else {
            (now_ms % self.period_ms) as f32 / self.period_ms as f32
        };

        self.uv_scroll.advance(&mut self.uv_offset, phase, now_ms);
    }

    pub fn tint(&self, now_ms: u32) -> [f32; 4] {
        let k = if self.pulse && self.period_ms != 0 {
            let phase = (now_ms % self.period_ms) as f32 / self.period_ms as f32;

            0.8 + 0.2 * (std::f32::consts::TAU * phase).sin()
        } else {
            1.0
        };

        let byte = |shift: u32| ((self.colour >> shift) & 0xFF) as f32 / 255.0 * k;

        [byte(16), byte(8), byte(0), byte(24)]
    }

    pub fn world(&self) -> render::camera::Matrix4 {
        use render::camera::Matrix4;

        let spun = Matrix4::rotation_yaw_pitch_roll(self.yaw, 0.0, 0.0);

        let scaled = Matrix4::multiply(
            &spun,
            &Matrix4::scaling(self.scale.0, self.scale.1, self.scale.0),
        );

        Matrix4::multiply(
            &scaled,
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        )
    }
}

pub fn build(
    zones: &mut Zones,
    lamps: &mut Lamps,
    records: &[ObjRecord],
    zone: (u8, u8),
    scene: u32,
) -> Lights {
    let mut out = Lights::default();

    for record in records {
        let world = record.world_position(u32::from(zone.0), u32::from(zone.1));

        let position = Vec3::from(world);

        if record.class() == formats::world::objects::ObjectClass::Tree {
            let (x, z) = (position.x + 2.0, position.z - 2.0);

            let landed = zones.register(x, z, 0);

            out.registrations.push(Registration {
                kind: record.kind,
                at: (x as i32, z as i32),
                colour: 0,
                landed,
            });
        }

        let cell = super::emitters::cell_key(position.x, position.z);

        let yaw = record.yaw - std::f32::consts::FRAC_PI_2;

        let [e0, e1] = record.extra.unwrap_or([1.0, 1.0]);

        let Some(taken) = arm(record.kind) else {
            continue;
        };

        match taken {
            Arm::Particle => {
                let (Some(tint), Some(shade)) = (
                    particle_tint(record.kind),
                    particle_shade_colour(record.kind),
                ) else {
                    continue;
                };

                let colour = blend(zones.sample(position.x, position.z), tint);

                let landed = zones.register(position.x, position.z, colour);

                out.registrations.push(Registration {
                    kind: record.kind,
                    at: (position.x as i32, position.z as i32),
                    colour,
                    landed,
                });

                out.shades.push(Shade::new(
                    particle_shade_grid(scene),
                    PARTICLE_SHADE_TEXTURE,
                    shade,
                    position,
                    cell,
                ));
            }
            Arm::Glow => {
                let landed = zones.register(position.x, position.z, GLOW_LIGHT);

                out.registrations.push(Registration {
                    kind: record.kind,
                    at: (position.x as i32, position.z as i32),
                    colour: GLOW_LIGHT,
                    landed,
                });

                let mut mesh = EffectMesh::new(
                    GLOW_MESH_ID,
                    glow_colour(scene, position.x, position.z),
                    yaw,
                    0,
                    LOAD_CLOCK_MS,
                );

                mesh.position = position;

                mesh.scale_xz = e0;

                mesh.scale_y = e1;

                mesh.pulse = true;

                mesh.period_ms = glow_period_ms(scene);

                mesh.sector = glow_sector(scene, cell);

                out.push(mesh);

                out.shades.push(Shade::new(
                    glow_shade_grid(scene),
                    GLOW_SHADE_TEXTURE,
                    glow_shade_colour(scene, position.x, position.z),
                    position,
                    cell,
                ));
            }
            Arm::Flare => {
                let mut mesh =
                    EffectMesh::new(record.kind as i32, FLARE_TINT, yaw, 0, LOAD_CLOCK_MS);

                mesh.position = position;

                mesh.scale_xz = e0;

                mesh.scale_y = e1;

                mesh.sector = cell;

                out.push(mesh);
            }
            Arm::Dust => out.dusts.push(DustSeed {
                kind: record.kind,
                position,
                plume_base_height: e1,
                mode: DUST_MODE,
            }),
            Arm::Aurora => {
                let mut mesh = EffectMesh::new(AURORA_MESH_ID, AURORA_TINT, yaw, 0, LOAD_CLOCK_MS);

                mesh.position = position;

                mesh.scale_xz = e0;

                mesh.scale_y = e1;

                apply_pulse_and_scroll(&mut mesh);

                mesh.period_ms = AURORA_PERIOD_MS;

                mesh.sector = cell;

                out.push(mesh);
            }
            Arm::Remap(mesh_id) => {
                let mut mesh = EffectMesh::new(mesh_id, FLARE_TINT, yaw, 0, LOAD_CLOCK_MS);

                mesh.position = position;

                mesh.sector = cell;

                out.push(mesh);
            }
            Arm::Wisp => {
                let mut trunk =
                    EffectMesh::new(WISP_TRUNK_MESH_ID, AURORA_TINT, yaw, 0, LOAD_CLOCK_MS);

                trunk.position = Vec3::new(position.z, position.y, position.x);

                trunk.sector = cell;

                out.push(trunk);

                let lifted = Vec3::new(
                    position.x + WISP_OFFSET.0,
                    WISP_Y,
                    position.z + WISP_OFFSET.1,
                );

                let mut glow =
                    EffectMesh::new(WISP_GLOW_MESH_ID, AURORA_TINT, yaw, 0, LOAD_CLOCK_MS);

                glow.position = lifted;

                glow.scale_xz = WISP_GLOW_SCALE_XZ;

                glow.scale_y = position.y / WISP_GLOW_SCALE_Y_DIVISOR + WISP_GLOW_SCALE_Y_BIAS;

                apply_pulse_and_scroll(&mut glow);

                glow.period_ms = AURORA_PERIOD_MS;

                glow.sector = cell;

                out.push(glow);

                let mut cap =
                    EffectMesh::new(WISP_CAP_MESH_ID, WISP_CAP_TINT, yaw, 0, LOAD_CLOCK_MS);

                cap.position = lifted;

                cap.sector = cell;

                out.push(cap);
            }
            Arm::Lamp => {
                let e0 = record.extra.map_or(0.0, |[e0, _]| e0);

                let index = lamps.index();

                let rg = lamps.next(e0);

                out.lamps.push(Lamp {
                    index,
                    position,
                    diffuse: [rg.0, rg.1, 0.0, e0],
                    range: lamps.range(index),
                });

                let colour = lamp_colour(rg, e0);

                for step in 0..LAMP_STEPS {
                    let (x, z) = (
                        position.x + LAMP_ORIGIN.0 + step as f32,
                        position.z + LAMP_ORIGIN.1 + step as f32,
                    );

                    let landed = zones.register(x, z, colour);

                    out.registrations.push(Registration {
                        kind: record.kind,
                        at: (x as i32, z as i32),
                        colour,
                        landed,
                    });
                }
            }
        }
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lamps {
    scene: u32,
    index: u32,
}

impl Lamps {
    pub fn new(scene: u32) -> Lamps {
        Lamps {
            scene,
            index: LAMP_INDEX_START,
        }
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn scene(self) -> u32 {
        self.scene
    }

    pub fn range(self, index: u32) -> f32 {
        if self.scene == super::world::SCENE_SELECT_CHAR && index == 6 {
            LAMP_RANGE_LONG
        } else {
            LAMP_RANGE
        }
    }

    pub fn next(&mut self, e0: f32) -> (f32, f32) {
        let mut rg = (e0, e0);

        if self.scene == super::world::SCENE_SELECT_CHAR {
            if self.index.wrapping_sub(2) <= 1 {
                rg = (e0, 0.0);
            } else if self.index < 6 {
                rg = (0.0, LAMP_GREEN);
            }

            self.index += 1;
        }

        rg
    }
}

pub fn lamp_colour(rg: (f32, f32), e0: f32) -> u32 {
    let b = |v: f32| (v * 255.0).round_ties_even() as i64 as u32;

    (b(e0) << 24) | (b(rg.0) << 16) | (b(rg.1) << 8)
}

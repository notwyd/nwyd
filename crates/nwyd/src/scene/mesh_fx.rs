use super::effect_tree::{
    Base, Class, Cull, Effect, MeshSubmission, OwnedGroup, Pass, RotateLit, Sink, Submit, Tick,
};
use super::effects::Billboard;
use super::motion::Owner;
use super::rng::Lcg;
use glam::Vec3;
use render::camera::Matrix4;
use render::world::Material;

pub const START_MESH: i32 = 0x2BF;

pub const CHARGE_MESH_CORE: i32 = 0x2C0;

pub const CHARGE_MESH_SHELL: i32 = 0x2C1;

pub const ROTATE_MESH_UNSET: i32 = -1;

pub const START_LIFE_MS: u32 = 3_000;

pub const START_FREE_STYLE: i32 = 4;

pub const START_FREE_PERIOD_MS: u32 = 2_000;

pub const CHARGE_LIFE_MS: u32 = 1_000;

pub const DRAW_FLOOR: f32 = 0.05;

pub const START_TEXTURES: [(i32, i32); 6] = [
    (0, 0x33),
    (1, 0x34),
    (2, 0x36),
    (3, 0x14),
    (4, 0x33),
    (5, 0x98),
];

pub const CHARGE_TEXTURES: (i32, i32) = (0x39, 0x63);

pub const HUMAN_ROTOR_MESHES: [i32; 7] = [3, 6, 4, 7, 5, 6, 7];

pub fn human_rotors(entity: u16, at: Vec3) -> [MeshRotate; 7] {
    std::array::from_fn(|i| {
        let mut rotor = MeshRotate::new(
            Vec3::new(at.x, at.y + 0.8 + i as f32 * 0.2, at.z),
            0,
            true,
            false,
            0,
        )
        .follow(entity);

        rotor.mesh_id = HUMAN_ROTOR_MESHES[i];

        rotor.start_ms = i as u32 * 150;

        rotor
    })
}

pub fn rotate_lit_material(terrain: [f32; 4], light0: [f32; 4]) -> Material {
    let red = light0[0] * 0.3 + terrain[0] * 0.2;

    let green = light0[1] * 0.3 + red * 0.2;

    let blue = light0[2] * 0.3 + red * 0.2;

    let colour = [red, green, blue, 1.0];

    Material {
        diffuse: colour,
        ambient: colour,
        specular: [1.0, 1.0, 1.0, 0.0],
        emissive: colour,
        power: 0.0,
    }
}

pub fn rotate_lit_world(position: Vec3, yaw: f32, scale: f32) -> Matrix4 {
    let rotation = Matrix4::rotation_yaw_pitch_roll(
        yaw,
        -std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_2,
    );

    let world = Matrix4::multiply(&super::objects::DEVICE_WORLD, &rotation);

    let world = Matrix4::multiply(&world, &Matrix4::scaling(scale, scale, scale));

    Matrix4::multiply(
        &world,
        &Matrix4::translation(position.x, position.y, position.z),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateChild {
    pub kind: usize,
    pub size: Vec3,
    pub frames: u32,
    pub argb: u32,
    pub colour_mode: u32,
}

pub const ROTATE_CHILDREN: [Option<RotateChild>; 4] = [
    Some(RotateChild {
        kind: 0xB,
        size: Vec3::new(0.8, 1.0, 0.8),
        frames: 8,
        argb: 0xFF44_0000,
        colour_mode: 0,
    }),
    Some(RotateChild {
        kind: 0x38,
        size: Vec3::new(1.5, 1.5, 1.5),
        frames: 1,
        argb: 0xFF55_AAFF,
        colour_mode: 2,
    }),
    None,
    None,
];

pub const ROTATE_CHILD_FRAME_MS: u32 = 0x50;

pub const ROTATE_CHILD_LIFETIME_MS: u32 = 0;

pub const ROTATE_CHILD_GROWTH: f32 = 0.0;

pub const ROTATE_CHILD_ADDITIVE: bool = true;

pub const ROTATE_CHILD_LIFT: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateSpark {
    pub kind: usize,
    pub lifetime_ms: u32,
    pub modulus: u32,
    pub bias: u32,
    pub step: f32,
    pub step_y: f32,
    pub offset: f32,
    pub growth: f32,
    pub argb: u32,
    pub motion: u32,
    pub amp_rise: f32,
    pub amp_h: Option<f32>,
    pub sink: f32,
}

pub const ROTATE_SPARKS: [Option<RotateSpark>; 4] = [
    None,
    Some(RotateSpark {
        kind: 0,
        lifetime_ms: 800,
        modulus: 5,
        bias: 0,
        step: 0.05,
        step_y: 0.1,
        offset: 0.02,
        growth: 0.0005,
        argb: 0xFF66_88AA,
        motion: 1,
        amp_rise: 0.0,
        amp_h: None,
        sink: 0.0,
    }),
    Some(RotateSpark {
        kind: 0,
        lifetime_ms: 900,
        modulus: 20,
        bias: 5,
        step: 0.2,
        step_y: 0.2,
        offset: 0.0,
        growth: 0.0005,
        argb: 0xFF66_77CC,
        motion: 3,
        amp_rise: 2.0,
        amp_h: None,
        sink: 0.0,
    }),
    Some(RotateSpark {
        kind: 0,
        lifetime_ms: 1_500,
        modulus: 20,
        bias: 3,
        step: 0.2,
        step_y: 0.2,
        offset: 0.0,
        growth: 1e-5,
        argb: 0xFFFF_0000,
        motion: 3,
        amp_rise: 5.0,
        amp_h: Some(1.0),
        sink: 0.8,
    }),
];

pub const ROTATE_SPARK_FRAMES: u32 = 1;

pub fn rotate_child(mode: i32, at: Vec3) -> Option<Billboard> {
    let row = (*ROTATE_CHILDREN.get(usize::try_from(mode).ok()?)?)?;

    let mut child = Billboard::new(
        row.kind,
        ROTATE_CHILD_LIFETIME_MS,
        row.size,
        ROTATE_CHILD_GROWTH,
        row.frames,
        ROTATE_CHILD_FRAME_MS,
    );

    child.set_color(row.argb);

    child.pass = render::billboard::BillboardPass::Additive;

    child.colour_mode = row.colour_mode;

    child.position = at;

    Some(child)
}

pub const START_CLASS: Class = Class {
    name: "EffectStart",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const CHARGE_CLASS: Class = Class {
    name: "EffectCharge",
    method_count: 16,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const ROTATE_CLASS: Class = Class {
    name: "EffectMeshRotate",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub fn pulse(t: f32) -> f32 {
    (std::f32::consts::PI * t).sin().abs()
}

fn byte_of(v: f32) -> u32 {
    v.round_ties_even().clamp(0.0, 255.0) as u32
}

#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    pub style: i32,
    pub position: Vec3,
    pub life_ms: u32,
    pub born_ms: u32,
    pub progress: f32,
    pub yaw: f32,
    pub scale_xz: f32,
    pub grey: u8,
    pub owner: Option<Owner>,
    pub owner_key: Option<super::effect_context::ActorKey>,
    pub detail: i32,
    gone: bool,
}

impl Start {
    pub fn new(position: Vec3, style: i32, owner: Option<Owner>, now_ms: u32) -> Start {
        Start {
            style,
            position,
            life_ms: match style == START_FREE_STYLE {
                true => 0,
                false => START_LIFE_MS,
            },
            born_ms: now_ms,
            progress: 0.0,
            yaw: 0.0,
            scale_xz: 1.0,
            grey: 0,
            owner,
            owner_key: None,
            detail: 0,
            gone: false,
        }
    }

    pub fn texture_base(&self) -> i32 {
        START_TEXTURES
            .iter()
            .find(|(style, _)| *style == self.style)
            .map_or(self.style + 0x33, |(_, base)| *base)
    }

    fn scale_for(&self, k: f32) -> f32 {
        match self.style {
            1 => self.progress * 0.7 + 0.2,
            4 => {
                let mounted = self.owner.is_some_and(|owner| owner.seat.is_some());

                let coefficient = match mounted {
                    true => 0.7,
                    false => 0.3,
                };

                coefficient * k + 0.7
            }
            5 => k * 0.5 + 0.3,
            _ => k * 0.5 + 0.5,
        }
    }

    pub fn draw(&self) -> Option<super::effect_tree::TexturedMesh> {
        if self.gone || self.detail == 1 || self.progress < DRAW_FLOOR {
            return None;
        }

        let yaw = if self.style == 1 { 0.0 } else { self.yaw };

        let rotation = Matrix4::rotation_yaw_pitch_roll(
            yaw,
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
        );

        let world = Matrix4::multiply(
            &Matrix4::multiply(
                &rotation,
                &Matrix4::scaling(self.scale_xz, 1.0, self.scale_xz),
            ),
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        );

        let grey = f32::from(self.grey) / 255.0;

        Some(super::effect_tree::TexturedMesh {
            mesh_id: 703,
            world,
            uv_offset: [0.0; 2],
            texture: usize::try_from(self.texture_base()).ok()?,
            tint: [grey, grey, grey, 1.0],
            blend: 1,
        })
    }
}

impl Effect for Start {
    fn class(&self) -> &'static Class {
        &START_CLASS
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.detail = live.detail;

        if let Some(key) = self.owner_key {
            self.owner = (live.body)(key).map(|body| Owner {
                position: body.position,
                seat: body.appearance.filter(|a| a.mounted).map(|_| body.position),
            });

            if self.owner.is_none() {
                self.owner_key = None;
            }
        }

        self.update(tick);
    }

    fn update(&mut self, tick: &Tick) {
        let elapsed = tick.now_ms.wrapping_sub(self.born_ms);

        self.progress = match self.life_ms {
            0 => (elapsed % START_FREE_PERIOD_MS) as f32 / START_FREE_PERIOD_MS as f32,
            life => elapsed as f32 / life as f32,
        };

        if self.progress > 1.0 {
            self.gone = self.life_ms != 0;

            return;
        }

        let k = pulse(self.progress);

        self.grey = byte_of(k * 255.0) as u8;

        self.scale_xz = self.scale_for(k);

        self.yaw = self.progress * std::f32::consts::PI;

        if let Some(owner) = self.owner {
            self.position = owner.position;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        if let Some(draw) = self.draw() {
            out.mesh.push(MeshSubmission::Textured(draw));
        }
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Charge {
    pub mode: i32,
    pub position: Vec3,
    pub born_ms: u32,
    pub progress: f32,
    pub yaw: f32,
    pub rgb: (u32, u32, u32),
    pub k: f32,
    pub owner: Option<Owner>,
    pub owner_key: Option<super::effect_context::ActorKey>,
    pub detail: i32,
    gone: bool,
}

impl Charge {
    pub fn new(owner: Option<Owner>, mode: i32, argb: u32, now_ms: u32) -> Charge {
        Charge {
            mode,
            position: owner.map_or(Vec3::ZERO, |owner| owner.position),
            born_ms: now_ms,
            progress: 0.0,
            yaw: 0.0,
            rgb: ((argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF),
            k: 0.0,
            owner,
            owner_key: None,
            detail: 0,
            gone: false,
        }
    }

    pub fn texture_base(&self) -> i32 {
        match self.mode {
            0 => CHARGE_TEXTURES.0,
            _ => CHARGE_TEXTURES.1,
        }
    }

    pub fn scale(&self) -> f32 {
        let base = match self.owner.is_some() {
            true => 1.8,
            false => 1.0,
        };

        match self.mode {
            3 => base * 1.4,
            _ => base * 0.8,
        }
    }

    fn life_ms(&self) -> u32 {
        CHARGE_LIFE_MS
    }

    pub fn draw(&self) -> Vec<super::effect_tree::TexturedMesh> {
        if self.gone || self.detail == 1 || self.progress < DRAW_FLOOR {
            return Vec::new();
        }

        let rotation = Matrix4::rotation_yaw_pitch_roll(
            -self.yaw,
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
        );

        let world = Matrix4::multiply(
            &Matrix4::multiply(
                &rotation,
                &Matrix4::scaling(self.scale(), 1.0, self.scale()),
            ),
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        );

        let channel = |v: u32| byte_of(v as f32 * self.k) as f32 / 255.0;

        let tint = [
            channel(self.rgb.0),
            channel(self.rgb.1),
            channel(self.rgb.2),
            1.0,
        ];

        [704, 705]
            .into_iter()
            .map(|mesh_id| super::effect_tree::TexturedMesh {
                uv_offset: [0.0; 2],
                mesh_id,
                world,
                texture: self.texture_base() as usize,
                tint,
                blend: 1,
            })
            .collect()
    }
}

impl Effect for Charge {
    fn class(&self) -> &'static Class {
        &CHARGE_CLASS
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.detail = live.detail;

        if let Some(key) = self.owner_key {
            self.owner = (live.body)(key).map(|body| Owner {
                position: body.position,
                seat: body.appearance.filter(|a| a.mounted).and(body.skill_anchor),
            });

            if self.owner.is_none() {
                self.owner_key = None;
            }
        }

        self.update(tick);
    }

    fn update(&mut self, tick: &Tick) {
        let elapsed = tick.now_ms.wrapping_sub(self.born_ms);

        if self.life_ms() != 0 {
            self.progress = elapsed as f32 / self.life_ms() as f32;
        }

        if self.progress > 1.0 {
            match self.mode {
                2 => self.born_ms = tick.now_ms,
                _ => self.gone = true,
            }

            return;
        }

        self.k = pulse(self.progress);

        let lift = match self.mode {
            3 => self.k,
            _ => 0.0,
        };

        if let Some(owner) = self.owner {
            self.position = owner.seat.unwrap_or(owner.position);

            self.position.y += lift;
        }

        if self.mode == 1 {
            self.position.y += self.progress;
        }

        self.yaw =
            (self.progress * std::f32::consts::PI * 0.5).sin().abs() * std::f32::consts::PI * 6.0;
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        out.mesh
            .extend(self.draw().into_iter().map(MeshSubmission::Textured));
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshRotate {
    pub mode: i32,
    pub mesh_id: i32,
    pub home: Vec3,
    owner: Option<u16>,
    pub owner_key: Option<super::effect_context::ActorKey>,
    pub affect_slot: Option<u16>,
    detail: i32,
    owner_disabled: bool,
    visible: bool,
    child_disabled: bool,
    pub position: Vec3,
    pub start_ms: u32,
    pub period_ms: u32,
    pub life_ms: u32,
    pub anchor_exact: bool,
    pub additive: bool,
    pub scale: f32,
    pub argb: u32,
    pub yaw: f32,
    pub phase: f32,
    child: Option<Billboard>,
    child_position: std::rc::Rc<std::cell::Cell<Vec3>>,
    sparks: Vec<Billboard>,
    gone: bool,
}

#[derive(Debug)]
struct HumanRotorChild {
    owner: u16,
    billboard: Billboard,
    position: std::rc::Rc<std::cell::Cell<Vec3>>,
}

impl Effect for HumanRotorChild {
    fn class(&self) -> &'static Class {
        <Billboard as Effect>::class(&self.billboard)
    }

    fn update(&mut self, tick: &Tick) {
        self.billboard.position = self.position.get();

        <Billboard as Effect>::update(&mut self.billboard, tick);
    }

    fn expired(&self) -> bool {
        <Billboard as Effect>::expired(&self.billboard)
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        <Billboard as Effect>::submit(&self.billboard, cx, out);
    }

    fn owned_group(&self) -> Option<OwnedGroup> {
        Some(OwnedGroup::HumanRotors(self.owner))
    }

    fn cull(&self, eye: Vec3, field: bool) -> Cull {
        <Billboard as Effect>::cull(&self.billboard, eye, field)
    }
}

impl MeshRotate {
    pub fn new(
        home: Vec3,
        mode: i32,
        spawn_child: bool,
        anchor_exact: bool,
        now_ms: u32,
    ) -> MeshRotate {
        MeshRotate {
            mode,
            mesh_id: ROTATE_MESH_UNSET,
            home,
            owner: None,
            owner_key: None,
            affect_slot: None,
            detail: 0,
            owner_disabled: false,
            visible: true,
            child_disabled: false,
            position: home,
            start_ms: now_ms,
            period_ms: match mode == 1 {
                true => 2_000,
                false => 1_000,
            },
            life_ms: 0,
            anchor_exact,
            additive: false,
            scale: 1.0,
            argb: 0xFFFF_FFFF,
            yaw: 0.0,
            phase: 0.0,
            child: match spawn_child {
                true => rotate_child(mode, home),
                false => None,
            },
            child_position: std::rc::Rc::new(std::cell::Cell::new(home)),
            sparks: Vec::new(),
            gone: false,
        }
    }

    pub fn follow(mut self, entity: u16) -> MeshRotate {
        self.owner = Some(entity);

        self
    }

    pub fn child(&self) -> Option<&Billboard> {
        self.child.as_ref()
    }

    pub fn sparks(&self) -> &[Billboard] {
        &self.sparks
    }

    pub fn set_color(&mut self, argb: u32) {
        self.argb = argb;
    }

    pub fn tint_scale(&self) -> f32 {
        match self.mode {
            1 => 1.0,
            _ => (self.scale * std::f32::consts::PI).sin(),
        }
    }

    pub fn tint(&self) -> u32 {
        let k = self.tint_scale();

        let channel = |shift: u32| byte_of(((self.argb >> shift) & 0xFF) as f32 * k) << shift;

        channel(24) | channel(16) | channel(8) | channel(0)
    }
}

impl MeshRotate {
    pub fn spark_row(&self) -> Option<RotateSpark> {
        *ROTATE_SPARKS.get(usize::try_from(self.mode).ok()?)?
    }

    pub fn spark(&self, roll: u32) -> Option<Billboard> {
        let row = self.spark_row()?;

        let f = (roll % row.modulus + row.bias) as f32;

        let (xz, y) = (f * row.step + row.offset, f * row.step_y + row.offset);

        let mut spark = Billboard::new(
            row.kind,
            row.lifetime_ms,
            Vec3::new(xz, y, xz),
            row.growth,
            ROTATE_SPARK_FRAMES,
            ROTATE_CHILD_FRAME_MS,
        );

        spark.position = self.position;

        spark.origin = spark.position;

        spark.pass = render::billboard::BillboardPass::Additive;

        spark.motion = super::effects::Motion::from_field(row.motion);

        spark.rise = row.amp_rise;

        if let Some(amp) = row.amp_h {
            spark.amp_h = amp;
        }

        spark.set_color(row.argb);

        Some(spark)
    }

    fn emit(&mut self, rng: &mut Lcg) {
        let roll = rng.next();

        self.position.y -= self.spark_row().map_or(0.0, |row| row.sink);

        if let Some(spark) = self.spark(roll) {
            self.sparks.push(spark);
        }
    }

    pub fn tick_with(&mut self, now_ms: u32, rng: &mut Lcg) {
        if self.period_ms == 0 {
            return;
        }

        let down = self.start_ms.wrapping_sub(now_ms) % self.period_ms;

        self.phase = down as f32 / self.period_ms as f32;

        if self.mode != 1 {
            self.yaw = std::f32::consts::TAU * self.phase * 2.0;
        }

        let angle = std::f32::consts::TAU * self.phase;

        self.position = match self.anchor_exact {
            true => self.home,
            false => Vec3::new(
                self.home.x + angle.cos(),
                self.home.y,
                self.home.z + angle.sin(),
            ),
        };

        let mut progress = self.phase;

        if self.mode == 2 {
            progress = now_ms.wrapping_sub(self.start_ms) as f32 / self.life_ms as f32;
        }

        let child_at = self.position
            + Vec3::Y
                * if self.mode == 1 {
                    0.0
                } else {
                    ROTATE_CHILD_LIFT
                };

        self.child_position.set(child_at);

        if let Some(child) = &mut self.child {
            child.position = child_at;
        }

        match self.mode {
            1 => self.emit(rng),
            2 | 3 => match progress > 1.0 {
                true => self.gone = true,
                false => self.emit(rng),
            },
            _ => {}
        }
    }
}

impl Effect for MeshRotate {
    fn class(&self) -> &'static Class {
        &ROTATE_CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = Lcg::new(self.start_ms);

        self.tick_with(tick.now_ms, &mut rng);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn tracked_entity(&self) -> Option<u16> {
        self.owner.filter(|_| self.owner_key.is_none())
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.detail = live.detail;

        if self.detail == 1 {
            return;
        }

        if let Some(key) = self.owner_key {
            let body = (live.body)(key);

            if self.mode == 1
                && let Some(body) = body
            {
                self.yaw = body.yaw;
            }

            self.resolve_tracked_entity(body.map(|body| body.position));

            if body.is_none() {
                if self.affect_slot.is_some() {
                    self.gone = true;

                    return;
                }

                self.owner_key = None;
            }
        }

        if self.visible {
            self.visible = super::effects::visible_in_scene(live.scene, !self.owner_disabled);

            self.tick_with(tick.now_ms, live.rng);
        }

        if self.affect_slot.is_some()
            && let Some(child) = self.child.as_mut()
        {
            Effect::update(child, tick);

            child.ready = super::effects::visible_in_scene(live.scene, !self.child_disabled);
        }
    }

    fn human_owner_hidden(&mut self) {
        self.owner_disabled = true;

        if self.affect_slot.is_some() {
            self.child_disabled = true;
        }
    }

    fn human_draw_visibility(&mut self, visible: bool) {
        self.visible = visible;

        if !visible && self.affect_slot.is_some() {
            self.child_disabled = true;
        }
    }

    fn resolve_tracked_entity(&mut self, position: Option<Vec3>) {
        match position {
            Some(position) => {
                self.home = Vec3::new(position.x, position.y + 1.0, position.z);
            }
            None => self.owner = None,
        }
    }

    fn owned_group(&self) -> Option<OwnedGroup> {
        if let Some(slot) = self.affect_slot {
            return self.owner_key.map(|key| OwnedGroup::HumanAffect(key, slot));
        }

        self.owner.map(OwnedGroup::HumanRotors)
    }

    fn published_position(&self) -> Option<Vec3> {
        Some(self.position)
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.gone || self.detail == 1 || !self.visible {
            return;
        }

        if self.mesh_id < 1 {
            return;
        }

        match self.additive {
            true => {
                let world = rotate_lit_world(self.position, self.yaw, self.scale);

                let tint = self.tint().to_be_bytes();

                out.mesh
                    .push(MeshSubmission::Authored(super::effect_tree::AuthoredMesh {
                        mesh_id: self.mesh_id as u32,
                        world,
                        texture: None,
                        material: super::effect_tree::MeshMaterial::Effect {
                            blend: 1,
                            uv_offset: [0.0; 2],
                            tint: [
                                tint[1] as f32 / 255.0,
                                tint[2] as f32 / 255.0,
                                tint[3] as f32 / 255.0,
                                tint[0] as f32 / 255.0,
                            ],
                        },
                    }));
            }
            false => out.mesh.push(MeshSubmission::RotateLit(RotateLit {
                mesh_id: self.mesh_id,
                position: self.position,
                world: rotate_lit_world(self.position, self.yaw, self.scale),
                owner: self.owner,
            })),
        }
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        let mut out: Vec<Box<dyn Effect>> = Vec::new();

        if self.affect_slot.is_none()
            && let Some(child) = self.child.take()
        {
            match self.owner {
                Some(owner) => out.push(Box::new(HumanRotorChild {
                    owner,
                    billboard: child,
                    position: self.child_position.clone(),
                })),
                None => out.push(Box::new(child)),
            }
        }

        out.extend(
            self.sparks
                .drain(..)
                .map(|b| Box::new(b) as Box<dyn Effect>),
        );

        out
    }

    fn submit_effect_children(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.gone || self.detail == 1 {
            return;
        }

        if self.affect_slot.is_some()
            && let Some(child) = &self.child
        {
            child.submit(cx, out);
        }
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

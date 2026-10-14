use super::effect_tree::{Effect, Sink, Submit, Tick};
use super::effects::{Motion, SECTOR_UNASSIGNED};
use super::motion::PI;
use super::shade::{Ground, Shade};
use glam::Vec3;

pub const BREATHE_PERIOD_MS: u32 = 2_000;

pub const FLARE_PERIOD_MS: u32 = 4_000;

pub const BREATHE_SWELL: f64 = 0.2_f32 as f64;

pub const FLARE_SWELL: f64 = 3.0;

pub const BREATHE_DIM: f64 = 155.0;

pub const FLARE_DIM: f64 = 255.0;

pub const FLARE_SETTLE: f32 = 0.99;

pub const GLOW_CEILING: i32 = 0xFF;

pub const GLOW_RECORD_KIND: u32 = 2035;

pub const SHIPPED_GLOW_RECORDS: [(&str, f32, f32); 2] = [
    ("Field1830.dat", 2366.5, 3893.5),
    ("Field1830.dat", 2366.5, 3926.5),
];

pub const MODE_CACHE_POSITION: (f32, f32) = (2366.5, 3926.5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glow {
    Off,
    Breathe,
    FlareOnce,
    FlareLoop,
    Unhandled(i32),
}

impl Glow {
    pub const fn from_field(value: i32) -> Glow {
        match value {
            0 => Glow::Off,
            1 => Glow::Breathe,
            2 => Glow::FlareOnce,
            3 => Glow::FlareLoop,
            other => Glow::Unhandled(other),
        }
    }

    pub const fn field(self) -> i32 {
        match self {
            Glow::Off => 0,
            Glow::Breathe => 1,
            Glow::FlareOnce => 2,
            Glow::FlareLoop => 3,
            Glow::Unhandled(v) => v,
        }
    }

    pub const fn suppresses_facing(self) -> bool {
        !matches!(self, Glow::Off)
    }

    pub const fn reachable_in_this_install(self) -> bool {
        matches!(
            self,
            Glow::Off | Glow::Breathe | Glow::FlareOnce | Glow::FlareLoop
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowFrame {
    pub size: Vec3,
    pub argb: u32,
    pub channels: [u8; 4],
}

fn sin(theta: f32) -> f32 {
    (theta as f64).sin() as f32
}

fn wave(now_ms: u32, period_ms: u32) -> (f32, f32) {
    let u = ((now_ms % period_ms) as f64 / period_ms as f64) as f32;

    (u, sin((u as f64 * PI) as f32))
}

fn grey(s: f32, range: f64) -> (u32, [u8; 4]) {
    let c = GLOW_CEILING - (s as f64 * range) as i32;

    let c = c as u32;

    let argb = (((c << 8 | c) << 8 | c) << 8) | c;

    (argb, [c as u8; 4])
}

pub fn breathe(now_ms: u32, grown: Vec3) -> GlowFrame {
    let (_, s) = wave(now_ms, BREATHE_PERIOD_MS);

    let swell = (s as f64 * BREATHE_SWELL) as f32;

    let (argb, channels) = grey(s, BREATHE_DIM);

    GlowFrame {
        size: Vec3::new(grown.x + swell, grown.y + swell, grown.z),
        argb,
        channels,
    }
}

pub fn frame(mode: Glow, now_ms: u32, grown: Vec3) -> Option<GlowFrame> {
    match mode {
        Glow::Breathe => Some(breathe(now_ms, grown)),
        Glow::FlareOnce | Glow::FlareLoop => Some(flare(now_ms, grown)),
        Glow::Off | Glow::Unhandled(_) => None,
    }
}

pub fn flare(now_ms: u32, grown: Vec3) -> GlowFrame {
    let (_, s) = wave(now_ms, FLARE_PERIOD_MS);

    let swell = (s as f64 * FLARE_SWELL) as f32;

    let (argb, channels) = grey(s, FLARE_DIM);

    GlowFrame {
        size: Vec3::new(grown.x + swell, grown.y + swell, grown.z),
        argb,
        channels,
    }
}

pub fn next_mode(mode: Glow, now_ms: u32) -> Glow {
    if mode == Glow::FlareOnce && wave(now_ms, FLARE_PERIOD_MS).0 >= FLARE_SETTLE {
        Glow::Breathe
    } else {
        mode
    }
}

pub const ATTACHED_TEXTURE: usize = 7;

pub const ATTACHED_RADIUS: f32 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Attached {
    pub shade: Shade,
    pub placed: bool,
}

impl Attached {
    pub fn new(argb: u32, grid: i32) -> Attached {
        let mut shade = Shade::new(grid, ATTACHED_TEXTURE, argb, Vec3::ZERO, SECTOR_UNASSIGNED);

        shade.radius = ATTACHED_RADIUS;

        shade.pass = render::billboard::BillboardPass::Additive;

        Attached {
            shade,
            placed: false,
        }
    }

    pub fn follow(&mut self, at: Vec3, ground: &Ground<'_>) {
        self.shade.centre = Vec3::new(at.x, 0.0, at.z);

        self.shade.place(ground);

        self.placed = true;
    }

    pub fn update(&mut self, tick: &Tick) {
        Effect::update(&mut self.shade, tick);
    }

    pub fn expired(&self) -> bool {
        Effect::expired(&self.shade)
    }

    pub fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        Effect::submit(&self.shade, cx, out);
    }
}

pub fn attach(slot: &mut Option<Attached>, argb: u32, grid: i32) -> bool {
    let replaced = slot.is_some();

    *slot = Some(Attached::new(argb, grid));

    replaced
}

#[derive(Debug, Clone, PartialEq)]
pub struct BillboardWithShade {
    pub billboard: super::effects::Billboard,
    pub attached: Option<Attached>,
}

impl BillboardWithShade {
    pub fn new(billboard: super::effects::Billboard) -> Self {
        Self {
            billboard,
            attached: None,
        }
    }

    pub fn attach_shade(&mut self, argb: u32, grid: i32) -> bool {
        attach(&mut self.attached, argb, grid)
    }

    fn update_attached(&mut self, tick: &Tick, ground: Option<&Ground<'_>>) {
        if self.billboard.expired() {
            self.attached = None;

            return;
        }

        if let Some(ground) = ground {
            update_attached_position(
                self.billboard.motion,
                self.attached.as_mut(),
                self.billboard.position,
                ground,
            );
        }

        if let Some(attached) = self.attached.as_mut() {
            attached.update(tick);

            if attached.expired() {
                self.attached = None;
            }
        }
    }
}

impl Effect for BillboardWithShade {
    fn class(&self) -> &'static super::effect_tree::Class {
        self.billboard.class()
    }

    fn update(&mut self, tick: &Tick) {
        Effect::update(&mut self.billboard, tick);

        self.update_attached(tick, None);
    }

    fn needs_ground(&self) -> bool {
        self.attached.is_some() && self.billboard.motion == Motion::Follow
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        Effect::update(&mut self.billboard, tick);

        self.update_attached(tick, Some(ground));
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.billboard.update_live(tick, live);

        self.update_attached(tick, live.ground);
    }

    fn expired(&self) -> bool {
        self.billboard.expired()
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        self.billboard.submit(cx, out);

        if !self.expired()
            && let Some(attached) = &self.attached
        {
            attached.submit(cx, out);
        }
    }

    fn cull(&self, eye: Vec3, field: bool) -> super::effect_tree::Cull {
        self.billboard.cull(eye, field)
    }
}

pub fn update_attached_position(
    motion: Motion,
    attached: Option<&mut Attached>,
    at: Vec3,
    ground: &Ground<'_>,
) -> bool {
    if motion != Motion::Follow {
        return false;
    }

    let Some(attached) = attached else {
        return false;
    };

    attached.follow(at, ground);

    true
}

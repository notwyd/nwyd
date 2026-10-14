use formats::ui::pane::RasterOp;

pub const DEFAULT_DISABLED: u32 = 0xFF80_8080;

pub const DEFAULT_COLOR: u32 = 0xFFFF_FFFF;

pub const DEFAULT_FONT: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum State {
    #[default]
    Normal = 0,
    Pressed = 1,
    MouseOver = 2,
    Disabled = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorSet {
    pub normal: u32,
    pub pressed: u32,
    pub mouseover: u32,
    pub disabled: u32,
    pub live: [f32; 4],
}

impl Default for ColorSet {
    fn default() -> ColorSet {
        ColorSet {
            normal: 0,
            pressed: 0,
            mouseover: 0,
            disabled: 0,
            live: [0.0; 4],
        }
    }
}

impl ColorSet {
    pub fn init(argb: u32, disabled: u32) -> ColorSet {
        ColorSet {
            normal: argb,
            pressed: argb,
            mouseover: argb,
            disabled,
            live: unpack(argb),
        }
    }

    pub fn set_all(&mut self, argb: u32) {
        self.normal = argb;

        self.pressed = argb;

        self.mouseover = argb;

        self.live = unpack(argb);
    }

    pub fn get(&self, state: State) -> u32 {
        match state {
            State::Normal => self.normal,
            State::Pressed => self.pressed,
            State::MouseOver => self.mouseover,
            State::Disabled => self.disabled,
        }
    }

    pub fn set(&mut self, state: State, argb: u32) {
        match state {
            State::Normal => self.normal = argb,
            State::Pressed => self.pressed = argb,
            State::MouseOver => self.mouseover = argb,
            State::Disabled => self.disabled = argb,
        }
    }

    pub fn select(&mut self, state: State) {
        self.live = unpack(self.get(state));
    }

    pub fn set_normal_live(&mut self, argb: u32) {
        self.normal = argb;

        self.live = unpack(argb);
    }

    pub fn live_argb(&self) -> u32 {
        pack(self.live)
    }

    pub fn lerp(&mut self, state: State, factor: f32) {
        if factor == 0.0 {
            self.select(state);

            return;
        }

        let target = unpack(self.get(state));

        for (live, target) in self.live.iter_mut().zip(target) {
            *live += factor * (target - *live);
        }

        self.live[3] = self.live[3].clamp(0.0, 1.0);
    }
}

pub fn pack(live: [f32; 4]) -> u32 {
    fn channel(x: f32) -> u32 {
        if x >= 1.0 {
            255
        } else if x > 0.0 {
            (f64::from(x) * 255.0 + 0.5) as u32
        } else {
            0
        }
    }

    channel(live[3]) << 24 | channel(live[0]) << 16 | channel(live[1]) << 8 | channel(live[2])
}

pub fn unpack(argb: u32) -> [f32; 4] {
    const SCALE: f32 = 0.003_921_569;

    [
        ((argb >> 16) & 0xFF) as f32 * SCALE,
        ((argb >> 8) & 0xFF) as f32 * SCALE,
        (argb & 0xFF) as f32 * SCALE,
        (argb >> 24) as f32 * SCALE,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub texture_index: i32,
    pub uv: [i32; 4],
    pub font_index: i32,
    pub font_align: u32,
    pub font_offset: [i32; 2],
    pub texture_color: ColorSet,
    pub font_color: ColorSet,
    pub raster_op: RasterOp,
}

impl Default for Material {
    fn default() -> Material {
        Material {
            texture_index: 0,
            uv: [0; 4],
            font_index: 0,
            font_align: 0,
            font_offset: [0; 2],
            texture_color: ColorSet::default(),
            font_color: ColorSet::default(),
            raster_op: RasterOp::Wrap,
        }
    }
}

impl Material {
    pub fn seed() -> Material {
        Material {
            texture_index: 0,
            uv: [0; 4],
            font_index: DEFAULT_FONT,
            font_align: 0,
            font_offset: [0; 2],
            texture_color: ColorSet::init(DEFAULT_COLOR, DEFAULT_DISABLED),
            font_color: ColorSet::init(DEFAULT_COLOR, DEFAULT_DISABLED),
            raster_op: RasterOp::Stretch,
        }
    }

    pub fn widget_seed() -> Material {
        Material {
            font_index: 0,
            ..Material::seed()
        }
    }

    pub fn set_texture_index(&mut self, index: i32) {
        self.texture_index = index;

        self.uv = [0; 4];

        self.texture_color = ColorSet::init(DEFAULT_COLOR, DEFAULT_DISABLED);

        self.raster_op = RasterOp::Stretch;
    }

    pub fn set_font_index(&mut self, index: i32) {
        self.font_index = index;

        self.font_align = 0;

        self.font_offset = [0; 2];

        self.font_color = ColorSet::init(DEFAULT_COLOR, DEFAULT_DISABLED);
    }
}

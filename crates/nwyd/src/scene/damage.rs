use super::field::S_MODE;
use super::login_static::Layout;
use super::stext::{self, SceneText};
use crate::frame::{PlannedQuad, TextureKey};
use formats::texture::texture_set_list::{Item, TextureSetList};
use render::preset::RenderMode;
use render::sprite::{Quad, UvRule};
use std::f32::consts::PI;

const FONT_KEY: u32 = 0x0700_0000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontStyle {
    pub colour: u32,
    pub scale: f32,
}

impl Default for FontStyle {
    fn default() -> Self {
        Self {
            colour: 0xffff_ffff,
            scale: 1.0,
        }
    }
}

pub const SETS: [u32; 8] = [0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8D, 0x8F, 0x90];

pub const KIND_GREEN: u8 = 1;

pub const KIND_BLUE: u8 = 2;

pub const KIND_YELLOW: u8 = 3;

pub const KIND_RED: u8 = 4;

pub const KIND_CRIT: u8 = 5;

pub const KIND_CRIT_SMALL: u8 = 6;

pub const KIND_MAGIC: u8 = 7;

pub const KIND_MAGIC_CRIT: u8 = 8;

pub const POP_KINDS: [u8; 3] = [KIND_CRIT, KIND_CRIT_SMALL, KIND_MAGIC_CRIT];

pub fn ctor_scale(kind: u8) -> f32 {
    match kind {
        KIND_GREEN => 0.6,
        KIND_CRIT | KIND_MAGIC_CRIT => 1.1,
        KIND_CRIT_SMALL => 0.8,
        _ => 0.7,
    }
}

pub const LIFE_MS: u32 = 0x5DC;

pub const EXP_LIFE_MS: u32 = 0x4B0;

pub const HOLD_MS: [u32; 5] = [100, 300, 400, 500, 600];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    pub from: f32,
    pub until: f32,
    pub grow: f32,
    pub fade: f32,
}

impl Ramp {
    pub const SHORT: Ramp = Ramp {
        from: 0.3,
        until: 0.9,
        grow: 0.9,
        fade: 0.6,
    };

    pub const LONG: Ramp = Ramp {
        from: 0.35,
        until: 0.9,
        grow: 0.7,
        fade: 0.6,
    };

    pub fn knee(v: f32) -> f32 {
        match v >= KNEE {
            true => (v - KNEE) / (1.0 - KNEE),
            false => 0.0,
        }
    }
}

pub const KNEE: f32 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drift(pub u16);

impl Drift {
    pub const RISE: Drift = Drift(1);

    pub fn up(self) -> bool {
        matches!(self.0, 1 | 5 | 7 | 9)
    }

    pub fn down(self) -> bool {
        matches!(self.0, 2 | 6 | 8)
    }

    pub fn left(self) -> bool {
        matches!(self.0, 3 | 5 | 8)
    }

    pub fn right(self) -> bool {
        matches!(self.0, 4 | 6 | 7)
    }

    pub fn jump_left(self) -> bool {
        self.0 == 9
    }
}

pub const JUMP_LEFT: i32 = 0x32;

pub const RISE: f32 = 100.0;

pub const RISE_CRIT: f32 = 120.0;

pub const RISE_REFERENCE: f32 = 900.0;

pub const POP_FLOOR: f32 = 0.2;

pub const POP_KNEE: f32 = 0.25;

pub fn half_of(count: usize) -> f32 {
    count as f32 * 0.5 - 1.0
}

pub const SPREAD_CRIT: f32 = 1.3;

pub const SPREAD: f32 = 0.55;

pub const SPREAD_DAMP: f32 = 0.3;

pub const ONE_KERN: f32 = 3.0;

pub const GAP_ADVANCE: f32 = 5.0;

pub const GLYPH_SLOTS: usize = 11;

pub const GLYPH_CLAMP: usize = 12;

pub const GLYPH_PLUS: i32 = 10;

pub const GLYPH_WORD: i32 = 11;

pub const GLYPH_MINUS: i32 = 12;

pub const GLYPH_COLON: i32 = 13;

pub const GLYPH_QUERY_GREEN: i32 = 14;

pub const GLYPH_QUERY_RED: i32 = 12;

pub const GLYPH_CRITICAL_WORD: i32 = 15;

pub fn query_glyph(set: u32) -> i32 {
    match set {
        0x89 => GLYPH_QUERY_GREEN,
        0x8C => GLYPH_QUERY_RED,
        _ => GLYPH_WORD,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    pub x: f32,
    pub y: f32,
    pub index: i32,
}

pub fn glyphs(text: &str, set: u32, items: &[Item], scale: f32, sx: f32) -> Vec<Glyph> {
    let bytes = text.as_bytes();

    let count = bytes.len().min(GLYPH_CLAMP);

    let one = |index: i32| {
        vec![Glyph {
            x: 0.0,
            y: 0.0,
            index,
        }]
    };

    match bytes.first() {
        None => Vec::new(),
        Some(b'm') => one(GLYPH_WORD),
        Some(b'?') => one(query_glyph(set)),
        Some(&first @ (b'E' | b'C')) => {
            let (lead, skip) = match first {
                b'E' => (GLYPH_WORD, 2),
                _ => (GLYPH_CRITICAL_WORD, 1),
            };

            let count = count.saturating_sub(skip);

            let mut out = one(lead);

            let mut pen = advance(lead, items, scale, sx);

            for i in 1..count {
                let index = bytes.get(i + skip).map_or(-1, |&b| glyph_of(b));

                out.push(Glyph {
                    x: pen,
                    y: 0.0,
                    index,
                });

                pen += advance(index, items, scale, sx);
            }

            out
        }
        Some(_) => {
            let mut out = Vec::with_capacity(count);

            let mut pen = 0.0;

            for &b in &bytes[..count] {
                let index = glyph_of(b);

                out.push(Glyph {
                    x: pen,
                    y: 0.0,
                    index,
                });

                pen += advance(index, items, scale, sx);
            }

            out
        }
    }
}

pub fn glyph_of(byte: u8) -> i32 {
    match byte {
        b'0'..=b'9' => i32::from(byte - b'0'),
        b'+' => GLYPH_PLUS,
        b'-' => GLYPH_MINUS,
        b':' => GLYPH_COLON,
        _ => -1,
    }
}

pub fn advance(index: i32, items: &[Item], scale: f32, sx: f32) -> f32 {
    let width = usize::try_from(index)
        .ok()
        .and_then(|i| items.get(i))
        .map(|item| item.w);

    match width {
        Some(w) => (w as f32 * sx * scale).trunc(),
        None => (GAP_ADVANCE * sx).trunc(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Number {
    pub kind: u8,
    pub text: String,
    pub font: FontStyle,
    pub glyphs: Vec<Glyph>,
    pub x: f32,
    pub y: f32,
    pub x0: f32,
    pub y0: f32,
    pub start_ms: u32,
    pub life_ms: u32,
    pub drift: Drift,
    pub mode: usize,
    pub scale: f32,
    pub rise: f32,
    pub grow: f32,
    pub fade: f32,
    pub pop: f32,
    pub visible: bool,
}

impl Number {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        text: &str,
        at: (f32, f32),
        kind: u8,
        delay_ms: u32,
        life_ms: u32,
        now_ms: u32,
        items: &[Item],
        layout: &Layout,
    ) -> Number {
        let scale = ctor_scale(kind);

        let set = set_of(kind).unwrap_or(0);

        Number {
            kind,
            text: text.to_string(),
            font: FontStyle::default(),
            glyphs: glyphs(text, set, items, scale, layout.sx),
            x: at.0,
            y: at.1,
            x0: at.0,
            y0: at.1,
            start_ms: now_ms.wrapping_add(delay_ms),
            life_ms,
            drift: Drift::RISE,
            mode: 0,
            scale,
            rise: (life_ms as f32 / RISE_REFERENCE).max(1.0),
            grow: 0.0,
            fade: 0.0,
            pop: 0.0,
            visible: false,
        }
    }

    pub fn tick(&mut self, now_ms: u32, layout: &Layout) -> bool {
        if now_ms.wrapping_sub(self.start_ms) > u32::MAX / 2 {
            self.visible = false;

            return true;
        }

        if self.life_ms == 0 {
            return true;
        }

        let hold = HOLD_MS.get(self.mode).copied().unwrap_or(600);

        let elapsed = now_ms.wrapping_sub(self.start_ms);

        let t = elapsed.saturating_sub(hold) as f32 / (self.life_ms as f32 - hold as f32);

        let ramp = match self.mode {
            0 => Ramp::SHORT,
            _ => Ramp::LONG,
        };

        self.visible = t > ramp.from && t < ramp.until;

        self.grow = Ramp::knee((t - ramp.from) / ramp.grow);

        self.fade = Ramp::knee((t - ramp.from) / ramp.fade);

        match self.kind {
            KIND_CRIT | KIND_MAGIC_CRIT => {
                self.scale = pop_scale(self.grow, 1.0);
            }
            KIND_CRIT_SMALL => {
                self.scale = pop_scale(self.grow, 0.6);
            }
            _ => {}
        }

        if t >= 1.0 {
            return false;
        }

        self.drift_to(t, layout);

        self.pop = match POP_KINDS.contains(&self.kind) {
            true => (t * PI).sin() * self.scale,
            false => (t + POP_FLOOR) * self.scale,
        };

        true
    }

    fn drift_to(&mut self, t: f32, layout: &Layout) {
        let step = |distance: f32, scale: f32| (self.rise * t * distance * scale).trunc();

        if matches!(self.kind, KIND_CRIT | KIND_CRIT_SMALL) {
            self.y = (self.y0 - step(RISE_CRIT, layout.sy)).trunc();

            return;
        }

        let d = step(RISE, layout.sy);

        if self.drift.up() {
            self.y = (self.y0 - d).trunc();
        }

        if self.drift.down() {
            self.y = (self.y0 + d).trunc();
        }

        if self.drift.left() {
            self.x = (self.x0 - d).trunc();
        }

        if self.drift.right() {
            self.x = (self.x0 + d).trunc();
        }

        if self.drift.jump_left() {
            self.x = self.x0 - JUMP_LEFT as f32;
        }
    }

    pub fn quads(&self, items: &[Item], layout: &Layout) -> Vec<PlannedQuad> {
        if !self.visible {
            return Vec::new();
        }

        let half = half_of(self.glyphs.len());

        let alpha = ((1.0 - self.fade) * 255.0).trunc().clamp(0.0, 255.0) as u32;

        let color = alpha << 24 | 0x00FF_FFFF;

        let s = self.pop * layout.sx;

        let mut out = Vec::with_capacity(self.glyphs.len());

        for (i, glyph) in self.glyphs.iter().enumerate() {
            let Ok(index) = usize::try_from(glyph.index) else {
                continue;
            };

            let Some(item) = items.get(index) else {
                continue;
            };

            let Ok(texture) = u32::try_from(item.texture_index) else {
                continue;
            };

            if texture > 511 {
                continue;
            }

            let (w, h) = (item.w as f32, item.h as f32);

            let frac = match half == 0.0 {
                true => 0.0,
                false => (i as f32 - half) / half,
            };

            let spread = match POP_KINDS.contains(&self.kind) {
                true => (SPREAD_CRIT - self.pop) * w * -frac * half,
                false => (SPREAD - self.pop) * w * -frac * half * SPREAD_DAMP,
            };

            let mut cx = self.x + (item.dst_offset[0] as f32 + glyph.x) + spread;

            if self.text.as_bytes().get(i) == Some(&b'1') {
                cx -= layout.sy * ONE_KERN;
            }

            let cy = self.y + item.dst_offset[1] as f32 + glyph.y;

            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::Scene(texture),
                quad: Quad {
                    normalized_corners: None,
                    clip: None,
                    dst_x: cx - w * s * 0.5,
                    dst_y: cy - h * s * 0.5,
                    dst_w: w * s,
                    dst_h: h * s,
                    src_x: item.x as f32,
                    src_y: item.y as f32,
                    src_w: w,
                    src_h: h,
                    color,
                    rotation: 0.0,
                },
                uv: UvRule::TexelCentre,
            });
        }

        out
    }
}

pub fn pop_scale(grow: f32, base: f32) -> f32 {
    match grow >= POP_KNEE {
        true => base,
        false => (POP_KNEE - grow) + (POP_KNEE - grow) + base,
    }
}

pub fn set_of(kind: u8) -> Option<u32> {
    let index = usize::from(kind).checked_sub(1)?;

    SETS.get(index).copied()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Numbers {
    pub live: Vec<Number>,
    pub draw_suppressed: bool,
}

impl Numbers {
    pub fn push(&mut self, number: Number) {
        self.live.push(number);
    }

    pub fn tick(&mut self, now_ms: u32, layout: &Layout) -> usize {
        self.live.retain_mut(|number| number.tick(now_ms, layout));

        self.live.len()
    }

    pub fn visible(&self) -> bool {
        !self.draw_suppressed && self.live.iter().any(|number| number.visible)
    }

    pub fn quads(&self, sets: &TextureSetList, layout: &Layout) -> Vec<PlannedQuad> {
        if self.draw_suppressed {
            return Vec::new();
        }

        self.live
            .iter()
            .flat_map(
                |number| match set_of(number.kind).and_then(|set| set_items(sets, set)) {
                    Some(items) => number.quads(items, layout),
                    None => Vec::new(),
                },
            )
            .collect()
    }

    pub fn frame_draws(
        &self,
        sets: &TextureSetList,
        layout: &Layout,
        text: &mut SceneText,
    ) -> Vec<PlannedQuad> {
        let mut out = Vec::new();

        let mut retained = std::collections::BTreeSet::new();

        if !self.draw_suppressed {
            for (index, number) in self.live.iter().enumerate() {
                if !number.visible {
                    continue;
                }

                if number.kind != 0 {
                    if let Some(items) = set_of(number.kind).and_then(|set| set_items(sets, set)) {
                        out.extend(number.quads(items, layout));
                    }

                    continue;
                }

                let Ok(index) = u32::try_from(index) else {
                    continue;
                };

                if index > 0x00ff_ffff || !number.font.scale.is_finite() || number.font.scale <= 0.0
                {
                    continue;
                }

                let key = FONT_KEY | index;

                let source = number.text.split('\0').next().unwrap_or_default();

                if !text.resolve(key, source, number.font.colour) {
                    continue;
                }

                retained.insert(key);

                let line = text.get(key).expect("resolved floating text");

                let at = (number.x, number.y + 1.0);

                for (mode, mut quad) in [
                    (RenderMode::Ui2dPoint, stext::shadow_quad(line, at)),
                    (S_MODE, stext::quad(line, at)),
                ] {
                    quad.dst_w *= number.font.scale;

                    quad.dst_h *= number.font.scale;

                    out.push(PlannedQuad {
                        mode,
                        texture: TextureKey::SceneText(key),
                        quad,
                        uv: UvRule::TexelCentre,
                    });
                }
            }
        }

        let retired: Vec<_> = text
            .iter()
            .map(|(key, _)| *key)
            .filter(|key| key & 0xff00_0000 == FONT_KEY && !retained.contains(key))
            .collect();

        for key in retired {
            text.forget(key);
        }

        out
    }
}

pub fn set_items(sets: &TextureSetList, set: u32) -> Option<&[Item]> {
    Some(sets.get(usize::try_from(set).ok()?)?.items.as_slice())
}

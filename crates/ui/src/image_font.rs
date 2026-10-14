use crate::resource::Resource;
use formats::{
    text::codepage::{self, Codepage},
    ui::pane::{Property, font_align},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub texture: i32,
    pub uv: [i32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageFont {
    pub font_height: i32,
    pub margin: [i32; 2],
    pub width_scale: f32,
    pub text: String,
    prefix: String,
    glyphs: [Option<Glyph>; 128],
    resolve_copy: bool,
}

impl Default for ImageFont {
    fn default() -> Self {
        Self {
            font_height: 32,
            margin: [0, 0],
            width_scale: 1.0,
            text: String::new(),
            prefix: String::new(),
            glyphs: [None; 128],
            resolve_copy: false,
        }
    }
}

pub struct Layout {
    pub body: [i32; 4],
    pub glyphs: Vec<(Glyph, [i32; 4])>,
}

impl ImageFont {
    pub fn set_prefix(&mut self, prefix: &str, resource: &Resource) {
        self.prefix = prefix.into();

        self.glyphs = std::array::from_fn(|code| {
            resource.alias(&format!("{prefix}{code}")).map(|a| Glyph {
                texture: a.texture_index,
                uv: a.rect,
            })
        });

        self.resolve_copy = false;
    }

    pub fn copy_template_from(&mut self, source: &Self) {
        self.font_height = source.font_height;

        self.margin = source.margin;

        self.width_scale = source.width_scale;

        self.text = source.text.clone();

        self.prefix = source.prefix.clone();

        self.glyphs = [None; 128];

        self.resolve_copy = true;
    }

    pub fn resolve_template(&mut self, resource: &Resource) {
        if self.resolve_copy {
            let prefix = self.prefix.clone();

            self.set_prefix(&prefix, resource);
        }
    }

    pub fn apply(&mut self, p: &Property, resource: &Resource) -> bool {
        match p.base_key() {
            k if k.eq_ignore_ascii_case("FontHeight") => self.font_height = p.int(),
            k if k.eq_ignore_ascii_case("Margin") => {
                self.margin = [0, 0];

                p.scan(&mut self.margin);
            }
            k if k.eq_ignore_ascii_case("WidthScale") => self.width_scale = p.float(),
            k if k.eq_ignore_ascii_case("FontPreAlise") => self.set_prefix(&p.value, resource),
            k if k.eq_ignore_ascii_case("TextValue") => {
                self.text = p.value.split('\0').next().unwrap_or("").into();
            }
            _ => return false,
        }

        true
    }

    pub fn layout(&self, rect: [i32; 4], align: u32) -> Option<Layout> {
        let [l, t, r, b] = rect.map(i64::from);

        let [mx, my] = self.margin.map(i64::from);

        let height = i64::from(self.font_height);

        let budget = r - l - 2 * mx;

        if self.resolve_copy
            || r <= l
            || b <= t
            || height <= 0
            || budget <= 0
            || self.text.is_empty()
            || !self.width_scale.is_finite()
            || self.width_scale < 0.0
        {
            return None;
        }

        let bytes = codepage::encode(&self.text, Codepage::Windows1252);

        let mut glyphs = Vec::new();

        let mut used = 0_i64;

        let mut extent = budget;

        for code in bytes.into_iter().take_while(|c| *c != 0) {
            if !(1..=127).contains(&code) {
                continue;
            }

            let Some(glyph) = self.glyphs[usize::from(code)] else {
                continue;
            };

            let w = i64::from(glyph.uv[2]) - i64::from(glyph.uv[0]);

            let h = i64::from(glyph.uv[3]) - i64::from(glyph.uv[1]);

            if w < 1 || h < 1 {
                continue;
            }

            let advance =
                (w as f64 * height as f64 * f64::from(self.width_scale) / h as f64 + 0.5) as i32;

            let advance = i64::from(advance);

            if used + advance > budget {
                break;
            }

            if advance > 0 {
                glyphs.push((glyph, used, advance));
            }

            used += advance;

            extent = used;
        }

        if extent <= 0 {
            return None;
        }

        let x = if align & font_align::RIGHT != 0 {
            r - mx - extent
        } else if align & font_align::CENTER != 0 {
            l + (r - l - extent) / 2 + mx
        } else {
            l + mx
        };

        let y = if align & font_align::BOTTOM != 0 {
            b - height - my
        } else if align & font_align::VCENTER != 0 {
            t + (b - t - height) / 2 + my
        } else {
            t + my
        };

        let bounded = |v: i64| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;

        Some(Layout {
            body: [x - mx, y - my, x + extent + mx, y + height + my].map(bounded),
            glyphs: glyphs
                .into_iter()
                .map(|(g, offset, w)| (g, [x + offset, y, x + offset + w, y + height].map(bounded)))
                .collect(),
        })
    }
}

pub fn resolve_copies(panel: &mut crate::Panel, resource: &Resource) {
    fn visit(elements: &mut [crate::Element], resource: &Resource) {
        for e in elements {
            if let Some(font) = e.widget.image_font_mut() {
                font.resolve_template(resource);
            }

            if let Some(list) = e.widget.list_mut() {
                for row in &mut list.items {
                    visit(&mut row.children, resource);
                }
            }
        }
    }

    visit(&mut panel.elements, resource);
}

use crate::fonts::Chains;
use crate::scene::login_static::{Layout, Rect};
use render::font::table::pixel_size;
use render::font::{Font, TextBitmap, units};
use render::sprite::Quad;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

pub fn display_text(value: &str) -> String {
    let text = value
        .split('\0')
        .next()
        .unwrap_or_default()
        .replace('_', " ");

    if text == " " { String::new() } else { text }
}

pub const LEFT_INSET: f32 = 8.0;

pub const LINE_BOX: f32 = 16.0;

pub const ALIGN: u32 = render::font::align::NOCLIP;

pub const ALIGN_LEFT: i32 = 0;

pub const SURFACE_MAX_W: i32 = 512;

pub const SURFACE_MAX_H: i32 = SURFACE_MAX_W / 8;

pub const SURFACE_PAD: i32 = 2;

pub const SURFACE_MIN: i32 = 3;

pub fn surface(measured: (i32, i32)) -> Option<(u32, u32)> {
    scaled_surface(measured, 1.0)
}

fn scaled_surface(measured: (i32, i32), scale: f32) -> Option<(u32, u32)> {
    surface_with_width_limit(
        measured,
        scale,
        (SURFACE_MAX_W as f32 * scale).ceil() as u32,
    )
}

fn surface_with_width_limit(
    measured: (i32, i32),
    scale: f32,
    max_width: u32,
) -> Option<(u32, u32)> {
    if measured.0 < 1 || measured.1 < 1 {
        return None;
    }

    let pad = (SURFACE_PAD as f32 * scale).round() as i32;

    let (mut w, mut h) = (measured.0 + pad, measured.1 + pad);

    if w < SURFACE_MIN || h < SURFACE_MIN {
        return None;
    }

    w += w & 1;

    h += h & 1;

    let max_h = (SURFACE_MAX_H as f32 * scale).ceil() as i32;

    Some(((w as u32).min(max_width), h.min(max_h) as u32))
}

pub fn font_height(width: u32) -> i32 {
    match width {
        640 => 10,
        800 => 12,
        1024 => 14,
        1280 => 20,
        _ => (24.0 * Layout::extended_pixel_scale(width)).round() as i32,
    }
}

pub fn origin(layout: &Layout, parent: (f32, f32), control: Rect) -> (f32, f32) {
    (
        parent.0 + control.x + LEFT_INSET * layout.sx,
        parent.1 + control.y + (control.h - LINE_BOX * layout.sy) * 0.5,
    )
}

#[derive(Debug, Clone)]
pub struct Line {
    slashed: bool,
    max_width: u32,
    pub text: String,
    pub colour: u32,
    pub rect: [i32; 4],
    pub bitmap: TextBitmap,
    pub serial: usize,
}

pub struct SceneText {
    identity: u64,
    lifetime: Arc<()>,
    font: Option<Font>,
    pixel_scale: f32,
    lines: BTreeMap<u32, Line>,
    zero_styles: Vec<(u32, u32)>,
    next_zero_style: usize,
    built: usize,
    notes: Vec<String>,
}

impl std::fmt::Debug for SceneText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneText")
            .field("size", &self.size())
            .field("lines", &self.lines.keys().collect::<Vec<_>>())
            .field("built", &self.built)
            .field("notes", &self.notes)
            .finish()
    }
}

impl SceneText {
    pub fn new(chains: &Chains, backbuffer_width: u32) -> SceneText {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        let faces = chains.get(0).faces.clone();

        let mut notes = Vec::new();

        let font = match faces.is_empty() {
            true => {
                notes.push("the S* layer has no face: chain 0 resolved to nothing".to_string());

                None
            }
            false => Some(Font::new(
                pixel_size(font_height(backbuffer_width), 1.0),
                faces,
            )),
        };

        SceneText {
            lifetime: Arc::new(()),
            identity: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            font,
            pixel_scale: Layout::extended_pixel_scale(backbuffer_width),
            lines: BTreeMap::new(),
            zero_styles: Vec::new(),
            next_zero_style: 0,
            built: 0,
            notes,
        }
    }

    pub fn resolve(&mut self, name_id: u32, text: &str, colour: u32) -> bool {
        self.resolve_with_width_limit(
            name_id,
            text,
            colour,
            (SURFACE_MAX_W as f32 * self.pixel_scale).ceil() as u32,
        )
    }

    pub fn resolve_kind(&mut self, id: u32, text: &str, colour: u32, kind: u32) -> bool {
        if kind == 1 {
            let entry = (id, ansi_hash(text));

            if let Some(old) = self.zero_styles.iter_mut().find(|v| v.0 == id) {
                *old = entry;
            } else if self.zero_styles.len() < 256 {
                self.zero_styles.push(entry);
            } else {
                self.zero_styles[self.next_zero_style] = entry;

                self.next_zero_style = (self.next_zero_style + 1) & 255;
            }
        }

        self.resolve(id, text, colour)
    }

    pub fn resolve_with_width_limit(
        &mut self,
        name_id: u32,
        text: &str,
        colour: u32,
        max_width: u32,
    ) -> bool {
        let max_width = max_width.max((SURFACE_MAX_W as f32 * self.pixel_scale).ceil() as u32);

        let slashed = self.zero_styles.contains(&(name_id, ansi_hash(text)));

        let fresh = self.lines.get(&name_id).is_some_and(|line| {
            line.text == text
                && line.colour == colour
                && line.max_width == max_width
                && line.slashed == slashed
        });

        if fresh {
            return true;
        }

        let Some(font) = self.font.as_mut() else {
            return false;
        };

        let glyphs = units(text);

        let Some(surface) =
            surface_with_width_limit(font.measure(&glyphs), self.pixel_scale, max_width)
        else {
            self.lines.remove(&name_id);

            return false;
        };

        let rect = [0, 0, surface.0 as i32, surface.1 as i32];

        let mut bitmap = font.render(&glyphs, Some(rect), ALIGN, colour, surface);

        if slashed {
            if text.contains('/') && text.bytes().all(|b| matches!(b, b'/' | b' ')) {
                bitmap.pixels.fill(0);
            } else {
                slash_zeros(font, &glyphs, colour, &mut bitmap);
            }
        }

        self.built += 1;

        let serial = self.built;

        self.lines.insert(
            name_id,
            Line {
                max_width,
                slashed,
                text: text.to_string(),
                colour,
                rect,
                bitmap,
                serial,
            },
        );

        true
    }

    pub fn get(&self, name_id: u32) -> Option<&Line> {
        self.lines.get(&name_id)
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub(crate) fn lifetime(&self) -> Weak<()> {
        Arc::downgrade(&self.lifetime)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u32, &Line)> {
        self.lines.iter()
    }

    pub fn forget(&mut self, name_id: u32) -> bool {
        self.lines.remove(&name_id).is_some()
    }

    pub fn built(&self) -> usize {
        self.built
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    pub fn size(&self) -> Option<u32> {
        self.font.as_ref().map(|f| f.size())
    }

    pub fn measure(&mut self, text: &str) -> Option<(i32, i32)> {
        self.font.as_mut().map(|font| font.measure(&units(text)))
    }
}

pub fn quad(line: &Line, at: (f32, f32)) -> Quad {
    Quad {
        normalized_corners: None,
        clip: None,
        dst_x: at.0,
        dst_y: at.1,
        dst_w: line.bitmap.width as f32,
        dst_h: line.bitmap.height as f32,
        src_x: 0.0,
        src_y: 0.0,
        src_w: line.bitmap.width as f32,
        src_h: line.bitmap.height as f32,
        color: line.colour | 0x00FF_FFFF,
        rotation: 0.0,
    }
}

pub fn shadow_quad(line: &Line, at: (f32, f32)) -> Quad {
    let mut shadow = quad(line, (at.0 + 1.0, at.1 + 1.0));

    shadow.color = 0xff000000;

    shadow
}

fn ansi_hash(text: &str) -> u32 {
    formats::text::codepage::encode(text, formats::text::Codepage::Windows1252)
        .into_iter()
        .take_while(|&b| b != 0)
        .fold(0x811c9dc5u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x1000193)
        })
}

fn slash_zeros(
    font: &mut Font,
    glyphs: &[u16],
    colour: u32,
    bitmap: &mut render::font::TextBitmap,
) {
    let height = font.size() as i32;

    if height <= 2 {
        return;
    }

    let mut left = 0;

    for &glyph in glyphs {
        let width = font.advance(glyph);

        let right = left + width;

        if glyph == u16::from(b'0') && width > 2 {
            let mut x = left + (width / 5).max(1);

            let mut y = height - (height / 4).max(1) - 1;

            let x2 = right - (width / 5).max(1) - 1;

            let y2 = (height / 4).max(1);

            if x < x2 && y2 < y {
                let dx = x2 - x;

                let dy = y - y2;

                let mut error = dx - dy;

                loop {
                    if x >= 0 && y >= 0 && x < bitmap.width as i32 && y < bitmap.height as i32 {
                        let i = (y as usize * bitmap.width as usize + x as usize) * 4;

                        bitmap.pixels[i..i + 4].copy_from_slice(&[
                            (colour >> 16) as u8,
                            (colour >> 8) as u8,
                            colour as u8,
                            255,
                        ]);
                    }

                    if x == x2 && y == y2 {
                        break;
                    }

                    let twice = 2 * error;

                    if twice > -dy {
                        error -= dy;

                        x += 1;
                    }

                    if twice < dx {
                        error += dx;

                        y -= 1;
                    }
                }
            }
        }

        left = right;
    }
}

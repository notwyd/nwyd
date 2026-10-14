use super::{Face, Font, TextBitmap, texture_size, units};
use crate::sprite::Quad;
use formats::ui::guimat::FontDef;
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::sync::Arc;

pub const CAPTION_ALIGN: u32 = 0x120;

pub const ZOOM_KNEE: f32 = 1.2;

pub const ZOOM_DAMPING: f64 = 0.85;

pub fn zoom_scale(zoom: f32) -> f64 {
    if zoom < ZOOM_KNEE {
        f64::from(zoom)
    } else {
        1.0 + ZOOM_DAMPING * (f64::from(zoom) - 1.0)
    }
}

pub fn pixel_size(height: i32, zoom: f32) -> u32 {
    let scaled = (f64::from(height) * zoom_scale(zoom)) as i32;

    scaled.max(1) as u32
}

#[derive(Default)]
pub struct FontTable {
    fonts: BTreeMap<i32, Font>,
    notes: Vec<String>,
}

impl std::fmt::Debug for FontTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontTable")
            .field("rows", &self.fonts.keys().collect::<Vec<_>>())
            .field("notes", &self.notes)
            .finish()
    }
}

impl FontTable {
    pub fn build(rows: &[FontDef], chains: [&[Arc<Face>]; 2], zoom: f32) -> FontTable {
        let mut table = FontTable::default();

        for row in rows {
            let faces = chains[usize::from(row.face == 1)];

            if faces.is_empty() {
                table.notes.push(format!(
                    "font {}: chain {} has no faces",
                    row.index, row.face
                ));

                continue;
            }

            let size = pixel_size(row.height, zoom);

            table
                .fonts
                .insert(row.index, Font::new(size, faces.to_vec()));
        }

        table
    }

    pub fn get(&mut self, index: i32) -> Option<&mut Font> {
        self.fonts.get_mut(&index)
    }

    pub fn contains(&self, index: i32) -> bool {
        self.fonts.contains_key(&index)
    }

    pub fn indices(&self) -> impl Iterator<Item = i32> + '_ {
        self.fonts.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cached {
    pub text: String,
    pub rect: [i32; 4],
    pub font: i32,
    pub align: u32,
    pub bitmap: TextBitmap,
    pub serial: usize,
}

#[derive(Debug, Clone)]
pub struct TextCache<K> {
    entries: HashMap<K, Cached>,
    built: usize,
}

impl<K> Default for TextCache<K> {
    fn default() -> TextCache<K> {
        TextCache {
            entries: HashMap::new(),
            built: 0,
        }
    }
}

impl<K: Eq + Hash + Clone> TextCache<K> {
    pub fn new() -> TextCache<K> {
        TextCache::default()
    }

    pub fn resolve(
        &mut self,
        key: K,
        text: &str,
        font_index: i32,
        rect: [i32; 4],
        align: u32,
        table: &mut FontTable,
    ) -> Option<&Cached> {
        let fresh = self.entries.get(&key).is_some_and(|c| {
            c.text == text && c.rect == rect && c.font == font_index && c.align == align
        });

        if !fresh {
            let font = table.get(font_index)?;

            let surface = texture_size(rect[2], rect[3]);

            let bitmap = font.render(
                &units(text),
                Some(rect),
                align | CAPTION_ALIGN,
                0xFFFF_FFFF,
                surface,
            );

            self.built += 1;

            self.entries.insert(
                key.clone(),
                Cached {
                    text: text.to_string(),
                    rect,
                    font: font_index,
                    align,
                    bitmap,
                    serial: self.built,
                },
            );
        }

        self.entries.get(&key)
    }

    pub fn get(&self, key: &K) -> Option<&Cached> {
        self.entries.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &Cached)> {
        self.entries.iter()
    }

    pub fn forget(&mut self, key: &K) -> bool {
        self.entries.remove(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn built(&self) -> usize {
        self.built
    }
}

pub fn shadow_offsets(kind: i32) -> Vec<(i32, i32)> {
    super::shadow::offsets(kind)
}

pub fn quads(
    bitmap: &TextBitmap,
    at: (i32, i32),
    colour: u32,
    shadow: i32,
    shadow_colour: u32,
) -> Vec<Quad> {
    let (x, y) = (at.0 as f32, at.1 as f32);

    let (w, h) = (bitmap.width as f32, bitmap.height as f32);

    let whole = |dst_x: f32, dst_y: f32, dst_w: f32, dst_h: f32, color: u32| Quad {
        normalized_corners: None,
        clip: None,
        dst_x,
        dst_y,
        dst_w,
        dst_h,
        src_x: 0.0,
        src_y: 0.0,
        src_w: w,
        src_h: h,
        color,
        rotation: 0.0,
    };

    let mut out = Vec::new();

    for (dx, dy) in shadow_offsets(shadow) {
        let (dx, dy) = (dx as f32, dy as f32);

        out.push(whole(x + dx, y + dy, w - dx, h - dy, shadow_colour));
    }

    out.push(whole(x, y, w, h, colour));

    out
}

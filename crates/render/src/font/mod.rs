pub mod cmap;
pub mod table;

pub use cmap::CharMap;
use fontdue::FontSettings;
use std::collections::HashMap;
use std::sync::Arc;
pub use table::{CAPTION_ALIGN, Cached, FontTable, TextCache};

pub struct Face {
    raster: fontdue::Font,
    cmap: CharMap,
}

impl Face {
    pub fn parse(bytes: &[u8], index: u32) -> Result<Face, String> {
        let settings = FontSettings {
            collection_index: index,
            ..FontSettings::default()
        };

        let raster = fontdue::Font::from_bytes(bytes, settings).map_err(str::to_string)?;

        Ok(Face {
            raster,
            cmap: CharMap::parse(bytes, index),
        })
    }

    pub fn glyph_index(&self, unit: u16) -> u16 {
        self.cmap.glyph(u32::from(unit))
    }

    pub fn charmap(&self) -> &CharMap {
        &self.cmap
    }

    pub fn raster(&self) -> &fontdue::Font {
        &self.raster
    }
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Face")
            .field("glyphs", &self.raster.glyph_count())
            .field("charmap", &self.cmap.selected())
            .field("mapped", &self.cmap.len())
            .finish()
    }
}

pub const PAGE_SIZE: u32 = 1024;

pub const CELL_SLACK: u32 = 8;

pub const SMALL_SIZE: u32 = 13;

pub const CHAIN_DEPTH: usize = 3;

pub const RASTER_DEPTH: usize = 2;

pub mod align {

    pub const CENTER: u32 = 0x1;

    pub const RIGHT: u32 = 0x2;

    pub const VCENTER: u32 = 0x4;

    pub const BOTTOM: u32 = 0x8;

    pub const WORDBREAK: u32 = 0x10;

    pub const NOCLIP: u32 = 0x100;
}

pub mod shadow {

    pub const NONE: i32 = 0;

    pub const DOWN: i32 = 1;

    pub const OUTLINE: i32 = 2;

    pub const UP: i32 = 3;

    pub fn offsets(kind: i32) -> Vec<(i32, i32)> {
        match kind {
            DOWN => vec![(0, 1)],
            UP => vec![(0, -1)],
            OUTLINE => (-1..=1)
                .flat_map(|x| (-1..=1).map(move |y| (x, y)))
                .filter(|o| *o != (0, 0))
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Glyph {
    pub advance: i32,
    pub page: usize,
    pub cell: [i32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Placed {
    pub page: usize,
    pub cell: [i32; 4],
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]

pub struct Page {
    pixels: Vec<u8>,
    cell: u32,
    columns: u32,
    rows: u32,
    capacity: u32,
    used: u32,
    dirty: bool,
}

impl Page {
    pub fn new(size: u32) -> Page {
        let cell = size + CELL_SLACK;

        let columns = PAGE_SIZE / cell;

        let rows = PAGE_SIZE / cell;

        Page {
            pixels: vec![0; (PAGE_SIZE * PAGE_SIZE * 4) as usize],
            cell,
            columns,
            rows,
            capacity: columns * rows,
            used: 0,
            dirty: false,
        }
    }

    pub fn allocate(&mut self) -> Option<u32> {
        if self.used >= self.capacity {
            return None;
        }

        let slot = self.used;

        self.used += 1;

        Some(slot)
    }

    pub fn cell_rect(&self, slot: u32) -> [i32; 4] {
        if slot >= self.capacity {
            return [0; 4];
        }

        let cell = self.cell as i32;

        let x = cell * (slot % self.columns) as i32;

        let y = cell * (slot / self.columns) as i32;

        [x, y, x + cell, y + cell]
    }

    fn blit(&mut self, pen_x: i32, baseline_y: i32, bitmap: &Bitmap) {
        blit(
            &mut self.pixels,
            PAGE_SIZE,
            PAGE_SIZE,
            pen_x,
            baseline_y,
            bitmap,
            WHITE,
        );

        self.dirty = true;
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn cell_size(&self) -> u32 {
        self.cell
    }

    pub fn columns(&self) -> u32 {
        self.columns
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    pub fn is_full(&self) -> bool {
        self.used >= self.capacity
    }
}

struct Bitmap {
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    coverage: Vec<u8>,
}

const WHITE: u32 = 0xFFFF_FFFF;

fn blit(
    dst: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    pen_x: i32,
    baseline_y: i32,
    bitmap: &Bitmap,
    colour: u32,
) {
    let rgb = [(colour >> 16) as u8, (colour >> 8) as u8, colour as u8];

    for row in 0..bitmap.height as i32 {
        for col in 0..bitmap.width as i32 {
            let y = row - bitmap.top + baseline_y;

            let x = bitmap.left + col + pen_x;

            if x < 0 || x >= dst_width as i32 || y < 0 || y >= dst_height as i32 {
                continue;
            }

            let coverage = bitmap.coverage[(row * bitmap.width as i32 + col) as usize];

            let i = ((y * dst_width as i32 + x) * 4) as usize;

            dst[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], coverage]);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct TextBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub measured: i32,
    pub placed: usize,
}

pub fn texture_size(width: i32, height: i32) -> (u32, u32) {
    let round = |v: i32| (v.max(0) as u32).max(64).next_power_of_two();

    (round(width), round(height))
}

pub struct Font {
    size: u32,
    faces: Vec<Arc<Face>>,
    face_sizes: Vec<f32>,
    glyphs: HashMap<u16, Glyph>,
    advances: HashMap<u16, i32>,
    pages: Vec<Page>,
}

impl Font {
    pub fn new(size: u32, faces: Vec<Arc<Face>>) -> Font {
        let faces: Vec<Arc<Face>> = faces.into_iter().take(CHAIN_DEPTH).collect();

        let face_sizes = (0..faces.len())
            .map(|slot| Font::slot_size(size, slot) as f32)
            .collect();

        Font {
            size,
            faces,
            face_sizes,
            glyphs: HashMap::new(),
            advances: HashMap::new(),
            pages: Vec::new(),
        }
    }

    pub fn face(bytes: &[u8], index: u32) -> Result<Arc<Face>, String> {
        Face::parse(bytes, index).map(Arc::new)
    }

    pub fn slot_size(size: u32, slot: usize) -> u32 {
        if slot != 0 && size < SMALL_SIZE {
            size + 1
        } else {
            size
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn pages_mut(&mut self) -> &mut [Page] {
        &mut self.pages
    }

    fn slot_for(&self, unit: u16, depth: usize) -> Option<(usize, u16)> {
        for (slot, face) in self.faces.iter().enumerate().take(depth) {
            let index = face.glyph_index(unit);

            if index != 0 {
                return Some((slot, index));
            }
        }

        None
    }

    pub fn advance(&mut self, unit: u16) -> i32 {
        if let Some(advance) = self.advances.get(&unit) {
            return *advance;
        }

        let (slot, index) = self.slot_for(unit, CHAIN_DEPTH).unwrap_or((0, 0));

        let advance = match self.faces.get(slot) {
            Some(face) => {
                let metrics = face.raster().metrics_indexed(index, self.face_sizes[slot]);

                metrics.advance_width.round() as i32
            }
            None => 0,
        };

        self.advances.insert(unit, advance);

        advance
    }

    pub fn glyph(&mut self, unit: u16) -> Option<Glyph> {
        if let Some(glyph) = self.glyphs.get(&unit) {
            return Some(*glyph);
        }

        if self.faces.is_empty() {
            return None;
        }

        let page = match self.pages.iter().position(|p| !p.is_full()) {
            Some(page) => page,
            None => {
                self.pages.push(Page::new(self.size));

                self.pages.len() - 1
            }
        };

        let slot = self.pages[page].allocate()?;

        let cell = self.pages[page].cell_rect(slot);

        let (face_slot, index) = self.slot_for(unit, RASTER_DEPTH).unwrap_or((0, 0));

        let size = self.face_sizes[face_slot];

        let (metrics, coverage) = self.faces[face_slot]
            .raster()
            .rasterize_indexed(index, size);

        let bitmap = Bitmap {
            left: metrics.xmin,
            top: metrics.ymin + metrics.height as i32,
            width: metrics.width,
            height: metrics.height,
            coverage,
        };

        self.pages[page].blit(cell[0], cell[1] + self.size as i32, &bitmap);

        let glyph = Glyph {
            advance: metrics.advance_width.round() as i32,
            page,
            cell,
        };

        self.glyphs.insert(unit, glyph);

        Some(glyph)
    }

    pub fn measure(&mut self, text: &[u16]) -> (i32, i32) {
        let width = text.iter().map(|unit| self.advance(*unit)).sum();

        (width, self.size as i32)
    }

    pub fn layout(&mut self, text: &[u16], rect: Option<[i32; 4]>, flags: u32) -> Vec<Placed> {
        let total: i32 = text.iter().map(|unit| self.advance(*unit)).sum();

        let size = self.size as i32;

        let (mut pen_x, mut pen_y, min_x, box_w, box_h) = match rect {
            Some([left, top, right, bottom]) => {
                let (min_x, max_x) = (left.min(right), left.max(right));

                (left, top, min_x, max_x - min_x + 1, bottom - top + 1)
            }
            None => (1, 1, 1, 1, size - 1 - 1 + 1),
        };

        let mut clipping = box_w < total;

        let mut visible = if clipping { box_w } else { 0 };

        let mut skipped = 0;

        if flags != 0 {
            if flags & align::CENTER != 0 {
                if total < box_w {
                    pen_x = ((box_w - total) >> 1) + min_x;
                } else if total != box_w {
                    skipped = (total - box_w) >> 1;

                    visible = total - skipped;

                    pen_x = min_x - skipped;
                }
            } else if flags & align::RIGHT != 0 {
                if total < box_w {
                    pen_x = (box_w - total) - 1 + min_x;
                } else if total != box_w {
                    skipped = total - box_w;

                    visible = total;

                    pen_x = (box_w - total) + min_x;
                }
            }

            if flags & align::VCENTER != 0 {
                if size < box_h {
                    pen_y += (box_h - size) / 2;
                } else if size != box_h {
                    pen_y -= (size - box_h) / 2;
                }
            } else if flags & align::BOTTOM != 0 && size != box_h {
                pen_y += box_h - size;
            }
        }

        if flags & align::NOCLIP != 0 {
            clipping = false;
        }

        pen_y -= (size / 6 - 1).max(0);

        let clip_start = skipped + pen_x;

        let clip_end = visible + pen_x;

        let mut placed = Vec::with_capacity(text.len());

        let mut end = pen_x;

        for unit in text {
            let Some(glyph) = self.glyph(*unit) else {
                continue;
            };

            if clipping {
                end += glyph.advance;

                if pen_x >= clip_start && end <= clip_end {
                    placed.push(Placed {
                        page: glyph.page,
                        cell: glyph.cell,
                        x: pen_x,
                        y: pen_y,
                    });
                }
            } else {
                placed.push(Placed {
                    page: glyph.page,
                    cell: glyph.cell,
                    x: pen_x,
                    y: pen_y,
                });
            }

            pen_x += glyph.advance;
        }

        placed
    }

    pub fn render(
        &mut self,
        text: &[u16],
        rect: Option<[i32; 4]>,
        flags: u32,
        colour: u32,
        surface: (u32, u32),
    ) -> TextBitmap {
        let (tex_w, tex_h) = surface;

        let size = self.size as i32;

        let mut measured = 0;

        let mut run: Vec<u16> = Vec::with_capacity(text.len());

        for unit in text {
            measured += self.advance(*unit);

            if (tex_w as i32) < measured {
                break;
            }

            run.push(*unit);
        }

        let (mut pen_x, mut pen_y, min_x, box_h) = match rect {
            Some([left, top, right, bottom]) => (left, top, left.min(right), bottom - top - 2),
            None => (1, 1, 1, size - 1 - 1 - 2),
        };

        let box_w = match rect {
            Some([left, _, right, _]) => ((right - left).abs() - 2).min(tex_w as i32),
            None => (tex_w as i32).min(-2),
        };

        if flags != 0 {
            if flags & align::CENTER != 0 {
                if measured < box_w {
                    pen_x = ((box_w - measured) >> 1) + min_x;
                } else if measured != box_w {
                    pen_x = min_x - ((measured - box_w) >> 1);
                }
            } else if flags & align::RIGHT != 0 {
                if measured < box_w {
                    pen_x = (box_w - measured) - 1 + min_x;
                } else if measured != box_w {
                    pen_x = (box_w - measured) + min_x;
                }
            }

            if flags & align::VCENTER != 0 {
                if size < box_h {
                    pen_y += (box_h - size) / 2;
                } else if size != box_h {
                    pen_y -= (size - box_h) / 2;
                }
            } else if flags & align::BOTTOM != 0 && size != box_h {
                pen_y += box_h - size;
            }
        }

        pen_y -= (size / 6 - 1).max(0);

        let baseline = if flags != 0 { size + pen_y } else { pen_y };

        let mut pixels = vec![0u8; (tex_w * tex_h * 4) as usize];

        let mut pen = (pen_x << 6).max(0);

        let placed = run.len();

        for unit in &run {
            let Some((slot, index)) = self
                .slot_for(*unit, CHAIN_DEPTH)
                .or(Some((0, 0)))
                .filter(|(slot, _)| *slot < self.faces.len())
            else {
                continue;
            };

            let face_size = self.face_sizes[slot];

            let (metrics, coverage) = self.faces[slot]
                .raster()
                .rasterize_indexed(index, face_size);

            let bitmap = Bitmap {
                left: metrics.xmin,
                top: metrics.ymin + metrics.height as i32,
                width: metrics.width,
                height: metrics.height,
                coverage,
            };

            blit(
                &mut pixels,
                tex_w,
                tex_h,
                pen >> 6,
                baseline,
                &bitmap,
                colour,
            );

            pen += advance_26_6(metrics.advance_width);
        }

        TextBitmap {
            width: tex_w,
            height: tex_h,
            pixels,
            measured,
            placed,
        }
    }

    pub fn fit(&mut self, text: &[u16], width: i32) -> (usize, bool) {
        let mut pen = 0;

        for (i, unit) in text.iter().enumerate() {
            let advance = self.glyph(*unit).map(|g| g.advance).unwrap_or(0);

            pen += advance;

            let half = round_half_up(advance as f32 / 2.0);

            if pen - half >= width {
                return (i, false);
            }
        }

        (text.len(), true)
    }
}

fn advance_26_6(advance_width: f32) -> i32 {
    (advance_width.round() as i32) << 6
}

fn round_half_up(x: f32) -> i32 {
    let floor = x.floor();

    if x - floor >= 0.5 {
        floor as i32 + 1
    } else {
        floor as i32
    }
}

pub fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

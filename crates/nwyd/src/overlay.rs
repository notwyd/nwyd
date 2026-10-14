use formats::texture::{MipLevel, PixelFormat, Texture};
use render::font::Face;
use render::sprite::Quad;
use std::collections::VecDeque;

pub const FIRST_CHAR: u8 = 0x20;

pub const LAST_CHAR: u8 = 0x7E;

pub const GLYPHS: usize = (LAST_CHAR - FIRST_CHAR) as usize + 1;

const COLUMNS: u32 = 16;

pub const SIZE_PX: f32 = 13.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]

pub struct Cell {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
    pub advance: f32,
}

#[derive(Debug, Clone)]

pub struct Atlas {
    pub texture: Texture,
    pub cells: Vec<Cell>,
    pub ascent: f32,
    pub line_height: f32,
}

impl Atlas {
    pub fn build(face: &Face, px: f32) -> Atlas {
        let rasterised: Vec<_> = (FIRST_CHAR..=LAST_CHAR)
            .map(|c| {
                let index = face.glyph_index(u16::from(c));

                face.raster().rasterize_indexed(index, px)
            })
            .collect();

        let cell_w = rasterised
            .iter()
            .map(|(m, _)| m.width)
            .max()
            .unwrap_or(1)
            .max(1) as u32
            + 1;

        let cell_h = rasterised
            .iter()
            .map(|(m, _)| m.height)
            .max()
            .unwrap_or(1)
            .max(1) as u32
            + 1;

        let rows = (GLYPHS as u32).div_ceil(COLUMNS);

        let (width, height) = (COLUMNS * cell_w, rows * cell_h);

        let mut data = vec![0u8; (width * height * 4) as usize];

        let mut cells = Vec::with_capacity(GLYPHS);

        for (i, (metrics, coverage)) in rasterised.iter().enumerate() {
            let cx = (i as u32 % COLUMNS) * cell_w;

            let cy = (i as u32 / COLUMNS) * cell_h;

            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let a = coverage[row * metrics.width + col];

                    let o = (((cy + row as u32) * width + cx + col as u32) * 4) as usize;

                    data[o..o + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, a]);
                }
            }

            cells.push(Cell {
                x: cx,
                y: cy,
                w: metrics.width as u32,
                h: metrics.height as u32,
                left: metrics.xmin,
                top: metrics.ymin + metrics.height as i32,
                advance: metrics.advance_width,
            });
        }

        let len = data.len();

        Atlas {
            texture: Texture {
                width,
                height,
                format: PixelFormat::Rgba8Unorm,
                mips: vec![MipLevel {
                    width,
                    height,
                    offset: 0,
                    len,
                }],
                data,
                source_len: len,
            },
            cells,
            ascent: px,
            line_height: px + 4.0,
        }
    }

    fn cell(&self, c: char) -> Option<&Cell> {
        let byte = u8::try_from(u32::from(c)).ok()?;

        (FIRST_CHAR..=LAST_CHAR)
            .contains(&byte)
            .then(|| &self.cells[(byte - FIRST_CHAR) as usize])
    }

    pub fn measure(&self, text: &str) -> f32 {
        text.chars()
            .filter_map(|c| self.cell(c))
            .map(|cell| cell.advance)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Line {
    pub text: String,
    pub color: u32,
}

impl Line {
    pub fn new(text: impl Into<String>, color: u32) -> Line {
        Line {
            text: text.into(),
            color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Draw {
    pub quad: Quad,
    pub glyph: bool,
}

pub const TITLE_COLOR: u32 = 0xFFFF_D060;

pub const BODY_COLOR: u32 = 0xFFE8_E8E8;

pub const PANEL_COLOR: u32 = 0xCC00_0000;

const PADDING: f32 = 5.0;

#[derive(Debug)]

pub struct Overlay {
    atlas: Atlas,
    samples: VecDeque<f32>,
}

impl Overlay {
    pub const SAMPLES: usize = 60;

    pub fn new(atlas: Atlas) -> Overlay {
        Overlay {
            atlas,
            samples: VecDeque::with_capacity(Self::SAMPLES + 1),
        }
    }

    pub fn bundled() -> Result<Overlay, String> {
        let face = Face::parse(crate::fonts::FALLBACK, 0)?;

        Ok(Overlay::new(Atlas::build(&face, SIZE_PX)))
    }

    pub fn atlas(&self) -> &Atlas {
        &self.atlas
    }

    pub fn record(&mut self, frame_ms: f32) {
        self.samples.push_back(frame_ms);

        if self.samples.len() > Self::SAMPLES {
            self.samples.pop_front();
        }
    }

    pub fn mean_frame_ms(&self) -> Option<f32> {
        (!self.samples.is_empty())
            .then(|| self.samples.iter().sum::<f32>() / self.samples.len() as f32)
    }

    pub fn fps(&self) -> Option<f32> {
        self.mean_frame_ms()
            .filter(|ms| *ms > 0.0)
            .map(|ms| 1_000.0 / ms)
    }

    pub fn worst_frame_ms(&self) -> Option<f32> {
        self.samples.iter().copied().reduce(f32::max)
    }

    pub fn draws(&self, lines: &[Line], origin: (f32, f32)) -> Vec<Draw> {
        if lines.is_empty() {
            return Vec::new();
        }

        let widest = lines
            .iter()
            .map(|l| self.atlas.measure(&l.text))
            .fold(0.0, f32::max);

        let mut draws = vec![Draw {
            quad: Quad {
                normalized_corners: None,
                clip: None,
                dst_x: origin.0,
                dst_y: origin.1,
                dst_w: widest + PADDING * 2.0,
                dst_h: self.atlas.line_height * lines.len() as f32 + PADDING * 2.0,
                src_x: 0.0,
                src_y: 0.0,
                src_w: 1.0,
                src_h: 1.0,
                color: PANEL_COLOR,
                rotation: 0.0,
            },
            glyph: false,
        }];

        for (row, line) in lines.iter().enumerate() {
            let baseline =
                origin.1 + PADDING + self.atlas.line_height * row as f32 + self.atlas.ascent;

            let mut pen = origin.0 + PADDING;

            for c in line.text.chars() {
                let Some(cell) = self.atlas.cell(c) else {
                    continue;
                };

                if cell.w > 0 && cell.h > 0 {
                    draws.push(Draw {
                        quad: Quad {
                            normalized_corners: None,
                            clip: None,
                            dst_x: (pen + cell.left as f32).round(),
                            dst_y: (baseline - cell.top as f32).round(),
                            dst_w: cell.w as f32,
                            dst_h: cell.h as f32,
                            src_x: cell.x as f32,
                            src_y: cell.y as f32,
                            src_w: cell.w as f32,
                            src_h: cell.h as f32,
                            color: line.color,
                            rotation: 0.0,
                        },
                        glyph: true,
                    });
                }

                pen += cell.advance;
            }
        }

        draws
    }
}

impl Overlay {
    pub fn lines(&self) -> Vec<Line> {
        let rate = match (self.fps(), self.mean_frame_ms(), self.worst_frame_ms()) {
            (Some(fps), Some(ms), Some(worst)) => {
                format!("{fps:.1} fps   {ms:.2} ms   peak {worst:.1} ms")
            }
            _ => "-- fps   measuring".to_string(),
        };

        vec![Line::new(
            format!("{rate}   version: {}", env!("NWYD_BUILD_COMMIT")),
            TITLE_COLOR,
        )]
    }
}

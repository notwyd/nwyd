use crate::draw::{Command, DrawList, LayerDraw, Owner, TextDraw};
use crate::layout::{Measure, line_offset};
use crate::material::Material;
use crate::resource::Resource;
use crate::rich::{Layout, wrap};
use formats::ui::pane::{ElementType, RasterOp, ShadowType};

pub const MEASURE_WIDTH: i32 = 500;

pub const PADDING: [i32; 4] = [7, 4, 7, 7];

pub const WRAP_WIDTH: i32 = MEASURE_WIDTH - PADDING[0] - PADDING[2];

pub const LINE_GAP: i32 = 3;

pub const MIN_WIDTH: i32 = 20;

pub const SOURCE_GAP: i32 = 2;

pub const RIGHT_MARGIN: i32 = 5;

pub const AUTO_OFFSET: u32 = 0x7FFF_7FFF;

pub const BODY_ALPHA: f32 = 0.7;

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetState {
    pub flat: bool,
    pub anchor: [f32; 2],
    pub font_color: u32,
}

impl Default for WidgetState {
    fn default() -> Self {
        Self {
            flat: false,
            anchor: [0.0; 2],
            font_color: 0xFFFFFFFF,
        }
    }
}

impl WidgetState {
    pub fn translation(&self, width: i32, height: i32) -> [i32; 2] {
        if self.flat {
            return [0, 0];
        }

        [
            (self.anchor[0] - width as f32 * 0.5) as i32,
            (self.anchor[1] - height as f32 - 60.0) as i32,
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Balloon {
    visible: bool,
    text: String,
    rect: [i32; 4],
    layout: Layout,
    material: Option<Material>,
    font_color: u32,
}

impl Balloon {
    pub fn new(resource: &Resource) -> Balloon {
        let material = resource
            .layers(ElementType::BalloonBox)
            .find(|layer| layer.layer == 0)
            .map(|layer| layer.material);

        Balloon {
            visible: false,
            text: String::new(),
            rect: [0; 4],
            layout: Layout::default(),
            material,
            font_color: 0xFFFFFFFF,
        }
    }

    pub fn set_font_color(&mut self, argb: u32) {
        self.font_color = argb;

        if let Some(body) = &mut self.material {
            body.font_color.set_normal_live(argb);
        }
    }

    pub fn font_color(&self) -> u32 {
        self.font_color
    }

    pub fn show(
        &mut self,
        text: &str,
        source: [i32; 4],
        screen: [i32; 4],
        offset: Option<(i16, i16)>,
        measure: &mut dyn Measure,
    ) -> bool {
        if text.is_empty() {
            self.hide();

            return false;
        }

        let Some(material) = self.material else {
            self.hide();

            return false;
        };

        let Some(layout) = wrap(text, material.font_index, WRAP_WIDTH, LINE_GAP, measure) else {
            self.hide();

            return false;
        };

        let max_width = layout
            .lines
            .iter()
            .map(|line| line.width)
            .max()
            .unwrap_or(0);

        let width = (PADDING[0] + PADDING[2] + 1 + max_width).max(MIN_WIDTH);

        let height = PADDING[1] + layout.total_height() + PADDING[3];

        self.rect = place((width, height), source, screen, offset);

        self.text.clear();

        self.text.push_str(text);

        self.layout = layout;

        self.visible = true;

        true
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn rect(&self) -> [i32; 4] {
        self.rect
    }

    pub fn position(&self) -> (i32, i32) {
        (self.rect[0], self.rect[1])
    }

    pub fn size(&self) -> (i32, i32) {
        (self.rect[2] - self.rect[0], self.rect[3] - self.rect[1])
    }

    pub fn draw_list(&self, owner: Owner) -> DrawList {
        let mut list = DrawList::default();

        if !self.visible {
            return list;
        }

        let (width, height) = self.size();

        let Some(mut body) = self.material else {
            return list;
        };

        body.texture_color.live[3] = BODY_ALPHA;

        list.commands.push(Command::Layer(LayerDraw {
            owner,
            texture_index: body.texture_index,
            uv: body.uv,
            rect: [0, 0, width, height],
            color: body.texture_color.live_argb(),
            raster_op: RasterOp::Split,
            additive: false,
        }));

        let align = body.font_align;

        let mut y = PADDING[1];

        for line in &self.layout.lines {
            let mut dx = line_offset(align, WRAP_WIDTH, line.width);

            for run in &line.runs {
                let base_color = body.font_color.live_argb();

                list.commands.push(Command::Text(TextDraw {
                    owner,
                    text: run.text.clone(),
                    rect: [
                        PADDING[0] + dx,
                        y,
                        PADDING[0] + dx + run.width,
                        y + line.height,
                    ],
                    indent: dx,
                    font: run.style.font.unwrap_or(body.font_index),
                    align,
                    color: (base_color & 0xFF00_0000)
                        | run.style.color.unwrap_or(base_color) & 0x00FF_FFFF,
                    shadow: ShadowType::None,
                    shadow_color: run
                        .style
                        .shadow
                        .map_or(0xFF00_0000, |rgb| 0xFF00_0000 | rgb),
                    bold: false,
                }));

                dx += run.width;
            }

            y += line.height;
        }

        list
    }
}

fn place(
    size: (i32, i32),
    source: [i32; 4],
    screen: [i32; 4],
    offset: Option<(i16, i16)>,
) -> [i32; 4] {
    let (mut left, top) = match offset {
        Some((x, y)) => (source[0] + i32::from(x), source[1] + i32::from(y)),
        None => {
            let mut x = source[2] + SOURCE_GAP;

            if screen[2] < x {
                x = source[2] - size.0 - SOURCE_GAP;
            }

            let mut y = source[1] - size.1 - SOURCE_GAP;

            if y < 0 {
                y = source[3] + SOURCE_GAP;
            }

            (x, y)
        }
    };

    let mut right = left + size.0;

    if screen[2] <= right {
        let correction = right - screen[2] + RIGHT_MARGIN;

        left -= correction;

        right -= correction;
    }

    [left, top, right, top + size.1]
}

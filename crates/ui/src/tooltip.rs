use crate::element::Element;
use crate::panel::Panel;
use crate::resource::Resource;
use crate::text::Strings;
use formats::ui::pane::{ElementType, Pane, ShadowType, font_align};

pub const CLIENT_INSET: i32 = 15;

pub const ROW_STEP: i32 = 16;

pub const ROW_EXTRA: i32 = 2;

pub const ROW_FONT: i32 = 2;

pub const ROW_UNSTEPPED_FONT: i32 = 1;

pub const ROW_SEED_COLOR: u32 = 0xFFCC_CCCC;

pub const ROW_SHADOW_COLOR: u32 = 0xFF00_0000;

pub const ROW_NAME: &str = "TextParam";

pub const COMMENT_NAME: &str = "Comment";

pub const COMMENT_STEP: i32 = 0x12;

pub const COMMENT_PAD: i32 = 9;

pub const TITLE_NAME: &str = "Name";

pub const ANCHOR_NAME: &str = "Align";

pub const HEIGHT_TRIM: i32 = 9;

pub const PARK_MARGIN: i32 = 10;

pub const PARK_TOP: i32 = 50;

pub const MAX_Z_ORDER: i32 = 399;

pub const PANEL_COLOR: u32 = 0xA0FF_FFFF;

pub fn park(size: (i32, i32), screen: (i32, i32)) -> (i32, i32) {
    (screen.0 - size.0 - PARK_MARGIN, PARK_TOP)
}

pub fn anchor(
    size: (i32, i32),
    cell: [i32; 4],
    owner: (i32, i32),
    screen: (i32, i32),
) -> (i32, i32) {
    let y = if owner.1 + (cell[1] + cell[3]) / 2 < screen.1 / 2 {
        owner.1 + cell[3]
    } else {
        owner.1 + cell[1] - size.1
    };

    let mut x = owner.0 + cell[0] - size.0 / 2;

    if x < 1 {
        x = 0;
    }

    x = x.min(screen.0 - size.0);

    (x, y)
}

pub fn z_order(owner: i32) -> i32 {
    (owner - 1).min(MAX_Z_ORDER)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub color: u32,
    pub comment: bool,
}

impl Row {
    pub fn new(text: impl Into<String>, color: u32) -> Row {
        Row {
            text: text.into(),
            color,
            comment: false,
        }
    }

    pub fn comment(text: impl Into<String>) -> Row {
        Row {
            text: text.into(),
            color: ROW_SEED_COLOR,
            comment: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tooltip {
    panel: Panel,
    client: [i32; 4],
    y: i32,
    rows: usize,
}

impl Tooltip {
    pub fn new(window: &Pane, template: &Pane, resource: &Resource, strings: &Strings) -> Tooltip {
        let mut panel = Panel::instantiate(window, resource, strings, false);

        let template = Panel::instantiate(template, resource, strings, false);

        let client = [
            panel.left,
            panel.top + CLIENT_INSET,
            panel.right,
            panel.bottom - CLIENT_INSET,
        ];

        for src in &template.elements {
            let dst = panel.element_or_create(&src.name, src.element_type, resource);

            let id = dst.id;

            let image = if let (crate::Widget::Image(state), crate::Widget::Image(source)) =
                (&dst.widget, &src.widget)
            {
                let mut state = state.clone();

                state.copy_template_from(source);

                Some(state)
            } else {
                None
            };

            *dst = src.clone();

            if let Some(state) = image {
                dst.widget = crate::Widget::Image(state);
            }

            dst.id = id;
        }

        panel.right = template.right + panel.left - template.left;

        panel.bottom = panel.top + (template.bottom - template.top);

        let anchor = panel
            .element(ANCHOR_NAME, ElementType::Static)
            .map_or(0, |e| e.y);

        let y = client[1] + anchor - CLIENT_INSET;

        Tooltip {
            panel,
            client,
            y,
            rows: 0,
        }
    }

    pub fn title(&mut self, row: &Row) {
        let Some(element) = self.panel.element_mut(TITLE_NAME, ElementType::Static) else {
            return;
        };

        element.set_display(&row.text);

        set_row_color(element, row.color);
    }

    pub fn push(&mut self, row: &Row, resource: &Resource) {
        if row.comment {
            return self.push_comment(row, resource);
        }

        let (x, width, height) = (
            self.client[0],
            self.client[2] - self.client[0],
            self.client[3],
        );

        let y = self.y;

        let element = self
            .panel
            .create_element(ROW_NAME, ElementType::Static, resource);

        element.x = x;

        element.y = y;

        element.width = width;

        element.height = height;

        element.set_display(&row.text);

        element.set_rich_text(true);

        element.shadow_type = ShadowType::BottomRight;

        element.shadow_color = ROW_SHADOW_COLOR;

        for layer in &mut element.layers {
            layer.font_index = ROW_FONT;

            layer.font_align = layer.font_align & !0xF | font_align::CENTER;

            layer.font_color.set_all(ROW_SEED_COLOR);
        }

        set_row_color(element, row.color);

        let font = element.layers.first().map_or(0, |l| l.font_index);

        self.y += ROW_STEP
            + if font != ROW_UNSTEPPED_FONT {
                ROW_EXTRA
            } else {
                0
            };

        self.rows += 1;
    }

    pub fn push_comment(&mut self, row: &Row, resource: &Resource) {
        let (x, width) = (self.client[0], self.client[2] - self.client[0]);

        let y = self.y;

        let element = self
            .panel
            .create_element(COMMENT_NAME, ElementType::Static, resource);

        element.x = x;

        element.y = y;

        element.width = width;

        element.set_display(&row.text);

        element.set_rich_text(true);

        element.shadow_type = ShadowType::BottomRight;

        element.shadow_color = ROW_SHADOW_COLOR;

        for layer in &mut element.layers {
            layer.font_index = ROW_FONT;

            layer.font_align = layer.font_align & !0xF | font_align::CENTER;

            layer.font_color.set_all(ROW_SEED_COLOR);
        }

        let measured = 0;

        element.height = measured + COMMENT_PAD;

        self.y += COMMENT_STEP + measured;

        self.rows += 1;
    }

    pub fn gap(&mut self, pixels: i32) {
        self.y += pixels;
    }

    pub fn cursor(&self) -> i32 {
        self.y
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn height(&self) -> i32 {
        CLIENT_INSET - HEIGHT_TRIM + self.y
    }

    pub fn finish(mut self) -> Panel {
        self.panel.bottom = self.panel.top + self.height();

        self.panel
    }

    pub fn panel(&self) -> &Panel {
        &self.panel
    }
}

fn set_row_color(element: &mut Element, argb: u32) {
    if let Some(layer) = element.layers.first_mut() {
        layer.font_color.set_normal_live(argb);
    }
}

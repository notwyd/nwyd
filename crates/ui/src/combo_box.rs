use crate::{
    material::Material,
    widget::{ButtonState, ScrollBar, ScrollBarRects},
};
use formats::ui::pane::{ElementType, Property};

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub text: String,
    pub payload: i32,
    pub color: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComboBox {
    pub button: ButtonState,
    pub items: Vec<Row>,
    pub selected: i32,
    pub hovered: i32,
    pub open: bool,
    pub dropbox_length: i32,
    pub scroll: i32,
    pub scrollbar: ScrollBar,
    pub wheel_locked: bool,
    pub screen: Option<[i32; 2]>,
    pub font_heights: Vec<(i32, i32)>,
    pub drag_offset: Option<i32>,
}

impl Default for ComboBox {
    fn default() -> Self {
        Self {
            button: ButtonState::default(),
            items: Vec::new(),
            selected: -1,
            hovered: -1,
            open: false,
            dropbox_length: 0,
            scroll: 0,
            scrollbar: ScrollBar::default(),
            wheel_locked: false,
            screen: None,
            font_heights: Vec::new(),
            drag_offset: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub face: [i32; 4],
    pub arrow: [i32; 4],
    pub popup: [i32; 4],
    pub content: [i32; 4],
    pub bar: ScrollBarRects,
    pub first: usize,
    pub page: usize,
    pub pitch: i32,
    pub rows: Vec<(usize, [i32; 4])>,
}

impl ComboBox {
    pub fn key(&mut self, vk: u32, repeat: bool) -> Option<bool> {
        match vk {
            0x0d if self.open => {
                let changed = self.hovered != self.selected
                    && self.hovered >= 0
                    && self.select(self.hovered as usize);

                self.open = false;

                Some(changed)
            }
            0x25 | 0x26 => {
                if self.hovered > 0 {
                    self.hovered -= 1;
                }

                Some(false)
            }
            0x27 | 0x28 => {
                if self.hovered.saturating_add(1) < self.items.len() as i32 {
                    self.hovered += 1;
                }

                Some(false)
            }
            0x73 => {
                if repeat {
                    return Some(false);
                }

                self.open = !self.open;

                Some(!self.open)
            }
            _ => None,
        }
    }

    pub fn wheel(&mut self, notches: i32, lines: u32, layout: &Layout) -> Option<bool> {
        if self.open {
            let next = layout.first as i64 - i64::from(notches) * i64::from(lines);

            self.scroll = next.clamp(0, self.items.len().saturating_sub(layout.page) as i64) as i32;

            Some(false)
        } else if self.wheel_locked {
            None
        } else if notches == 0 {
            Some(false)
        } else {
            let next = self
                .hovered
                .saturating_add(if notches > 0 { -1 } else { 1 });

            Some(next >= 0 && self.select(next as usize))
        }
    }

    pub fn apply(&mut self, p: &Property) -> bool {
        if p.base_key().eq_ignore_ascii_case("dropbox_length") {
            self.dropbox_length = p.int();

            true
        } else {
            self.button.apply(p, ElementType::ComboBox)
        }
    }

    pub fn add_item(
        &mut self,
        text: &str,
        payload: i32,
        color: u32,
        material: &Material,
    ) -> Option<usize> {
        let text = text.split('\0').next().unwrap_or("");

        if text.encode_utf16().count() > 255 {
            return None;
        }

        let index = self.items.len();

        self.items.push(Row {
            text: text.into(),
            payload,
            color: if color == 0 {
                material.font_color.get(crate::material::State::Normal)
            } else {
                color
            },
        });

        if index == 0 {
            self.selected = 0;

            self.hovered = 0;
        }

        Some(index)
    }

    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.items.len() || index > i32::MAX as usize {
            return false;
        }

        self.selected = index as i32;

        self.hovered = index as i32;

        true
    }

    pub fn clear(&mut self) {
        self.items.clear();

        self.selected = -1;

        self.hovered = -1;

        self.scroll = 0;

        self.drag_offset = None;
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }

        self.items.remove(index);

        if self.selected >= self.items.len() as i32 {
            self.selected = self.items.len() as i32 - 1;
        }

        true
    }

    pub fn font_height(&self, font: i32) -> Option<i32> {
        self.font_heights
            .iter()
            .find(|(index, _)| *index == font)
            .and_then(|(_, h)| h.checked_abs())
    }

    pub fn layout(&self, rect: [i32; 4], face_height: i32, popup_height: i32) -> Option<Layout> {
        let [l, t, r, b] = rect;

        if r <= l || b <= t || face_height <= 0 || popup_height <= 0 {
            return None;
        }

        let pitch = face_height.checked_add(4)?;

        let page_pitch = popup_height.checked_add(4)?;

        let side = ((i64::from(b) - i64::from(t)) as f64 * 0.8999999761581421) as i32;

        let face = [l, t, r.saturating_sub(side), b];

        let arrow = [face[2], t, r, b];

        let length = if self.dropbox_length == 0 {
            i32::try_from(self.items.len())
                .ok()?
                .checked_mul(pitch)?
                .checked_add(8)?
        } else {
            self.dropbox_length
        };

        let mut popup = [l, b, r, b.checked_add(length)?];

        if let Some([top, bottom]) = self.screen {
            if popup[3] > bottom {
                popup[1] = t.checked_sub(length)?;

                popup[3] = t;
            }

            popup[1] = popup[1].max(top);
        }

        let content = [
            l.saturating_add(4),
            popup[1].saturating_add(4),
            r.saturating_sub(4),
            popup[3].saturating_sub(4),
        ];

        let page = (content[3].saturating_sub(content[1]).max(0) / page_pitch) as usize;

        let first = (self.scroll.max(0) as usize).min(self.items.len().saturating_sub(page));

        let end = first.saturating_add(page).min(self.items.len());

        let rows = (first..end)
            .filter_map(|i| {
                let y =
                    content[1].checked_add(i32::try_from(i - first).ok()?.checked_mul(pitch)?)?;

                let bottom = y.checked_add(pitch)?;

                (content[2] > content[0] && bottom <= content[3])
                    .then_some((i, [content[0], y, content[2], bottom]))
            })
            .collect();

        let bar = [r, popup[1], r.saturating_add(side), popup[3]];

        let bounded = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);

        let bar = self.scrollbar.rects(
            bar,
            [0, bounded(self.items.len())],
            bounded(page),
            bounded(first),
        );

        Some(Layout {
            face,
            arrow,
            popup,
            content,
            bar,
            first,
            page,
            pitch,
            rows,
        })
    }

    pub fn stored_layout(&self, rect: [i32; 4], layers: &[Material]) -> Option<Layout> {
        self.layout(
            rect,
            self.font_height(layers.first()?.font_index)?,
            self.font_height(layers.get(2)?.font_index)?,
        )
    }
}

use crate::{
    Element,
    widget::{ScrollBar, ScrollBarRects},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub rects: ScrollBarRects,
    pub range: [i32; 2],
    pub page: i32,
    pub position: i32,
}

impl Layout {
    pub fn clamp(&self, position: i32) -> i32 {
        position.clamp(
            self.range[0],
            self.range[1].saturating_sub(self.page).max(self.range[0]),
        )
    }
}

pub fn state(element: &Element) -> Option<&ScrollBar> {
    if let Some(list) = element.widget.list() {
        Some(&list.scrollbar)
    } else if let Some(combo) = element.widget.combo_box() {
        Some(&combo.scrollbar)
    } else {
        element
            .widget
            .edit()?
            .pad
            .as_ref()
            .map(|pad| &pad.scrollbar)
    }
}

pub fn state_mut(element: &mut Element) -> Option<(&mut ScrollBar, &mut i32)> {
    match &mut element.widget {
        crate::Widget::List(list) => Some((&mut list.scrollbar, &mut list.scroll)),
        crate::Widget::ComboBox(combo) => Some((&mut combo.scrollbar, &mut combo.scroll)),
        crate::Widget::Edit(edit) => edit
            .pad
            .as_mut()
            .map(|pad| (&mut pad.scrollbar, &mut pad.scroll)),
        _ => None,
    }
}

pub(crate) fn thumb_dragging(element: &Element) -> bool {
    state(element).is_some_and(|bar| bar.drag_offset.is_some())
        || element
            .widget
            .combo_box()
            .is_some_and(|combo| combo.drag_offset.is_some())
        || element.widget.list().is_some_and(|list| {
            list.tree
                .as_ref()
                .is_some_and(|tree| tree.drag_offset.is_some())
                || list
                    .custom
                    .as_ref()
                    .is_some_and(|custom| custom.drag_offset.is_some())
        })
}

pub(crate) fn release_thumb(element: &mut Element) {
    if let Some((bar, _)) = state_mut(element) {
        bar.drag_offset = None;
    }

    if let Some(combo) = element.widget.combo_box_mut() {
        combo.drag_offset = None;
    }

    if let Some(list) = element.widget.list_mut() {
        if let Some(tree) = &mut list.tree {
            tree.drag_offset = None;
        }

        if let Some(custom) = &mut list.custom {
            custom.drag_offset = None;
        }
    }
}

pub fn layout(element: &Element) -> Option<Layout> {
    let rect = element.rect();

    if let Some(list) = element.widget.list() {
        let (rects, range, page, position) = if let Some(tree) = &list.tree {
            let l = tree.layout(list, rect);

            (l.bar, l.range, l.page as i32, l.first as i32)
        } else if list.custom.is_some() {
            let l = crate::custom_list::layout(list, rect);

            (
                l.bar,
                [0, l.max_scroll.saturating_add(l.page)],
                l.page,
                l.scroll,
            )
        } else {
            if list.plain && list.text_row_height.is_none() {
                return None;
            }

            let content = list.content_rect(rect);

            let (min, max) = list.scroll_range(content);

            (
                list.scrollbar_rects(rect),
                [min, max],
                list.rows_per_page(content),
                list.clamped_scroll(content),
            )
        };

        Some(Layout {
            rects,
            range,
            page,
            position,
        })
    } else if let Some(combo) = element.widget.combo_box() {
        let l = combo.stored_layout(rect, &element.layers)?;

        if !combo.open || l.page >= combo.items.len() {
            return None;
        }

        Some(Layout {
            rects: l.bar,
            range: [0, combo.items.len() as i32],
            page: l.page as i32,
            position: l.first as i32,
        })
    } else {
        let edit = element.widget.edit()?;

        let pad = edit.pad.as_ref()?;

        let [count, page] = pad.scroll_metrics?;

        let [l, t, r, b] = rect;

        let border = edit.border_width;

        let width = pad.scrollbar_width.max(0);

        let left = if edit.left_scrollbar {
            l + border
        } else {
            r - border - width
        };

        let position = pad.scroll.clamp(0, count.saturating_sub(page).max(0));

        Some(Layout {
            rects: pad.scrollbar.rects(
                [left, t + border, left + width, b - border],
                [0, count],
                page,
                position,
            ),
            range: [0, count],
            page,
            position,
        })
    }
}

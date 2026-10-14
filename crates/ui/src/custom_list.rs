use crate::{
    Element,
    widget::{ListState, ScrollBarRects},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomList {
    pub focused: Option<(usize, usize)>,
    pub hovered: Option<(usize, usize)>,
    pub(crate) drag_offset: Option<i32>,
}

#[derive(Debug)]
pub struct Layout {
    pub rows: Vec<(usize, [i32; 4])>,
    pub scroll: i32,
    pub max_scroll: i32,
    pub page: i32,
    pub bar: ScrollBarRects,
    pub show_bar: bool,
}

pub fn layout(list: &ListState, rect: [i32; 4]) -> Layout {
    let content = list.content_rect(rect);

    let bar_rect = list.scrollbar_rect(rect);

    let total = list.items.iter().fold(0i64, |n, row| {
        n + i64::from(row.height.unwrap_or(list.item_size[1]).max(0))
    });

    let bar_height = i64::from(bar_rect[3])
        .saturating_sub(i64::from(bar_rect[1]))
        .max(0);

    let excess = total
        .saturating_sub(bar_height)
        .max(0)
        .min(i64::from(i32::MAX) - i64::from(i32::MAX) / 10);

    let (range, page) = if excess == 0 {
        (1, 1)
    } else {
        (excess + excess / 10, (excess / 10).max(1))
    };

    let max_scroll = (range - page) as i32;

    let scroll = list.scroll.clamp(0, max_scroll);

    let mut end = 0i64;

    let rows = list
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let start = end;

            end += i64::from(row.height.unwrap_or(list.item_size[1]).max(0));

            let view_bottom = i64::from(content[3]) - i64::from(content[1]) + i64::from(scroll);

            if end < i64::from(scroll) || start > view_bottom {
                return None;
            }

            let top = i64::from(content[1]) - i64::from(scroll) + start;

            let left = content[0].min(content[2]);

            let right = content[0].saturating_add(list.item_size[0]).min(content[2]);

            let bounded = |n: i64| n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;

            Some((index, [left, bounded(top), right, bounded(top + end)]))
        })
        .collect();

    let bar = list
        .scrollbar
        .rects(bar_rect, [0, range as i32], page as i32, scroll);

    Layout {
        rows,
        scroll,
        max_scroll,
        page: page as i32,
        bar,
        show_bar: list.show_scrollbar
            && (!list.auto_hide_scrollbar || page < list.items.len() as i64),
    }
}

impl ListState {
    pub fn add_custom_item(&mut self, height: i32, children: Vec<Element>) -> Option<usize> {
        if self.custom.is_none() || height < 0 {
            return None;
        }

        let index = self.add_line("", 0);

        self.items[index].height = Some(height);

        self.items[index].children = children;

        Some(index)
    }
}

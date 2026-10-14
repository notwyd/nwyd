use crate::{
    Element, Material,
    layout::Measure,
    widget::{ScrollBar, ScrollBarRects},
};
use std::{collections::BTreeMap, ops::Range};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowStyle {
    pub color: u32,
    pub background: u32,
    pub shadow: formats::ui::pane::ShadowType,
    pub shadow_color: u32,
    pub tag: i32,
    pub speaker: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub style: RowStyle,
    pub rich: Option<crate::rich::Line>,
    pub soft_wrap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RowSnapshot {
    buffer: String,
    rows: Vec<Row>,
    layout_key: [i32; 11],
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub read_only: bool,
    pub scrollbar_width: i32,
    pub scroll: i32,
    pub scrollbar: ScrollBar,
    pub child_layers: Vec<Material>,
    pub(crate) selection_held: bool,
    pointer_row: Option<usize>,
    pub font_heights: BTreeMap<i32, i32>,
    measured_caret_row: usize,
    pub(crate) scroll_metrics: Option<[i32; 2]>,
    pub(crate) rows_cleared: bool,
    pub test_string: String,
    page_drawn: bool,
    layout_revision: u64,
    drawn_page: Option<PageAdmission>,
    rows: Option<RowSnapshot>,
    reflow_pending: bool,
    measured_layout: Option<[i32; 11]>,
    pub draw_row_backgrounds: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageAdmission {
    key: [i32; 11],
    revision: u64,
    page: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            read_only: false,
            scrollbar_width: 24,
            scroll: 0,
            scrollbar: ScrollBar::default(),
            child_layers: Vec::new(),
            selection_held: false,
            pointer_row: None,
            font_heights: BTreeMap::new(),
            measured_caret_row: 0,
            scroll_metrics: None,
            rows_cleared: false,
            test_string: String::new(),
            page_drawn: false,
            layout_revision: 0,
            drawn_page: None,
            rows: None,
            reflow_pending: false,
            measured_layout: None,
            draw_row_backgrounds: true,
        }
    }
}

impl State {
    pub fn layout_changed(&mut self) {
        self.layout_revision = self.layout_revision.wrapping_add(1);

        self.drawn_page = None;

        self.rows = None;

        self.rows_cleared = false;

        self.reflow_pending = true;
    }

    pub fn wheel_scroll(&mut self, delta: i32) {
        if let Some([count, page]) = self.scroll_metrics {
            self.scroll = self
                .scroll
                .saturating_sub(delta / 120)
                .clamp(0, count.saturating_sub(page).max(0));
        }
    }
}

pub fn clear_rows(edit: &mut crate::widget::EditState) -> bool {
    let Some(pad) = edit.pad.as_mut() else {
        return false;
    };

    edit.text.clear();

    edit.caret = 0;

    edit.anchor = 0;

    pad.scroll = 0;

    pad.measured_caret_row = 0;

    pad.rows_cleared = true;

    pad.rows = None;

    pad.reflow_pending = false;

    pad.measured_layout = None;

    if let Some(metrics) = &mut pad.scroll_metrics {
        metrics[0] = 1;
    }

    true
}

pub struct Layout {
    pub chars: Vec<char>,
    pub lines: Vec<Range<usize>>,
    pub client: [i32; 4],
    pub pitch: i32,
    pub page: usize,
    pub visible_rows: usize,
    pub page_admission: Option<PageAdmission>,
    pub retained_rows: Option<Vec<Row>>,
    pub scroll: usize,
    pub bar: ScrollBarRects,
}

pub fn layout(element: &Element, measure: &mut dyn Measure, follow_caret: bool) -> Option<Layout> {
    layout_inner(element, measure, follow_caret, false)
}

pub(crate) fn layout_for_draw(element: &Element, measure: &mut dyn Measure) -> Option<Layout> {
    let edit = element.widget.edit()?;

    let pad = edit.pad.as_ref()?;

    if edit.richtext
        && (pad.reflow_pending
            || pad
                .measured_layout
                .is_some_and(|old| Some(old) != page_key(element)))
    {
        return None;
    }

    layout_inner(element, measure, false, true)
}

fn page_key(element: &Element) -> Option<[i32; 11]> {
    let edit = element.widget.edit()?;

    let pad = edit.pad.as_ref()?;

    let font = element.layers.first()?.font_index;

    let height = *pad.font_heights.get(&font)?;

    let [l, t, r, b] = element.rect();

    Some([
        l,
        t,
        r,
        b,
        edit.border_width,
        edit.spacing,
        edit.item_spacing,
        pad.scrollbar_width,
        font,
        height,
        i32::from(edit.left_scrollbar),
    ])
}

pub fn commit_page(element: &mut Element, admission: PageAdmission) {
    let key = page_key(element);

    let Some(pad) = element.widget.edit_mut().and_then(|e| e.pad.as_mut()) else {
        return;
    };

    if pad.page_drawn {
        return;
    }

    pad.page_drawn = true;

    if key == Some(admission.key) && pad.layout_revision == admission.revision {
        pad.drawn_page = Some(admission);
    }
}

fn layout_inner(
    element: &Element,
    measure: &mut dyn Measure,
    follow_caret: bool,
    drawing: bool,
) -> Option<Layout> {
    let edit = element.widget.edit()?;

    let pad = edit.pad.as_ref()?;

    let font = element.layers.first()?.font_index;

    let pitch = *pad.font_heights.get(&font)?;

    if pitch <= 0 {
        return None;
    }

    let [left, top, right, bottom] = element.rect();

    let b = edit.border_width;

    let inset = b + edit.spacing;

    let w = pad.scrollbar_width.max(0);

    let client = [
        left + inset + if edit.left_scrollbar { w } else { 0 },
        top + inset,
        right - inset - if edit.left_scrollbar { 0 } else { w },
        bottom - inset,
    ];

    let width = client[2] - client[0];

    if width <= 0 {
        return None;
    }

    let chars: Vec<char> = edit.text.chars().collect();

    let key = page_key(element)?;

    let retained_rows = pad
        .rows
        .as_ref()
        .filter(|s| s.buffer == edit.text && s.layout_key == key)
        .map(|s| s.rows.clone());

    let mut lines = Vec::new();

    let mut start = 0;

    let mut i = 0;

    let mut advance = 0;

    let mut space = None;

    if let Some(rows) = &retained_rows {
        for (index, row) in rows.iter().enumerate() {
            let end = start + row.text.chars().count();

            lines.push(start..end);

            start = end + usize::from(!row.soft_wrap && index + 1 < rows.len());
        }
    }

    while retained_rows.is_none() && i < chars.len() {
        if chars[i] == '\n' || chars[i] == '\r' {
            lines.push(start..i);

            if chars[i] == '\r' && chars.get(i + 1) == Some(&'\n') {
                i += 1;
            }

            i += 1;

            start = i;

            advance = 0;

            space = None;

            continue;
        }

        let mut buf = [0; 2];

        for unit in chars[i].encode_utf16(&mut buf) {
            advance += measure.advance(font, *unit)?;
        }

        if chars[i] == ' ' {
            space = Some(i);
        }

        i += 1;

        if advance >= width {
            let end = if pad.read_only {
                space.filter(|s| *s > start).map(|s| s + 1).unwrap_or(i)
            } else {
                i
            };

            lines.push(start..end);

            start = end;

            i = end;

            advance = 0;

            space = None;
        }
    }

    if retained_rows.is_none() && (!pad.rows_cleared || !chars.is_empty()) {
        lines.push(start..chars.len());
    }

    let height = i64::from(client[3]) - i64::from(client[1]);

    let visible_rows = (height.max(0) / i64::from(pitch)) as usize;

    let first_draw = drawing && !pad.page_drawn && !lines.is_empty();

    let page_admission = first_draw.then_some(PageAdmission {
        key,
        revision: pad.layout_revision,
        page: visible_rows,
    });

    let retained = pad
        .drawn_page
        .filter(|p| p.key == key && p.revision == pad.layout_revision);

    let page = if first_draw {
        visible_rows
    } else if let Some(p) = retained {
        p.page
    } else {
        let step = i64::from(pitch) + i64::from(edit.item_spacing);

        if step <= 0 {
            0
        } else {
            (height.max(0) / step) as usize
        }
    };

    let max = lines.len().saturating_sub(page);

    let mut scroll = (pad.scroll.max(0) as usize).min(max);

    if follow_caret && !pad.read_only {
        let current = lines
            .iter()
            .rposition(|line| line.start <= edit.caret.max(0) as usize)
            .unwrap_or(0);

        if current < scroll {
            scroll = current;
        }

        if current >= scroll + page.max(1) {
            scroll = current + 1 - page.max(1);
        }
    }

    let bar_left = if edit.left_scrollbar {
        left + b
    } else {
        right - b - w
    };

    let rect = [bar_left, top + b, bar_left + w, bottom - b];

    let bounded = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);

    let bar = pad.scrollbar.rects(
        rect,
        [0, bounded(lines.len().max(1))],
        bounded(page),
        bounded(scroll),
    );

    Some(Layout {
        chars,
        lines,
        client,
        pitch,
        page,
        visible_rows,
        page_admission,
        retained_rows,
        scroll,
        bar,
    })
}

pub fn append_row(
    element: &mut Element,
    text: &str,
    style: RowStyle,
    measure: &mut dyn Measure,
) -> Option<usize> {
    rebuild_after_layout(element, measure)?;

    let view = layout(element, measure, false)?;

    let edit = element.widget.edit()?;

    let incoming = if edit.richtext {
        let font = element.layers.get(1)?.font_index;

        crate::rich::wrap(text, font, view.client[2] - view.client[0], 0, measure)?
            .lines
            .into_iter()
            .map(|line| Row {
                text: line.text(),
                style: style.clone(),
                rich: Some(line),
                soft_wrap: false,
            })
            .collect::<Vec<_>>()
    } else {
        vec![Row {
            text: text.to_owned(),
            style,
            rich: None,
            soft_wrap: false,
        }]
    };

    let mut rows = view.retained_rows.unwrap_or_else(|| {
        view.lines
            .iter()
            .enumerate()
            .map(|(i, r)| Row {
                text: view.chars[r.clone()].iter().collect(),
                style: RowStyle::default(),
                rich: None,
                soft_wrap: view
                    .lines
                    .get(i + 1)
                    .is_some_and(|next| next.start == r.end),
            })
            .collect()
    });

    let first = rows.len();

    rows.extend(incoming);

    let mut buffer = String::new();

    for (i, row) in rows.iter().enumerate() {
        buffer.push_str(&row.text);

        if !row.soft_wrap && i + 1 < rows.len() {
            buffer.push('\n');
        }
    }

    let layout_key = page_key(element)?;

    let edit = element.widget.edit_mut()?;

    edit.text = buffer.clone();

    let pad = edit.pad.as_mut()?;

    pad.rows_cleared = rows.is_empty();

    pad.rows = Some(RowSnapshot {
        buffer,
        rows,
        layout_key,
    });

    refresh_scroll(element, measure);

    Some(first)
}

fn rebuild_after_layout(element: &mut Element, measure: &mut dyn Measure) -> Option<()> {
    let key = page_key(element)?;

    let edit = element.widget.edit()?;

    let pad = edit.pad.as_ref()?;

    let changed = pad.measured_layout.is_some_and(|old| old != key);

    if !pad.reflow_pending && !changed {
        return Some(());
    }

    let view = layout(element, measure, false)?;

    let replacement = if edit.richtext {
        let font = element.layers.get(1)?.font_index;

        let mut rows = Vec::new();

        for (index, range) in view.lines.iter().enumerate() {
            let text: String = view.chars[range.clone()].iter().collect();

            let parsed =
                crate::rich::wrap(&text, font, view.client[2] - view.client[0], 0, measure)?;

            rows.extend(parsed.lines.into_iter().map(|line| Row {
                text: line.text(),
                style: RowStyle::default(),
                rich: Some(line),
                soft_wrap: false,
            }));

            if view
                .lines
                .get(index + 1)
                .is_some_and(|next| next.start == range.end)
                && let Some(last) = rows.last_mut()
            {
                last.soft_wrap = true;
            }
        }

        let mut buffer = String::new();

        for (index, row) in rows.iter().enumerate() {
            buffer.push_str(&row.text);

            if !row.soft_wrap && index + 1 < rows.len() {
                buffer.push('\n');
            }
        }

        Some(RowSnapshot {
            buffer,
            rows,
            layout_key: key,
        })
    } else {
        None
    };

    let edit = element.widget.edit_mut()?;

    if let Some(rows) = &replacement {
        edit.text = rows.buffer.clone();

        let count = edit.text.chars().count() as i32;

        edit.caret = edit.caret.clamp(0, count);

        edit.anchor = edit.anchor.clamp(0, count);
    }

    let pad = edit.pad.as_mut()?;

    pad.rows_cleared = replacement.as_ref().is_some_and(|r| r.rows.is_empty());

    pad.rows = replacement;

    pad.reflow_pending = false;

    pad.measured_layout = Some(key);

    Some(())
}

pub fn refresh_scroll(element: &mut Element, measure: &mut dyn Measure) {
    if rebuild_after_layout(element, measure).is_none() {
        if let Some(pad) = element.widget.edit_mut().and_then(|e| e.pad.as_mut()) {
            pad.scroll_metrics = None;
        }

        return;
    }

    let key = page_key(element);

    if let Some(edit) = element.widget.edit_mut()
        && let Some(pad) = edit.pad.as_mut()
        && pad
            .rows
            .as_ref()
            .is_some_and(|s| s.buffer != edit.text || key.is_some_and(|key| s.layout_key != key))
    {
        pad.rows = None;
    }

    if let Some(pad) = element.widget.edit_mut().and_then(|e| e.pad.as_mut())
        && pad.drawn_page.is_some_and(|p| Some(p.key) != key)
    {
        pad.drawn_page = None;
    }

    let Some(view) = layout(element, measure, false) else {
        if let Some(pad) = element.widget.edit_mut().and_then(|e| e.pad.as_mut()) {
            pad.scroll_metrics = None;
        }

        return;
    };

    let edit = element.widget.edit_mut().unwrap();

    let row = view
        .lines
        .iter()
        .rposition(|r| r.start <= edit.caret.max(0) as usize)
        .unwrap_or(0);

    let pad = edit.pad.as_mut().unwrap();

    let mut scroll = view.scroll;

    if !pad.read_only && row != pad.measured_caret_row {
        if row < scroll {
            scroll = row;
        }

        if row >= scroll + view.page.max(1) {
            scroll = row + 1 - view.page.max(1);
        }
    }

    pad.scroll = scroll as i32;

    pad.measured_caret_row = row;

    if !edit.text.is_empty() {
        pad.rows_cleared = false;
    }

    pad.scroll_metrics = Some([
        i32::try_from(view.lines.len().max(1)).unwrap_or(i32::MAX),
        i32::try_from(view.page).unwrap_or(i32::MAX),
    ]);

    pad.measured_layout = key;
}

pub fn refresh_panel(panel: &mut crate::Panel, measure: &mut dyn Measure) {
    let mut elements: Vec<_> = panel.elements.iter_mut().collect();

    while let Some(element) = elements.pop() {
        refresh_scroll(element, measure);

        if let Some(list) = element.widget.list_mut() {
            for row in &mut list.items {
                elements.extend(row.children.iter_mut());
            }
        }
    }
}

pub fn click(element: &mut Element, x: i32, y: i32, shift: bool, measure: &mut dyn Measure) {
    refresh_scroll(element, measure);

    let Some(view) = layout(element, measure, false) else {
        return;
    };

    let font = element.layers[0].font_index;

    let edit = element.widget.edit_mut().unwrap();

    if x >= view.bar.track[0] && x < view.bar.track[2] {
        return;
    }

    let row = ((y - view.client[1]).max(0) / view.pitch) as usize + view.scroll;

    let Some(range) = view.lines.get(row) else {
        return;
    };

    let mut pen = view.client[0];

    let mut caret = range.start;

    for (index, ch) in view.chars[range.clone()].iter().enumerate() {
        let mut buf = [0; 2];

        let width: i32 = ch
            .encode_utf16(&mut buf)
            .iter()
            .filter_map(|u| measure.advance(font, *u))
            .sum();

        if x < pen + width / 2 {
            break;
        }

        pen += width;

        caret = range.start + index + 1;
    }

    edit.caret = caret as i32;

    if !shift {
        edit.anchor = edit.caret;
    }

    refresh_scroll(element, measure);
}

pub fn pointer(
    element: &mut Element,
    event: crate::input::Mouse,
    x: i32,
    y: i32,
    shift: bool,
    measure: &mut dyn Measure,
) -> bool {
    use crate::input::Mouse;

    refresh_scroll(element, measure);

    let Some(view) = layout(element, measure, false) else {
        release_pointer(element);

        return false;
    };

    let font = element.layers[0].font_index;

    let edit = element.widget.edit_mut().unwrap();

    let pad = edit.pad.as_mut().unwrap();

    let row = if event == Mouse::Move {
        if !pad.selection_held {
            return false;
        }

        let Some(row) = pad.pointer_row else {
            return false;
        };

        row
    } else if matches!(event, Mouse::LeftDown | Mouse::LeftDouble) {
        if x < view.client[0] || x >= view.client[2] || y < view.client[1] || y >= view.client[3] {
            pad.selection_held = false;

            pad.pointer_row = None;

            return false;
        }

        ((y - view.client[1]) / view.pitch) as usize + view.scroll
    } else {
        return false;
    };

    let Some(range) = view.lines.get(row) else {
        pad.selection_held = false;

        pad.pointer_row = None;

        return false;
    };

    if event == Mouse::LeftDouble {
        edit.anchor = range.start as i32;

        edit.caret = range.end as i32;
    } else {
        let mut pen = view.client[0];

        let mut caret = range.start;

        for (index, ch) in view.chars[range.clone()].iter().enumerate() {
            let mut buf = [0; 2];

            let width: i32 = ch
                .encode_utf16(&mut buf)
                .iter()
                .filter_map(|u| measure.advance(font, *u))
                .sum();

            if x < pen + width / 2 {
                break;
            }

            pen += width;

            caret = range.start + index + 1;
        }

        edit.caret = caret as i32;

        if event == Mouse::LeftDown && !shift {
            edit.anchor = edit.caret;
        }
    }

    pad.pointer_row = Some(row);

    pad.selection_held = true;

    edit.reset_caret();

    refresh_scroll(element, measure);

    true
}

pub fn release_pointer(element: &mut Element) {
    if let Some(pad) = element.widget.edit_mut().and_then(|e| e.pad.as_mut()) {
        pad.selection_held = false;

        pad.pointer_row = None;
    }
}

pub fn pointer_active(element: &Element) -> bool {
    element
        .widget
        .edit()
        .and_then(|e| e.pad.as_ref())
        .is_some_and(|p| p.selection_held)
}

pub fn key(
    element: &mut Element,
    key: crate::text_entry::Key,
    mods: crate::text_entry::Mods,
    measure: &mut dyn Measure,
) {
    key_with_clipboard(
        element,
        key,
        mods,
        measure,
        &mut crate::text_entry::Local::default(),
    );
}

pub fn key_with_clipboard(
    element: &mut Element,
    key: crate::text_entry::Key,
    mods: crate::text_entry::Mods,
    measure: &mut dyn Measure,
    clipboard: &mut dyn crate::text_entry::Clipboard,
) {
    use crate::text_entry::Key;

    refresh_scroll(element, measure);

    let Some(view) = layout(element, measure, true) else {
        return;
    };

    let edit = element.widget.edit_mut().unwrap();

    let row = view
        .lines
        .iter()
        .rposition(|r| r.start <= edit.caret.max(0) as usize)
        .unwrap_or(0);

    let Some(range) = view.lines.get(row) else {
        return;
    };

    let delta = match key {
        Key::Up => -1,
        Key::Down => 1,
        Key::PageUp => -(view.page.max(1) as i32),
        Key::PageDown => view.page.max(1) as i32,
        _ => 0,
    };

    if delta != 0 {
        let target = (row as i32 + delta).clamp(0, view.lines.len() as i32 - 1) as usize;

        let next = &view.lines[target];

        edit.caret =
            (next.start + (edit.caret.max(0) as usize - range.start).min(next.len())) as i32;

        if !mods.shift {
            edit.anchor = edit.caret;
        }

        let pad = edit.pad.as_mut().unwrap();

        pad.scroll = if target < view.scroll {
            target
        } else if target >= view.scroll + view.page.max(1) {
            target + 1 - view.page.max(1)
        } else {
            view.scroll
        } as i32;
    } else if !mods.ctrl && matches!(key, Key::Home | Key::End) {
        edit.caret = if key == Key::Home {
            range.start
        } else {
            range.end
        } as i32;

        if !mods.shift {
            edit.anchor = edit.caret;
        }
    } else {
        crate::text_entry::key(edit, key, mods, clipboard);
    }

    refresh_scroll(element, measure);
}

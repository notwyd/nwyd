use crate::{Element, input::Mouse, layout::Measure};

pub fn pointer(
    element: &mut Element,
    rect: [i32; 4],
    event: Mouse,
    x: i32,
    measure: &mut dyn Measure,
) -> bool {
    let kind = element.element_type;

    let Some(font) = element.layers.get(1).map(|m| m.font_index) else {
        return false;
    };

    let Some(edit) = element
        .widget
        .edit_mut()
        .filter(|e| !e.note_pad && e.selection_held)
    else {
        return false;
    };

    if !matches!(event, Mouse::LeftDown | Mouse::LeftDouble | Mouse::Move) {
        return false;
    }

    let text_rect = edit
        .rects(kind, rect[0], rect[1], rect[2] - rect[0], rect[3] - rect[1])
        .text;

    let mut edges = vec![0i32];

    for ch in edit.masked().chars() {
        let mut advance = 0i32;

        for unit in ch.encode_utf16(&mut [0; 2]) {
            let Some(width) = measure.advance(font, *unit) else {
                return false;
            };

            advance = advance.saturating_add(width.max(0));
        }

        edges.push(edges.last().unwrap().saturating_add(advance));
    }

    let scroll = if edit.password {
        0
    } else {
        edit.scroll.max(0) as usize
    }
    .min(edges.len() - 1);

    let position = x.saturating_sub(text_rect[0]).saturating_add(edges[scroll]);

    let caret = edges
        .windows(2)
        .position(|w| i64::from(position) * 2 < i64::from(w[0]) + i64::from(w[1]))
        .unwrap_or(edges.len() - 1);

    edit.preedit = None;

    edit.caret = caret as i32;

    if event != Mouse::Move {
        edit.anchor = edit.caret;
    }

    if event == Mouse::LeftDouble {
        edit.anchor = 0;

        edit.caret = edit.text.chars().count() as i32;
    }

    if !edit.password && !edit.multiline {
        let caret = (edit.caret.max(0) as usize).min(edges.len() - 1);

        let end = edges[(caret + 1).min(edges.len() - 1)];

        let mut first = scroll.min(caret);

        let width = text_rect[2].saturating_sub(text_rect[0]).max(0);

        while first < caret && end.saturating_sub(edges[first]) > width {
            first += 1;
        }

        edit.scroll = first as i32;
    }

    edit.reset_caret();

    true
}

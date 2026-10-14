use formats::ui::pane::font_align;
use std::ops::Range;

pub trait Measure {
    fn advance(&mut self, font: i32, unit: u16) -> Option<i32>;

    fn height(&mut self, font: i32) -> Option<i32>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unmeasured;

impl Measure for Unmeasured {
    fn advance(&mut self, _font: i32, _unit: u16) -> Option<i32> {
        None
    }

    fn height(&mut self, _font: i32) -> Option<i32> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub range: Range<usize>,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truncation {
    UnfittableUnit,
    IterationCap,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    pub lines: Vec<Line>,
    pub max_width: i32,
    pub truncated: Option<Truncation>,
}

impl Layout {
    pub fn laid_out(&self) -> usize {
        self.lines.last().map_or(0, |l| l.range.end)
    }

    pub fn total_height(&self) -> i32 {
        self.lines.iter().map(|l| l.height).sum()
    }
}

pub fn line_offset(align: u32, wrap_width: i32, line_width: i32) -> i32 {
    let difference = wrap_width - line_width;

    if align & font_align::RIGHT != 0 {
        difference
    } else if align & font_align::CENTER != 0 {
        ((difference as u32) >> 1) as i32
    } else {
        0
    }
}

pub fn block_offset(align: u32, box_height: i32, total_height: i32) -> i32 {
    let mut offset = 0;

    if align & font_align::VCENTER != 0 {
        offset = (box_height - total_height) / 2;
    }

    if align & font_align::BOTTOM != 0 {
        offset = box_height - total_height;
    }

    offset
}

pub const MAX_ITERATIONS: u32 = 999;

pub const BREAK_TAG: &[u16] = &[b'<' as u16, b'n' as u16, b'>' as u16];

pub fn wrap(
    units: &[u16],
    font: i32,
    wrap_width: i32,
    gap: i32,
    measure: &mut dyn Measure,
) -> Option<Layout> {
    let run_height = measure.height(font)?;

    let mut layout = Layout::default();

    let mut start = 0usize;

    let mut index = 0usize;

    let mut accumulated = 0i32;

    let mut last_fail = 0usize;

    let mut iterations = 0u32;

    while index < units.len() {
        iterations += 1;

        if iterations > MAX_ITERATIONS {
            layout.truncated = Some(Truncation::IterationCap);

            return Some(layout);
        }

        if let Some(skip) = separator_at(units, index) {
            commit(&mut layout, start..index, accumulated, run_height, gap);

            index += skip;

            start = index;

            accumulated = 0;

            continue;
        }

        let advance = measure.advance(font, units[index]).unwrap_or(0);

        if accumulated + advance >= wrap_width {
            if last_fail == index {
                layout.truncated = Some(Truncation::UnfittableUnit);

                return Some(layout);
            }

            commit(&mut layout, start..index, accumulated, run_height, gap);

            last_fail = index;

            start = index;

            accumulated = 0;

            continue;
        }

        accumulated += advance;

        index += 1;
    }

    commit(&mut layout, start..index, accumulated, run_height, gap);

    Some(layout)
}

fn separator_at(units: &[u16], index: usize) -> Option<usize> {
    if units[index..].starts_with(BREAK_TAG) {
        return Some(BREAK_TAG.len());
    }

    const CR: u16 = 0x000D;

    const LF: u16 = 0x000A;

    match units[index] {
        CR if units.get(index + 1) == Some(&LF) => Some(2),
        LF => Some(1),
        _ => None,
    }
}

fn commit(layout: &mut Layout, range: Range<usize>, width: i32, run_height: i32, gap: i32) {
    let height = if range.is_empty() { 0 } else { run_height };

    if let Some(previous) = layout.lines.last_mut() {
        previous.height += gap;
    }

    layout.max_width = layout.max_width.max(width);

    layout.lines.push(Line {
        range,
        width,
        height,
    });
}

#[cfg(test)]
mod tests {

    use super::*;

    struct Fixed;

    impl Measure for Fixed {
        fn advance(&mut self, _: i32, _: u16) -> Option<i32> {
            Some(3)
        }

        fn height(&mut self, _: i32) -> Option<i32> {
            Some(10)
        }
    }

    #[test]
    fn explicit_breaks_skip_tags_and_crlf_without_including_them_in_lines() {
        let units: Vec<u16> = "a<n>b\r\nc\nd".encode_utf16().collect();

        let layout = wrap(&units, 0, 20, 2, &mut Fixed).unwrap();

        assert_eq!(
            layout
                .lines
                .iter()
                .map(|line| line.range.clone())
                .collect::<Vec<_>>(),
            [0..1, 4..5, 7..8, 9..10]
        );

        assert_eq!(layout.total_height(), 46);

        assert_eq!(layout.max_width, 3);

        assert_eq!(layout.laid_out(), units.len());

        assert_eq!(layout.truncated, None);
    }

    #[test]
    fn wrapping_requires_space_beyond_glyph_width_and_reports_unfittable_units() {
        let units: Vec<u16> = "abcde".encode_utf16().collect();

        let layout = wrap(&units, 0, 7, 2, &mut Fixed).unwrap();

        assert_eq!(
            layout.lines,
            vec![
                Line {
                    range: 0..2,
                    width: 6,
                    height: 12
                },
                Line {
                    range: 2..4,
                    width: 6,
                    height: 12
                },
                Line {
                    range: 4..5,
                    width: 3,
                    height: 10
                },
            ]
        );

        let blocked = wrap(&units, 0, 3, 2, &mut Fixed).unwrap();

        assert_eq!(blocked.truncated, Some(Truncation::UnfittableUnit));

        assert_eq!(blocked.laid_out(), 0);
    }

    #[test]
    fn missing_metrics_and_iteration_limit_are_explicit() {
        assert!(wrap(&[b'a' as u16], 0, 10, 0, &mut Unmeasured).is_none());

        let units = vec![b'a' as u16; MAX_ITERATIONS as usize + 1];

        let layout = wrap(&units, 0, 4_000, 0, &mut Fixed).unwrap();

        assert_eq!(layout.truncated, Some(Truncation::IterationCap));

        assert_eq!(layout.laid_out(), 0);

        let complete = wrap(&units[..MAX_ITERATIONS as usize], 0, 4_000, 0, &mut Fixed).unwrap();

        assert_eq!(complete.truncated, None);

        assert_eq!(complete.laid_out(), MAX_ITERATIONS as usize);
    }
}

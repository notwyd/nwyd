use super::*;

struct Measure;

impl ui::layout::Measure for Measure {
    fn advance(&mut self, _font: i32, unit: u16) -> Option<i32> {
        Some(if unit == u16::from(b'W') { 80 } else { 10 })
    }

    fn height(&mut self, _font: i32) -> Option<i32> {
        Some(12)
    }
}

fn wrapped(text: &str, width: i64, content_width: i32) -> Option<Vec<String>> {
    wrap_styled_lines(text, 0, width, content_width, &mut Measure)
        .map(|lines| lines.iter().map(ui::rich::Line::text).collect())
}

#[test]
fn wrapping_width_override_must_be_in_range_without_retrying_content_width() {
    for width in [1, 79, 2001, i64::from(i32::MAX) + 1, i64::MAX] {
        assert_eq!(wrapped("message", width, 300), None, "width {width}");
    }

    for width in [80, 2000] {
        assert_eq!(
            wrapped("message", width, 0),
            Some(vec!["message".to_string()]),
            "width {width}"
        );
    }
}

#[test]
fn nonpositive_overrides_use_content_width_with_the_same_bounds() {
    for width in [i64::MIN, -1, 0] {
        for content_width in [-1, 0, 79, 2001] {
            assert_eq!(wrapped("message", width, content_width), None);
        }

        for content_width in [80, 2000] {
            assert_eq!(
                wrapped("message", width, content_width),
                Some(vec!["message".to_string()])
            );
        }
    }
}

#[test]
fn wrapping_accepts_nine_complete_rows_and_rejects_the_tenth() {
    assert_eq!(
        wrapped(&"a".repeat(63), 80, 300),
        Some(vec!["a".repeat(7); 9])
    );
    assert_eq!(wrapped(&"a".repeat(64), 80, 300), None);
}

#[test]
fn unfittable_continuation_rejects_the_already_wrapped_prefix() {
    let layout = ui::rich::wrap("iW", 0, 80, 0, &mut Measure).unwrap();
    assert_eq!(layout.lines[0].text(), "i");
    assert_eq!(
        layout.truncated,
        Some(ui::layout::Truncation::UnfittableUnit)
    );
    assert_eq!(wrapped("iW", 80, 300), None);
}

#[test]
fn iteration_limit_rejects_the_already_wrapped_prefix() {
    let text = format!("a<n>{}", "<pass>".repeat(1000));
    let layout = ui::rich::wrap(&text, 0, 80, 0, &mut Measure).unwrap();
    assert_eq!(layout.lines[0].text(), "a");
    assert_eq!(layout.truncated, Some(ui::layout::Truncation::IterationCap));
    assert_eq!(wrapped(&text, 80, 300), None);
}

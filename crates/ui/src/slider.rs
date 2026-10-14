use formats::ui::pane::Property;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slider {
    pub range: [i32; 2],
    pub value: i32,
    pub mouse_move_lock: bool,
    pub drag_offset: Option<i32>,
}

impl Default for Slider {
    fn default() -> Self {
        Self {
            range: [0, 100],
            value: 0,
            mouse_move_lock: false,
            drag_offset: None,
        }
    }
}

impl Slider {
    pub fn set_value(&mut self, value: i32) -> bool {
        let value = value.max(self.range[0]).min(self.range[1]);

        let changed = self.value != value;

        self.value = value;

        changed
    }

    pub fn apply(&mut self, property: &Property) -> bool {
        match property.base_key() {
            k if k.eq_ignore_ascii_case("Range") => {
                property.scan(&mut self.range);

                self.set_value(self.value);
            }
            k if k.eq_ignore_ascii_case("Value") => {
                self.set_value(property.int());
            }
            k if k.eq_ignore_ascii_case("MouseMoveLock") => {
                self.mouse_move_lock = property.value.eq_ignore_ascii_case("true");
            }
            _ => return false,
        }

        true
    }

    pub fn offset(&self, rect: [i32; 4]) -> i32 {
        let span = i128::from(self.range[1]) - i128::from(self.range[0]);

        if span == 0 {
            return 0;
        }

        let offset = (i128::from(self.value) - i128::from(self.range[0]))
            * (i128::from(rect[2]) - i128::from(rect[0]))
            / span;

        offset.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
    }

    pub fn thumb(&self, rect: [i32; 4]) -> [i32; 4] {
        let height = rect[3].saturating_sub(rect[1]);

        let left = rect[0]
            .saturating_sub(height / 2)
            .saturating_add(self.offset(rect));

        [left, rect[1], left.saturating_add(height), rect[3]]
    }

    pub fn value_at(&self, rect: [i32; 4], x: i32) -> i32 {
        let width = f64::from(rect[2]) - f64::from(rect[0]);

        if width == 0.0 {
            return self.value;
        }

        let ratio = ((f64::from(self.range[1]) - f64::from(self.range[0])) / width) as f32;

        (f64::from(ratio) * (f64::from(x) - f64::from(rect[0])) + f64::from(self.range[0]) + 0.5)
            as i32
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn values_clamp_and_range_changes_reclamp_current_value() {
        let mut slider = Slider::default();

        assert!(!slider.set_value(-1));

        assert!(slider.set_value(120));

        assert_eq!(slider.value, 100);

        assert!(!slider.set_value(101));

        assert!(slider.apply(&Property {
            key: "rAnGe".into(),
            value: "10 40".into()
        }));

        assert_eq!((slider.range, slider.value), ([10, 40], 40));

        assert!(slider.set_value(-100));

        assert_eq!(slider.value, 10);
    }

    #[test]
    fn offset_supports_full_integer_range_and_collapsed_range() {
        let mut slider = Slider {
            range: [i32::MIN, i32::MAX],
            value: i32::MIN,
            ..Slider::default()
        };

        assert_eq!(slider.offset([10, 0, 110, 20]), 0);

        slider.set_value(i32::MAX);

        assert_eq!(slider.offset([10, 0, 110, 20]), 100);

        assert_eq!(slider.offset([i32::MIN, 0, i32::MAX, 20]), i32::MAX);

        slider.range = [7, 7];

        slider.set_value(100);

        assert_eq!(slider.offset([10, 0, 110, 20]), 0);
    }

    #[test]
    fn thumb_centers_on_value_and_pointer_conversion_handles_zero_width() {
        let slider = Slider {
            range: [20, 40],
            value: 30,
            ..Slider::default()
        };

        let rect = [100, 10, 300, 30];

        assert_eq!(slider.thumb(rect), [190, 10, 210, 30]);

        assert_eq!(slider.value_at(rect, 100), 20);

        assert_eq!(slider.value_at(rect, 200), 30);

        assert_eq!(slider.value_at(rect, 300), 40);

        assert_eq!(slider.value_at([100, 10, 100, 30], i32::MAX), 30);
    }
}

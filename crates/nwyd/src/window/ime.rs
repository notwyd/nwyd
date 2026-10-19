#[derive(Default)]
pub(super) struct CursorAreaCache {
    sent: Option<([f64; 4], f64)>,
}

impl CursorAreaCache {
    pub fn update(&mut self, area: [f64; 4], scale_factor: f64) -> Option<[f64; 4]> {
        let next = (area, scale_factor);

        if self.sent == Some(next) {
            return None;
        }

        self.sent = Some(next);

        Some(area)
    }

    pub fn clear(&mut self) {
        self.sent = None;
    }
}

#[cfg(test)]
mod tests;

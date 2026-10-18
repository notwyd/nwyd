use super::World;
use glam::Vec2;

#[cfg(test)]
mod tests;

pub(crate) fn required_zones(
    player: Vec2,
    retail_far: f32,
    view_far: f32,
    camera_offset: f32,
    viewport: (u32, u32),
) -> Vec<(u8, u8)> {
    if !player.is_finite()
        || !player.cmpge(Vec2::ZERO).all()
        || !player.cmplt(Vec2::splat(4096.0)).all()
        || !view_far.is_finite()
        || view_far <= retail_far
        || !camera_offset.is_finite()
        || viewport.0 == 0
        || viewport.1 == 0
    {
        return Vec::new();
    }

    let half_y = (render::camera::FOV_FRACTION * render::camera::PI * 0.5).tan();
    let half_x = half_y * viewport.0 as f32 / viewport.1 as f32;
    let radius = view_far * (1.0 + half_y * half_y + half_x * half_x).sqrt()
        + camera_offset.max(0.0)
        + super::STREAM_MARGIN;
    let span = formats::world::ZONE_CELLS as f32;
    let low = ((player - Vec2::splat(radius)) / span)
        .floor()
        .max(Vec2::ZERO);
    let high = ((player + Vec2::splat(radius)) / span)
        .floor()
        .min(Vec2::splat(31.0));
    let mut zones = Vec::new();

    for z in low.y as u8..=high.y as u8 {
        for x in low.x as u8..=high.x as u8 {
            let min = Vec2::new(f32::from(x), f32::from(z)) * span;
            let nearest = player.clamp(min, min + Vec2::splat(span));
            let distance = player.distance_squared(nearest);

            if distance <= radius * radius {
                let centre_distance = player.distance_squared(min + Vec2::splat(span * 0.5));

                zones.push(((x, z), distance, centre_distance));
            }
        }
    }

    zones.sort_unstable_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then_with(|| a.2.total_cmp(&b.2))
            .then_with(|| a.0.cmp(&b.0))
    });

    zones.into_iter().map(|(zone, _, _)| zone).collect()
}

impl World {
    pub(crate) fn poll_visual_zone(
        &mut self,
        game: &assets::GameDir,
        stage: &mut impl FnMut(u64, &super::loading::Prepared) -> bool,
    ) -> anyhow::Result<bool> {
        let Some(zone) = self.visual_zone else {
            return Ok(false);
        };

        if self.holds(zone) || self.force_load.is_some_and(|forced| forced != zone) {
            let result = self
                .background_loader
                .as_mut()
                .map_or(Ok(true), |loader| loader.discard(zone));

            match result {
                Ok(true) => self.visual_zone = None,
                Ok(false) => {}
                Err(error) => {
                    self.background_loader = None;

                    self.visual_zone = None;

                    return Err(error);
                }
            }

            return Ok(false);
        }

        let result = self.load_visual_zone_background(game, zone, stage);

        if matches!(result, Ok(true)) {
            self.visual_collision_dirty = true;
        }

        if self.holds(zone)
            || self
                .background_loader
                .as_ref()
                .is_none_or(super::loading::Loader::is_idle)
        {
            self.visual_zone = None;
        }

        result
    }

    pub(crate) fn prefetch_visual_zones(
        &mut self,
        game: &assets::GameDir,
        player: Vec2,
        camera_offset: f32,
        viewport: (u32, u32),
        stage: &mut impl FnMut(u64, &super::loading::Prepared) -> bool,
    ) -> anyhow::Result<bool> {
        if self.visual_zone.is_some() || self.force_load.is_some() {
            return Ok(false);
        }

        let zones = required_zones(
            player,
            self.environment.far_plane,
            self.view_far_plane(),
            camera_offset,
            viewport,
        );

        if zones.is_empty() {
            return Ok(false);
        }

        if let Some(zone) = self
            .background_loader
            .as_ref()
            .and_then(super::loading::Loader::active_zone)
        {
            self.visual_zone = Some(zone);

            return self.poll_visual_zone(game, stage);
        }

        let Some(zone) = zones.into_iter().find(|&zone| {
            !self.holds(zone)
                && self
                    .background_loader
                    .as_ref()
                    .is_none_or(|loader| loader.can_request(zone))
                && game.contains(format!("Env/Field{:02}{:02}.trn", zone.0, zone.1).as_bytes())
                && game.contains(format!("Env/Field{:02}{:02}.dat", zone.0, zone.1).as_bytes())
        }) else {
            return Ok(false);
        };

        self.visual_zone = Some(zone);

        self.poll_visual_zone(game, stage)
    }

    pub(crate) fn restore_cached_stream_collision(&mut self, direction: super::Direction) {
        if let Some((zone, _)) = super::neighbour_of(self.current, direction) {
            self.restore_stream_collision(direction, zone, false);
        }
    }
}

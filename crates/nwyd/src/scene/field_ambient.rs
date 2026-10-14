use super::{
    effects::{Billboard, Motion},
    magic_arrow::MagicArrow,
    rng::Lcg,
};
use glam::Vec3;

#[derive(Debug, Default)]
pub struct Batch {
    pub ambient: bool,
    pub seeds: Vec<MagicArrow>,
}

#[derive(Debug, Clone, Default)]
pub struct State {
    ambient_ms: u32,
    seeds_ms: u32,
}

impl State {
    pub fn tick(
        &mut self,
        now: u32,
        cycle: bool,
        env_global: i32,
        player: Vec3,
        rng: &mut Lcg,
    ) -> Batch {
        if !cycle || now.wrapping_sub(self.ambient_ms) <= 1000 {
            return Batch::default();
        }

        self.ambient_ms = now;

        let mut batch = Batch {
            ambient: true,
            ..Batch::default()
        };

        if env_global != 3 || now.wrapping_sub(self.seeds_ms) <= 2000 {
            return batch;
        }

        self.seeds_ms = now;

        batch.seeds = (0..8)
            .map(|i| {
                let x = rng.next_mod(5) as f32;

                let z = rng.next_mod(5) as f32;

                let i = i as f32;

                let source = player + Vec3::new(-16.0 + i + 4.0 * x, 8.0, -12.0 + i + 2.0 * z);

                let dest = player + Vec3::new(4.0 + i + 2.0 * x, 0.5, 4.0 + i + z);

                MagicArrow::new(source, dest, 3, None, now)
            })
            .collect();

        batch
    }
}

pub fn billboards(
    env_type: i32,
    day: u32,
    player: Vec3,
    detail: bool,
    rng: &mut Lcg,
) -> Vec<Billboard> {
    if !detail {
        return Vec::new();
    }

    let tile = super::world::tile_of(player.x, player.z);

    let mut out = Vec::new();

    if env_type == 0 {
        let r = rng.next_mod(7) as f32;

        let texture = if matches!(tile.0, 29 | 30) && tile.1 == 22 {
            2
        } else {
            95
        };

        let mut b = Billboard::new(texture, 20000, Vec3::splat(0.2), 0.0, 1, 80);

        b.position = player + Vec3::new(2.0 - r, 1.0 + 0.2 * r, 2.0 - r);

        b.origin = b.position;

        b.phase = 1.0 + 0.1 * r;

        b.amp_h = 7.0 + 0.5 * rng.next_mod(7) as f32;

        let motion = rng.next_mod(7);

        b.motion = Motion::from_field(motion % 3 + 3);

        b.rise = 1.0 + 0.1 * motion as f32;

        b.pass = render::billboard::BillboardPass::Additive;

        b.set_color(0xAAAAAAAA);

        out.push(b);
    }

    if matches!(day, 3 | 13) && !super::world::is_special_tile(tile) {
        let r = rng.next_mod(7);

        for (texture, size) in [(56, 0.2), (60, 0.07)] {
            let mut b = Billboard::new(texture, 20000, Vec3::splat(size), 0.0, 1, 80);

            b.position = player + Vec3::new(2.0 - r as f32, 1.5 + 0.2 * r as f32, 2.0 - r as f32);

            b.origin = b.position;

            b.phase = 1.5 + 0.1 * r as f32;

            b.amp_h = 5.0 + 0.5 * r as f32;

            b.rise = 0.2 + 0.05 * r as f32;

            b.motion = Motion::from_field(r % 3 + 6);

            b.pass = render::billboard::BillboardPass::Additive;

            b.set_color(0xFFFFFF00);

            out.push(b);
        }
    }

    out
}

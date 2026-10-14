use glam::Vec3;

pub fn enabled(kind: u32) -> bool {
    matches!(kind, 1549..=1551)
}

pub fn position(anchor: Vec3, yaw: f32, now: u32) -> Vec3 {
    let offset = ((now % 20_000) as f32 * std::f32::consts::PI / 10_000.0).sin() * 3.0;

    let along_z = yaw > -0.01 && yaw < 0.01 || yaw > 3.13 && yaw < 3.15 || yaw > 6.27 && yaw < 6.29;

    anchor
        + if along_z {
            Vec3::Z * offset
        } else {
            Vec3::X * offset
        }
}

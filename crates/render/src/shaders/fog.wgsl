fn distance_fog_visibility(view_depth: f32, end: f32, inverse_range: f32) -> f32 {
    let progress = clamp(1.0 - (end - view_depth) * inverse_range, 0.0, 1.0);
    let eased = progress * progress;
    return 1.0 - eased * eased * (3.0 - 2.0 * eased);
}

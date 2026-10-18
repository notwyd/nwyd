@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;
@group(0) @binding(2) var preservation_mask: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}

fn luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

fn neighbor(pixel: vec2<i32>, limit: vec2<i32>) -> f32 {
    return luma(textureLoad(frame, clamp(pixel, vec2<i32>(0), limit), 0).rgb);
}

fn preservation(pixel: vec2<i32>) -> f32 {
    let limit = vec2<i32>(textureDimensions(preservation_mask)) - vec2<i32>(1);
    var weight = textureLoad(preservation_mask, clamp(pixel, vec2<i32>(0), limit), 0).r;

    if any(limit > vec2<i32>(0)) && weight < 1.0 {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                weight = max(weight, textureLoad(preservation_mask,
                    clamp(pixel + vec2<i32>(x, y), vec2<i32>(0), limit), 0).r);
            }
        }
    }

    return clamp(weight, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let size = textureDimensions(frame);
    let limit = vec2<i32>(size) - vec2<i32>(1);
    let pixel = clamp(vec2<i32>(position.xy), vec2<i32>(0), limit);
    let center = textureLoad(frame, pixel, 0);
    let middle = luma(center.rgb);
    let nw = neighbor(pixel + vec2<i32>(-1, -1), limit);
    let ne = neighbor(pixel + vec2<i32>(1, -1), limit);
    let sw = neighbor(pixel + vec2<i32>(-1, 1), limit);
    let se = neighbor(pixel + vec2<i32>(1, 1), limit);
    let darkest = min(middle, min(min(nw, ne), min(sw, se)));
    let brightest = max(middle, max(max(nw, ne), max(sw, se)));

    if brightest - darkest < max(0.0625, brightest * 0.125) {
        return center;
    }

    let weight = preservation(pixel);

    if weight >= 1.0 {
        return center;
    }

    let edge = vec2<f32>(sw + se - nw - ne, nw + sw - ne - se);
    let reduction = max((nw + ne + sw + se) * 0.03125, 0.0078125);
    let scale = 1.0 / (min(abs(edge.x), abs(edge.y)) + reduction);
    let direction = clamp(edge * scale, vec2<f32>(-4.0), vec2<f32>(4.0)) / vec2<f32>(size);
    let uv = (vec2<f32>(pixel) + vec2<f32>(0.5)) / vec2<f32>(size);
    let inner = 0.5 * (
        textureSampleLevel(frame, frame_sampler, uv - direction / 6.0, 0.0).rgb +
        textureSampleLevel(frame, frame_sampler, uv + direction / 6.0, 0.0).rgb
    );
    let outer = inner * 0.5 + 0.25 * (
        textureSampleLevel(frame, frame_sampler, uv - direction * 0.5, 0.0).rgb +
        textureSampleLevel(frame, frame_sampler, uv + direction * 0.5, 0.0).rgb
    );
    let outer_luma = luma(outer);
    let filtered = select(outer, inner, outer_luma < darkest || outer_luma > brightest);

    return vec4<f32>(mix(center.rgb, filtered, 0.75 * (1.0 - weight)), center.a);
}

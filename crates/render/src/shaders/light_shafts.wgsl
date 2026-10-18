struct Frame {
    inverse_view_projection: mat4x4<f32>,
    viewport_jitter: vec4<f32>,
};

struct Volume {
    inverse_world: mat4x4<f32>,
    color_density: vec4<f32>,
    source_width: vec4<f32>,
    uv_scale_offset: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var scene_depth: texture_depth_2d;
@group(1) @binding(0) var<uniform> volume: Volume;
@group(2) @binding(0) var mask: texture_2d<f32>;
@group(2) @binding(1) var mask_sampler: sampler;

@vertex
fn vs(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn scatter(pixel: vec4<f32>) -> vec4<f32> {
    let depth = textureLoad(scene_depth, vec2<i32>(pixel.xy), 0);
    let ndc = vec2<f32>(pixel.x / frame.viewport_jitter.x * 2.0 - 1.0,
        1.0 - pixel.y / frame.viewport_jitter.y * 2.0) - frame.viewport_jitter.zw;
    let near_h = vec4<f32>(ndc, 0.0, 1.0) * frame.inverse_view_projection;
    let surface_h = vec4<f32>(ndc, depth, 1.0) * frame.inverse_view_projection;
    if abs(near_h.w) < 0.000001 || abs(surface_h.w) < 0.000001 {
        return vec4<f32>(0.0);
    }
    let origin = near_h.xyz / near_h.w;
    let surface = surface_h.xyz / surface_h.w;
    let length = distance(origin, surface);
    if length < 0.000001 {
        return vec4<f32>(0.0);
    }
    let direction = (surface - origin) / length;
    let local_origin = (vec4<f32>(origin, 1.0) * volume.inverse_world).xyz;
    let local_direction = (vec4<f32>(direction, 0.0) * volume.inverse_world).xyz;
    var entry = 0.0;
    var exit = length;
    for (var axis = 0u; axis < 3u; axis += 1u) {
        if abs(local_direction[axis]) < 0.000001 {
            if abs(local_origin[axis]) >= 1.0 {
                return vec4<f32>(0.0);
            }
        } else {
            let a = (-1.0 - local_origin[axis]) / local_direction[axis];
            let b = (1.0 - local_origin[axis]) / local_direction[axis];
            entry = max(entry, min(a, b));
            exit = min(exit, max(a, b));
        }
    }
    if exit <= entry {
        return vec4<f32>(0.0);
    }
    let step = (exit - entry) / 24.0;
    var optical_depth = 0.0;
    for (var sample = 0u; sample < 24u; sample += 1u) {
        let t = entry + (f32(sample) + 0.5) * step;
        let point = local_origin + local_direction * t;
        let width = mix(vec2<f32>(1.0), volume.source_width.xy, point.z * 0.5 + 0.5);
        let cross_section = point.xy / width;
        let edge = smoothstep(0.0, 0.18, 1.0 - abs(cross_section.x))
            * smoothstep(0.0, 0.45, 1.0 - abs(cross_section.y))
            * smoothstep(0.0, 0.12, 1.0 - abs(point.z));
        let uv = (vec2<f32>(cross_section.x, point.z) * 0.5 + 0.5) * volume.uv_scale_offset.xy + volume.uv_scale_offset.zw;
        let texel = textureSampleLevel(mask, mask_sampler, uv, 0.0).rgb;
        let intensity = dot(texel, vec3<f32>(0.2126, 0.7152, 0.0722));
        optical_depth += intensity * edge * step;
    }
    let opacity = 1.0 - exp(-optical_depth * volume.color_density.w);
    return vec4<f32>(volume.color_density.rgb * opacity, opacity);
}

@fragment
fn fs(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    return scatter(pixel);
}

struct ReactiveOutput {
    @location(0) color: vec4<f32>,
    @location(1) reactive: f32,
};

@fragment
fn fs_reactive(@builtin(position) pixel: vec4<f32>) -> ReactiveOutput {
    var result: ReactiveOutput;
    result.color = scatter(pixel);
    result.reactive = result.color.a;
    return result;
}

struct Uniforms {
    viewport: vec2<f32>,
    alpha_reference: f32,

    opaque: f32,

    select_texture_rgb: f32,

    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> frame: Uniforms;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs(
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
) -> VsOut {
    var out: VsOut;

    let p = pos + vec2<f32>(0.5, 0.5);
    out.clip = vec4<f32>(
        p.x * 2.0 / frame.viewport.x - 1.0,
        1.0 - p.y * 2.0 / frame.viewport.y,
        0.0,
        1.0,
    );

    out.color = color.zyxw;
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var t = textureSample(tex, samp, in.uv);

    t.a = mix(t.a, 1.0, frame.opaque);

    let modulated = t * in.color;
    let selected = vec4<f32>(t.rgb, t.a * in.color.a);
    let c = mix(modulated, selected, frame.select_texture_rgb);
    
    if (c.a < frame.alpha_reference) {
        discard;
    }
    return c;
}

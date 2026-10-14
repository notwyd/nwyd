struct Frame {
    world: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    light0_color: vec4<f32>,
    light1_color: vec4<f32>,
    light0_direction: vec4<f32>,
    light1_direction: vec4<f32>,
    material_emissive: vec4<f32>,
    material_ambient: vec4<f32>,
    global_ambient: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
};

struct Object {
    world: mat4x4<f32>,
    
    flags: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var tex: texture_2d_array<f32>;
@group(2) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) diffuse: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    let world_pos = vec4<f32>(pos, 1.0) * object.world;
    out.clip = (world_pos * frame.view) * frame.projection;
    
    out.diffuse = color.zyxw;
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {

    let texel = textureSample(tex, samp, in.uv, i32(object.flags.y));

    let stored = mix(texel.a, 1.0, object.flags.z);

    let rgb = texel.rgb * in.diffuse.rgb;
    
    let a = mix(stored * in.diffuse.a, stored, object.flags.x);
    return vec4<f32>(rgb, a);
}

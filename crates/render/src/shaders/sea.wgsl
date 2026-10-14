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
    point_lights: array<PointLight, 8>,
};

struct Object {
    world: mat4x4<f32>,
    material: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var base_tex: texture_2d<f32>;
@group(2) @binding(1) var base_samp: sampler;
@group(2) @binding(2) var detail_tex: texture_2d<f32>;
@group(2) @binding(3) var detail_samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) diffuse: vec4<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) uv1: vec2<f32>,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    
    let world_pos = vec4<f32>(pos, 1.0) * object.world;
    out.clip = (world_pos * frame.view) * frame.projection;

    let vertex_color = color.zyxw;
    
    let n = vec3<f32>(0.0, 0.0, 0.0);
    let n_dot_l0 = max(dot(n, frame.light0_direction.xyz), 0.0);
    let n_dot_l1 = max(dot(n, frame.light1_direction.xyz), 0.0);

    let points = point_light_terms(frame.point_lights, world_pos.xyz, n, vertex_color.rgb);
    let lit = vec3<f32>(object.material.y)
        + object.material.x * (frame.global_ambient.rgb + points[1])
        + frame.light0_color.rgb * vertex_color.rgb * n_dot_l0
        + frame.light1_color.rgb * vertex_color.rgb * n_dot_l1
        + points[0];
    out.diffuse = vec4<f32>(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), vertex_color.a);
    out.uv0 = uv0;
    out.uv1 = uv1;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    
    let base = textureSample(base_tex, base_samp, in.uv0);
    let detail = textureSample(detail_tex, detail_samp, in.uv1);

    let stage0 = vec4<f32>(base.rgb * in.diffuse.rgb, base.a);
    if (object.material.w < 0.5) { return stage0; }
    
    let rgb = clamp(detail.rgb * stage0.rgb * object.material.z, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, detail.a);
}

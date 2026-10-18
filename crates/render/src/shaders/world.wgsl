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
    ground_wave: vec4<f32>,
};

struct Draw {
    base_layer: u32,
    detail_layer: u32,
    stage1_enabled: f32,
    scroll: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var tiles: texture_2d_array<f32>;
@group(1) @binding(1) var base_sampler: sampler;
@group(1) @binding(2) var detail_sampler: sampler;
@group(1) @binding(3) var detail_tiles: texture_2d_array<f32>;
@group(2) @binding(0) var<uniform> draw: Draw;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) diffuse: vec4<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) uv1: vec2<f32>,
    
    @location(3) view_depth: f32,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv0: vec2<f32>,
    @location(4) uv1: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    
    var displaced = pos;

    let delta = frame.ground_wave.xy - pos.xz;
    displaced.y += cos(delta.x * delta.y / 10.0 + frame.ground_wave.z) * frame.ground_wave.w;
    let world_pos = vec4<f32>(displaced, 1.0) * frame.world;
    let view_pos = world_pos * frame.view;
    out.clip = view_pos * frame.projection;

    let vertex_color = color.zyxw;

    let world_normal = (vec4<f32>(normal, 0.0) * frame.normal_matrix).xyz;
    let len = length(world_normal);
    var n = vec3<f32>(0.0, 0.0, 0.0);
    if (len > 0.0) {
        n = world_normal / len;
    }

    let n_dot_l0 = max(dot(n, frame.light0_direction.xyz), 0.0);
    let n_dot_l1 = max(dot(n, frame.light1_direction.xyz), 0.0);

    let points = point_light_terms(frame.point_lights, world_pos.xyz, n, vertex_color.rgb);
    let lit = frame.material_emissive.rgb
        + frame.material_ambient.rgb * (frame.global_ambient.rgb + points[1])
        + frame.light0_color.rgb * vertex_color.rgb * n_dot_l0
        + frame.light1_color.rgb * vertex_color.rgb * n_dot_l1
        + points[0];

    out.diffuse = vec4<f32>(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), vertex_color.a);

    out.view_depth = view_pos.z;
    out.uv0 = uv0;
    out.uv1 = uv1;
    if (draw.scroll >= 1.0) { out.uv0.x += draw.scroll - 1.0; }
    if (draw.scroll <= -1.0) { out.uv1.y += -draw.scroll - 1.0; }
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {

    var color = vec4<f32>(in.diffuse.rgb,1.0);
    if (draw.stage1_enabled >= 0.0) {
        let base = textureSample(tiles, base_sampler, in.uv0, draw.base_layer);
        
        color = vec4<f32>(base.rgb * in.diffuse.rgb,1.0);
        
        if (draw.stage1_enabled > 0.0) {
            let detail = textureSample(detail_tiles, detail_sampler, in.uv1, draw.detail_layer);
            color = vec4<f32>(clamp(color.rgb * detail.rgb * draw.stage1_enabled,
                vec3<f32>(0.0),vec3<f32>(1.0)),1.0);
        }
    }

    let visibility = distance_fog_visibility(in.view_depth, frame.fog_params.y, frame.fog_params.z);
    let fogged = mix(frame.fog_color.rgb, color.rgb, visibility);
    color = vec4<f32>(mix(color.rgb, fogged, frame.fog_params.w), color.a);
    return color;
}

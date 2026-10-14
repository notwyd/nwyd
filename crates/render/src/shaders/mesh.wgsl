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
    normal_matrix: mat4x4<f32>,
    material_diffuse: vec4<f32>,
    material_ambient: vec4<f32>,
    material_emissive: vec4<f32>,
    
    flags: vec4<f32>,
    
    alpha: vec4<f32>,
    uv0: vec4<f32>,
    uv1: vec4<f32>,
    refine: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var tex: texture_2d<f32>;
@group(2) @binding(1) var samp: sampler;
@group(3) @binding(0) var tex1: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) diffuse: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fog: f32,
    @location(3) uv1: vec2<f32>,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    
    let world_pos = vec4<f32>(pos, 1.0) * object.world;
    let view_pos = world_pos * frame.view;
    out.clip = view_pos * frame.projection;

    let vertex_color = color.zyxw;

    let diffuse_material = mix(object.material_diffuse, vertex_color, object.flags.w);

    let world_normal = (vec4<f32>(normal, 0.0) * object.normal_matrix).xyz;
    let len = length(world_normal);
    var n = vec3<f32>(0.0, 0.0, 0.0);
    if (len > 0.0) {
        n = world_normal / len;
    }

    let n_dot_l0 = max(dot(n, frame.light0_direction.xyz), 0.0);
    let n_dot_l1 = max(dot(n, frame.light1_direction.xyz), 0.0);

    let points = point_light_terms(frame.point_lights, world_pos.xyz, n, diffuse_material.rgb);
    let lit = object.material_emissive.rgb
        + object.material_ambient.rgb * (frame.global_ambient.rgb + points[1])
        + frame.light0_color.rgb * diffuse_material.rgb * n_dot_l0
        + frame.light1_color.rgb * diffuse_material.rgb * n_dot_l1
        + points[0];

    let rgb = mix(diffuse_material.rgb, clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), object.flags.x);
    out.diffuse = vec4<f32>(rgb, diffuse_material.a);

    out.fog = clamp((frame.fog_params.y - view_pos.z) * frame.fog_params.z, 0.0, 1.0);
    out.uv = uv * object.uv0.xy + object.uv0.zw;
    out.uv1 = uv * object.uv1.xy + object.uv1.zw;
    if (object.refine.w > 0.5) {
        
        let view_normal = (vec4<f32>(n, 0.0) * frame.view).xyz;
        let incident = view_pos.xyz / max(length(view_pos.xyz), 0.000001);
        let reflected = reflect(incident, view_normal);
        out.uv1 = reflected.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    }
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var texel = vec4<f32>(1.0);
    if (object.flags.z >= 0.0) {
        texel = textureSample(tex, samp, in.uv);
    }
    
    texel.a = mix(texel.a, 1.0, object.alpha.y);

    let modulated = texel.rgb * in.diffuse.rgb;
    let added = clamp(texel.rgb + in.diffuse.rgb - vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));
    var stage0 = modulated;
    if (object.flags.z > 1.5) {
        stage0 = added;
    } else if (object.flags.z > 0.5) {
        stage0 = texel.rgb;
    }
    if (object.refine.x > 1.5) {
        stage0 = clamp(stage0 * 2.0, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    var color = vec4<f32>(stage0, texel.a * (1.0 - object.refine.z));
    if (object.flags.z < 0.0) {
        color = vec4<f32>(in.diffuse.rgb, in.diffuse.a * (1.0 - object.refine.z));
    }
    if (object.flags.z >= 0.0 && object.alpha.z > 0.5) {
        let overlay = textureSample(tex1, samp, in.uv1 + vec2<f32>(object.alpha.w)).rgb;
        if (object.alpha.z > 7.5) {
            let d = clamp(4.0 * dot(texel.rgb - vec3<f32>(0.5), in.diffuse.rgb - vec3<f32>(0.5)), 0.0, 1.0);
            color = vec4<f32>(clamp(4.0 * vec3<f32>(d) * overlay, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (object.alpha.z > 6.5) {
            
            let current = clamp(color.rgb + vec3<f32>(8.0 / 255.0), vec3<f32>(0.0), vec3<f32>(1.0));
            color = vec4<f32>(mix(current, overlay, object.refine.y), color.a);
        } else if (object.alpha.z > 5.5) {
            color = vec4<f32>(clamp(texel.rgb + overlay - vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (object.alpha.z > 4.5) {
            let factor = vec3<f32>(1.0, 0.549019608, 0.470588235);
            let d = clamp(4.0 * dot(texel.rgb - vec3<f32>(0.5), factor - vec3<f32>(0.5)), 0.0, 1.0);
            color = vec4<f32>(vec3<f32>(d) * overlay, color.a);
        } else if (object.alpha.z > 3.5) {
            color = vec4<f32>(clamp(color.rgb + color.a * overlay, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (object.alpha.z > 2.5) {
            color = vec4<f32>(clamp(2.0 * color.rgb * overlay, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (object.alpha.z > 1.5) {
            color = vec4<f32>(color.rgb + overlay * (vec3<f32>(1.0) - color.rgb), color.a);
        } else {
            color = vec4<f32>(color.rgb * overlay, color.a);
        }
    }

    if (color.a < object.alpha.x) {
        discard;
    }

    let fogged = mix(frame.fog_color.rgb, color.rgb, in.fog);
    color = vec4<f32>(mix(color.rgb, fogged, frame.fog_params.w * object.flags.y), color.a);
    return color;
}

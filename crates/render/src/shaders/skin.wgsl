struct Frame {
    projection: mat4x4<f32>,
    view_inverse: mat4x4<f32>,
    light_direction: vec4<f32>,
    fog_color: vec4<f32>,
    
    scroll: vec4<f32>,
};

struct Object {
    material_diffuse: vec4<f32>,
    material_ambient_emissive: vec4<f32>,
    
    fog_params: vec4<f32>,
    
    flags: vec4<f32>,
    
    stage: vec4<f32>,
    
    glow: vec4<f32>,
};

struct Palette {
    bones: array<mat4x4<f32>, 40>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var tex: texture_2d<f32>;
@group(2) @binding(1) var samp: sampler;
@group(2) @binding(2) var glow_tex: texture_2d<f32>;
@group(2) @binding(3) var glow_samp: sampler;
@group(3) @binding(0) var<uniform> palette: Palette;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) diffuse: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) view_depth: f32,
    
    @location(3) glow_uv: vec2<f32>,
};

const TINT_FACTOR = vec3<f32>(1.0, 0.549019608, 0.470588235);

fn unit(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if (len > 0.0) {
        return v / len;
    }
    return vec3<f32>(0.0, 0.0, 0.0);
}

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) bone: vec4<u32>,
    @location(4) weight: vec4<f32>,
) -> VsOut {

    let last = u32(object.flags.z) - 1u;
    var view_pos = vec3<f32>(0.0, 0.0, 0.0);
    var view_nrm = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        let m = palette.bones[min(bone[i], last)];

        view_pos = view_pos + (vec4<f32>(pos, 1.0) * m).xyz * weight[i];
        view_nrm = view_nrm + (vec4<f32>(normal, 0.0) * m).xyz * weight[i];
    }

    var out: VsOut;
    out.clip = vec4<f32>(view_pos, 1.0) * frame.projection;

    let view_n = unit(view_nrm);
    let world_n = unit((vec4<f32>(view_n, 0.0) * frame.view_inverse).xyz);
    
    let n_dot_l = max(dot(world_n, frame.light_direction.xyz), 0.0);

    let lit = object.material_diffuse * n_dot_l + object.material_ambient_emissive;
    let unlit = object.material_ambient_emissive + object.material_diffuse;
    
    out.diffuse = min(mix(lit, unlit, object.flags.x), vec4<f32>(1.0));

    out.view_depth = view_pos.z;
    out.uv = uv;

    out.glow_uv = vec2<f32>(view_n.x + frame.scroll.x, view_pos.y + frame.scroll.x);
    return out;
}

fn skin_fog_visibility(view_depth: f32) -> f32 {
    var end = object.fog_params.y;
    var inverse_range = object.fog_params.z;
    if (frame.scroll.y > 0.0 && frame.scroll.y < end && inverse_range > 0.0) {
        let start = end - 1.0 / inverse_range;
        end = frame.scroll.y;
        inverse_range = 1.0 / max(end - start, 0.00001);
    }
    return min(max(distance_fog_visibility(view_depth, end, inverse_range), object.fog_params.w), object.fog_params.x);
}

@fragment
fn fs_untextured(in: VsOut) -> @location(0) vec4<f32> {
    if (in.diffuse.a<object.flags.y) {discard;}
    let visibility = skin_fog_visibility(in.view_depth);
    return vec4<f32>(mix(frame.fog_color.rgb,in.diffuse.rgb,visibility),in.diffuse.a);
}

@fragment
fn fs_highlight(in: VsOut) -> @location(0) vec4<f32> {
    let alpha=mix(textureSample(tex,samp,in.uv).a,1.0,object.flags.w);
    if (alpha<object.flags.y) {discard;}
    return vec4<f32>(object.material_diffuse.rgb,alpha*object.material_diffuse.a);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var texel = textureSample(tex, samp, in.uv);
    texel.a = mix(texel.a, 1.0, object.flags.w);

    let alpha = texel.a * mix(1.0, in.diffuse.a, object.stage.x);
    let mode = u32(object.glow.x);
    let scale = 1.0 + object.glow.y;

    var stage0 = clamp(texel.rgb * in.diffuse.rgb * scale, vec3<f32>(0.0), vec3<f32>(1.0));
    var rgb = stage0;
    if (mode != 0u) {
        
        let guv = mix(in.glow_uv, in.uv, object.glow.z);
        let sheet = textureSample(glow_tex, glow_samp, guv);
        if (mode == 4u) {
            
            let d = 4.0 * dot(texel.rgb - vec3<f32>(0.5), TINT_FACTOR - vec3<f32>(0.5));
            stage0 = clamp(vec3<f32>(d), vec3<f32>(0.0), vec3<f32>(1.0));
            rgb = stage0 * sheet.rgb;
        } else if (mode == 5u) {

            rgb = texel.rgb + sheet.rgb - vec3<f32>(0.5);
        } else if (mode == 1u) {
            rgb = stage0 * sheet.rgb;
        } else if (mode == 2u) {
            rgb = stage0 + sheet.rgb - stage0 * sheet.rgb;
        } else {
            
            rgb = stage0 + alpha * sheet.rgb;
        }
        rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    var color = vec4<f32>(rgb, alpha);
    if (color.a < object.flags.y) {
        discard;
    }

    let visibility = skin_fog_visibility(in.view_depth);
    return vec4<f32>(mix(frame.fog_color.rgb, color.rgb, visibility), color.a);
}

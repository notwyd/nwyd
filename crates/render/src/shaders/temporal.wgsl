override temporal_coverage: u32 = 0u;
struct TemporalCamera {
    reprojection: mat4x4<f32>,
    jitter_scale: vec4<f32>,
    options: vec4<f32>,
};
@group(4) @binding(0) var<uniform> temporal: TemporalCamera;
struct TemporalOutput {
    @location(0) color: vec4<f32>,
    @location(1) motion: vec2<f32>,
    @location(2) bias: f32,
};
fn temporal_output(in: VsOut, color: vec4<f32>) -> TemporalOutput {
    var result: TemporalOutput;
    result.color = color;
    let current = in.temporal_current;
    let previous = in.temporal_previous;
    
    let valid = current.w > 0.00001 && previous.w > 0.00001;
    result.motion = vec2<f32>(0.0);
    if valid {
        result.motion = clamp((previous.xy/previous.w-current.xy/current.w)*temporal.jitter_scale.zw,
            vec2<f32>(-65000.0),vec2<f32>(65000.0));
    }
    result.bias = max(max(in.temporal_bias,temporal.options.x),select(1.0,0.0,valid));
    if temporal_coverage != 0u {

        var coverage = clamp(color.a,0.0,1.0);
        if temporal_coverage == 2u {
            coverage *= clamp(max(color.r,max(color.g,color.b)),0.0,1.0);
        }
        result.bias = coverage;
    }
    return result;
}

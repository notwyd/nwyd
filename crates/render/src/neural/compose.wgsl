@group(0) @binding(0) var native_color: texture_2d<f32>;
@group(0) @binding(1) var neural_color: texture_2d<f32>;
@group(0) @binding(2) var reactive_mask: texture_2d<f32>;

@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(positions[index], 0.0, 1.0);
}

@fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy);
    let native = textureLoad(native_color, pixel, 0);
    let neural = textureLoad(neural_color, pixel, 0);
    let reactive = clamp(textureLoad(reactive_mask, pixel, 0).r, 0.0, 1.0);
    return vec4(mix(neural.rgb, native.rgb, reactive), native.a);
}

@group(0) @binding(0) var depth: texture_depth_2d;

@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(positions[index], 0.0, 1.0);
}

@fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) f32 {
    return textureLoad(depth, vec2<i32>(position.xy), 0);
}

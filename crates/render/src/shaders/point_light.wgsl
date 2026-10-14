struct PointLight {
    
    position: vec4<f32>,
    
    diffuse: vec4<f32>,
    
    attenuation: vec4<f32>,
};

fn point_light_terms(
    lights: array<PointLight, 8>,
    world_pos: vec3<f32>,
    n: vec3<f32>,
    diffuse_material: vec3<f32>,
) -> array<vec3<f32>, 2> {
    var diffuse = vec3<f32>(0.0);
    var ambient = vec3<f32>(0.0);
    var slots = lights;
    for (var i = 0; i < 8; i = i + 1) {
        let l = slots[i];
        if (l.attenuation.w == 0.0) {
            continue;
        }
        let to_light = l.position.xyz - world_pos;
        let dist = length(to_light);
        if (dist > l.position.w) {
            continue;
        }
        let atten = 1.0 / (l.attenuation.x + l.attenuation.y * dist
            + l.attenuation.z * dist * dist);
        var n_dot_l = 0.0;
        if (dist > 0.0) {
            n_dot_l = max(dot(n, to_light / dist), 0.0);
        }
        diffuse = diffuse + l.diffuse.rgb * diffuse_material * n_dot_l * atten;
        ambient = ambient + l.diffuse.rgb * atten;
    }
    return array<vec3<f32>, 2>(diffuse, ambient);
}

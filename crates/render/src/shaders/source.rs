pub(crate) fn shader(source: &str, temporal: bool, reactive: bool) -> String {
    shader_with_history(
        source,
        temporal,
        "out.clip * temporal.reprojection",
        if reactive { "1.0" } else { "0.0" },
    )
}

pub(crate) fn shader_with_history(
    source: &str,
    temporal: bool,
    previous_clip: &str,
    bias: &str,
) -> String {
    if !temporal {
        return source.to_owned();
    }

    assert_eq!(
        source.matches("return out;").count(),
        1,
        "temporal vertex epilogue must be unique"
    );

    let mut source = source
        .replace(
            "struct VsOut {",
            "struct VsOut {\n    @location(10) temporal_current: vec4<f32>,\n    @location(11) temporal_previous: vec4<f32>,\n    @location(12) @interpolate(flat) temporal_bias: f32,",
        );

    source = source.replace(
        "return out;",
        &format!(
            r#"
    out.temporal_current = out.clip;
    out.temporal_previous = {previous_clip};
    out.temporal_bias = {bias};
    out.clip = vec4<f32>(out.clip.xy + temporal.jitter_scale.xy * out.clip.w, out.clip.zw);
    return out;"#
        ),
    );

    for entry in ["fs_untextured", "fs_highlight", "fs"] {
        let signature = format!("@fragment\nfn {entry}(in: VsOut) -> @location(0) vec4<f32>");

        if source.contains(&signature) {
            source = source.replace(
                &signature,
                &format!("fn {entry}_color(in: VsOut) -> vec4<f32>"),
            );

            source
                .push_str(
                    &format!(
                        "\n@fragment fn {entry}(in: VsOut) -> TemporalOutput {{\n    return temporal_output(in, {entry}_color(in));\n}}\n"
                    ),
                );
        }
    }

    source = biased_samples(&source);

    source.push_str(CAMERA_WGSL);

    source
}

pub(crate) fn biased_samples(source: &str) -> String {
    let mut result = String::with_capacity(source.len());

    let mut remaining = source;

    while let Some(start) = remaining.find("textureSample(") {
        result.push_str(&remaining[..start]);

        result.push_str("textureSampleBias(");

        remaining = &remaining[start + "textureSample(".len()..];

        let mut nesting = 0;

        let end = remaining
            .char_indices()
            .find_map(|(index, ch)| match ch {
                '(' => {
                    nesting += 1;

                    None
                }
                ')' if nesting == 0 => Some(index),
                ')' => {
                    nesting -= 1;

                    None
                }
                _ => None,
            })
            .expect("balanced textureSample arguments");

        result.push_str(&remaining[..end]);

        result.push_str(", temporal.options.y)");

        remaining = &remaining[end + 1..];
    }

    result.push_str(remaining);

    result
}

const CAMERA_WGSL: &str = include_str!("temporal.wgsl");

use super::kind::Shader;

const SPRITE_WGSL: &str = include_str!("sprite.wgsl");

const WORLD_WGSL: &str = include_str!("world.wgsl");

const POINT_LIGHT_WGSL: &str = include_str!("point_light.wgsl");

const SKIN_WGSL: &str = include_str!("skin.wgsl");

const MESH_WGSL: &str = include_str!("mesh.wgsl");

const SEA_WGSL: &str = include_str!("sea.wgsl");

const BILLBOARD_WGSL: &str = include_str!("billboard.wgsl");

const BLIT_WGSL: &str = include_str!("blit.wgsl");

fn lit_shader(body: &str) -> String {
    format!("{POINT_LIGHT_WGSL}{body}")
}

fn skin(temporal: bool) -> String {
    if temporal {
        let source = SKIN_WGSL
            .replace(
                "bones: array<mat4x4<f32>, 40>,",
                "bones: array<mat4x4<f32>, 40>,\n previous_bones: array<mat4x4<f32>, 40>,\n previous_projection: mat4x4<f32>,\n history: vec4<f32>,",
            );

        let source = source.replace(
            "var view_pos = vec3<f32>(0.0, 0.0, 0.0);",
            "var previous_view_pos = vec3<f32>(0.0);\n var view_pos = vec3<f32>(0.0, 0.0, 0.0);",
        );

        let source = source
            .replace(
                "let m = palette.bones[min(bone[i], last)];",
                "let m = palette.bones[min(bone[i], last)];\n previous_view_pos += (vec4<f32>(pos,1.0) * palette.previous_bones[min(bone[i],last)]).xyz * weight[i];",
            );

        shader_with_history(
            &source,
            true,
            "vec4<f32>(previous_view_pos,1.0) * palette.previous_projection",
            "palette.history.x",
        )
    } else {
        SKIN_WGSL.to_owned()
    }
}

fn mesh(temporal: bool) -> String {
    let source = lit_shader(MESH_WGSL);

    if temporal {
        let source = source.replace(
            "refine: vec4<f32>,",
            "refine: vec4<f32>,\n previous_clip: mat4x4<f32>,\n history: vec4<f32>,",
        );

        shader_with_history(
            &source,
            true,
            "vec4<f32>(pos,1.0) * object.previous_clip",
            "object.history.x",
        )
    } else {
        source
    }
}

pub fn assemble(kind: Shader) -> String {
    match kind {
        Shader::Sprite => SPRITE_WGSL.to_owned(),
        Shader::CubeSprite => {
            SPRITE_WGSL
                .replace("texture_2d<f32>", "texture_cube<f32>")
                .replace(
                    "textureSample(tex, samp, in.uv)",
                    "textureSample(tex, samp, select(vec3<f32>(in.uv, 0.0), vec3<f32>(0.0, 0.0, 1.0), all(in.uv == vec2<f32>(0.0))))",
                )
        }
        Shader::Blit => BLIT_WGSL.to_owned(),
        Shader::Terrain | Shader::TemporalTerrain => {
            shader(
                &lit_shader(WORLD_WGSL),
                matches!(kind, Shader::TemporalTerrain),
                false,
            )
        }
        Shader::Mesh | Shader::TemporalMesh => mesh(matches!(kind, Shader::TemporalMesh)),
        Shader::Skin | Shader::TemporalSkin => skin(matches!(kind, Shader::TemporalSkin)),
        Shader::Sea | Shader::TemporalSea => {
            shader(&lit_shader(SEA_WGSL), matches!(kind, Shader::TemporalSea), true)
        }
        Shader::Billboard | Shader::TemporalBillboard => {
            shader(BILLBOARD_WGSL, matches!(kind, Shader::TemporalBillboard), true)
        }
    }
}

#[cfg(test)]

mod tests {

    use super::*;
    use wgpu::naga;

    #[test]

    fn every_shader_variant_parses_and_validates() {
        for kind in super::super::SHADERS {
            let source = assemble(kind);

            let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|error| {
                panic!(
                    "shader {}: {}",
                    kind as usize,
                    error.emit_to_string(&source)
                );
            });

            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|error| {
                panic!(
                    "shader {}: {}",
                    kind as usize,
                    error.emit_to_string(&source)
                );
            });

            assert!(
                module.entry_points.iter().any(|entry| {
                    entry.stage == naga::ShaderStage::Vertex && entry.name == "vs"
                })
            );

            let fragments: Vec<_> = module
                .entry_points
                .iter()
                .filter(|entry| entry.stage == naga::ShaderStage::Fragment)
                .collect();

            assert!(fragments.iter().any(|entry| entry.name == "fs"));

            assert_eq!(
                fragments.len(),
                if matches!(kind, Shader::Skin | Shader::TemporalSkin) {
                    3
                } else {
                    1
                }
            );
        }
    }
}

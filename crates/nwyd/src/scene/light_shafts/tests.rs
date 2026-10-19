use super::*;
use formats::mesh::StaticVertex;
use formats::mesh::msa::{StaticMesh, Subset};
use formats::texture::texture_list::FormatFlag;
use formats::texture::{MipLevel, PixelFormat, Texture};
use render::cull::Aabb;
use render::mesh::MeshPass;

fn mesh() -> StaticMesh {
    let mut vertices = Vec::new();

    let mut indices = Vec::new();

    for angle in [-0.6_f32, 0.0, 0.6] {
        let base = vertices.len() as u16;

        for (end, side) in [(0, -1.0), (0, 1.0), (1, -1.0), (1, 1.0)] {
            let width = if end == 0 { 2.0 } else { 0.75 };

            vertices.push(StaticVertex {
                position: [
                    side * width * angle.cos(),
                    side * width * angle.sin(),
                    end as f32 * 8.0,
                ],
                uv: [if side < 0.0 { 0.1 } else { 0.9 }, 0.05 + end as f32 * 0.9],
                normal: [0.0; 3],
                color: 0,
            });
        }

        indices.extend([base, base + 1, base + 3, base + 3, base + 2, base]);
    }

    StaticMesh {
        fvf: 0x142,
        subsets: vec![Subset {
            material_id: 0,
            start_triangle: 0,
            prim_count: 6,
            min_vertex_index: 0,
            num_vertices: 12,
        }],
        texture_fields: vec![b"beam".to_vec()],
        indices,
        vertices,
        has_normals: false,
        has_uvs: true,
        has_colors: true,
    }
}

fn resource() -> MeshResource {
    MeshResource {
        id: GLOW_MESH_ID as u32,
        path: "synthetic-beam".into(),
        mesh: mesh(),
        bounds: Aabb::EMPTY,
        alloc: 0,
        subset_slots: vec![Some(0)],
        subset_textures: vec![Some(0)],
        subset_zero_pass: MeshPass::Additive,
        subset_passes: vec![MeshPass::Additive],
        vertex_color: true,
    }
}

fn texture() -> BoundTexture {
    BoundTexture {
        pending: false,
        effect_table: true,
        slot: 0,
        path: "synthetic-beam".into(),
        flag: FormatFlag::None_,
        texture: Texture {
            width: 1,
            height: 1,
            format: PixelFormat::Rgba8Unorm,
            mips: vec![MipLevel {
                width: 1,
                height: 1,
                offset: 0,
                len: 4,
            }],
            data: vec![255; 4],
            source_len: 4,
        }
        .into(),
        mip_policy_override: None,
        prepared: None,
    }
}

fn glow() -> Glow {
    Glow {
        mesh_id: GLOW_MESH_ID,
        position: Vec3::new(12.0, 3.0, 7.0),
        yaw: 0.7,
        scale: (3.0, 2.0),
        colour: 0xff88_7777,
        pulse: true,
        period_ms: 10_000,
        sector: -1,
        uv_scroll: super::super::effect_mesh::UvScroll::None,
        uv_offset: [0.0; 2],
    }
}

#[test]
fn each_authored_quad_keeps_its_direction_width_and_source_uv_in_a_volume() {
    let resource = resource();

    let glow = glow();

    let shafts = from_glow(&resource, &[texture()], &glow, 0).unwrap();

    assert_eq!(shafts.len(), 3);

    for (shaft, vertices) in shafts.iter().zip(resource.mesh.vertices.chunks_exact(4)) {
        let inverse = glam::Mat4::from_cols_array(&shaft.world.0).inverse();

        for vertex in vertices {
            let position = glow
                .world()
                .transform_coord(Vec3::from_array(vertex.position));

            let local = inverse.transform_point3(position);

            assert!(local.abs().max_element() < 1.0001, "{local:?}");

            let v = (local.z * 0.5 + 0.5) * shaft.uv_scale_offset[1] + shaft.uv_scale_offset[3];

            assert!((v - vertex.uv[1]).abs() < 0.0001);

            let width = if vertex.uv[1] < 0.5 {
                1.0
            } else {
                shaft.source_width[0]
            };

            assert!((local.x.abs() - width).abs() < 0.0001);
        }

        let lower = shaft.world.transform_coord(Vec3::new(0.0, 0.0, -1.0));

        let upper = shaft.world.transform_coord(Vec3::new(0.0, 0.0, 1.0));

        assert!((lower.distance(upper) - 24.0).abs() < 0.0001);
        assert_eq!(shaft.texture, 0);
        assert!((shaft.uv_scale_offset[0] - 0.8).abs() < 0.0001);
        assert!((shaft.source_width[0] - 0.375).abs() < 0.0001);
    }
}

#[test]
fn pulse_changes_intensity_without_moving_the_volume() {
    let resource = resource();

    let textures = [texture()];

    let glow = glow();

    let bright = from_glow(&resource, &textures, &glow, 2_500).unwrap();

    let dim = from_glow(&resource, &textures, &glow, 7_500).unwrap();

    assert_eq!(bright.len(), dim.len());

    for (bright, dim) in bright.iter().zip(&dim) {
        assert_eq!(bright.world, dim.world);
        assert!(bright.density > dim.density);
        assert_eq!(bright.color, dim.color);
    }
}

#[test]
fn unrelated_scenes_and_glows_keep_their_existing_rendering() {
    assert!(enabled(super::super::world::SCENE_SELECT_CHAR, "Character"));
    assert!(!enabled(super::super::world::SCENE_FIELD, "Character"));
    assert!(!enabled(super::super::world::SCENE_LOGIN, "Character"));
    assert!(!enabled(
        super::super::world::SCENE_SELECT_CHAR,
        "Field1616"
    ));

    let mut glow = glow();

    glow.mesh_id += 1;

    assert!(from_glow(&resource(), &[texture()], &glow, 0).is_none());
}

#[test]
fn missing_texture_or_invalid_geometry_keeps_the_mesh_fallback() {
    let mut resource = resource();

    let mut textures = [texture()];

    assert!(from_glow(&resource, &[], &glow(), 0).is_none());

    textures[0].pending = true;

    assert!(from_glow(&resource, &textures, &glow(), 0).is_none());

    textures[0].pending = false;

    resource.subset_textures[0] = None;

    assert!(from_glow(&resource, &textures, &glow(), 0).is_none());

    resource.subset_textures[0] = Some(0);

    resource.mesh.indices[0] = 100;

    assert!(from_glow(&resource, &textures, &glow(), 0).is_none());

    resource.mesh = mesh();

    resource.mesh.vertices[0].position[0] = f32::NAN;

    assert!(from_glow(&resource, &textures, &glow(), 0).is_none());

    resource.mesh = mesh();

    for vertex in &mut resource.mesh.vertices {
        vertex.position[2] = 0.0;
    }

    assert!(from_glow(&resource, &textures, &glow(), 0).is_none());

    resource.mesh = mesh();

    let mut collapsed = glow();

    collapsed.scale.0 = 0.0;

    assert!(from_glow(&resource, &textures, &collapsed, 0).is_none());
}

#[test]
fn unused_vertices_do_not_shift_the_volumes() {
    let mut resource = resource();

    let expected = from_glow(&resource, &[texture()], &glow(), 0).unwrap();

    resource.mesh.vertices.push(StaticVertex {
        position: [10_000.0; 3],
        normal: [0.0; 3],
        uv: [0.5, 0.0],
        color: 0,
    });

    assert_eq!(
        from_glow(&resource, &[texture()], &glow(), 0),
        Some(expected)
    );
}

#[test]
fn a_malformed_second_quad_keeps_all_original_sheets() {
    let mut resource = resource();

    resource.mesh.indices[6] = 100;

    assert!(from_glow(&resource, &[texture()], &glow(), 0).is_none());
}

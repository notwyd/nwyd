use super::fog_tests::field_world;
use crate::scene::field::ClickMarker;
use crate::scene::objects::{BoundTexture, MeshResource};
use formats::mesh::StaticVertex;
use formats::mesh::msa::{StaticMesh, Subset};
use formats::texture::texture_list::FormatFlag;
use formats::texture::{MipLevel, PixelFormat, Texture};
use glam::Vec3;
use render::camera::Matrix4;
use render::cull::{Aabb, Frustum};
use render::mesh::{Cull, MeshPass};
use render::preset::{ColorOp, Preset};
use render::quirk::VendorClass;
use std::sync::Arc;

fn marker_world() -> super::World {
    let mut world = field_world();

    for (index, id) in [316, 317].into_iter().enumerate() {
        let mesh = StaticMesh {
            fvf: 0x142,
            subsets: vec![Subset {
                material_id: 0,
                start_triangle: 0,
                prim_count: 1,
                min_vertex_index: 0,
                num_vertices: 3,
            }],
            texture_fields: vec![format!("marker{index}").into_bytes()],
            indices: vec![0, 1, 2],
            vertices: [[-0.1, 0.0, 0.0], [0.1, 0.0, 0.0], [0.0, 0.0, 2.0]]
                .map(|position| StaticVertex {
                    position,
                    normal: [0.0; 3],
                    uv: [0.5; 2],
                    color: 0xFFFF_FFFF,
                })
                .to_vec(),
            has_normals: false,
            has_uvs: true,
            has_colors: true,
        };
        let alloc = world.objects.arena.push(&mesh);
        world.objects.meshes.push(MeshResource {
            id,
            path: format!("Effect/marker{index}.msa"),
            mesh,
            bounds: Aabb {
                min: [-0.1, 0.0, 0.0],
                max: [0.1, 0.0, 2.0],
            },
            alloc,
            subset_slots: vec![Some(index)],
            subset_textures: vec![Some(index)],
            subset_zero_pass: MeshPass::Cutout,
            subset_passes: vec![MeshPass::Cutout],
            vertex_color: true,
        });
        world.objects.by_id_meshes.insert(id, index);
        world.objects.textures.push(BoundTexture {
            pending: false,
            effect_table: true,
            slot: index,
            path: format!("Effect/marker{index}.wys"),
            flag: FormatFlag::Alpha,
            texture: Arc::new(Texture {
                width: 1,
                height: 1,
                format: PixelFormat::Rgba8Unorm,
                source_len: 4,
                mips: vec![MipLevel {
                    width: 1,
                    height: 1,
                    offset: 0,
                    len: 4,
                }],
                data: vec![240, 160, 80, 170],
            }),
            mip_policy_override: None,
            prepared: None,
        });
    }

    world
}

#[test]
fn destination_meshes_reach_world_draws_with_upright_bounds_and_additive_materials() {
    let world = marker_world();
    let marker = ClickMarker {
        at: Vec3::new(12.0, 3.3, 41.0),
        tile: (12, 41),
        walking: true,
    };
    let tints = [
        [17.0 / 255.0, 17.0 / 255.0, 136.0 / 255.0, 1.0],
        [1.0, 0.0, 0.0, 1.0],
    ];

    for now_ms in [0, 125, 250, 500, 750] {
        let meshes = marker.draws(now_ms).meshes;
        let frame = world.object_frame_with(
            VendorClass::DEFAULT,
            &[],
            &[],
            now_ms,
            &meshes,
            &Frustum::ADMITS_EVERYTHING,
        );
        let expected = Matrix4::multiply(
            &Matrix4::rotation_yaw_pitch_roll(
                ClickMarker::spin(now_ms),
                -std::f32::consts::FRAC_PI_2,
                std::f32::consts::FRAC_PI_2,
            ),
            &Matrix4::translation(marker.at.x, marker.at.y, marker.at.z),
        );

        assert_eq!(frame.draws.len(), 2);

        for (index, draw) in frame.draws.iter().enumerate() {
            let uniform = &frame.objects[draw.object as usize];
            assert_eq!(uniform.world, expected.shader_constants());
            assert_eq!(uniform.material_diffuse, tints[index]);
            assert_eq!(uniform.flags, [0.0, 0.0, ColorOp::Modulate.uniform(), 0.0]);
            assert_eq!(uniform.alpha, [0.0; 4]);
            assert_eq!(uniform.uv0, [1.0, 1.0, 0.0, 0.0]);
            assert_eq!(draw.pass, MeshPass::Additive);
            assert_eq!(draw.cull, Cull::None);
            assert_eq!(draw.texture, index);
            assert_eq!(draw.stage1, None);
            assert_eq!(draw.subset.index_count, 3);

            let depth = draw
                .pass
                .depth_state(&Preset::LIT_3D, render::DEPTH_FORMAT)
                .unwrap();
            assert_eq!(depth.depth_write_enabled, Some(false));
            assert_eq!(depth.depth_compare, Some(wgpu::CompareFunction::LessEqual));
        }
    }

    let frustum = Frustum::from_clip(&Matrix4::IDENTITY);

    for (at, count) in [
        (Vec3::new(0.0, -2.5, 0.5), 2),
        (Vec3::new(4.0, -2.5, 0.5), 0),
    ] {
        for now_ms in [0, 250, 500, 750] {
            let meshes = ClickMarker { at, ..marker }.draws(now_ms).meshes;
            let frame =
                world.object_frame_with(VendorClass::DEFAULT, &[], &[], now_ms, &meshes, &frustum);
            assert_eq!(frame.draws.len(), count, "{at:?} at {now_ms} ms");
        }
    }
}

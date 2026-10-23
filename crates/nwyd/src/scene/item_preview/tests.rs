use super::*;
use formats::data::item_list::{self, ItemList, field};
use formats::net::account::Effect;
use render::cull::Aabb;

fn items() -> ItemList {
    let mut body = vec![0; item_list::BODY_LEN];

    for (id, mesh, mask, tier) in [
        (1usize, 80i16, 64u32, 3i16),
        (2, 5, 64, 0),
        (3, 0, 0, 0),
        (10, 0, 1, 0),
        (4152, 80, 0, 0),
    ] {
        let entry =
            &mut body[id * formats::data::ITEM_RECORD_LEN..][..formats::data::ITEM_RECORD_LEN];
        entry[field::MESH..field::MESH + 2].copy_from_slice(&mesh.to_le_bytes());
        entry[field::EQUIP_MASK..field::EQUIP_MASK + 4].copy_from_slice(&mask.to_le_bytes());
        entry[field::TIER..field::TIER + 2].copy_from_slice(&tier.to_le_bytes());
        if id == 10 {
            entry[field::EFFECT..field::EFFECT + 2].copy_from_slice(
                &i16::from(formats::data::item_effect::effect::CLASS).to_le_bytes(),
            );
            entry[field::EFFECT + 2..field::EFFECT + 4].copy_from_slice(&1i16.to_le_bytes());
        }
    }

    ItemList::from_plain(body, formats::text::Codepage::Windows1252)
}

fn registries() -> Arc<Registries> {
    use formats::mesh::registry;
    use formats::texture::texture_list;

    Arc::new(Registries {
        resource_names: formats::scene::rc::parse(&[]).unwrap(),
        entries: Vec::new(),
        valid: registry::parse_valid_index(&vec![0; registry::VALID_INDEX_LEN]).unwrap(),
        sounds: formats::audio::ani_sound::AniSound::default(),
        textures: texture_list::parse(&[]).unwrap(),
        effect_textures: texture_list::parse(&[]).unwrap(),
        items: items(),
        list: Default::default(),
        arena_visual: Vec::new(),
    })
}

#[test]
fn resolves_static_shapes_and_full_item_refinement() {
    let table = items();
    let mut item = Item {
        index: 1,
        ..Item::default()
    };
    assert_eq!(recipe(item, &table).unwrap().mesh, 80);
    item.index = 2;
    assert_eq!(recipe(item, &table).unwrap().mesh, 5);
    item.index = 1;
    item.effects[0] = Effect { id: 43, value: 230 };
    let refined = recipe(item, &table).unwrap();
    assert_eq!(refined.refine, 10);
    assert_eq!(refined.style, 9);
    item.effects[0] = Effect { id: 116, value: 9 };
    let tinted = recipe(item, &table).unwrap();
    assert_eq!(tinted.refine, 9);
    assert_eq!(tinted.style, 116);
    assert_eq!(tinted.refinement(false).unwrap().texture, 283);
    assert!(recipe(Item::default(), &table).is_none());
    assert!(
        recipe(
            Item {
                index: 3,
                ..Item::default()
            },
            &table
        )
        .is_none()
    );
    assert!(
        recipe(
            Item {
                index: u16::MAX,
                ..Item::default()
            },
            &table
        )
        .is_none()
    );
}

#[test]
fn static_skins_keep_the_original_items_refinement() {
    let table = items();
    let item = Item {
        index: 1,
        appearance: 2,
        effects: [
            Effect { id: 43, value: 230 },
            Effect::default(),
            Effect::default(),
        ],
        ..Item::default()
    };
    let skin = recipe(item, &table).unwrap();
    assert_eq!(skin.mesh, 5);
    assert_eq!(skin.refine, 10);
    assert_eq!(skin.style, 9);
}

#[test]
fn missing_costume_models_never_fall_back_to_the_ground_mesh() {
    let game = GameDir::from_files("preview", []).unwrap();
    let mut preview = Preview::new(registries());
    let item = Item {
        index: 4152,
        ..Item::default()
    };
    let rect = [10.0, 10.0, 120.0, 120.0];
    assert!(!preview.prepare(&game, item, rect, (800, 600), 0));
    assert!(preview.resources.missing_by_id.is_empty());
    assert!(preview.loaded.is_empty());
    assert_eq!(preview.actors.len(), 1);
    let reads = game.reads();
    assert!(!preview.prepare(&game, item, rect, (800, 600), 1));
    assert_eq!(game.reads(), reads);
    assert!(preview.frame.draws.is_empty());
}

#[test]
fn rotation_keeps_asymmetric_off_center_mesh_inside_the_preview() {
    let bounds = Aabb {
        min: [-40.0, 6.0, -2.0],
        max: [2.0, 10.0, 26.0],
    };
    let rect = [200.0, 30.0, 160.0, 120.0];
    let frame = uniforms((800, 600));
    let projection = Matrix4(frame.projection).transpose();
    let center = (Vec3::from(bounds.min) + Vec3::from(bounds.max)) * 0.5;

    for step in 0..120 {
        let angle = step as f32 * std::f32::consts::TAU / 120.0;
        let world = placement(bounds, rect, angle).unwrap();
        let up =
            (world.transform_coord(center + Vec3::Z) - world.transform_coord(center)).normalize();
        assert!(up.distance(Vec3::Y) < 0.0001);
        let drawn = bounds.transformed(&world);
        assert!(drawn.min[0] >= 200.0 && drawn.max[0] <= 360.0);
        assert!(drawn.min[1] >= -150.0 && drawn.max[1] <= -30.0);
        assert!(
            world
                .transform_coord(center)
                .distance(Vec3::new(280.0, -90.0, 0.0))
                < 0.001
        );
        let clip = drawn.transformed(&projection);
        assert!(clip.min[2] > 0.0 && clip.max[2] < 1.0);
        let pixel = projection.transform_coord(world.transform_coord(center));
        assert!((pixel.x - -0.3).abs() < 0.0001);
        assert!((pixel.y - 0.7).abs() < 0.0001);
    }
}

#[test]
fn invalid_geometry_cannot_produce_a_model_transform() {
    let valid = Aabb {
        min: [-1.0; 3],
        max: [1.0; 3],
    };
    assert!(placement(Aabb::EMPTY, [0.0, 0.0, 100.0, 100.0], 0.0).is_none());
    assert!(
        placement(
            Aabb {
                min: [0.0; 3],
                max: [0.0; 3]
            },
            [0.0, 0.0, 100.0, 100.0],
            0.0
        )
        .is_none()
    );
    assert!(placement(valid, [0.0, 0.0, 0.0, 100.0], 0.0).is_none());
    assert!(placement(valid, [0.0, f32::NAN, 100.0, 100.0], 0.0).is_none());
    assert!(placement(valid, [0.0, 0.0, 100.0, 100.0], f32::INFINITY).is_none());
}

#[test]
fn missing_models_fall_back_without_repeated_loads_or_unbounded_cache_growth() {
    let game = GameDir::from_files("preview", []).unwrap();
    let mut preview = Preview::new(registries());
    let item = Item {
        index: 1,
        ..Item::default()
    };
    let rect = [10.0, 10.0, 120.0, 120.0];

    for now in 0..10 {
        assert!(!preview.prepare(&game, item, rect, (800, 600), now));
    }

    assert_eq!(preview.loaded.len(), 1);
    assert_eq!(preview.resources.missing_by_id, [80]);
    let identity = preview.identity;
    preview
        .loaded
        .extend((100..132).map(|mesh| ((mesh, 0, 0, 0), false)));
    let next = Item {
        index: 2,
        ..Item::default()
    };
    assert!(!preview.prepare(&game, next, rect, (800, 600), 20));
    assert_ne!(preview.identity, identity);
    assert_eq!(preview.loaded.len(), 1);
    assert_eq!(preview.resources.missing_by_id, [5]);
    assert!(preview.frame.draws.is_empty());
    assert!(preview.range.is_empty());
}

#[test]
fn prepared_mesh_rotates_and_does_not_survive_a_missing_hover() {
    use super::super::objects::{BoundTexture, MeshResource};
    use formats::mesh::{
        StaticVertex,
        msa::{StaticMesh, Subset},
    };
    use formats::texture::{MipLevel, PixelFormat, Texture, texture_list::FormatFlag};

    let game = GameDir::from_files("preview", []).unwrap();
    let mut preview = Preview::new(registries());
    let mesh = StaticMesh {
        fvf: 0x112,
        subsets: vec![Subset {
            material_id: 0,
            start_triangle: 0,
            prim_count: 1,
            min_vertex_index: 0,
            num_vertices: 3,
        }],
        texture_fields: vec![b"preview".to_vec()],
        indices: vec![0, 1, 2],
        vertices: [[-2.0, 0.0, -1.0], [2.0, 0.0, -1.0], [0.0, 0.0, 3.0]]
            .map(|position| StaticVertex {
                position,
                normal: [0.0, -1.0, 0.0],
                uv: [0.5; 2],
                color: u32::MAX,
            })
            .to_vec(),
        has_normals: true,
        has_uvs: true,
        has_colors: false,
    };
    let alloc = preview.resources.arena.push(&mesh);
    preview.resources.meshes.push(MeshResource {
        id: 80,
        path: "mesh\\preview.msa".into(),
        mesh,
        bounds: Aabb {
            min: [-2.0, 0.0, -1.0],
            max: [2.0, 0.0, 3.0],
        },
        alloc,
        subset_slots: vec![Some(0)],
        subset_textures: vec![Some(0)],
        subset_zero_pass: MeshPass::Opaque,
        subset_passes: vec![MeshPass::Opaque],
        vertex_color: false,
    });
    preview.resources.by_id_meshes.insert(80, 0);
    preview.resources.textures.push(BoundTexture {
        pending: false,
        effect_table: false,
        slot: 0,
        path: "mesh\\preview.wys".into(),
        flag: FormatFlag::None_,
        texture: Arc::new(Texture {
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
        }),
        mip_policy_override: None,
        prepared: None,
    });
    let item = Item {
        index: 1,
        ..Item::default()
    };
    let rect = [10.0, 10.0, 120.0, 120.0];
    assert!(preview.prepare(&game, item, rect, (800, 600), 0));
    assert_eq!(preview.range, 0..1);
    assert_eq!(preview.frame.draws[0].subset.index_count, 3);
    let first = preview.frame.objects[0].world;
    preview.resources.textures[0].flag = FormatFlag::Alpha;
    assert!(preview.prepare(&game, item, rect, (800, 600), 3000));
    assert_ne!(preview.frame.objects[0].world, first);
    assert_eq!(preview.frame.draws[0].pass, MeshPass::Cutout);
    assert_eq!(
        preview.frame.objects[preview.frame.draws[0].object as usize].alpha[0],
        170.0 / 255.0
    );
    assert_eq!(preview.resources.meshes.len(), 1);
    preview.loaded.insert((80, 0, 0, 3), false);
    assert!(!preview.prepare(&game, item, rect, (800, 600), 3500));
    assert!(preview.frame.draws.is_empty());
    assert!(!preview.prepare(&game, Item::default(), rect, (800, 600), 4000));
    assert!(preview.frame.objects.is_empty());
    assert!(preview.frame.draws.is_empty());
    assert!(preview.range.is_empty());
}

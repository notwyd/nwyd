use super::*;
use crate::scene::actors::{ActorMeshPlacement, HeldDraw};
use crate::scene::drops::DropDraw;
use formats::audio::ani_sound::AniSound;
use formats::data::item_list;
use formats::mesh::registry;
use formats::text::Codepage;
use formats::texture::texture_list;
use render::cull::Frustum;
use render::quirk::VendorClass;

pub(super) fn triangle_mesh(subsets: usize) -> Vec<u8> {
    let mut bytes = Vec::new();

    for value in [0x112, 32, subsets as u32] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    for index in 0..subsets {
        for value in [index as u32, 0, 1, 0, 3] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }

    for index in 0..subsets {
        let name = format!("tex{index}");

        let mut field = [0; 11];

        field[..name.len()].copy_from_slice(name.as_bytes());

        bytes.extend_from_slice(&field);
    }

    bytes.extend_from_slice(&6_u32.to_le_bytes());

    for index in [0_u16, 1, 2] {
        bytes.extend_from_slice(&index.to_le_bytes());
    }

    bytes.extend_from_slice(&96_u32.to_le_bytes());

    for position in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for value in position.into_iter().chain([0.0, 0.0, 1.0, 0.0, 0.0]) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }

    bytes
}

pub(super) fn synthetic_scene(flags: &[FormatFlag], subsets: usize) -> ObjectScene {
    let mut records = vec![0; flags.len() * texture_list::LEGACY_RECORD_LEN];

    for (index, flag) in flags.iter().enumerate() {
        let path = format!(r"mesh\tex{index}.wys");

        let start = index * texture_list::LEGACY_RECORD_LEN;

        records[start..start + path.len()].copy_from_slice(path.as_bytes());

        records[start + 255] = flag.to_byte();
    }

    let registries = Arc::new(Registries {
        resource_names: formats::scene::rc::parse(&[]).unwrap(),
        entries: Vec::new(),
        valid: registry::parse_valid_index(&vec![0; registry::VALID_INDEX_LEN]).unwrap(),
        sounds: AniSound::default(),
        textures: texture_list::parse_legacy(&records).unwrap(),
        effect_textures: texture_list::parse(&[]).unwrap(),
        items: item_list::ItemList::from_plain(vec![0; item_list::BODY_LEN], Codepage::Windows1252),
        list: mesh_list::parse(br"100 mesh\sample.msa"),
        arena_visual: Vec::new(),
    });

    let game = GameDir::from_files(
        "synthetic-static-scene",
        [("mesh/sample.msa".into(), triangle_mesh(subsets))],
    )
    .unwrap();

    let mut scene = ObjectScene::empty(Arc::clone(&registries));

    let mesh = scene
        .load_mesh(
            &game,
            &registries.list,
            &registries.textures,
            None,
            100,
            true,
        )
        .unwrap()
        .unwrap();

    for slot in 0..flags.len() {
        scene
            .bind_slot(&game, &registries.textures, false, slot)
            .unwrap();
    }

    scene.by_id_meshes.insert(100, mesh);

    scene
}

pub(super) fn place(scene: &mut ObjectScene) {
    scene.instances.push(Instance {
        kind: 100,
        mesh: 0,
        position: Vec3::ZERO,
        yaw: 0.0,
    });

    scene.placements.push(Placement {
        position: Vec3::ZERO,
        world: Matrix4::IDENTITY,
        bounds: scene.meshes[0].bounds,
        fade: Default::default(),
    });
}

pub(super) fn frame(
    scene: &ObjectScene,
    held: &[HeldDraw],
    drops: &[DropDraw],
    effects: &[RotateLitDraw],
) -> Frame {
    scene.frame(
        Vec3::ZERO,
        VendorClass::DEFAULT,
        held,
        drops,
        &[],
        effects,
        0,
        false,
        false,
        &Frustum::ADMITS_EVERYTHING,
    )
}

fn assert_alpha(frame: &Frame, index: usize, pass: MeshPass, cutoff: f32, opaque: f32) {
    let draw = &frame.draws[index];

    let uniform = &frame.objects[draw.object as usize];

    assert_eq!(draw.pass, pass);
    assert_eq!(uniform.alpha[0], cutoff);
    assert_eq!(uniform.alpha[1], opaque);
    assert_eq!(draw.subset.index_count, 3);
}

#[test]
fn loaded_static_subsets_preserve_alpha_for_color_key_and_alpha_textures() {
    let mut scene = synthetic_scene(
        &[
            FormatFlag::ColorKey,
            FormatFlag::Alpha,
            FormatFlag::AlphaLower,
            FormatFlag::None_,
            FormatFlag::Other(b'?'),
        ],
        5,
    );

    place(&mut scene);

    let frame = frame(&scene, &[], &[], &[]);

    assert_eq!(frame.draws.len(), 5);
    assert_eq!(scene.meshes[0].subset_zero_pass, MeshPass::Cutout);

    for index in 0..3 {
        assert_alpha(&frame, index, MeshPass::Cutout, 170.0 / 255.0, 0.0);
        assert_eq!(frame.draws[index].texture, index);
    }

    for index in 3..5 {
        assert_alpha(&frame, index, MeshPass::Opaque, 0.0, 1.0);
        assert_eq!(frame.draws[index].texture, index);
    }
}

#[test]
fn biased_ground_drops_use_the_replacement_texture_alpha_mode() {
    let scene = synthetic_scene(
        &[
            FormatFlag::None_,
            FormatFlag::Alpha,
            FormatFlag::AlphaLower,
            FormatFlag::ColorKey,
            FormatFlag::ColorKey,
            FormatFlag::None_,
            FormatFlag::Alpha,
            FormatFlag::AlphaLower,
        ],
        4,
    );

    let drop = DropDraw {
        mesh_id: 100,
        world: Matrix4::IDENTITY,
        material: Material::STATIC_OBJECT,
        overlay: None,
        tex_bias: 4,
        flag_colour: None,
        gate_stage1_source: None,
    };

    let frame = frame(&scene, &[], &[drop], &[]);

    assert_eq!(frame.draws.len(), 4);

    for index in [0, 2, 3] {
        assert_alpha(&frame, index, MeshPass::Cutout, 170.0 / 255.0, 0.0);
    }

    assert_alpha(&frame, 1, MeshPass::Opaque, 0.0, 1.0);

    for (index, draw) in frame.draws.iter().enumerate() {
        assert_eq!(draw.texture, index + 4);
    }
}

#[test]
fn fading_color_key_scenery_disables_the_cutoff() {
    let mut scene = synthetic_scene(&[FormatFlag::ColorKey], 1);

    place(&mut scene);

    scene.placements[0].fade.opacity = 0.7;

    let frame = frame(&scene, &[], &[], &[]);

    assert_eq!(frame.draws.len(), 1);
    assert_alpha(&frame, 0, MeshPass::Blended, 0.0, 0.0);
    assert_eq!(frame.objects[0].refine[2], 1.0 - 0.7);
}

#[test]
fn blended_mesh_effects_keep_alpha_testing_disabled() {
    let scene = synthetic_scene(&[FormatFlag::ColorKey], 1);

    let effect = RotateLitDraw {
        mesh_id: 100,
        world: Matrix4::IDENTITY,
        material: Material::STATIC_OBJECT,
        fog: true,
    };

    let frame = frame(&scene, &[], &[], &[effect]);

    assert_eq!(frame.draws.len(), 1);
    assert_alpha(&frame, 0, MeshPass::Blended, 0.0, 0.0);
}

#[test]
fn color_key_weapon_shine_preserves_its_alpha_mask_without_a_cutoff() {
    let mut scene = synthetic_scene(&[FormatFlag::ColorKey], 1);

    scene.textures.push(BoundTexture {
        mip_policy_override: None,
        prepared: None,
        pending: false,
        effect_table: true,
        slot: 0,
        path: "synthetic-shine".into(),
        flag: FormatFlag::None_,
        texture: scene.textures[0].texture.clone(),
    });

    let weapon = HeldDraw {
        motion_key: (0, 0),
        mesh_id: 100,
        world: Matrix4::IDENTITY,
        placement: ActorMeshPlacement::Hand,
        silhouette: false,
        glow: Some(0),
        ordinary_glow: None,
    };

    let frame = frame(&scene, &[weapon], &[], &[]);

    assert_eq!(frame.draws.len(), 1);
    assert_alpha(&frame, 0, MeshPass::Opaque, 0.0, 0.0);
    assert_eq!(frame.draws[0].stage1, Some(1));
    assert_eq!(frame.draws[0].cull, Cull::None);
    assert_eq!(frame.objects[0].alpha[2], 4.0);
}

#[test]
fn window_beams_replace_authored_sheets_and_restore_draws_when_resources_are_unavailable() {
    let mut scene = synthetic_scene(&[FormatFlag::None_], 1);

    let mesh = &mut scene.meshes[0].mesh;

    mesh.vertices = (0..8)
        .map(|index| formats::mesh::StaticVertex {
            position: [
                if index & 1 == 0 { -1.0 } else { 1.0 },
                if index & 2 == 0 { -0.4 } else { 0.4 },
                if index & 4 == 0 { 0.0 } else { 4.0 },
            ],
            uv: [(index & 1) as f32, (index >> 2) as f32],
            normal: [0.0; 3],
            color: 0,
        })
        .collect();

    mesh.indices = vec![0, 1, 5, 5, 4, 0, 2, 3, 7, 7, 6, 2];

    scene.by_id_meshes.insert(506, 0);

    scene.textures[0].pending = false;

    let glow = super::super::lights::glow_of(&super::super::effect_mesh::EffectMesh::new(
        506,
        0xff88_7777,
        0.0,
        0,
        0,
    ))
    .unwrap();

    let frame = |scene: &ObjectScene, enabled: bool| {
        scene.frame(
            Vec3::ZERO,
            VendorClass::DEFAULT,
            &[],
            &[],
            &[glow],
            &[],
            0,
            false,
            enabled,
            &Frustum::ADMITS_EVERYTHING,
        )
    };

    let original = frame(&scene, false);

    assert_eq!(original.draws.len(), 1);
    assert!(original.light_shafts.is_empty());

    let replaced = frame(&scene, true);

    assert!(replaced.draws.is_empty());
    assert_eq!(replaced.light_shafts.len(), 2);

    let original_bounds = scene.meshes[0].bounds;

    for vertex in &mut scene.meshes[0].mesh.vertices {
        vertex.position[1] += 1.5;
    }

    let mut raised_bounds = render::cull::Aabb::EMPTY;

    for vertex in &scene.meshes[0].mesh.vertices {
        raised_bounds.grow(vertex.position);
    }

    scene.meshes[0].bounds = raised_bounds;

    let frustum = Frustum::from_clip(&Matrix4::IDENTITY);

    assert!(!frustum.admits(&raised_bounds));

    let visible_volume = scene.frame(
        Vec3::ZERO,
        VendorClass::DEFAULT,
        &[],
        &[],
        &[glow],
        &[],
        0,
        false,
        true,
        &frustum,
    );

    assert_eq!(visible_volume.light_shafts.len(), 2);

    scene.meshes[0].bounds = original_bounds;

    for vertex in &mut scene.meshes[0].mesh.vertices {
        vertex.position[1] -= 1.5;
    }

    scene.textures[0].pending = true;

    assert_eq!(frame(&scene, true), original);

    scene.textures[0].pending = false;

    for vertex in &mut scene.meshes[0].mesh.vertices {
        vertex.position[2] = 0.0;
    }

    assert_eq!(frame(&scene, true), original);
}

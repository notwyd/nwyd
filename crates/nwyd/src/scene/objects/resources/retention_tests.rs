use super::*;
use crate::scene::{actors::texture_loading::Loader, drops, placed};
use formats::audio::ani_sound::AniSound;
use formats::data::item_list;
use formats::mesh::registry;
use formats::net::account::Item;
use formats::text::Codepage;
use formats::texture::{MipLevel, PixelFormat, texture_list};
use web_time::{Duration, Instant};

fn scene() -> ObjectScene {
    ObjectScene::empty(Arc::new(Registries {
        resource_names: formats::scene::rc::parse(&[]).unwrap(),
        entries: Vec::new(),
        valid: registry::parse_valid_index(&vec![0; registry::VALID_INDEX_LEN]).unwrap(),
        sounds: AniSound::default(),
        textures: texture_list::parse(&[]).unwrap(),
        effect_textures: texture_list::parse(&[]).unwrap(),
        items: item_list::ItemList::from_plain(vec![0; item_list::BODY_LEN], Codepage::Windows1252),
        list: Default::default(),
        arena_visual: Vec::new(),
    }))
}

fn texture(slot: usize, effect_table: bool) -> BoundTexture {
    BoundTexture {
        pending: false,
        effect_table,
        slot,
        path: format!("synthetic-{slot}.wys"),
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
            data: vec![128, 96, 64, 255],
            source_len: 4,
        }
        .into(),
        mip_policy_override: None,
        prepared: None,
    }
}

fn mesh(scene: &mut ObjectScene, id: u32, textures: &[usize]) {
    let index = scene.meshes.len();
    scene.meshes.push(MeshResource {
        id,
        path: format!("mesh\\synthetic-{id}.msa"),
        mesh: StaticMesh {
            fvf: 0x112,
            subsets: Vec::new(),
            texture_fields: Vec::new(),
            indices: Vec::new(),
            vertices: Vec::new(),
            has_normals: true,
            has_uvs: true,
            has_colors: false,
        },
        bounds: render::cull::Aabb::EMPTY,
        alloc: 0,
        subset_slots: textures
            .iter()
            .map(|&i| Some(scene.textures[i].slot))
            .collect(),
        subset_textures: textures.iter().copied().map(Some).collect(),
        subset_zero_pass: MeshPass::Opaque,
        subset_passes: vec![MeshPass::Opaque; textures.len()],
        vertex_color: false,
    });
    scene.by_id_meshes.insert(id, index);
}

fn drop(id: u16, mesh: i32, bias: i32) -> drops::Drop {
    drops::Drop {
        entity: 15000,
        item: Item {
            index: id,
            ..Default::default()
        },
        cell: (0, 0),
        position: Vec3::ZERO,
        yaw: 0.0,
        mesh: Some(mesh),
        use_kind: 0,
        pose: drops::Pose::default(),
        tex_bias: bias,
        refine: 0,
        overlay: None,
        flag_colour: None,
        hovered: false,
        cannon: None,
    }
}

fn gate() -> placed::Gate {
    placed::Gate {
        entity: 15001,
        item_id: 459,
        item: Item {
            index: 459,
            ..Default::default()
        },
        cell: (0, 0),
        position: Vec3::ZERO,
        yaw: 0.0,
        direction: 0,
        table: placed::table(136),
        state: placed::state::CLOSED,
        since_ms: 0,
        swing: 0.0,
        in_box: true,
        hovered: false,
    }
}

fn maintain(
    scene: &mut ObjectScene,
    loader: &mut Loader,
    drops: &drops::Drops,
    gates: &placed::Gates,
    now: Instant,
) {
    let used = scene.retained_texture_indices(&[], drops, gates);
    loader.maintain(
        &mut scene.textures,
        &mut scene.texture_updates,
        used.into_iter(),
        now,
    );
}

#[test]
fn live_tower_textures_survive_eviction_and_retire_after_the_owner_leaves() {
    let mut scene = scene();
    scene.textures = vec![texture(231, false), texture(232, false)];
    mesh(&mut scene, 2723, &[0, 0, 1]);
    let mut drops = drops::Drops {
        items: vec![drop(3145, 2723, 0)],
    };
    let gates = placed::Gates::default();
    let game = GameDir::from_files("synthetic-tower-retention", []).unwrap();
    let mut loader = Loader::new(&game, false);
    let base = Instant::now();

    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(5),
    );
    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(130),
    );

    assert!(scene.textures.iter().all(|t| !t.pending));
    assert!(scene.texture_updates.is_empty());
    assert_eq!(scene.textures[0].texture.data, [128, 96, 64, 255]);
    assert_eq!(scene.textures[1].texture.data, [128, 96, 64, 255]);

    drops.items.clear();
    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(255),
    );

    assert!(scene.textures.iter().all(|t| t.pending));
    assert_eq!(scene.texture_updates, [0, 1]);
    assert_eq!(scene.textures[0].texture.data, [0; 4]);
    assert_eq!(scene.textures[1].texture.data, [0; 4]);
}

#[test]
fn both_gate_leaves_keep_their_textures_until_the_gate_is_removed() {
    let mut scene = scene();
    scene.textures = vec![texture(50, false), texture(51, false)];
    mesh(&mut scene, 136, &[0]);
    mesh(&mut scene, 137, &[1]);
    let drops = drops::Drops::default();
    let mut gates = placed::Gates {
        items: vec![gate()],
    };
    let game = GameDir::from_files("synthetic-gate-retention", []).unwrap();
    let mut loader = Loader::new(&game, false);
    let base = Instant::now();

    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(5),
    );
    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(130),
    );

    assert!(scene.textures.iter().all(|t| !t.pending));
    assert!(scene.texture_updates.is_empty());

    gates.items.clear();
    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(255),
    );

    assert!(scene.textures.iter().all(|t| t.pending));
    assert_eq!(scene.texture_updates, [0, 1]);
}

#[test]
fn biased_ground_items_retain_each_variant_in_the_correct_texture_table() {
    let mut scene = scene();
    scene.textures = vec![
        texture(100, false),
        texture(102, false),
        texture(110, false),
        texture(112, false),
        texture(102, true),
    ];
    mesh(&mut scene, 500, &[0, 2]);
    let drops = drops::Drops {
        items: vec![drop(1000, 500, 2)],
    };
    let gates = placed::Gates::default();
    let game = GameDir::from_files("synthetic-variant-retention", []).unwrap();
    let mut loader = Loader::new(&game, false);
    let base = Instant::now();

    assert_eq!(scene.retained_texture_indices(&[], &drops, &gates), [1, 3]);
    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(5),
    );
    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(130),
    );

    assert!(!scene.textures[1].pending);
    assert!(!scene.textures[3].pending);
    assert!(scene.textures[0].pending);
    assert!(scene.textures[2].pending);
    assert_eq!(scene.texture_updates, [0, 2]);
}

#[test]
fn negative_texture_bias_respects_namespaces_and_rejects_underflow() {
    let mut scene = scene();
    scene.textures = vec![texture(12, false), texture(4, true), texture(4, false)];
    mesh(&mut scene, 500, &[0]);
    let mut drops = drops::Drops {
        items: vec![drop(1000, 500, -8)],
    };
    let gates = placed::Gates::default();

    assert_eq!(scene.retained_texture_indices(&[], &drops, &gates), [2]);

    scene.meshes[0].path = "Effect\\synthetic.msa".into();
    assert_eq!(scene.retained_texture_indices(&[], &drops, &gates), [1]);

    drops.items[0].tex_bias = -13;
    assert!(
        scene
            .retained_texture_indices(&[], &drops, &gates)
            .is_empty()
    );

    drops.items[0].tex_bias = 100;
    assert!(
        scene
            .retained_texture_indices(&[], &drops, &gates)
            .is_empty()
    );
}

#[test]
fn held_mask_variants_retire_even_when_scenery_keeps_the_source_texture() {
    let mut scene = scene();
    scene.textures = vec![texture(100, false), texture(100, false)];
    scene.textures[1].mip_policy_override = Some(render::texture::MipPolicy::Blend);
    mesh(&mut scene, 777, &[0]);
    scene.held_mask_textures.insert(0, 1);
    scene.instances.push(Instance {
        kind: 777,
        mesh: 0,
        position: Vec3::ZERO,
        yaw: 0.0,
    });
    let drops = drops::Drops::default();
    let gates = placed::Gates::default();
    let game = GameDir::from_files("synthetic-held-retention", []).unwrap();
    let mut loader = Loader::new(&game, false);
    let base = Instant::now();

    for seconds in [5, 130] {
        let used = scene.retained_texture_indices(&[777], &drops, &gates);
        assert_eq!(used, [0, 1]);
        loader.maintain(
            &mut scene.textures,
            &mut scene.texture_updates,
            used.into_iter(),
            base + Duration::from_secs(seconds),
        );
    }

    assert!(scene.textures.iter().all(|t| !t.pending));
    assert!(scene.texture_updates.is_empty());

    maintain(
        &mut scene,
        &mut loader,
        &drops,
        &gates,
        base + Duration::from_secs(255),
    );

    assert!(!scene.textures[0].pending);
    assert!(scene.textures[1].pending);
    assert_eq!(scene.texture_updates, [1]);
}

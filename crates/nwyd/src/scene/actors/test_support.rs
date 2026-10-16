use super::*;
use formats::mesh::{SkinVertex, identity};
use formats::text::Codepage;
use formats::texture::texture_list::FormatFlag;
use formats::texture::{MipLevel, PixelFormat, Texture};
use formats::world::trn::TrnCell;

pub(super) fn empty_scene() -> ActorScene {
    let mut masks = vec![0; object_bin::PAYLOAD_LEN];

    masks.extend_from_slice(&object_bin::checksum(&masks).to_le_bytes());

    let game = GameDir::from_files("synthetic-actor-scene", [("object.bin".into(), masks)])
        .expect("in-memory game files");

    let registries = Registries {
        resource_names: formats::scene::rc::parse(&[]).unwrap(),
        entries: Vec::new(),
        valid: registry::parse_valid_index(&vec![0; registry::VALID_INDEX_LEN]).unwrap(),
        sounds: AniSound::default(),
        textures: texture_list::parse(&[]).unwrap(),
        effect_textures: texture_list::parse(&[]).unwrap(),
        items: item_list::ItemList::from_plain(vec![0; item_list::BODY_LEN], Codepage::Windows1252),
        list: Default::default(),
        arena_visual: Vec::new(),
    };

    let terrain = Terrain {
        name: "synthetic".into(),
        zone_x: 0,
        zone_y: 0,
        cells: vec![
            TrnCell {
                height: 0,
                uv_mode: 0,
                uv_mode2: 0,
                pad: 0,
                tile0: 0,
                tile1: 0,
                color: [255; 4],
            };
            formats::world::TRN_DIM * formats::world::TRN_DIM
        ],
    };

    build(
        &game,
        None,
        &terrain,
        &[],
        [1.0; 4],
        [1.0; 4],
        false,
        0,
        Arc::new(registries),
    )
    .expect("empty actor scene")
}

pub(super) fn add_model(scene: &mut ActorScene, model_id: u32, flags: &[FormatFlag]) -> usize {
    let entry = ModelEntry {
        model_id,
        num_anims: 1,
        num_parts: flags.len() as u32,
        name: format!("synthetic-model-{model_id}"),
    };

    let skeleton = bon::parse(&[255, 255, 255, 255, 1, 0, 0, 0]).unwrap();

    let clip = ani::AnimClip {
        num_frames: 1,
        num_bones: 2,
        matrices: vec![identity(); 2],
    };

    let animation =
        Arc::new(ModelAnimation::assemble(&entry, skeleton, 8, &[Slot::Loaded(&clip)]).unwrap());

    let mesh = Arc::new(SkinMesh {
        header: msh::MshHeader {
            magic: 0,
            attach_bone: 1,
            fvf: 0x1116,
            vertex_stride: 36,
            num_influences: 1,
            num_matrices: 1,
            num_vertices: 3,
            num_indices: 3,
        },
        bone_offsets: vec![identity()],
        bone_indices: vec![1],
        vertices: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .into_iter()
            .map(|position| SkinVertex {
                position,
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                bone_indices: [0; 4],
                bone_weights: [1.0, 0.0, 0.0, 0.0],
            })
            .collect(),
        indices: vec![0, 1, 2],
    });

    let mut parts = Vec::new();

    for (index, &flag) in flags.iter().enumerate() {
        let texture = scene.textures.len();

        scene.textures.push(BoundTexture {
            pending: false,
            effect_table: false,
            slot: texture,
            path: format!("synthetic-{texture}"),
            flag,
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
            },
        });

        parts.push(Part {
            index,
            file_name: "synthetic-triangle".into(),
            mesh_path: "synthetic-triangle".into(),
            mesh: Arc::clone(&mesh),
            alloc: scene.arena.push(&mesh),
            texture_name: format!("synthetic-{texture}"),
            texture: Some(texture),
        });
    }

    let index = scene.models.len();

    scene.models.push(Model {
        model_id,
        base_name: entry.name,
        animation,
        parts,
        slot: 0,
        sub_frame_ms: 80,
        missing_parts: Vec::new(),
    });

    index
}

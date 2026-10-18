use super::*;
use formats::texture::{PixelFormat, Texture};
use formats::world::env_texture_list;

fn wys(colors: &[u16]) -> Vec<u8> {
    let mut bytes = vec![0; 129];

    bytes[..4].copy_from_slice(b"WS10");

    for (offset, value) in [
        (5, 124u32),
        (13, 4),
        (17, 4),
        (29, colors.len() as u32),
        (81, 4),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    bytes[85] = b'2';

    for color in colors {
        bytes.extend(color.to_le_bytes());
        bytes.extend([0; 6]);
    }

    bytes
}

fn fixture() -> (GameDir, EnvTextureList, Vec<Texture>) {
    let files = [
        ("Env/generated.wys", wys(&[0xf800])),
        ("Env/partial.wys", wys(&[0xf800, 0x07e0])),
        ("Env/authored.wys", wys(&[0xf800, 0x07e0, 0x001f])),
    ];
    let originals = files
        .iter()
        .map(|(_, bytes)| formats::texture::decode(bytes).unwrap())
        .collect();
    let mut list = vec![0; env_texture_list::FILE_LEN];

    for (index, (path, _)) in files.iter().enumerate() {
        let start = (10 + index) * env_texture_list::ENTRY_LEN;

        list[start..start + path.len()].copy_from_slice(path.as_bytes());
        list[start + 0x1fe] = b'N';
    }

    let game = GameDir::from_files(
        "terrain-mipmap-regression",
        files.map(|(path, bytes)| (path.into(), bytes)),
    )
    .unwrap();

    (game, env_texture_list::parse(&list).unwrap(), originals)
}

#[test]
fn tile_loader_builds_only_missing_mips_and_does_not_reload_resident_tiles() {
    let (game, list, originals) = fixture();
    let mut tiles = Vec::new();

    assert!(
        load_ids(&game, &list, [12, 10, 11, 10].into_iter(), &mut tiles)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        tiles.iter().map(|tile| tile.id).collect::<Vec<_>>(),
        [10, 11, 12]
    );

    for tile in &tiles {
        assert_eq!(tile.texture.mips.len(), 3);
    }

    assert_eq!(tiles[0].texture.format, PixelFormat::Rgba8Unorm);
    assert_eq!(tiles[0].texture.level(2).unwrap(), [255, 0, 0, 255]);

    let partial = originals[1].clone().into_rgba8().unwrap();

    assert_eq!(tiles[1].texture.level(0), partial.level(0));
    assert_eq!(tiles[1].texture.level(1), partial.level(1));
    assert_eq!(tiles[1].texture.level(2).unwrap(), [0, 255, 0, 255]);
    assert_eq!(tiles[2].texture, originals[2]);

    let reads = game.reads();
    let pointers: Vec<_> = tiles
        .iter()
        .map(|tile| tile.texture.data.as_ptr())
        .collect();

    assert!(
        load_ids(&game, &list, [10, 11, 12].into_iter(), &mut tiles)
            .unwrap()
            .is_empty()
    );
    assert_eq!(game.reads(), reads);
    assert_eq!(
        tiles
            .iter()
            .map(|tile| tile.texture.data.as_ptr())
            .collect::<Vec<_>>(),
        pointers
    );
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn tile_cache_accepts_loaded_mixed_mip_chains_and_reuses_uploaded_layers() {
    use render::world::TileCache;
    use render::{Gpu, TerrainRenderer};

    let (game, list, _) = fixture();
    let mut tiles = Vec::new();
    let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).unwrap();
    let renderer = TerrainRenderer::new(
        &gpu,
        render::COLOR_FORMAT,
        wgpu::TextureFormat::Depth32Float,
    );
    let mut cache = TileCache::default();
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);

    load_ids(&game, &list, [10].into_iter(), &mut tiles).unwrap();

    let references: Vec<_> = tiles.iter().map(|tile| (tile.id, &tile.texture)).collect();

    assert!(cache.sync(&gpu, &renderer, &references));
    assert_eq!(cache.stats().uploaded_layers, 1);

    load_ids(&game, &list, [11, 12].into_iter(), &mut tiles).unwrap();

    let references: Vec<_> = tiles.iter().map(|tile| (tile.id, &tile.texture)).collect();

    assert!(cache.sync(&gpu, &renderer, &references));
    assert_eq!(cache.stats().uploaded_layers, 3);
    assert_eq!(cache.bound().unwrap().layers.len(), 3);

    let uploaded = cache.stats();

    assert!(cache.sync(&gpu, &renderer, &references));
    assert_eq!(cache.stats(), uploaded);

    gpu.queue.submit([]);

    assert!(pollster::block_on(validation.pop()).is_none());
}

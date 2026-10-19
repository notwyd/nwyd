use super::*;
use formats::texture::MipLevel;

fn rgba() -> Texture {
    Texture {
        width: 4,
        height: 2,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width: 4,
            height: 2,
            offset: 0,
            len: 32,
        }],
        data: [
            [0, 10, 20, 0],
            [100, 30, 40, 64],
            [200, 50, 60, 128],
            [255, 70, 80, 255],
            [20, 90, 100, 255],
            [120, 110, 120, 128],
            [220, 130, 140, 64],
            [235, 150, 160, 0],
        ]
        .concat(),
        source_len: 1000,
    }
}

#[test]
fn prepared_material_matches_existing_filtering_for_every_policy() {
    let source = rgba();

    for policy in [
        MipPolicy::Opaque,
        MipPolicy::Blend,
        MipPolicy::Cutout(0),
        MipPolicy::Cutout(64),
        MipPolicy::Cutout(170),
    ] {
        let prepared = PreparedMaterial::new(&source, policy);
        assert_eq!(prepared.policy(), policy);
        assert_eq!(prepared.texture(), with_mipmaps(&source, policy).as_ref());
        assert_eq!(prepared.upload_bytes(), 32 + 8 + 4);
    }

    assert_eq!(source, rgba());
}

#[test]
fn prepared_material_preserves_complete_authored_chains_including_padding() {
    let mut source = rgba();
    source.data.extend_from_slice(&[77; 4]);
    source
        .data
        .extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);
    source.data.extend_from_slice(&[88; 4]);
    source.data.extend_from_slice(&[91, 92, 93, 94]);
    source.data.extend_from_slice(&[99; 32]);
    source.mips.extend([
        MipLevel {
            width: 2,
            height: 1,
            offset: 36,
            len: 8,
        },
        MipLevel {
            width: 1,
            height: 1,
            offset: 48,
            len: 4,
        },
    ]);

    for policy in [MipPolicy::Opaque, MipPolicy::Blend, MipPolicy::Cutout(170)] {
        let prepared = PreparedMaterial::new(&source, policy);
        assert_eq!(prepared.texture(), &source);
        assert_eq!(prepared.upload_bytes(), 44);
    }
}

#[test]
fn prepared_material_expands_incomplete_compression_without_changing_source() {
    for (format, data) in [
        (
            PixelFormat::Bc1RgbaUnorm,
            vec![0xe0, 0x07, 0, 0, 0, 0, 0, 0],
        ),
        (
            PixelFormat::Bc2RgbaUnorm,
            [vec![0xff; 8], vec![0xe0, 0x07, 0, 0, 0, 0, 0, 0]].concat(),
        ),
    ] {
        let source = Texture {
            width: 4,
            height: 4,
            format,
            mips: vec![MipLevel {
                width: 4,
                height: 4,
                offset: 0,
                len: data.len(),
            }],
            source_len: 1234,
            data,
        };
        let prepared = PreparedMaterial::new(&source, MipPolicy::Cutout(170));
        assert_eq!(prepared.texture().format, PixelFormat::Rgba8Unorm);
        assert_eq!(prepared.texture().mips.len(), 3);
        assert_eq!(
            prepared.texture().level(2),
            Some([0, 255, 0, 255].as_slice())
        );
        assert_eq!(prepared.upload_bytes(), 64 + 16 + 4);
        assert_eq!(source.mips.len(), 1);
        assert_eq!(source.format, format);
    }
}

#[test]
fn upload_size_counts_texel_payloads_instead_of_container_or_padding_bytes() {
    let mut source = rgba();
    source.data.extend_from_slice(&[77; 100]);
    source.mips[0].offset = 8;
    source.mips[0].len = 64;
    assert_eq!(upload_size(&source), 32);

    for (format, block) in [
        (PixelFormat::Bc1RgbaUnorm, 8),
        (PixelFormat::Bc2RgbaUnorm, 16),
    ] {
        let source = Texture {
            width: 5,
            height: 7,
            format,
            mips: vec![
                MipLevel {
                    width: 5,
                    height: 7,
                    offset: 3,
                    len: 4 * block + 5,
                },
                MipLevel {
                    width: 2,
                    height: 3,
                    offset: 4 * block + 11,
                    len: block + 7,
                },
                MipLevel {
                    width: 1,
                    height: 1,
                    offset: 5 * block + 21,
                    len: block,
                },
            ],
            data: vec![0; 6 * block + 100],
            source_len: 10000,
        };
        assert_eq!(upload_size(&source), 6 * block);
    }
}

#[test]
fn upload_size_stops_at_invalid_ranges_and_never_overflows() {
    let mut source = rgba();
    source.mips.push(MipLevel {
        width: 2,
        height: 1,
        offset: source.data.len(),
        len: 8,
    });
    assert_eq!(upload_size(&source), 32);
    source.mips[0].offset = usize::MAX;
    assert_eq!(upload_size(&source), 0);
    source.mips[0].offset = 0;
    source.mips[0].len = 31;
    assert_eq!(upload_size(&source), 0);
    source.mips[0].len = 32;
    source.mips[0].width = 0;
    assert_eq!(upload_size(&source), 0);
    source.mips[0].width = u32::MAX;
    source.mips[0].height = u32::MAX;
    assert_eq!(upload_size(&source), 0);
}

#[test]
fn prepared_material_preserves_malformed_input_without_panicking() {
    let mut source = rgba();
    source.mips[0].offset = usize::MAX;
    let prepared = PreparedMaterial::new(&source, MipPolicy::Opaque);
    assert_eq!(prepared.texture(), &source);
    assert_eq!(prepared.upload_bytes(), 0);
}

#[test]
fn prepared_material_can_be_shared_immutably_between_worker_and_render_threads() {
    fn shareable<T: Send + Sync>() {}

    shareable::<PreparedMaterial>();
    let prepared = std::sync::Arc::new(PreparedMaterial::new(&rgba(), MipPolicy::Blend));
    let shared = prepared.clone();
    assert!(std::sync::Arc::ptr_eq(&prepared, &shared));
    assert!(std::ptr::eq(prepared.texture(), shared.texture()));
}

#[test]
fn prepared_material_reuses_shared_complete_authored_chains() {
    let source = Arc::new(with_mipmaps(&rgba(), MipPolicy::Opaque).into_owned());

    for policy in [MipPolicy::Opaque, MipPolicy::Blend, MipPolicy::Cutout(170)] {
        let prepared = PreparedMaterial::from_shared(source.clone(), policy);
        assert!(Arc::ptr_eq(&prepared.texture, &source));
        assert_eq!(prepared.texture(), source.as_ref());
        assert_eq!(prepared.policy(), policy);
        assert_eq!(prepared.upload_bytes(), 44);
    }
}

#[test]
fn prepared_material_reuses_shared_malformed_sources_without_copying_pixels() {
    let mut source = rgba();
    source.mips[0].offset = usize::MAX;
    let source = Arc::new(source);
    let prepared = PreparedMaterial::from_shared(source.clone(), MipPolicy::Blend);
    assert!(Arc::ptr_eq(&prepared.texture, &source));
    assert_eq!(prepared.texture(), source.as_ref());
    assert_eq!(prepared.upload_bytes(), 0);
}

#[test]
fn prepared_material_generates_a_separate_chain_without_mutating_shared_source() {
    let source = Arc::new(rgba());

    for policy in [MipPolicy::Opaque, MipPolicy::Blend, MipPolicy::Cutout(170)] {
        let prepared = PreparedMaterial::from_shared(source.clone(), policy);
        assert!(!Arc::ptr_eq(&prepared.texture, &source));
        assert_eq!(prepared.texture(), with_mipmaps(&source, policy).as_ref());
        assert_eq!(prepared.policy(), policy);
        assert_eq!(prepared.upload_bytes(), 44);
        assert_eq!(source.as_ref(), &rgba());
    }
}

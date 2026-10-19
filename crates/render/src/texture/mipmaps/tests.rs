use super::*;
use formats::texture::{MipLevel, PixelFormat};

fn rgba(width: u32, height: u32, data: Vec<u8>) -> Texture {
    Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        source_len: data.len(),
        mips: vec![MipLevel {
            width,
            height,
            offset: 0,
            len: data.len(),
        }],
        data,
    }
}

fn pixels(texture: &Texture, level: usize) -> &[u8] {
    let mip = texture.mips[level];
    &texture.data[mip.offset..mip.offset + mip.len]
}

#[test]
fn missing_mips_reach_one_texel_without_changing_source() {
    let source = rgba(8, 4, [80, 120, 160, 255].repeat(32));
    let result = with_mipmaps(&source, MipPolicy::Opaque);
    assert_eq!(
        result
            .mips
            .iter()
            .map(|m| (m.width, m.height))
            .collect::<Vec<_>>(),
        [(8, 4), (4, 2), (2, 1), (1, 1)]
    );
    assert_eq!(pixels(&result, 3), [80, 120, 160, 255]);
    assert_eq!(source.mips.len(), 1);
    assert_eq!(result.source_len, source.source_len);
}

#[test]
fn odd_dimensions_keep_the_last_row_and_column() {
    let source = rgba(
        3,
        3,
        (0..9)
            .flat_map(|i| {
                if i == 8 {
                    [180, 90, 45, 255]
                } else {
                    [0, 0, 0, 255]
                }
            })
            .collect(),
    );
    let result = with_mipmaps(&source, MipPolicy::Opaque);
    assert_eq!(result.mips.len(), 2);
    assert_eq!(pixels(&result, 1), [20, 10, 5, 255]);
}

#[test]
fn cutout_color_ignores_transparent_black_but_opaque_materials_do_not() {
    let source = rgba(2, 2, [[40, 180, 80, 255], [0; 4], [0; 4], [0; 4]].concat());
    let cutout = with_mipmaps(&source, MipPolicy::Cutout(170));
    let opaque = with_mipmaps(&source, MipPolicy::Opaque);
    assert_eq!(&pixels(&cutout, 1)[..3], [40, 180, 80]);
    assert_eq!(pixels(&opaque, 1), [10, 45, 20, 255]);
}

#[test]
fn fully_transparent_mips_stay_transparent_and_finite() {
    let source = rgba(4, 4, [90, 120, 150, 0].repeat(16));
    let result = with_mipmaps(&source, MipPolicy::Cutout(170));
    assert_eq!(pixels(&result, 2), [0; 4]);
}

#[test]
fn cutout_coverage_uses_the_requested_threshold_without_changing_blends() {
    let source = rgba(
        8,
        2,
        [255, 255, 255, 0, 255, 0, 0, 0, 255, 255, 0, 0, 255, 0, 0, 0]
            .into_iter()
            .flat_map(|alpha| [40, 180, 80, alpha])
            .collect(),
    );
    let blend = with_mipmaps(&source, MipPolicy::Blend);
    let cutout = with_mipmaps(&source, MipPolicy::Cutout(170));
    let coverage = |t: &Texture, level| {
        pixels(t, level)
            .chunks_exact(4)
            .filter(|p| p[3] >= 170)
            .count()
    };
    assert_eq!(coverage(&source, 0), 7);
    assert_eq!(coverage(&blend, 1), 1);
    assert_eq!(coverage(&cutout, 1), 2);
    assert_eq!(
        pixels(&blend, 1)
            .chunks_exact(4)
            .map(|p| p[3])
            .collect::<Vec<_>>(),
        [255, 64, 128, 0]
    );
    let low = with_mipmaps(&source, MipPolicy::Cutout(64));
    assert_ne!(pixels(&cutout, 1), pixels(&low, 1));
}

#[test]
fn complete_authored_chain_is_borrowed_and_partial_chain_is_retained() {
    let mut source = rgba(4, 4, [255, 0, 0, 255].repeat(16));
    source.mips.push(MipLevel {
        width: 2,
        height: 2,
        offset: 64,
        len: 16,
    });
    source.data.extend([0, 80, 160, 255].repeat(4));
    let partial = with_mipmaps(&source, MipPolicy::Opaque);
    assert_eq!(&partial.data[..source.data.len()], source.data);
    assert_eq!(pixels(&partial, 2), [0, 80, 160, 255]);
    let complete = partial.into_owned();
    assert!(matches!(
        with_mipmaps(&complete, MipPolicy::Cutout(170)),
        Cow::Borrowed(_)
    ));
}

#[test]
fn incomplete_block_compression_is_expanded_before_adding_mips() {
    for (format, data) in [
        (
            PixelFormat::Bc1RgbaUnorm,
            vec![0xE0, 0x07, 0, 0, 0, 0, 0, 0],
        ),
        (
            PixelFormat::Bc2RgbaUnorm,
            [vec![0xFF; 8], vec![0xE0, 0x07, 0, 0, 0, 0, 0, 0]].concat(),
        ),
    ] {
        let mut source = rgba(4, 4, data);
        source.format = format;
        let result = with_mipmaps(&source, MipPolicy::Cutout(170));
        assert_eq!(result.format, PixelFormat::Rgba8Unorm);
        assert_eq!(result.mips.len(), 3);
        assert_eq!(pixels(&result, 2), [0, 255, 0, 255]);
    }
}

#[test]
fn malformed_and_oversized_source_chains_are_not_rebuilt() {
    let mut source = rgba(4, 4, [255; 4].repeat(16));
    source.mips[0].len -= 1;
    assert!(matches!(
        with_mipmaps(&source, MipPolicy::Opaque),
        Cow::Borrowed(_)
    ));
    source.width = u32::MAX;
    source.height = u32::MAX;
    assert!(matches!(
        with_mipmaps(&source, MipPolicy::Opaque),
        Cow::Borrowed(_)
    ));
}

#[test]
fn sparse_cutouts_do_not_turn_into_opaque_final_mips() {
    let source = rgba(
        4,
        4,
        (0..16)
            .flat_map(|i| [80, 160, 40, if i == 0 { 255 } else { 0 }])
            .collect(),
    );
    let result = with_mipmaps(&source, MipPolicy::Cutout(170));
    assert!(pixels(&result, 2)[3] < 170);
    assert_eq!(&pixels(&result, 2)[..3], [80, 160, 40]);
}

#[test]
fn effect_rgb_is_filtered_independently_from_unused_alpha() {
    let source = rgba(2, 2, [80, 160, 40, 0].repeat(4));
    let result = with_mipmaps(&source, MipPolicy::Blend);
    assert_eq!(pixels(&result, 1), [80, 160, 40, 0]);
}

#[test]
fn generated_multilevel_pixels_are_exact_for_each_filtering_policy() {
    let source = rgba(
        4,
        2,
        [
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
    );

    for (policy, middle, last) in [
        (
            MipPolicy::Opaque,
            [60, 60, 70, 255, 228, 100, 110, 255],
            [144, 80, 90, 255],
        ),
        (
            MipPolicy::Blend,
            [60, 60, 70, 112, 228, 100, 110, 112],
            [144, 80, 90, 112],
        ),
        (
            MipPolicy::Cutout(0),
            [60, 87, 97, 112, 234, 73, 83, 112],
            [147, 80, 90, 112],
        ),
    ] {
        let result = with_mipmaps(&source, policy);
        assert_eq!(pixels(&result, 0), source.data);
        assert_eq!(pixels(&result, 1), middle, "{policy:?}");
        assert_eq!(pixels(&result, 2), last, "{policy:?}");
    }
}

#[test]
fn authored_compressed_chains_preserve_every_byte_offset_and_policy_independently() {
    for (format, block) in [
        (PixelFormat::Bc1RgbaUnorm, 8),
        (PixelFormat::Bc2RgbaUnorm, 16),
    ] {
        let source = Texture {
            width: 4,
            height: 4,
            format,
            mips: [4, 2, 1]
                .into_iter()
                .enumerate()
                .map(|(level, edge)| MipLevel {
                    width: edge,
                    height: edge,
                    offset: 3 + level * (block + 4),
                    len: block,
                })
                .collect(),
            data: (0..3 * (block + 4) + 3).map(|n| n as u8).collect(),
            source_len: 10000,
        };

        for policy in [MipPolicy::Opaque, MipPolicy::Blend, MipPolicy::Cutout(170)] {
            let result = with_mipmaps(&source, policy);
            assert!(matches!(result, Cow::Borrowed(_)));
            assert_eq!(result.as_ref(), &source);
        }
    }
}

#[test]
fn malformed_descriptors_are_returned_unchanged_without_panicking() {
    let source = rgba(4, 4, [1, 2, 3, 4].repeat(16));
    let mut cases = Vec::new();
    let mut changed = source.clone();
    changed.width = 0;
    cases.push(changed);
    let mut changed = source.clone();
    changed.mips.clear();
    cases.push(changed);
    let mut changed = source.clone();
    changed.mips[0].width = 3;
    cases.push(changed);
    let mut changed = source.clone();
    changed.mips[0].len -= 1;
    cases.push(changed);
    let mut changed = source.clone();
    changed.mips[0].offset = usize::MAX;
    cases.push(changed);
    let mut changed = source.clone();
    changed.data.pop();
    cases.push(changed);
    let mut changed = source.clone();
    changed.mips.resize(4, changed.mips[0]);
    cases.push(changed);
    let mut changed = source.clone();
    changed.width = u32::MAX;
    changed.height = u32::MAX;
    cases.push(changed);
    let mut changed = source;
    changed.format = PixelFormat::Bc1RgbaUnorm;
    cases.push(changed);

    for source in cases {
        for policy in [MipPolicy::Opaque, MipPolicy::Blend, MipPolicy::Cutout(170)] {
            let result = with_mipmaps(&source, policy);
            assert!(matches!(result, Cow::Borrowed(_)));
            assert_eq!(result.as_ref(), &source);
        }
    }
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn generated_compressed_tails_and_mixed_arrays_upload_without_validation_errors() {
    use crate::device::Gpu;
    use crate::texture::{AlphaPolicy, upload_material, upload_material_array};
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWake(std::thread::Thread);

    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let mut source = rgba(4, 4, vec![0xE0, 0x07, 0, 0, 0, 0, 0, 0]);
    source.format = PixelFormat::Bc1RgbaUnorm;
    let mut authored = source.clone();
    authored.data.extend_from_slice(&source.data);
    authored.data.extend_from_slice(&source.data);
    authored.mips.extend([
        MipLevel {
            width: 2,
            height: 2,
            offset: 8,
            len: 8,
        },
        MipLevel {
            width: 1,
            height: 1,
            offset: 16,
            len: 8,
        },
    ]);

    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);

    let texture = upload_material(
        &gpu,
        &source,
        AlphaPolicy::Stored,
        MipPolicy::Cutout(170),
        "generated BC cutout",
    );
    assert_eq!(texture.texture.format(), wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(texture.mip_levels, 3);
    let array = upload_material_array(
        &gpu,
        &[&source, &authored],
        AlphaPolicy::Opaque,
        MipPolicy::Opaque,
        "mixed authored and generated tiles",
    )
    .unwrap();
    assert_eq!(array.texture.format(), wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(array.mip_levels, 3);
    assert_eq!(array.texture.depth_or_array_layers(), 2);
    let sampler = crate::preset::Preset::LIT_3D
        .material_sampler_descriptor(gpu.adapter.get_downlevel_capabilities().flags);
    let _sampler = gpu.device.create_sampler(&sampler);
    gpu.queue.submit([]);
    assert!(block_on(validation.pop()).is_none());
}

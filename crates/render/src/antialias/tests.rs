use super::*;
use crate::Offscreen;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

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

fn process(
    gpu: &Gpu,
    antialias: &SceneAntialias,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    pixels: &[u8],
) -> Vec<u8> {
    process_with_mask(gpu, antialias, format, size, pixels, None)
}

fn process_with_mask(
    gpu: &Gpu,
    antialias: &SceneAntialias,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    pixels: &[u8],
    preservation: Option<&wgpu::TextureView>,
) -> Vec<u8> {
    let (width, height) = size;
    let frame = Offscreen::with_format(gpu, width, height, format);
    let target = Offscreen::with_format(gpu, width, height, format);
    let source = match preservation {
        Some(mask) => antialias.source_with_preservation(gpu, &frame.view, mask),
        None => antialias.source(gpu, &frame.view),
    };

    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &frame.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        frame.texture.size(),
    );

    antialias.render(gpu, &source, &target.view);

    let bytes_per_row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("antialias readback"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());

    antialias.encode(&mut encoder, &source, &target.view);

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        target.texture.size(),
    );

    let submission = gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);

    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).expect("send antialias readback");
    });

    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .expect("wait for antialias draw");

    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("receive antialias readback")
        .expect("map antialias readback");

    let result = slice
        .get_mapped_range()
        .expect("access antialias readback")
        .chunks_exact(bytes_per_row as usize)
        .flat_map(|row| row[..width as usize * 4].iter().copied())
        .collect();

    readback.unmap();

    result
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn preservation_masks_protect_effect_edges_and_attenuate_foliage_filtering() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let antialias = SceneAntialias::new(&gpu, format);
    let size = (37, 29);
    let pixels = (0..size.1)
        .flat_map(|y| {
            (0..size.0).flat_map(move |x| {
                let value = if x as f32 > 9.0 + 0.4 * y as f32 {
                    255
                } else {
                    0
                };
                [value, value, value, ((x * 17 + y * 31) % 256) as u8]
            })
        })
        .collect::<Vec<_>>();
    let unmasked = process(&gpu, &antialias, format, size, &pixels);
    let mask = Offscreen::with_format(&gpu, size.0, size.1, wgpu::TextureFormat::R8Unorm);
    let write_mask = |bytes: &[u8]| {
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &mask.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.0),
                rows_per_image: Some(size.1),
            },
            mask.texture.size(),
        );
    };
    let mut unmasked_change = 0_u64;

    for (before, after) in pixels.chunks_exact(4).zip(unmasked.chunks_exact(4)) {
        unmasked_change += u64::from(before[0].abs_diff(after[0]));
    }

    assert!(unmasked_change > 100, "baseline must filter the diagonal");

    for weight in [0, 64, 255] {
        write_mask(&vec![weight; (size.0 * size.1) as usize]);

        let output = process_with_mask(&gpu, &antialias, format, size, &pixels, Some(&mask.view));
        let mut change = 0_u64;

        for (before, after) in pixels.chunks_exact(4).zip(output.chunks_exact(4)) {
            assert_eq!(before[3], after[3]);
            change += u64::from(before[0].abs_diff(after[0]));
        }

        match weight {
            0 => assert_eq!(output, unmasked),
            255 => assert_eq!(output, pixels, "native protected pixels stay exact"),
            _ => assert!(
                change > 0 && change < unmasked_change,
                "partial mask attenuates AA"
            ),
        }
    }

    let localized = (0..size.1)
        .flat_map(|y| {
            (0..size.0).map(move |x| {
                if (14..=20).contains(&x) && (14..=22).contains(&y) {
                    255
                } else {
                    0
                }
            })
        })
        .collect::<Vec<_>>();

    write_mask(&localized);

    let output = process_with_mask(&gpu, &antialias, format, size, &pixels, Some(&mask.view));
    let mut outside_changes = 0;

    for y in 0..size.1 {
        for x in 0..size.0 {
            let offset = ((y * size.0 + x) * 4) as usize;

            if (13..=21).contains(&x) && (13..=23).contains(&y) {
                assert_eq!(
                    &output[offset..offset + 4],
                    &pixels[offset..offset + 4],
                    "protected pixel or immediate neighbor {x},{y}"
                );
            } else {
                outside_changes += usize::from(output[offset] != pixels[offset]);
            }
        }
    }

    assert!(outside_changes > 5, "unprotected edges still antialias");
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn uniform_colors_alpha_boundaries_and_resize_are_preserved() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);

    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8UnormSrgb,
    ] {
        assert!(supported(&gpu, format));

        let antialias = SceneAntialias::new(&gpu, format);

        for size in [(1, 1), (1, 9), (11, 1), (13, 17), (64, 32), (3, 5)] {
            for color in [[0, 0, 0, 0], [43, 101, 197, 255], [255, 255, 255, 67]] {
                let pixels = color.repeat((size.0 * size.1) as usize);
                let output = process(&gpu, &antialias, format, size, &pixels);

                assert_eq!(output, pixels, "uniform {color:?}, {size:?}, {format:?}");
            }
        }

        for size in [(1, 9), (11, 1)] {
            let pixels = (0..size.0 * size.1)
                .flat_map(|index| {
                    let value = if index > 4 { 255 } else { 0 };
                    [value, value, value, (index * 23) as u8]
                })
                .collect::<Vec<_>>();
            let output = process(&gpu, &antialias, format, size, &pixels);

            assert_eq!(output, pixels, "single-pixel edge: {size:?}, {format:?}");
        }

        let size = (31, 19);
        let pixels = (0..size.1)
            .flat_map(|y| {
                (0..size.0).flat_map(move |x| {
                    let value = if x > 15 { 255 } else { 0 };
                    [value, value, value, ((x * 17 + y * 31) % 256) as u8]
                })
            })
            .collect::<Vec<_>>();
        let output = process(&gpu, &antialias, format, size, &pixels);

        assert_eq!(output, pixels, "straight edges stay sharp: {format:?}");
    }

    assert!(block_on(scope.pop()).is_none());
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn low_contrast_texture_and_alternating_fine_lines_stay_sharp() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let antialias = SceneAntialias::new(&gpu, format);
    let size = (37, 29);

    for pattern in 0..3 {
        let pixels = (0..size.1)
            .flat_map(|y| {
                (0..size.0).flat_map(move |x| {
                    let value = match pattern {
                        0 => 128 + ((x * 7 + y * 11) % 13) as u8,
                        1 => {
                            if x % 2 == 0 {
                                255
                            } else {
                                0
                            }
                        }
                        _ => {
                            if y % 2 == 0 {
                                255
                            } else {
                                0
                            }
                        }
                    };
                    [value, value, value, 255]
                })
            })
            .collect::<Vec<_>>();
        let output = process(&gpu, &antialias, format, size, &pixels);

        assert_eq!(output, pixels, "fine detail pattern: {pattern}");
    }
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn diagonal_edges_approach_pixel_coverage_without_spreading_or_changing_alpha() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let antialias = SceneAntialias::new(&gpu, format);
    let size = (67, 43);

    for slope in [0.4_f32, 1.0, 2.5, -0.7] {
        let signed_distance = |x: f32, y: f32| x - 33.25 - slope * (y - 21.0);
        let pixels = (0..size.1)
            .flat_map(|y| {
                (0..size.0).flat_map(move |x| {
                    let value = if signed_distance(x as f32 + 0.5, y as f32 + 0.5) > 0.0 {
                        255
                    } else {
                        0
                    };
                    [value, value, value, ((x * 17 + y * 31) % 256) as u8]
                })
            })
            .collect::<Vec<_>>();
        let output = process(&gpu, &antialias, format, size, &pixels);
        let mut before_error = 0.0;
        let mut after_error = 0.0;
        let mut smoothed = 0;

        for y in 2..size.1 - 2 {
            for x in 2..size.0 - 2 {
                let offset = ((y * size.0 + x) * 4) as usize;
                let covered = (0..16)
                    .flat_map(|sy| (0..16).map(move |sx| (sx, sy)))
                    .filter(|&(sx, sy)| {
                        signed_distance(
                            x as f32 + (sx as f32 + 0.5) / 16.0,
                            y as f32 + (sy as f32 + 0.5) / 16.0,
                        ) > 0.0
                    })
                    .count() as f32
                    / 256.0;
                before_error += (pixels[offset] as f32 / 255.0 - covered).powi(2);
                after_error += (output[offset] as f32 / 255.0 - covered).powi(2);
                smoothed += usize::from(output[offset] > 0 && output[offset] < 255);

                if signed_distance(x as f32 + 0.5, y as f32 + 0.5).abs() > 1.5 + slope.abs() {
                    assert_eq!(output[offset], pixels[offset], "edge spread at {x},{y}");
                }
            }
        }

        assert!(smoothed > 10, "diagonal must be antialiased: {slope}");
        assert!(
            after_error < before_error * 0.9,
            "coverage error must improve: {slope}, before={before_error}, after={after_error}"
        );

        for (before, after) in pixels.chunks_exact(4).zip(output.chunks_exact(4)) {
            assert_eq!(before[3], after[3], "preserve center alpha");
        }
    }

    assert!(block_on(scope.pop()).is_none());
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn float_targets_validate_and_noncolor_formats_are_rejected() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let format = wgpu::TextureFormat::Rgba16Float;

    assert!(supported(&gpu, format));

    let antialias = SceneAntialias::new(&gpu, format);
    let frame = Offscreen::with_format(&gpu, 7, 9, format);
    let target = Offscreen::with_format(&gpu, 7, 9, format);
    let source = antialias.source(&gpu, &frame.view);

    antialias.render(&gpu, &source, &target.view);

    for format in [
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureFormat::Rgba8Uint,
        wgpu::TextureFormat::Rgba8Sint,
        wgpu::TextureFormat::Bc1RgbaUnorm,
    ] {
        assert!(!supported(&gpu, format), "unsupported format: {format:?}");
    }

    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(30)),
        })
        .expect("wait for float antialias draw");

    assert!(block_on(scope.pop()).is_none());
}

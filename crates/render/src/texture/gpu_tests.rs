use super::*;
use formats::texture::MipLevel;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

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

fn wait(gpu: &Gpu, submission: wgpu::SubmissionIndex) {
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .expect("wait for synthetic texture work");
}

fn synthetic(edge: u32, compressed: bool, seed: u32) -> Texture {
    let (format, data) = if compressed {
        let data = (0..edge.div_ceil(4).pow(2))
            .flat_map(|index| {
                let indices = 0xe4e4e4e4_u32.rotate_left((index + seed) % 32);
                [vec![0x1f, 0, 0, 0xf8], indices.to_le_bytes().to_vec()].concat()
            })
            .collect::<Vec<_>>();
        (PixelFormat::Bc1RgbaUnorm, data)
    } else {
        let data = (0..edge)
            .flat_map(|y| {
                (0..edge).flat_map(move |x| {
                    [
                        (x * 17 + y * 3 + seed) as u8,
                        (x * 5 + y * 23 + seed * 3) as u8,
                        (x * 11 + y * 7 + seed * 5) as u8,
                        if (x + y + seed) % 7 < 3 { 255 } else { 0 },
                    ]
                })
            })
            .collect::<Vec<_>>();
        (PixelFormat::Rgba8Unorm, data)
    };

    Texture {
        width: edge,
        height: edge,
        format,
        mips: vec![MipLevel {
            width: edge,
            height: edge,
            offset: 0,
            len: data.len(),
        }],
        source_len: data.len(),
        data,
    }
}

fn readback_pipeline(gpu: &Gpu) -> wgpu::ComputePipeline {
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("prepared material pixel readback"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> pixels: array<u32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(image);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    pixels[id.y * size.x + id.x] = pack4x8unorm(textureLoad(image, vec2<i32>(id.xy), 0));
}
"#
                .into(),
            ),
        });

    gpu.device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("prepared material pixel readback"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
}

fn read_level(
    gpu: &Gpu,
    pipeline: &wgpu::ComputePipeline,
    texture: &GpuTexture,
    level: u32,
) -> Vec<u8> {
    let width = (texture.width >> level).max(1);
    let height = (texture.height >> level).max(1);
    let size = u64::from(width) * u64::from(height) * 4;
    let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("prepared material readback storage"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("prepared material readback mapping"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
        base_mip_level: level,
        mip_level_count: Some(1),
        ..Default::default()
    });
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("prepared material readback bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, size);
    let submission = gpu.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
    wait(gpu, submission);
    receiver
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    let result = readback.slice(..).get_mapped_range().unwrap().to_vec();
    readback.unmap();
    result
}

fn verify_pixels(gpu: &Gpu) {
    let pipeline = readback_pipeline(gpu);
    for compressed in [false, true] {
        let source = Arc::new(synthetic(32, compressed, 7));
        let prepared = PreparedMaterial::from_shared(source.clone(), MipPolicy::Cutout(170));
        let old = upload_material(
            gpu,
            &source,
            AlphaPolicy::Stored,
            MipPolicy::Cutout(170),
            "legacy synthetic cutout",
        );
        let new = prepared.upload(gpu, AlphaPolicy::Stored, "prepared synthetic cutout");
        assert_eq!(old.mip_levels, new.mip_levels);
        assert_eq!(old.texture.format(), new.texture.format());

        for level in 0..old.mip_levels {
            let old_pixels = read_level(gpu, &pipeline, &old, level);
            let new_pixels = read_level(gpu, &pipeline, &new, level);
            assert_eq!(
                old_pixels, new_pixels,
                "compressed={compressed}, mip={level}"
            );
            assert_eq!(
                new_pixels,
                prepared.texture().level(level as usize).unwrap()
            );
        }
    }
}

fn upload_burst(
    gpu: &Gpu,
    sources: &[Arc<Texture>],
    prepared: Option<&[PreparedMaterial]>,
) -> Duration {
    let start = Instant::now();
    let textures = match prepared {
        Some(prepared) => prepared
            .iter()
            .map(|material| material.upload(gpu, AlphaPolicy::Stored, "prepared cold burst"))
            .collect::<Vec<_>>(),
        None => sources
            .iter()
            .map(|texture| {
                upload_material(
                    gpu,
                    texture,
                    AlphaPolicy::Stored,
                    MipPolicy::Cutout(170),
                    "legacy cold burst",
                )
            })
            .collect::<Vec<_>>(),
    };
    let submission = gpu.queue.submit([]);
    let elapsed = start.elapsed();
    wait(gpu, submission);
    drop(textures);
    elapsed
}

fn report(label: &str, times: &mut [Duration]) {
    times.sort_unstable();
    let percentile =
        |percent: usize| times[(times.len() * percent).div_ceil(100) - 1].as_secs_f64() * 1000.0;
    println!(
        "{label}: n={} p95_ms={:.3} p99_ms={:.3} max_ms={:.3}",
        times.len(),
        percentile(95),
        percentile(99),
        times.last().unwrap().as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "requires a compatible GPU adapter; run release with --nocapture for timings"]
fn prepared_material_gpu_pixels_and_cold_burst_upload_timings() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    verify_pixels(&gpu);

    const BURSTS: usize = 120;
    const TEXTURES: usize = 6;
    const EDGE: u32 = 256;
    let mut preparation = Vec::with_capacity(BURSTS);
    let mut legacy = Vec::with_capacity(BURSTS);
    let mut prepared_upload = Vec::with_capacity(BURSTS);

    for burst in 0..BURSTS {
        let sources = (0..TEXTURES)
            .map(|index| {
                Arc::new(synthetic(
                    EDGE,
                    index % 2 == 0,
                    (burst * TEXTURES + index) as u32,
                ))
            })
            .collect::<Vec<_>>();
        let (prepared, elapsed) = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let start = Instant::now();
                    let prepared = sources
                        .iter()
                        .map(|source| {
                            PreparedMaterial::from_shared(source.clone(), MipPolicy::Cutout(170))
                        })
                        .collect::<Vec<_>>();
                    (prepared, start.elapsed())
                })
                .join()
                .unwrap()
        });
        preparation.push(elapsed);

        if burst % 2 == 0 {
            legacy.push(upload_burst(&gpu, &sources, None));
            prepared_upload.push(upload_burst(&gpu, &sources, Some(&prepared)));
        } else {
            prepared_upload.push(upload_burst(&gpu, &sources, Some(&prepared)));
            legacy.push(upload_burst(&gpu, &sources, None));
        }
    }

    assert!(block_on(validation.pop()).is_none());
    println!(
        "adapter={:?} release={}",
        gpu.info(),
        !cfg!(debug_assertions)
    );
    println!(
        "GPU pixels matched legacy and prepared CPU bytes for every mip of BC1/RGBA cutouts; {BURSTS} cold bursts x {TEXTURES} fresh {EDGE}x{EDGE} textures (3 incomplete BC1 + 3 RGBA), Cutout(170)"
    );
    println!(
        "Upload phase is CPU texture creation/write_texture/queue.submit; GPU waits and source generation are excluded. Preparation runs on a worker before each simulated frame; alternating upload order; no end-to-end speedup claim."
    );
    report("legacy frame upload including CPU preparation", &mut legacy);
    report("prepared frame upload only", &mut prepared_upload);
    report("worker CPU preparation outside frame", &mut preparation);
}

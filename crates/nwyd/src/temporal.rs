use render::{BlitSource, Blitter, Gpu, Offscreen, temporal::TemporalCamera};
use web_time::{Duration, Instant};

pub(crate) struct TemporalFrame {
    pub camera: TemporalCamera,
    pub scene: Offscreen,
    pub motion: wgpu::TextureView,
    pub bias: wgpu::TextureView,
    blitter: Blitter,
    scene_source: BlitSource,
    previous_scene: Option<(u64, u32, Instant)>,
}

impl TemporalFrame {
    pub fn new(gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) -> Option<Self> {
        if gpu.device.limits().max_bind_groups < 5 || size.0 == 0 || size.1 == 0 {
            return None;
        }

        let scene = Offscreen::with_format(gpu, size.0, size.1, format);

        let blitter = Blitter::new(gpu, format);

        let scene_source = blitter.source(gpu, &scene);

        Some(Self {
            camera: TemporalCamera::new(gpu),
            scene,
            motion: attachment(
                gpu,
                [size.0, size.1],
                render::temporal::MOTION_FORMAT,
                "scene motion vectors",
            ),
            bias: attachment(
                gpu,
                [size.0, size.1],
                render::temporal::BIAS_FORMAT,
                "scene reactive mask",
            ),
            blitter,
            scene_source,
            previous_scene: None,
        })
    }

    pub fn resize(&mut self, gpu: &Gpu, size: (u32, u32)) {
        if (self.scene.width, self.scene.height) == size {
            return;
        }

        self.scene = Offscreen::with_format(gpu, size.0, size.1, self.scene.format);

        self.scene_source = self.blitter.source(gpu, &self.scene);

        self.motion = attachment(
            gpu,
            [size.0, size.1],
            render::temporal::MOTION_FORMAT,
            "scene motion vectors",
        );

        self.bias = attachment(
            gpu,
            [size.0, size.1],
            render::temporal::BIAS_FORMAT,
            "scene reactive mask",
        );

        self.reset();
    }

    pub fn prepare(&mut self, gpu: &Gpu, frame: &render::world::FrameUniforms) {
        self.camera.prepare(
            gpu,
            frame,
            [self.scene.width, self.scene.height],
            [0.0; 2],
            0.0,
        );
    }

    pub fn scene_stamp(&mut self, incarnation: u64, clock: u32) -> bool {
        let now = Instant::now();

        let reset = scene_discontinuity(self.previous_scene, incarnation, clock, now);

        if reset {
            self.reset();
        }

        self.previous_scene = Some((incarnation, clock, now));

        reset
    }

    pub fn reset(&mut self) {
        self.camera.reset();
    }

    pub fn finish(&self, gpu: &Gpu, target: &wgpu::TextureView) {
        self.blitter.render(
            gpu,
            &self.scene_source,
            target,
            (self.scene.width, self.scene.height),
        );
    }
}

fn scene_discontinuity(
    previous: Option<(u64, u32, Instant)>,
    incarnation: u64,
    clock: u32,
    now: Instant,
) -> bool {
    previous.is_some_and(|(old, time, observed)| {
        let elapsed = now.saturating_duration_since(observed);

        let advance = clock.wrapping_sub(time);

        old != incarnation
            || elapsed >= Duration::from_secs(5)
            || u128::from(advance) > elapsed.as_millis() + 250
    })
}

fn attachment(
    gpu: &Gpu,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: render::gpu_label(label),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

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
    #[cfg(feature = "neural-rendering")]
    neural: Option<render::neural::NeuralRenderer>,
    reset_pending: bool,
    failure: Option<String>,
}

impl TemporalFrame {
    pub fn new(gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) -> Option<Self> {
        if !render::temporal::supported(gpu, format, size) {
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
            #[cfg(feature = "neural-rendering")]
            neural: None,
            reset_pending: true,
            failure: None,
        })
    }

    #[cfg(feature = "neural-rendering")]
    pub fn new_neural(
        gpu: &Gpu,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        if format != render::COLOR_FORMAT {
            return Err("Neural Rendering requires an RGBA8 scene target".into());
        }

        let mut frame = Self::new(gpu, size, format)
            .ok_or("The device cannot render Neural Rendering input buffers")?;

        frame.neural = Some(render::neural::NeuralRenderer::new(gpu, [size.0, size.1])?);

        Ok(frame)
    }

    pub fn resize(&mut self, gpu: &Gpu, size: (u32, u32)) {
        if (self.scene.width, self.scene.height) == size {
            return;
        }

        #[cfg(feature = "neural-rendering")]
        if let Some(neural) = &mut self.neural
            && let Err(error) = neural.resize(gpu, [size.0, size.1])
        {
            self.failure = Some(error);
            self.neural = None;
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
        self.reset_pending |= self.camera.prepare(
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
        self.reset_pending = true;
    }

    pub fn release_neural(&mut self) {
        #[cfg(feature = "neural-rendering")]
        {
            self.neural = None;
        }

        self.reset();
    }

    pub fn take_failure(&mut self) -> Option<String> {
        self.failure.take()
    }

    pub fn finish(
        &mut self,
        gpu: &Gpu,
        target: &wgpu::TextureView,
        depth: Option<&wgpu::TextureView>,
    ) {
        #[cfg(feature = "neural-rendering")]
        if let Some(neural) = &mut self.neural {
            let result = depth
                .ok_or_else(|| "Neural Rendering depth is unavailable".to_owned())
                .and_then(|depth| {
                    neural.evaluate(
                        gpu,
                        &self.scene.view,
                        depth,
                        &self.motion,
                        &self.bias,
                        target,
                        self.reset_pending,
                    )
                });

            match result {
                Ok(()) => {
                    self.reset_pending = false;
                    return;
                }
                Err(error) => {
                    self.failure = Some(error);
                    self.neural = None;
                }
            }
        }

        #[cfg(not(feature = "neural-rendering"))]
        let _ = depth;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_history_tracks_elapsed_time_and_wrapping_game_clocks() {
        let start = Instant::now();
        assert!(!scene_discontinuity(None, 1, 100, start));
        assert!(!scene_discontinuity(
            Some((1, 100, start)),
            1,
            130,
            start + Duration::from_millis(30),
        ));
        assert!(!scene_discontinuity(
            Some((1, u32::MAX - 9, start)),
            1,
            10,
            start + Duration::from_millis(20),
        ));
        assert!(scene_discontinuity(
            Some((1, 100, start)),
            1,
            99,
            start + Duration::from_millis(20),
        ));
    }

    #[test]
    fn scene_changes_pauses_and_clock_jumps_discard_model_history() {
        let start = Instant::now();
        assert!(scene_discontinuity(Some((1, 100, start)), 2, 100, start));
        assert!(scene_discontinuity(
            Some((1, 100, start)),
            1,
            5100,
            start + Duration::from_secs(5),
        ));
        assert!(scene_discontinuity(
            Some((1, 100, start)),
            1,
            400,
            start + Duration::from_millis(20),
        ));
        assert!(!scene_discontinuity(Some((1, 100, start)), 1, 350, start,));
    }
}

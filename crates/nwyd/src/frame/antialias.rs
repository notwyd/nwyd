use render::{
    Gpu, Offscreen,
    antialias::{AntialiasSource, SceneAntialias},
};

pub(super) struct AntialiasFrame {
    effect: SceneAntialias,
    frame: Option<(Offscreen, AntialiasSource)>,
    format: wgpu::TextureFormat,
    preservation: Option<wgpu::TextureView>,
}

impl AntialiasFrame {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Option<Self> {
        render::antialias::supported(gpu, format).then(|| Self {
            effect: SceneAntialias::new(gpu, format),
            frame: None,
            format,
            preservation: None,
        })
    }

    pub fn resize(
        &mut self,
        gpu: &Gpu,
        width: u32,
        height: u32,
        preservation: Option<&wgpu::TextureView>,
    ) {
        let same_size = self
            .frame
            .as_ref()
            .is_some_and(|(frame, _)| frame.width == width && frame.height == height);
        if same_size && self.preservation.as_ref() == preservation {
            return;
        }
        let scene = if same_size {
            self.frame.take().unwrap().0
        } else {
            Offscreen::with_format(gpu, width, height, self.format)
        };
        let source = match preservation {
            Some(mask) => self.effect.source_with_preservation(gpu, &scene.view, mask),
            None => self.effect.source(gpu, &scene.view),
        };
        self.frame = Some((scene, source));
        self.preservation = preservation.cloned();
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self
            .frame
            .as_ref()
            .expect("scene antialias target prepared")
            .0
            .view
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        self.effect.encode(
            encoder,
            &self
                .frame
                .as_ref()
                .expect("scene antialias target prepared")
                .1,
            target,
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::time::Duration;

    fn read(gpu: &Gpu, target: &Offscreen) -> Vec<u8> {
        let bytes_per_row = (target.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene antialias wrapper readback"),
            size: u64::from(bytes_per_row) * u64::from(target.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(target.height),
                },
            },
            target.texture.size(),
        );

        let submission = gpu.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);

        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).expect("send scene antialias readback");
        });

        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .expect("wait for scene antialias readback");
        receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("receive scene antialias readback")
            .expect("map scene antialias readback");

        let pixels = slice
            .get_mapped_range()
            .expect("access scene antialias readback")
            .chunks_exact(bytes_per_row as usize)
            .flat_map(|row| row[..target.width as usize * 4].iter().copied())
            .collect();

        buffer.unmap();

        pixels
    }

    fn write(gpu: &Gpu, texture: &wgpu::Texture, pixels: &[u8], channels: u32) {
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(texture.width() * channels),
                rows_per_image: Some(texture.height()),
            },
            texture.size(),
        );
    }

    #[test]
    #[ignore = "requires a compatible GPU adapter"]
    fn resize_reuses_targets_and_rebinds_changed_masks_without_stale_frames() {
        let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
        let format = render::COLOR_FORMAT;
        let mut antialias = AntialiasFrame::new(&gpu, format).expect("antialias support");
        let mut previous = None;

        for (index, (width, height)) in [(17, 11), (17, 11), (1, 9), (31, 19)]
            .into_iter()
            .enumerate()
        {
            antialias.resize(&gpu, width, height, None);

            let texture = antialias.view().texture();
            if let Some(old) = &previous {
                assert_eq!(
                    old == texture,
                    index == 1,
                    "only matching dimensions reuse allocation"
                );
            }
            previous = Some(texture.clone());

            let color = [32 + index as u8 * 40, 113, 217, 79];
            let pixels = color.repeat((width * height) as usize);
            let target = Offscreen::with_format(&gpu, width, height, format);

            write(&gpu, texture, &pixels, 4);

            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            antialias.encode(&mut encoder, &target.view);
            gpu.queue.submit([encoder.finish()]);

            assert_eq!(
                read(&gpu, &target),
                pixels,
                "resized source binding must be current"
            );
        }

        let size = (31, 19);
        let color_texture = antialias.view().texture().clone();
        let masks = [0_u8, 255].map(|weight| {
            let mask = Offscreen::with_format(&gpu, size.0, size.1, wgpu::TextureFormat::R8Unorm);
            write(
                &gpu,
                &mask.texture,
                &vec![weight; (size.0 * size.1) as usize],
                1,
            );
            mask
        });
        let pixels = (0..size.1)
            .flat_map(|y| {
                (0..size.0).flat_map(move |x| {
                    let value = if x as f32 > 8.0 + 0.4 * y as f32 {
                        255
                    } else {
                        0
                    };
                    [value, value, value, 255]
                })
            })
            .collect::<Vec<_>>();
        write(&gpu, &color_texture, &pixels, 4);
        let target = Offscreen::with_format(&gpu, size.0, size.1, format);

        for (mask, protected) in [
            (Some(&masks[0].view), false),
            (Some(&masks[1].view), true),
            (Some(&masks[1].view), true),
            (None, false),
        ] {
            antialias.resize(&gpu, size.0, size.1, mask);
            assert_eq!(
                &color_texture,
                antialias.view().texture(),
                "mask changes retain scene allocation"
            );

            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            antialias.encode(&mut encoder, &target.view);
            gpu.queue.submit([encoder.finish()]);

            assert_eq!(
                read(&gpu, &target) == pixels,
                protected,
                "mask binding must match the current texture"
            );
        }
    }

    #[test]
    #[ignore = "requires a compatible GPU adapter"]
    fn scene_composition_keeps_interface_diagonals_crisp_after_antialiasing() {
        use crate::frame::{PlannedQuad, SceneRenderer, TextureKey};
        use render::world::{Fog, FrameUniforms, Lighting, Material, PointLights};
        use render::{Matrix4, Quad, RenderMode, UvRule};

        let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let format = render::COLOR_FORMAT;
        let size = (67, 43);
        let expected = Offscreen::with_format(&gpu, size.0, size.1, format);
        let target = Offscreen::with_format(&gpu, size.0, size.1, format);
        let mut renderer = SceneRenderer::new(&gpu, format);
        let frame = FrameUniforms::new(
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            &Lighting {
                light0: [0.0; 4],
                light1: [0.0; 4],
                material: Material::TERRAIN,
                global_ambient: 0,
                point_lights: PointLights::default(),
            },
            &Fog {
                enabled: false,
                color: 0,
                start: 100.0,
                end: 200.0,
            },
        );
        let mut quad = Quad::whole(16.0, 12.0, 1, 1, 0xFFFF_FFFF);
        quad.dst_w = 30.0;
        quad.dst_h = 5.0;
        quad.rotation = 0.4;
        let planned = PlannedQuad {
            mode: RenderMode::NuiPanel2d,
            texture: TextureKey::White,
            quad,
            uv: UvRule::TexelEdge,
        };

        renderer.world_frame = Some((frame, false));
        renderer.mesh_queued = true;
        renderer.clear = wgpu::Color::BLACK;
        renderer.push(&planned);
        renderer.prepare(&gpu, size);
        renderer.render(&gpu, &expected.view);

        let reference = read(&gpu, &expected);
        assert!(
            reference
                .chunks_exact(4)
                .any(|pixel| pixel[..3] == [255; 3])
        );
        assert!(
            reference
                .chunks_exact(4)
                .all(|pixel| pixel[0] == 0 || pixel[0] == 255)
        );

        renderer.render_frame(&gpu, &target.view);
        assert_eq!(
            read(&gpu, &target),
            reference,
            "native AA must precede interface drawing"
        );

        let mut temporal = SceneRenderer::new_with_motion(&gpu, format, size);
        temporal.world_frame = Some((frame, false));
        temporal.mesh_queued = true;
        temporal.clear = wgpu::Color::BLACK;
        temporal.push(&planned);
        temporal.prepare(&gpu, size);
        temporal.render_frame(&gpu, &target.view);
        assert_eq!(
            read(&gpu, &target),
            reference,
            "temporal composition and AA must precede interface drawing"
        );
        assert!(pollster::block_on(scope.pop()).is_none());
    }
}

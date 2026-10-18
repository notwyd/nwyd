use crate::{Gpu, world::FrameUniforms};
use bytemuck::{Pod, Zeroable};
use glam::{DMat4, Mat4};

pub const MOTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

pub const BIAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

pub fn supported(gpu: &Gpu, format: wgpu::TextureFormat, size: (u32, u32)) -> bool {
    let features = gpu.device.features();
    let downlevel = gpu.adapter.get_downlevel_capabilities().flags;
    let formats = [
        format,
        MOTION_FORMAT,
        BIAS_FORMAT,
        wgpu::TextureFormat::Depth32Float,
    ];

    if formats
        .iter()
        .any(|format| !features.contains(format.required_features()))
    {
        return false;
    }

    supported_capabilities(
        &gpu.device.limits(),
        downlevel,
        format,
        size,
        formats.map(|format| {
            effective_format_features(
                format,
                features,
                downlevel,
                gpu.adapter.get_texture_format_features(format),
            )
        }),
    )
}

fn effective_format_features(
    format: wgpu::TextureFormat,
    features: wgpu::Features,
    downlevel: wgpu::DownlevelFlags,
    mut supported: wgpu::TextureFormatFeatures,
) -> wgpu::TextureFormatFeatures {
    if !features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
        && downlevel.contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT)
    {
        let guaranteed = format.guaranteed_format_features(features);
        supported.allowed_usages &= guaranteed.allowed_usages;
        supported.flags &= guaranteed.flags;
    }

    if matches!(
        format,
        wgpu::TextureFormat::R32Float
            | wgpu::TextureFormat::Rg32Float
            | wgpu::TextureFormat::Rgba32Float
    ) && !features.contains(wgpu::Features::FLOAT32_FILTERABLE)
    {
        supported
            .flags
            .remove(wgpu::TextureFormatFeatureFlags::FILTERABLE);
    }

    supported
}

fn supported_capabilities(
    limits: &wgpu::Limits,
    downlevel: wgpu::DownlevelFlags,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    formats: [wgpu::TextureFormatFeatures; 4],
) -> bool {
    let Some(attachment_bytes) =
        [format, MOTION_FORMAT, BIAS_FORMAT]
            .into_iter()
            .try_fold(0_u32, |bytes, format| {
                Some(
                    bytes.next_multiple_of(format.target_component_alignment()?)
                        + format.target_pixel_byte_cost()?,
                )
            })
    else {
        return false;
    };
    let palette_size = crate::skin::TEMPORAL_PALETTE_SIZE;
    let alignment = u64::from(limits.min_uniform_buffer_offset_alignment).max(1);
    let palette_buffer_size =
        palette_size.div_ceil(alignment) * alignment * u64::from(crate::skin::PALETTE_CAPACITY);
    let usages = wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    let [color, motion, bias, depth] = formats;

    size.0 > 0
        && size.1 > 0
        && size.0 <= limits.max_texture_dimension_2d
        && size.1 <= limits.max_texture_dimension_2d
        && downlevel.contains(wgpu::DownlevelFlags::INDEPENDENT_BLEND)
        && limits.max_bind_groups >= 5
        && limits.max_bind_groups_plus_vertex_buffers >= 6
        && limits.max_uniform_buffers_per_shader_stage >= 4
        && u64::from(limits.max_uniform_buffer_binding_size) >= palette_size
        && limits.max_buffer_size >= palette_buffer_size
        && limits.max_inter_stage_shader_variables >= 13
        && limits.max_color_attachments >= 3
        && limits.max_color_attachment_bytes_per_sample >= attachment_bytes
        && color
            .allowed_usages
            .contains(usages | wgpu::TextureUsages::COPY_DST)
        && color.flags.contains(
            wgpu::TextureFormatFeatureFlags::FILTERABLE
                | wgpu::TextureFormatFeatureFlags::BLENDABLE,
        )
        && motion.allowed_usages.contains(usages)
        && bias.allowed_usages.contains(usages)
        && bias
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
        && depth.allowed_usages.contains(usages)
}

pub(crate) fn layouts<'a>(
    base: &[Option<&'a wgpu::BindGroupLayout>],
    camera: Option<&'a TemporalCamera>,
) -> Vec<Option<&'a wgpu::BindGroupLayout>> {
    let mut result = base.to_vec();

    if let Some(camera) = camera {
        result.resize(4, None);

        result.push(Some(&camera.layout));
    }

    result
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    reprojection: [f32; 16],
    jitter_scale: [f32; 4],
    options: [f32; 4],
}

pub struct TemporalCamera {
    pub layout: wgpu::BindGroupLayout,
    pub binding: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    previous: Option<DMat4>,
    previous_eye: Option<glam::Vec3>,
    previous_projection: Option<[f32; 16]>,
}

impl TemporalCamera {
    pub fn new(gpu: &Gpu) -> Self {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("temporal camera"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Uniform>() as u64
                        ),
                    },
                    count: None,
                }],
            });

        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("temporal camera"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("temporal camera"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            layout,
            binding,
            buffer,
            previous: None,
            previous_eye: None,
            previous_projection: None,
        }
    }

    pub fn reset(&mut self) {
        self.previous = None;

        self.previous_eye = None;

        self.previous_projection = None;
    }

    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        frame: &FrameUniforms,
        size: [u32; 2],
        jitter: [f32; 2],
        mip_bias: f32,
    ) -> bool {
        let view = Mat4::from_cols_array(&frame.view).as_dmat4();

        let current = view * Mat4::from_cols_array(&frame.projection).as_dmat4();

        let eye = view.inverse().transpose().w_axis.truncate().as_vec3();

        let inverse = current.inverse();

        let valid = current.is_finite() && inverse.is_finite();

        let reset = self.previous.is_none()
            || !valid
            || self
                .previous_eye
                .is_some_and(|old| old.distance_squared(eye) > 25.0)
            || self
                .previous_projection
                .is_some_and(|old| old != frame.projection);

        let previous = if reset {
            current
        } else {
            self.previous.unwrap_or(current)
        };

        let reprojection = if valid {
            camera_delta(current, previous)
        } else {
            Mat4::IDENTITY
        };

        let scale = [size[0] as f32 * 0.5, -(size[1] as f32) * 0.5];

        let uniform = Uniform {
            reprojection: reprojection.to_cols_array(),
            jitter_scale: [
                jitter[0] / scale[0],
                jitter[1] / scale[1],
                scale[0],
                scale[1],
            ],
            options: [
                if frame.ground_wave[3] != 0.0 {
                    1.0
                } else {
                    0.0
                },
                mip_bias,
                0.0,
                0.0,
            ],
        };

        gpu.queue
            .write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));

        self.previous = valid.then_some(current);

        self.previous_eye = valid.then_some(eye);

        self.previous_projection = valid.then_some(frame.projection);

        reset
    }
}

fn camera_delta(current: DMat4, previous: DMat4) -> Mat4 {
    (current.inverse() * previous).as_mat4()
}

pub(crate) fn targets(
    color: wgpu::ColorTargetState,
    temporal: bool,
    depth_write: bool,
) -> Vec<Option<wgpu::ColorTargetState>> {
    let mut targets = vec![Some(color)];

    if temporal {
        targets.push(Some(wgpu::ColorTargetState {
            format: MOTION_FORMAT,
            blend: None,
            write_mask: if depth_write {
                wgpu::ColorWrites::ALL
            } else {
                wgpu::ColorWrites::empty()
            },
        }));

        let max = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        };

        targets.push(Some(wgpu::ColorTargetState {
            format: BIAS_FORMAT,
            blend: Some(wgpu::BlendState {
                color: max,
                alpha: max,
            }),
            write_mask: wgpu::ColorWrites::ALL,
        }));
    }

    targets
}

pub(crate) fn fragment_options(
    temporal: bool,
    blend: Option<wgpu::BlendState>,
    depth_write: bool,
) -> wgpu::PipelineCompilationOptions<'static> {
    let constants = if !temporal {
        &[][..]
    } else if depth_write {
        &[("temporal_coverage", 0.0)][..]
    } else {
        match blend.map(|b| (b.color.src_factor, b.color.dst_factor, b.color.operation)) {
            Some((
                wgpu::BlendFactor::SrcAlpha,
                wgpu::BlendFactor::OneMinusSrcAlpha,
                wgpu::BlendOperation::Add,
            )) => &[("temporal_coverage", 1.0)][..],
            Some((
                wgpu::BlendFactor::SrcAlpha,
                wgpu::BlendFactor::One,
                wgpu::BlendOperation::Add,
            )) => &[("temporal_coverage", 2.0)][..],
            _ => &[("temporal_coverage", 0.0)][..],
        }
    };

    wgpu::PipelineCompilationOptions {
        constants,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::camera::Matrix4;
    use glam::{Vec3, Vec4};

    fn temporal_limits() -> wgpu::Limits {
        let palette_size = crate::skin::TEMPORAL_PALETTE_SIZE;

        wgpu::Limits {
            max_texture_dimension_2d: 4096,
            max_bind_groups: 5,
            max_bind_groups_plus_vertex_buffers: 6,
            max_uniform_buffers_per_shader_stage: 4,
            max_uniform_buffer_binding_size: palette_size,
            max_buffer_size: palette_size.div_ceil(256)
                * 256
                * u64::from(crate::skin::PALETTE_CAPACITY),
            min_uniform_buffer_offset_alignment: 256,
            max_inter_stage_shader_variables: 13,
            max_color_attachments: 3,
            max_color_attachment_bytes_per_sample: 13,
            ..Default::default()
        }
    }

    fn temporal_formats(format: wgpu::TextureFormat) -> [wgpu::TextureFormatFeatures; 4] {
        [
            format,
            MOTION_FORMAT,
            BIAS_FORMAT,
            wgpu::TextureFormat::Depth32Float,
        ]
        .map(|format| format.guaranteed_format_features(wgpu::Features::empty()))
    }

    fn supports_temporal(limits: &wgpu::Limits, formats: [wgpu::TextureFormatFeatures; 4]) -> bool {
        supported_capabilities(
            limits,
            wgpu::DownlevelFlags::INDEPENDENT_BLEND,
            crate::COLOR_FORMAT,
            (1920, 1080),
            formats,
        )
    }

    #[test]
    fn temporal_capability_limits_cover_pipeline_and_history_buffers() {
        let limits = temporal_limits();
        let formats = temporal_formats(crate::COLOR_FORMAT);
        assert!(supports_temporal(&limits, formats));

        let cases: [(&str, fn(&mut wgpu::Limits)); 8] = [
            ("bind groups", |limits| limits.max_bind_groups -= 1),
            ("bind groups and vertex buffers", |limits| {
                limits.max_bind_groups_plus_vertex_buffers -= 1
            }),
            ("uniforms per stage", |limits| {
                limits.max_uniform_buffers_per_shader_stage -= 1
            }),
            ("history binding", |limits| {
                limits.max_uniform_buffer_binding_size -= 1
            }),
            ("history buffer", |limits| limits.max_buffer_size -= 1),
            ("varying locations", |limits| {
                limits.max_inter_stage_shader_variables -= 1
            }),
            ("color attachments", |limits| {
                limits.max_color_attachments -= 1
            }),
            ("attachment byte cost", |limits| {
                limits.max_color_attachment_bytes_per_sample -= 1
            }),
        ];

        for (name, reduce) in cases {
            let mut reduced = limits.clone();
            reduce(&mut reduced);
            assert!(!supports_temporal(&reduced, formats), "{name}");
        }

        assert!(!supported_capabilities(
            &limits,
            wgpu::DownlevelFlags::empty(),
            crate::COLOR_FORMAT,
            (1920, 1080),
            formats
        ));
    }

    #[test]
    fn temporal_capability_checks_dimensions_and_rejects_noncolor_targets() {
        let limits = temporal_limits();
        let formats = temporal_formats(crate::COLOR_FORMAT);

        for size in [(1, 1), (4096, 4096)] {
            assert!(supported_capabilities(
                &limits,
                wgpu::DownlevelFlags::INDEPENDENT_BLEND,
                crate::COLOR_FORMAT,
                size,
                formats
            ));
        }
        for size in [(0, 1), (1, 0), (4097, 1), (1, 4097), (u32::MAX, u32::MAX)] {
            assert!(!supported_capabilities(
                &limits,
                wgpu::DownlevelFlags::INDEPENDENT_BLEND,
                crate::COLOR_FORMAT,
                size,
                formats
            ));
        }
        for format in [
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureFormat::Bc1RgbaUnorm,
            wgpu::TextureFormat::Rgba8Uint,
        ] {
            assert!(!supported_capabilities(
                &limits,
                wgpu::DownlevelFlags::INDEPENDENT_BLEND,
                format,
                (1920, 1080),
                temporal_formats(format)
            ));
        }
    }

    #[test]
    fn temporal_attachment_budget_includes_format_cost_and_alignment() {
        let mut limits = temporal_limits();
        let format = wgpu::TextureFormat::R8Unorm;
        let formats = temporal_formats(format);
        limits.max_color_attachment_bytes_per_sample = 7;
        assert!(supported_capabilities(
            &limits,
            wgpu::DownlevelFlags::INDEPENDENT_BLEND,
            format,
            (1920, 1080),
            formats
        ));
        limits.max_color_attachment_bytes_per_sample = 6;
        assert!(!supported_capabilities(
            &limits,
            wgpu::DownlevelFlags::INDEPENDENT_BLEND,
            format,
            (1920, 1080),
            formats
        ));
        limits.max_color_attachment_bytes_per_sample = 12;
        assert!(!supports_temporal(
            &limits,
            temporal_formats(crate::COLOR_FORMAT)
        ));
        limits.max_color_attachment_bytes_per_sample = 13;
        assert!(supports_temporal(
            &limits,
            temporal_formats(crate::COLOR_FORMAT)
        ));
    }

    #[test]
    fn temporal_capability_requires_each_attachment_usage_and_reactive_blending() {
        let limits = temporal_limits();
        let formats = temporal_formats(crate::COLOR_FORMAT);
        for index in 0..formats.len() {
            for usage in [
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                wgpu::TextureUsages::TEXTURE_BINDING,
                wgpu::TextureUsages::COPY_SRC,
            ] {
                let mut unsupported = formats;
                unsupported[index].allowed_usages.remove(usage);
                assert!(
                    !supports_temporal(&limits, unsupported),
                    "attachment {index}, usage {usage:?}"
                );
            }
        }
        let mut unsupported = formats;
        unsupported[0]
            .allowed_usages
            .remove(wgpu::TextureUsages::COPY_DST);
        assert!(!supports_temporal(&limits, unsupported));

        for (index, flag) in [
            (0, wgpu::TextureFormatFeatureFlags::FILTERABLE),
            (0, wgpu::TextureFormatFeatureFlags::BLENDABLE),
            (2, wgpu::TextureFormatFeatureFlags::BLENDABLE),
        ] {
            let mut unsupported = formats;
            unsupported[index].flags.remove(flag);
            assert!(
                !supports_temporal(&limits, unsupported),
                "attachment {index}, flag {flag:?}"
            );
        }
        let mut supported = formats;
        supported[1].flags = wgpu::TextureFormatFeatureFlags::empty();
        supported[3].flags = wgpu::TextureFormatFeatureFlags::empty();
        assert!(supports_temporal(&limits, supported));
    }

    #[test]
    fn effective_temporal_formats_respect_enabled_device_features() {
        let format = wgpu::TextureFormat::Rgba8Snorm;
        let mut adapter = format.guaranteed_format_features(wgpu::Features::empty());
        adapter
            .allowed_usages
            .insert(wgpu::TextureUsages::RENDER_ATTACHMENT);
        let downlevel = wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT;

        let core = effective_format_features(format, wgpu::Features::empty(), downlevel, adapter);
        assert!(
            !core
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        );
        let enabled = effective_format_features(
            format,
            wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
            downlevel,
            adapter,
        );
        assert!(
            enabled
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        );
        let downlevel = effective_format_features(
            format,
            wgpu::Features::empty(),
            wgpu::DownlevelFlags::empty(),
            adapter,
        );
        assert!(
            downlevel
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        );

        let format = wgpu::TextureFormat::Rgba32Float;
        let enabled = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::FLOAT32_FILTERABLE;
        let adapter = format.guaranteed_format_features(enabled);
        for features in [
            wgpu::Features::empty(),
            wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        ] {
            let core = effective_format_features(
                format,
                features,
                wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT,
                adapter,
            );
            assert!(
                !core
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
            );
        }
        assert!(
            effective_format_features(
                format,
                enabled,
                wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT,
                adapter
            )
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
        );

        let format = crate::COLOR_FORMAT;
        let mut adapter = format.guaranteed_format_features(wgpu::Features::empty());
        adapter
            .flags
            .remove(wgpu::TextureFormatFeatureFlags::BLENDABLE);
        let core = effective_format_features(
            format,
            wgpu::Features::empty(),
            wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT,
            adapter,
        );
        assert!(
            !core
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
        );
    }

    fn clip(eye: Vec3) -> DMat4 {
        let view = Matrix4::look_at_lh(eye, Vec3::ZERO, Vec3::Y);

        let projection = Matrix4::perspective_fov_lh(1.0, 1.5, 0.5, 100.0);

        Mat4::from_cols_array(&Matrix4::multiply(&view, &projection).shader_constants()).as_dmat4()
    }

    #[test]
    fn stationary_camera_has_identity_reprojection() {
        let current = clip(Vec3::new(4.0, 2.0, -8.0));

        assert!(camera_delta(current, current).abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn reprojection_maps_current_clip_position_to_previous_camera() {
        let current = clip(Vec3::new(4.0, 2.0, -8.0));

        let previous = clip(Vec3::new(3.0, 2.5, -7.0));

        let delta = camera_delta(current, previous).transpose();

        for point in [
            Vec4::new(1.0, 0.0, 2.0, 1.0),
            Vec4::new(-2.0, 1.0, 0.5, 1.0),
        ] {
            let current_clip = current.as_mat4().transpose() * point;

            let expected = previous.as_mat4().transpose() * point;

            assert!((delta * current_clip).abs_diff_eq(expected, 1e-5));
        }
    }

    #[test]
    fn transparent_temporal_pass_preserves_motion_and_accumulates_bias() {
        let color = wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba8Unorm,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };

        assert_eq!(
            targets(color.clone(), false, true),
            vec![Some(color.clone())]
        );

        for depth_write in [false, true] {
            let output = targets(color.clone(), true, depth_write);

            assert_eq!(output.len(), 3);

            assert_eq!(output[0], Some(color.clone()));

            let motion = output[1].as_ref().unwrap();

            assert_eq!(motion.format, MOTION_FORMAT);

            assert_eq!(motion.blend, None);

            assert_eq!(
                motion.write_mask,
                if depth_write {
                    wgpu::ColorWrites::ALL
                } else {
                    wgpu::ColorWrites::empty()
                }
            );

            let bias = output[2].as_ref().unwrap();

            assert_eq!(bias.format, BIAS_FORMAT);

            assert_eq!(bias.write_mask, wgpu::ColorWrites::ALL);

            let blend = bias.blend.unwrap();

            assert_eq!(blend.color.operation, wgpu::BlendOperation::Max);

            assert_eq!(blend.alpha.operation, wgpu::BlendOperation::Max);
        }
    }

    #[test]
    fn temporal_coverage_distinguishes_alpha_additive_and_opaque_passes() {
        let alpha = wgpu::BlendState::ALPHA_BLENDING;

        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };

        assert!(
            fragment_options(false, Some(alpha), false)
                .constants
                .is_empty()
        );

        for (blend, depth_write, coverage) in [
            (Some(alpha), false, 1.0),
            (Some(additive), false, 2.0),
            (None, false, 0.0),
            (Some(alpha), true, 0.0),
            (Some(additive), true, 0.0),
        ] {
            assert_eq!(
                fragment_options(true, blend, depth_write).constants,
                &[("temporal_coverage", coverage)]
            );
        }
    }
}

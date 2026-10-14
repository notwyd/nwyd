use crate::{Gpu, world::FrameUniforms};
use bytemuck::{Pod, Zeroable};
use glam::{DMat4, Mat4};

pub const MOTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

pub const BIAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

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

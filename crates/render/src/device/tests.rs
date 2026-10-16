use super::backend::{
    adapter_rank, first_available, supports_renderer, supports_surface_formats, unique_backends,
};
use super::{Gpu, REQUIRED_FEATURES, RendererBackend};

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct ThreadWake(std::thread::Thread);
    impl std::task::Wake for ThreadWake {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(ThreadWake(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(result) => return result,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

fn color_format() -> wgpu::TextureFormatFeatures {
    wgpu::TextureFormatFeatures {
        allowed_usages: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        flags: wgpu::TextureFormatFeatureFlags::FILTERABLE
            | wgpu::TextureFormatFeatureFlags::BLENDABLE,
    }
}

fn depth_format() -> wgpu::TextureFormatFeatures {
    wgpu::TextureFormatFeatures {
        allowed_usages: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        flags: wgpu::TextureFormatFeatureFlags::empty(),
    }
}

#[test]
fn automatic_ranking_prefers_hardware_then_vulkan() {
    use RendererBackend::{Dx12, Gl, Vulkan};
    use wgpu::DeviceType::{Cpu, DiscreteGpu, IntegratedGpu};
    let rank = |backend, device| adapter_rank(backend, device, None);
    assert!(rank(Dx12, DiscreteGpu) < rank(Vulkan, IntegratedGpu));
    assert!(rank(Gl, IntegratedGpu) < rank(Vulkan, Cpu));
    assert!(rank(Vulkan, DiscreteGpu) < rank(Dx12, DiscreteGpu));
    assert!(adapter_rank(Gl, Cpu, Some(Gl)) < adapter_rank(Vulkan, DiscreteGpu, Some(Gl)));
}

#[test]
fn renderer_options_are_deterministic_and_unique() {
    use RendererBackend::{Gl, Vulkan};
    assert_eq!(unique_backends([Gl, Vulkan, Gl, Vulkan]), vec![Vulkan, Gl]);
    assert_eq!(unique_backends([Vulkan, Gl]), vec![Vulkan, Gl]);
}

#[test]
fn failed_devices_fall_back_once_and_stop_after_success() {
    let mut attempts = Vec::new();
    let result = block_on(first_available(
        &["requested", "fallback", "unused"],
        |candidate| {
            attempts.push(*candidate);
            std::future::ready(if *candidate == "fallback" {
                Ok(7)
            } else {
                Err(*candidate)
            })
        },
    ));
    assert_eq!(result, Ok((1, 7)));
    assert_eq!(attempts, ["requested", "fallback"]);
    assert_eq!(
        block_on(first_available(&[1, 2], |candidate| std::future::ready(
            Err::<(), _>(*candidate)
        ))),
        Err(vec![1, 2])
    );
}

#[test]
fn surface_requires_a_compatible_eight_bit_format() {
    use wgpu::TextureFormat::*;
    assert!(supports_surface_formats(&[Rgba16Float, Bgra8Unorm]));
    assert!(supports_surface_formats(&[Rgba8UnormSrgb]));
    assert!(!supports_surface_formats(&[Rgba16Float]));
    assert!(!supports_surface_formats(&[]));
}

#[test]
fn renderer_rejects_missing_features_and_downlevel_draw_capabilities() {
    let limits = wgpu::Limits::default();
    let mut downlevel = wgpu::DownlevelCapabilities::default();
    let supports = |features, downlevel: &wgpu::DownlevelCapabilities| {
        supports_renderer(features, &limits, downlevel, color_format(), depth_format())
    };
    assert!(supports(REQUIRED_FEATURES, &downlevel));
    assert!(!supports(wgpu::Features::empty(), &downlevel));
    for flag in [
        wgpu::DownlevelFlags::BASE_VERTEX,
        wgpu::DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES,
    ] {
        downlevel.flags.remove(flag);
        assert!(!supports(REQUIRED_FEATURES, &downlevel));
        downlevel.flags.insert(flag);
    }
}

#[test]
fn renderer_requires_base_pipeline_limits_and_palette_capacity() {
    let supports = |limits: &wgpu::Limits| {
        supports_renderer(
            REQUIRED_FEATURES,
            limits,
            &wgpu::DownlevelCapabilities::default(),
            color_format(),
            depth_format(),
        )
    };
    let palette_size = std::mem::size_of::<crate::skin::BonePalette>() as u64;
    let mut limits = wgpu::Limits::default();
    limits.max_bind_groups = 4;
    limits.max_bind_groups_plus_vertex_buffers = 5;
    limits.max_bindings_per_bind_group = 4;
    limits.max_dynamic_uniform_buffers_per_pipeline_layout = 2;
    limits.max_uniform_buffers_per_shader_stage = 3;
    limits.max_uniform_buffer_binding_size = palette_size;
    limits.min_uniform_buffer_offset_alignment = 4096;
    limits.max_buffer_size =
        palette_size.div_ceil(4096) * 4096 * u64::from(crate::skin::PALETTE_CAPACITY);
    limits.max_vertex_buffers = 1;
    limits.max_vertex_attributes = 5;
    limits.max_vertex_buffer_array_stride = crate::skin::SkinVertexGpu::STRIDE as u32;
    limits.max_sampled_textures_per_shader_stage = 2;
    limits.max_samplers_per_shader_stage = 2;
    limits.max_inter_stage_shader_variables = 4;
    limits.max_texture_array_layers = 1;
    limits.max_color_attachments = 1;
    limits.max_color_attachment_bytes_per_sample = 8;
    assert!(supports(&limits));

    macro_rules! rejects_smaller {
        ($($field:ident),+ $(,)?) => {
            $(
                let mut reduced = limits.clone();
                reduced.$field -= 1;
                assert!(!supports(&reduced), stringify!($field));
            )+
        };
    }
    rejects_smaller!(
        max_bind_groups,
        max_bind_groups_plus_vertex_buffers,
        max_bindings_per_bind_group,
        max_dynamic_uniform_buffers_per_pipeline_layout,
        max_uniform_buffers_per_shader_stage,
        max_uniform_buffer_binding_size,
        max_buffer_size,
        max_vertex_buffers,
        max_vertex_attributes,
        max_vertex_buffer_array_stride,
        max_sampled_textures_per_shader_stage,
        max_samplers_per_shader_stage,
        max_inter_stage_shader_variables,
        max_texture_array_layers,
        max_color_attachments,
        max_color_attachment_bytes_per_sample,
    );
}

#[test]
fn renderer_requires_usable_color_and_depth_formats() {
    let supports = |color, depth| {
        supports_renderer(
            REQUIRED_FEATURES,
            &wgpu::Limits::default(),
            &wgpu::DownlevelCapabilities::default(),
            color,
            depth,
        )
    };
    for usage in [
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        wgpu::TextureUsages::TEXTURE_BINDING,
        wgpu::TextureUsages::COPY_SRC,
        wgpu::TextureUsages::COPY_DST,
    ] {
        let mut color = color_format();
        color.allowed_usages.remove(usage);
        assert!(!supports(color, depth_format()), "color {usage:?}");
        if usage != wgpu::TextureUsages::COPY_DST {
            let mut depth = depth_format();
            depth.allowed_usages.remove(usage);
            assert!(!supports(color_format(), depth), "depth {usage:?}");
        }
    }
    for flag in [
        wgpu::TextureFormatFeatureFlags::FILTERABLE,
        wgpu::TextureFormatFeatureFlags::BLENDABLE,
    ] {
        let mut color = color_format();
        color.flags.remove(flag);
        assert!(!supports(color, depth_format()), "color {flag:?}");
    }
}

#[test]
fn base_renderer_does_not_require_optional_motion_capabilities() {
    let mut downlevel = wgpu::DownlevelCapabilities::default();
    downlevel.flags.remove(
        wgpu::DownlevelFlags::INDEPENDENT_BLEND
            | wgpu::DownlevelFlags::COMPUTE_SHADERS
            | wgpu::DownlevelFlags::DEPTH_TEXTURE_AND_BUFFER_COPIES,
    );
    let mut limits = wgpu::Limits::default();
    limits.max_bind_groups = 8;
    limits.max_color_attachments = 1;
    limits.max_inter_stage_shader_variables = 4;
    assert!(supports_renderer(
        REQUIRED_FEATURES,
        &limits,
        &downlevel,
        color_format(),
        depth_format()
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a graphics adapter"]
fn available_renderer_selection_and_unavailable_preference_fallback() {
    let instance = Gpu::instance();
    let automatic = block_on(Gpu::open(instance.clone(), None)).expect("automatic renderer");
    assert!(
        automatic
            .available_backends()
            .contains(&automatic.backend())
    );
    let selected = automatic.backend();
    let unavailable = [
        RendererBackend::Vulkan,
        RendererBackend::Dx12,
        RendererBackend::Metal,
        RendererBackend::Gl,
        RendererBackend::BrowserWebGpu,
    ]
    .into_iter()
    .find(|backend| !automatic.available_backends().contains(backend));
    drop(automatic);
    let explicit = block_on(Gpu::open_with_backend(
        instance.clone(),
        None,
        Some(selected),
    ))
    .expect("selected renderer");
    assert_eq!(explicit.backend(), selected);
    assert!(explicit.available_backends().contains(&selected));
    drop(explicit);
    if let Some(unavailable) = unavailable {
        let fallback = block_on(Gpu::open_with_backend(instance, None, Some(unavailable)))
            .expect("unavailable renderer fallback");
        assert_ne!(fallback.backend(), unavailable);
        assert!(fallback.available_backends().contains(&fallback.backend()));
        assert!(!fallback.available_backends().contains(&unavailable));
    }
}

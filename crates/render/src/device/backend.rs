use super::REQUIRED_FEATURES;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RendererBackend {
    Vulkan,
    Dx12,
    Metal,
    Gl,
    BrowserWebGpu,
}

impl RendererBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vulkan => "Vulkan",
            Self::Dx12 => "DirectX 12",
            Self::Metal => "Metal",
            Self::Gl => "OpenGL",
            Self::BrowserWebGpu => "WebGPU",
        }
    }

    fn from_wgpu(backend: wgpu::Backend) -> Option<Self> {
        match backend {
            wgpu::Backend::Vulkan => Some(Self::Vulkan),
            wgpu::Backend::Dx12 => Some(Self::Dx12),
            wgpu::Backend::Metal => Some(Self::Metal),
            wgpu::Backend::Gl => Some(Self::Gl),
            wgpu::Backend::BrowserWebGpu => Some(Self::BrowserWebGpu),
            wgpu::Backend::Noop => None,
        }
    }
}

pub(super) struct Candidate {
    pub adapter: wgpu::Adapter,
    pub backend: RendererBackend,
    info: wgpu::AdapterInfo,
}

impl Candidate {
    pub fn new(adapter: wgpu::Adapter, surface: Option<&wgpu::Surface<'_>>) -> Option<Self> {
        let info = adapter.get_info();
        let backend = RendererBackend::from_wgpu(info.backend)?;
        if let Some(surface) = surface {
            if !adapter.is_surface_supported(surface)
                || !supports_surface_formats(&surface.get_capabilities(&adapter).formats)
            {
                return None;
            }
        }
        if !supports_renderer(
            adapter.features(),
            &adapter.limits(),
            &adapter.get_downlevel_capabilities(),
            adapter.get_texture_format_features(crate::target::COLOR_FORMAT),
            adapter.get_texture_format_features(crate::target::DEPTH_FORMAT),
        ) {
            tracing::debug!(adapter = %info.name, ?backend, "renderer capabilities unavailable");
            return None;
        }
        Some(Self {
            adapter,
            backend,
            info,
        })
    }

    pub fn rank(
        &self,
        preferred: Option<RendererBackend>,
    ) -> (bool, u8, RendererBackend, &str, u32, u32) {
        let (preference, device_type, backend) =
            adapter_rank(self.backend, self.info.device_type, preferred);
        (
            preference,
            device_type,
            backend,
            &self.info.name,
            self.info.vendor,
            self.info.device,
        )
    }
}

pub(super) fn adapter_rank(
    backend: RendererBackend,
    device_type: wgpu::DeviceType,
    preferred: Option<RendererBackend>,
) -> (bool, u8, RendererBackend) {
    let device_type = match device_type {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    };
    (
        preferred.is_some_and(|preferred| preferred != backend),
        device_type,
        backend,
    )
}

pub(super) fn unique_backends(
    backends: impl IntoIterator<Item = RendererBackend>,
) -> Vec<RendererBackend> {
    let mut backends = backends.into_iter().collect::<Vec<_>>();
    backends.sort_unstable();
    backends.dedup();
    backends
}

pub(super) fn supports_surface_formats(formats: &[wgpu::TextureFormat]) -> bool {
    formats.iter().any(|format| {
        matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        )
    })
}

pub(super) fn supports_renderer(
    features: wgpu::Features,
    limits: &wgpu::Limits,
    downlevel: &wgpu::DownlevelCapabilities,
    color: wgpu::TextureFormatFeatures,
    depth: wgpu::TextureFormatFeatures,
) -> bool {
    let palette_size = std::mem::size_of::<crate::skin::BonePalette>() as u64;
    let alignment = u64::from(limits.min_uniform_buffer_offset_alignment).max(1);
    let palette_buffer_size =
        palette_size.div_ceil(alignment) * alignment * u64::from(crate::skin::PALETTE_CAPACITY);
    let uniform_size = palette_size.max(std::mem::size_of::<crate::world::FrameUniforms>() as u64);
    features.contains(REQUIRED_FEATURES)
        && downlevel.flags.contains(
            wgpu::DownlevelFlags::BASE_VERTEX
                | wgpu::DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES,
        )
        && limits.max_bind_groups >= 4
        && limits.max_bind_groups_plus_vertex_buffers >= 5
        && limits.max_bindings_per_bind_group >= 4
        && limits.max_dynamic_uniform_buffers_per_pipeline_layout >= 2
        && limits.max_uniform_buffers_per_shader_stage >= 3
        && u64::from(limits.max_uniform_buffer_binding_size) >= uniform_size
        && limits.max_buffer_size >= palette_buffer_size
        && limits.max_vertex_buffers >= 1
        && limits.max_vertex_attributes >= 5
        && u64::from(limits.max_vertex_buffer_array_stride) >= crate::skin::SkinVertexGpu::STRIDE
        && limits.max_sampled_textures_per_shader_stage >= 2
        && limits.max_samplers_per_shader_stage >= 2
        && limits.max_inter_stage_shader_variables >= 4
        && limits.max_texture_array_layers >= 1
        && limits.max_color_attachments >= 1
        && limits.max_color_attachment_bytes_per_sample >= 8
        && color.allowed_usages.contains(
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        )
        && color.flags.contains(
            wgpu::TextureFormatFeatureFlags::FILTERABLE
                | wgpu::TextureFormatFeatureFlags::BLENDABLE,
        )
        && depth.allowed_usages.contains(
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        )
}

pub(super) async fn first_available<T, U, E, F, Fut>(
    candidates: &[T],
    mut open: F,
) -> Result<(usize, U), Vec<E>>
where
    F: FnMut(&T) -> Fut,
    Fut: std::future::Future<Output = Result<U, E>>,
{
    let mut errors = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        match open(candidate).await {
            Ok(device) => return Ok((index, device)),
            Err(error) => errors.push(error),
        }
    }
    Err(errors)
}

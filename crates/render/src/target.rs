use crate::device::Gpu;

pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth16Unorm;

pub const DEPTH_CLEAR: f32 = 1.0;

pub struct DepthBuffer {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl DepthBuffer {
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> DepthBuffer {
        Self::with_format(gpu, width, height, DEPTH_FORMAT)
    }

    pub fn with_format(
        gpu: &Gpu,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> DepthBuffer {
        assert!(
            width > 0 && height > 0,
            "a {width}x{height} depth buffer has no pixels"
        );

        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: crate::gpu_label("depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        DepthBuffer {
            texture,
            view,
            width,
            height,
        }
    }

    pub fn fits(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }

    pub fn attachment(&self) -> wgpu::RenderPassDepthStencilAttachment<'_> {
        wgpu::RenderPassDepthStencilAttachment {
            view: &self.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(DEPTH_CLEAR),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }
    }
}

pub struct Offscreen {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

impl Offscreen {
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Offscreen {
        Offscreen::with_format(gpu, width, height, COLOR_FORMAT)
    }

    pub fn with_format(
        gpu: &Gpu,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Offscreen {
        assert!(
            width > 0 && height > 0,
            "a {width}x{height} target has no pixels"
        );

        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: crate::gpu_label("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Offscreen {
            texture,
            view,
            width,
            height,
            format,
        }
    }
}

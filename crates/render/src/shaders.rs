mod kind;

pub use kind::{ALL as SHADERS, Shader};

pub(crate) mod source;

pub fn module(device: &wgpu::Device, shader: Shader) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: crate::gpu_label("shader"),
        source: wgpu::ShaderSource::Wgsl(source::assemble(shader).into()),
    })
}

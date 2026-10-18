use render::{
    Gpu,
    light_shafts::{LightShaft, LightShaftRenderer, LightShaftTexture},
    world::FrameUniforms,
};

pub(super) struct LightShaftFrame {
    renderer: LightShaftRenderer,
    views: Vec<wgpu::TextureView>,
    textures: Vec<Option<LightShaftTexture>>,
    prepared: bool,
}

impl LightShaftFrame {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        Self {
            renderer: LightShaftRenderer::new(gpu, format),
            views: Vec::new(),
            textures: Vec::new(),
            prepared: false,
        }
    }

    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        frame: &FrameUniforms,
        shafts: &[LightShaft],
        depth: &wgpu::TextureView,
        views: &[wgpu::TextureView],
    ) {
        let mut bound = Vec::with_capacity(shafts.len());
        for shaft in shafts {
            let Some(view) = views.get(shaft.texture) else {
                continue;
            };
            let texture = self
                .views
                .iter()
                .position(|cached| cached == view)
                .unwrap_or_else(|| {
                    let index = self.views.len();
                    self.textures
                        .push(Some(self.renderer.bind_texture(gpu, view)));
                    self.views.push(view.clone());
                    index
                });
            let mut shaft = *shaft;
            shaft.texture = texture;
            bound.push(shaft);
        }
        self.renderer.prepare(gpu, frame, &bound, depth, [0.0; 2]);
        self.prepared = self.renderer.draw_count() != 0;
    }

    pub fn ready(&self) -> bool {
        self.prepared
    }

    pub fn clear(&mut self) {
        self.prepared = false;
    }

    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        reactive: Option<&wgpu::TextureView>,
    ) {
        if self.prepared {
            self.renderer
                .encode(encoder, target, &self.textures, reactive);
        }
    }
}

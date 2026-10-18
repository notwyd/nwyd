#[cfg(test)]
mod tests;

#[allow(dead_code)]
pub mod d3d {

    pub const ZENABLE: u32 = 7;

    pub const FILLMODE: u32 = 8;

    pub const SHADEMODE: u32 = 9;

    pub const ZWRITEENABLE: u32 = 14;

    pub const ALPHATESTENABLE: u32 = 15;

    pub const SRCBLEND: u32 = 19;

    pub const DESTBLEND: u32 = 20;

    pub const CULLMODE: u32 = 22;

    pub const ZFUNC: u32 = 23;

    pub const ALPHAREF: u32 = 24;

    pub const ALPHAFUNC: u32 = 25;

    pub const DITHERENABLE: u32 = 26;

    pub const ALPHABLENDENABLE: u32 = 27;

    pub const FOGENABLE: u32 = 28;

    pub const SPECULARENABLE: u32 = 29;

    pub const FOGCOLOR: u32 = 34;

    pub const FOGSTART: u32 = 36;

    pub const FOGEND: u32 = 37;

    pub const RANGEFOGENABLE: u32 = 48;

    pub const STENCILENABLE: u32 = 52;

    pub const CLIPPING: u32 = 136;

    pub const LIGHTING: u32 = 137;

    pub const AMBIENT: u32 = 139;

    pub const FOGVERTEXMODE: u32 = 140;

    pub const COLORVERTEX: u32 = 141;

    pub const NORMALIZENORMALS: u32 = 143;

    pub const DIFFUSEMATERIALSOURCE: u32 = 145;

    pub const SPECULARMATERIALSOURCE: u32 = 146;

    pub const AMBIENTMATERIALSOURCE: u32 = 147;

    pub const EMISSIVEMATERIALSOURCE: u32 = 148;

    pub const MULTISAMPLEANTIALIAS: u32 = 161;

    pub const VERTEXBLEND: u32 = 151;

    pub const INDEXEDVERTEXBLENDENABLE: u32 = 167;

    pub const ANTIALIASEDLINEENABLE: u32 = 176;

    pub const BLENDOP: u32 = 171;

    pub const BLENDFACTOR: u32 = 193;

    pub const CMP_LESSEQUAL: u32 = 4;

    pub const CMP_GREATEREQUAL: u32 = 7;

    pub const BLEND_ZERO: u32 = 1;

    pub const BLEND_ONE: u32 = 2;

    pub const BLEND_SRCCOLOR: u32 = 3;

    pub const BLEND_INVSRCCOLOR: u32 = 4;

    pub const BLEND_SRCALPHA: u32 = 5;

    pub const BLEND_INVSRCALPHA: u32 = 6;

    pub const BLEND_DESTALPHA: u32 = 7;

    pub const BLEND_DESTCOLOR: u32 = 9;

    pub const BLEND_INVDESTCOLOR: u32 = 10;

    pub const BLEND_BLENDFACTOR: u32 = 14;

    pub const BLEND_INVBLENDFACTOR: u32 = 15;

    pub const FOG_LINEAR: u32 = 3;

    pub const MCS_MATERIAL: u32 = 0;

    pub const MCS_COLOR1: u32 = 1;

    pub const CULL_CCW: u32 = 3;

    pub const CULL_CW: u32 = 2;

    pub const TEXF_NONE: u32 = 0;

    pub const TEXF_POINT: u32 = 1;

    pub const TEXF_LINEAR: u32 = 2;

    pub const TADDRESS_WRAP: u32 = 1;

    pub const TADDRESS_CLAMP: u32 = 3;

    pub const TOP_DISABLE: u32 = 1;

    pub const TOP_SELECTARG1: u32 = 2;

    pub const TOP_MODULATE: u32 = 4;

    pub const TA_DIFFUSE: u32 = 0;

    pub const TA_TEXTURE: u32 = 2;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderMode {
    Unlit3d = 0,
    Lit3d = 1,
    Ui2dPoint = 2,
    Ui2dLinear = 3,
    SControl2d = 100,
    LensFlare2d = 102,
    NuiPanel2d = 101,
    NuiHighlight2d = 103,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorOp {
    Modulate,
    SelectTextureRgb,
    AddSigned,
}

impl ColorOp {
    pub fn uniform(self) -> f32 {
        match self {
            ColorOp::Modulate => 0.0,
            ColorOp::SelectTextureRgb => 1.0,
            ColorOp::AddSigned => 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlphaTest {
    pub reference: u32,
    pub func: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    Wrap,
    Clamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub mode: RenderMode,
    pub blend: bool,
    pub src_blend: u32,
    pub dst_blend: u32,
    pub depth: bool,
    pub depth_write: bool,
    pub depth_func: u32,
    pub alpha_test: Option<AlphaTest>,
    pub lighting: bool,
    pub fog: bool,
    pub filter: u32,
    pub mip_filter: u32,
    pub color_op: ColorOp,
    pub address: Address,
}

impl Preset {
    pub const UI_POINT: Preset = Preset {
        mode: RenderMode::Ui2dPoint,
        blend: true,
        src_blend: d3d::BLEND_SRCALPHA,
        dst_blend: d3d::BLEND_INVSRCALPHA,
        depth: false,
        depth_write: false,
        depth_func: d3d::CMP_LESSEQUAL,
        alpha_test: Some(AlphaTest {
            reference: 8,
            func: d3d::CMP_GREATEREQUAL,
        }),
        lighting: false,
        fog: false,
        filter: d3d::TEXF_POINT,
        mip_filter: d3d::TEXF_POINT,
        color_op: ColorOp::Modulate,
        address: Address::Wrap,
    };

    pub const UI_LINEAR: Preset = Preset {
        mode: RenderMode::Ui2dLinear,
        blend: false,
        src_blend: d3d::BLEND_SRCALPHA,
        dst_blend: d3d::BLEND_INVSRCALPHA,
        depth: false,
        depth_write: false,
        depth_func: d3d::CMP_LESSEQUAL,
        alpha_test: Some(AlphaTest {
            reference: 8,
            func: d3d::CMP_GREATEREQUAL,
        }),
        lighting: false,
        fog: false,
        filter: d3d::TEXF_LINEAR,
        mip_filter: d3d::TEXF_LINEAR,
        color_op: ColorOp::Modulate,
        address: Address::Wrap,
    };

    pub const UNLIT_3D: Preset = Preset {
        mode: RenderMode::Unlit3d,
        blend: false,
        src_blend: d3d::BLEND_SRCALPHA,
        dst_blend: d3d::BLEND_DESTALPHA,
        depth: true,
        depth_write: true,
        depth_func: d3d::CMP_LESSEQUAL,
        alpha_test: None,
        lighting: false,
        fog: false,
        filter: d3d::TEXF_LINEAR,
        mip_filter: d3d::TEXF_POINT,
        color_op: ColorOp::SelectTextureRgb,
        address: Address::Wrap,
    };

    pub const LIT_3D: Preset = Preset {
        mode: RenderMode::Lit3d,
        blend: true,
        src_blend: d3d::BLEND_ONE,
        dst_blend: d3d::BLEND_INVSRCALPHA,
        depth: true,
        depth_write: true,
        depth_func: d3d::CMP_LESSEQUAL,
        alpha_test: Some(AlphaTest {
            reference: Preset::ALPHAREF_NVIDIA_32BPP,
            func: d3d::CMP_GREATEREQUAL,
        }),
        lighting: true,
        fog: true,
        filter: d3d::TEXF_LINEAR,
        mip_filter: d3d::TEXF_LINEAR,
        color_op: ColorOp::Modulate,
        address: Address::Wrap,
    };

    pub const ALPHAREF_NVIDIA_32BPP: u32 = 0xFF00_0000;

    pub const ALPHAREF_OTHER_32BPP: u32 = 0xDD;

    pub fn lit_3d(vendor: crate::quirk::VendorClass) -> Preset {
        let reference = match vendor {
            crate::quirk::VendorClass::Nvidia | crate::quirk::VendorClass::NvidiaMx => {
                Preset::ALPHAREF_NVIDIA_32BPP
            }
            _ => Preset::ALPHAREF_OTHER_32BPP,
        };

        Preset {
            alpha_test: Some(AlphaTest {
                reference,
                func: d3d::CMP_GREATEREQUAL,
            }),
            ..Preset::LIT_3D
        }
    }

    pub const S_CONTROL_2D: Preset = Preset {
        mode: RenderMode::SControl2d,
        blend: true,
        src_blend: d3d::BLEND_SRCALPHA,
        dst_blend: d3d::BLEND_INVSRCALPHA,
        depth: false,
        depth_write: false,
        depth_func: d3d::CMP_LESSEQUAL,
        alpha_test: None,
        lighting: false,
        fog: false,
        filter: d3d::TEXF_LINEAR,
        mip_filter: d3d::TEXF_POINT,
        color_op: ColorOp::SelectTextureRgb,
        address: Address::Wrap,
    };

    pub const LENS_FLARE_2D: Preset = Preset {
        mode: RenderMode::LensFlare2d,
        blend: true,
        src_blend: d3d::BLEND_SRCCOLOR,
        dst_blend: d3d::BLEND_DESTALPHA,
        depth: false,
        depth_write: false,
        depth_func: d3d::CMP_LESSEQUAL,
        alpha_test: None,
        lighting: false,
        fog: false,
        filter: d3d::TEXF_LINEAR,
        mip_filter: d3d::TEXF_POINT,
        color_op: ColorOp::Modulate,
        address: Address::Wrap,
    };

    pub const NUI_PANEL_2D: Preset = Preset {
        mode: RenderMode::NuiPanel2d,
        blend: true,
        src_blend: d3d::BLEND_SRCALPHA,
        dst_blend: d3d::BLEND_INVSRCALPHA,
        depth: false,
        depth_write: false,
        depth_func: d3d::CMP_LESSEQUAL,
        alpha_test: Some(AlphaTest {
            reference: 0,
            func: d3d::CMP_GREATEREQUAL,
        }),
        lighting: false,
        fog: false,
        filter: d3d::TEXF_LINEAR,
        mip_filter: d3d::TEXF_NONE,
        color_op: ColorOp::Modulate,
        address: Address::Clamp,
    };

    pub const fn for_mode(mode: RenderMode) -> Option<Preset> {
        match mode {
            RenderMode::Ui2dPoint => Some(Preset::UI_POINT),
            RenderMode::Ui2dLinear => Some(Preset::UI_LINEAR),
            RenderMode::SControl2d => Some(Preset::S_CONTROL_2D),
            RenderMode::NuiPanel2d => Some(Preset::NUI_PANEL_2D),
            RenderMode::NuiHighlight2d => Some(Preset {
                mode,
                src_blend: d3d::BLEND_ONE,
                dst_blend: d3d::BLEND_ONE,
                ..Preset::NUI_PANEL_2D
            }),
            RenderMode::LensFlare2d => Some(Preset::LENS_FLARE_2D),
            RenderMode::Unlit3d => Some(Preset::UNLIT_3D),
            RenderMode::Lit3d => Some(Preset::LIT_3D),
        }
    }

    pub fn blend_state(&self) -> Option<wgpu::BlendState> {
        let factor = |d3d| match d3d {
            d3d::BLEND_DESTALPHA => wgpu::BlendFactor::One,
            other => blend_factor(other),
        };

        let component = wgpu::BlendComponent {
            src_factor: factor(self.src_blend),
            dst_factor: factor(self.dst_blend),
            operation: wgpu::BlendOperation::Add,
        };

        self.blend.then_some(wgpu::BlendState {
            color: component,
            alpha: component,
        })
    }

    pub fn depth_state(&self, format: wgpu::TextureFormat) -> Option<wgpu::DepthStencilState> {
        self.depth.then(|| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(self.depth_write),
            depth_compare: Some(depth_compare(self.depth_func)),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        })
    }

    pub fn sampler_descriptor(&self) -> wgpu::SamplerDescriptor<'static> {
        let f = |v: u32| {
            if v == d3d::TEXF_LINEAR {
                wgpu::FilterMode::Linear
            } else {
                wgpu::FilterMode::Nearest
            }
        };

        let address = match self.address {
            Address::Wrap => wgpu::AddressMode::Repeat,
            Address::Clamp => wgpu::AddressMode::ClampToEdge,
        };

        wgpu::SamplerDescriptor {
            label: crate::gpu_label("preset"),
            address_mode_u: address,
            address_mode_v: address,
            address_mode_w: address,
            mag_filter: f(self.filter),
            min_filter: f(self.filter),
            mipmap_filter: if self.mip_filter == d3d::TEXF_LINEAR {
                wgpu::MipmapFilterMode::Linear
            } else {
                wgpu::MipmapFilterMode::Nearest
            },
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }

    pub fn primitive_state(&self) -> wgpu::PrimitiveState {
        wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Cw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        }
    }

    pub fn material_sampler_descriptor(
        &self,
        capabilities: wgpu::DownlevelFlags,
    ) -> wgpu::SamplerDescriptor<'static> {
        let mut descriptor = self.sampler_descriptor();

        if matches!(self.mode, RenderMode::Lit3d | RenderMode::Unlit3d) {
            descriptor.mag_filter = wgpu::FilterMode::Linear;
            descriptor.min_filter = wgpu::FilterMode::Linear;
            descriptor.mipmap_filter = wgpu::MipmapFilterMode::Linear;
            descriptor.anisotropy_clamp =
                if capabilities.contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING) {
                    16
                } else {
                    1
                };
        }

        descriptor
    }

    pub fn color_op_uniform(&self) -> f32 {
        self.color_op.uniform()
    }

    pub fn alpha_reference_normalised(&self) -> f32 {
        match self.alpha_test {
            Some(t) => (t.reference & 0xFF) as f32 / 255.0,
            None => 0.0,
        }
    }
}

pub fn blend_factor(d3d_blend: u32) -> wgpu::BlendFactor {
    match blend_factor_checked(d3d_blend) {
        Some(factor) => factor,
        None => {
            tracing::info!(
                "render: D3DBLEND {d3d_blend} is not a supported blend mode; \
                 falling back to D3DBLEND_ONE"
            );

            wgpu::BlendFactor::One
        }
    }
}

pub fn blend_factor_checked(d3d_blend: u32) -> Option<wgpu::BlendFactor> {
    Some(match d3d_blend {
        d3d::BLEND_ZERO => wgpu::BlendFactor::Zero,
        d3d::BLEND_ONE => wgpu::BlendFactor::One,
        d3d::BLEND_SRCCOLOR => wgpu::BlendFactor::Src,
        d3d::BLEND_INVSRCCOLOR => wgpu::BlendFactor::OneMinusSrc,
        d3d::BLEND_SRCALPHA => wgpu::BlendFactor::SrcAlpha,
        d3d::BLEND_INVSRCALPHA => wgpu::BlendFactor::OneMinusSrcAlpha,
        d3d::BLEND_DESTALPHA => wgpu::BlendFactor::DstAlpha,
        d3d::BLEND_DESTCOLOR => wgpu::BlendFactor::Dst,
        d3d::BLEND_INVDESTCOLOR => wgpu::BlendFactor::OneMinusDst,
        d3d::BLEND_BLENDFACTOR => wgpu::BlendFactor::Constant,
        d3d::BLEND_INVBLENDFACTOR => wgpu::BlendFactor::OneMinusConstant,
        _ => return None,
    })
}

fn depth_compare(d3d_cmp: u32) -> wgpu::CompareFunction {
    match d3d_cmp {
        d3d::CMP_LESSEQUAL => wgpu::CompareFunction::LessEqual,
        d3d::CMP_GREATEREQUAL => wgpu::CompareFunction::GreaterEqual,
        other => unreachable!("unsupported depth comparison {other}"),
    }
}

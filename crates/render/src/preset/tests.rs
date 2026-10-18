use super::*;

#[test]
fn material_samplers_filter_mips_and_gate_anisotropy_on_adapter_support() {
    for preset in [Preset::LIT_3D, Preset::UNLIT_3D] {
        for (flags, expected) in [
            (wgpu::DownlevelFlags::empty(), 1),
            (wgpu::DownlevelFlags::ANISOTROPIC_FILTERING, 16),
        ] {
            let sampler = preset.material_sampler_descriptor(flags);
            assert_eq!(sampler.anisotropy_clamp, expected);
            assert_eq!(sampler.mag_filter, wgpu::FilterMode::Linear);
            assert_eq!(sampler.min_filter, wgpu::FilterMode::Linear);
            assert_eq!(sampler.mipmap_filter, wgpu::MipmapFilterMode::Linear);
            assert!(sampler.lod_max_clamp >= 16.0);
        }
    }
}

#[test]
fn ui_filters_remain_unchanged_with_anisotropy_available() {
    for preset in [
        Preset::UI_POINT,
        Preset::UI_LINEAR,
        Preset::NUI_PANEL_2D,
        Preset::S_CONTROL_2D,
    ] {
        let before = preset.sampler_descriptor();
        let after = preset.material_sampler_descriptor(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING);
        assert_eq!(after.anisotropy_clamp, 1);
        assert_eq!(after.mag_filter, before.mag_filter);
        assert_eq!(after.min_filter, before.min_filter);
        assert_eq!(after.mipmap_filter, before.mipmap_filter);
    }
}

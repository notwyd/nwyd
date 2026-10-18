#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Tuning {
    pub intensity: f32,
    pub tone: f32,
    pub structure: f32,
    pub skin: f32,
    pub sharpness: f32,
    pub auto_mask: u32,
    pub style: u32,
    pub passes: u32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            intensity: 2.0,
            tone: 2.0,
            structure: 2.05,
            skin: -1.0,
            sharpness: 0.25,
            auto_mask: 1,
            style: 2,
            passes: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_all_model_controls_and_both_passes() {
        let tuning = Tuning::default();
        assert_eq!(tuning.intensity, 2.0);
        assert_eq!(tuning.tone, 2.0);
        assert_eq!(tuning.structure, 2.05);
        assert_eq!(tuning.skin, -1.0);
        assert_eq!(tuning.sharpness, 0.25);
        assert_eq!(tuning.auto_mask, 1);
        assert_eq!(tuning.style, 2);
        assert_eq!(tuning.passes, 2);
    }

    #[test]
    fn tuning_has_the_fixed_native_abi_layout() {
        assert_eq!(std::mem::size_of::<Tuning>(), 32);
        assert_eq!(std::mem::align_of::<Tuning>(), 4);
        assert_eq!(std::mem::offset_of!(Tuning, sharpness), 16);
        assert_eq!(std::mem::offset_of!(Tuning, auto_mask), 20);
        assert_eq!(std::mem::offset_of!(Tuning, style), 24);
        assert_eq!(std::mem::offset_of!(Tuning, passes), 28);
    }
}

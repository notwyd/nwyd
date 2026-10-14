#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum VendorClass {
    NvidiaMx = 0,
    Nvidia = 1,
    Radeon = 2,
    Unclassified = -1,
}

impl VendorClass {
    pub const DEFAULT: VendorClass = VendorClass::Nvidia;

    pub fn from_adapter_description(description: &str) -> VendorClass {
        let desc = description.to_uppercase();

        if desc.contains("NVIDIA") {
            return if desc.contains("MX") {
                VendorClass::NvidiaMx
            } else {
                VendorClass::Nvidia
            };
        }

        if desc.contains("RADEON") {
            return VendorClass::Radeon;
        }

        VendorClass::Unclassified
    }

    pub fn as_u32(self) -> u32 {
        self as i32 as u32
    }
}

pub fn quirk_nvidia(description: &str) -> bool {
    description.to_uppercase().contains("NVIDIA")
}

pub fn quirk_riva_tnt(description: &str) -> bool {
    let desc = description.to_uppercase();

    desc.contains("NVIDIA")
        && (desc.contains("RIVA") || desc.contains("TNT") || desc.contains("TNT2"))
}

pub fn quirk_savage(description: &str) -> bool {
    description.to_uppercase().contains("SAVAGE")
}

pub fn quirk_matrox_g4(description: &str) -> bool {
    let desc = description.to_uppercase();

    desc.contains("MATROX") && desc.contains("G4")
}

pub fn quirk_intel_ps_lt_2_0(description: &str, pixel_shader_version: u32) -> bool {
    let desc = description.to_uppercase();

    (desc.contains("INTEL") || desc.contains("810")) && pixel_shader_version < 0xFFFF_0200
}

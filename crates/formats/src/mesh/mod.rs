pub mod ani;
pub mod appearance;
pub mod bon;
pub mod costume;
pub mod glow;
pub mod mesh_list;
pub mod model;
pub mod msa;
pub mod msh;
pub mod pose;
pub mod registry;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub bone_indices: [u8; 4],
    pub bone_weights: [f32; 4],
}

pub type Matrix4 = [f32; 16];

pub const MATRIX_BYTES: usize = 64;

pub const fn identity() -> Matrix4 {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub fn matrix_multiply(a: &Matrix4, b: &Matrix4) -> Matrix4 {
    let mut out = [0f32; 16];

    for i in 0..4 {
        for j in 0..4 {
            out[i * 4 + j] = a[i * 4] * b[j]
                + a[i * 4 + 1] * b[4 + j]
                + a[i * 4 + 2] * b[8 + j]
                + a[i * 4 + 3] * b[12 + j];
        }
    }

    out
}

pub(crate) fn read_matrix(bytes: &[u8]) -> Matrix4 {
    let mut m = [0f32; 16];

    for (i, chunk) in bytes.chunks_exact(4).take(16).enumerate() {
        m[i] = f32::from_le_bytes(chunk.try_into().expect("slice is 4 bytes"));
    }

    m
}

#[inline]
pub(crate) fn read_f32(bytes: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(
        bytes[off..off + 4]
            .try_into()
            .expect("caller checked bounds"),
    )
}

#[inline]
pub(crate) fn read_vec3(bytes: &[u8], off: usize) -> [f32; 3] {
    [
        read_f32(bytes, off),
        read_f32(bytes, off + 4),
        read_f32(bytes, off + 8),
    ]
}

#[inline]
pub(crate) fn read_vec2(bytes: &[u8], off: usize) -> [f32; 2] {
    [read_f32(bytes, off), read_f32(bytes, off + 4)]
}

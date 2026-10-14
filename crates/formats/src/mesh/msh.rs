use super::{MATRIX_BYTES, Matrix4, SkinVertex, read_f32, read_matrix, read_vec2, read_vec3};
use crate::error::{Error, Kind, Result};
use crate::util::u32_at;

const HEADER_LEN: usize = 32;

pub const MAX_BONE_PALETTE: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct MshHeader {
    pub magic: u32,
    pub attach_bone: u32,
    pub fvf: u32,
    pub vertex_stride: u32,
    pub num_influences: u32,
    pub num_matrices: u32,
    pub num_vertices: u32,
    pub num_indices: u32,
}

#[derive(Debug, Clone, PartialEq)]

pub struct SkinMesh {
    pub header: MshHeader,
    pub bone_offsets: Vec<Matrix4>,
    pub bone_indices: Vec<u32>,
    pub vertices: Vec<SkinVertex>,
    pub indices: Vec<u16>,
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::Msh,
        message: message.into(),
    }
}

struct VertexLayout {
    weights_off: usize,
    weight_count: usize,
    indices_off: usize,
    normal_off: Option<usize>,
    uv_off: Option<usize>,
    stride: usize,
}

fn layout(fvf: u32, stride: u32, influences: u32) -> Result<VertexLayout> {
    let l = match fvf {
        0x1116 => VertexLayout {
            weights_off: 12,
            weight_count: 0,
            indices_off: 12,
            normal_off: Some(16),
            uv_off: Some(28),
            stride: 36,
        },
        0x1118 => VertexLayout {
            weights_off: 12,
            weight_count: 1,
            indices_off: 16,
            normal_off: Some(20),
            uv_off: Some(32),
            stride: 40,
        },
        0x111A => VertexLayout {
            weights_off: 12,
            weight_count: 2,
            indices_off: 20,
            normal_off: Some(24),
            uv_off: Some(36),
            stride: 44,
        },
        0x111C => VertexLayout {
            weights_off: 12,
            weight_count: 3,
            indices_off: 24,
            normal_off: Some(28),
            uv_off: Some(40),
            stride: 48,
        },
        0x12 => VertexLayout {
            weights_off: 0,
            weight_count: 0,
            indices_off: usize::MAX,
            normal_off: Some(12),
            uv_off: None,
            stride: 24,
        },
        other => return Err(err(format!("unsupported FVF {other:#x}"))),
    };

    if stride as usize != l.stride {
        return Err(err(format!(
            "FVF {fvf:#x} implies stride {} but the file says {stride}",
            l.stride
        )));
    }

    let expected_influences = if fvf == 0x12 {
        0
    } else {
        l.weight_count as u32 + 1
    };

    if influences != expected_influences {
        return Err(err(format!(
            "FVF {fvf:#x} implies {expected_influences} influences but the file says {influences}"
        )));
    }

    Ok(l)
}

pub fn parse(bytes: &[u8]) -> Result<SkinMesh> {
    parse_inner(bytes, false)
}

pub fn parse_available_triangles(bytes: &[u8]) -> Result<SkinMesh> {
    parse_inner(bytes, true)
}

fn parse_inner(bytes: &[u8], allow_index_tail: bool) -> Result<SkinMesh> {
    let g = |off| u32_at(bytes, off, Kind::Msh);

    let header = MshHeader {
        magic: g(0)?,
        attach_bone: g(4)?,
        fvf: g(8)?,
        vertex_stride: g(12)?,
        num_influences: g(16)?,
        num_matrices: g(20)?,
        num_vertices: g(24)?,
        num_indices: g(28)?,
    };

    let l = layout(header.fvf, header.vertex_stride, header.num_influences)?;

    let num_matrices = header.num_matrices as usize;

    if num_matrices > MAX_BONE_PALETTE {
        return Err(err(format!(
            "bone palette of {num_matrices} exceeds the {MAX_BONE_PALETTE}-matrix shader limit"
        )));
    }

    let offsets_end = HEADER_LEN + num_matrices * MATRIX_BYTES;

    let ids_end = offsets_end + num_matrices * 4;

    let num_vertices = header.num_vertices as usize;

    let num_indices = header.num_indices as usize;

    let verts_end = ids_end
        .checked_add(
            num_vertices
                .checked_mul(l.stride)
                .ok_or_else(|| err("vertex block overflows"))?,
        )
        .ok_or_else(|| err("vertex block overflows"))?;

    let indices_end = verts_end
        .checked_add(
            num_indices
                .checked_mul(2)
                .ok_or_else(|| err("index block overflows"))?,
        )
        .ok_or_else(|| err("index block overflows"))?;

    let short = bytes.len() < indices_end;

    if short && (!allow_index_tail || bytes.len() < verts_end.saturating_add(6)) {
        return Err(Error::Truncated {
            kind: Kind::Msh,
            offset: 0,
            needed: indices_end,
            len: bytes.len(),
        });
    }

    let indices_end = if short {
        verts_end + (bytes.len() - verts_end) / 6 * 6
    } else {
        indices_end
    };

    let bone_offsets = (0..num_matrices)
        .map(|i| read_matrix(&bytes[HEADER_LEN + i * MATRIX_BYTES..]))
        .collect();

    let bone_indices = (0..num_matrices)
        .map(|i| u32_at(bytes, offsets_end + i * 4, Kind::Msh))
        .collect::<Result<Vec<_>>>()?;

    let vertices = bytes[ids_end..verts_end]
        .chunks_exact(l.stride)
        .map(|v| {
            let mut bone_weights = [0f32; 4];

            let mut total = 0f32;

            for (i, slot) in bone_weights.iter_mut().take(l.weight_count).enumerate() {
                *slot = read_f32(v, l.weights_off + i * 4);

                total += *slot;
            }

            if l.indices_off != usize::MAX {
                bone_weights[l.weight_count] = 1.0 - total;
            }

            let mut bone_indices = [0u8; 4];

            if l.indices_off != usize::MAX {
                bone_indices.copy_from_slice(&v[l.indices_off..l.indices_off + 4]);
            }

            SkinVertex {
                position: read_vec3(v, 0),
                normal: l.normal_off.map_or([0.0; 3], |o| read_vec3(v, o)),
                uv: l.uv_off.map_or([0.0; 2], |o| read_vec2(v, o)),
                bone_indices,
                bone_weights,
            }
        })
        .collect();

    let indices: Vec<u16> = bytes[verts_end..indices_end]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    if short && indices.iter().any(|&index| index as usize >= num_vertices) {
        return Err(err("available triangle prefix references a missing vertex"));
    }

    Ok(SkinMesh {
        header,
        bone_offsets,
        bone_indices,
        vertices,
        indices,
    })
}

impl SkinMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn encoded_len(&self) -> usize {
        let n = self.bone_offsets.len();

        HEADER_LEN
            + n * MATRIX_BYTES
            + n * 4
            + self.vertices.len() * self.header.vertex_stride as usize
            + self.indices.len() * 2
    }

    pub fn palette_indices_in_range(&self) -> bool {
        let n = self.bone_offsets.len();

        if n == 0 {
            return self.vertices.iter().all(|v| v.bone_weights == [0.0; 4]);
        }

        self.vertices.iter().all(|v| {
            v.bone_indices
                .iter()
                .zip(v.bone_weights)
                .all(|(&idx, w)| w == 0.0 || (idx as usize) < n)
        })
    }
}

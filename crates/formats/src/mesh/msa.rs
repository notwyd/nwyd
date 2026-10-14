use super::{StaticVertex, read_vec2, read_vec3};
use crate::error::{Error, Kind, Result};
use crate::util::u32_at;

const HEADER_LEN: usize = 12;

const SUBSET_LEN: usize = 20;

const TEX_NAME_LEN: usize = 11;

const FVF_XYZ_NORMAL_TEX1: u32 = 0x112;

const FVF_XYZ_DIFFUSE_TEX1: u32 = 0x142;

const FVF_XYZ_NORMAL: u32 = 0x12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Subset {
    pub material_id: u32,
    pub start_triangle: u32,
    pub prim_count: u32,
    pub min_vertex_index: u32,
    pub num_vertices: u32,
}

impl Subset {
    pub fn start_index(&self) -> u32 {
        self.start_triangle * 3
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct StaticMesh {
    pub fvf: u32,
    pub subsets: Vec<Subset>,
    pub texture_fields: Vec<Vec<u8>>,
    pub indices: Vec<u16>,
    pub vertices: Vec<StaticVertex>,
    pub has_normals: bool,
    pub has_uvs: bool,
    pub has_colors: bool,
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::Msa,
        message: message.into(),
    }
}

struct VertexLayout {
    normal_off: Option<usize>,
    uv_off: Option<usize>,
    color_off: Option<usize>,
    stride: usize,
}

fn layout_of(fvf: u32) -> Option<VertexLayout> {
    Some(match fvf {
        FVF_XYZ_NORMAL_TEX1 => VertexLayout {
            normal_off: Some(12),
            uv_off: Some(24),
            color_off: None,
            stride: 32,
        },
        FVF_XYZ_DIFFUSE_TEX1 => VertexLayout {
            normal_off: None,
            uv_off: Some(16),
            color_off: Some(12),
            stride: 24,
        },
        FVF_XYZ_NORMAL => VertexLayout {
            normal_off: Some(12),
            uv_off: None,
            color_off: None,
            stride: 24,
        },
        _ => return None,
    })
}

fn layout(fvf: u32, stride: u32) -> Result<VertexLayout> {
    let l = layout_of(fvf).ok_or_else(|| err(format!("unsupported FVF {fvf:#x}")))?;

    if stride as usize != l.stride {
        return Err(err(format!(
            "FVF {fvf:#x} implies stride {} but the file says {stride}",
            l.stride
        )));
    }

    Ok(l)
}

pub fn parse(bytes: &[u8]) -> Result<StaticMesh> {
    let fvf = u32_at(bytes, 0, Kind::Msa)?;

    let stride = u32_at(bytes, 4, Kind::Msa)?;

    let num_subsets = u32_at(bytes, 8, Kind::Msa)? as usize;

    let VertexLayout {
        normal_off,
        uv_off,
        color_off,
        stride,
    } = layout(fvf, stride)?;

    let subsets_end = HEADER_LEN
        .checked_add(
            num_subsets
                .checked_mul(SUBSET_LEN)
                .ok_or_else(|| err(format!("subset count {num_subsets} overflows")))?,
        )
        .ok_or_else(|| err("subset table overflows"))?;

    let mut subsets = Vec::with_capacity(num_subsets);

    for i in 0..num_subsets {
        let base = HEADER_LEN + i * SUBSET_LEN;

        subsets.push(Subset {
            material_id: u32_at(bytes, base, Kind::Msa)?,
            start_triangle: u32_at(bytes, base + 4, Kind::Msa)?,
            prim_count: u32_at(bytes, base + 8, Kind::Msa)?,
            min_vertex_index: u32_at(bytes, base + 12, Kind::Msa)?,
            num_vertices: u32_at(bytes, base + 16, Kind::Msa)?,
        });
    }

    let names_end = subsets_end + num_subsets * TEX_NAME_LEN;

    let mut texture_fields = Vec::with_capacity(num_subsets);

    for i in 0..num_subsets {
        let base = subsets_end + i * TEX_NAME_LEN;

        let field = bytes
            .get(base..base + TEX_NAME_LEN)
            .ok_or_else(|| err("texture name table is truncated"))?;

        let end = field.iter().position(|&b| b == 0).unwrap_or(TEX_NAME_LEN);

        texture_fields.push(field[..end].to_vec());
    }

    let index_bytes = u32_at(bytes, names_end, Kind::Msa)? as usize;

    if !index_bytes.is_multiple_of(2) {
        return Err(err(format!(
            "index block of {index_bytes} bytes is not u16-aligned"
        )));
    }

    let idx_start = names_end + 4;

    let idx_data = bytes
        .get(idx_start..idx_start + index_bytes)
        .ok_or_else(|| err("index block is truncated"))?;

    let indices: Vec<u16> = idx_data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    let vb_len_off = idx_start + index_bytes;

    let vertex_bytes = u32_at(bytes, vb_len_off, Kind::Msa)? as usize;

    if !vertex_bytes.is_multiple_of(stride) {
        return Err(err(format!(
            "vertex block of {vertex_bytes} bytes is not a multiple of stride {stride}"
        )));
    }

    let vtx_start = vb_len_off + 4;

    let vtx_data = bytes
        .get(vtx_start..vtx_start + vertex_bytes)
        .ok_or_else(|| err("vertex block is truncated"))?;

    let vertices = vtx_data
        .chunks_exact(stride)
        .map(|v| StaticVertex {
            position: read_vec3(v, 0),
            normal: normal_off.map_or([0.0; 3], |o| read_vec3(v, o)),
            uv: uv_off.map_or([0.0; 2], |o| read_vec2(v, o)),
            color: color_off.map_or(0xFFFF_FFFF, |o| {
                u32::from_le_bytes(v[o..o + 4].try_into().expect("stride checked"))
            }),
        })
        .collect();

    Ok(StaticMesh {
        fvf,
        subsets,
        texture_fields,
        indices,
        vertices,
        has_normals: normal_off.is_some(),
        has_uvs: uv_off.is_some(),
        has_colors: color_off.is_some(),
    })
}

impl StaticMesh {
    pub fn encoded_len(&self) -> usize {
        let n = self.subsets.len();

        let stride = layout_of(self.fvf).map_or(0, |l| l.stride);

        HEADER_LEN
            + n * SUBSET_LEN
            + n * TEX_NAME_LEN
            + 4
            + self.indices.len() * 2
            + 4
            + self.vertices.len() * stride
    }
}

pub fn texture_base_name(mesh_file_name: &str, field: &[u8]) -> String {
    let own_base = || {
        mesh_file_name
            .split('.')
            .next()
            .unwrap_or(mesh_file_name)
            .to_string()
    };

    let lower = mesh_file_name.as_bytes();

    if lower.len() >= 2
        && lower[0].eq_ignore_ascii_case(&b'a')
        && lower[1].eq_ignore_ascii_case(&b'a')
    {
        return own_base();
    }

    let tail = match field.iter().rposition(|&b| b == b'\\') {
        Some(i) => &field[i + 1..],
        None => field,
    };

    let base: Vec<u8> = tail.iter().copied().take_while(|&b| b != b'.').collect();

    if base.is_empty() {
        return own_base();
    }

    String::from_utf8_lossy(&base).into_owned()
}

use crate::device::Gpu;
use formats::texture::texture_list::FormatFlag;
use formats::texture::{PixelFormat, Texture};

mod mipmaps;

pub use mipmaps::{MipPolicy, with_mipmaps};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaPolicy {
    Stored,
    Opaque,
}

impl AlphaPolicy {
    pub fn for_flag(flag: FormatFlag) -> AlphaPolicy {
        match flag {
            FormatFlag::Alpha | FormatFlag::AlphaLower | FormatFlag::ColorKey => {
                AlphaPolicy::Stored
            }
            FormatFlag::None_ | FormatFlag::Other(_) => AlphaPolicy::Opaque,
        }
    }

    pub fn as_uniform(self) -> f32 {
        match self {
            AlphaPolicy::Stored => 0.0,
            AlphaPolicy::Opaque => 1.0,
        }
    }
}

pub const COLOR_KEY: [u8; 4] = [0, 0, 0, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorKey {
    Fires,
    CannotFire,
    Unresolved,
}

impl ColorKey {
    pub fn for_wyt(flag: FormatFlag, source_bits_per_pixel: u8) -> ColorKey {
        match flag {
            FormatFlag::Alpha | FormatFlag::AlphaLower => match source_bits_per_pixel {
                32 | 16 => ColorKey::Fires,
                _ => ColorKey::Unresolved,
            },
            FormatFlag::None_ | FormatFlag::ColorKey | FormatFlag::Other(_) => ColorKey::CannotFire,
        }
    }
}

pub fn apply_color_key(texture: &mut Texture) -> usize {
    if texture.format != PixelFormat::Rgba8Unorm {
        return 0;
    }

    let mut keyed = 0;

    for texel in texture.data.chunks_exact_mut(4) {
        if texel == COLOR_KEY {
            texel.copy_from_slice(&[0, 0, 0, 0]);

            keyed += 1;
        }
    }

    keyed
}

pub fn apply_guimat_color_key(texture: &mut Texture, argb: u32) -> usize {
    if texture.format != PixelFormat::Rgba8Unorm {
        return 0;
    }

    let key = [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8];

    let mut keyed = 0;

    for texel in texture.data.chunks_exact_mut(4) {
        if texel[..3] == key {
            texel.copy_from_slice(&[0, 0, 0, 0]);

            keyed += 1;
        }
    }

    keyed
}

pub fn wgpu_format(format: PixelFormat) -> wgpu::TextureFormat {
    match format {
        PixelFormat::Bc1RgbaUnorm => wgpu::TextureFormat::Bc1RgbaUnorm,
        PixelFormat::Bc2RgbaUnorm => wgpu::TextureFormat::Bc2RgbaUnorm,
        PixelFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
    }
}

pub fn bytes_per_row(format: PixelFormat, width: u32) -> u32 {
    match format.block_bytes() {
        Some(block) => width.div_ceil(4) * block as u32,
        None => width * 4,
    }
}

pub fn copy_extent(format: PixelFormat, width: u32, height: u32) -> (u32, u32) {
    match format.block_bytes() {
        Some(_) => (width.div_ceil(4) * 4, height.div_ceil(4) * 4),
        None => (width, height),
    }
}

#[derive(Debug)]
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub alpha: AlphaPolicy,
}

pub fn upload_material(
    gpu: &Gpu,
    texture: &Texture,
    alpha: AlphaPolicy,
    mips: MipPolicy,
    label: &str,
) -> GpuTexture {
    upload(gpu, &with_mipmaps(texture, mips), alpha, label)
}

pub fn upload_material_array(
    gpu: &Gpu,
    textures: &[&Texture],
    alpha: AlphaPolicy,
    mips: MipPolicy,
    label: &str,
) -> Option<GpuTexture> {
    let mut prepared: Vec<_> = textures
        .iter()
        .map(|texture| with_mipmaps(texture, mips))
        .collect();
    let first_format = prepared.first()?.format;

    if prepared
        .iter()
        .any(|texture| texture.format != first_format)
    {
        for texture in &mut prepared {
            if texture.format.is_block_compressed() {
                *texture = std::borrow::Cow::Owned(texture.clone().into_owned().into_rgba8().ok()?);
            }
        }
    }

    let textures: Vec<_> = prepared.iter().map(|texture| texture.as_ref()).collect();

    upload_array(gpu, &textures, alpha, label)
}

pub fn upload(gpu: &Gpu, texture: &Texture, alpha: AlphaPolicy, label: &str) -> GpuTexture {
    let format = wgpu_format(texture.format);

    let mip_levels = texture.mips.len() as u32;

    let gpu_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: crate::gpu_label(label),
        size: wgpu::Extent3d {
            width: texture.width,
            height: texture.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (level, mip) in texture.mips.iter().enumerate() {
        let Some(data) = texture.data.get(mip.offset..mip.offset + mip.len) else {
            break;
        };

        let rows = match texture.format.block_bytes() {
            Some(_) => mip.height.div_ceil(4),
            None => mip.height,
        };

        let (copy_width, copy_height) = copy_extent(texture.format, mip.width, mip.height);

        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu_texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row(texture.format, mip.width)),
                rows_per_image: Some(rows),
            },
            wgpu::Extent3d {
                width: copy_width,
                height: copy_height,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

    GpuTexture {
        texture: gpu_texture,
        view,
        width: texture.width,
        height: texture.height,
        mip_levels,
        alpha,
    }
}

pub fn upload_cube(
    gpu: &Gpu,
    cube: &formats::texture::CubeTexture,
    alpha: AlphaPolicy,
    label: &str,
) -> Option<GpuTexture> {
    let first = &cube.faces[0];

    if first.width == 0
        || first.width != first.height
        || first.width > gpu.device.limits().max_texture_dimension_2d
        || first.mips.is_empty()
        || first.mips.len() > (32 - first.width.leading_zeros()) as usize
    {
        return None;
    }

    for face in &cube.faces {
        for (level, mip) in face.mips.iter().enumerate() {
            let edge = first.width.checked_shr(level as u32)?.max(1);

            let bytes = match face.format.block_bytes() {
                Some(block) => u64::from(edge.div_ceil(4)).pow(2) * block as u64,
                None => u64::from(edge).pow(2) * 4,
            };

            if mip.width != edge
                || mip.height != edge
                || u64::try_from(mip.len).ok()? != bytes
                || mip.offset.checked_add(mip.len)? > face.data.len()
            {
                return None;
            }
        }
    }

    let faces: Vec<_> = cube.faces.iter().collect();

    let mut texture = upload_array(gpu, &faces, alpha, label)?;

    texture.view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
        label: crate::gpu_label(label),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    Some(texture)
}

pub fn upload_array(
    gpu: &Gpu,
    textures: &[&Texture],
    alpha: AlphaPolicy,
    label: &str,
) -> Option<GpuTexture> {
    let first = textures.first()?;

    let shape = |t: &Texture| (t.width, t.height, t.format, t.mips.len());

    if textures.iter().any(|t| shape(t) != shape(first)) {
        return None;
    }

    let gpu_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: crate::gpu_label(label),
        size: wgpu::Extent3d {
            width: first.width,
            height: first.height,
            depth_or_array_layers: textures.len() as u32,
        },
        mip_level_count: first.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_format(first.format),
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (layer, texture) in textures.iter().enumerate() {
        for (level, mip) in texture.mips.iter().enumerate() {
            let Some(data) = texture.data.get(mip.offset..mip.offset + mip.len) else {
                break;
            };

            let rows = match texture.format.block_bytes() {
                Some(_) => mip.height.div_ceil(4),
                None => mip.height,
            };

            let (copy_width, copy_height) = copy_extent(texture.format, mip.width, mip.height);

            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu_texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row(texture.format, mip.width)),
                    rows_per_image: Some(rows),
                },
                wgpu::Extent3d {
                    width: copy_width,
                    height: copy_height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    let view = gpu_texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });

    Some(GpuTexture {
        texture: gpu_texture,
        view,
        width: first.width,
        height: first.height,
        mip_levels: first.mips.len() as u32,
        alpha,
    })
}

use super::{TerrainRenderer, TileArray};
use crate::{
    device::Gpu,
    texture::{bytes_per_row, copy_extent, wgpu_format},
};
use formats::texture::{PixelFormat, Texture};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileUploadStats {
    pub allocations: usize,
    pub uploaded_layers: usize,
    pub uploaded_bytes: u64,
    pub copied_layers: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shape {
    width: u32,
    height: u32,
    format: PixelFormat,
    mips: u32,
}

impl Shape {
    fn of(t: &Texture) -> Self {
        Self {
            width: t.width,
            height: t.height,
            format: t.format,
            mips: t.mips.len() as u32,
        }
    }

    fn valid(self, t: &Texture, limit: u32) -> bool {
        if self != Self::of(t)
            || self.width == 0
            || self.height == 0
            || self.width > limit
            || self.height > limit
            || self.mips == 0
            || self.mips > self.width.max(self.height).ilog2() + 1
        {
            return false;
        }

        if self.format.is_block_compressed() && (self.width % 4 != 0 || self.height % 4 != 0) {
            return false;
        }

        t.mips.iter().enumerate().all(|(level, mip)| {
            let width = (self.width >> level).max(1);

            let height = (self.height >> level).max(1);

            let rows = if self.format.is_block_compressed() {
                height.div_ceil(4)
            } else {
                height
            };

            mip.width == width
                && mip.height == height
                && mip.len as u64 == u64::from(bytes_per_row(self.format, width)) * u64::from(rows)
                && mip
                    .offset
                    .checked_add(mip.len)
                    .is_some_and(|end| end <= t.data.len())
        })
    }
}

#[derive(Default)]
pub struct TileCache {
    arrays: Vec<ArrayCache>,
    bound: Option<TileArray>,
    stats: TileUploadStats,
}

impl TileCache {
    pub fn clear(&mut self) {
        self.arrays.clear();

        self.bound = None;
    }

    pub fn bound(&self) -> Option<&TileArray> {
        self.bound.as_ref()
    }

    pub fn stats(&self) -> TileUploadStats {
        self.stats
    }

    pub fn sync(
        &mut self,
        gpu: &Gpu,
        renderer: &TerrainRenderer,
        tiles: &[(i32, &Texture)],
    ) -> bool {
        if tiles.is_empty() {
            if !self.arrays.is_empty() || self.bound.is_none() {
                self.clear();

                self.bound = Some(renderer.bind_null_tiles(gpu));
            }

            return true;
        }

        let limits = gpu.device.limits();

        let mut ids = HashSet::new();

        let mut groups: Vec<(Shape, Vec<(i32, &Texture)>)> = Vec::new();

        for &(id, image) in tiles {
            let shape = Shape::of(image);

            if !ids.insert(id) || !shape.valid(image, limits.max_texture_dimension_2d) {
                self.clear();

                return false;
            }

            let index = groups
                .iter()
                .position(|(s, _)| *s == shape)
                .unwrap_or_else(|| {
                    groups.push((shape, Vec::new()));

                    groups.len() - 1
                });

            groups[index].1.push((id, image));

            if groups[index].1.len() > limits.max_texture_array_layers as usize {
                self.clear();

                return false;
            }
        }

        if self
            .bound
            .as_ref()
            .is_some_and(|b| b.layers.keys().any(|id| !ids.contains(id)))
        {
            self.clear();
        }

        let previous = self.stats;

        for (shape, group) in groups {
            let index = self
                .arrays
                .iter()
                .position(|a| a.shape == Some(shape))
                .unwrap_or_else(|| {
                    self.arrays.push(ArrayCache::default());

                    self.arrays.len() - 1
                });

            let array = &mut self.arrays[index];

            let old = array.stats();

            if !array.sync(gpu, renderer, &group) {
                self.clear();

                return false;
            }

            let current = array.stats();

            self.stats.allocations += current.allocations - old.allocations;

            self.stats.uploaded_layers += current.uploaded_layers - old.uploaded_layers;

            self.stats.uploaded_bytes += current.uploaded_bytes - old.uploaded_bytes;

            self.stats.copied_layers += current.copied_layers - old.copied_layers;
        }

        if self.bound.is_some() && self.stats == previous {
            return true;
        }

        let mut bound = TileArray {
            bind_group: self.arrays[0].bound().unwrap().bind_group.clone(),
            layers: Default::default(),
            groups: Default::default(),
            pairs: Vec::new(),
            group_count: self.arrays.len(),
        };

        for (index, array) in self.arrays.iter().enumerate() {
            for (&id, &layer) in &array.bound().unwrap().layers {
                bound.layers.insert(id, layer);

                bound.groups.insert(id, index);
            }
        }

        if self.arrays.len() > 1 {
            let views: Vec<_> = self
                .arrays
                .iter()
                .map(|a| {
                    a.texture
                        .as_ref()
                        .unwrap()
                        .create_view(&wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::D2Array),
                            ..Default::default()
                        })
                })
                .collect();

            for base in &views {
                for detail in &views {
                    bound.pairs.push(renderer.bind_tile_pair(gpu, base, detail));
                }
            }
        }

        self.bound = Some(bound);

        true
    }
}

#[derive(Default)]
struct ArrayCache {
    texture: Option<wgpu::Texture>,
    bound: Option<TileArray>,
    shape: Option<Shape>,
    capacity: u32,
    stats: TileUploadStats,
}

impl ArrayCache {
    pub fn clear(&mut self) {
        self.bound = None;

        self.texture = None;

        self.shape = None;

        self.capacity = 0;
    }

    pub fn bound(&self) -> Option<&TileArray> {
        self.bound.as_ref()
    }

    pub fn stats(&self) -> TileUploadStats {
        self.stats
    }

    pub fn sync(
        &mut self,
        gpu: &Gpu,
        renderer: &TerrainRenderer,
        tiles: &[(i32, &Texture)],
    ) -> bool {
        let Some((_, first)) = tiles.first() else {
            if self.texture.is_some() || self.bound.is_none() {
                self.clear();

                self.bound = Some(renderer.bind_null_tiles(gpu));
            }

            return true;
        };

        let limits = gpu.device.limits();

        let shape = Shape::of(first);

        let ids: HashSet<_> = tiles.iter().map(|(id, _)| *id).collect();

        if tiles.len() > limits.max_texture_array_layers as usize
            || ids.len() != tiles.len()
            || tiles
                .iter()
                .any(|(_, t)| !shape.valid(t, limits.max_texture_dimension_2d))
        {
            self.clear();

            return false;
        }

        if self.shape != Some(shape)
            || self
                .bound
                .as_ref()
                .is_some_and(|b| b.layers.keys().any(|id| !ids.contains(id)))
        {
            self.clear();
        }

        let count = tiles.len() as u32;

        if count > self.capacity {
            let capacity = count
                .next_power_of_two()
                .max(16)
                .min(limits.max_texture_array_layers);

            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: crate::gpu_label("resident terrain tiles"),
                size: wgpu::Extent3d {
                    width: shape.width,
                    height: shape.height,
                    depth_or_array_layers: capacity,
                },
                mip_level_count: shape.mips,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu_format(shape.format),
                view_formats: &[],
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
            });

            let layers = self.bound.take().map(|b| b.layers).unwrap_or_default();

            if let Some(old) = &self.texture {
                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: crate::gpu_label("grow terrain tile array"),
                        });

                for mip in 0..shape.mips {
                    let (width, height) = copy_extent(
                        shape.format,
                        (shape.width >> mip).max(1),
                        (shape.height >> mip).max(1),
                    );

                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: old,
                            mip_level: mip,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: mip,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: layers.len() as u32,
                        },
                    );
                }

                gpu.queue.submit([encoder.finish()]);

                self.stats.copied_layers += layers.len();
            }

            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });

            self.bound = Some(renderer.bind_tiles(gpu, &view, layers));

            self.texture = Some(texture);

            self.shape = Some(shape);

            self.capacity = capacity;

            self.stats.allocations += 1;
        }

        let texture = self
            .texture
            .as_ref()
            .expect("nonempty valid set allocated an array");

        let bound = self.bound.as_mut().expect("array has a binding");

        for &(id, t) in tiles {
            if bound.layers.contains_key(&id) {
                continue;
            }

            let layer = bound.layers.len() as u32;

            for (level, mip) in t.mips.iter().enumerate() {
                let rows = if t.format.is_block_compressed() {
                    mip.height.div_ceil(4)
                } else {
                    mip.height
                };

                let (width, height) = copy_extent(t.format, mip.width, mip.height);

                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: level as u32,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &t.data[mip.offset..mip.offset + mip.len],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row(t.format, mip.width)),
                        rows_per_image: Some(rows),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );

                self.stats.uploaded_bytes += mip.len as u64;
            }

            bound.layers.insert(id, layer);

            self.stats.uploaded_layers += 1;
        }

        true
    }
}

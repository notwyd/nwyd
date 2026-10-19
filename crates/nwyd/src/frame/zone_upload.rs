use super::world_textures::WorldTextures;
use crate::scene::world::{World, loading::Prepared};
use render::{
    device::Gpu,
    geometry::{VertexCopy, stage_vertices},
    mesh::MeshVertex,
    skin::SkinVertexGpu,
};
use std::collections::HashMap;
use web_time::{Duration, Instant};

pub(crate) struct Budget {
    start: Option<Instant>,
    bytes: usize,
    assets: usize,
    limit: usize,
}

impl Budget {
    #[cfg(test)]
    pub(super) fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            ..Self::new()
        }
    }

    pub fn new() -> Self {
        Self {
            start: None,
            bytes: 0,
            assets: 0,
            limit: 2 * 1024 * 1024,
        }
    }

    fn admits(&self, bytes: usize, elapsed: Duration) -> bool {
        self.assets == 0
            || (elapsed < Duration::from_micros(500)
                && self.bytes.saturating_add(bytes) <= self.limit)
    }

    pub(super) fn take(&mut self, bytes: usize) -> bool {
        if !self.admits(
            bytes,
            self.start.map_or(Duration::ZERO, |start| start.elapsed()),
        ) {
            return false;
        }

        self.start.get_or_insert_with(Instant::now);

        self.bytes = self.bytes.saturating_add(bytes);

        self.assets += 1;

        true
    }

    pub fn report(&self) {
        if self.assets != 0 {
            tracing::debug!(
                bytes = self.bytes,
                assets = self.assets,
                window_ms = self
                    .start
                    .map_or(Duration::ZERO, |start| start.elapsed())
                    .as_secs_f64()
                    * 1000.0,
                "frame GPU upload budget"
            );
        }
    }
}

#[derive(Default)]
pub(super) struct Staging {
    key: Option<(u64, u64)>,
    cursor: usize,
    pub complete: bool,
    meshes: HashMap<String, wgpu::Buffer>,
    skins: HashMap<String, wgpu::Buffer>,
}

impl Staging {
    pub fn matches(&self, identity: u64) -> bool {
        self.key.is_some_and(|k| k.0 == identity)
    }

    pub fn advance(
        &mut self,
        gpu: &Gpu,
        identity: u64,
        prepared: &Prepared,
        textures: &mut WorldTextures,
        budget: &mut Budget,
    ) -> bool {
        let key = (identity, prepared.serial);

        if self.key != Some(key) {
            *self = Self {
                key: Some(key),
                ..Self::default()
            };

            textures.clear_staged();
        }

        let Some(resources) = &prepared.resources else {
            self.complete = true;

            return true;
        };

        let mut index = 0;

        for (skin, t) in resources
            .objects
            .textures
            .iter()
            .map(|t| (false, t))
            .chain(resources.actors.textures.iter().map(|t| (true, t)))
        {
            if index >= self.cursor {
                if textures.needs_stage(t, skin) {
                    let Some(material) = textures.prepared(t, skin) else {
                        return false;
                    };
                    if !budget.take(material.upload_bytes()) {
                        return false;
                    }

                    textures.stage(gpu, t, skin, &material);
                }

                self.cursor += 1;
            }

            index += 1;
        }

        for mesh in &resources.objects.meshes {
            if index >= self.cursor {
                let alloc = &resources.objects.arena.allocs[mesh.alloc];

                let start = alloc.base_vertex as usize;

                let vertices =
                    &resources.objects.arena.vertices[start..start + alloc.vertex_count as usize];

                if !vertices.is_empty() && !self.meshes.contains_key(&mesh.path) {
                    if !budget.take(std::mem::size_of_val(vertices)) {
                        return false;
                    }

                    self.meshes
                        .insert(mesh.path.clone(), stage_vertices(gpu, vertices));
                }

                self.cursor += 1;
            }

            index += 1;
        }

        for model in &resources.actors.models {
            for part in &model.parts {
                if index >= self.cursor {
                    let key = part.mesh_path.clone();

                    let alloc = &resources.actors.arena.allocs[part.alloc];

                    let start = alloc.base_vertex as usize;

                    let vertices = &resources.actors.arena.vertices
                        [start..start + alloc.vertex_count as usize];

                    if !vertices.is_empty() && !self.skins.contains_key(&key) {
                        if !budget.take(std::mem::size_of_val(vertices)) {
                            return false;
                        }

                        self.skins.insert(key, stage_vertices(gpu, vertices));
                    }

                    self.cursor += 1;
                }

                index += 1;
            }
        }

        self.complete = true;

        tracing::debug!(
            zone = ? (prepared.terrain.zone_x, prepared.terrain.zone_y), assets = self
            .cursor, "zone GPU staging ready for publication"
        );

        true
    }

    pub fn mesh_copies(&self, world: &World) -> Vec<VertexCopy<'_>> {
        if !self.complete || !self.matches(world.identity) {
            return Vec::new();
        }

        let mut copies: Vec<_> = world
            .objects
            .meshes
            .iter()
            .filter_map(|mesh| {
                let source = self.meshes.get(&mesh.path)?;

                let alloc = &world.objects.arena.allocs[mesh.alloc];

                let size = u64::from(alloc.vertex_count) * MeshVertex::STRIDE;

                (source.size() == size).then_some(VertexCopy {
                    source,
                    source_offset: 0,
                    destination_offset: alloc.base_vertex as u64 * MeshVertex::STRIDE,
                    size,
                })
            })
            .collect();

        copies.sort_unstable_by_key(|c| c.destination_offset);

        copies
    }

    pub fn skin_copies(&self, world: &World) -> Vec<VertexCopy<'_>> {
        if !self.complete || !self.matches(world.identity) {
            return Vec::new();
        }

        let mut copies: Vec<_> = world
            .actors
            .models
            .iter()
            .flat_map(|model| {
                model.parts.iter().filter_map(move |part| {
                    let source = self.skins.get(&part.mesh_path)?;

                    let alloc = &world.actors.arena.allocs[part.alloc];

                    let size = u64::from(alloc.vertex_count) * SkinVertexGpu::STRIDE;

                    (source.size() == size).then_some(VertexCopy {
                        source,
                        source_offset: 0,
                        destination_offset: alloc.base_vertex as u64 * SkinVertexGpu::STRIDE,
                        size,
                    })
                })
            })
            .collect();

        copies.sort_unstable_by_key(|c| c.destination_offset);

        copies.dedup_by_key(|c| c.destination_offset);

        copies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_asset_is_admitted_even_when_larger_than_the_budget() {
        let budget = Budget::new();
        assert!(budget.admits(budget.limit + 1, Duration::from_secs(1)));
    }

    #[test]
    fn later_assets_obey_both_time_and_byte_limits() {
        let mut budget = Budget::new();
        budget.assets = 1;
        budget.bytes = budget.limit - 128;
        assert!(budget.admits(128, Duration::from_micros(499)));
        assert!(!budget.admits(129, Duration::ZERO));
        assert!(!budget.admits(128, Duration::from_micros(500)));
        assert!(!budget.admits(usize::MAX, Duration::ZERO));
    }

    #[test]
    fn repeated_upload_phases_share_the_limit_until_next_frame() {
        let mut budget = Budget::with_limit(0);
        assert!(budget.start.is_none());
        assert!(budget.take(1024));
        assert!(budget.start.is_some());
        for _ in 0..3 {
            assert!(!budget.take(1024));
        }
        assert_eq!(budget.bytes, 1024);
        assert_eq!(budget.assets, 1);
        budget = Budget::with_limit(0);
        assert!(budget.take(1024));
    }

    #[test]
    fn later_upload_phases_cannot_restart_an_expired_window() {
        let mut budget = Budget::new();
        assert!(budget.take(1));
        budget.start = Some(Instant::now() - Duration::from_secs(1));
        assert!(!budget.take(1));
        assert_eq!(budget.assets, 1);
    }
}

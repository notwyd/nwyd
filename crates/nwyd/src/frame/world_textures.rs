use crate::scene::objects::BoundTexture;
use render::device::Gpu;
use render::mesh::{MeshRenderer, MeshTexture};
use render::skin::{SkinRenderer, SkinTexture};
use render::texture;
use std::collections::{HashSet, VecDeque};
use web_time::Instant;

mod preparation;

#[derive(Debug, Default, Clone, Copy)]
pub struct MaterialUploadStats {
    pub uploads: usize,
    pub bytes: u64,
    pub elapsed_ms: f64,
    pub longest_ms: f64,
    pub pending: usize,
}

#[derive(Default)]
struct PendingUploads {
    queue: VecDeque<(bool, usize)>,
    queued: HashSet<(bool, usize)>,
}

impl PendingUploads {
    fn push(&mut self, key: (bool, usize)) {
        if self.queued.insert(key) {
            self.queue.push_back(key);
        }
    }

    fn pop(&mut self) {
        if let Some(key) = self.queue.pop_front() {
            self.queued.remove(&key);
        }
    }

    fn defer(&mut self) {
        if let Some(key) = self.queue.pop_front() {
            self.queue.push_back(key);
        }
    }
}

type Key = (bool, String, bool, texture::MipPolicy);

fn key(t: &BoundTexture, skin: bool) -> Key {
    (
        skin,
        t.path.clone(),
        t.alpha_policy() == texture::AlphaPolicy::Opaque,
        t.mip_policy(skin),
    )
}

#[derive(Default)]
pub(super) struct WorldTextures {
    pub meshes: Vec<MeshTexture>,
    pub mesh_views: Vec<wgpu::TextureView>,
    mesh_keys: Vec<Key>,
    mesh_pending: Vec<bool>,
    pub skins: Vec<SkinTexture>,
    skin_views: Vec<wgpu::TextureView>,
    skin_keys: Vec<Key>,
    skin_pending: Vec<bool>,
    pub uploads: (usize, usize),
    staged: std::collections::HashMap<Key, wgpu::TextureView>,
    resident: std::collections::HashMap<Key, wgpu::TextureView>,
    pending: PendingUploads,
    placeholder: Option<wgpu::TextureView>,
    stats: MaterialUploadStats,
    preparation: preparation::Preparation,
}

impl WorldTextures {
    pub fn clear(&mut self) {
        self.meshes.clear();

        self.mesh_views.clear();

        self.mesh_keys.clear();

        self.mesh_pending.clear();

        self.skins.clear();

        self.skin_views.clear();

        self.skin_keys.clear();

        self.skin_pending.clear();

        self.staged.clear();

        self.resident.clear();

        self.pending = PendingUploads::default();

        self.placeholder = None;

        self.preparation.clear();
    }

    pub fn clear_staged(&mut self) {
        self.staged.clear();

        self.preparation.clear();
    }

    pub fn needs_stage(&self, t: &BoundTexture, skin: bool) -> bool {
        let key = key(t, skin);

        !t.pending && !self.resident.contains_key(&key) && !self.staged.contains_key(&key)
    }

    pub fn prepared(
        &mut self,
        t: &BoundTexture,
        skin: bool,
    ) -> Option<std::sync::Arc<texture::PreparedMaterial>> {
        if t.prepared_material(skin).is_some() {
            return t.prepared.clone();
        }
        self.preparation.poll();
        let key = key(t, skin);
        if let Some(prepared) = self.preparation.get(&key) {
            return Some(prepared);
        }
        self.preparation.request(&key, &t.texture);
        None
    }

    pub fn stage(
        &mut self,
        gpu: &Gpu,
        t: &BoundTexture,
        skin: bool,
        material: &texture::PreparedMaterial,
    ) {
        let view = self.upload(gpu, t, skin, material);

        self.staged.insert(key(t, skin), view);
    }

    fn upload(
        &mut self,
        gpu: &Gpu,
        t: &BoundTexture,
        skin: bool,
        material: &texture::PreparedMaterial,
    ) -> wgpu::TextureView {
        let start = Instant::now();
        let view = material.upload(gpu, t.alpha_policy(), &t.path).view;
        self.preparation.invalidate(&key(t, skin));
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.stats.uploads += 1;
        self.stats.bytes += material.upload_bytes() as u64;
        self.stats.elapsed_ms += elapsed_ms;
        self.stats.longest_ms = self.stats.longest_ms.max(elapsed_ms);
        if skin {
            self.uploads.1 += 1;
        } else {
            self.uploads.0 += 1;
        }
        tracing::debug!(path = %t.path, bytes = material.upload_bytes(), elapsed_ms, "material GPU upload");
        view
    }

    fn placeholder(&mut self, gpu: &Gpu) -> wgpu::TextureView {
        self.placeholder
            .get_or_insert_with(|| {
                texture::upload(
                    gpu,
                    &crate::scene::actors::texture_loading::placeholder(),
                    texture::AlphaPolicy::Stored,
                    "pending world material",
                )
                .view
            })
            .clone()
    }

    fn initial_view(
        &mut self,
        gpu: &Gpu,
        t: &BoundTexture,
        skin: bool,
        index: usize,
    ) -> wgpu::TextureView {
        let key = key(t, skin);
        if !t.pending
            && let Some(view) = self.staged.get(&key).or_else(|| self.resident.get(&key))
        {
            let view = view.clone();
            self.resident.insert(key, view.clone());
            return view;
        }
        if !t.pending {
            self.pending.push((skin, index));
        }
        self.placeholder(gpu)
    }

    pub fn refresh_skins(
        &mut self,
        _gpu: &Gpu,
        _renderer: &SkinRenderer,
        textures: &[BoundTexture],
        _pairs: &[(usize, usize)],
        updates: &[usize],
        old_plain: usize,
    ) {
        let changed: std::collections::HashSet<_> =
            updates.iter().copied().filter(|&i| i < old_plain).collect();

        for &index in &changed {
            let t = &textures[index];
            let next = key(t, true);
            let publish = self.skin_pending[index] && !t.pending;
            let old = &self.skin_keys[index];
            if !publish || old != &next {
                self.staged.remove(old);
            }
            self.resident.remove(old);
            self.preparation.invalidate(old);

            if !publish {
                self.staged.remove(&next);
            }
            if old != &next {
                self.resident.remove(&next);
                self.preparation.invalidate(&next);
            }
            self.skin_keys[index] = next;
            self.skin_pending[index] = t.pending;

            self.pending.push((true, index));
        }
    }

    pub fn refresh_meshes(
        &mut self,
        _gpu: &Gpu,
        _renderer: &MeshRenderer,
        textures: &[BoundTexture],
        updates: &[usize],
        old_len: usize,
    ) {
        let changed: std::collections::BTreeSet<_> =
            updates.iter().copied().filter(|&i| i < old_len).collect();

        for index in changed {
            let t = &textures[index];
            let next = key(t, false);
            let publish = self.mesh_pending[index] && !t.pending;
            let old = &self.mesh_keys[index];
            if !publish || old != &next {
                self.staged.remove(old);
            }
            self.resident.remove(old);
            self.preparation.invalidate(old);

            if !publish {
                self.staged.remove(&next);
            }
            if old != &next {
                self.resident.remove(&next);
                self.preparation.invalidate(&next);
            }
            self.mesh_keys[index] = next;
            self.mesh_pending[index] = t.pending;

            self.pending.push((false, index));
        }
    }

    pub fn sync_meshes(&mut self, gpu: &Gpu, renderer: &MeshRenderer, textures: &[BoundTexture]) {
        for (index, t) in textures.iter().enumerate().skip(self.meshes.len()) {
            let view = self.initial_view(gpu, t, false, index);

            self.meshes.push(renderer.bind_texture(gpu, &view));

            self.mesh_views.push(view);

            self.mesh_keys.push(key(t, false));

            self.mesh_pending.push(t.pending);
        }
    }

    pub fn sync_skins(
        &mut self,
        gpu: &Gpu,
        renderer: &SkinRenderer,
        textures: &[BoundTexture],
        pairs: &[(usize, usize)],
    ) {
        let plain = self.skin_views.len();

        let paired = self.skins.len() - plain;

        for (index, t) in textures.iter().enumerate().skip(plain) {
            let view = self.initial_view(gpu, t, true, index);

            self.skins.push(renderer.bind_texture(gpu, &view));

            self.skin_views.push(view);

            self.skin_keys.push(key(t, true));

            self.skin_pending.push(t.pending);
        }

        self.skins[plain..].rotate_right(textures.len() - plain);

        for &(base, sheet) in &pairs[paired..] {
            self.skins.push(renderer.bind_glow(
                gpu,
                &self.skin_views[base],
                &self.skin_views[sheet],
            ));
        }
    }

    pub fn stats(&self) -> MaterialUploadStats {
        MaterialUploadStats {
            pending: self.pending.queue.len(),
            ..self.stats
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn flush(
        &mut self,
        gpu: &Gpu,
        mesh_renderer: &MeshRenderer,
        meshes: &[BoundTexture],
        skin_renderer: &SkinRenderer,
        skins: &[BoundTexture],
        pairs: &[(usize, usize)],
        budget: &mut super::zone_upload::Budget,
    ) {
        let mut changed_skins = HashSet::new();
        self.preparation.poll();
        for _ in 0..self.pending.queue.len() {
            let Some(&(skin, index)) = self.pending.queue.front() else {
                break;
            };
            let Some(t) = (if skin { skins } else { meshes }).get(index) else {
                self.pending.pop();
                continue;
            };
            let key = key(t, skin);
            let view = if t.pending {
                self.placeholder(gpu)
            } else if let Some(view) = self.staged.get(&key).or_else(|| self.resident.get(&key)) {
                view.clone()
            } else {
                let Some(material) = self.prepared(t, skin) else {
                    self.pending.defer();
                    continue;
                };
                if !budget.take(material.upload_bytes()) {
                    break;
                }
                self.upload(gpu, t, skin, &material)
            };
            if !t.pending {
                self.resident.insert(key, view.clone());
            }
            if skin {
                self.skins[index] = skin_renderer.bind_texture(gpu, &view);
                self.skin_views[index] = view;
                changed_skins.insert(index);
            } else {
                self.meshes[index] = mesh_renderer.bind_texture(gpu, &view);
                self.mesh_views[index] = view;
            }
            self.pending.pop();
        }
        for (index, &(base, sheet)) in pairs.iter().enumerate() {
            if changed_skins.contains(&base) || changed_skins.contains(&sheet) {
                self.skins[skins.len() + index] =
                    skin_renderer.bind_glow(gpu, &self.skin_views[base], &self.skin_views[sheet]);
            }
        }
    }
}

#[cfg(test)]
mod tests;

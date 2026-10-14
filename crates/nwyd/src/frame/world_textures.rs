use crate::scene::objects::BoundTexture;
use render::device::Gpu;
use render::mesh::{MeshRenderer, MeshTexture};
use render::skin::{SkinRenderer, SkinTexture};
use render::texture;

type Key = (bool, String, bool);

fn key(t: &BoundTexture, skin: bool) -> Key {
    (
        skin,
        t.path.clone(),
        t.alpha_policy() == texture::AlphaPolicy::Opaque,
    )
}

#[derive(Default)]

pub(super) struct WorldTextures {
    pub meshes: Vec<MeshTexture>,
    pub skins: Vec<SkinTexture>,
    skin_views: Vec<wgpu::TextureView>,
    pub uploads: (usize, usize),
    staged: std::collections::HashMap<Key, wgpu::TextureView>,
    resident: std::collections::HashSet<Key>,
}

impl WorldTextures {
    pub fn clear(&mut self) {
        self.meshes.clear();

        self.skins.clear();

        self.skin_views.clear();

        self.staged.clear();

        self.resident.clear();
    }

    pub fn clear_staged(&mut self) {
        self.staged.clear();
    }

    pub fn needs_stage(&self, t: &BoundTexture, skin: bool) -> bool {
        let key = key(t, skin);

        !self.resident.contains(&key) && !self.staged.contains_key(&key)
    }

    pub fn stage(&mut self, gpu: &Gpu, t: &BoundTexture, skin: bool) {
        let view = texture::upload(
            gpu,
            &t.texture,
            t.alpha_policy(),
            "unpublished zone texture",
        )
        .view;

        self.staged.insert(key(t, skin), view);

        if skin {
            self.uploads.1 += 1;
        } else {
            self.uploads.0 += 1;
        }
    }

    fn view(&mut self, gpu: &Gpu, t: &BoundTexture, skin: bool) -> wgpu::TextureView {
        let key = key(t, skin);

        let view = (!t.pending)
            .then(|| self.staged.get(&key).cloned())
            .flatten()
            .unwrap_or_else(|| {
                if skin {
                    self.uploads.1 += 1;
                } else {
                    self.uploads.0 += 1;
                }

                texture::upload(gpu, &t.texture, t.alpha_policy(), &t.path).view
            });

        if !t.pending {
            self.resident.insert(key);
        }

        view
    }

    pub fn refresh_skins(
        &mut self,
        gpu: &Gpu,
        renderer: &SkinRenderer,
        textures: &[BoundTexture],
        pairs: &[(usize, usize)],
        updates: &[usize],
        old_plain: usize,
    ) {
        let changed: std::collections::HashSet<_> =
            updates.iter().copied().filter(|&i| i < old_plain).collect();

        for &index in &changed {
            let t = &textures[index];

            self.staged.remove(&key(t, true));

            self.resident.remove(&key(t, true));

            let view = self.view(gpu, t, true);

            self.skins[index] = renderer.bind_texture(gpu, &view);

            self.skin_views[index] = view;
        }

        for (i, &(base, sheet)) in pairs.iter().enumerate() {
            if changed.contains(&base) || changed.contains(&sheet) {
                self.skins[textures.len() + i] =
                    renderer.bind_glow(gpu, &self.skin_views[base], &self.skin_views[sheet]);
            }
        }
    }

    pub fn refresh_meshes(
        &mut self,
        gpu: &Gpu,
        renderer: &MeshRenderer,
        textures: &[BoundTexture],
        updates: &[usize],
        old_len: usize,
    ) {
        let changed: std::collections::BTreeSet<_> =
            updates.iter().copied().filter(|&i| i < old_len).collect();

        for index in changed {
            self.staged.remove(&key(&textures[index], false));

            self.resident.remove(&key(&textures[index], false));

            let view = self.view(gpu, &textures[index], false);

            self.meshes[index] = renderer.bind_texture(gpu, &view);
        }
    }

    pub fn sync_meshes(&mut self, gpu: &Gpu, renderer: &MeshRenderer, textures: &[BoundTexture]) {
        for t in &textures[self.meshes.len()..] {
            let view = self.view(gpu, t, false);

            self.meshes.push(renderer.bind_texture(gpu, &view));
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

        for t in &textures[plain..] {
            let view = self.view(gpu, t, true);

            self.skins.push(renderer.bind_texture(gpu, &view));

            self.skin_views.push(view);
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
}

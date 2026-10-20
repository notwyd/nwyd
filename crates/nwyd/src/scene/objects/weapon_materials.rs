use super::*;
use render::texture::MipPolicy;

impl ObjectScene {
    pub(super) fn prepare_held_materials(&mut self, id: u32) {
        let Some(&mesh) = self.by_id_meshes.get(&id) else {
            return;
        };

        let textures = self.meshes[mesh].subset_textures.clone();

        for index in textures.into_iter().flatten() {
            let source = &self.textures[index];

            if !matches!(source.mip_policy(false), MipPolicy::Cutout(_))
                || self.held_mask_textures.contains_key(&index)
            {
                continue;
            }

            let existing = self.textures.iter().position(|texture| {
                texture.slot == source.slot
                    && texture.effect_table == source.effect_table
                    && texture.path == source.path
                    && texture.flag == source.flag
                    && texture.mip_policy(false) == MipPolicy::Blend
            });

            let variant = match existing {
                Some(existing) => existing,
                None => {
                    let mut texture = source.clone();

                    texture.mip_policy_override = Some(MipPolicy::Blend);
                    texture.prepared = None;
                    self.textures.push(texture);

                    self.textures.len() - 1
                }
            };

            self.held_mask_textures.insert(index, variant);
        }
    }

    pub(super) fn held_materials_ready(&self, id: u32) -> bool {
        self.by_id_meshes.get(&id).is_some_and(|&mesh| {
            self.meshes[mesh]
                .subset_textures
                .iter()
                .flatten()
                .all(|&index| {
                    !matches!(self.textures[index].mip_policy(false), MipPolicy::Cutout(_))
                        || self.held_mask_textures.contains_key(&index)
                })
        })
    }

    pub(super) fn held_mask_texture(&self, texture: usize) -> usize {
        self.held_mask_textures
            .get(&texture)
            .copied()
            .unwrap_or(texture)
    }
}

#[cfg(test)]
mod tests;

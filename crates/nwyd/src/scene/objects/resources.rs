use super::*;

#[derive(Debug)]

pub(crate) struct Bundle {
    pub meshes: Vec<MeshResource>,
    pub textures: Vec<BoundTexture>,
    pub arena: MeshArena,
    by_id: BTreeMap<u32, usize>,
}

impl ObjectScene {
    pub(super) fn empty(registries: Arc<Registries>) -> Self {
        ObjectScene {
            records: Vec::new(),
            neighbour_records: Vec::new(),
            classes: ClassCounts::default(),
            meshes: Vec::new(),
            textures: Vec::new(),
            instances: Vec::new(),
            placements: Vec::new(),
            house_parts: Vec::new(),
            house_emitters: Vec::new(),
            roof_hidden: false,
            house_target: None,
            environment: 0,
            by_id_meshes: BTreeMap::new(),
            missing_by_id: Vec::new(),
            arena: MeshArena::default(),
            sky: None,
            missing_meshes: Vec::new(),
            missing_textures: Vec::new(),
            registries,
            held_loader: None,
            texture_loader: None,
            texture_updates: Vec::new(),
        }
    }

    pub(crate) fn poll_texture_cache(
        &mut self,
        game: &GameDir,
        actors: &super::super::actors::ActorScene,
    ) -> bool {
        let loader = self
            .texture_loader
            .get_or_insert_with(|| super::super::actors::texture_loading::Loader::new(game));

        let meshes = self
            .instances
            .iter()
            .map(|i| i.mesh)
            .chain(self.house_parts.iter().map(|p| p.mesh))
            .chain(
                actors
                    .held_mesh_ids()
                    .into_iter()
                    .filter_map(|id| self.by_id_meshes.get(&id).copied()),
            );

        let used = meshes
            .filter_map(|i| self.meshes.get(i))
            .flat_map(|m| m.subset_textures.iter().flatten().copied())
            .chain(
                self.textures
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| !t.effect_table && matches!(t.slot, 1 | 2 | 8))
                    .map(|(i, _)| i),
            );

        let used: Vec<_> = used.collect();

        let before = self.texture_updates.len();

        loader.maintain(
            &mut self.textures,
            &mut self.texture_updates,
            used.into_iter(),
            web_time::Instant::now(),
        );

        loader.poll(&mut self.textures, &mut self.texture_updates)
            || self.texture_updates.len() != before
    }

    pub(crate) fn install_resources(&mut self, bundle: Bundle) {
        let mut textures = Vec::new();

        for t in bundle.textures {
            let i = if let Some(i) = self
                .textures
                .iter()
                .position(|old| old.slot == t.slot && old.effect_table == t.effect_table)
            {
                if self.textures[i].pending && !t.pending {
                    self.textures[i] = t;

                    self.texture_updates.push(i);
                }

                i
            } else {
                self.textures.push(t);

                self.textures.len() - 1
            };

            textures.push(i);
        }

        let mut meshes = Vec::new();

        for mut mesh in bundle.meshes {
            let i = self
                .meshes
                .iter()
                .position(|old| old.id == mesh.id && old.path == mesh.path)
                .unwrap_or_else(|| {
                    mesh.alloc = self.arena.push(&mesh.mesh);

                    for texture in &mut mesh.subset_textures {
                        *texture = texture.map(|i| textures[i]);
                    }

                    self.meshes.push(mesh);

                    self.meshes.len() - 1
                });

            meshes.push(i);
        }

        for (id, i) in bundle.by_id {
            self.by_id_meshes.entry(id).or_insert(meshes[i]);
        }
    }
}

impl Bundle {
    pub(super) fn from_scene(scene: ObjectScene) -> Self {
        Self {
            meshes: scene.meshes,
            textures: scene.textures,
            arena: scene.arena,
            by_id: scene.by_id_meshes,
        }
    }

    pub(crate) fn prepare(
        game: &GameDir,
        reg: Arc<Registries>,
        records: &[ObjRecord],
        known: &[u32],
        glows: &[u32],
    ) -> Result<Self> {
        let mut scene = ObjectScene::empty(reg.clone());

        let mut done = std::collections::BTreeSet::new();

        for r in records {
            if r.class().uses_mesh_list_kind() && !known.contains(&r.kind) && done.insert(r.kind) {
                let _ = scene.load_mesh(game, &reg.list, &reg.textures, None, r.kind, true);
            }

            if r.class() == ObjectClass::House {
                scene.place_house(
                    Some(game),
                    &reg.list,
                    &reg.textures,
                    r.kind,
                    Vec3::ZERO,
                    r.yaw,
                );
            }
        }

        if records.iter().any(|r| reflective_scenery(r.kind)) {
            scene.load_effect_textures(game, &[0, 67, 68, 69, 70]);
        }

        if records.iter().any(|r| r.kind == 0x736) {
            let slots: Vec<_> = scene
                .meshes
                .iter()
                .filter(|m| m.id == 0x736)
                .flat_map(|m| m.subset_slots.iter().flatten().copied())
                .collect();

            scene.load_effect_textures(game, &slots);
        }

        scene.load_by_id(game, glows)?;

        Ok(Self::from_scene(scene))
    }
}

use super::*;

#[derive(Debug, Default)]

pub(super) struct PartCache {
    meshes: BTreeMap<String, Arc<SkinMesh>>,
    allocations: BTreeMap<String, usize>,
}

impl PartCache {
    fn snapshot(&self) -> Self {
        Self {
            meshes: self.meshes.clone(),
            allocations: BTreeMap::new(),
        }
    }

    fn resident(
        &mut self,
        path: &str,
        mesh: Arc<SkinMesh>,
        arena: &mut SkinArena,
    ) -> (Arc<SkinMesh>, usize) {
        if let (Some(mesh), Some(&alloc)) = (self.meshes.get(path), self.allocations.get(path)) {
            return (Arc::clone(mesh), alloc);
        }

        let mesh = Arc::clone(self.meshes.entry(path.to_owned()).or_insert(mesh));

        let alloc = *self
            .allocations
            .entry(path.to_owned())
            .or_insert_with(|| arena.push(&mesh));

        (mesh, alloc)
    }

    fn load(
        &mut self,
        game: &GameDir,
        path: &str,
        arena: &mut SkinArena,
    ) -> Result<Option<(Arc<SkinMesh>, usize)>> {
        if let (Some(mesh), Some(&alloc)) = (self.meshes.get(path), self.allocations.get(path)) {
            return Ok(Some((Arc::clone(mesh), alloc)));
        }

        if let Some(mesh) = self.meshes.get(path) {
            return Ok(Some(self.resident(path, Arc::clone(mesh), arena)));
        }

        let bytes = { game.read(path.as_bytes()) };

        let Ok(bytes) = bytes else {
            return Ok(None);
        };

        let mesh =
            { msh::parse_available_triangles(&bytes).with_context(|| format!("parsing {path}"))? };

        if mesh.indices.len() < mesh.header.num_indices as usize {
            tracing::warn!(
                % path, declared_indices = mesh.header.num_indices, retained_indices =
                mesh.indices.len(),
                "rendering complete triangles from a short retail mesh index tail"
            );
        }

        Ok(Some(self.resident(path, Arc::new(mesh), arena)))
    }
}

#[derive(Debug)]

pub(crate) struct Snapshot {
    known: std::collections::BTreeSet<ModelSpec>,
    animations: BTreeMap<u32, Option<LoadedAnimation>>,
    parts: PartCache,
}

#[derive(Debug, Default)]

pub(crate) struct Bundle {
    pub textures: Vec<BoundTexture>,
    pub models: Vec<Model>,
    pub arena: SkinArena,
    animations: BTreeMap<u32, Option<LoadedAnimation>>,
    by_spec: BTreeMap<ModelSpec, Option<usize>>,
    parts: PartCache,
}

impl ActorScene {
    pub(crate) fn resource_snapshot(&self) -> Snapshot {
        Snapshot {
            known: self.by_spec.keys().copied().collect(),
            animations: self.animations.clone(),
            parts: self.part_cache.snapshot(),
        }
    }

    pub(crate) fn install_resources(&mut self, bundle: Bundle) {
        let Bundle {
            textures,
            models,
            animations,
            by_spec,
            ..
        } = bundle;

        let mut remap = Vec::with_capacity(textures.len());

        for texture in textures {
            let existing = self
                .textures
                .iter()
                .position(|t| t.slot == texture.slot && t.effect_table == texture.effect_table);

            let index = match existing {
                Some(index) => {
                    if self.textures[index].pending && !texture.pending {
                        self.textures[index] = texture;

                        self.texture_updates.push(index);
                    }

                    index
                }
                None => {
                    self.textures.push(texture);

                    self.textures.len() - 1
                }
            };

            remap.push(index);
        }

        for (id, animation) in animations {
            self.animations.entry(id).or_insert(animation);
        }

        let mut keys = vec![None; models.len()];

        for (spec, index) in by_spec {
            if let Some(index) = index {
                keys[index] = Some(spec);
            } else {
                self.by_spec.entry(spec).or_insert(None);
            }
        }

        for (mut model, spec) in models.into_iter().zip(keys) {
            let Some(spec) = spec else {
                continue;
            };

            if self.by_spec.contains_key(&spec) {
                continue;
            }

            for part in &mut model.parts {
                part.texture = part.texture.map(|i| remap[i]);

                (part.mesh, part.alloc) = self.part_cache.resident(
                    &part.mesh_path,
                    Arc::clone(&part.mesh),
                    &mut self.arena,
                );
            }

            self.by_spec.insert(spec, Some(self.models.len()));

            self.models.push(model);
        }
    }
}

impl Bundle {
    pub(crate) fn prepare(
        game: &GameDir,
        reg: &Registries,
        snapshot: Snapshot,
        terrain: &Terrain,
        records: &[ObjRecord],
        heights: &[i8],
    ) -> Result<Self> {
        let mut specs = Vec::new();

        let spec = |model_id, part, slot, sub_frame_ms| {
            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[0] = part;

            ModelSpec {
                model_id,
                parts,
                clip: ClipChoice::Fixed { slot, sub_frame_ms },
                variant: 0,
                costume_body: 0,
            }
        };

        let zone = (terrain.zone_x, terrain.zone_y);

        let grass = grass::Field::build(records, terrain, heights, [0.0; 4], true);

        for (mesh, tex) in grass
            .blades
            .iter()
            .map(|b| (grass::Field::mesh_index(b.kind) as i16, b.colour as i16))
            .collect::<std::collections::BTreeSet<_>>()
        {
            specs.push(spec(
                grass::LEAF_MODEL,
                PartRef { mesh, tex },
                grass::LEAF_SLOT,
                grass::LEAF_SUB_FRAME_MS,
            ));
        }

        for (model, part) in
            trees::Grove::build(records, terrain, heights, [0.0; 4], [0.0; 4]).specs()
        {
            specs.push(spec(
                model,
                part,
                trees::TREE_SLOT,
                trees::TREE_SUB_FRAME_MS,
            ));
        }

        for mesh in fish::School::build(records, zone, 0).meshes() {
            specs.push(spec(
                fish::FISH_MODEL,
                PartRef { mesh, tex: 0 },
                fish::FISH_SLOT,
                fish::FISH_SUB_FRAME_MS,
            ));
        }

        for (model, tex, ms) in butterflies::Flight::build(records, zone).specs() {
            specs.push(spec(
                model,
                PartRef { mesh: 0, tex },
                butterflies::BUTTERFLY_SLOT,
                ms,
            ));
        }

        if floats::Raft::build(records, zone, |_, _| None, |_, _| [1.0; 4])
            .meshed()
            .next()
            .is_some()
        {
            specs.push(spec(
                floats::FLOAT_MODEL,
                PartRef::EMPTY,
                floats::FLOAT_SLOT,
                floats::FLOAT_SUB_FRAME_MS,
            ));
        }

        for ship in super::super::ships::build(records, zone) {
            specs.push(spec(60, ship.part_zero(), 0, 30));
        }

        let mut bundle = Self {
            animations: snapshot.animations,
            parts: snapshot.parts,
            ..Self::default()
        };

        let mut loader = Loader {
            models: &mut bundle.models,
            textures: &mut bundle.textures,
            arena: &mut bundle.arena,
            animations: &mut bundle.animations,
            by_spec: &mut bundle.by_spec,
            parts: &mut bundle.parts,
            background_textures: false,
        };

        for spec in specs {
            if !snapshot.known.contains(&spec) {
                loader.load_model(
                    game,
                    &reg.entries,
                    &reg.valid,
                    &reg.sounds,
                    &reg.textures,
                    &spec,
                )?;
            }
        }

        Ok(bundle)
    }
}

pub(super) struct Loader<'a> {
    pub models: &'a mut Vec<Model>,
    pub textures: &'a mut Vec<BoundTexture>,
    pub arena: &'a mut SkinArena,
    pub animations: &'a mut BTreeMap<u32, Option<LoadedAnimation>>,
    pub by_spec: &'a mut BTreeMap<ModelSpec, Option<usize>>,
    pub parts: &'a mut PartCache,
    pub background_textures: bool,
}

impl Loader<'_> {
    fn bind_slot(&mut self, game: &GameDir, textures: &TextureList, slot: usize) -> Option<usize> {
        self.bind_table_slot(game, textures, slot, false)
    }

    pub(super) fn load_model(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
        spec: &ModelSpec,
    ) -> Result<Option<usize>> {
        if let Some(&hit) = self.by_spec.get(spec) {
            return Ok(hit);
        }

        let ModelSpec {
            model_id,
            clip,
            parts: slots,
            variant,
            costume_body,
        } = *spec;

        let Some(entry) = entries.iter().find(|e| e.model_id == model_id) else {
            self.by_spec.insert(*spec, None);

            return Ok(None);
        };

        let Some(loaded) = self.load_animation(game, entry, valid)? else {
            self.by_spec.insert(*spec, None);

            return Ok(None);
        };

        let LoadedAnimation {
            animation,
            anim_slots,
        } = loaded;

        let (slot, sub_frame_ms) = match clip {
            ClipChoice::Fixed { slot, sub_frame_ms } => (slot, sub_frame_ms),
            ClipChoice::Action(action) => {
                match Model::from_action(sounds, &anim_slots, model_id, DEMO_ANIM_GROUP, action) {
                    Some(pair) => pair,
                    None => return Ok(None),
                }
            }
            ClipChoice::Mounted(action) => {
                let resolved = Model::from_mounted_action(
                    sounds,
                    &anim_slots,
                    model_id,
                    DEMO_ANIM_GROUP,
                    action,
                );

                match resolved {
                    Some(pair) => pair,
                    None => return Ok(None),
                }
            }
        };

        if animation.frames(slot).is_none() {
            return Ok(None);
        }

        let dir = entry.dir();

        let mut parts = Vec::new();

        let mut missing_parts = Vec::new();

        let substitute = match costume_body {
            0 | costume::CAPE_BODY => Vec::new(),
            body => costume::body_substitution(body, entry.num_parts as usize),
        };

        for part in 0..entry.num_parts {
            if model_id < 19 && matches!(part, 6 | 7) {
                continue;
            }

            let slot = slots[(part as usize).min(appearance::PARTS - 1)];

            if !skin_part_admitted(slots[0].mesh, part, slot.mesh) {
                continue;
            }

            let (mesh_index, tex_index) = (slot.mesh.max(0) as u32, slot.tex.max(0) as u32);

            let mut file_name = entry.part_mesh_file_name(part, mesh_index, variant);

            let mut texture_name =
                entry.part_texture_file_name(part, mesh_index, variant, tex_index);

            entry.substitute_part_names(part, &mut file_name, &mut texture_name);

            if model_id == 85 && costume_body == 100 {
                file_name = "newmt.msh".to_string();

                texture_name = match slot.tex {
                    0 => "newmtB000.wys",
                    1 => "newmtR000.wys",
                    _ => "newmtW000.wys",
                }
                .to_string();
            }

            let named = substitute.get(part as usize).copied().flatten();

            if let Some(name) = named.and_then(|n| n.mesh) {
                file_name = name.to_string();
            }

            if let Some(name) = named.and_then(|n| n.texture) {
                texture_name = name.to_string();
            }

            let path = format!(r"{dir}\{file_name}");

            let mesh_path = assets::fold(path.as_bytes());

            let Some((mesh, alloc)) = self.parts.load(game, &mesh_path, self.arena)? else {
                missing_parts.push(file_name);

                continue;
            };

            let found = textures.find(format!(r"{dir}\{texture_name}").as_bytes());

            let texture = found.and_then(|s| self.bind_slot(game, textures, s));

            parts.push(Part {
                index: part as usize,
                file_name,
                mesh_path,
                mesh,
                alloc,
                texture_name,
                texture,
            });
        }

        self.models.push(Model {
            model_id,
            base_name: entry.base_name().to_string(),
            animation,
            parts,
            slot,
            sub_frame_ms,
            missing_parts,
        });

        let index = self.models.len() - 1;

        self.by_spec.insert(*spec, Some(index));

        Ok(Some(index))
    }

    pub(super) fn load_animation(
        &mut self,
        game: &GameDir,
        entry: &ModelEntry,
        valid: &ValidIndex,
    ) -> Result<Option<LoadedAnimation>> {
        if let Some(hit) = self.animations.get(&entry.model_id) {
            return Ok(hit.clone());
        }

        let dir = entry.dir();

        let bon_path = format!(r"{dir}\{}", entry.skeleton_file_name());

        let bon_bytes = {
            game.read(bon_path.as_bytes())
                .with_context(|| format!("reading {bon_path}"))?
        };

        let skeleton = { bon::parse(&bon_bytes).with_context(|| format!("parsing {bon_path}"))? };

        let mut clips = Vec::new();

        for name in valid.clip_names(entry) {
            let path = format!(r"{dir}\{name}");

            let bytes = { game.read(path.as_bytes()) };

            match bytes {
                Ok(bytes) => {
                    clips.push(Some(
                        ani::parse(&bytes).with_context(|| format!("parsing {path}"))?,
                    ));
                }
                Err(_) => clips.push(None),
            }
        }

        let present: Vec<bool> = clips.iter().map(Option::is_some).collect();

        let slots: Vec<Slot> = clips
            .iter()
            .map(|c| c.as_ref().map_or(Slot::Missing, Slot::Loaded))
            .collect();

        let animation = ModelAnimation::assemble(entry, skeleton, bon_bytes.len(), &slots)
            .with_context(|| format!("assembling model {}", entry.model_id))?;

        let loaded = LoadedAnimation {
            animation: Arc::new(animation),
            anim_slots: Arc::new(registry::AnimSlotTable::build(
                entry.model_id,
                valid,
                entry,
                &present,
            )),
        };

        self.animations.insert(entry.model_id, Some(loaded.clone()));

        Ok(Some(loaded))
    }

    pub(super) fn bind_table_slot(
        &mut self,
        game: &GameDir,
        textures: &TextureList,
        slot: usize,
        effect_table: bool,
    ) -> Option<usize> {
        if let Some(i) = self
            .textures
            .iter()
            .position(|t| t.slot == slot && t.effect_table == effect_table)
        {
            return Some(i);
        }

        let entry = textures.get(slot)?;

        if self.background_textures {
            self.textures.push(BoundTexture {
                pending: true,
                effect_table,
                slot,
                path: entry.path_str(),
                flag: entry.format_flag,
                texture: super::texture_loading::placeholder(),
            });

            return Some(self.textures.len() - 1);
        }

        let bytes = { game.read(&entry.path).ok()? };

        let texture = { formats::texture::decode(&bytes).ok()? };

        self.textures.push(BoundTexture {
            pending: false,
            effect_table,
            slot,
            path: entry.path_str(),
            flag: entry.format_flag,
            texture,
        });

        Some(self.textures.len() - 1)
    }
}

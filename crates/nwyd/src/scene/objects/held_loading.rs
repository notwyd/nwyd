use super::*;
use std::{collections::BTreeSet, sync::mpsc};
use web_time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]

enum Key {
    Mesh(u32),
    Effect(usize),
}

impl Key {
    fn resident(self, scene: &ObjectScene) -> bool {
        match self {
            Self::Mesh(id) => scene.by_id_meshes.contains_key(&id),
            Self::Effect(slot) => scene
                .textures
                .iter()
                .any(|t| t.effect_table && t.slot == slot),
        }
    }

    fn prepare(self, game: &GameDir, registries: Arc<Registries>) -> Result<resources::Bundle> {
        let mut scene = ObjectScene::empty(registries.clone());

        match self {
            Self::Mesh(id) => {
                scene.load_by_id(game, &[id])?;

                if let Some(&index) = scene.by_id_meshes.get(&id) {
                    let mesh = &scene.meshes[index];

                    anyhow::ensure!(
                        mesh.subset_slots
                            .iter()
                            .zip(&mesh.subset_textures)
                            .all(|(slot, texture)| slot.is_none() || texture.is_some()),
                        "held mesh base texture is unavailable"
                    );
                }
            }
            Self::Effect(slot) => {
                scene.bind_slot(game, &registries.effect_textures, true, slot);
            }
        }

        anyhow::ensure!(self.resident(&scene), "held resource is unavailable");

        Ok(resources::Bundle::from_scene(scene))
    }
}

fn wanted(scene: &ObjectScene, actor: &super::super::actors::Actor) -> BTreeSet<Key> {
    let mut keys = BTreeSet::new();

    for hand in &actor.held {
        keys.insert(Key::Mesh(hand.mesh_id));

        if let Some(slot) = hand.glow {
            keys.insert(Key::Effect(slot));

            keys.insert(Key::Effect(super::super::weapon_glow::after_draw(
                hand.mesh_id,
                slot,
            )));
        }

        if let Some(glow) = scene.ordinary_held_glow_for_request(hand.mesh_id, hand.ordinary_glow) {
            keys.insert(Key::Effect(glow));
        }
    }

    if let Some(id) = super::super::siege_mesh::mesh(actor.parts[7].mesh) {
        keys.insert(Key::Mesh(id));
    }

    keys
}

type Reply = (Key, Result<resources::Bundle>);

#[derive(Debug)]

struct Worker {
    requests: mpsc::SyncSender<Key>,
    replies: mpsc::Receiver<Reply>,
}

impl Worker {
    fn new(game: GameDir, registries: Arc<Registries>) -> std::io::Result<Self> {
        let (requests, work) = mpsc::sync_channel::<Key>(1);

        let (done, replies) = mpsc::sync_channel(1);

        crate::background::spawn("nwyd-held-loader", work, done, move |key| {
            let start = Instant::now();

            let result = key.prepare(&game, registries.clone());

            tracing::debug!(
                ?key,
                elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
                "held resource background preparation completed"
            );

            (key, result)
        })?;

        Ok(Self { requests, replies })
    }
}

#[derive(Debug)]

pub(super) struct Loader {
    game: GameDir,
    worker: Option<Worker>,
    pending: Option<Key>,
    retry: BTreeMap<Key, Instant>,
    dirty: bool,
    next_scan: Option<Instant>,
    restart_after: Instant,
}

impl Loader {
    pub(super) fn invalidate(&mut self) {
        self.dirty = true;
    }

    fn poll(
        &mut self,
        scene: &mut ObjectScene,
        actors: &mut super::super::actors::ActorScene,
        player: Option<usize>,
    ) -> bool {
        let now = Instant::now();

        if self.worker.is_none() {
            if now < self.restart_after {
                return false;
            }

            match Worker::new(self.game.clone(), scene.registries.clone()) {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    self.restart_after = now + Duration::from_secs(1);

                    tracing::warn!(
                        % error, "held resource worker could not start; will retry"
                    );

                    return false;
                }
            }
        }

        let mut published = false;

        match self.worker.as_ref().unwrap().replies.try_recv() {
            Ok((key, result)) => {
                self.pending = None;

                self.dirty = true;

                let needed = actors
                    .actors
                    .iter()
                    .any(|a| wanted(scene, a).contains(&key));

                match result {
                    Ok(bundle) if needed && !key.resident(scene) => {
                        scene.install_resources(bundle);

                        published = true;
                    }
                    Err(error) if needed && !key.resident(scene) => {
                        if self.retry.len() == 128 {
                            let oldest = *self.retry.iter().min_by_key(|(_, t)| *t).unwrap().0;

                            self.retry.remove(&oldest);
                        }

                        self.retry.insert(key, now + Duration::from_secs(1));

                        tracing::debug!(
                            ? key, % error, "held resource unavailable; will retry"
                        );
                    }
                    _ => {}
                }

                if needed
                    && key.resident(scene)
                    && let Key::Mesh(id) = key
                {
                    for index in 0..actors.actors.len() {
                        if actors.actors[index].held.iter().any(|h| h.mesh_id == id) {
                            actors.place_actor_swing_radii(index, |id| scene.mesh_max_z(id));
                        }
                    }

                    actors.siege_meshes = scene.resident_siege_meshes();
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.worker = None;

                self.pending = None;

                self.dirty = true;

                self.restart_after = now + Duration::from_secs(1);

                tracing::warn!("held resource worker disconnected; will restart");

                return false;
            }
        }

        if self.pending.is_some()
            || (!self.dirty && self.next_scan.is_none_or(|deadline| now < deadline))
        {
            return published;
        }

        self.dirty = false;

        self.retry.retain(|_, deadline| *deadline > now);

        let origin = player
            .and_then(|i| actors.actors.get(i))
            .map(|a| a.position);

        let mut order: Vec<_> = (0..actors.actors.len()).collect();

        order.sort_by(|&a, &b| {
            let distance = |i: usize| {
                if Some(i) == player {
                    return -1.0;
                }

                origin.map_or(0.0, |p| (actors.actors[i].position - p).length_squared())
            };

            distance(a).total_cmp(&distance(b))
        });

        let key = order.into_iter().find_map(|index| {
            wanted(scene, &actors.actors[index])
                .into_iter()
                .find(|key| !key.resident(scene) && !self.retry.contains_key(key))
        });

        if let Some(key) = key {
            match self.worker.as_ref().unwrap().requests.try_send(key) {
                Ok(()) => self.pending = Some(key),
                Err(_) => {
                    self.worker = None;

                    self.dirty = true;

                    self.restart_after = now + Duration::from_secs(1);
                }
            }
        }

        self.next_scan = self.retry.values().copied().min();

        published
    }
}

impl ObjectScene {
    pub fn enable_background_held(&mut self, game: &GameDir) {
        self.held_loader.get_or_insert_with(|| Loader {
            game: game.clone(),
            worker: None,
            pending: None,
            retry: BTreeMap::new(),
            dirty: true,
            next_scan: None,
            restart_after: Instant::now(),
        });
    }

    pub fn poll_held_resources(
        &mut self,
        actors: &mut super::super::actors::ActorScene,
        player: Option<usize>,
    ) -> bool {
        let Some(mut loader) = self.held_loader.take() else {
            return false;
        };

        let published = loader.poll(self, actors, player);

        self.held_loader = Some(loader);

        published
    }

    fn ordinary_held_glow_for_request(
        &self,
        id: u32,
        glow: Option<super::super::weapon_glow::Ordinary>,
    ) -> Option<usize> {
        glow?
            .resolve(
                self.registries
                    .textures
                    .get(0)
                    .is_some_and(|t| t.format_flag == FormatFlag::ColorKey),
            )
            .filter(|_| self.registries.list.get(id).is_some())
            .map(|g| g.texture as usize)
    }
}

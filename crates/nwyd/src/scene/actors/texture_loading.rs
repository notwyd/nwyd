use super::*;
use formats::texture::{MipLevel, PixelFormat, Texture};
use render::texture::{MipPolicy, PreparedMaterial};
use std::{
    collections::BTreeSet,
    sync::{Arc, mpsc},
};
use web_time::{Duration, Instant};

#[cfg(test)]
mod tests;

pub(crate) fn placeholder() -> Texture {
    Texture {
        width: 1,
        height: 1,
        format: PixelFormat::Rgba8Unorm,
        data: vec![0; 4],
        source_len: 0,
        mips: vec![MipLevel {
            width: 1,
            height: 1,
            offset: 0,
            len: 4,
        }],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
    index: usize,
    path: String,
    policy: MipPolicy,
    generation: u64,
}

impl Request {
    fn matches(&self, textures: &[BoundTexture], skin: bool) -> bool {
        textures.get(self.index).is_some_and(|texture| {
            texture.pending && texture.path == self.path && texture.mip_policy(skin) == self.policy
        })
    }
}

#[derive(Debug)]
struct Loaded {
    texture: Arc<Texture>,
    prepared: Arc<PreparedMaterial>,
}

type Reply = (Request, Result<Loaded>);

#[derive(Debug)]
struct Worker {
    requests: mpsc::SyncSender<Request>,
    replies: mpsc::Receiver<Reply>,
}

impl Worker {
    fn new(game: GameDir) -> std::io::Result<Self> {
        let (requests, work) = mpsc::sync_channel::<Request>(1);

        let (done, replies) = mpsc::sync_channel(1);

        crate::background::spawn("nwyd-skin-textures", work, done, move |request| {
            let start = Instant::now();

            let result = game
                .read(request.path.as_bytes())
                .map_err(anyhow::Error::from)
                .and_then(|bytes| formats::texture::decode(&bytes).map_err(Into::into))
                .map(|texture| {
                    let texture = Arc::new(texture);
                    let prepared = Arc::new(PreparedMaterial::from_shared(
                        Arc::clone(&texture),
                        request.policy,
                    ));
                    Loaded { texture, prepared }
                });

            tracing::debug!(
                elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
                "skin texture background preparation completed"
            );

            (request, result)
        })?;

        Ok(Self { requests, replies })
    }
}

#[derive(Debug)]
pub(crate) struct Loader {
    game: GameDir,
    skin: bool,
    worker: Option<Worker>,
    pending: Option<Request>,
    generation: u64,
    scanned: usize,
    wanted: BTreeSet<usize>,
    retry: BTreeMap<usize, (Request, Instant)>,
    restart: Instant,
    touched: BTreeMap<usize, Instant>,
    evicted: BTreeSet<usize>,
    sweep: Instant,
    dirty: bool,
}

impl Loader {
    pub(crate) fn new(game: &GameDir, skin: bool) -> Self {
        Self {
            game: game.clone(),
            skin,
            worker: None,
            pending: None,
            generation: 0,
            scanned: 0,
            wanted: BTreeSet::new(),
            retry: BTreeMap::new(),
            restart: Instant::now(),
            touched: BTreeMap::new(),
            evicted: BTreeSet::new(),
            sweep: Instant::now(),
            dirty: false,
        }
    }

    pub(crate) fn maintain(
        &mut self,
        textures: &mut [BoundTexture],
        updates: &mut Vec<usize>,
        used: impl Iterator<Item = usize>,
        now: Instant,
    ) {
        for index in used {
            if let Some(texture) = textures.get(index)
                && !texture.effect_table
                && texture.slot < 3048
            {
                self.touched.insert(index, now);

                if self.evicted.remove(&index) {
                    self.wanted.insert(index);
                }
            }
        }

        if now.duration_since(self.sweep) <= Duration::from_millis(4999) {
            return;
        }

        self.sweep = now;

        for (index, texture) in textures.iter_mut().enumerate() {
            if texture.effect_table || texture.slot >= 3048 || texture.pending {
                continue;
            }

            let touched = self.touched.entry(index).or_insert(now);

            if now.duration_since(*touched) > Duration::from_millis(120_000) {
                texture.texture = placeholder().into();

                texture.prepared = None;

                texture.pending = true;

                updates.push(index);

                self.dirty = true;

                self.evicted.insert(index);
            }
        }
    }

    pub(crate) fn poll(&mut self, textures: &mut [BoundTexture], updates: &mut Vec<usize>) -> bool {
        let mut changed = std::mem::take(&mut self.dirty);

        self.wanted
            .extend((self.scanned..textures.len()).filter(|&i| textures[i].pending));

        self.scanned = textures.len();

        self.wanted.retain(|&i| {
            textures.get(i).is_some_and(|texture| texture.pending) && !self.evicted.contains(&i)
        });

        self.evicted.retain(|&i| i < textures.len());

        self.touched.retain(|&i, _| i < textures.len());

        let now = Instant::now();

        if self.worker.is_none() {
            if self.wanted.is_empty() || now < self.restart {
                return changed;
            }

            match Worker::new(self.game.clone()) {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    self.restart = now + Duration::from_secs(1);

                    tracing::warn!(
                        % error, "skin texture worker could not start; will retry"
                    );

                    return changed;
                }
            }
        }

        match self.worker.as_ref().unwrap().replies.try_recv() {
            Ok((request, result)) => {
                if self.pending.as_ref() != Some(&request) {
                    return changed;
                }

                self.pending = None;

                let index = request.index;

                if request.matches(textures, self.skin) {
                    match result {
                        Ok(loaded) => {
                            textures[index].texture = loaded.texture;

                            textures[index].prepared = Some(loaded.prepared);

                            textures[index].pending = false;

                            updates.push(index);

                            self.wanted.remove(&index);

                            self.retry.remove(&index);

                            changed = true;
                        }
                        Err(error) => {
                            self.retry
                                .insert(index, (request, now + Duration::from_millis(15_001)));

                            tracing::debug!(
                                % error, "skin texture unavailable; will retry"
                            );
                        }
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.worker = None;

                self.pending = None;

                self.restart = now + Duration::from_secs(1);

                tracing::warn!("skin texture worker disconnected; will restart");

                return changed;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if self.pending.is_none() {
            self.retry
                .retain(|_, (request, until)| request.matches(textures, self.skin) && *until > now);

            if let Some(&index) = self.wanted.iter().find(|i| !self.retry.contains_key(i)) {
                self.generation = self.generation.wrapping_add(1);

                let request = Request {
                    index,
                    path: textures[index].path.clone(),
                    policy: textures[index].mip_policy(self.skin),
                    generation: self.generation,
                };

                if self
                    .worker
                    .as_ref()
                    .unwrap()
                    .requests
                    .try_send(request.clone())
                    .is_ok()
                {
                    self.pending = Some(request);
                } else {
                    self.worker = None;

                    self.restart = now + Duration::from_secs(1);
                }
            }
        }

        changed
    }
}

impl ActorScene {
    pub(super) fn maintain_texture_cache(&mut self, draws: &[SkinDraw]) {
        let Some(loader) = &mut self.texture_loader else {
            return;
        };

        let count = self.textures.len();

        let used = draws
            .iter()
            .filter_map(|d| d.texture)
            .flat_map(|index| {
                if index < count {
                    [Some(index), None]
                } else {
                    self.glow_pairs
                        .get(index - count)
                        .map_or([None, None], |&(a, b)| [Some(a), Some(b)])
                }
            })
            .flatten();

        let rigs = self
            .actors
            .iter()
            .flat_map(|actor| {
                [
                    Some(actor.model),
                    actor.mount.as_ref().map(|r| r.model),
                    actor.cape.as_ref().map(|r| r.model),
                ]
            })
            .flatten()
            .chain(self.grove.trees.iter().filter_map(|v| v.model))
            .chain(self.grass.field.blades.iter().filter_map(|v| v.model))
            .chain(self.school.fish.iter().filter_map(|v| v.model))
            .chain(self.flight.butterflies.iter().filter_map(|v| v.model))
            .chain(self.ships.iter().filter_map(|v| v.model))
            .chain(
                self.skinfx
                    .objects
                    .iter()
                    .flat_map(|v| [v.rig.as_ref(), v.rig2.as_ref()])
                    .flatten()
                    .map(|r| r.model),
            )
            .filter_map(|i| self.models.get(i))
            .flat_map(|model| model.parts.iter().filter_map(|part| part.texture));

        loader.maintain(
            &mut self.textures,
            &mut self.texture_updates,
            used.chain(rigs),
            Instant::now(),
        );
    }

    pub fn enable_background_textures(&mut self, game: &GameDir) {
        self.texture_loader
            .get_or_insert_with(|| Loader::new(game, true));
    }

    pub fn poll_background_textures(&mut self) -> bool {
        let Some(loader) = &mut self.texture_loader else {
            return false;
        };

        loader.poll(&mut self.textures, &mut self.texture_updates)
    }
}

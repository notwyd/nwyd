use super::*;
use formats::world::objects::{self as records, ObjRecord};
use std::{collections::HashMap, sync::mpsc};
use web_time::{Duration, Instant};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(crate) struct Prepared {
    pub serial: u64,
    pub terrain: Terrain,
    pub mesh: TerrainMesh,
    pub records: Vec<ObjRecord>,
    pub tiles: Vec<Tile>,
    pub resources: Option<Resources>,
}

#[derive(Debug)]
pub(crate) struct Resources {
    pub objects: objects::resources::Bundle,
    pub actors: actors::resources::Bundle,
    pub heights: Vec<i8>,
    pub relief: Vec<i8>,
}

#[derive(Debug)]
pub(super) struct Inputs {
    pub registries: Arc<Registries>,
    pub actors: actors::resources::Snapshot,
    pub meshes: Vec<u32>,
    pub scene: u32,
}

#[derive(Debug)]
struct Request {
    zone: (u8, u8),
    resident: Vec<i32>,
    missing: Vec<i32>,
    inputs: Option<Inputs>,
}

pub(super) fn prepare(
    game: &GameDir,
    list: &EnvTextureList,
    zone: (u8, u8),
    resident: &[i32],
    missing: &[i32],
) -> Result<Option<Prepared>> {
    let name = format!("Field{:02}{:02}", zone.0, zone.1);

    let path = format!("Env/{name}.trn");

    let Ok(bytes) = game.read(path.as_bytes()) else {
        return Ok(None);
    };

    let terrain = trn::parse(&bytes).with_context(|| format!("parsing {path}"))?;

    let path = format!("Env/{name}.dat");

    let records = records::parse(
        &game
            .read(path.as_bytes())
            .with_context(|| format!("reading {path}"))?,
    )
    .with_context(|| format!("parsing {path}"))?;

    let mesh = world::build_terrain(&terrain);

    let mut tiles = Vec::new();

    let ids = mesh
        .batches
        .iter()
        .flat_map(|b| {
            std::iter::once(b.base_texture_id)
                .chain(b.material.needs_detail().then_some(b.detail_texture_id))
        })
        .chain(missing.iter().copied())
        .filter(|id| resident.binary_search(id).is_err());

    super::tiles::load_ids(game, list, ids, &mut tiles)?;

    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    Ok(Some(Prepared {
        serial: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        terrain,
        mesh,
        records,
        tiles,
        resources: None,
    }))
}

fn prepare_resources(game: &GameDir, prepared: &mut Prepared, inputs: Inputs) -> Result<()> {
    use formats::world::{collision, object_bin};

    let relief = prepared.terrain.build_height_grid();

    let mut heights = relief.clone();

    let masks = object_bin::parse(
        &game
            .read(actors::OBJECT_BIN)
            .context("reading object.bin")?,
    )
    .context("parsing object.bin")?;

    collision::stamp_all(&mut heights, &masks, &prepared.records);

    let mut terrain = prepared.terrain.clone();

    let zone = (terrain.zone_x, terrain.zone_y);

    let lights = lights::build(
        &mut lights::Zones::new(vec![&mut terrain]),
        &mut lights::Lamps::new(inputs.scene),
        &prepared.records,
        zone,
        inputs.scene,
    );

    let ids: Vec<_> = lights.glows.iter().map(|g| g.mesh_id as u32).collect();

    let objects = objects::resources::Bundle::prepare(
        game,
        inputs.registries.clone(),
        &prepared.records,
        &inputs.meshes,
        &ids,
    )?;

    let actors = actors::resources::Bundle::prepare(
        game,
        &inputs.registries,
        inputs.actors,
        &prepared.terrain,
        &prepared.records,
        &heights,
    )?;

    prepared.resources = Some(Resources {
        objects,
        actors,
        heights,
        relief,
    });

    Ok(())
}

type Reply = ((u8, u8), Result<Option<Prepared>>);

#[derive(Debug)]
pub(super) struct Loader {
    requests: mpsc::SyncSender<Request>,
    replies: mpsc::Receiver<Reply>,
    pending: Option<(u8, u8)>,
    ready: Option<((u8, u8), Prepared)>,
    retry: HashMap<(u8, u8), Instant>,
    disconnected: bool,
}

impl Loader {
    pub fn is_idle(&self) -> bool {
        self.pending.is_none() && self.ready.is_none()
    }

    pub fn active_zone(&self) -> Option<(u8, u8)> {
        self.ready.as_ref().map(|(zone, _)| *zone).or(self.pending)
    }

    pub fn can_request(&self, zone: (u8, u8)) -> bool {
        !self.disconnected
            && !self
                .retry
                .get(&zone)
                .is_some_and(|until| *until > Instant::now())
    }

    pub fn discard(&mut self, zone: (u8, u8)) -> Result<bool> {
        if !self.is_idle() {
            self.take_or_stage(zone, &[], &[], || None, &mut |_| true)?;
        }

        Ok(self.is_idle())
    }

    pub fn new(game: GameDir, list: EnvTextureList) -> std::io::Result<Self> {
        let (requests, work) = mpsc::sync_channel::<Request>(1);

        let (done, replies) = mpsc::sync_channel(1);

        crate::background::spawn("nwyd-zone-loader", work, done, move |request| {
            let start = Instant::now();

            let result = prepare(
                &game,
                &list,
                request.zone,
                &request.resident,
                &request.missing,
            )
            .and_then(|mut prepared| {
                if let (Some(prepared), Some(inputs)) = (&mut prepared, request.inputs) {
                    prepare_resources(&game, prepared, inputs)?;
                }

                Ok(prepared)
            });

            tracing::debug!(
                zone = ? request.zone, elapsed_ms = start.elapsed().as_secs_f64()
                * 1000.0, "zone background preparation completed"
            );

            (request.zone, result)
        })?;

        Ok(Self {
            requests,
            replies,
            pending: None,
            ready: None,
            retry: HashMap::new(),
            disconnected: false,
        })
    }

    pub fn take_or_stage(
        &mut self,
        zone: (u8, u8),
        resident: &[Tile],
        missing: &[i32],
        inputs: impl FnOnce() -> Option<Inputs>,
        stage: &mut impl FnMut(&Prepared) -> bool,
    ) -> Result<Option<Prepared>> {
        match self.replies.try_recv() {
            Ok((key, result)) => {
                self.pending = None;

                match result {
                    Ok(Some(prepared)) => {
                        self.ready = Some((key, prepared));
                    }
                    result => {
                        self.retry
                            .insert(key, Instant::now() + Duration::from_secs(1));

                        if let Err(error) = result {
                            tracing::warn!(
                                zone = ? key, % error,
                                "zone background preparation failed; will retry"
                            );
                        }
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.disconnected = true;
            }
        }

        anyhow::ensure!(!self.disconnected, "zone loader disconnected");

        if self.ready.as_ref().is_some_and(|(key, _)| *key == zone) {
            return Ok(if stage(&self.ready.as_ref().unwrap().1) {
                self.ready.take().map(|(_, prepared)| prepared)
            } else {
                None
            });
        }

        if self.pending.is_none()
            && !self
                .retry
                .get(&zone)
                .is_some_and(|until| *until > Instant::now())
        {
            self.requests.try_send(Request {
                zone,
                resident: resident.iter().map(|t| t.id).collect(),
                missing: missing.to_vec(),
                inputs: inputs(),
            })?;

            self.pending = Some(zone);
        }

        Ok(None)
    }
}

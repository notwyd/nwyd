use super::Tile;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::world::env_texture_list::EnvTextureList;
use render::world::TerrainMesh;

pub(super) fn load(
    game: &GameDir,
    list: &EnvTextureList,
    mesh: &TerrainMesh,
    tiles: &mut Vec<Tile>,
) -> Result<Vec<i32>> {
    load_ids(
        game,
        list,
        mesh.batches.iter().flat_map(|b| {
            std::iter::once(b.base_texture_id)
                .chain(b.material.needs_detail().then_some(b.detail_texture_id))
        }),
        tiles,
    )
}

pub(super) fn load_ids(
    game: &GameDir,
    list: &EnvTextureList,
    ids: impl Iterator<Item = i32>,
    tiles: &mut Vec<Tile>,
) -> Result<Vec<i32>> {
    extend(tiles, ids, |id| {
        let Some(entry) = usize::try_from(id).ok().and_then(|i| list.get(i)) else {
            return Ok(None);
        };

        let Ok(bytes) = game.read(entry.path.as_bytes()) else {
            return Ok(None);
        };

        let texture = formats::texture::decode(&bytes)
            .with_context(|| format!("decoding Env tile {id}: {}", entry.path))?;

        Ok(Some(Tile {
            id,
            path: entry.path.clone(),
            texture,
        }))
    })
}

fn extend(
    tiles: &mut Vec<Tile>,
    ids: impl Iterator<Item = i32>,
    mut read: impl FnMut(i32) -> Result<Option<Tile>>,
) -> Result<Vec<i32>> {
    let mut ids: Vec<_> = ids.collect();

    ids.sort_unstable();

    ids.dedup();

    let mut added = Vec::new();

    let mut missing = Vec::new();

    for id in ids {
        if tiles.binary_search_by_key(&id, |t| t.id).is_ok() {
            continue;
        }

        match read(id)? {
            Some(tile) => added.push(tile),
            None => missing.push(id),
        }
    }

    if !added.is_empty() {
        tiles.extend(added);

        tiles.sort_unstable_by_key(|t| t.id);
    }

    Ok(missing)
}

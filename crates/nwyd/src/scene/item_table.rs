use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::item_list::{self, ItemList};
use formats::text::Codepage;

pub const ITEM_LIST: &[u8] = b"ItemList.bin";

pub const EXTRA_ITEM: &[u8] = b"extraitem.bin";

pub const ITEM_PRICE: &[u8] = b"ItemPrice.bin";

pub fn load(game: &GameDir) -> Result<ItemList> {
    let raw = game.read(ITEM_LIST).context("reading ItemList.bin")?;

    let patch = game.read(EXTRA_ITEM).unwrap_or_default();

    let mut items = merge(&raw, &patch)?;

    if let Ok(prices) = game.read(ITEM_PRICE) {
        items.apply_prices(&formats::data::item_price::parse(&prices));
    }

    Ok(items)
}

pub fn merge(raw: &[u8], patch: &[u8]) -> Result<ItemList> {
    if patch.is_empty() {
        return item_list::parse(raw, Codepage::Windows1252).context("parsing ItemList.bin");
    }

    item_list::parse_with_patch(raw, patch, Codepage::Windows1252)
        .context("parsing ItemList.bin with extraitem.bin merged")
}

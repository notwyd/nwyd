#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Update {
    pub prices: [i32; 2],
}

pub fn parse(payload: &[u8]) -> Option<Update> {
    let words = payload.get(..8)?;

    Some(Update {
        prices: [
            i32::from_le_bytes(words[..4].try_into().ok()?),
            i32::from_le_bytes(words[4..8].try_into().ok()?),
        ],
    })
}

impl Update {
    pub fn apply(self, items: &mut crate::data::item_list::ItemList) {
        for (id, price) in [412, 413].into_iter().zip(self.prices) {
            items.set_price(id, price);
        }
    }
}

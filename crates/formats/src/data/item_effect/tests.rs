use super::*;
use crate::data::{ITEM_RECORD_LEN, item_list};
use crate::net::account::Effect;
use crate::text::Codepage;

fn items_with_use_kind(ids: &[u16]) -> ItemList {
    let mut raw = vec![0; item_list::BODY_LEN];

    for &id in ids {
        let offset = usize::from(id) * ITEM_RECORD_LEN + item_list::field::EFFECT;

        raw[offset..offset + 2].copy_from_slice(&i16::from(effect::USE_KIND).to_le_bytes());
        raw[offset + 2..offset + 4].copy_from_slice(&243i16.to_le_bytes());
    }

    ItemList::from_plain(raw, Codepage::Windows1252)
}

#[test]
fn scaled_total_reads_jewel_effects_without_changing_other_accessors() {
    let ids = [3200, 3203, 3300];
    let items = items_with_use_kind(&ids);
    let effects = Effects::new(&items, &[]);

    for index in ids {
        let item = Item {
            index,
            effects: [
                Effect {
                    id: effect::DAMAGE,
                    value: 7,
                },
                Effect::default(),
                Effect::default(),
            ],
            ..Item::default()
        };

        assert_eq!(effects.total_scaled(&item, effect::USE_KIND), 243);
        assert_eq!(effects.total_scaled(&item, effect::DAMAGE), 7);

        for kind in [effect::USE_KIND, effect::DAMAGE] {
            assert_eq!(effects.template(&item, kind), 0);
            assert_eq!(effects.slot(&item, kind), 0);
            assert_eq!(effects.slot_scaled(&item, kind), 0);
            assert_eq!(effects.total(&item, kind), 0);
        }
    }
}

#[test]
fn scaled_total_preserves_item_id_bounds() {
    let items = items_with_use_kind(&[1, MAX_ID as u16]);
    let effects = Effects::new(&items, &[]);

    for index in [1, MAX_ID as u16] {
        let item = Item {
            index,
            ..Item::default()
        };

        assert_eq!(effects.total_scaled(&item, effect::USE_KIND), 243);
    }

    for index in [0, MAX_ID as u16 + 1, 0x8000, u16::MAX] {
        let item = Item {
            index,
            effects: [
                Effect {
                    id: effect::USE_KIND,
                    value: 243,
                },
                Effect::default(),
                Effect::default(),
            ],
            ..Item::default()
        };

        assert_eq!(effects.total_scaled(&item, effect::USE_KIND), 0);
    }
}

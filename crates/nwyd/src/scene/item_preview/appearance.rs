use super::actor::Appearance;
use formats::data::item_effect::{Effects, effect};
use formats::data::item_list::{ItemList, equip};
use formats::data::mount::MountVisual;
use formats::mesh::appearance::{self, Look};
use formats::net::account::{Equipment, Item};

pub(super) fn handles(item: Item, items: &ItemList) -> bool {
    items.get(usize::from(item.index)).is_some_and(|entry| {
        item.index != 0
            && (appearance::is_costume(i32::from(item.index), items)
                || appearance::MOUNT_ITEM_TYPES.contains(&entry.item_type)
                || entry.item_type == appearance::MOUNT_SKIN_ITEM_TYPE
                || appearance::MOUNT_ID_WINDOW.contains(&i32::from(item.index))
                || entry.equip_mask & equip::CAPE != 0
                || armor_slot(entry.equip_mask).is_some())
    })
}

pub(super) fn select(
    item: Item,
    context: Option<&Equipment>,
    items: &ItemList,
    mounts: &[MountVisual],
) -> Option<Appearance> {
    let entry = items.get(usize::from(item.index))?;

    if appearance::MOUNT_ITEM_TYPES.contains(&entry.item_type)
        || entry.item_type == appearance::MOUNT_SKIN_ITEM_TYPE
        || appearance::MOUNT_ID_WINDOW.contains(&i32::from(item.index))
    {
        let mount = appearance::mount_for_item(&item, &Effects::new(items, &[]), mounts, 0)?;
        return Appearance::mount(&mount);
    }

    let costume = appearance::is_costume(i32::from(item.index), items);
    let cape = entry.equip_mask & equip::CAPE != 0;
    let slot = if costume {
        12
    } else if cape {
        15
    } else {
        armor_slot(entry.equip_mask)?
    };
    let class = if costume || cape {
        0
    } else {
        entry.effect_sum(i16::from(effect::CLASS))
    };
    let mut equipment = Equipment::default();
    equipment.items[0] = body(class, context, items)?;
    equipment.items[slot] = item;

    if costume {
        equipment.items[15] = context.map_or(Item::default(), Equipment::cape);
    }

    let mut look = Look::for_field(
        &equipment,
        &Effects::new(items, &[]),
        mounts,
        appearance::Wearer::default(),
        0,
    )?;

    if cape {
        let worn = look.cape.as_mut()?;
        worn.rank = worn.refine;
        return Appearance::cape(&look);
    }

    let mut target = Appearance::body(&look)?;
    target.visible_parts = if costume { 0x3F } else { 1 << slot };
    Some(target)
}

fn armor_slot(mask: u32) -> Option<usize> {
    [
        equip::HELMET,
        equip::ARMOR,
        equip::PANTS,
        equip::GLOVES,
        equip::BOOTS,
    ]
    .into_iter()
    .position(|slot| mask & slot != 0)
    .map(|slot| slot + 1)
}

fn body(class: i32, context: Option<&Equipment>, items: &ItemList) -> Option<Item> {
    let fits = |entry: &formats::data::item_list::Entry| {
        let body_class = entry.effect_sum(i16::from(effect::CLASS));
        entry.equip_mask == equip::FACE
            && matches!(body_class, 1 | 2 | 4 | 8)
            && (class == 0 || class & body_class != 0)
    };

    if let Some(item) = context.map(Equipment::body)
        && items
            .get(usize::from(item.index))
            .as_ref()
            .is_some_and(fits)
    {
        return Some(item);
    }

    items
        .iter()
        .find(|(_, entry)| fits(entry))
        .map(|(index, _)| Item {
            index: index as u16,
            ..Item::default()
        })
}

#[cfg(test)]
mod tests;

use super::*;
use formats::data::item_list::{self, field, item_type};
use formats::mesh::appearance::PartRef;
use formats::net::account::Effect;

fn tables() -> (ItemList, Vec<MountVisual>) {
    let mut raw = vec![0; item_list::BODY_LEN];

    for (id, mesh, mask, class, kind) in [
        (1, 0, equip::FACE, 1, 0),
        (11, 0, equip::FACE, 2, 0),
        (21, 0, equip::FACE, 4, 0),
        (31, 0, equip::FACE, 8, 0),
        (100, 1, equip::MOUNT, 255, item_type::PET_STAGE2),
        (101, 300, 0, 0, appearance::MOUNT_SKIN_ITEM_TYPE),
        (102, 1, equip::MOUNT, 255, item_type::MOUNT),
        (200, 47, equip::ARMOR, 4, 0),
        (201, 48, equip::ARMOR, 4, 0),
        (202, 42, equip::HELMET, 8, 0),
        (203, 80, equip::WEAPON, 255, 0),
        (500, 0, equip::CAPE, 255, 0),
        (501, 0, equip::CAPE, 255, 0),
        (4366, 2885, equip::MEDAL, 255, item_type::COSTUME_SET),
        (4461, 2885, equip::MEDAL, 255, item_type::COSTUME_SET),
        (4462, 2885, equip::MEDAL, 255, item_type::COSTUME_SET),
    ] {
        let record =
            &mut raw[id * formats::data::ITEM_RECORD_LEN..][..formats::data::ITEM_RECORD_LEN];
        record[field::MESH..field::MESH + 2].copy_from_slice(&(mesh as i16).to_le_bytes());
        record[field::EQUIP_MASK..field::EQUIP_MASK + 4].copy_from_slice(&mask.to_le_bytes());
        record[field::EFFECT..field::EFFECT + 2]
            .copy_from_slice(&i16::from(effect::CLASS).to_le_bytes());
        record[field::EFFECT + 2..field::EFFECT + 4].copy_from_slice(&(class as i16).to_le_bytes());
        record[field::ITEM_TYPE..field::ITEM_TYPE + 2].copy_from_slice(&kind.to_le_bytes());
    }

    let cape_skin = 501 * formats::data::ITEM_RECORD_LEN + field::MESH_SUB;
    raw[cape_skin..cape_skin + 2].copy_from_slice(&7i16.to_le_bytes());

    let empty = MountVisual {
        kind: 0,
        grid: [[0; 3]; 3],
        scale: 1.0,
        visual_kind: 0,
        v14: 0,
    };
    let mut mounts = vec![empty; 301];
    mounts[1] = MountVisual {
        kind: 20,
        grid: [[3, 4, 0], [1, 2, 0], [0, 0, -1]],
        ..empty
    };
    mounts[300] = MountVisual {
        kind: 49,
        grid: [[26, 0, 0], [2, 0, 0], [12, 0, 0]],
        ..empty
    };

    (
        ItemList::from_plain(raw, formats::text::Codepage::Windows1252),
        mounts,
    )
}

fn item(index: u16) -> Item {
    Item {
        index,
        ..Item::default()
    }
}

#[test]
fn body_cosmetics_use_full_costume_models_and_appearance_overrides() {
    let (items, mounts) = tables();
    let target = select(item(4462), None, &items, &mounts).unwrap();
    assert_eq!(target.spec.model_id, 1);
    assert_eq!(target.spec.costume_body, 274);
    assert_eq!(target.visible_parts, 0x3F);
    assert_eq!(&target.refine[..6], &[9; 6]);
    assert!(target.costume);
    assert_eq!(
        target,
        select(
            Item {
                appearance: 4462,
                ..item(4461)
            },
            None,
            &items,
            &mounts
        )
        .unwrap()
    );
}

#[test]
fn adaptive_cosmetics_use_the_hover_context_body_and_cape() {
    let (items, mounts) = tables();
    let mut context = Equipment::default();
    context.items[0] = item(21);
    context.items[15] = item(500);
    let male = select(item(4366), Some(&context), &items, &mounts).unwrap();
    assert_eq!(male.spec.costume_body, 168);
    assert_eq!(male.spec.model_id, 0);
    context.items[0] = item(11);
    let female = select(item(4366), Some(&context), &items, &mounts).unwrap();
    assert_eq!(female.spec.costume_body, 171);
    assert_eq!(female.spec.model_id, 1);
}

#[test]
fn armor_uses_its_class_rig_and_only_the_worn_part() {
    let (items, mounts) = tables();
    let mut context = Equipment::default();
    context.items[0] = item(11);
    context.items[12] = item(4462);
    let mut armor = item(200);
    armor.effects[0] = Effect { id: 43, value: 230 };
    let target = select(armor, Some(&context), &items, &mounts).unwrap();
    assert_eq!(target.spec.model_id, 0);
    assert_eq!(target.spec.variant, 1);
    assert_eq!(target.spec.costume_body, 0);
    assert_eq!(target.visible_parts, 1 << 2);
    assert_eq!(target.spec.parts[2], PartRef { mesh: 47, tex: 0 });
    assert_eq!(target.refine[2], 10);
    armor.appearance = 201;
    let overridden = select(armor, Some(&context), &items, &mounts).unwrap();
    assert_eq!(overridden.spec.parts[2].mesh, 48);
    assert_eq!(overridden.refine[2], 10);
    let helmet = select(item(202), Some(&context), &items, &mounts).unwrap();
    assert_eq!(helmet.spec.model_id, 1);
    assert_eq!(helmet.spec.variant, 1);
    assert_eq!(helmet.visible_parts, 1 << 1);
}

#[test]
fn mount_overrides_and_standalone_skins_share_authored_models_and_colors() {
    let (items, mounts) = tables();
    let mut mount = item(100);
    mount.effects[0].id = 1;
    mount.effects[1].id = 90;
    let base = select(mount, None, &items, &mounts).unwrap();
    assert_eq!(base.spec.model_id, 20);
    assert_eq!(base.spec.parts[0], PartRef { mesh: 3, tex: 1 });
    assert_eq!(&base.refine[..3], &[9, 9, 0]);
    mount.appearance = 101;
    let worn = select(mount, None, &items, &mounts).unwrap();
    let skin = select(item(101), None, &items, &mounts).unwrap();
    assert_eq!(worn, skin);
    assert_eq!(skin.spec.model_id, 49);
    assert_eq!(skin.spec.parts[0], PartRef { mesh: 26, tex: 2 });
    assert_eq!(&skin.refine[..3], &[12, 0, 0]);
    let effects = Effects::new(&items, &[]);
    let as_worn =
        appearance::mount_for_item(&mount, &effects, &mounts, appearance::FLAG_MOUNT_AS_WORN)
            .unwrap();
    assert_eq!(as_worn.model, Some(20));
    assert_eq!(as_worn.part_color, [9, 9, 0]);
}

#[test]
fn capes_use_the_equipped_model_skin_and_item_refinement() {
    let (items, mounts) = tables();
    let mut cape = item(500);
    cape.appearance = 501;
    cape.effects[0] = Effect { id: 43, value: 230 };
    let target = select(cape, None, &items, &mounts).unwrap();
    assert!(handles(cape, &items));
    assert_eq!(target.spec.model_id, appearance::CAPE_MODEL);
    assert_eq!(target.spec.parts[0], PartRef { mesh: 0, tex: 7 });
    assert_eq!(target.refine[0], 10);
    assert_eq!(target.tier[0], 12);
}

#[test]
fn item_preview_does_not_change_field_mount_eligibility() {
    let (items, mounts) = tables();
    let effects = Effects::new(&items, &[]);
    let mut equipment = Equipment::default();
    equipment.items[appearance::MOUNT_SLOT] = item(100);
    assert!(appearance::mount_for_field(&equipment, &effects, &mounts, 0).is_none());
    assert!(appearance::mount_for_item(&equipment.mount(), &effects, &mounts, 0).is_some());
    equipment.items[appearance::MOUNT_SLOT].effects[0].id = 1;
    assert_eq!(
        appearance::mount_for_field(&equipment, &effects, &mounts, 0),
        appearance::mount_for_item(&equipment.mount(), &effects, &mounts, 0)
    );
    equipment.items[appearance::MOUNT_SLOT] = item(101);
    assert!(appearance::mount_for_field(&equipment, &effects, &mounts, 0).is_none());
    assert!(appearance::mount_for_item(&equipment.mount(), &effects, &mounts, 0).is_some());
    equipment.items[appearance::MOUNT_SLOT] = item(102);
    assert!(appearance::mount_for_field(&equipment, &effects, &mounts, 0).is_some());
}

#[test]
fn missing_actor_assets_and_unrelated_items_do_not_use_static_fallbacks() {
    let (items, _) = tables();
    assert!(handles(item(100), &items));
    assert!(handles(item(101), &items));
    assert!(handles(item(200), &items));
    assert!(handles(item(4462), &items));
    assert!(select(item(100), None, &items, &[]).is_none());
    assert!(select(item(101), None, &items, &[]).is_none());
    assert!(!handles(item(203), &items));
    assert!(!handles(Item::default(), &items));
    assert!(!handles(item(u16::MAX), &items));
}

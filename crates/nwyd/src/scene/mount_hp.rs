use super::field::FieldScene;
use super::spawn::Statics;
use formats::data::item_effect::effect;
use formats::data::item_list::item_type;
use formats::data::mount;
use formats::net::account::Equipment;

pub fn current(statics: &Statics, equipment: &Equipment) -> i32 {
    statics
        .effects()
        .total_scaled(&equipment.mount(), effect::PET_PROGRESS)
}

pub fn max(statics: &Statics, equipment: &Equipment, bonus: i16) -> i32 {
    if !equipment.mount_in_item_list() {
        return 0;
    }

    let id = usize::try_from(equipment.mount_id()).unwrap_or(0);

    let index = statics
        .items
        .get(id)
        .map_or(0, |entry| i32::from(entry.mount_index));

    let base = match usize::try_from(index).ok().filter(|&i| i < mount::RECORDS) {
        Some(i) => statics.mounts.get(i).map_or(0, |row| row.base_max_hp()),
        None => mount::BASE_MAX_HP_FALLBACK,
    };

    let scaled = mount::maturity_scale(base, i32::from(equipment.mount_maturity()));

    let bonus = i32::from(bonus);

    scaled.saturating_add(bonus.saturating_mul(scaled) / 100)
}

pub fn pair(scene: &FieldScene) -> (i32, i32) {
    let (statics, equipment) = (&scene.statics, &scene.entered.equipment);

    let bonus = super::cc::mount_hp_bonus(scene) as i16;

    (current(statics, equipment), max(statics, equipment, bonus))
}

pub fn worn_item_type(statics: &Statics, equipment: &Equipment) -> i16 {
    if !equipment.mount_in_item_list() {
        return item_type::ORDINARY;
    }

    usize::try_from(equipment.mount_id())
        .ok()
        .and_then(|id| statics.items.get(id))
        .map_or(item_type::ORDINARY, |entry| entry.item_type)
}

pub fn hud_shown(equipment: &Equipment) -> bool {
    equipment.mount_id() != 0
}

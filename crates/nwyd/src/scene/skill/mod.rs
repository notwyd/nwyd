pub mod arrow;
pub mod bash;
pub mod billboard;
pub mod bow;
pub mod cure;
pub mod double_swing;
pub mod effect_mesh;
pub mod fire;
pub mod fire_ring;
pub mod flash;
pub mod freeze_blade;
pub mod haste;
pub mod heal;
pub mod heavens_dust;
pub mod holy_touch;
pub mod ice_spear;
pub mod judgement;
pub mod level_up;
pub mod magic_arrow;
pub mod meteor_storm;
pub mod particle;
pub mod poison;
pub mod shade_decal;
pub mod skin_mesh;
pub mod slow_slash;
pub mod sp_change;
pub mod speedup;
pub mod thunderbolt;
pub mod town_portal;
pub mod volley;

pub fn static_mesh_ids() -> Vec<u32> {
    let mut ids = Vec::new();

    ids.extend([706, 707, 2818, 712, 713, 714, 715, 716, 717, 718]);

    ids.extend([8, 10, 501, 502, 505, 531, 701, 703, 2838, 2839]);

    ids.extend(super::arrow::model_id(0x99, 99).and_then(|id| u32::try_from(id).ok()));

    for spawn in &formats::vfx::dispatch::SPAWNS {
        if let Some(order) = arrow::read(spawn) {
            let arrow = arrow::build(&order, glam::Vec3::ZERO, glam::Vec3::X, 0);

            ids.extend(arrow.model.and_then(|id| u32::try_from(id).ok()));

            ids.extend(arrow.plane.and_then(|p| u32::try_from(p.mesh_id).ok()));
        } else if let Some(order) = effect_mesh::read(spawn) {
            ids.extend(u32::try_from(order.mesh_id).ok());
        } else if let Some(order) = double_swing::read(spawn) {
            ids.push(if order.mode == 2 { 12 } else { 702 });
        } else if ice_spear::read(spawn).is_some() {
            ids.extend([708, 707]);
        } else if meteor_storm::read(spawn).is_some() {
            ids.extend([708, 1608, 2816, 2817]);
        }
    }

    ids.sort_unstable();

    ids.dedup();

    ids
}

pub fn mesh_effect_textures() -> Vec<usize> {
    let mut slots = vec![6, 58, 99, 163, 176, 202, 256, 270, 229];

    slots.extend(11..15);

    slots.extend(101..105);

    for spawn in &formats::vfx::dispatch::SPAWNS {
        if let Some(order) = effect_mesh::read(spawn) {
            slots.extend(
                order
                    .texture_bias
                    .and_then(|slot| usize::try_from(slot).ok()),
            );
        }
    }

    slots.sort_unstable();

    slots.dedup();

    slots
}

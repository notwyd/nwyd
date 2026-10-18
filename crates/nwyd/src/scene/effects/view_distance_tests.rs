use super::*;
use crate::scene::view_distance::ViewDistance;

#[test]
fn loaded_billboards_render_to_the_selected_field_distance_without_mutating_effects() {
    let mut scene = BillboardScene::empty();

    scene.scene_id = super::super::world::SCENE_FIELD;

    scene.by_slot.insert(0, 0);

    scene.placements.push((0, 0));

    for distance in [
        45.0, 46.0, 67.5, 68.0, 90.0, 91.0, 135.0, 136.0, 180.0, 181.0, 225.0, 226.0, 270.0, 271.0,
        315.0, 316.0, 360.0, 361.0,
    ] {
        let mut billboard = Billboard::new(0, 0, Vec3::ONE, 0.0, 1, 80);

        billboard.position = Vec3::new(0.0, 0.0, distance);

        billboard.visible = true;

        scene.billboards.push(billboard);
    }

    let original = scene.billboards.clone();

    for (choice, visible) in [
        (ViewDistance::Retail, 1),
        (ViewDistance::Extended, 3),
        (ViewDistance::Distant, 5),
        (ViewDistance::Far, 7),
        (ViewDistance::Maximum, 9),
        (ViewDistance::FiveTimes, 11),
        (ViewDistance::SixTimes, 13),
        (ViewDistance::SevenTimes, 15),
        (ViewDistance::EightTimes, 17),
        (ViewDistance::Retail, 1),
    ] {
        scene.view_distance = choice;

        let frame = scene.frame(super::super::actors::Reach::at(Vec3::ZERO), 0);

        assert_eq!(frame.billboards, visible);
        assert_eq!(frame.culled, original.len() - visible);
        assert_eq!(scene.billboards, original);
    }

    scene.view_distance = ViewDistance::EightTimes;

    for scene_id in [
        super::super::world::SCENE_LOGIN,
        super::super::world::SCENE_SELECT_CHAR,
        super::super::world::SCENE_BUDGET,
    ] {
        scene.scene_id = scene_id;

        assert_eq!(
            scene
                .frame(super::super::actors::Reach::at(Vec3::ZERO), 0)
                .billboards,
            1
        );
        assert_eq!(scene.billboards, original);
    }
}

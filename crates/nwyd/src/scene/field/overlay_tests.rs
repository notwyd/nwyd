use super::*;
use crate::{clock::Calendar, scene::damage};
use render::sprite::SpriteBatcher;

#[test]
fn speech_balloon_keeps_retail_position_above_nameplate_at_different_ui_scales() {
    for (viewport, expected) in [
        ((800, 600), (340.0, 240.0, 120.0, 36.0)),
        ((1600, 1200), (330.0, 240.0, 140.0, 56.0)),
    ] {
        let layout = Layout::new(viewport.0, viewport.1);
        let anchor = (400.0, 300.0);
        let rect = balloon::place(anchor, balloon::size((100, 16), &layout));
        assert_eq!((rect.x, rect.y, rect.w, rect.h), expected);
        let standing = nameplate::place(anchor, &layout, 9, 0, false);
        let mounted = nameplate::place(anchor, &layout, 9, 0, true);
        assert_eq!(standing.name.1 - rect.y, 60.0);
        assert_eq!(mounted.name.1 - rect.y, 40.0);
    }
}

#[test]
#[ignore = "requires game files via GAME_DIR"]
fn speech_balloon_draws_over_damage_and_under_its_nameplate() {
    let game = GameDir::open(std::env::var_os("GAME_DIR").unwrap()).unwrap();
    let mut packet =
        formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();
    packet.client_id = 37;
    packet.x = 8 * 128 + 64;
    packet.z = 13 * 128 + 64;
    let mut entered = Entered::from_packet(&packet, 0, None);
    entered.equipment.items[0].index = 1;
    let mut scene = build(
        &game,
        (800, 600),
        &SceneTime::pinned(2000, Calendar::GOLDEN),
        &entered,
        &Chrome::bare(),
    )
    .unwrap();
    scene.captions = true;
    scene.camera.dist = 0.0;
    let view = Matrix4::IDENTITY;
    let mut projection = Matrix4::scaling(0.0, 0.0, 0.0);
    projection.0[14] = 0.5;
    scene.projected = Some((view, projection));
    let player = scene.player.unwrap();
    let actor = &mut scene.world.actors.actors[player];
    actor.name = "Synthetic".into();
    actor.camera_visible = true;
    actor.balloon = Some(balloon::Balloon {
        text: "Synthetic speech".into(),
        start_ms: 2000,
        life_ms: 3000,
    });
    let at = nameplate::project_actor(&view, &projection, actor, &scene.layout, 0.0).unwrap();
    let balloon_draws = balloon::draws(
        &mut scene.text,
        actor.entity,
        actor.balloon.as_ref().unwrap(),
        at,
        &scene.layout,
    );
    assert_eq!(balloon_draws.len(), 3);
    assert_eq!(balloon_draws[0].quad.color, 0x7700_0000);

    for kind in [0, 4] {
        let items = damage::set_items(&scene.sets, 0x8C).unwrap();
        let mut number = damage::Number::new(
            "42",
            (at.0, at.1 - 60.0),
            kind,
            0,
            1500,
            2000,
            items,
            &scene.layout,
        );
        number.visible = true;
        number.pop = 0.7;
        scene.damage.push(number);
    }
    let damage_draws = scene
        .damage
        .frame_draws(&scene.sets, &scene.layout, &mut scene.text);
    assert_eq!(damage_draws.len(), 4);
    assert!(
        damage_draws
            .iter()
            .any(|draw| matches!(draw.texture, TextureKey::Scene(_)))
    );
    assert!(
        damage_draws
            .iter()
            .any(|draw| matches!(draw.texture, TextureKey::SceneText(_)))
    );

    let mut draws = Vec::new();
    append_frame_draws(&mut scene, &mut draws);
    let positions = |expected: &[PlannedQuad]| -> Vec<usize> {
        expected
            .iter()
            .map(|draw| draws.iter().position(|actual| actual == draw).unwrap())
            .collect()
    };
    let damage_positions = positions(&damage_draws);
    let balloon_positions = positions(&balloon_draws);
    assert!(
        damage_positions.iter().max().unwrap() < balloon_positions.iter().min().unwrap(),
        "damage {damage_positions:?} must be behind the complete balloon {balloon_positions:?}"
    );
    assert!(balloon_positions.windows(2).all(|pair| pair[0] < pair[1]));
    let name_positions: Vec<_> = draws
        .iter()
        .enumerate()
        .filter_map(|(index, draw)| {
            (draw.texture == TextureKey::SceneText(0x8000_0025)).then_some(index)
        })
        .collect();
    assert_eq!(name_positions.len(), 2);
    assert!(
        balloon_positions.iter().max().unwrap() < name_positions.iter().min().unwrap(),
        "balloon {balloon_positions:?} must be behind its nameplate {name_positions:?}"
    );

    let mut batcher = SpriteBatcher::new();
    for draw in &draws {
        batcher.push_with(draw.mode, draw.texture, &draw.quad, 512, 512, draw.uv);
    }
    let damage_batch = batcher
        .batches()
        .iter()
        .rposition(|batch| {
            damage_draws
                .iter()
                .any(|draw| draw.texture == batch.texture)
        })
        .unwrap();
    let text_batch = batcher
        .batches()
        .iter()
        .position(|batch| batch.texture == TextureKey::SceneText(0x4000_0025))
        .unwrap();
    assert!(damage_batch < text_batch);
    let last_balloon_batch = batcher
        .batches()
        .iter()
        .rposition(|batch| batch.texture == TextureKey::SceneText(0x4000_0025))
        .unwrap();
    let name_batch = batcher
        .batches()
        .iter()
        .position(|batch| batch.texture == TextureKey::SceneText(0x8000_0025))
        .unwrap();
    assert!(last_balloon_batch < name_batch);
}

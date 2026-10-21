use crate::{
    clock::Calendar,
    frame::TextureKey,
    scene::{Scene, enter::Entered, field, login_static::SceneTime},
};
use formats::{net::arena::List, ui::pane::ElementType};
use ui::input::Mouse;

fn field(scene: &mut Scene) -> &mut field::FieldScene {
    let Scene::Field(field) = scene else {
        panic!("expected field scene");
    };
    field
}

fn scene(game: &assets::GameDir, viewport: (u32, u32)) -> Scene {
    let mut packet =
        formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();
    packet.client_id = 37;
    packet.x = 2118;
    packet.z = 2102;
    packet.mob.equipment.items[0].index = 1;
    packet.mob.status.hp = 100;
    packet.mob.status.max_hp = 100;
    Scene::Field(Box::new(
        field::build(
            game,
            viewport,
            &SceneTime::pinned(20000, Calendar::GOLDEN),
            &Entered::from_packet(&packet, 0, None),
            &field::Chrome {
                hud: true,
                captions: true,
            },
        )
        .unwrap(),
    ))
}

fn arena_button(scene: &mut Scene) -> (i32, i32) {
    let f = field(scene);
    let placed = &f.panels.as_ref().unwrap().screen(0x20).unwrap().placed;
    let [left, top, right, bottom] = placed
        .ui
        .panel
        .element("RoyalArena", ElementType::Button)
        .unwrap()
        .rect();
    let scale = f.layout.ui_scale();
    (
        ((placed.origin.0 + (left + right) / 2) as f32 * scale) as i32,
        ((placed.origin.1 + (top + bottom) / 2) as f32 * scale) as i32,
    )
}

fn assert_schedule(scene: &mut Scene) {
    let panels = field(scene).panels.as_ref().unwrap();
    let shown = panels.tooltip.shown.as_ref().expect("schedule tooltip");
    let lines: Vec<_> = shown
        .panel
        .elements
        .iter()
        .map(|e| e.display.as_str())
        .collect();
    for title in [
        "Arenas",
        "Arenas Champion Hall (Servidor 1)(GMT-3)",
        "Arena Aspirants Field (Servidor 2)(GMT-3)",
    ] {
        assert!(lines.contains(&title), "missing schedule title {title:?}");
    }
    assert_eq!(
        lines
            .iter()
            .filter(|&&line| line == "01:00 PM/07:00 PM/08:30 PM/11:00 PM")
            .count(),
        2,
        "both arenas retain their complete schedule"
    );
    assert!(
        scene
            .frame_draws()
            .iter()
            .filter(|draw| { matches!(draw.texture, TextureKey::Caption(0xF0, _, _)) })
            .count()
            >= 5
    );
}

fn assert_hidden(scene: &mut Scene) {
    assert!(
        !scene
            .frame_draws()
            .iter()
            .any(|draw| { matches!(draw.texture, TextureKey::Caption(0xF0, _, _)) })
    );
    assert!(!field(scene).panels.as_ref().unwrap().tooltip.visible());
}

#[test]
#[ignore = "requires game files via GAME_DIR"]
fn arena_schedule_remains_visible_when_registration_is_disabled() {
    let game = assets::GameDir::open(std::env::var_os("GAME_DIR").unwrap()).unwrap();
    for viewport in [(800, 600), (1024, 768)] {
        let mut scene = scene(&game, viewport);
        for (state, cost, slot, enabled) in [
            (0, 1500, 0, false),
            (2, 1500, 3, false),
            (1, -1, 3, false),
            (1, 1500, 3, true),
        ] {
            let mut list = List {
                state: [0; 10],
                current: [0; 10],
                max: [0; 10],
                cost: [0; 10],
            };
            list.state[slot] = state;
            list.current[slot] = 7;
            list.max[slot] = 30;
            list.cost[slot] = cost;
            if state != 0 {
                list.state[7] = 1;
            }
            assert_eq!(scene.arena_list(&list).unwrap().slot, slot);
            let (x, y) = arena_button(&mut scene);
            scene.mouse(Mouse::Move, 0, 0);
            scene.mouse(Mouse::Move, x, y);
            assert_schedule(&mut scene);
            assert_eq!(
                field(&mut scene)
                    .panels
                    .as_ref()
                    .unwrap()
                    .screen(0x20)
                    .unwrap()
                    .element("RoyalArena", ElementType::Button)
                    .unwrap()
                    .enabled,
                enabled
            );
            scene.mouse(Mouse::LeftDown, x, y);
            scene.mouse(Mouse::LeftUp, x, y);
            assert_eq!(
                field(&mut scene)
                    .panels
                    .as_ref()
                    .unwrap()
                    .boxes
                    .topmost()
                    .is_some(),
                enabled
            );
            assert!(field::take_outbox(field(&mut scene)).is_empty());
            if enabled {
                scene.key(ui::text_entry::Key::Escape, Default::default());
            }
            scene.mouse(Mouse::Move, 0, 0);
            assert_hidden(&mut scene);
            scene.mouse(Mouse::Move, x, y);
            assert_schedule(&mut scene);
            field(&mut scene)
                .panels
                .as_mut()
                .unwrap()
                .set_open(0x20, false);
            assert_hidden(&mut scene);
        }
    }
}

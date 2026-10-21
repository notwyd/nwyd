use super::*;
use ui::{draw::Owner, input::Target, widget::EditState};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

fn login_client(field: usize) -> Client {
    let mut login = crate::scene::login_panels::language_tests::login();
    for (_, panel) in login.msgbox.visible_mut() {
        panel.ui.panel.visible = false;
    }
    login.manager.focused = Some(Target {
        panel: 1,
        owner: Owner::Element(field),
    });
    let edit = login.focused_edit().unwrap();
    edit.text = "tester".into();
    edit.caret = 6;
    edit.anchor = 6;

    client(Scene::LoginPanels(Box::new(login)))
}

fn client(scene: Scene) -> Client {
    Client::new(
        Settings {
            title: "Keyboard test".into(),
            backbuffer: (800, 600),
            fullscreen: false,
            present_mode: wgpu::PresentMode::Immediate,
            renderer: None,
            overlay: false,
            pointers: crate::scene::cursor::Pointers::none(),
        },
        scene,
    )
}

fn edit(client: &mut Client) -> &mut EditState {
    let Scene::LoginPanels(login) = &mut client.runtime.scene else {
        panic!("expected login scene");
    };
    login.focused_edit().unwrap()
}

fn backspace(client: &mut Client, text: Option<&str>, state: ElementState, repeat: bool) {
    client.route_key(
        &PhysicalKey::Code(KeyCode::Backspace),
        &Key::Named(NamedKey::Backspace),
        text,
        state,
        repeat,
    );
}

#[test]
fn backspace_deletes_once_in_login_and_password_with_or_without_event_text() {
    for field in [0, 1] {
        for text in [None, Some("\u{8}")] {
            let mut client = login_client(field);
            backspace(&mut client, text, ElementState::Pressed, false);
            assert_eq!(edit(&mut client).text, "teste");
            assert_eq!((edit(&mut client).caret, edit(&mut client).anchor), (5, 5));

            backspace(&mut client, text, ElementState::Pressed, true);
            assert_eq!(edit(&mut client).text, "test");

            backspace(&mut client, text, ElementState::Released, false);
            assert_eq!(edit(&mut client).text, "test");
        }
    }
}

#[test]
fn browser_backspace_removes_selection_and_respects_the_start_of_the_field() {
    let mut client = login_client(0);
    edit(&mut client).anchor = 2;
    edit(&mut client).caret = 5;
    backspace(&mut client, None, ElementState::Pressed, false);
    assert_eq!(edit(&mut client).text, "ter");
    assert_eq!((edit(&mut client).caret, edit(&mut client).anchor), (2, 2));

    edit(&mut client).anchor = 0;
    edit(&mut client).caret = 0;
    backspace(&mut client, None, ElementState::Pressed, false);
    assert_eq!(edit(&mut client).text, "ter");
}

#[test]
fn browser_backspace_does_not_edit_while_ime_is_composing() {
    let mut client = login_client(0);
    client.runtime.ime.composing = true;
    backspace(&mut client, None, ElementState::Pressed, false);
    assert_eq!(edit(&mut client).text, "tester");
}

fn field_client() -> Client {
    use crate::{clock::Calendar, scene::enter::Entered};
    let game = assets::GameDir::open(std::env::var_os("GAME_DIR").unwrap()).unwrap();
    let mut packet =
        formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();
    packet.client_id = 37;
    packet.x = 2118;
    packet.z = 2102;
    packet.mob.equipment.items[0].index = 1;
    packet.mob.status.hp = 100;
    packet.mob.status.max_hp = 100;
    let field = field::build(
        &game,
        (800, 600),
        &crate::scene::login_static::SceneTime::pinned(20000, Calendar::GOLDEN),
        &Entered::from_packet(&packet, 0, None),
        &field::Chrome::default(),
    )
    .unwrap();
    client(Scene::Field(Box::new(field)))
}

fn field(client: &mut Client) -> &mut field::FieldScene {
    let Scene::Field(field) = &mut client.runtime.scene else {
        panic!("expected field scene");
    };
    field
}

fn enter(
    client: &mut Client,
    physical: KeyCode,
    text: Option<&str>,
    state: ElementState,
    repeat: bool,
) {
    client.route_key(
        &PhysicalKey::Code(physical),
        &Key::Named(NamedKey::Enter),
        text,
        state,
        repeat,
    );
}

fn assert_chat_focus(client: &mut Client, expected: bool) {
    let field = field(client);
    let panels = field.panels.as_ref().unwrap();
    assert_eq!(panels.chat.focused, expected);
    assert_eq!(
        crate::scene::chat::holds_caret(panels, &field.hud.as_ref().unwrap().manager),
        expected
    );
}

#[test]
#[ignore = "requires game files via GAME_DIR"]
fn held_enter_toggles_chat_and_submits_once_without_bypassing_ime() {
    use crate::scene::chat;
    let mut client = field_client();
    for physical in [KeyCode::Enter, KeyCode::NumpadEnter] {
        for text in [None, Some("\r")] {
            field(&mut client).time.now_ms += 10000;
            enter(&mut client, physical, text, ElementState::Pressed, false);
            assert_chat_focus(&mut client, true);
            enter(&mut client, physical, text, ElementState::Pressed, true);
            assert_chat_focus(&mut client, false);
            enter(&mut client, physical, text, ElementState::Pressed, true);
            assert_chat_focus(&mut client, true);

            chat::set_text(field(&mut client).panels.as_mut().unwrap(), "Held Enter");
            client.runtime.ime.composing = true;
            enter(&mut client, physical, text, ElementState::Pressed, true);
            assert_chat_focus(&mut client, true);
            assert_eq!(
                chat::edit_text(field(&mut client).panels.as_ref().unwrap()),
                "Held Enter"
            );
            assert!(field::take_outbox(field(&mut client)).is_empty());
            client.runtime.ime.composing = false;
            enter(&mut client, physical, text, ElementState::Pressed, true);
            assert_chat_focus(&mut client, false);
            let mut expected = vec![0; 140];
            expected[4..8].copy_from_slice(&[0x33, 3, 37, 0]);
            expected[12..22].copy_from_slice(b"Held Enter");
            assert_eq!(field::take_outbox(field(&mut client)), [expected]);

            enter(&mut client, physical, text, ElementState::Pressed, true);
            assert_chat_focus(&mut client, true);
            enter(&mut client, physical, text, ElementState::Released, false);
            assert_chat_focus(&mut client, true);
            enter(&mut client, physical, text, ElementState::Pressed, false);
            assert_chat_focus(&mut client, false);
            enter(&mut client, physical, text, ElementState::Released, false);
            assert_chat_focus(&mut client, false);
            assert!(field::take_outbox(field(&mut client)).is_empty());
        }
    }
}

fn ask_gate(client: &mut Client) {
    let field = field(client);
    field.world.actors.actors[field.player.unwrap()].action = 0;
    crate::scene::gate::clear_latch(&mut field.gate, false);
    assert!(matches!(
        crate::scene::gate::tick(field),
        crate::scene::gate::Step::Asked { .. }
    ));
    assert!(
        field
            .panels
            .as_ref()
            .unwrap()
            .msgbox
            .placed
            .ui
            .panel
            .visible
    );
}

fn assert_gate_answer(client: &mut Client) {
    assert_chat_focus(client, false);
    let field = field(client);
    let panels = field.panels.as_ref().unwrap();
    assert!(panels.boxes.find(0x3C).is_none());
    assert!(!panels.msgbox.placed.ui.panel.visible);
    assert_eq!(
        field::take_outbox(field),
        [vec![0, 0, 0, 0, 0x90, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
        "one Enter sends exactly one confirmation reply"
    );
    assert_eq!(field.cc.last_action_ms, field.time.now_ms);
    assert_eq!(
        crate::scene::gate::tick(field),
        crate::scene::gate::Step::Nothing
    );
}

#[test]
#[ignore = "requires game files via GAME_DIR"]
fn held_enter_accepts_successive_confirmations_without_opening_chat() {
    let mut client = field_client();
    for physical in [KeyCode::Enter, KeyCode::NumpadEnter] {
        for text in [None, Some("\r")] {
            for repeat in [false, true, true] {
                ask_gate(&mut client);
                enter(&mut client, physical, text, ElementState::Pressed, repeat);
                assert_gate_answer(&mut client);
            }
            ask_gate(&mut client);
            enter(&mut client, physical, text, ElementState::Released, false);
            assert!(
                field(&mut client)
                    .panels
                    .as_ref()
                    .unwrap()
                    .boxes
                    .find(0x3C)
                    .is_some()
            );
            assert!(field::take_outbox(field(&mut client)).is_empty());
            enter(&mut client, physical, text, ElementState::Pressed, false);
            assert_gate_answer(&mut client);
        }
    }
}

#[test]
#[ignore = "requires game files via GAME_DIR"]
fn held_enter_waits_for_the_field_lock_before_accepting_a_confirmation() {
    let mut client = field_client();
    ask_gate(&mut client);
    field(&mut client).lock.stamp(0, 20000);
    for now_ms in [20000, 25999] {
        field(&mut client).time.now_ms = now_ms;
        enter(
            &mut client,
            KeyCode::Enter,
            Some("\r"),
            ElementState::Pressed,
            true,
        );
        let field = field(&mut client);
        assert!(field.panels.as_ref().unwrap().boxes.find(0x3C).is_some());
        assert!(field::take_outbox(field).is_empty());
    }
    field(&mut client).time.now_ms = 26000;
    enter(
        &mut client,
        KeyCode::Enter,
        Some("\r"),
        ElementState::Pressed,
        true,
    );
    assert_gate_answer(&mut client);
}

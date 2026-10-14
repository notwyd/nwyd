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

    Client::new(
        Settings {
            title: "Keyboard test".into(),
            backbuffer: (800, 600),
            fullscreen: false,
            present_mode: wgpu::PresentMode::Immediate,
            overlay: false,
            pointers: crate::scene::cursor::Pointers::none(),
        },
        Scene::LoginPanels(Box::new(login)),
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
        // Web winit omits text for Backspace; native backends may provide it.
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

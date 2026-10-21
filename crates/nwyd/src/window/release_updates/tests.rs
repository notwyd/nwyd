use super::*;
use crate::window::{ElementState, MouseButton, Settings};
use ui::{draw::Owner, input::Target};

const BUTTON: Option<(i32, i32)> = Some((698, 575));
const URL: &str =
    "https://github.com/notwyd/nwyd/releases/latest/download/nwyd-x86_64-unknown-linux-gnu.tar.gz";

fn client() -> Client {
    let mut login = crate::scene::login_panels::language_tests::login();

    for (_, panel) in login.msgbox.visible_mut() {
        panel.ui.panel.visible = false;
    }

    let mut client = Client::new(
        Settings {
            title: "Update notification test".into(),
            backbuffer: (800, 600),
            fullscreen: false,
            present_mode: wgpu::PresentMode::Immediate,
            renderer: None,
            overlay: false,
            pointers: crate::scene::cursor::Pointers::none(),
        },
        Scene::LoginPanels(Box::new(login)),
    );

    client.updates.panel = Some(Panel::for_test());
    client.updates.download_url = Some(URL.into());
    client.fit = Some(render::blit::fit((800, 600), (800, 600)));
    client
}

fn never_open(_: &str) -> std::io::Result<()> {
    panic!("unexpected download")
}

#[test]
fn complete_click_opens_download_once_and_cancels_only_scene_pointer_input() {
    let mut client = client();
    let Scene::LoginPanels(login) = &mut client.runtime.scene else {
        unreachable!()
    };
    let target = Some(Target {
        panel: 1,
        owner: Owner::Element(0),
    });
    login.manager.focused = target;
    login.manager.pressed = target;
    login.manager.capture = target;
    client.runtime.input.mouse.rmb = true;

    assert!(client.update_notice_button(0, true, BUTTON, never_open));
    assert!(!client.runtime.input.mouse.rmb);

    let Scene::LoginPanels(login) = &client.runtime.scene else {
        unreachable!()
    };
    assert_eq!(login.manager.focused, target);
    assert_eq!(login.manager.pressed, None);
    assert_eq!(login.manager.capture, None);

    let mut downloads = Vec::new();
    assert!(client.update_notice_button(0, false, BUTTON, |url| {
        downloads.push(url.to_owned());
        Ok(())
    }));
    assert_eq!(downloads, [URL]);
    assert!(client.update_notice_button(0, false, BUTTON, never_open));
}

#[test]
fn covered_release_cancels_an_earlier_scene_press_without_downloading() {
    let mut client = client();
    let Scene::LoginPanels(login) = &mut client.runtime.scene else {
        unreachable!()
    };
    let target = Some(Target {
        panel: 1,
        owner: Owner::Element(0),
    });
    login.manager.pressed = target;
    login.manager.capture = target;
    assert!(client.update_notice_button(0, false, BUTTON, never_open));
    let Scene::LoginPanels(login) = &client.runtime.scene else {
        unreachable!()
    };
    assert_eq!(login.manager.pressed, None);
    assert_eq!(login.manager.capture, None);
}

#[test]
fn all_update_states_remain_visible_and_only_available_updates_download() {
    let mut client = client();

    for status in [
        Status::Checking,
        Status::UpToDate,
        Status::Unavailable,
        Status::Development,
    ] {
        client.set_release_status(status.clone());
        assert!(client.update_notice_visible());
        assert_eq!(client.updates.status, Some(status));
        assert!(client.updates.download_url.is_none());
        assert!(client.update_notice_button(0, true, BUTTON, never_open));
        assert!(client.update_notice_button(0, false, BUTTON, never_open));
    }

    client.set_release_status(Status::Available(crate::updates::AvailableUpdate {
        commit: "a".repeat(40),
        build_number: 10,
        download_url: URL.into(),
    }));
    assert!(client.update_notice_visible());
    assert!(client.update_notice_button(0, true, BUTTON, never_open));
    let mut opened = false;
    assert!(client.update_notice_button(0, false, BUTTON, |url| {
        assert_eq!(url, URL);
        opened = true;
        Ok(())
    }));
    assert!(opened);

    assert!(client.update_notice_button(0, true, BUTTON, never_open));
    client.set_release_status(Status::UpToDate);
    assert!(client.update_notice_visible());
    assert!(client.update_notice_button(0, false, BUTTON, never_open));
}

#[test]
fn hovering_and_clicking_preserve_held_camera_modifiers() {
    let mut client = client();
    client.runtime.input.mods.alt = true;
    client.runtime.input.mouse.alt = true;
    client.runtime.input.mouse.rmb = true;
    client.runtime.input.mouse.mmb = true;
    assert!(client.update_notice_motion(BUTTON));
    assert!(!client.runtime.input.mouse.rmb);
    assert!(!client.runtime.input.mouse.mmb);
    assert!(client.runtime.input.mouse.alt);
    assert!(!client.update_notice_motion(Some((0, 0))));
    assert!(client.runtime.input.mouse.alt);
    client.set_release_status(Status::UpToDate);
    assert!(client.update_notice_button(0, true, BUTTON, never_open));
    assert!(client.update_notice_button(0, false, BUTTON, never_open));
    assert!(client.runtime.input.mouse.alt);
}

#[test]
fn dragging_off_the_button_consumes_release_without_downloading() {
    let mut client = client();
    assert!(client.update_notice_button(0, true, BUTTON, never_open));
    assert!(client.update_notice_motion(Some((0, 0))));
    assert!(client.update_notice_button(0, false, Some((0, 0)), never_open));
    assert!(!client.update_notice_blocks_pointer(Some((0, 0))));
    assert!(!client.update_notice_button(0, true, Some((0, 0)), never_open));
}

#[test]
fn scene_modal_hides_notification_and_cancels_its_pending_click() {
    let mut client = client();
    assert!(client.update_notice_visible());
    assert!(client.update_notice_button(0, true, BUTTON, never_open));
    let Scene::LoginPanels(login) = &mut client.runtime.scene else {
        unreachable!()
    };
    login.msgbox.placed.ui.panel.visible = true;
    assert!(!client.update_notice_visible());
    assert!(client.update_notice_button(0, false, BUTTON, never_open));
    assert!(!client.update_notice_button(0, true, BUTTON, never_open));
}

#[test]
fn right_and_middle_clicks_do_not_reach_camera_input() {
    let mut client = client();
    client.pointer = (698.0, 575.0);

    for button in [MouseButton::Right, MouseButton::Middle] {
        client.pointer_button(ElementState::Pressed, button);
        assert!(!client.runtime.input.mouse.rmb);
        assert!(!client.runtime.input.mouse.mmb);
        client.pointer = (0.0, 0.0);
        client.pointer_button(ElementState::Released, button);
        assert!(!client.updates.panel.as_ref().unwrap().blocks_pointer());
        client.pointer = (698.0, 575.0);
    }
}

#[test]
fn unavailable_check_and_download_open_failure_leave_the_client_usable() {
    let mut client = client();
    client.updates = State::default();
    client.poll_release_update();
    assert!(!client.update_notice_visible());
    assert!(!client.update_notice_button(0, true, BUTTON, never_open));
    client.updates.panel = Some(Panel::for_test());
    client.updates.download_url = Some(URL.into());
    assert!(client.update_notice_button(0, true, BUTTON, never_open));
    assert!(client.update_notice_button(0, false, BUTTON, |_| {
        Err(std::io::Error::other("browser unavailable"))
    }));
    assert!(client.update_notice_visible());
    assert!(!client.updates.panel.as_ref().unwrap().blocks_pointer());
}

#[test]
#[ignore = "requires game assets"]
fn character_and_world_confirmations_take_priority_over_downloads() {
    use crate::{
        clock::Calendar,
        scene::{enter::Entered, field, login_static::SceneTime, msgbox, select_char},
    };

    let game = assets::GameDir::open(std::env::var_os("GAME_DIR").unwrap()).unwrap();
    let time = SceneTime::pinned(20000, Calendar::GOLDEN);
    let selection = select_char::build(&game, (800, 600), &time, true).unwrap();
    let mut packet =
        formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();
    packet.client_id = 37;
    packet.x = 2118;
    packet.z = 2102;
    packet.mob.equipment.items[0].index = 1;
    packet.mob.status.hp = 100;
    packet.mob.status.max_hp = 100;
    let mut world = field::build(
        &game,
        (800, 600),
        &time,
        &Entered::from_packet(&packet, 0, None),
        &field::Chrome::default(),
    )
    .unwrap();
    world.panels = Some(crate::scene::panels::Panels::for_test());

    for (_, panel) in world.panels.as_mut().unwrap().msgbox.visible_mut() {
        panel.ui.panel.visible = false;
    }

    for scene in [
        Scene::SelectChar(Box::new(selection)),
        Scene::Field(Box::new(world)),
    ] {
        let mut client = client();
        client.runtime.scene = scene;
        assert!(client.update_notice_visible());
        assert!(client.update_notice_button(0, true, BUTTON, never_open));
        let boxes = match &mut client.runtime.scene {
            Scene::SelectChar(scene) => &mut scene.boxes,
            Scene::Field(scene) => &mut scene.panels.as_mut().unwrap().boxes,
            _ => unreachable!(),
        };
        let slot = boxes
            .raise(&msgbox::Request::restart_notice(), &ui::Strings::default())
            .unwrap();
        assert!(!client.update_notice_visible());
        assert!(client.update_notice_button(0, false, BUTTON, never_open));
        assert!(!client.update_notice_button(0, true, BUTTON, never_open));
        match &mut client.runtime.scene {
            Scene::SelectChar(scene) => scene.boxes.close(slot),
            Scene::Field(scene) => scene.panels.as_mut().unwrap().boxes.close(slot),
            _ => unreachable!(),
        }
        assert!(client.update_notice_visible());

        if let Scene::SelectChar(scene) = &mut client.runtime.scene {
            scene.name_dialog.visible = true;
            assert!(!client.update_notice_visible());
        }
    }
}

use super::*;
use crate::{
    game_settings::{Panel, Preferences},
    runtime::message_packets::{self, Packet},
    scene::login_panels::language_tests::{login, translated},
    window::Settings,
};
use formats::config::Config;
use protocol::server::SystemMessage;

fn client() -> Client {
    let mut scene = login();
    scene.strings = translated(
        &[
            (1462, "%s refined an item to +%s"),
            (1562, "%s refined %s to +%s"),
            (1584, "%s received %s at %s"),
            (1176, "Your refinement succeeded"),
            (12, "Login denied"),
        ],
        "",
    );
    let mut client = Client::new(
        Settings {
            title: "Announcement preferences test".into(),
            backbuffer: (800, 600),
            fullscreen: false,
            present_mode: wgpu::PresentMode::Immediate,
            renderer: None,
            overlay: false,
            pointers: crate::scene::cursor::Pointers::none(),
        },
        Scene::LoginPanels(Box::new(scene)),
    );
    client.runtime.game_settings = Some(Panel::for_test(Preferences::from_config(
        &Config::default(),
    )));
    client
}

fn login_scene(client: &mut Client) -> &mut crate::scene::login_panels::LoginPanels {
    let Scene::LoginPanels(scene) = &mut client.runtime.scene else {
        panic!("expected login scene")
    };
    scene
}

fn message(client: &mut Client, record: i16, argument: &str, has_argument: bool) {
    message_packets::apply(
        client,
        Packet::Message(Box::new(SystemMessage {
            kind: 0,
            code: record - 1000,
            argument: argument.into(),
            has_argument,
        })),
    );
}

#[test]
fn announcement_filter_uses_applied_preferences_and_changes_without_restarting() {
    let mut client = client();
    client
        .runtime
        .game_settings
        .as_mut()
        .unwrap()
        .draft
        .interface
        .hide_item_announcements = true;
    message(&mut client, 1584, "Synthetic Item NPC", true);
    assert!(login_scene(&mut client).notice.state.showing);
    assert_eq!(login_scene(&mut client).notice.state.sounds, [33]);

    login_scene(&mut client).notice = crate::scene::notice::Overlay::for_test();
    login_scene(&mut client).notice.placed.ui.panel.visible = false;
    client
        .runtime
        .game_settings
        .as_mut()
        .unwrap()
        .applied
        .interface
        .hide_item_announcements = true;
    for (record, argument) in [
        (1462, "Synthetic 15"),
        (1562, "3 15 Synthetic"),
        (1584, "Synthetic Item NPC"),
    ] {
        message(&mut client, record, argument, true);
    }
    let notice = &login_scene(&mut client).notice;
    assert_eq!(notice.state, Default::default());
    assert!(!notice.placed.ui.panel.visible);

    message(&mut client, 1176, "", false);
    assert!(login_scene(&mut client).notice.state.showing);
    assert_eq!(login_scene(&mut client).notice.state.sounds, [33]);

    login_scene(&mut client).notice = crate::scene::notice::Overlay::for_test();
    client
        .runtime
        .game_settings
        .as_mut()
        .unwrap()
        .applied
        .interface
        .hide_item_announcements = false;
    message(&mut client, 1584, "Synthetic Item NPC", true);
    assert!(login_scene(&mut client).notice.state.showing);
}

#[test]
fn hidden_announcements_preserve_login_reply_handling_and_launcher_errors() {
    let mut client = client();
    client
        .runtime
        .game_settings
        .as_mut()
        .unwrap()
        .applied
        .interface
        .hide_item_announcements = true;
    let scene = login_scene(&mut client);
    for element in &mut scene.login2.ui.panel.elements {
        element.enabled = false;
    }
    message(&mut client, 1584, "Synthetic Item NPC", true);
    let scene = login_scene(&mut client);
    assert!(!scene.notice.state.showing);
    for element in &scene.login2.ui.panel.elements {
        if matches!(element.name.as_str(), "btnLogin" | "password") {
            assert!(element.enabled);
        }
    }
    client.launcher_login = true;
    message(&mut client, 12, "", false);
    assert!(client.runtime.scene.legacy_modal());
}

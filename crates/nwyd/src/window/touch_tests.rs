use super::*;
use ui::{draw::Owner, input::Target};
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, Touch, TouchPhase},
};

const ACCOUNT: (f64, f64) = (140.0, 150.0);
const PASSWORD: (f64, f64) = (140.0, 200.0);
const BUTTON: (f64, f64) = (140.0, 250.0);

fn client() -> Client {
    let mut login = crate::scene::login_panels::language_tests::login();
    for (_, panel) in login.msgbox.visible_mut() {
        panel.ui.panel.visible = false;
    }
    login.login.ui.panel.visible = false;
    login.copyright.ui.panel.visible = false;
    login.login2.origin = (100, 120);
    for (index, element) in login.login2.ui.panel.elements.iter_mut().enumerate() {
        element.x = 20;
        element.y = 20 + index as i32 * 50;
        element.width = 180;
        element.height = 30;
    }
    // Exercise a real button without issuing a login request.
    login.login2.ui.panel.elements[2].name = "touch-test".into();

    let mut client = Client::new(
        Settings {
            title: "Touch test".into(),
            backbuffer: (800, 600),
            fullscreen: false,
            present_mode: wgpu::PresentMode::Immediate,
            overlay: false,
            pointers: crate::scene::cursor::Pointers::none(),
        },
        Scene::LoginPanels(Box::new(login)),
    );
    client.fit = Some(Fit {
        x: 100,
        y: 50,
        width: 1600,
        height: 1200,
    });
    client
}

fn login(client: &Client) -> &crate::scene::login_panels::LoginPanels {
    let Scene::LoginPanels(login) = &client.runtime.scene else {
        panic!("expected login scene");
    };
    login
}

fn target(element: usize) -> Option<Target> {
    Some(Target {
        panel: 1,
        owner: Owner::Element(element),
    })
}

fn touch(client: &mut Client, id: u64, phase: TouchPhase, at: (f64, f64)) {
    client.pointer_touch(Touch {
        device_id: DeviceId::dummy(),
        phase,
        location: PhysicalPosition::new(at.0, at.1),
        force: None,
        id,
    });
}

fn at(client: &mut Client, id: u64, phase: TouchPhase, point: (f64, f64)) {
    let fit = client.fit.unwrap();
    touch(
        client,
        id,
        phase,
        (
            fit.x as f64 + point.0 * fit.width as f64 / 800.0,
            fit.y as f64 + point.1 * fit.height as f64 / 600.0,
        ),
    );
}

#[test]
fn first_touch_focuses_login_fields_without_prior_cursor_motion() {
    for (field, point) in [ACCOUNT, PASSWORD].into_iter().enumerate() {
        let mut client = client();
        at(&mut client, 1, TouchPhase::Started, point);
        assert_eq!(login(&client).manager.focused, target(field));
        assert_eq!(login(&client).manager.capture, target(field));

        at(&mut client, 1, TouchPhase::Ended, point);
        assert_eq!(login(&client).manager.focused, target(field));
        assert!(login(&client).manager.capture.is_none());
        assert!(login(&client).manager.pressed.is_none());
        assert!(login(&client).manager.hovered.is_none());
        assert!(client.primary_touch.is_none());
        assert!(matches!(
            client.pointer_location,
            crate::scene::cursor::Location::Outside
        ));
    }
}

#[test]
fn physical_touch_coordinates_respect_scale_and_letterboxing_and_click_once() {
    let mut client = client();
    // Physical pixels already include the display scale and fitted-content offset.
    touch(&mut client, 1, TouchPhase::Started, (380.0, 550.0));
    assert_eq!(client.pointer_in_backbuffer(), Some((140, 250)));
    assert_eq!(login(&client).manager.pressed, target(2));
    assert!(login(&client).manager.button_sounds.is_empty());

    touch(&mut client, 1, TouchPhase::Ended, (380.0, 550.0));
    assert_eq!(login(&client).manager.button_sounds, [53]);
    touch(&mut client, 1, TouchPhase::Ended, (380.0, 550.0));
    assert_eq!(login(&client).manager.button_sounds, [53]);
}

#[test]
fn motion_and_final_release_coordinates_do_not_click_the_original_button() {
    let mut client = client();
    at(&mut client, 1, TouchPhase::Started, BUTTON);
    at(&mut client, 1, TouchPhase::Moved, (150.0, 250.0));
    assert_eq!(client.pointer_in_backbuffer(), Some((150, 250)));

    // The final event may contain a new position without a preceding move event.
    at(&mut client, 1, TouchPhase::Ended, (700.0, 500.0));
    assert_eq!(client.pointer, (1500.0, 1050.0));
    assert!(login(&client).manager.button_sounds.is_empty());
    assert!(login(&client).manager.pressed.is_none());
    assert!(login(&client).manager.capture.is_none());
}

#[test]
fn secondary_fingers_cannot_move_release_or_cancel_the_active_touch() {
    let mut client = client();
    at(&mut client, 1, TouchPhase::Started, BUTTON);
    let pointer = client.pointer;
    for phase in [
        TouchPhase::Started,
        TouchPhase::Moved,
        TouchPhase::Cancelled,
        TouchPhase::Ended,
    ] {
        at(&mut client, 2, phase, ACCOUNT);
        assert_eq!(client.primary_touch, Some((DeviceId::dummy(), 1)));
        assert_eq!(client.pointer, pointer);
        assert_eq!(login(&client).manager.pressed, target(2));
    }
    at(&mut client, 1, TouchPhase::Started, PASSWORD);
    assert_eq!(client.pointer, pointer);
    at(&mut client, 1, TouchPhase::Ended, BUTTON);
    assert_eq!(login(&client).manager.button_sounds, [53]);

    at(&mut client, 2, TouchPhase::Moved, ACCOUNT);
    at(&mut client, 2, TouchPhase::Ended, ACCOUNT);
    assert!(client.primary_touch.is_none());
    assert_eq!(client.pointer, pointer);

    at(&mut client, 3, TouchPhase::Started, PASSWORD);
    assert_eq!(login(&client).manager.focused, target(1));
}

#[test]
fn cancelled_touch_disarms_without_clicking_and_allows_the_next_tap() {
    let mut client = client();
    at(&mut client, 1, TouchPhase::Started, BUTTON);
    at(&mut client, 1, TouchPhase::Cancelled, BUTTON);
    assert!(client.primary_touch.is_none());
    assert!(login(&client).manager.pressed.is_none());
    assert!(login(&client).manager.capture.is_none());
    assert!(login(&client).manager.button_sounds.is_empty());
    at(&mut client, 1, TouchPhase::Ended, BUTTON);
    assert!(login(&client).manager.button_sounds.is_empty());

    at(&mut client, 2, TouchPhase::Started, BUTTON);
    at(&mut client, 2, TouchPhase::Ended, BUTTON);
    assert_eq!(login(&client).manager.button_sounds, [53]);
}

#[test]
fn release_outside_fitted_content_disarms_the_pending_button() {
    let mut client = client();
    at(&mut client, 1, TouchPhase::Started, BUTTON);
    touch(&mut client, 1, TouchPhase::Ended, (99.0, 550.0));
    assert!(client.primary_touch.is_none());
    assert!(login(&client).manager.pressed.is_none());
    assert!(login(&client).manager.capture.is_none());
    assert!(login(&client).manager.button_sounds.is_empty());

    at(&mut client, 2, TouchPhase::Started, BUTTON);
    at(&mut client, 2, TouchPhase::Ended, BUTTON);
    assert_eq!(login(&client).manager.button_sounds, [53]);
}

#[test]
fn focus_loss_cleanup_preserves_edit_focus_and_ignores_late_touch_events() {
    let mut client = client();
    at(&mut client, 1, TouchPhase::Started, ACCOUNT);
    client.cancel_touch();
    assert!(client.primary_touch.is_none());
    assert_eq!(login(&client).manager.focused, target(0));
    assert!(login(&client).manager.capture.is_none());
    assert!(login(&client).manager.pressed.is_none());
    assert!(login(&client).manager.hovered.is_none());

    at(&mut client, 1, TouchPhase::Moved, PASSWORD);
    at(&mut client, 1, TouchPhase::Ended, PASSWORD);
    assert_eq!(login(&client).manager.focused, target(0));
    client.cancel_touch();
    assert_eq!(login(&client).manager.focused, target(0));

    at(&mut client, 2, TouchPhase::Started, PASSWORD);
    at(&mut client, 2, TouchPhase::Ended, PASSWORD);
    assert_eq!(login(&client).manager.focused, target(1));
}

#[test]
fn cancelled_slider_drag_does_not_follow_the_next_touch() {
    let mut client = client();
    let Scene::LoginPanels(scene) = &mut client.runtime.scene else {
        unreachable!();
    };
    let mut slider = ui::Element::new(
        formats::ui::pane::ElementType::Slider,
        "Slider",
        "touch-slider",
        &scene.login2.ui.resource,
    );
    slider.x = 20;
    slider.y = 170;
    slider.width = 180;
    slider.height = 20;
    slider.widget.slider_mut().unwrap().value = 50;
    scene.login2.ui.panel.elements.push(slider);

    let slider = |client: &Client| {
        login(client).login2.ui.panel.elements[3]
            .widget
            .slider()
            .unwrap()
            .clone()
    };

    at(&mut client, 1, TouchPhase::Started, (210.0, 300.0));
    at(&mut client, 1, TouchPhase::Moved, (255.0, 300.0));
    assert_eq!(slider(&client).value, 75);
    assert!(slider(&client).drag_offset.is_some());
    assert_eq!(login(&client).manager.focused, target(3));

    at(&mut client, 1, TouchPhase::Cancelled, (255.0, 300.0));
    assert_eq!(login(&client).manager.focused, target(3));
    assert!(login(&client).manager.capture.is_none());

    // A focused slider can consume motion even after manager capture is cleared.
    at(&mut client, 2, TouchPhase::Started, ACCOUNT);
    at(&mut client, 2, TouchPhase::Ended, ACCOUNT);
    assert_eq!(slider(&client).value, 75);
    assert!(slider(&client).drag_offset.is_none());
    assert_eq!(login(&client).manager.focused, target(0));
}

#[test]
fn starting_touch_clears_mouse_camera_motion_and_buttons_but_preserves_alt() {
    let mut client = client();
    client.runtime.input.mods.alt = true;
    client.runtime.input.mouse = crate::scene::camera::MouseInput {
        dx: 11,
        dy: -12,
        dz: 3,
        mmb: true,
        rmb: true,
        alt: false,
    };

    at(&mut client, 1, TouchPhase::Started, ACCOUNT);
    assert_eq!(
        client.runtime.input.mouse,
        crate::scene::camera::MouseInput {
            alt: true,
            ..Default::default()
        }
    );
    assert_eq!(login(&client).manager.focused, target(0));
}

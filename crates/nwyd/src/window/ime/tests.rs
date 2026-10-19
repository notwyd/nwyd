use super::CursorAreaCache;
use crate::scene::{Scene, ime::Bridge, login_panels};
use std::collections::VecDeque;
use winit::event::Ime;

const AREA: [f64; 4] = [120.0, 180.0, 240.0, 30.0];

#[test]
fn unchanged_ime_events_do_not_resubmit_cursor_geometry() {
    let mut cache = CursorAreaCache::default();

    let requests = (0..10_000)
        .filter_map(|_| cache.update(AREA, 1.0))
        .collect::<Vec<_>>();

    assert_eq!(requests, [AREA]);
}

#[test]
fn cursor_position_and_size_changes_are_submitted() {
    let mut cache = CursorAreaCache::default();

    assert_eq!(cache.update(AREA, 1.0), Some(AREA));

    let mut area = AREA;

    for component in 0..area.len() {
        area[component] += 1.0;

        assert_eq!(cache.update(area, 1.0), Some(area));
        assert_eq!(cache.update(area, 1.0), None);
    }
}

#[test]
fn scale_changes_resubmit_unchanged_physical_geometry() {
    let mut cache = CursorAreaCache::default();

    assert_eq!(cache.update(AREA, 1.0), Some(AREA));
    assert_eq!(cache.update(AREA, 1.5), Some(AREA));
    assert_eq!(cache.update(AREA, 1.5), None);
    assert_eq!(cache.update(AREA, 1.0), Some(AREA));
}

#[test]
fn new_ime_session_resubmits_unchanged_geometry() {
    let mut cache = CursorAreaCache::default();

    assert_eq!(cache.update(AREA, 1.0), Some(AREA));
    assert_eq!(cache.update(AREA, 1.0), None);

    cache.clear();

    assert_eq!(cache.update(AREA, 1.0), Some(AREA));
    assert_eq!(cache.update(AREA, 1.0), None);
}

fn login_scene() -> Scene {
    let mut panels = login_panels::language_tests::login();

    panels.msgbox.hide();

    panels.focus_login_field();

    Scene::LoginPanels(Box::new(panels))
}

#[test]
fn wayland_empty_preedit_acknowledgments_do_not_create_a_feedback_loop() {
    let mut scene = login_scene();

    let mut bridge = Bridge::default();

    let mut cache = CursorAreaCache::default();

    assert_eq!(bridge.synchronize(&mut scene), Some(true));

    let mut events = VecDeque::from([Ime::Enabled]);

    let mut requests = 0;

    let mut handled = 0;

    while let Some(event) = events.pop_front() {
        handled += 1;

        assert!(handled < 100, "IME acknowledgments must settle");

        assert_eq!(bridge.synchronize(&mut scene), None);

        if let Some(area) = bridge.area(&mut scene)
            && cache.update(area, 1.0).is_some()
        {
            requests += 1;

            events.push_back(Ime::Preedit(String::new(), None));
        }

        if matches!(event, Ime::Enabled | Ime::Disabled) {
            cache.clear();
        }

        bridge.event(&mut scene, event);

        assert_eq!(bridge.synchronize(&mut scene), None);

        if let Some(area) = bridge.area(&mut scene)
            && cache.update(area, 1.0).is_some()
        {
            requests += 1;

            events.push_back(Ime::Preedit(String::new(), None));
        }
    }

    assert_eq!(requests, 1);
    assert_eq!(handled, 2);
    assert!(!bridge.composing);

    bridge.event(&mut scene, Ime::Preedit("test".into(), Some((4, 4))));

    assert!(bridge.composing);
    assert_eq!(cache.update(bridge.area(&mut scene).unwrap(), 1.0), None);

    bridge.event(&mut scene, Ime::Commit("test".into()));

    let Scene::LoginPanels(panels) = scene else {
        unreachable!()
    };

    assert_eq!(panels.field_text(login_panels::FIELD_ACCOUNT), "test");
    assert!(!bridge.composing);
}

#[test]
fn refocusing_login_resubmits_unchanged_geometry() {
    let mut scene = login_scene();

    let mut bridge = Bridge::default();

    let mut cache = CursorAreaCache::default();

    assert_eq!(bridge.synchronize(&mut scene), Some(true));

    bridge.event(&mut scene, Ime::Enabled);

    let area = bridge.area(&mut scene).unwrap();

    assert_eq!(cache.update(area, 1.0), Some(area));

    bridge.focused = false;

    assert_eq!(bridge.synchronize(&mut scene), Some(false));

    cache.clear();

    assert_eq!(bridge.area(&mut scene), None);

    bridge.event(&mut scene, Ime::Disabled);

    bridge.focused = true;

    assert_eq!(bridge.synchronize(&mut scene), Some(true));

    bridge.event(&mut scene, Ime::Enabled);

    assert_eq!(bridge.area(&mut scene), Some(area));
    assert_eq!(cache.update(area, 1.0), Some(area));
}

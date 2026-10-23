use super::*;
use formats::net::account::Item;
use formats::ui::pane::ElementType;

fn shown() -> Shown {
    let mut panel = Panel {
        right: 275,
        bottom: 125,
        ..Panel::default()
    };
    for (name, y, height) in [
        (ui::tooltip::TITLE_NAME, 9, 19),
        (ui::tooltip::ANCHOR_NAME, 33, 15),
        (ui::tooltip::ROW_NAME, 33, 18),
        (ui::tooltip::ROW_NAME, 51, 18),
        (ui::tooltip::COMMENT_NAME, 69, 36),
    ] {
        let element = panel.create_element(name, ElementType::Static, &Resource::default());
        element.x = 1;
        element.y = y;
        element.width = 250;
        element.height = height;
        element.set_display(name);
    }
    Shown {
        panel,
        origin: (100, 200),
        z_order: 100,
        item: 1,
        source_content: Some(0),
        preview_source: Some(PreviewSource {
            cell: Cell {
                index: 1,
                item: Item {
                    index: 1,
                    ..Item::default()
                },
                rect: [0, 0, 32, 32],
                owner_origin: (400, 300),
                owner_z: 101,
                owner_content: Some(0),
                asking_price: 0,
            },
            right_aligned: false,
            equipment: formats::net::account::Equipment::default(),
        }),
    }
}

#[test]
fn preview_reserves_space_below_the_title_and_shifts_all_description_rows() {
    let shown = shown();
    let original = shown.panel.clone();
    let (panel, origin, rect) = preview_layout(&shown, (800, 600), 1.0).unwrap();

    assert_eq!(panel.width(), original.width());
    assert_eq!(panel.height(), original.height() + 112);
    for (before, after) in original.elements.iter().zip(&panel.elements) {
        let mut expected = before.clone();
        if matches!(
            before.name.as_str(),
            ui::tooltip::ROW_NAME | ui::tooltip::COMMENT_NAME
        ) {
            expected.y += 112;
        }
        assert_eq!(after, &expected);
    }
    assert_eq!(
        rect,
        [(origin.0 + 8) as f32, (origin.1 + 37) as f32, 259.0, 104.0]
    );
    assert_eq!(shown.panel, original);
}

#[test]
fn preview_stays_on_screen_with_scaled_and_right_aligned_sources() {
    for scale in [1.0, 1.5, 2.0] {
        for owner_origin in [(-30, -20), (790, 590), (400, 300)] {
            for right_aligned in [false, true] {
                let mut shown = shown();
                let source = shown.preview_source.as_mut().unwrap();
                source.cell.owner_origin = owner_origin;
                source.right_aligned = right_aligned;
                let (panel, origin, rect) = preview_layout(&shown, (800, 600), scale).unwrap();
                let screen = ((800.0 / scale) as i32, (600.0 / scale) as i32);

                assert!(origin.0 >= 0 && origin.1 >= 0);
                assert!(origin.0 + panel.width() <= screen.0);
                assert!(origin.1 + panel.height() <= screen.1);
                assert_eq!(rect[0], (origin.0 + 8) as f32 * scale);
                assert_eq!(rect[1], (origin.1 + 37) as f32 * scale);
                assert_eq!(rect[2], 259.0 * scale);
                assert!(rect[0] >= 0.0 && rect[1] >= 0.0);
                assert!(rect[0] + rect[2] <= 800.0);
                assert!(rect[1] + rect[3] <= 600.0);
                if right_aligned {
                    assert_eq!(
                        origin.0,
                        (owner_origin.0 + 32 - panel.width()).clamp(0, screen.0 - panel.width())
                    );
                }
            }
        }
    }
}

#[test]
fn preview_shrinks_to_available_height_and_rejects_insufficient_space() {
    let shown = shown();
    let (panel, _, rect) = preview_layout(&shown, (800, 197), 1.0).unwrap();
    assert_eq!(panel.height(), shown.panel.height() + 64);
    assert_eq!(rect[3], 56.0);

    for viewport in [(800, 196), (274, 600), (0, 0)] {
        assert!(preview_layout(&shown, viewport, 1.0).is_none());
    }
    for scale in [0.0, -1.0, f32::INFINITY, f32::NAN] {
        assert!(preview_layout(&shown, (800, 600), scale).is_none());
    }
}

#[test]
fn text_only_tooltips_and_missing_anchor_do_not_allocate_preview_space() {
    let mut shown = shown();
    shown.preview_source = None;
    assert!(preview_layout(&shown, (800, 600), 1.0).is_none());

    let mut shown = self::shown();
    shown
        .panel
        .elements
        .retain(|element| element.name != ui::tooltip::ANCHOR_NAME);
    assert!(preview_layout(&shown, (800, 600), 1.0).is_none());
}

fn registries() -> Arc<super::super::registries::Registries> {
    use formats::data::item_list::{self, field};
    use formats::mesh::registry;
    use formats::texture::texture_list;

    let mut body = vec![0; item_list::BODY_LEN];
    let entry = &mut body[formats::data::ITEM_RECORD_LEN..][..formats::data::ITEM_RECORD_LEN];
    entry[field::MESH..field::MESH + 2].copy_from_slice(&80i16.to_le_bytes());
    entry[field::EQUIP_MASK..field::EQUIP_MASK + 4].copy_from_slice(&64u32.to_le_bytes());
    Arc::new(super::super::registries::Registries {
        resource_names: formats::scene::rc::parse(&[]).unwrap(),
        entries: Vec::new(),
        valid: registry::parse_valid_index(&vec![0; registry::VALID_INDEX_LEN]).unwrap(),
        sounds: formats::audio::ani_sound::AniSound::default(),
        textures: texture_list::parse(&[]).unwrap(),
        effect_textures: texture_list::parse(&[]).unwrap(),
        items: ItemList::from_plain(body, formats::text::Codepage::Windows1252),
        list: Default::default(),
        arena_visual: Vec::new(),
    })
}

#[test]
fn disabled_preview_and_missing_models_preserve_the_text_panel() {
    let registries = registries();
    let mut tip = Tip::for_test();
    let shown = shown();
    tip.game = Some(GameDir::from_files("synthetic-tooltip-preview", []).unwrap());
    tip.shown = Some(shown.clone());

    for enabled in [false, true, false] {
        tip.configure_preview(enabled, &registries, (800, 600), 1000);
        tip.frame_draws(true);
        assert_eq!(tip.ui.panel, shown.panel);
        assert!(!tip.preview_frame.ready);
        tip.mark_preview(10);
        assert!(tip.preview_frame.after.is_none());
        assert_eq!(tip.shown.as_ref().unwrap().origin, shown.origin);
    }
}

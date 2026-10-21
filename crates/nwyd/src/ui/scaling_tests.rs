use super::*;
use formats::ui::pane::ElementType;
use ui::layout::Measure;
use ui::material::{ColorSet, Material};

fn panel(scale: f32) -> PanelUi {
    let mut resource = Resource::default();
    resource.fonts.push(guimat::FontDef {
        index: 0,
        face: 0,
        height: 12,
        width_adjust: 0,
        field4: 0,
        bold: false,
    });
    let mut chains = Chains::default();
    chains.chains[0].faces.push(std::sync::Arc::new(
        Face::parse(crate::fonts::FALLBACK, 0).unwrap(),
    ));
    let material = Material {
        font_index: 0,
        font_color: ColorSet::init(0xFFFF_FFFF, 0xFF80_8080),
        ..Default::default()
    };
    let mut caption = ui::Element::new(ElementType::Static, "Static", "caption", &resource);
    caption.x = 11;
    caption.y = 13;
    caption.width = 67;
    caption.height = 80;
    caption.multiline = true;
    caption.multiline_gap = 2;
    caption.shadow_type = ShadowType::BottomRight;
    caption.layers = vec![material];
    caption.set_display("Alpha bravo charlie delta echo foxtrot");
    let mut edit = ui::Element::new(ElementType::ImeEditBox, "ImeEditBox", "edit", &resource);
    edit.x = 11;
    edit.y = 103;
    edit.width = 93;
    edit.height = 22;
    edit.layers = vec![material; 2];
    let state = edit.widget.edit_mut().unwrap();
    state.text = "Chat input".into();
    state.caret = 7;
    state.anchor = 2;
    let panel = Panel {
        right: 160,
        bottom: 140,
        elements: vec![caption, edit],
        ..Default::default()
    };
    PanelUi::from_resource(panel, resource, &chains, scale)
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn display_scale_preserves_caption_wrapping_edit_selection_and_hit_regions() {
    let mut ui = panel(1.0);
    let interaction = Interaction {
        focused: Some(Owner::Element(1)),
        ..Default::default()
    };
    let expected = ui.draw_list(&interaction);
    let lines: Vec<_> = expected
        .texts()
        .filter(|text| text.owner == Owner::Element(0))
        .map(|text| (text.text.clone(), text.rect))
        .collect();
    assert!(lines.len() >= 3, "{lines:?}");
    assert!(expected.commands.iter().any(|command| matches!(command,
        Command::Fill(fill) if fill.owner == Owner::Element(1) && fill.rect[2] - fill.rect[0] == 2
    )));
    assert!(expected.commands.iter().any(|command| matches!(command,
        Command::Fill(fill) if fill.owner == Owner::Element(1) && fill.color == 0xFFFF_FF00
    )));
    let advance = ui.measure().advance(0, u16::from(b'W')).unwrap();
    for scale in [1.0, 1.5, 1.8, 2.4, 3.6, 1.0] {
        ui.scale = scale;
        assert_eq!(ui.draw_list(&interaction), expected, "scale {scale}");
        assert_eq!(ui.measure().height(0), Some(12));
        assert_eq!(ui.measure().advance(0, u16::from(b'W')), Some(advance));
        assert_eq!(
            ui::input::hit_test(&ui.panel, 12, 104),
            Some(Owner::Element(1))
        );
        assert_eq!(ui::input::hit_test(&ui.panel, 105, 126), None);
    }
}

fn caption_list(shadow: ShadowType) -> ui::DrawList {
    ui::DrawList {
        commands: vec![Command::Text(TextDraw {
            owner: Owner::Element(0),
            text: "Chat text".into(),
            rect: [11, 13, 78, 31],
            indent: 0,
            font: 0,
            align: 0,
            color: 0xFFFF_FFFF,
            shadow,
            shadow_color: 0xFF00_0000,
            bold: false,
        })],
        ..Default::default()
    }
}

#[test]
fn display_scale_preserves_caption_extent_and_shadow_size() {
    let mut ui = panel(1.0);
    let list = caption_list(ShadowType::BottomRight);
    for scale in [1.0, 1.5, 1.8, 2.4, 3.6] {
        ui.scale = scale;
        let draws = ui.frame_of(&list, (7, 9));
        assert_eq!(draws.len(), 2);
        let shadow = draws[0].quad;
        let foreground = draws[1].quad;
        near(foreground.dst_w, 128.0 * scale);
        near(foreground.dst_h, 64.0 * scale);
        assert!((foreground.dst_x + 0.5 - 18.0 * scale).abs() <= 0.5001);
        assert!((foreground.dst_y + 0.5 - 22.0 * scale).abs() <= 0.5001);
        near(shadow.dst_x - foreground.dst_x, 0.0);
        near(shadow.dst_y - foreground.dst_y, scale);
        near(shadow.dst_w, 128.0 * scale);
        near(shadow.dst_h, 63.0 * scale);
    }
}

#[test]
fn nested_caption_clips_preserve_scaled_screen_edges() {
    let mut ui = panel(1.0);
    let mut list = caption_list(ShadowType::None);
    list.commands.insert(
        0,
        Command::ClipPush {
            owner: Owner::Background,
            rect: [12, 12, 70, 35],
        },
    );
    list.commands.insert(
        1,
        Command::ClipPush {
            owner: Owner::Element(0),
            rect: [10, 15, 60, 33],
        },
    );
    list.commands.push(Command::ClipPop {
        owner: Owner::Element(0),
    });
    list.commands.push(Command::ClipPop {
        owner: Owner::Background,
    });
    for scale in [1.0, 1.5, 1.8, 2.4, 3.6] {
        ui.scale = scale;
        let draws = ui.frame_of(&list, (7, 9));
        assert_eq!(draws.len(), 1);
        let quad = draws[0].quad;
        near(quad.dst_x, 19.0 * scale);
        near(quad.dst_y, 24.0 * scale);
        near(quad.dst_w, 48.0 * scale);
        near(quad.dst_h, 18.0 * scale);
        assert!(quad.src_x > 0.0 && quad.src_y > 0.0);
        near(quad.src_w, quad.dst_w);
        near(quad.src_h, quad.dst_h);
        let origin = (quad.dst_x + 0.5 - quad.src_x, quad.dst_y + 0.5 - quad.src_y);

        near(origin.0, origin.0.round());
        near(origin.1, origin.1.round());
    }
}

#[test]
fn caption_bitmaps_follow_display_density_without_changing_logical_metrics() {
    let mut ui = panel(1.0);
    let list = caption_list(ShadowType::None);
    let key = (Owner::Element(0), 0);
    ui.frame_of(&list, (0, 0));
    let original = ui.cache.get(&key).unwrap().bitmap.clone();
    assert!(original.measured > 0);
    assert_eq!(original.placed, 9);
    for (scale, dimensions) in [
        (1.0, (128, 64)),
        (1.5, (192, 96)),
        (1.8, (231, 116)),
        (2.4, (308, 154)),
        (3.6, (461, 231)),
    ] {
        ui.scale = scale;
        let draws = ui.frame_of(&list, (0, 0));
        let bitmap = &ui.cache.get(&key).unwrap().bitmap;
        assert_eq!((bitmap.width, bitmap.height), dimensions, "scale {scale}");
        assert_eq!(
            bitmap.pixels.len(),
            (dimensions.0 * dimensions.1 * 4) as usize
        );
        assert_eq!(bitmap.measured, original.measured);
        assert_eq!(bitmap.placed, original.placed);
        near(draws[0].quad.src_w, 128.0 * scale);
        near(draws[0].quad.src_h, 64.0 * scale);
        near(draws[0].quad.dst_w, 128.0 * scale);
        near(draws[0].quad.dst_h, 64.0 * scale);
        let origin = (draws[0].quad.dst_x + 0.5, draws[0].quad.dst_y + 0.5);

        near(origin.0, origin.0.round());
        near(origin.1, origin.1.round());
        assert_eq!(ui.measure().height(0), Some(12));
    }
}

#[test]
fn caption_cache_reuses_unchanged_density_and_rebuilds_when_resized() {
    use crate::scene::login_static::Layout;

    let mut ui = panel(1.0);
    let list = caption_list(ShadowType::None);
    let key = (Owner::Element(0), 0);
    let mut previous = None;
    for (width, height, should_rebuild) in [
        (1920, 1080, true),
        (1920, 1080, false),
        (2560, 1080, false),
        (2560, 1440, true),
        (800, 600, true),
        (800, 600, false),
    ] {
        ui.scale = Layout::new(width, height).ui_scale();
        ui.frame_of(&list, (0, 0));
        let serial = ui.cache.get(&key).unwrap().serial;
        if let Some(old) = previous {
            if should_rebuild {
                assert!(
                    serial > old,
                    "{width}x{height} retained density from the previous frame"
                );
            } else {
                assert_eq!(
                    serial, old,
                    "{width}x{height} unnecessarily rasterized text again"
                );
            }
        }
        previous = Some(serial);
    }
    assert_eq!(ui.cache.len(), 1);
    assert_eq!(ui.cache.built(), 3);
}

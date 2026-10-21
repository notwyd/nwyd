use render::font::{Font, TextBitmap, align, raster_scale, units};

fn font() -> Font {
    Font::new(12, vec![Font::face(crate::fonts::FALLBACK, 0).unwrap()])
}

fn fallback_font() -> Font {
    let mut primary = crate::fonts::FALLBACK.to_vec();
    let count = u16::from_be_bytes(primary[4..6].try_into().unwrap()) as usize;

    for record in (12..12 + count * 16).step_by(16) {
        if &primary[record..record + 4] == b"cmap" {
            let offset =
                u32::from_be_bytes(primary[record + 8..record + 12].try_into().unwrap()) as usize;

            primary[offset + 2..offset + 4].copy_from_slice(&0u16.to_be_bytes());
            break;
        }
    }

    let primary = Font::face(&primary, 0).unwrap();

    assert_eq!(primary.glyph_index('A' as u16), 0);

    Font::new(
        12,
        vec![primary, Font::face(crate::fonts::FALLBACK, 0).unwrap()],
    )
}

fn ink_bounds(bitmap: &TextBitmap) -> Option<[u32; 4]> {
    let mut bounds: Option<[u32; 4]> = None;

    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            if bitmap.pixels[((y * bitmap.width + x) * 4 + 3) as usize] == 0 {
                continue;
            }

            bounds = Some(match bounds {
                None => [x, y, x + 1, y + 1],
                Some([left, top, right, bottom]) => {
                    [left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1)]
                }
            });
        }
    }

    bounds
}

#[test]
fn caption_render_preserves_logical_alignment_and_baseline() {
    let mut font = font();

    for (flags, bounds) in [
        (align::VCENTER, [4, 10, 18, 22]),
        (align::CENTER | align::VCENTER, [23, 10, 37, 22]),
        (align::RIGHT | align::BOTTOM, [42, 15, 56, 27]),
    ] {
        let bitmap = font.render(
            &units("Ag"),
            Some([4, 3, 60, 27]),
            flags,
            0xFF12_3456,
            (64, 32),
        );

        assert_eq!((bitmap.width, bitmap.height), (64, 32));
        assert_eq!((bitmap.measured, bitmap.placed), (15, 2));
        assert_eq!(ink_bounds(&bitmap), Some(bounds), "flags {flags}");
        assert!(
            bitmap
                .pixels
                .chunks_exact(4)
                .filter(|pixel| pixel[3] != 0)
                .all(|pixel| pixel[..3] == [0x12, 0x34, 0x56])
        );
    }
}

#[test]
fn caption_render_admits_only_whole_glyphs_and_measures_the_overflow() {
    let bitmap = font().render(
        &units("AAAAAA"),
        Some([2, 2, 20, 24]),
        align::CENTER | align::VCENTER,
        0xFFFF_FFFF,
        (19, 32),
    );

    assert_eq!((bitmap.width, bitmap.height), (19, 32));
    assert_eq!((bitmap.measured, bitmap.placed), (24, 2));
    assert_eq!(ink_bounds(&bitmap), Some([0, 8, 16, 17]));
}

#[test]
fn caption_render_without_flags_preserves_the_absolute_baseline() {
    let bitmap = font().render(&units("Ag"), None, 0, 0xFFFF_FFFF, (64, 32));

    assert_eq!((bitmap.measured, bitmap.placed), (15, 2));
    assert_eq!(ink_bounds(&bitmap), Some([9, 0, 15, 3]));
}

#[test]
fn caption_render_uses_the_larger_fallback_size_below_thirteen_pixels() {
    let bitmap = fallback_font().render(
        &units("Ag"),
        Some([4, 3, 60, 27]),
        align::VCENTER,
        0xFFFF_FFFF,
        (64, 32),
    );

    assert_eq!((bitmap.measured, bitmap.placed), (16, 2));
    assert_eq!(ink_bounds(&bitmap), Some([4, 10, 20, 22]));
}

#[test]
fn caption_raster_density_never_reduces_or_invalidates_the_base_bitmap() {
    let mut font = font();
    let text = units("Ag");
    let rect = Some([4, 3, 60, 27]);
    let expected = font.render(&text, rect, align::VCENTER, 0xFFFF_FFFF, (64, 32));

    for scale in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -2.0,
        0.0,
        0.5,
        1.0,
    ] {
        assert_eq!(raster_scale(scale), 1.0);
        assert_eq!(
            font.render_scaled(&text, rect, align::VCENTER, 0xFFFF_FFFF, (64, 32), scale),
            expected,
            "scale {scale}"
        );
    }
}

#[test]
fn caption_raster_density_keeps_logical_measurement_and_glyph_admission() {
    let mut font = font();

    for (scale, regular_size, clipped_size) in [
        (1.25, (80, 40), (24, 40)),
        (1.8, (116, 58), (35, 58)),
        (2.4, (154, 77), (46, 77)),
    ] {
        assert_eq!(raster_scale(scale), scale);

        for flags in [
            align::VCENTER,
            align::CENTER | align::VCENTER,
            align::RIGHT | align::BOTTOM,
        ] {
            let bitmap = font.render_scaled(
                &units("Ag"),
                Some([4, 3, 60, 27]),
                flags,
                0xFFFF_FFFF,
                (64, 32),
                scale,
            );

            assert_eq!((bitmap.width, bitmap.height), regular_size);
            assert_eq!((bitmap.measured, bitmap.placed), (15, 2));
        }

        let bitmap = font.render_scaled(
            &units("AAAAAA"),
            Some([2, 2, 20, 24]),
            align::CENTER | align::VCENTER,
            0xFFFF_FFFF,
            (19, 32),
            scale,
        );

        assert_eq!((bitmap.width, bitmap.height), clipped_size);
        assert_eq!((bitmap.measured, bitmap.placed), (24, 2));
    }
}

fn assert_native_glyph_mask(bitmap: &TextBitmap, size: f32, pens: &[i32], baseline: i32) {
    let face = Font::face(crate::fonts::FALLBACK, 0).unwrap();
    let (metrics, coverage) = face
        .raster()
        .rasterize_indexed(face.glyph_index('A' as u16), size);
    let mut expected = vec![0u8; (bitmap.width * bitmap.height) as usize];

    for pen in pens {
        for (index, alpha) in coverage.iter().enumerate() {
            let x = pen + metrics.xmin + (index % metrics.width) as i32;
            let y =
                baseline - metrics.ymin - metrics.height as i32 + (index / metrics.width) as i32;

            assert!(x >= 0 && x < bitmap.width as i32);
            assert!(y >= 0 && y < bitmap.height as i32);
            expected[(y as u32 * bitmap.width + x as u32) as usize] = *alpha;
        }
    }

    let actual: Vec<u8> = bitmap
        .pixels
        .chunks_exact(4)
        .map(|pixel| pixel[3])
        .collect();

    assert_eq!(actual, expected);
}

#[test]
fn full_hd_captions_use_native_glyph_masks_at_existing_logical_positions() {
    let mut font = font();

    for (flags, pens, baseline) in [
        (align::VCENTER, [7, 27], 34),
        (align::CENTER | align::VCENTER, [38, 58], 34),
        (align::RIGHT | align::BOTTOM, [68, 88], 43),
    ] {
        let bitmap = font.render_scaled(
            &units("A A"),
            Some([4, 3, 60, 27]),
            flags,
            0xFFFF_FFFF,
            (64, 32),
            1.8,
        );

        assert_eq!((bitmap.width, bitmap.height), (116, 58));
        assert_eq!((bitmap.measured, bitmap.placed), (19, 3));
        assert_native_glyph_mask(&bitmap, 12.0 * 1.8, &pens, baseline);
    }
}

#[test]
fn full_hd_captions_preserve_fallback_face_size_and_base_font_baseline() {
    let bitmap = fallback_font().render_scaled(
        &units("A A"),
        Some([4, 3, 60, 27]),
        align::VCENTER,
        0xFFFF_FFFF,
        (64, 32),
        1.8,
    );

    assert_eq!((bitmap.width, bitmap.height), (116, 58));
    assert_eq!((bitmap.measured, bitmap.placed), (22, 3));
    assert_native_glyph_mask(&bitmap, 13.0 * 1.8, &[7, 31], 34);
}

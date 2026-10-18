use super::*;

fn texture(format: PixelFormat, data: Vec<u8>) -> Texture {
    Texture {
        width: 4,
        height: 4,
        format,
        mips: vec![MipLevel {
            width: 4,
            height: 4,
            offset: 0,
            len: data.len(),
        }],
        source_len: data.len(),
        data,
    }
}

#[test]
fn samples_bc1_palette_transparency_and_bc2_explicit_alpha() {
    let color = [0x00, 0xf8, 0xe0, 0x07, 0xe4, 0xe4, 0xe4, 0xe4];
    let bc1 = texture(PixelFormat::Bc1RgbaUnorm, color.to_vec());

    assert_eq!(bc1.sample_rgba8(0, 0), Some([255, 0, 0, 255]));
    assert_eq!(bc1.sample_rgba8(1, 3), Some([0, 255, 0, 255]));
    assert_eq!(bc1.sample_rgba8(2, 1), Some([170, 85, 0, 255]));
    assert_eq!(bc1.sample_rgba8(3, 2), Some([85, 170, 0, 255]));

    let transparent = texture(
        PixelFormat::Bc1RgbaUnorm,
        vec![0, 0, 255, 255, 255, 255, 255, 255],
    );

    assert_eq!(transparent.sample_rgba8(3, 3), Some([0; 4]));

    let mut data = vec![0x50, 0xfa, 0x50, 0xfa, 0x50, 0xfa, 0x50, 0xfa];

    data.extend(color);

    let bc2 = texture(PixelFormat::Bc2RgbaUnorm, data);

    assert_eq!(bc2.sample_rgba8(0, 0), Some([255, 0, 0, 0]));
    assert_eq!(bc2.sample_rgba8(1, 2), Some([0, 255, 0, 85]));
    assert_eq!(bc2.sample_rgba8(2, 1), Some([170, 85, 0, 170]));
    assert_eq!(bc2.sample_rgba8(3, 3), Some([85, 170, 0, 255]));
}

#[test]
fn sampling_respects_mip_offset_and_rejects_out_of_bounds_or_truncated_data() {
    let mut source = texture(PixelFormat::Rgba8Unorm, vec![17; 64]);

    source.data[24..28].copy_from_slice(&[1, 2, 3, 4]);

    assert_eq!(source.sample_rgba8(2, 1), Some([1, 2, 3, 4]));
    assert_eq!(source.sample_rgba8(4, 0), None);
    assert_eq!(source.sample_rgba8(0, 4), None);

    source.data.insert(0, 99);
    source.mips[0].offset = 1;

    assert_eq!(source.sample_rgba8(2, 1), Some([1, 2, 3, 4]));

    source.data.truncate(24);

    assert_eq!(source.sample_rgba8(2, 1), None);

    let truncated = texture(PixelFormat::Bc1RgbaUnorm, vec![0; 7]);

    assert_eq!(truncated.sample_rgba8(0, 0), None);
}

use formats::texture::{MipLevel, Texture};
use std::borrow::Cow;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MipPolicy {
    Opaque,
    Blend,
    Cutout(u8),
}

pub fn with_mipmaps(texture: &Texture, policy: MipPolicy) -> Cow<'_, Texture> {
    let Some(levels) = valid_levels(texture) else {
        return Cow::Borrowed(texture);
    };

    if texture.mips.len() == levels {
        return Cow::Borrowed(texture);
    }

    let Ok(mut result) = texture.clone().into_rgba8() else {
        return Cow::Borrowed(texture);
    };

    let base = result.mips[0];
    let coverage = match policy {
        MipPolicy::Cutout(reference) if reference > 0 => {
            let covered = result.data[base.offset..base.offset + base.len]
                .chunks_exact(4)
                .filter(|pixel| pixel[3] >= reference)
                .count();

            Some((reference, covered as f64 / (base.len / 4) as f64))
        }
        _ => None,
    };

    let mut previous = *result.mips.last().unwrap();
    let mut unscaled = None;

    while result.mips.len() < levels {
        let width = (previous.width / 2).max(1);
        let height = (previous.height / 2).max(1);
        let source = unscaled
            .as_deref()
            .unwrap_or(&result.data[previous.offset..previous.offset + previous.len]);
        let next = downsample(
            source,
            previous.width,
            previous.height,
            width,
            height,
            policy,
        );
        let offset = result.data.len();

        result.data.extend_from_slice(&next);

        if let Some((reference, target)) = coverage {
            preserve_coverage(&mut result.data[offset..], reference, target);
        }

        previous = MipLevel {
            width,
            height,
            offset,
            len: next.len(),
        };

        result.mips.push(previous);
        unscaled = Some(next);
    }

    Cow::Owned(result)
}

fn valid_levels(texture: &Texture) -> Option<usize> {
    if texture.width == 0 || texture.height == 0 || texture.mips.is_empty() {
        return None;
    }

    let levels = (texture.width.max(texture.height).ilog2() + 1) as usize;

    if texture.mips.len() > levels {
        return None;
    }

    let mut remaining = 256 * 1024 * 1024usize;

    for level in 0..levels {
        let width = (texture.width >> level).max(1);
        let height = (texture.height >> level).max(1);
        let rgba_bytes = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;

        remaining = remaining.checked_sub(rgba_bytes)?;

        if let Some(mip) = texture.mips.get(level) {
            let len = match texture.format.block_bytes() {
                Some(block) => (width.div_ceil(4) as usize)
                    .checked_mul(height.div_ceil(4) as usize)?
                    .checked_mul(block)?,
                None => rgba_bytes,
            };

            if mip.width != width
                || mip.height != height
                || mip.len != len
                || mip.offset.checked_add(len)? > texture.data.len()
            {
                return None;
            }
        }
    }

    Some(levels)
}

fn downsample(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    policy: MipPolicy,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(width as usize * height as usize * 4);
    let scale_x = source_width as f64 / width as f64;
    let scale_y = source_height as f64 / height as f64;

    for y in 0..height {
        let top = y as f64 * scale_y;
        let bottom = (y + 1) as f64 * scale_y;

        for x in 0..width {
            let left = x as f64 * scale_x;
            let right = (x + 1) as f64 * scale_x;
            let mut color = [0.0; 3];
            let mut alpha = 0.0;
            let mut color_weight = 0.0;

            for sy in top.floor() as u32..(bottom.ceil() as u32).min(source_height) {
                let weight_y = bottom.min((sy + 1) as f64) - top.max(sy as f64);

                for sx in left.floor() as u32..(right.ceil() as u32).min(source_width) {
                    let weight = weight_y * (right.min((sx + 1) as f64) - left.max(sx as f64));
                    let offset = (sy as usize * source_width as usize + sx as usize) * 4;
                    let pixel = &source[offset..offset + 4];
                    let a = if policy == MipPolicy::Opaque {
                        255.0
                    } else {
                        pixel[3] as f64
                    };
                    let weighted_color = weight
                        * if matches!(policy, MipPolicy::Cutout(_)) {
                            a
                        } else {
                            255.0
                        };

                    for channel in 0..3 {
                        color[channel] += pixel[channel] as f64 * weighted_color;
                    }

                    color_weight += weighted_color;
                    alpha += weight * a;
                }
            }

            for value in color {
                data.push(if color_weight > 0.0 {
                    (value / color_weight).round() as u8
                } else {
                    0
                });
            }

            data.push((alpha / (scale_x * scale_y)).round() as u8);
        }
    }

    data
}

fn preserve_coverage(data: &mut [u8], reference: u8, target: f64) {
    let mut histogram = [0usize; 256];

    for pixel in data.chunks_exact(4) {
        histogram[pixel[3] as usize] += 1;
    }

    let count = (data.len() / 4) as f64;
    let current: usize = histogram[reference as usize..].iter().sum();
    let mut best_error = (current as f64 / count - target).abs();
    let mut best_ratio = (1u32, 1u32);
    let mut covered = 0usize;

    for alpha in (1..=255).rev() {
        if histogram[alpha] == 0 {
            continue;
        }

        covered += histogram[alpha];

        let scale = reference as f64 / alpha as f64;
        let error = (covered as f64 / count - target).abs();

        let best_scale = best_ratio.0 as f64 / best_ratio.1 as f64;

        if error < best_error || (error == best_error && scale.ln().abs() < best_scale.ln().abs()) {
            best_error = error;
            best_ratio = (reference as u32, alpha as u32);
        }
    }

    if target < best_error {
        best_ratio = (
            (reference - 1) as u32,
            histogram
                .iter()
                .rposition(|&count| count > 0)
                .unwrap_or(1)
                .max(1) as u32,
        );
    }

    if best_ratio.0 != best_ratio.1 {
        for pixel in data.chunks_exact_mut(4) {
            pixel[3] = ((pixel[3] as u32 * best_ratio.0) / best_ratio.1).min(255) as u8;
        }
    }
}

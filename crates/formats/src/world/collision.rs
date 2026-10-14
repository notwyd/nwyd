use super::attribute_map::AttributeMap;
use super::object_bin::{MASK_CENTRE, MASK_DIM, ObjectMasks};
use super::objects::{ObjRecord, ObjectClass};
use super::path::RUNTIME_DIM;
use super::trn::HEIGHT_GRID_DIM;
use super::{IMPASSABLE, ZONE_CELLS};

pub const PIVOT: f64 = 7.5;

pub const HALF_CELL: f64 = 0.5;

pub const MASK_STEP: i32 = 3;

pub const HEIGHT_DIVISOR: f64 = 0.1f32 as f64;

pub fn bearing(yaw: f32) -> f64 {
    f64::from(std::f32::consts::PI) + f64::from(yaw)
}

pub fn stamps(class: ObjectClass) -> bool {
    matches!(
        class,
        ObjectClass::StaticMesh
            | ObjectClass::Suppressed
            | ObjectClass::Tree
            | ObjectClass::Ship
            | ObjectClass::Prop
            | ObjectClass::Dust
            | ObjectClass::House
    )
}

pub fn stamp(grid: &mut [i8], masks: &ObjectMasks, record: &ObjRecord) -> bool {
    let Some(mask) = masks.mask(record.mask_id()) else {
        return false;
    };

    let angle = bearing(record.yaw);

    let (sin, cos) = (angle.sin(), angle.cos());

    let mut turned = [0u8; MASK_DIM * MASK_DIM];

    for row in 0..MASK_DIM {
        let dz = row as f64 + HALF_CELL - PIVOT;

        for col in 0..MASK_DIM {
            let dx = col as f64 + HALF_CELL - PIVOT;

            let sx = (cos * dx - sin * dz + PIVOT) as i32;

            let sz = (sin * dx + cos * dz + PIVOT) as i32;

            if (0..MASK_DIM as i32).contains(&sx) && (0..MASK_DIM as i32).contains(&sz) {
                turned[row * MASK_DIM + col] = mask[sz as usize * MASK_DIM + sx as usize];
            }
        }
    }

    let base_x = record.x as i32 - MASK_CENTRE as i32;

    let base_z = record.z as i32 - MASK_CENTRE as i32;

    let mut wrote = false;

    for row in 0..MASK_DIM {
        let z = base_z + row as i32;

        for col in 0..MASK_DIM {
            let x = base_x + col as i32;

            if !(0..=HEIGHT_GRID_DIM as i32).contains(&x)
                || !(0..=HEIGHT_GRID_DIM as i32).contains(&z)
            {
                continue;
            }

            let m = turned[row * MASK_DIM + col];

            if m == 0 {
                continue;
            }

            let raised =
                (MASK_STEP * i32::from(m as i8)) as f64 + f64::from(record.y) / HEIGHT_DIVISOR;

            let mut height = raised as i32 + i32::from(record.y > 0.0);

            height = height.min(i32::from(IMPASSABLE));

            let index = z as usize * HEIGHT_GRID_DIM + x as usize;

            if let Some(cell) = grid.get_mut(index)
                && height > i32::from(*cell)
            {
                *cell = height as i8;

                wrote = true;
            }
        }
    }

    wrote
}

pub fn stamp_all(grid: &mut [i8], masks: &ObjectMasks, records: &[ObjRecord]) -> usize {
    records
        .iter()
        .filter(|r| stamps(r.class()))
        .filter(|r| stamp(grid, masks, r))
        .count()
}

pub fn window(ground: &[i8], zone: (u8, u8), attributes: Option<&AttributeMap>) -> Vec<i8> {
    window_over(&[(zone, ground)], zone, attributes)
}

pub fn window_over(
    grids: &[((u8, u8), &[i8])],
    anchor: (u8, u8),
    attributes: Option<&AttributeMap>,
) -> Vec<i8> {
    let stride = RUNTIME_DIM as usize;

    let mut out = vec![0i8; stride * stride];

    for (zone, ground) in grids {
        let (Some(qx), Some(qz)) = (zone.0.checked_sub(anchor.0), zone.1.checked_sub(anchor.1))
        else {
            continue;
        };

        if usize::from(qx) * ZONE_CELLS >= stride || usize::from(qz) * ZONE_CELLS >= stride {
            continue;
        }

        let (ox, oz) = (usize::from(qx) * ZONE_CELLS, usize::from(qz) * ZONE_CELLS);

        for row in 0..HEIGHT_GRID_DIM {
            let src = row * HEIGHT_GRID_DIM;

            let Some(source) = ground.get(src..src + HEIGHT_GRID_DIM) else {
                break;
            };

            let dst = (oz + row) * stride + ox;

            out[dst..dst + HEIGHT_GRID_DIM].copy_from_slice(source);
        }
    }

    if let Some(map) = attributes {
        map.stamp_impassable(
            &mut out,
            usize::from(anchor.0) * ZONE_CELLS,
            usize::from(anchor.1) * ZONE_CELLS,
            stride,
        );
    }

    out
}

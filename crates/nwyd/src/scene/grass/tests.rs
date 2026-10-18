use super::*;
use formats::world::trn::TrnCell;

fn field(distances: &[f32]) -> Field {
    let records = distances
        .iter()
        .enumerate()
        .map(|(index, &distance)| ObjRecord {
            kind: 0x137,
            x: distance,
            z: 0.0,
            y: 0.0,
            yaw: index as f32 * 0.1,
            p0: 0,
            p1: 0,
            extra: None,
        })
        .collect::<Vec<_>>();
    let terrain = Terrain {
        name: "grass view distance".into(),
        zone_x: 0,
        zone_y: 0,
        cells: vec![
            TrnCell {
                height: 0,
                uv_mode: 0,
                uv_mode2: 0,
                pad: 0,
                tile0: 0,
                tile1: 0,
                color: [255; 4],
            };
            formats::world::TRN_DIM * formats::world::TRN_DIM
        ],
    };

    Field::build(&records, &terrain, &[], [0.25, 0.5, 0.75, 1.0], true)
}

#[test]
fn grass_visibility_extends_with_each_view_distance_without_changing_blades() {
    let mut grass = field(&[
        20.0, 24.0, 28.0, 30.0, 31.0, 40.0, 42.0, 45.0, 46.0, 56.0, 60.0, 61.0, 80.0, 84.0, 90.0,
        91.0, 112.0, 120.0, 121.0, 140.0, 150.0, 151.0, 168.0, 180.0, 181.0, 196.0, 210.0, 211.0,
        224.0, 240.0, 241.0,
    ]);
    let blades = grass.blades.clone();

    assert_eq!(grass.view_distance, ViewDistance::Retail);

    for (distance, count) in [
        (ViewDistance::Retail, 4),
        (ViewDistance::Extended, 8),
        (ViewDistance::Distant, 11),
        (ViewDistance::Far, 15),
        (ViewDistance::Maximum, 18),
        (ViewDistance::FiveTimes, 21),
        (ViewDistance::SixTimes, 24),
        (ViewDistance::SevenTimes, 27),
        (ViewDistance::EightTimes, 30),
        (ViewDistance::Retail, 4),
    ] {
        grass.view_distance = distance;
        let visible = grass
            .visible(Vec3::new(0.0, 1000.0, 0.0))
            .collect::<Vec<_>>();
        assert_eq!(visible.len(), count, "{distance:?}");
        assert_eq!(visible.last().unwrap().0, count - 1);

        for (index, blade, distance_squared) in visible {
            assert_eq!(blade, &blades[index]);
            assert_eq!(distance_squared, blade.position.x * blade.position.x);
        }
        assert_eq!(grass.blades, blades);
    }
}

#[test]
fn grass_material_fade_scales_with_view_distance_and_preserves_retail_fog() {
    let fog = SkinFog::on(50.0, 150.0);
    let ratios = [0.0, 20.0, 592.0_f32.sqrt(), 28.0, 30.0];

    for distance in ViewDistance::ALL {
        let multiplier = distance.multiplier();
        let mut grass = field(&ratios.map(|radius| radius * multiplier));
        grass.view_distance = distance;
        let visible = grass.visible(Vec3::ZERO).collect::<Vec<_>>();
        assert_eq!(visible.len(), ratios.len(), "{distance:?}");

        for ((index, blade, squared), expected) in
            visible
                .into_iter()
                .zip([1.0, 1.0, 0.5, 0.0, 1.0 - 500.0 / 384.0])
        {
            let params = grass.params(blade, squared, fog);
            assert!(
                (params.diffuse[3] - expected).abs() < 2e-6,
                "{distance:?} sample{index}: {} vs{expected}",
                params.diffuse[3]
            );
            assert_eq!(&params.diffuse[..3], &[0.0; 3]);
            assert_eq!(params.emissive, blade.emissive);
            assert_eq!(params.ambient, [0.0; 4]);
            assert_eq!(params.fog_start, fog.start);
            assert_eq!(params.fog_end, fog.end);
            assert_eq!(params.fog_enabled, fog.enabled);
            assert!(params.unlit && params.modulate_alpha);
            assert!(!params.force_opaque);
            assert!(params.glow.is_none());

            if distance == ViewDistance::Retail {
                assert_eq!(params.diffuse[3], Blade::alpha(squared));
            }
        }
    }
}

#[test]
fn disabled_grass_remains_hidden_for_every_view_distance() {
    let mut grass = field(&[
        0.0, 20.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0, 210.0, 240.0,
    ]);
    grass.enabled = false;

    for distance in ViewDistance::ALL {
        grass.view_distance = distance;
        assert_eq!(grass.visible(Vec3::ZERO).count(), 0);
        assert_eq!(grass.bucket_bounds(Vec3::ZERO), (0, 0));
    }
}

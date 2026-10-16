use super::*;
use crate::cull::Aabb;
use formats::world::trn::TrnCell;
use std::collections::BTreeMap;

fn quad(x: i32, z: i32, height: f32) -> TerrainMesh {
    let vertices: Vec<_> = [(0, 0), (0, 1), (1, 0), (1, 1)]
        .into_iter()
        .enumerate()
        .map(|(index, (dx, dz))| WorldVertex {
            pos: [(x + dx) as f32, height, (z + dz) as f32],
            normal: [height / 10.0, 1.0, 0.0],
            diffuse: height as u32,
            uv0: [index as f32, 0.25],
            uv1: [0.75, index as f32],
        })
        .collect();
    let mut bounds = Aabb::EMPTY;

    for vertex in &vertices {
        bounds.grow(vertex.pos);
    }

    TerrainMesh {
        vertices,
        batches: vec![TerrainBatch {
            base_texture_id: 10,
            detail_texture_id: 1024,
            material: TerrainMaterial::Detail,
            first_quad: 0,
            quad_count: 1,
            bounds,
        }],
        world: Matrix4::IDENTITY,
        normal_matrix: Matrix4::IDENTITY,
        zone: (16, 16),
    }
}

fn assert_sample(vertex: &WorldVertex, height: f32) {
    assert_eq!(vertex.pos[1], height);
    assert_eq!(vertex.normal, [height / 10.0, 1.0, 0.0]);
    assert_eq!(vertex.diffuse, height as u32);
}

fn assert_change_ranges(
    before: &[WorldVertex],
    after: &[WorldVertex],
    ranges: &[std::ops::Range<usize>],
) {
    let expected: Vec<_> = before
        .iter()
        .zip(after)
        .enumerate()
        .filter_map(|(index, (old, new))| (old != new).then_some(index))
        .collect();
    let actual: Vec<_> = ranges.iter().cloned().flatten().collect();

    assert_eq!(actual, expected);
    assert!(ranges.iter().all(|range| range.start < range.end));
    assert!(ranges.windows(2).all(|pair| pair[0].end < pair[1].start));
}

#[test]
fn shared_x_and_z_edges_report_exact_changes_and_refresh_bounds() {
    for (x, z, dx, dz, expected_ranges) in [
        (63, 20, 1, 0, vec![2..4]),
        (-1, 20, 1, 0, vec![2..4]),
        (20, 63, 0, 1, vec![1..2, 3..4]),
        (20, -1, 0, 1, vec![1..2, 3..4]),
    ] {
        let mut mesh = quad(x, z, 1.0);
        mesh.append_neighbour(&quad(x + dx, z + dz, 9.0), 0, 0);
        let before = mesh.vertices.clone();
        let ranges = mesh.stitch_boundaries();

        assert_eq!(ranges, expected_ranges);
        assert_change_ranges(&before, &mesh.vertices, &ranges);
        assert_eq!(mesh.batches[0].bounds.min[1], 1.0);
        assert_eq!(mesh.batches[0].bounds.max[1], 9.0);
        assert_eq!(mesh.batches[1].bounds.min[1], 9.0);
        assert_eq!(mesh.batches[1].bounds.max[1], 9.0);

        for range in ranges {
            for index in range {
                assert_sample(&mesh.vertices[index], 9.0);
            }
        }

        for (old, new) in before.iter().zip(&mesh.vertices) {
            assert_eq!(new.pos[0], old.pos[0]);
            assert_eq!(new.pos[2], old.pos[2]);
            assert_eq!(new.uv0, old.uv0);
            assert_eq!(new.uv1, old.uv1);
        }

        assert!(mesh.stitch_boundaries().is_empty());
    }
}

fn permutations(values: &[usize]) -> Vec<Vec<usize>> {
    if values.is_empty() {
        return vec![Vec::new()];
    }

    let mut result = Vec::new();

    for (index, &head) in values.iter().enumerate() {
        let mut rest = values.to_vec();
        rest.remove(index);

        for mut tail in permutations(&rest) {
            tail.insert(0, head);
            result.push(tail);
        }
    }

    result
}

fn ordered_quads(mesh: &TerrainMesh) -> BTreeMap<(i32, i32), [WorldVertex; 4]> {
    mesh.vertices
        .chunks_exact(4)
        .map(|quad| {
            (
                (quad[0].pos[0] as i32, quad[0].pos[2] as i32),
                quad.try_into().unwrap(),
            )
        })
        .collect()
}

#[test]
fn shared_corners_are_deterministic_for_every_arrival_order_and_missing_neighbor() {
    for (x, z) in [(64, 64), (0, 0), (-64, -64)] {
        let quads = [
            quad(x - 1, z - 1, 1.0),
            quad(x, z - 1, 2.0),
            quad(x - 1, z, 3.0),
            quad(x, z, 4.0),
        ];

        for present in [
            vec![0, 1, 2, 3],
            vec![0, 1, 2],
            vec![0, 1, 3],
            vec![0, 2, 3],
            vec![1, 2, 3],
            vec![0, 3],
        ] {
            let mut baseline = None;

            for order in permutations(&present) {
                let mut mesh = quads[order[0]].clone();
                let mut loaded = vec![order[0]];

                for &next in &order[1..] {
                    mesh.append_neighbour(&quads[next], 0, 0);
                    loaded.push(next);
                    let before = mesh.vertices.clone();
                    let ranges = mesh.stitch_boundaries();
                    assert_change_ranges(&before, &mesh.vertices, &ranges);

                    let owner = loaded
                        .iter()
                        .copied()
                        .max_by_key(|index| match index {
                            0 => (0, 0),
                            1 => (1, 0),
                            2 => (0, 1),
                            3 => (1, 1),
                            _ => unreachable!(),
                        })
                        .unwrap();

                    for vertex in &mesh.vertices {
                        if vertex.pos[0] == x as f32 && vertex.pos[2] == z as f32 {
                            assert_sample(vertex, owner as f32 + 1.0);
                        }
                    }

                    assert!(mesh.stitch_boundaries().is_empty());
                }

                let result = ordered_quads(&mesh);

                if let Some(expected) = &baseline {
                    assert_eq!(&result, expected);
                } else {
                    baseline = Some(result);
                }
            }
        }
    }
}

fn terrain(zone_x: u8, zone_y: u8, offset: i8) -> Terrain {
    Terrain {
        name: "synthetic-terrain".into(),
        zone_x,
        zone_y,
        cells: (0..64 * 64)
            .map(|index| {
                let x = index % 64;
                let z = index / 64;

                TrnCell {
                    height: offset + ((x + z) % 9) as i8,
                    uv_mode: (index % 8) as u8,
                    uv_mode2: (index % 32) as u8,
                    pad: 0,
                    tile0: (index % 2) as i16,
                    tile1: 0,
                    color: [x as u8, z as u8, offset as u8, 255],
                }
            })
            .collect(),
    }
}

#[test]
fn bucketed_terrain_stitches_without_changing_interiors_materials_or_source_data() {
    let west = terrain(16, 16, 0);
    let east = terrain(17, 16, 20);
    let source_data = (west.clone(), east.clone());
    let height_grids = (west.build_height_grid(), east.build_height_grid());
    let mut mesh = build_terrain(&west);
    let neighbor = build_terrain(&east);

    mesh.append_neighbour(&neighbor, 64, 0);
    let before = mesh.clone();
    let ranges = mesh.stitch_boundaries();

    assert!(!ranges.is_empty());
    assert_change_ranges(&before.vertices, &mesh.vertices, &ranges);
    assert_eq!(mesh.vertices.len(), 32_768);
    assert_eq!(mesh.vertices.len(), before.vertices.len());
    assert_eq!(mesh.world, before.world);
    assert_eq!(mesh.normal_matrix, before.normal_matrix);
    assert_eq!(mesh.zone, before.zone);
    assert_eq!(mesh.batches.len(), before.batches.len());

    for (old, new) in before.vertices.iter().zip(&mesh.vertices) {
        assert_eq!(new.pos[0], old.pos[0]);
        assert_eq!(new.pos[2], old.pos[2]);
        assert_eq!(new.uv0, old.uv0);
        assert_eq!(new.uv1, old.uv1);

        if new.pos[0] != 64.0 {
            assert_eq!(new, old);
        }
    }

    for vertex in &mesh.vertices {
        if vertex.pos[0] == 64.0 {
            let source = neighbor
                .vertices
                .iter()
                .find(|source| source.pos[0] == 0.0 && source.pos[2] == vertex.pos[2])
                .unwrap();

            assert_eq!(vertex.pos[1], source.pos[1]);
            assert_eq!(vertex.normal, source.normal);
            assert_eq!(vertex.diffuse, source.diffuse);
        }
    }

    for (old, new) in before.batches.iter().zip(&mesh.batches) {
        assert_eq!(new.base_texture_id, old.base_texture_id);
        assert_eq!(new.detail_texture_id, old.detail_texture_id);
        assert_eq!(new.material, old.material);
        assert_eq!(new.first_quad, old.first_quad);
        assert_eq!(new.quad_count, old.quad_count);
        let mut expected = Aabb::EMPTY;
        let start = new.first_quad as usize * 4;
        let end = start + new.quad_count as usize * 4;

        for vertex in &mesh.vertices[start..end] {
            expected.grow(vertex.pos);
        }

        assert_eq!(new.bounds, expected);
    }

    assert!(mesh.stitch_boundaries().is_empty());
    assert_eq!((west.clone(), east.clone()), source_data);
    assert_eq!(
        (west.build_height_grid(), east.build_height_grid()),
        height_grids
    );
}

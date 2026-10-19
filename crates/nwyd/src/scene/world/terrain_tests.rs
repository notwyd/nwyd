use super::*;
use formats::world::TRN_DIM;
use formats::world::trn::TrnCell;

fn terrain(zone_x: u8, height: i8, colour: u32) -> Terrain {
    Terrain {
        name: format!("synthetic-{zone_x}"),
        zone_x,
        zone_y: 16,
        cells: vec![
            TrnCell {
                height,
                uv_mode: 0,
                uv_mode2: 0,
                pad: 0,
                tile0: 0,
                tile1: 0,
                color: colour.to_le_bytes(),
            };
            TRN_DIM * TRN_DIM
        ],
    }
}

#[test]
fn appending_neighbour_marks_previously_resident_boundary_vertices_dirty() {
    let west = terrain(16, 7, 0xFF11_2233);

    let east = terrain(17, 13, 0xFF44_5566);

    let mut mesh = world::build_terrain(&west);

    let resident = mesh.vertices.len();

    let boundary: Vec<_> = mesh
        .vertices
        .iter()
        .enumerate()
        .filter_map(|(index, vertex)| (vertex.pos[0] == 64.0).then_some(index))
        .collect();

    assert!(!boundary.is_empty());

    let mut revision = 11;

    let mut pages = vec![11; resident.div_ceil(256)];

    mesh.append_neighbour(&world::build_terrain(&east), 64, 0);

    stitch_all(&mut mesh, &mut revision, &mut pages);

    assert_eq!(revision, 12);
    assert_eq!(pages.len(), mesh.vertices.len().div_ceil(256));

    for index in boundary {
        assert!(index < resident);
        assert_eq!(mesh.vertices[index].pos[1], 13.0);
        assert_eq!(mesh.vertices[index].diffuse, 0xFF44_5566);
        assert!(pages[index / 256] > 11);
    }

    let stitched = mesh.clone();

    let dirty_pages = pages.clone();

    stitch_all(&mut mesh, &mut revision, &mut pages);

    assert_eq!(mesh, stitched);
    assert_eq!(revision, 12);
    assert_eq!(pages, dirty_pages);
}

#[test]
fn recolouring_restitches_shared_colours_and_marks_existing_pages_dirty() {
    let west = terrain(16, 7, 0xFF11_2233);

    let mut east = terrain(17, 13, 0xFF44_5566);

    let mut mesh = world::build_terrain(&west);

    let resident_west = mesh.vertices.len();

    mesh.append_neighbour(&world::build_terrain(&east), 64, 0);

    let mut revision = 0;

    let mut pages = Vec::new();

    stitch_all(&mut mesh, &mut revision, &mut pages);

    let previous_revision = revision;

    let geometry: Vec<_> = mesh.vertices.iter().map(|v| (v.pos, v.normal)).collect();

    let bounds: Vec<_> = mesh.batches.iter().map(|batch| batch.bounds).collect();

    for cell in &mut east.cells {
        cell.color = 0xFF77_8899u32.to_le_bytes();
    }

    refresh_terrain_colours(&mut mesh, &[&west, &east], &mut revision, &mut pages, &[1]);

    assert!(revision > previous_revision);

    for (index, vertex) in mesh.vertices.iter().enumerate() {
        assert_eq!((vertex.pos, vertex.normal), geometry[index]);

        if vertex.pos[0] == 64.0 {
            assert_eq!(vertex.diffuse, 0xFF77_8899);
            assert!(pages[index / 256] > previous_revision);
        } else if index < resident_west {
            assert_eq!(vertex.diffuse, 0xFF11_2233);
        } else {
            assert_eq!(vertex.diffuse, 0xFF77_8899);
        }
    }

    assert_eq!(
        mesh.batches
            .iter()
            .map(|batch| batch.bounds)
            .collect::<Vec<_>>(),
        bounds,
    );
}

#[test]
fn boundary_lamp_recolours_both_zones_without_changing_collision() {
    let mut west = terrain(16, 7, 0xFF11_2233);
    let mut east = terrain(17, 13, 0xFF44_5566);
    let distant = terrain(23, 19, 0xFF77_8899);
    let grids = [west.build_height_grid(), east.build_height_grid()];
    let mut mesh = world::build_terrain(&west);
    mesh.append_neighbour(&world::build_terrain(&east), 64, 0);
    mesh.append_neighbour(&world::build_terrain(&distant), 448, 0);
    let mut revision = 1;
    let mut pages = vec![1; mesh.vertices.len().div_ceil(256)];
    stitch_all(&mut mesh, &mut revision, &mut pages);
    let previous = revision;
    let distant_vertices = mesh.vertices[2 * 64 * 64 * 4..].to_vec();
    let lamp = formats::world::objects::ObjRecord {
        kind: 0x1FF,
        x: 0.0,
        z: 30.0,
        y: 3.0,
        yaw: 0.0,
        p0: 0,
        p1: 0,
        extra: Some([0.5, 1.0]),
    };
    let mut zones = lights::Zones::new(vec![&mut west, &mut east]);
    let lights = lights::build(
        &mut zones,
        &mut lights::Lamps::new(SCENE_FIELD),
        &[lamp],
        (17, 16),
        SCENE_FIELD,
    );
    let registered = zones.registered_zones().to_vec();
    assert_eq!(registered, [0, 1]);
    assert!(lights.registrations.iter().all(|r| r.landed));
    let colour = lights.registrations[0].colour;
    assert_eq!(west.cells[13 * 64 + 63].color, colour.to_le_bytes());
    assert_eq!(east.cells[14 * 64].color, colour.to_le_bytes());
    refresh_terrain_colours(
        &mut mesh,
        &[&west, &east, &distant],
        &mut revision,
        &mut pages,
        &registered,
    );
    let mut expected = world::build_terrain(&west);
    expected.append_neighbour(&world::build_terrain(&east), 64, 0);
    expected.append_neighbour(&world::build_terrain(&distant), 448, 0);
    expected.stitch_boundaries();
    assert_eq!(mesh, expected);
    assert_eq!(&mesh.vertices[2 * 64 * 64 * 4..], &distant_vertices);
    assert!(
        pages[2 * 64 * 64 * 4 / 256..]
            .iter()
            .all(|v| *v <= previous)
    );
    assert!(pages[..64 * 64 * 4 / 256].iter().any(|v| *v > previous));
    assert!(
        pages[64 * 64 * 4 / 256..2 * 64 * 64 * 4 / 256]
            .iter()
            .any(|v| *v > previous)
    );
    assert_eq!([west.build_height_grid(), east.build_height_grid()], grids);
}

fn stitch_all(mesh: &mut TerrainMesh, revision: &mut u64, pages: &mut Vec<u64>) {
    let range = 0..mesh.vertices.len();
    stitch_terrain_boundaries(mesh, revision, pages, &[range]);
}

fn build_stitched(terrains: &[Terrain]) -> TerrainMesh {
    let mut mesh = world::build_terrain(&terrains[0]);
    for terrain in &terrains[1..] {
        mesh.append_neighbour(
            &world::build_terrain(terrain),
            (i32::from(terrain.zone_x) - i32::from(terrains[0].zone_x)) * 64,
            (i32::from(terrain.zone_y) - i32::from(terrains[0].zone_y)) * 64,
        );
    }
    mesh.stitch_boundaries();
    mesh
}

fn terrain_permutations(values: &[usize]) -> Vec<Vec<usize>> {
    if values.is_empty() {
        return vec![Vec::new()];
    }
    let mut orders = Vec::new();
    for (index, &value) in values.iter().enumerate() {
        let mut tail = values.to_vec();
        tail.remove(index);
        for mut order in terrain_permutations(&tail) {
            order.insert(0, value);
            orders.push(order);
        }
    }
    orders
}

#[test]
fn scoped_commits_match_full_rebuild_for_every_corner_arrival_order() {
    let zones = [(15, 15), (16, 15), (15, 16), (16, 16)];
    for order in terrain_permutations(&[0, 1, 2, 3]) {
        let mut terrains = vec![terrain(22, 3, 0xFF12_3456)];
        let mut mesh = build_stitched(&terrains);
        let mut revision = 0;
        let mut pages = Vec::new();
        for next in order {
            let (x, z) = zones[next];
            let mut terrain = terrain(x, (next * 3) as i8, 0xFF88_0000 + next as u32);
            terrain.zone_y = z;
            for (index, cell) in terrain.cells.iter_mut().enumerate() {
                cell.height += (index % 7) as i8;
                cell.tile0 = (index % 3) as i16;
                cell.color[1] = (index % 63) as u8;
            }
            let before = mesh.vertices.clone();
            let previous = revision;
            mesh.append_neighbour(
                &world::build_terrain(&terrain),
                (i32::from(x) - 22) * 64,
                (i32::from(z) - 16) * 64,
            );
            terrains.push(terrain);
            let references: Vec<_> = terrains.iter().collect();
            refresh_terrain_colours(
                &mut mesh,
                &references,
                &mut revision,
                &mut pages,
                &[terrains.len() - 1],
            );
            assert_eq!(mesh, build_stitched(&terrains));
            for (index, (old, new)) in before.iter().zip(&mesh.vertices).enumerate() {
                if old != new {
                    assert!(pages[index / 256] > previous);
                }
            }
            assert!(pages[..64 * 64 * 4 / 256].iter().all(|&v| v == 0));
        }
        let height_grids: Vec<_> = terrains.iter().map(Terrain::build_height_grid).collect();
        for index in [1, 3, 2, 4, 1, 2, 3, 4, 2, 1] {
            let before = mesh.vertices.clone();
            let previous = revision;
            for cell in &mut terrains[index].cells {
                cell.color[0] = cell.color[0].wrapping_add(7);
            }
            let references: Vec<_> = terrains.iter().collect();
            refresh_terrain_colours(&mut mesh, &references, &mut revision, &mut pages, &[index]);
            assert_eq!(mesh, build_stitched(&terrains));
            for (index, (old, new)) in before.iter().zip(&mesh.vertices).enumerate() {
                assert_eq!(old.pos, new.pos);
                assert_eq!(old.normal, new.normal);
                if old != new {
                    assert!(pages[index / 256] > previous);
                }
            }
            assert_eq!(
                terrains
                    .iter()
                    .map(Terrain::build_height_grid)
                    .collect::<Vec<_>>(),
                height_grids,
            );
        }
    }
}

#[test]
fn scoped_colour_refresh_preserves_outer_seams_and_skips_distant_vertices() {
    let mut terrains: Vec<_> = (12..=19)
        .map(|x| terrain(x, (x - 12) as i8, 0xFF11_2200 + u32::from(x)))
        .collect();
    let mut mesh = build_stitched(&terrains);
    let mut revision = 11;
    let mut pages = vec![11; mesh.vertices.len().div_ceil(256)];
    for cell in &mut terrains[3].cells {
        cell.color = 0xFF88_7766u32.to_le_bytes();
    }
    let references: Vec<_> = terrains.iter().collect();
    let ranges = terrain_boundary_ranges(&references, &[3]);
    assert_eq!(
        ranges,
        vec![
            2 * 16384..3 * 16384,
            3 * 16384..4 * 16384,
            4 * 16384..5 * 16384
        ]
    );
    let before = mesh.vertices.clone();
    refresh_terrain_colours(&mut mesh, &references, &mut revision, &mut pages, &[3]);
    assert_eq!(mesh, build_stitched(&terrains));
    for range in [0..2 * 16384, 5 * 16384..8 * 16384] {
        assert_eq!(mesh.vertices[range.clone()], before[range.clone()]);
        assert!(
            pages[range.start / 256..range.end / 256]
                .iter()
                .all(|&v| v == 11)
        );
    }
}

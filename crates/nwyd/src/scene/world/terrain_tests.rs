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

    stitch_terrain_boundaries(&mut mesh, &mut revision, &mut pages);

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

    stitch_terrain_boundaries(&mut mesh, &mut revision, &mut pages);

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

    stitch_terrain_boundaries(&mut mesh, &mut revision, &mut pages);

    let previous_revision = revision;

    let geometry: Vec<_> = mesh.vertices.iter().map(|v| (v.pos, v.normal)).collect();

    let bounds: Vec<_> = mesh.batches.iter().map(|batch| batch.bounds).collect();

    for cell in &mut east.cells {
        cell.color = 0xFF77_8899u32.to_le_bytes();
    }

    refresh_terrain_colours(&mut mesh, &[&west, &east], &mut revision, &mut pages);

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

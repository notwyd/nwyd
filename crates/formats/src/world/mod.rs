pub mod attribute_map;
pub mod collision;
pub mod env_texture_list;
pub mod gate_stamp;
pub mod height_map;
pub mod minimap;
pub mod object_bin;
pub mod objects;
pub mod path;
pub mod trn;
pub mod uv;

pub use height_map::HeightMap;
pub use minimap::Minimap;

pub const WORLD_CELLS: usize = 4096;

pub const ZONE_GRID: usize = 32;

pub const ZONE_CELLS: usize = 128;

pub const TRN_DIM: usize = 64;

pub const QUAD_SIZE: f32 = 2.0;

pub const HEIGHT_SCALE: f32 = 0.1;

pub const IMPASSABLE: u8 = 127;

pub fn zone_origin(zone_x: u32, zone_y: u32) -> (f32, f32) {
    (
        zone_x as f32 * ZONE_CELLS as f32,
        zone_y as f32 * ZONE_CELLS as f32,
    )
}

pub fn sector_key(world_x: f32, world_z: f32) -> u32 {
    let x = world_x as i32;

    let z = world_z as i32;

    (((z >> 5) as u32) << 16) | ((x >> 5) as u32 & 0xFFFF)
}

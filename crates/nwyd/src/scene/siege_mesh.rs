use glam::Vec3;
use render::camera::Matrix4;

pub fn mesh(main_part: i16) -> Option<u32> {
    match main_part as u16 {
        2888 => Some(2888),
        2889 => Some(2889),
        _ => None,
    }
}

pub fn yaw(position: Vec3) -> f32 {
    let bits = match (position.x as i32, position.z as i32) {
        (1224, 1975) | (1225, 1976) => 0xbdb2b8c2,
        (1216, 1968) | (1217, 1969) => 0x40a78d36,
        (1207, 1988) | (1206, 1987) => 0x40b2b8c2,
        (1189, 1955) | (1188, 1954) => 0x409c61aa,
        (1082, 2126) | (1081, 2125) => 0x3ff5be0b,
        (1074, 2119) | (1073, 2118) => 0x403de44e,
        (1108, 2142) | (1107, 2141) => 0x3fdf66f3,
        (1095, 2107) | (1094, 2106) => 0x401c61aa,
        (1060, 2090) | (1059, 2089) => 0x40543b67,
        _ => 0,
    };

    f32::from_bits(bits)
}

pub fn world(mesh: u32, position: Vec3) -> Matrix4 {
    let scale = if mesh == 2888 { 3.0 } else { 1.0 };

    let rotation = super::objects::world_at(Vec3::ZERO, yaw(position), 0.0, 0.0);

    let scaled = Matrix4::multiply(&rotation, &Matrix4::scaling(scale, scale, scale));

    Matrix4::multiply(
        &scaled,
        &Matrix4::translation(position.x, position.y, position.z),
    )
}

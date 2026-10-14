use formats::world::minimap::Label;

pub const MARKER: (f32, f32) = (4.0, 4.0);

pub const LABEL: (f32, f32) = (8.0, 12.0);

pub const LABEL_KEY: u32 = 0x0800_0000;

pub const LABEL_SHADOW: u32 = 0x7777_7777;

pub const GENERIC_CONTROL_ID: i32 = -2;

pub const CENTRE: f64 = 78.0;

pub const SCALE: f64 = 2.0;

pub const DIAGONAL: f64 = 0.7071070075035095;

pub const WIDE_SHIFT: f64 = 128.0;

pub const BOUND_NEAR: i32 = 130;

pub const BOUND_WIDE: i32 = 400;

pub const LABEL_MARGIN_X: i32 = 60;

pub const LABEL_MARGIN_Y: i32 = 10;

pub const LABEL_ZOOM: f32 = 1.0;

pub const COMPASS_NAME_ID: u32 = 0x123;

pub const COMPASS_CYCLED: (f32, f32) = (68.0, 68.0);

pub const COMPASS_CYCLED_WIDE: (f32, f32) = (200.0, 200.0);

pub const COMPASS_SIZE: (f32, f32) = (16.0, 16.0);

pub const COMPASS_OFFSET: f64 = 2.356194496154785;

pub const TITLE_PANEL_NAME_ID: u32 = 0x17F8;

pub const TITLE_TEXT_NAME_ID: u32 = 0x17F9;

pub const COORD_NAME_ID: u32 = 0x303;

pub const TEXT_COLOR: u32 = 0xFFFF_FFAA;

pub const TEXT_BOX: (i32, i32) = (80, 16);

pub const COORD_CYCLED: (f32, f32) = (13.0, 120.0);

pub const COORD_CYCLED_WIDE: (f32, f32) = (10.0, 0.0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub slot: usize,
    pub x: f32,
    pub y: f32,
    pub color: u32,
    pub marker: bool,
    pub label: bool,
}

impl Placed {
    pub fn label_at(&self, of: &Label) -> (f32, f32) {
        (of.ofs_x as f32 + self.x, of.ofs_y as f32 + self.y)
    }
}

pub fn bound(zoom: f32) -> i32 {
    if zoom < LABEL_ZOOM {
        BOUND_NEAR
    } else {
        BOUND_WIDE
    }
}

pub fn project(zoom: f32, player: glam::Vec3, label: (i32, i32)) -> (f32, f32) {
    let dx = f64::from(player.x - label.0 as f32);

    let dz = f64::from(player.z - label.1 as f32);

    let shift = f64::from(zoom as i32) * WIDE_SHIFT;

    let across = (dx * DIAGONAL - dz * DIAGONAL) * SCALE;

    let along = (dx * DIAGONAL + dz * DIAGONAL) * SCALE;

    (
        (CENTRE - across + shift) as f32,
        (CENTRE + along + shift) as f32,
    )
}

pub fn marker_visible(bound: i32, x: f32, y: f32) -> bool {
    let b = bound as f32;

    (0.0..=b).contains(&x) && (0.0..=b).contains(&y)
}

pub fn label_visible(zoom: f32, bound: i32, x: f32, y: f32, ofs: (i32, i32)) -> bool {
    let lx = ofs.0 as f32 + x;

    let ly = ofs.1 as f32 + y;

    lx >= 0.0
        && lx <= (bound - LABEL_MARGIN_X) as f32
        && ly >= 0.0
        && ly <= (bound - LABEL_MARGIN_Y) as f32
        && zoom >= LABEL_ZOOM
}

pub fn place(labels: &[Label], zoom: f32, player: glam::Vec3) -> Vec<Placed> {
    let bound = bound(zoom);

    labels
        .iter()
        .enumerate()
        .map(|(slot, label)| {
            let (x, y) = project(zoom, player, (label.x, label.y));

            Placed {
                slot,
                x,
                y,
                color: label.color,
                marker: marker_visible(bound, x, y),
                label: label_visible(zoom, bound, x, y, (label.ofs_x, label.ofs_y)),
            }
        })
        .collect()
}

pub fn compass_rotation(player_yaw: f32) -> f32 {
    (f64::from(player_yaw) - COMPASS_OFFSET) as f32
}

pub fn coordinate_text(player: glam::Vec3) -> String {
    format!("X: {:4}  Y: {:4}", player.x as i32, player.z as i32)
}

pub fn title_text(server: &str, channel: i32) -> String {
    format!("{server}-{channel}")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    pub server: usize,
    pub channel: i32,
    pub names: Vec<String>,
}

impl Session {
    pub fn name(&self) -> &str {
        self.names.get(self.server).map_or("", String::as_str)
    }

    pub fn title(&self) -> String {
        title_text(self.name(), self.channel)
    }

    pub fn adopt(&mut self, server: usize, channel: i32) {
        self.server = server;

        self.channel = channel;
    }
}

use glam::{Mat4, Vec3};
use render::camera::Matrix4;

pub const PICK_VOLUME: [(f32, f32); 100] = {
    let mut v = [(0.0, 0.0); 100];

    v[0] = (0.4, 2.0);

    v[1] = (0.4, 2.0);

    v[2] = (0.5, 2.0);

    v[3] = (0.4, 2.0);

    v[4] = (0.5, 2.0);

    v[5] = (0.4, 2.5);

    v[6] = (0.7, 1.0);

    v[7] = (1.0, 2.0);

    v[8] = (0.7, 2.0);

    v[9] = (0.7, 3.2);

    v[10] = (0.7, 2.0);

    v[11] = (1.0, 2.5);

    v[12] = (0.4, 2.0);

    v[13] = (1.0, 1.0);

    v[14] = (1.0, 1.0);

    v[15] = (1.0, 1.0);

    v[16] = (1.0, 1.0);

    v[17] = (1.0, 1.0);

    v[18] = (1.0, 1.0);

    v[19] = (1.0, 1.0);

    v[20] = (1.2, 2.2);

    v[21] = (0.45, 0.9);

    v[22] = (0.4, 1.3);

    v[23] = (2.5, 1.5);

    v[24] = (0.4, 1.0);

    v[25] = (0.7, 1.0);

    v[26] = (0.6, 2.5);

    v[27] = (0.8, 1.0);

    v[28] = (0.7, 1.0);

    v[29] = (0.7, 1.0);

    v[30] = (1.4, 1.2);

    v[31] = (1.8, 1.5);

    v[32] = (0.5, 0.2);

    v[33] = (1.0, 2.5);

    v[34] = (2.0, 2.0);

    v[35] = (1.8, 1.8);

    v[36] = (2.0, 1.8);

    v[37] = (1.0, 1.0);

    v[38] = (1.0, 1.8);

    v[39] = (1.2, 2.2);

    v[40] = (0.8, 1.0);

    v[41] = (2.9, 2.5);

    v[42] = (1.4, 3.0);

    v[43] = (1.4, 3.2);

    v[44] = (0.4, 3.5);

    v[45] = (0.4, 3.5);

    v[46] = (0.4, 2.1);

    v[47] = (0.4, 2.8);

    v[48] = (0.4, 2.5);

    v[49] = (0.4, 2.0);

    v[50] = (0.4, 2.0);

    v[51] = (0.4, 2.0);

    v[52] = (0.4, 1.0);

    v[53] = (1.0, 1.0);

    v[54] = (1.0, 1.0);

    v[55] = (1.0, 1.0);

    v[56] = (1.0, 1.0);

    v[57] = (1.0, 1.0);

    v[58] = (1.0, 1.0);

    v[59] = (1.8, 1.8);

    v
};

pub const GROUND_MARGIN: f32 = 0.5;

pub const BILLBOARD_INSET: f32 = 0.2;

pub const SIEGE_MESHES: [(u16, f32); 2] = [(0xB48, 3.0), (0xB49, 4.0)];

pub const MESH_IDS: std::ops::RangeInclusive<u16> = 1..=0xDAB;

pub fn siege_radius(weapon: u16) -> Option<f32> {
    if !MESH_IDS.contains(&weapon) {
        return None;
    }

    SIEGE_MESHES
        .iter()
        .find(|(id, _)| *id == weapon)
        .map(|(_, r)| *r)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub position: Vec3,
    pub model: u32,
    pub scale: f32,
    pub weapon: u16,
    pub billboard: bool,
}

impl Body {
    pub fn new(position: Vec3, model: u32, scale: f32) -> Body {
        Body {
            position,
            model,
            scale,
            weapon: 0,
            billboard: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn unproject(
        view: &Matrix4,
        projection: &Matrix4,
        mouse: (i32, i32),
        viewport: (u32, u32),
    ) -> Ray {
        let (w, h) = (viewport.0 as f32, viewport.1 as f32);

        let sx = ((mouse.0 as f32 * 2.0) / w - 1.0) / projection.0[0];

        let sy = -(((mouse.1 as f32 * 2.0) / h - 1.0) / projection.0[5]);

        let inv = Mat4::from_cols_array(&view.0).inverse();

        Ray {
            origin: inv.w_axis.truncate(),
            direction: (inv.x_axis * sx + inv.y_axis * sy + inv.z_axis).truncate(),
        }
    }
}

pub fn intersect_tri(ray: &Ray, v0: Vec3, v1: Vec3, v2: Vec3) -> bool {
    let e1 = v1 - v0;

    let e2 = v2 - v0;

    let p = ray.direction.cross(e2);

    let det = e1.dot(p);

    if det == 0.0 {
        return false;
    }

    let t = ray.origin - v0;

    let u = t.dot(p) / det;

    if !(0.0..=1.0).contains(&u) {
        return false;
    }

    let q = t.cross(e1);

    let v = ray.direction.dot(q) / det;

    v >= 0.0 && u + v <= 1.0
}

pub fn hits_quad(ray: &Ray, a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    intersect_tri(ray, a, b, c) || intersect_tri(ray, b, d, c)
}

pub fn hits(ray: &Ray, body: &Body) -> bool {
    let Some(&(radius, height)) = PICK_VOLUME.get(body.model as usize) else {
        return false;
    };

    let (radius, height) = if body.model == 55 {
        (0.45, 0.45)
    } else {
        (radius, height)
    };

    let (x, y, z) = (body.position.x, body.position.y, body.position.z);

    let top = y + height * body.scale;

    let mut margin = radius * body.scale + GROUND_MARGIN;

    if let Some(siege) = siege_radius(body.weapon) {
        margin = siege;

        if hits_quad(
            ray,
            Vec3::new(x - margin, y, z - margin),
            Vec3::new(x + margin, y, z - margin),
            Vec3::new(x - margin, y, z + margin),
            Vec3::new(x + margin, y, z + margin),
        ) {
            return true;
        }
    }

    let r = radius * body.scale;

    if body.billboard {
        let m = margin - BILLBOARD_INSET;

        if hits_quad(
            ray,
            Vec3::new(x - m, top, z),
            Vec3::new(x - m, y, z),
            Vec3::new(x + m, top, z),
            Vec3::new(x + m, y, z),
        ) {
            return true;
        }
    }

    let (a, b) = (Vec3::new(x - r, top, z - r), Vec3::new(x + r, top, z - r));

    let (c, d) = (Vec3::new(x + r, y, z + r), Vec3::new(x - r, y, z + r));

    if hits_quad(
        ray,
        a,
        Vec3::new(x + r, y, z - r),
        Vec3::new(x - r, top, z + r),
        c,
    ) || intersect_tri(ray, a, b, c)
        || intersect_tri(ray, c, d, a)
    {
        return true;
    }

    let (x0, x1) = (x - r, x + r);

    let (z0, z1) = (z - r, z + r);

    let corner = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);

    let (ntl, ntr) = (corner(x0, top, z0), corner(x1, top, z0));

    let (ftl, ftr) = (corner(x0, top, z1), corner(x1, top, z1));

    let (nbl, nbr) = (corner(x0, y, z0), corner(x1, y, z0));

    let (fbl, fbr) = (corner(x0, y, z1), corner(x1, y, z1));

    if intersect_tri(ray, ftl, ntl, nbl)
        || intersect_tri(ray, nbl, fbl, ftl)
        || intersect_tri(ray, ntl, ntr, nbr)
        || intersect_tri(ray, nbr, nbl, ntl)
        || intersect_tri(ray, ntr, ftr, fbr)
        || intersect_tri(ray, fbr, nbr, ntr)
        || intersect_tri(ray, ftr, ftl, fbl)
        || intersect_tri(ray, fbl, fbr, ftr)
    {
        return true;
    }

    body.billboard
        && hits_quad(
            ray,
            Vec3::new(x - r, y, z - r),
            Vec3::new(x + r, y, z - r),
            Vec3::new(x - r, y, z + r),
            Vec3::new(x + r, y, z + r),
        )
}

use formats::mesh::appearance::PartRef;
use formats::world::objects::{ObjRecord, ObjectClass};
use glam::Vec3;
use render::skin::{SkinFog, SkinParams};

#[derive(Debug, Clone, PartialEq)]
pub struct Ship {
    pub zone: (u8, u8),
    pub record: usize,
    pub kind: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub model: Option<usize>,
}

impl Ship {
    pub fn part_zero(&self) -> PartRef {
        PartRef {
            mesh: (self.kind - 0x1E7) as i16,
            tex: 0,
        }
    }

    pub fn params(&self, fog: SkinFog) -> SkinParams {
        SkinParams {
            diffuse: [0.7, 0.7, 0.7, 1.0],
            ambient: [0.0; 4],
            emissive: [0.3, 0.3, 0.3, 0.0],
            unlit: false,
            fog_start: fog.start,
            fog_end: fog.end,
            fog_enabled: fog.enabled,
            force_opaque: false,
            modulate_alpha: false,
            glow: None,
        }
    }
}

pub fn build(records: &[ObjRecord], zone: (u8, u8)) -> Vec<Ship> {
    records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.class() == ObjectClass::Ship)
        .map(|(record, r)| Ship {
            zone,
            record,
            kind: r.kind,
            position: Vec3::from_array(r.world_position(zone.0.into(), zone.1.into())),
            yaw: r.yaw + std::f32::consts::FRAC_PI_2,
            model: None,
        })
        .collect()
}

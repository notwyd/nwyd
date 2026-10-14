use formats::net::field_fx::{Carpet, Sound};
use formats::world::trn::Terrain;
use glam::Vec3;

pub const CARPET_MODE: i32 = 0;

#[derive(Debug, Clone, PartialEq)]
pub enum Carried {
    Carpet {
        cells: Vec<Vec3>,
        built: bool,
        raised: usize,
    },
    Sound {
        slot: i64,
    },
}

impl Carried {
    pub fn raised(&self) -> usize {
        match self {
            Carried::Carpet { raised, .. } => *raised,
            Carried::Sound { .. } => 0,
        }
    }
}

pub fn carpet(relief: &[i8], terrain: &Terrain, packet: &Carpet) -> Carried {
    let cells = packet
        .cells()
        .map(|cell| {
            let (x, z) = Carpet::cell_position(cell);

            Vec3::new(x, super::actors::ground_height(relief, terrain, x, z), z)
        })
        .collect();

    Carried::Carpet {
        cells,
        built: packet.builds(),
        raised: 0,
    }
}

pub fn raise(scene: &mut super::field::FieldScene, packet: &Carpet) -> Carried {
    let carried = carpet(&scene.world.actors.relief, &scene.world.terrain, packet);

    let Carried::Carpet { cells, built, .. } = carried else {
        return carried;
    };

    if !built {
        return Carried::Carpet {
            cells,
            built,
            raised: 0,
        };
    }

    let now_ms = scene.time.now_ms;

    let ground = {
        let heights = &scene.world.actors.heights;

        let mut zones: Vec<&Terrain> = vec![&scene.world.terrain];

        zones.extend(scene.world.streamed.iter().map(|s| &s.terrain));

        super::shade::Ground::new(&scene.world.terrain, heights, zones)
    };

    let mut raised = 0;

    for &at in &cells {
        let mut boom = super::fire_ring::FireRing::new(
            at,
            CARPET_MODE,
            formats::net::field_fx::RADIUS,
            formats::net::field_fx::INTERVAL_MS as i32,
            formats::net::field_fx::TINT,
            now_ms,
            Some(&ground),
        );

        for child in super::effect_tree::Effect::take_spawned(&mut boom) {
            scene.world.effects.spawn(child);

            raised += 1;
        }

        scene.world.effects.spawn(Box::new(boom));

        raised += 1;
    }

    Carried::Carpet {
        cells,
        built,
        raised,
    }
}

pub fn sound(packet: &Sound) -> Carried {
    Carried::Sound {
        slot: i64::from(packet.slot),
    }
}

pub fn describe(carried: &Carried) -> String {
    match carried {
        Carried::Carpet {
            cells,
            built,
            raised,
        } => {
            format!(
                "0x3a2 explosion carpet — {} cell(s), {}",
                cells.len(),
                match built {
                    true => format!(
                        "{raised} node(s) raised — a SkillFireRing and its Shade \
                    on each"
                    ),
                    false =>
                        "kind is not 0x20, so the ground was read and nothing built".to_string(),
                },
            )
        }
        Carried::Sound { slot } => {
            format!("0x3a3 field sound — slot {slot} queued play-once")
        }
    }
}

use crate::error::{Error, Kind, Result};

pub const ROSTER_RECORD_LEN: usize = 60;

pub const ENDING_RECORD_LEN: usize = 48;

pub const APPEARANCE_EFFECT: i16 = 0x12;

#[derive(Debug, Clone, PartialEq)]

pub struct Actor {
    pub face: i32,
    pub helmet: i32,
    pub armour: i32,
    pub cape: i32,
    pub left_hand: i32,
    pub right_hand: i32,
    pub equipment_color: i32,
    pub mount: i32,
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub yaw_deg: i32,
    pub tail: Vec<i32>,
    pub raw: Vec<i32>,
}

impl Actor {
    pub fn equipment(&self) -> [i32; 5] {
        [
            self.helmet,
            self.armour,
            self.cape,
            self.left_hand,
            self.right_hand,
        ]
    }

    pub fn is_bare(&self) -> bool {
        self.equipment() == [0; 5] && self.mount == 0
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct Roster {
    pub actors: Vec<Actor>,
    pub layout: Layout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Layout {
    Roster,
    Ending,
}

impl Layout {
    pub fn record_len(self) -> usize {
        match self {
            Layout::Roster => ROSTER_RECORD_LEN,
            Layout::Ending => ENDING_RECORD_LEN,
        }
    }

    pub fn for_file(file_name: &str) -> Layout {
        if file_name.eq_ignore_ascii_case("EndDemo.bin") {
            Layout::Ending
        } else {
            Layout::Roster
        }
    }
}

fn i32_at(rec: &[u8], word: usize) -> i32 {
    i32::from_le_bytes(rec[word * 4..word * 4 + 4].try_into().expect("4 bytes"))
}

fn f32_at(rec: &[u8], word: usize) -> f32 {
    f32::from_le_bytes(rec[word * 4..word * 4 + 4].try_into().expect("4 bytes"))
}

pub fn parse(bytes: &[u8], layout: Layout) -> Result<Roster> {
    let stride = layout.record_len();

    if !bytes.len().is_multiple_of(stride) {
        return Err(Error::structure(
            Kind::DemoRoster,
            format!(
                "length {} is not a multiple of the {stride}-byte {layout:?} record",
                bytes.len()
            ),
        ));
    }

    let actors = bytes
        .chunks_exact(stride)
        .map(|rec| {
            let raw: Vec<i32> = (0..stride / 4).map(|w| i32_at(rec, w)).collect();

            let (from, to, yaw_deg, tail) = match layout {
                Layout::Roster => (
                    (f32_at(rec, 8), f32_at(rec, 9)),
                    (f32_at(rec, 10), f32_at(rec, 11)),
                    raw[12],
                    raw[13..15].to_vec(),
                ),
                Layout::Ending => {
                    let at = (raw[8] as f32, raw[9] as f32);

                    (at, at, raw[10], raw[11..12].to_vec())
                }
            };

            Actor {
                face: raw[0],
                helmet: raw[1],
                armour: raw[2],
                cape: raw[3],
                left_hand: raw[4],
                right_hand: raw[5],
                equipment_color: raw[6],
                mount: raw[7],
                from,
                to,
                yaw_deg,
                tail,
                raw,
            }
        })
        .collect();

    Ok(Roster { actors, layout })
}

impl Roster {
    pub fn len(&self) -> usize {
        self.actors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    pub fn equipment_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .actors
            .iter()
            .flat_map(|a| a.equipment())
            .filter(|&id| id != 0)
            .collect();

        ids.sort_unstable();

        ids.dedup();

        ids
    }

    pub fn face_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .actors
            .iter()
            .map(|a| a.face)
            .filter(|&id| id != 0)
            .collect();

        ids.sort_unstable();

        ids.dedup();

        ids
    }

    pub fn mount_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .actors
            .iter()
            .map(|a| a.mount)
            .filter(|&id| id != 0)
            .collect();

        ids.sort_unstable();

        ids.dedup();

        ids
    }
}

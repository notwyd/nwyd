use super::item_move;
use super::rng::Lcg;
use super::screens::Quest;
use ui::Strings;

pub const AIR_MOVE: u16 = 0x0AD9;

pub const AIR_MOVE_LEN: usize = 0x14;

pub const PHASE_DEPART: u32 = 1;

pub const PHASE_ARRIVE: u32 = 2;

pub const ARRIVE_INDEX_MAX: u32 = 10;

pub const AIR_MOVE_ROWS: usize = 5;

pub const AIR_MOVE_DESTS: [(i32, i32); AIR_MOVE_ROWS] = [
    (2112, 2051),
    (2372, 2099),
    (2220, 1714),
    (2365, 2279),
    (1826, 1771),
];

pub const AIR_MOVE_ROUTES: [&[(i32, i32)]; AIR_MOVE_ROWS] = [
    &[(2112, 2051)],
    &[(2372, 2099)],
    &[(2485, 2097), (2485, 1712), (2220, 1714)],
    &[(2356, 2112), (2365, 2279)],
    &[
        (2485, 2097),
        (2485, 1712),
        (2220, 1712),
        (2220, 1593),
        (1982, 1593),
        (1982, 1734),
        (1826, 1771),
    ],
];

pub const AIR_MOVE_VERTICAL_MS: u32 = 0x0AF0;

pub const AIR_MOVE_STEP_START: f32 = f32::from_bits(0x3E4C_CCCD);

pub const AIR_MOVE_STEP_HIGH: f32 = f32::from_bits(0x3F33_3333);

pub const AIR_MOVE_STEP_LOW: f32 = AIR_MOVE_STEP_START;

pub const AIR_MOVE_ACCEL: f64 = 0.005f32 as f64;

pub const AIR_MOVE_BRAKE: f64 = 0.0023f32 as f64;

pub const AIR_MOVE_FAR: f64 = 70.0;

pub const AIR_MOVE_BOB_BASE_MS: u32 = 1000;

pub const AIR_MOVE_BOB_RANDOM_DIVISOR: f64 = 8000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirMoveStage {
    Rise,
    Route,
    Descend,
    Teardown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirMovePose {
    pub position: [f32; 3],
    pub ground_y: f32,
    pub draw_offset: [f32; 2],
    pub yaw: f32,
    pub mount_frame: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirMoveTick {
    Active,
    Teardown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirMoveFlight {
    stage: AirMoveStage,
    stamp_ms: u32,
    bob_down: bool,
    target: [f32; 2],
    step: f32,
    row: usize,
    waypoint: usize,
}

impl AirMoveFlight {
    pub fn start(row: usize, now_ms: u32) -> Option<AirMoveFlight> {
        let target = AIR_MOVE_ROUTES.get(row)?.first().copied()?;

        Some(AirMoveFlight {
            stage: AirMoveStage::Rise,
            stamp_ms: now_ms,
            bob_down: false,
            target: [target.0 as f32, target.1 as f32],
            step: AIR_MOVE_STEP_START,
            row,
            waypoint: 0,
        })
    }

    pub fn stage(&self) -> AirMoveStage {
        self.stage
    }

    pub fn row(&self) -> usize {
        self.row
    }

    pub fn waypoint(&self) -> usize {
        self.waypoint
    }

    pub fn target(&self) -> [f32; 2] {
        self.target
    }

    pub fn step_size(&self) -> f32 {
        self.step
    }

    pub fn bob_down(&self) -> bool {
        self.bob_down
    }

    pub fn tick(&mut self, now_ms: u32, pose: &mut AirMovePose, rng: &mut Lcg) -> AirMoveTick {
        match self.stage {
            AirMoveStage::Rise => self.rise(now_ms, pose),
            AirMoveStage::Route => self.route(now_ms, pose, rng),
            AirMoveStage::Descend => self.descend(now_ms, pose),
            AirMoveStage::Teardown => AirMoveTick::Teardown,
        }
    }

    fn rise(&mut self, now_ms: u32, pose: &mut AirMovePose) -> AirMoveTick {
        if now_ms > self.stamp_ms.wrapping_add(AIR_MOVE_VERTICAL_MS) {
            self.stage = AirMoveStage::Route;

            self.stamp_ms = now_ms;

            return AirMoveTick::Active;
        }

        let lift = vertical_wave(now_ms.wrapping_sub(self.stamp_ms)).clamp(0.0, 0.3);

        pose.mount_frame = pose.mount_frame.map(|_| 15);

        pose.position[1] += lift;

        AirMoveTick::Active
    }

    fn route(&mut self, now_ms: u32, pose: &mut AirMovePose, rng: &mut Lcg) -> AirMoveTick {
        let random = rng.next() % 300;

        let bob_deadline = self
            .stamp_ms
            .wrapping_add(AIR_MOVE_BOB_BASE_MS)
            .wrapping_add(random);

        if now_ms >= bob_deadline {
            self.stamp_ms = now_ms;

            self.bob_down = !self.bob_down;
        }

        let bob = bob_wave(now_ms.wrapping_sub(self.stamp_ms), random);

        if !self.bob_down && pose.position[1] < 14.0 {
            pose.position[1] += bob;
        } else if pose.position[1] > 8.0 {
            pose.position[1] -= bob;
        }

        let target = AIR_MOVE_ROUTES[self.row][self.waypoint];

        let target = [target.0 as f32, target.1 as f32];

        let mut dx = axis_step(pose.position[0], target[0], self.step);

        let mut dz = axis_step(pose.position[2], target[1], self.step);

        let residual_x = target[0] - (pose.position[0] + dx);

        let residual_z = target[1] - (pose.position[2] + dz);

        if residual_x > -0.5 && residual_x < 0.5 {
            dx = 0.0;
        }

        if residual_z > -0.5 && residual_z < 0.5 {
            dz = 0.0;
        }

        if self.step == dx.abs() && self.step == dz.abs() {
            dx *= 0.5;

            dz *= 0.5;
        }

        if dx == 0.0 && dz == 0.0 {
            self.waypoint += 1;

            match AIR_MOVE_ROUTES[self.row].get(self.waypoint).copied() {
                Some(next) => {
                    self.target = [next.0 as f32, next.1 as f32];

                    self.stamp_ms = now_ms;
                }
                None => {
                    self.stage = AirMoveStage::Descend;

                    self.stamp_ms = now_ms;
                }
            }

            return AirMoveTick::Active;
        }

        pose.draw_offset[0] += dx;

        pose.draw_offset[1] += dz;

        let manhattan = f64::from((target[0] - pose.position[0]).abs())
            + f64::from((target[1] - pose.position[2]).abs());

        let has_next =
            self.waypoint < 9 && AIR_MOVE_ROUTES[self.row].get(self.waypoint + 1).is_some();

        if manhattan >= AIR_MOVE_FAR || has_next {
            if self.step <= AIR_MOVE_STEP_HIGH {
                self.step = speed_up(self.step);
            }
        } else if self.step >= AIR_MOVE_STEP_LOW {
            self.step = speed_down(self.step);
        }

        pose.mount_frame = pose
            .mount_frame
            .map(|_| (self.step * -2.0).trunc() as i32 + 15);

        pose.yaw = dz.atan2(dx) + std::f32::consts::FRAC_PI_2;

        AirMoveTick::Active
    }

    fn descend(&mut self, now_ms: u32, pose: &mut AirMovePose) -> AirMoveTick {
        if now_ms > self.stamp_ms.wrapping_add(AIR_MOVE_VERTICAL_MS) {
            pose.position[1] = pose.ground_y;

            self.stage = AirMoveStage::Teardown;

            self.stamp_ms = now_ms;

            return AirMoveTick::Active;
        }

        let drop = vertical_wave(now_ms.wrapping_sub(self.stamp_ms));

        pose.mount_frame = pose.mount_frame.map(|_| 15);

        if pose.position[1] - drop > 0.0 {
            pose.position[1] -= drop;
        }

        AirMoveTick::Active
    }
}

fn axis_step(position: f32, target: f32, step: f32) -> f32 {
    if position < target {
        step
    } else if target < position {
        -step
    } else {
        0.0
    }
}

fn vertical_wave(age_ms: u32) -> f32 {
    let scaled = (f64::from(age_ms) / 15_000.0) as f32;

    sin(((f64::from(scaled) * super::motion::PI) / 6.0) as f32)
}

fn bob_wave(age_ms: u32, random: u32) -> f32 {
    let scaled = (f64::from(age_ms) / 10_000.0) as f32;

    let wave = sin(((f64::from(scaled) * super::motion::PI) / 6.0) as f32);

    (f64::from(wave) + f64::from(random) / AIR_MOVE_BOB_RANDOM_DIVISOR) as f32
}

fn speed_up(step: f32) -> f32 {
    let theta = (f64::from(step) * AIR_MOVE_ACCEL * super::motion::PI) as f32;

    (f64::from(step) + f64::from(sin(theta))) as f32
}

fn speed_down(step: f32) -> f32 {
    let theta = (f64::from(step) * AIR_MOVE_BRAKE * super::motion::PI) as f32;

    (f64::from(step) - f64::from(sin(theta))) as f32
}

fn sin(theta: f32) -> f32 {
    f64::from(theta).sin() as f32
}

pub const AIR_MOVE_QUEST_TAB: usize = 0;

pub const AIR_MOVE_QUEST_ROWS: [usize; 3] = [0, 1, 6];

pub const AIR_MOVE_TAIL_STRDEF: [usize; 2] = [212, 218];

pub const AIR_MOVE_LABELS: [(&str, usize); 3] = [("Text0", 378), ("Text1", 380), ("Text2", 379)];

pub const AIR_MOVE_HEADER_COLOR: u32 = 0xFFFF_FFFF;

pub const SCROLL_LABELS: [(&str, usize); 3] = [("Text0", 382), ("Text1", 383), ("Text2", 384)];

pub const SCROLL_ROW_COLOR: u32 = 0xFFFF_FFFF;

pub const ROW_NAME: &str = "Name";

pub const ROW_POS: &str = "Pos";

pub const ROW_MSG: &str = "Msg";

pub const LIST: &str = "List";

pub const OK: &str = "Ok";

pub const CLOSE: &str = "Close";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    AirMove,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Portal {
    pub mode: Option<Mode>,
    pub departed: Option<u32>,
    pub staged: Option<crate::scene::item_move::Request>,
    pub page: Option<i32>,
    pub destination: u16,
}

pub const STAGED_WORDS: usize = 9;

pub fn move_request(client_id: u16) -> Vec<u8> {
    item_move::Request::Drink.to_bytes(client_id, item_move::ANTI_CHEAT_TOKEN)
}

pub fn frame(index: u32, phase: u32) -> Vec<u8> {
    let mut out = vec![0u8; AIR_MOVE_LEN];

    protocol::header::set_opcode(&mut out, AIR_MOVE);

    let at = protocol::header::LEN;

    out[at..at + 4].copy_from_slice(&index.to_le_bytes());

    out[at + 4..at + 8].copy_from_slice(&phase.to_le_bytes());

    out
}

pub fn depart(index: u32) -> Vec<u8> {
    frame(index, PHASE_DEPART)
}

pub fn arrive(index: u32) -> Vec<u8> {
    frame(
        if index > ARRIVE_INDEX_MAX { 0 } else { index },
        PHASE_ARRIVE,
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub pos: String,
}

pub fn air_move_rows(quests: &[Vec<Quest>], strings: &Strings) -> Vec<Row> {
    let tab = quests.get(AIR_MOVE_QUEST_TAB);

    let mut rows = Vec::with_capacity(AIR_MOVE_ROWS);

    for (slot, record) in AIR_MOVE_QUEST_ROWS.iter().enumerate() {
        let quest = tab.and_then(|tab| tab.get(*record));

        rows.push(Row {
            name: match quest {
                Some(quest) if quest.level != 0 => {
                    format!("{}{}", level_prefix(slot, quest.level), quest.title)
                }
                Some(quest) => quest.title.clone(),
                None => String::new(),
            },
            pos: String::new(),
        });
    }

    for record in AIR_MOVE_TAIL_STRDEF {
        rows.push(Row {
            name: strings.translate(&format!("ss_{record}")),
            pos: String::new(),
        });
    }

    for (row, (x, z)) in rows.iter_mut().zip(AIR_MOVE_DESTS) {
        row.pos = format!("{x:04}  {z:04}");
    }

    rows
}

fn level_prefix(slot: usize, level: i32) -> String {
    match slot {
        0 => format!("Lv   {level:3} "),
        1 => format!("Lv  {level:3} "),
        _ => format!("Lv {level:3} "),
    }
}

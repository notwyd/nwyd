use crate::scene::hud::InputLock;
use crate::scene::{chat, item_move, msgbox};
use formats::net::attack::{self, Attack, Hit};

pub const STAMP_SLOT: usize = 4;

pub const FIRE_MS: u32 = 5_000;

pub const CANCEL_MS: u32 = 6_000;

pub const TICK_MS: u32 = 1_000;

pub const COLUMN_KIND: i32 = 1;

pub const COLUMN_LIFT: f32 = 0.05;

pub const DEATH_STAMP_SLOT: usize = 3;

pub const RESURRECT_STAMP_SLOT: usize = 5;

pub const DEATH_RETURN_OPCODE: u16 = 0x289;

pub const DEATH_RETURN_LEN: usize = 0x0C;

pub const RESURRECT_SKILL: i16 = 0x63;

pub const DEATH_BOX_CONTEXT: u32 = 0x2A;

pub const RESURRECT_BOX_CONTEXT: u32 = 0x2E;

pub const DEATH_BOX_BODY: usize = 27;

pub const RESURRECT_BOX_BODY: usize = 227;

pub const DEATH_ACCEPT_OPCODE: u16 = 0x3AE;

pub const DEATH_ACCEPT_LEN: usize = 0x10;

pub fn death_box() -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: super::interact::TALK_BOX_KIND,
        context: DEATH_BOX_CONTEXT,
        body: msgbox::Body::StrDef(DEATH_BOX_BODY),
        flags: msgbox::FLAGS,
    }
}

pub fn resurrect_box() -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: super::interact::TALK_BOX_KIND,
        context: RESURRECT_BOX_CONTEXT,
        body: msgbox::Body::StrDef(RESURRECT_BOX_BODY),
        flags: msgbox::FLAGS,
    }
}

pub fn death_accept_frame(client_id: u16) -> Vec<u8> {
    let mut frame = vec![0u8; DEATH_ACCEPT_LEN];

    protocol::header::set_opcode(&mut frame, DEATH_ACCEPT_OPCODE);

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame[12..16].copy_from_slice(&2u32.to_le_bytes());

    frame
}

pub fn resurrect_slot(quick: &[u8; 20]) -> Option<u8> {
    quick
        .iter()
        .position(|&code| i16::from(code) == RESURRECT_SKILL)
        .map(|slot| slot as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeathPending {
    pub armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResurrectPending {
    pub armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RevivePending {
    pub death: DeathPending,
    pub resurrect: ResurrectPending,
    pub shown: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathStep {
    Nothing,
    Expired,
    Tick(u32),
    Fire,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResurrectSource {
    pub client_id: u16,
    pub position: (f32, f32),
    pub from: Option<(u16, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResurrectStep {
    Nothing,
    Expired,
    Echo(Attack),
    Fire(Vec<u8>),
}

impl RevivePending {
    pub fn arm_death(&mut self, lock: &mut InputLock, now_ms: u32) {
        lock.stamp(DEATH_STAMP_SLOT, now_ms);

        self.death.armed = true;
    }

    pub fn arm_resurrect(&mut self, lock: &mut InputLock, now_ms: u32) {
        self.resurrect.armed = true;

        lock.stamp(RESURRECT_STAMP_SLOT, now_ms);
    }

    pub fn tick_death(&mut self, lock: &mut InputLock, now_ms: u32) -> DeathStep {
        let stamp = lock.stamps[DEATH_STAMP_SLOT];

        if stamp == 0 {
            return DeathStep::Nothing;
        }

        if !self.death.armed && stamp.wrapping_add(CANCEL_MS) < now_ms {
            lock.stamps[DEATH_STAMP_SLOT] = 0;

            return DeathStep::Expired;
        }

        if self.death.armed && stamp.wrapping_add(FIRE_MS) < now_ms {
            self.resurrect.armed = false;

            self.death.armed = false;

            return DeathStep::Fire;
        }

        let quotient = death_seconds(stamp, now_ms);

        if self.shown == Some(quotient) {
            return DeathStep::Nothing;
        }

        self.shown = Some(quotient);

        DeathStep::Tick(quotient)
    }

    pub fn tick_resurrect(
        &mut self,
        lock: &mut InputLock,
        source: ResurrectSource,
        now_ms: u32,
    ) -> ResurrectStep {
        let stamp = lock.stamps[RESURRECT_STAMP_SLOT];

        if stamp == 0 {
            return ResurrectStep::Nothing;
        }

        if !self.resurrect.armed && stamp.wrapping_add(CANCEL_MS) < now_ms {
            lock.stamps[RESURRECT_STAMP_SLOT] = 0;

            return ResurrectStep::Expired;
        }

        if self.resurrect.armed && stamp.wrapping_add(FIRE_MS) < now_ms {
            self.resurrect.armed = false;

            return ResurrectStep::Fire(resurrect_frame(source));
        }

        let quotient = resurrect_seconds(stamp, now_ms);

        if self.shown == Some(quotient) {
            return ResurrectStep::Nothing;
        }

        self.shown = Some(quotient);

        ResurrectStep::Echo(resurrect_attack(source, true))
    }
}

pub fn death_seconds(stamp_ms: u32, now_ms: u32) -> u32 {
    stamp_ms.wrapping_sub(now_ms).wrapping_add(FIRE_MS) / 1_000
}

pub fn resurrect_seconds(stamp_ms: u32, now_ms: u32) -> u32 {
    stamp_ms.wrapping_sub(now_ms).wrapping_add(FIRE_MS) / 2_000
}

pub fn death_return_frame(client_id: u16) -> Vec<u8> {
    let mut frame = vec![0u8; DEATH_RETURN_LEN];

    protocol::header::set_opcode(&mut frame, DEATH_RETURN_OPCODE);

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame
}

pub fn resurrect_attack(source: ResurrectSource, echo: bool) -> Attack {
    let at = source.from.filter(|(x, _)| *x != 0).unwrap_or_else(|| {
        (
            truncate_coordinate_to_u16(source.position.0),
            truncate_coordinate_to_u16(source.position.1),
        )
    });

    let mut frame = Attack {
        from: at,
        to: at,
        attacker: source.client_id,
        skill: 0,
        action: attack::ACTION_NONE,
        echo,
        flags: 0,
        variant: 0,
        damage: attack::DAMAGE_NONE,
        skill_id: RESURRECT_SKILL,
        ..Attack::default()
    };

    frame.hits[0] = Hit {
        entity: source.client_id,
        damage: attack::DAMAGE_NONE,
    };

    frame
}

pub fn resurrect_frame(source: ResurrectSource) -> Vec<u8> {
    let mut frame = resurrect_attack(source, false).to_bytes(attack::ATTACK);

    frame[6..8].copy_from_slice(&source.client_id.to_le_bytes());

    frame
}

fn truncate_coordinate_to_u16(value: f32) -> u16 {
    let truncated = f64::from(value).trunc();

    if !truncated.is_finite() || truncated < f64::from(i32::MIN) || truncated > f64::from(i32::MAX)
    {
        0
    } else {
        truncated as i32 as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pending {
    pub armed: bool,
    pub frame: Option<item_move::Request>,
    pub server: i32,
    pub shown: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Nothing,
    Expired,
    Tick(u32),
    Fire(item_move::Request),
    Transfer(i32),
}

impl Pending {
    pub fn arm(&mut self, lock: &mut InputLock, frame: item_move::Request, now_ms: u32) {
        lock.stamp(STAMP_SLOT, now_ms);

        self.armed = true;

        self.frame = Some(frame);
    }

    pub fn arm_server(&mut self, lock: &mut InputLock, server: i32, now_ms: u32) {
        self.server = server;

        lock.stamp(STAMP_SLOT, now_ms);

        self.armed = true;
    }

    pub fn seconds(deadline_ms: u32, now_ms: u32) -> u32 {
        deadline_ms.wrapping_sub(now_ms).wrapping_add(FIRE_MS) / TICK_MS
    }

    pub fn tick(&mut self, lock: &mut InputLock, now_ms: u32) -> Step {
        let deadline = lock.stamps[STAMP_SLOT];

        if deadline == 0 {
            return Step::Nothing;
        }

        if !self.armed && deadline.wrapping_add(CANCEL_MS) < now_ms {
            lock.stamps[STAMP_SLOT] = 0;

            return Step::Expired;
        }

        if self.armed && deadline.wrapping_add(FIRE_MS) < now_ms {
            if self.server != 0 {
                let server = std::mem::take(&mut self.server);

                self.armed = false;

                return Step::Transfer(server);
            }

            let frame = self.frame.take();

            self.armed = false;

            return match frame {
                Some(frame) => Step::Fire(frame),
                None => Step::Nothing,
            };
        }

        let seconds = Pending::seconds(deadline, now_ms);

        if self.shown == Some(seconds) {
            return Step::Nothing;
        }

        self.shown = Some(seconds);

        Step::Tick(seconds)
    }
}

pub fn server_frame(client_id: u16, server: i32) -> Vec<u8> {
    chat::channel_packet(client_id, chat::WHISPER, "srv", &server.to_string())
}

pub const COMMAND_STAMP_SLOT: usize = 7;

pub const COMMAND_FRAME_LEN: usize = super::chat::CHANNEL_LEN;

pub const COMMAND_MESSAGE_RECORD: usize = 223;

pub const COMMAND_MESSAGE_LIFE_MS: u32 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPending {
    pub armed: bool,
    frame: [u8; COMMAND_FRAME_LEN],
    pub shown: Option<u32>,
}

impl Default for CommandPending {
    fn default() -> Self {
        Self {
            armed: false,
            frame: [0; COMMAND_FRAME_LEN],
            shown: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStep {
    Nothing,
    Expired,
    Tick(u32),
    Fire([u8; COMMAND_FRAME_LEN]),
}

impl CommandPending {
    pub fn arm(&mut self, lock: &mut InputLock, frame: [u8; COMMAND_FRAME_LEN], now_ms: u32) {
        self.frame = frame;

        self.armed = true;

        lock.stamp(COMMAND_STAMP_SLOT, now_ms);
    }

    pub fn tick(&mut self, lock: &mut InputLock, now_ms: u32) -> CommandStep {
        let stamp = lock.stamps[COMMAND_STAMP_SLOT];

        if stamp == 0 {
            return CommandStep::Nothing;
        }

        if !self.armed && stamp.wrapping_add(CANCEL_MS) < now_ms {
            lock.stamps[COMMAND_STAMP_SLOT] = 0;

            return CommandStep::Expired;
        }

        if self.armed && stamp.wrapping_add(FIRE_MS) < now_ms {
            let frame = std::mem::replace(&mut self.frame, [0; COMMAND_FRAME_LEN]);

            lock.stamps[COMMAND_STAMP_SLOT] = 0;

            return CommandStep::Fire(frame);
        }

        let quotient = Pending::seconds(stamp, now_ms);

        if self.shown == Some(quotient) {
            return CommandStep::Nothing;
        }

        self.shown = Some(quotient);

        CommandStep::Tick(quotient.wrapping_add(1))
    }
}

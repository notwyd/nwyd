use protocol::client;

pub const SLOTS: usize = 20;

pub const COOLDOWN_MS: u32 = 2000;

pub const ESCALATE_AT: i16 = 10;

pub const FROM_MOVE: u32 = 0;

pub const FROM_ATTACK: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

struct Slot {
    id: u16,
    asked_ms: u32,
    count: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Ask {
    WhoIs(u16),
    RerequestEntity(u16, u32),
    Nothing,
}

impl Ask {
    pub fn packet(&self) -> Option<Vec<u8>> {
        match *self {
            Ask::WhoIs(id) => Some(client::who_is(id)),
            Ask::RerequestEntity(id, arg) => Some(client::rerequest_entity(id, arg)),
            Ask::Nothing => None,
        }
    }

    pub fn describe(&self) -> Option<String> {
        match *self {
            Ask::WhoIs(id) => Some(format!("NWYD: 0x369 — who is entity {id}?")),
            Ask::RerequestEntity(id, _) => Some(format!(
                "NWYD: 0x2EC — entity {id} moved {ESCALATE_AT} times and the server has not \
                 described it",
            )),
            Ask::Nothing => None,
        }
    }
}

#[derive(Debug, Clone)]

pub struct Roster {
    slots: [Slot; SLOTS],
}

impl Default for Roster {
    fn default() -> Roster {
        Roster {
            slots: [Slot::default(); SLOTS],
        }
    }
}

impl Roster {
    pub fn saw(&mut self, id: u16, arg: u32, now_ms: u32) -> Ask {
        for slot in self.slots.iter_mut() {
            if slot.id == 0 {
                continue;
            }

            if now_ms.wrapping_sub(slot.asked_ms) > COOLDOWN_MS {
                *slot = Slot::default();
            } else if slot.id == id {
                slot.count = slot.count.wrapping_add(1);

                return match slot.count == ESCALATE_AT {
                    true => Ask::RerequestEntity(id, arg),
                    false => Ask::Nothing,
                };
            }
        }

        let Some(free) = self.slots.iter_mut().find(|slot| slot.id == 0) else {
            return Ask::Nothing;
        };

        *free = Slot {
            id,
            asked_ms: now_ms,
            count: 1,
        };

        Ask::WhoIs(id)
    }

    pub fn answered(&mut self, id: u16) {
        for slot in self.slots.iter_mut().filter(|slot| slot.id == id) {
            *slot = Slot::default();
        }
    }

    pub fn pending(&self) -> usize {
        self.slots.iter().filter(|slot| slot.id != 0).count()
    }
}

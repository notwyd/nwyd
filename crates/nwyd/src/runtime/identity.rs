#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EntityId(u16);

impl EntityId {
    pub const fn from_wire(value: u16) -> Self {
        Self(value)
    }

    pub const fn wire(self) -> u16 {
        self.0
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct AccountSlot(u8);

impl AccountSlot {
    pub const fn new(index: usize) -> Option<Self> {
        if index < formats::net::account::SLOTS {
            Some(Self(index as u8))
        } else {
            None
        }
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl std::fmt::Display for AccountSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

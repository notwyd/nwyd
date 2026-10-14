use formats::net::kept::{self, Line, Record};

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Lines {
    pub slots: [Option<String>; kept::LINE_SLOTS],
}

impl Lines {
    pub fn new() -> Lines {
        Lines::default()
    }

    pub fn store(&mut self, line: &Line) -> usize {
        self.slots[line.slot] = Some(line.text.clone());

        line.slot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Records {
    pub slots: [Option<Record>; kept::RECORD_SLOTS],
    pub gate: bool,
}

impl Default for Records {
    fn default() -> Records {
        Records {
            slots: [const { None }; kept::RECORD_SLOTS],
            gate: false,
        }
    }
}

impl Records {
    pub fn new() -> Records {
        Records::default()
    }

    pub fn sealed(&self, key: u32) -> Option<kept::SealedCharacter> {
        self.slots
            .iter()
            .find(|record| record.as_ref().map_or(0, Record::key) == key)
            .map(|record| {
                record.map_or(kept::SealedCharacter::default(), |record| {
                    record.sealed_character()
                })
            })
    }

    pub fn keys(&self) -> [u32; kept::RECORD_SLOTS] {
        std::array::from_fn(|i| self.slots[i].as_ref().map_or(0, Record::key))
    }

    pub fn store(&mut self, record: Record) -> usize {
        let slot = kept::slot_for(&self.keys(), record.key());

        self.slots[slot] = Some(record);

        self.gate = true;

        slot
    }

    pub fn sheet_value(&self, slot: usize) -> Option<(i16, i16)> {
        if !self.gate || slot >= kept::RECORD_SLOTS {
            return None;
        }

        self.slots[slot].as_ref().map(Record::split_tenths)
    }
}

pub fn seal_key(index: i32, item: &formats::net::account::Item) -> Option<u32> {
    (index == 0xD73 && item.effects[0].id == b';')
        .then(|| (u32::from(item.effects[0].value) << 8) | u32::from(item.effects[1].value))
}

pub fn query_frame(client_id: u16, key: u32) -> Vec<u8> {
    let mut frame = vec![0; 16];

    protocol::header::set_opcode(&mut frame, 0x2CD);

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame[12..16].copy_from_slice(&key.to_le_bytes());

    frame
}

pub fn describe_line(lines: &Lines, slot: usize) -> String {
    format!(
        "0x2c8 field text line — slot {slot} of {} is now {:?}; nothing in the image reads it",
        kept::LINE_SLOTS,
        lines.slots[slot].as_deref().unwrap_or(""),
    )
}

pub fn describe_record(records: &Records, slot: usize) -> String {
    let value = records.sheet_value(slot);

    format!(
        "0xdc3 keyed record — slot {slot} of {}, key {}, details = {}; cached for Spirit Seal hover",
        kept::RECORD_SLOTS,
        records.slots[slot].as_ref().map_or(0, Record::key),
        match value {
            Some((whole, frac)) => format!("{whole}.{}", frac.abs()),
            None => "unreadable".to_string(),
        },
    )
}

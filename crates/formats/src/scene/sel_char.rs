use crate::error::{Error, Kind, Result};

pub const FIELDS: usize = 7;

pub const SLOTS: usize = 4;

pub type Row = [i32; FIELDS];

#[derive(Debug, Clone, PartialEq, Eq, Default)]

pub struct SelChar {
    pub rows: Vec<Row>,
}

pub fn parse(bytes: &[u8]) -> Result<SelChar> {
    let text: String = bytes.iter().map(|&b| b as char).collect();

    let mut rows = Vec::new();

    for (n, line) in text.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let fields: std::result::Result<Vec<i32>, _> =
            line.split(',').map(|f| f.trim().parse::<i32>()).collect();

        let fields =
            fields.map_err(|e| Error::structure(Kind::SelChar, format!("line {}: {e}", n + 1)))?;

        let row: Row = fields.try_into().map_err(|v: Vec<i32>| {
            Error::structure(
                Kind::SelChar,
                format!(
                    "line {}: expected {FIELDS} fields, found {}",
                    n + 1,
                    v.len()
                ),
            )
        })?;

        rows.push(row);
    }

    Ok(SelChar { rows })
}

impl SelChar {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn get(&self, slot: usize) -> Option<&Row> {
        self.rows.get(slot)
    }
}

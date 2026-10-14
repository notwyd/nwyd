pub const RECORDS: usize = 27;

pub const RECORD_LEN: usize = 16;

pub const LEN: usize = RECORDS * RECORD_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Coupon {
    pub id_from: i32,
    pub id_to: i32,
    pub param: i32,
    pub value: i32,
}

impl Coupon {
    pub fn matches(&self, query: i32) -> bool {
        if self.id_to == 0 {
            self.id_from == query
        } else {
            self.id_from <= query && query <= self.id_to
        }
    }
}

pub fn parse(bytes: &[u8]) -> Vec<Coupon> {
    (0..RECORDS)
        .map(|i| match bytes.get(i * RECORD_LEN..(i + 1) * RECORD_LEN) {
            Some(rec) => {
                let at = |off: usize| {
                    i32::from_le_bytes([rec[off], rec[off + 1], rec[off + 2], rec[off + 3]])
                };

                Coupon {
                    id_from: at(0),
                    id_to: at(4),
                    param: at(8),
                    value: at(12),
                }
            }
            None => Coupon::default(),
        })
        .collect()
}

pub fn lookup(table: &[Coupon], query: i32) -> Option<(i16, i32)> {
    for c in table.iter().take(RECORDS) {
        if c.param == 0 {
            return None;
        }

        if c.matches(query) {
            return Some((c.value as i16, c.param));
        }
    }

    None
}

use crate::text::codepage::{self, Codepage};

pub const LABELS: usize = 11;

pub const LABEL_LEN: usize = 9;

pub const LABELS_LEN: usize = LABELS * LABEL_LEN;

pub const SLOTS: usize = 11;

pub const SLOTS_LEN: usize = SLOTS * 4;

pub const LEN: usize = LABELS_LEN + SLOTS_LEN;

const UNMASK_KEY: [u8; 99] = [
    0xb1, 0xdd, 0xb0, 0xad, 0xbb, 0xea, 0xc3, 0xa3, 0xbe, 0xc6, 0xb0, 0xa1, 0xc0, 0xda, 0xc0, 0xcf,
    0xb8, 0xb8, 0xc0, 0xcc, 0xc3, 0xb5, 0xba, 0xc0, 0xba, 0xbc, 0xbc, 0xf6, 0xb7, 0xcf, 0xbe, 0xc6,
    0xb8, 0xa7, 0xb4, 0xe4, 0xb0, 0xed, 0xbd, 0xc5, 0xba, 0xf1, 0xc7, 0xcf, 0xb1, 0xb8, 0xb3, 0xaa,
    0xbf, 0xec, 0xb8, 0xae, 0xb3, 0xaa, 0xb6, 0xf3, 0xc1, 0xc1, 0xc0, 0xba, 0xb3, 0xaa, 0xb6, 0xf3,
    0xbb, 0xf5, 0xb3, 0xaa, 0xb6, 0xf3, 0xc0, 0xc7, 0xbe, 0xee, 0xb8, 0xb0, 0xc0, 0xcc, 0xb4, 0xc2,
    0xc0, 0xcf, 0xc2, 0xef, 0xc0, 0xcf, 0xbe, 0xee, 0xb3, 0xb3, 0xb4, 0xcf, 0xb4, 0xd9, 0xc0, 0xe1,
    0xb2, 0xd9, 0xb7,
];

pub fn unmask_first_byte(stored: u8) -> u8 {
    let mut b = stored;

    for (i, &k) in UNMASK_KEY.iter().enumerate() {
        b = b.wrapping_sub(k.wrapping_add(i as u8));
    }

    b
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ServerNames {
    pub labels: [String; LABELS],
    pub slots: [i32; SLOTS],
}

pub fn parse(bytes: &[u8], codepage: Codepage) -> ServerNames {
    let mut buf = [0u8; LEN];

    let n = bytes.len().min(LEN);

    buf[..n].copy_from_slice(&bytes[..n]);

    buf[0] = unmask_first_byte(buf[0]);

    let labels = std::array::from_fn(|i| {
        let r = &buf[i * LABEL_LEN..(i + 1) * LABEL_LEN];

        let end = r.iter().position(|&b| b == 0).unwrap_or(LABEL_LEN);

        codepage::decode(&r[..end], codepage)
    });

    let slots: [i32; SLOTS] = std::array::from_fn(|i| {
        i32::from_le_bytes(
            buf[LABELS_LEN + i * 4..LABELS_LEN + i * 4 + 4]
                .try_into()
                .expect("fixed size"),
        )
    });

    ServerNames { labels, slots }
}

impl ServerNames {
    pub fn label_for_slot(&self, slot: usize) -> &str {
        let Some(&index) = self.slots.get(slot) else {
            return "";
        };

        if index <= 0 {
            return "";
        }

        self.labels
            .get(index as usize - 1)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn occupied(&self) -> impl Iterator<Item = (usize, &str)> {
        (0..SLOTS)
            .map(|s| (s, self.label_for_slot(s)))
            .filter(|(_, l)| !l.is_empty())
    }
}

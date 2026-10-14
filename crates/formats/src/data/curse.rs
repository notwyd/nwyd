use crate::text::{Codepage, codepage};

pub const RECORDS: usize = 256;

pub const RECORD_LEN: usize = 0x20;

pub const FIELD_LEN: usize = 16;

pub const LEN: usize = RECORDS * RECORD_LEN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub bad: Vec<u8>,
    pub replacement: Vec<u8>,
}

impl Entry {
    pub fn bad_text(&self, codepage: Codepage) -> String {
        codepage::decode(&self.bad, codepage)
    }

    pub fn replacement_text(&self, codepage: Codepage) -> String {
        codepage::decode(&self.replacement, codepage)
    }
}

pub fn deobfuscate(buf: &mut [u8]) {
    for b in buf {
        *b = !*b;
    }
}

fn field(rec: &[u8], off: usize) -> Vec<u8> {
    let f = &rec[off..off + FIELD_LEN];

    let end = f.iter().position(|&b| b == 0).unwrap_or(FIELD_LEN);

    f[..end].to_vec()
}

pub fn parse(bytes: &[u8]) -> Vec<Entry> {
    let n = bytes.len().min(LEN);

    let mut buf = bytes[..n].to_vec();

    deobfuscate(&mut buf);

    let mut out = Vec::new();

    for rec in buf.chunks_exact(RECORD_LEN) {
        if rec[0] == 0 {
            break;
        }

        out.push(Entry {
            bad: field(rec, 0),
            replacement: field(rec, FIELD_LEN),
        });
    }

    out
}

pub fn to_bytes(entries: &[Entry]) -> Vec<u8> {
    let mut out = vec![0u8; LEN];

    for (i, e) in entries.iter().take(RECORDS).enumerate() {
        let rec = &mut out[i * RECORD_LEN..(i + 1) * RECORD_LEN];

        let n = e.bad.len().min(FIELD_LEN);

        rec[..n].copy_from_slice(&e.bad[..n]);

        let n = e.replacement.len().min(FIELD_LEN);

        rec[FIELD_LEN..FIELD_LEN + n].copy_from_slice(&e.replacement[..n]);
    }

    deobfuscate(&mut out);

    out
}

pub fn filter(text: &[u8], entries: &[Entry]) -> Vec<u8> {
    const MAX_SUBSTITUTIONS: usize = 1024;

    let mut out = text.to_vec();

    for _ in 0..MAX_SUBSTITUTIONS {
        let hit = entries
            .iter()
            .filter(|e| !e.bad.is_empty())
            .find_map(|e| find(&out, &e.bad).map(|at| (at, e)));

        let Some((at, entry)) = hit else {
            return out;
        };

        out.splice(at..at + entry.bad.len(), entry.replacement.iter().copied());
    }

    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

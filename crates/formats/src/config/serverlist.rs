use crate::error::{Error, Kind, Result};

pub const HEADER_LEN: usize = 4;

pub const RECORD_LEN: usize = 64;

pub const ROWS_PER_SERVER: usize = 11;

pub const SERVERS: usize = 10;

pub const RECORDS: usize = SERVERS * ROWS_PER_SERVER;

pub const LEN: usize = HEADER_LEN + RECORDS * RECORD_LEN;

pub const KEY: [u8; RECORD_LEN] = [
    0xc1, 0xb6, 0xc0, 0xcc, 0xc0, 0xd3, 0xc6, 0xd1, 0xc6, 0xae, 0xbe, 0xcf, 0xc8, 0xa3, 0xc8, 0xad,
    0xc0, 0xdb, 0xbe, 0xf7, 0xc0, 0xbb, 0xc0, 0xa7, 0xc7, 0xd1, 0xbd, 0xba, 0xc5, 0xa9, 0xb8, 0xb3,
    0xc6, 0xae, 0xc0, 0xd4, 0xb4, 0xcf, 0xb4, 0xd9, 0xb8, 0xb8, 0xc7, 0xd1, 0xb1, 0xdb, 0xb7, 0xce,
    0xbe, 0xcf, 0xc8, 0xad, 0xc8, 0xad, 0xc7, 0xd2, 0xc1, 0xd9, 0xa4, 0xbb, 0xa4, 0xbb, 0x00, 0x00,
];

pub fn decrypt(buf: &mut [u8]) {
    for block in buf.chunks_mut(RECORD_LEN) {
        for (i, b) in block.iter_mut().enumerate() {
            *b = b.wrapping_sub(KEY[RECORD_LEN - 1 - i]);
        }
    }
}

pub fn encrypt(buf: &mut [u8]) {
    for block in buf.chunks_mut(RECORD_LEN) {
        for (i, b) in block.iter_mut().enumerate() {
            *b = b.wrapping_add(KEY[RECORD_LEN - 1 - i]);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Server {
    pub status_url: String,
    pub channels: Vec<String>,
}

impl Server {
    pub fn is_configured(&self) -> bool {
        !self.status_url.is_empty() || !self.channels.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerList {
    pub header: [u8; HEADER_LEN],
    pub servers: Vec<Server>,
}

pub fn parse(bytes: &[u8]) -> Result<ServerList> {
    if bytes.len() < LEN {
        return Err(Error::Truncated {
            kind: Kind::ServerList,
            offset: 0,
            needed: LEN,
            len: bytes.len(),
        });
    }

    let mut body = bytes[HEADER_LEN..LEN].to_vec();

    decrypt(&mut body);

    let record = |i: usize| -> String {
        let r = &body[i * RECORD_LEN..(i + 1) * RECORD_LEN];

        let end = r.iter().position(|&b| b == 0).unwrap_or(RECORD_LEN);

        String::from_utf8_lossy(&r[..end]).into_owned()
    };

    let servers = (0..SERVERS)
        .map(|s| {
            let base = s * ROWS_PER_SERVER;

            let mut channels: Vec<String> =
                (1..ROWS_PER_SERVER).map(|r| record(base + r)).collect();

            while channels.last().is_some_and(String::is_empty) {
                channels.pop();
            }

            Server {
                status_url: record(base),
                channels,
            }
        })
        .collect();

    Ok(ServerList {
        header: bytes[..HEADER_LEN].try_into().expect("checked above"),
        servers,
    })
}

impl ServerList {
    pub fn configured(&self) -> impl Iterator<Item = (usize, &Server)> {
        self.servers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_configured())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; LEN];

        out[..HEADER_LEN].copy_from_slice(&self.header);

        for (s, server) in self.servers.iter().take(SERVERS).enumerate() {
            let rows = std::iter::once(&server.status_url).chain(server.channels.iter());

            for (r, text) in rows.take(ROWS_PER_SERVER).enumerate() {
                let start = HEADER_LEN + (s * ROWS_PER_SERVER + r) * RECORD_LEN;

                let n = text.len().min(RECORD_LEN - 1);

                out[start..start + n].copy_from_slice(&text.as_bytes()[..n]);
            }
        }

        encrypt(&mut out[HEADER_LEN..]);

        out
    }
}

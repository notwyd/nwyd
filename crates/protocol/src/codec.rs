use crate::cipher;
use crate::header::{self, Header};

pub const HELLO: [u8; 4] = 0x1F11_F311u32.to_le_bytes();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub header: Header,
    pub payload: Vec<u8>,
    pub checksum_ok: bool,
}

impl Packet {
    pub fn opcode(&self) -> u16 {
        self.header.opcode
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    BadHello(u32),
    BadSize(u16),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::BadHello(seen) => {
                write!(
                    f,
                    "the server's greeting was {seen:#010x}, not {:#010x}",
                    u32::from_le_bytes(HELLO)
                )
            }
            DecodeError::BadSize(size) => {
                write!(
                    f,
                    "a packet declared {size} bytes, outside the {}..={} the client accepts",
                    header::MIN_SIZE,
                    header::MAX_SIZE,
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Debug, Clone, Default)]
pub struct Decoder {
    buffer: Vec<u8>,
    hello_seen: bool,
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder {
            buffer: Vec::new(),
            hello_seen: true,
        }
    }

    pub fn expecting_hello() -> Decoder {
        Decoder {
            buffer: Vec::new(),
            hello_seen: false,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    pub fn next_packet(&mut self) -> Option<Result<Packet, DecodeError>> {
        if !self.hello_seen {
            if self.buffer.len() < HELLO.len() {
                return None;
            }

            let seen: [u8; 4] = self.buffer[..4].try_into().expect("checked above");

            if seen != HELLO {
                return Some(Err(DecodeError::BadHello(u32::from_le_bytes(seen))));
            }

            self.hello_seen = true;

            self.buffer.drain(..HELLO.len());
        }

        if self.buffer.len() < header::LEN {
            return None;
        }

        let size = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);

        if !Header::size_is_legal(size) {
            self.buffer.clear();

            return Some(Err(DecodeError::BadSize(size)));
        }

        if self.buffer.len() < size as usize {
            return None;
        }

        let mut frame: Vec<u8> = self.buffer.drain(..size as usize).collect();

        let checksum_ok = cipher::decrypt(&mut frame);

        let header = Header::parse(&frame).expect("a legal size is at least a whole header");

        Some(Ok(Packet {
            header,
            payload: frame.split_off(header::LEN),
            checksum_ok,
        }))
    }

    pub fn drain(&mut self) -> (Vec<Packet>, Option<DecodeError>) {
        let mut packets = Vec::new();

        while let Some(next) = self.next_packet() {
            match next {
                Ok(packet) => {
                    let checksum_ok = packet.checksum_ok;

                    packets.push(packet);

                    if !checksum_ok {
                        break;
                    }
                }
                Err(error) => return (packets, Some(error)),
            }
        }

        (packets, None)
    }
}

pub fn seal(body: &mut [u8], key: u8, client_id: u16, tick: u32) {
    let size = body.len();

    body[header::SIZE_OFF..header::SIZE_OFF + 2].copy_from_slice(&(size as u16).to_le_bytes());

    body[header::KEY_OFF] = key;

    body[header::CHECKSUM_OFF] = cipher::checksum(key, size);

    body[header::CLIENT_ID_OFF..header::CLIENT_ID_OFF + 2]
        .copy_from_slice(&client_id.to_le_bytes());

    body[header::TICK_OFF..header::TICK_OFF + 4].copy_from_slice(&tick.to_le_bytes());

    cipher::encrypt(body, key);
}

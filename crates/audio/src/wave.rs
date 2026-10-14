use std::fmt;

const RIFF_HEADER: usize = 12;

pub const FORMAT_PCM: u16 = 1;

const FMT_MIN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotRiff,
    Truncated,
    NoFormat,
    NoData,
    NotPcm(u16),
    Unsupported { channels: u16, bits: u16 },
    ZeroRate,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotRiff => write!(f, "not a RIFF/WAVE file"),
            Error::Truncated => write!(f, "a chunk header runs past the end of the file"),
            Error::NoFormat => write!(f, "no usable `fmt ` chunk"),
            Error::NoData => write!(f, "no `data` chunk"),
            Error::NotPcm(tag) => write!(f, "wFormatTag {tag} is not PCM"),
            Error::Unsupported { channels, bits } => {
                write!(f, "{channels} channels at {bits} bits is not supported")
            }
            Error::ZeroRate => write!(f, "nSamplesPerSec is zero"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, PartialEq, Eq)]
pub struct Wave {
    pub channels: u16,
    pub rate: u32,
    pub samples: Vec<i16>,
}

impl fmt::Debug for Wave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Wave")
            .field("channels", &self.channels)
            .field("rate", &self.rate)
            .field("frames", &self.frames())
            .finish()
    }
}

impl Wave {
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }

    pub fn duration_ms(&self) -> u64 {
        if self.rate == 0 {
            0
        } else {
            self.frames() as u64 * 1000 / self.rate as u64
        }
    }

    pub fn frame(&self, index: usize) -> [i16; 2] {
        match self.channels {
            1 => self.samples.get(index).map(|&s| [s, s]).unwrap_or([0, 0]),
            _ => {
                let i = index * 2;

                match (self.samples.get(i), self.samples.get(i + 1)) {
                    (Some(&l), Some(&r)) => [l, r],
                    _ => [0, 0],
                }
            }
        }
    }
}

pub fn parse(bytes: &[u8]) -> Result<Wave, Error> {
    if bytes.len() < RIFF_HEADER || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(Error::NotRiff);
    }

    let mut format: Option<(u16, u16, u32, u16)> = None;

    let mut data: Option<&[u8]> = None;

    let mut off = RIFF_HEADER;

    while off + 8 <= bytes.len() {
        let id = &bytes[off..off + 4];

        let size =
            u32::from_le_bytes(bytes[off + 4..off + 8].try_into().expect("fixed size")) as usize;

        let body = off + 8;

        let end = body.checked_add(size).ok_or(Error::Truncated)?;

        if end > bytes.len() {
            return Err(Error::Truncated);
        }

        match id {
            b"fmt " if size >= FMT_MIN => {
                let f = &bytes[body..body + FMT_MIN];

                format = Some((
                    u16::from_le_bytes(f[0..2].try_into().expect("fixed size")),
                    u16::from_le_bytes(f[2..4].try_into().expect("fixed size")),
                    u32::from_le_bytes(f[4..8].try_into().expect("fixed size")),
                    u16::from_le_bytes(f[14..16].try_into().expect("fixed size")),
                ));
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }

        off = end + (size & 1);
    }

    let (tag, channels, rate, bits) = format.ok_or(Error::NoFormat)?;

    let data = data.ok_or(Error::NoData)?;

    if tag != FORMAT_PCM {
        return Err(Error::NotPcm(tag));
    }

    if rate == 0 {
        return Err(Error::ZeroRate);
    }

    let samples = match (channels, bits) {
        (1 | 2, 8) => data.iter().map(|&b| ((b as i16) - 128) << 8).collect(),
        (1 | 2, 16) => data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect(),
        _ => {
            return Err(Error::Unsupported { channels, bits });
        }
    };

    Ok(Wave {
        channels,
        rate,
        samples,
    })
}

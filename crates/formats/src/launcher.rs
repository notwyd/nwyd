use crate::text::Codepage;

#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub kind: u16,
    account: String,
    password: String,
}

impl Credentials {
    pub fn account(&self) -> &str {
        &self.account
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn enabled(&self) -> bool {
        self.kind != 0
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LauncherCredentials")
            .field("kind", &self.kind)
            .field("credentials", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    ShortBlob,
    NonDecimal,
    AccountCount,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ShortBlob => "launcher hand-off needs 138 bytes at offset 19",
            Self::NonDecimal => "launcher hand-off has a non-decimal credential field",
            Self::AccountCount => "launcher account exceeds its encoded field",
        })
    }
}

impl std::error::Error for Error {}

fn decimal(bytes: &[u8]) -> Result<u16, Error> {
    if bytes.len() != 3 || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(Error::NonDecimal);
    }

    Ok(u16::from(bytes[0] - b'0') * 100
        + u16::from(bytes[1] - b'0') * 10
        + u16::from(bytes[2] - b'0'))
}

fn decode(bytes: &[u8], key: u8) -> Result<String, Error> {
    let mut decoded = Vec::with_capacity(bytes.len() / 3);

    for triple in bytes.chunks_exact(3) {
        decoded.push((decimal(triple)? as u8).wrapping_sub(key));
    }

    let end = decoded
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(decoded.len());

    Ok(crate::text::codepage::decode(
        &decoded[..end],
        Codepage::Windows1252,
    ))
}

pub fn parse(command_line: &[u8]) -> Result<Option<Credentials>, Error> {
    if !command_line.windows(13).any(|w| w == b"joyimpact.wyd") {
        return Ok(None);
    }

    let blob = command_line.get(19..157).ok_or(Error::ShortBlob)?;

    if blob.contains(&0) {
        return Err(Error::ShortBlob);
    }

    let key = decimal(&blob[..3])? as u8;

    let count = usize::from(decimal(&blob[3..6])?);

    let kind = decimal(&blob[6..9])?;

    if count > 25 {
        return Err(Error::AccountCount);
    }

    Ok(Some(Credentials {
        kind,
        account: decode(&blob[15..15 + count * 3], key)?,
        password: decode(&blob[90..120], 0)?,
    }))
}

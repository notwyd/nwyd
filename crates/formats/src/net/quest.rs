pub fn parse(payload: &[u8]) -> Option<[u8; 11]> {
    if payload.get(..2)? != [1, 11] {
        return None;
    }

    payload.get(2..13)?.try_into().ok()
}

pub fn request(client: u16) -> Option<Vec<u8>> {
    if client == 0 {
        return None;
    }

    let mut frame = vec![0; 32];

    frame[..2].copy_from_slice(&32u16.to_le_bytes());

    frame[4..6].copy_from_slice(&0x4306u16.to_le_bytes());

    frame[6..8].copy_from_slice(&client.to_le_bytes());

    Some(frame)
}

pub fn class_masks(bytes: &[u8]) -> [u8; 256] {
    let mut masks = [0; 256];

    let bytes = &bytes[..bytes.len().min(8191)];

    let bytes = &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())];

    for (row, columns) in bytes
        .split(|&b| b == b'\n')
        .map(|line| line.split(|&b| b == b'\t').take(8).collect::<Vec<_>>())
        .filter(|columns| columns.len() >= 4)
        .take(256)
        .enumerate()
    {
        let Some(value) = columns.get(4) else {
            continue;
        };

        let end = value
            .iter()
            .rposition(|b| !matches!(b, b' ' | b'\r'))
            .map_or(0, |n| n + 1);

        let value = &value[..end];

        if (1..=3).contains(&value.len()) && value.iter().all(u8::is_ascii_digit) {
            let value = value
                .iter()
                .fold(0u16, |n, &b| n * 10 + u16::from(b - b'0'));

            if (1..=7).contains(&value) {
                masks[row] = value as u8;
            }
        }
    }

    masks
}

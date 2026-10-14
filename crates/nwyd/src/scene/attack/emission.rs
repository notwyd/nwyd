const fn flag_table() -> [i32; 1024] {
    let mut table = [0; 1024];

    let mut value = 0;

    let mut stride = 512;

    let mut phase = 0;

    while stride != 0 {
        let mut index = 0;

        while index < 1024 {
            if table[index] == 0 {
                let candidate = match phase {
                    0 => value,
                    1 => 512 - value,
                    2 => 512 + value,
                    _ => 1024 - value,
                };

                table[index] = if candidate > 999 { 999 } else { candidate };

                phase += 1;

                if phase == 4 {
                    phase = 0;

                    value += 1;
                }
            }

            index += stride;
        }

        stride /= 2;
    }

    table[0] = 512;

    table
}

const FLAG_TABLE: [i32; 1024] = flag_table();

pub fn automatic_flags(sequence: &mut u16, critical: u8, attack_speed: i16) -> u8 {
    if *sequence >= 1024 {
        *sequence = 0;
    }

    let value = FLAG_TABLE[usize::from(*sequence)];

    let flags = (u8::from(value < i32::from(attack_speed)) * 8)
        | u8::from(1000 - value < i32::from(critical) * 4);

    *sequence += 1;

    flags
}

pub fn weapon_variant(kind: i32, mesh: i16) -> Option<u8> {
    match (kind, mesh) {
        (0x65, _) | (0x66, 0x367) | (0x67, 0x369) | (0x68, 0x36e) => Some(0),
        (0x66, 0x368) | (0x67, 0x36a) | (0x68, 0x36f) => Some(1),
        (0x67, mesh) => Some(match mesh {
            0x36b => 2,
            0x36c => 3,
            0x36d => 4,
            0x37c => 5,
            0x38b => 6,
            0x38c => 7,
            0x38d => 8,
            0x25 => 9,
            0x2ff => 10,
            0xafe => 11,
            0xb54 => 12,
            0xb75 => 13,
            0xb7e => 14,
            0xb89 => 15,
            0xb94 => 16,
            0xb9f => 17,
            0xbad => 18,
            3000 => 19,
            0xbc3 => 20,
            0xbce => 21,
            0xbfd => 22,
            _ => return None,
        }),
        _ => None,
    }
}

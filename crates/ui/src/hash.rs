pub fn name_hash(name: &str) -> i32 {
    let mut hash: i32 = 0x1505;

    for unit in name.encode_utf16() {
        if unit == 0 {
            break;
        }

        let folded = if unit.wrapping_sub(0x41) < 0x1a {
            unit + 0x20
        } else {
            unit
        };

        hash = hash.wrapping_mul(0x21).wrapping_add(i32::from(folded));
    }

    hash
}

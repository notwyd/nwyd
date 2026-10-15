use super::{DiscoveryError, build_word};

const BASE: u32 = 0x1000_0000;
const TEXT: u32 = 0x1000;
const HELPER: u32 = 0x1200;
const COPY: u32 = 0x1300;
const IMPORTS: u32 = 0x2000;
const IAT: u32 = 0x2060;
const GLOBALS: u32 = 0x3000;

fn word(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn dword(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn offset(rva: u32) -> usize {
    match rva {
        0x1000..0x2000 => 0x200 + (rva - 0x1000) as usize,
        0x2000..0x2200 => 0x1200 + (rva - 0x2000) as usize,
        0x3000..0x3200 => 0x1400 + (rva - 0x3000) as usize,
        _ => panic!("invalid fixture RVA {rva:#x}"),
    }
}

fn put(bytes: &mut [u8], rva: u32, data: &[u8]) {
    let at = offset(rva);
    bytes[at..at + data.len()].copy_from_slice(data);
}

fn call(code: &mut Vec<u8>, start: u32, target: u32) {
    let next = start + code.len() as u32 + 5;
    code.push(0xe8);
    code.extend_from_slice(&target.wrapping_sub(next).to_le_bytes());
}

fn imported(code: &mut Vec<u8>, address: u32) {
    code.extend_from_slice(&[0xff, 0x15]);
    code.extend_from_slice(&address.to_le_bytes());
}

struct Main {
    bytes: Vec<u8>,
    second_source: usize,
    second_helper: usize,
    second_destination: usize,
    second_patch: usize,
}

fn main_code(base: u32, start: u32, build: u32, slot: u8, first_register: u8) -> Main {
    let mut bytes = vec![0x56, 0x57, 0x53, 0x83, 0xec, 0x40, 0x8b];
    bytes.push(0x05 | (first_register << 3));
    bytes.extend_from_slice(&(base + GLOBALS).to_le_bytes());
    bytes.extend_from_slice(&[0x8b, 0x3d]);
    let second_destination = bytes.len();
    bytes.extend_from_slice(&(base + GLOBALS + 4).to_le_bytes());
    bytes.extend_from_slice(&[0x6a, 0x04, 0x8d, 0x54, 0x24, slot, 0xc7, 0x44, 0x24, slot]);
    bytes.extend_from_slice(&build.to_le_bytes());
    bytes.extend_from_slice(&[0x8b, 0xc8 | first_register]);
    call(&mut bytes, start, HELPER);
    bytes.extend_from_slice(&[0x83, 0xc4, 0x04, 0x84, 0xc0, 0x74]);
    let branch = bytes.len();
    bytes.push(0);
    bytes.extend_from_slice(&[0x6a, 0x04, 0x50 | first_register]);
    imported(&mut bytes, base + IAT + 4);
    bytes.push(0x50);
    imported(&mut bytes, base + IAT + 8);
    let second_patch = bytes.len();
    bytes[branch] = (second_patch - branch - 1) as u8;
    bytes.extend_from_slice(&[0x6a, 0x04, 0x8d, 0x54, 0x24]);
    let second_source = bytes.len();
    bytes.push(slot);
    bytes.extend_from_slice(&[0x8b, 0xcf]);
    let second_helper = bytes.len();
    call(&mut bytes, start, HELPER);
    bytes.extend_from_slice(&[0x83, 0xc4, 0x04, 0x83, 0xc4, 0x40, 0x5b, 0x5f, 0x5e, 0xc3]);
    Main {
        bytes,
        second_source,
        second_helper,
        second_destination,
        second_patch,
    }
}

fn helper_code(base: u32) -> Vec<u8> {
    let mut bytes = vec![
        0x55, 0x8b, 0xec, 0x83, 0xec, 0x08, 0x53, 0x56, 0x57, 0x8b, 0xf1, 0x8b, 0xfa, 0x8b, 0x5d,
        0x08, 0x8d, 0x45, 0xfc, 0x50, 0x6a, 0x40, 0x53, 0x56,
    ];
    imported(&mut bytes, base + IAT);
    bytes.extend_from_slice(&[0x53, 0x57, 0x56]);
    call(&mut bytes, HELPER, COPY);
    bytes.extend_from_slice(&[
        0x83, 0xc4, 0x0c, 0x8d, 0x45, 0xf8, 0x50, 0xff, 0x75, 0xfc, 0x53, 0x56,
    ]);
    imported(&mut bytes, base + IAT);
    bytes.extend_from_slice(&[0x53, 0x56]);
    imported(&mut bytes, base + IAT + 4);
    bytes.push(0x50);
    imported(&mut bytes, base + IAT + 8);
    bytes.extend_from_slice(&[0xb0, 0x01, 0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc3]);
    bytes
}

fn fixture(base: u32, build: u32, slot: u8, register: u8) -> (Vec<u8>, Main) {
    let mut bytes = vec![0; 0x1600];
    bytes[..2].copy_from_slice(b"MZ");
    dword(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    word(&mut bytes, 0x84, 0x14c);
    word(&mut bytes, 0x86, 3);
    word(&mut bytes, 0x94, 0xe0);
    word(&mut bytes, 0x96, 0x2102);
    word(&mut bytes, 0x98, 0x10b);
    dword(&mut bytes, 0xa8, TEXT);
    dword(&mut bytes, 0xb4, base);
    dword(&mut bytes, 0xb8, 0x1000);
    dword(&mut bytes, 0xbc, 0x200);
    dword(&mut bytes, 0xd0, 0x4000);
    dword(&mut bytes, 0xd4, 0x200);
    dword(&mut bytes, 0xf4, 16);
    dword(&mut bytes, 0x100, IMPORTS);
    dword(&mut bytes, 0x104, 40);
    for (index, name, rva, size, raw, flags) in [
        (0, b".text\0\0\0", TEXT, 0x1000, 0x200, 0x6000_0020),
        (1, b".rdata\0\0", IMPORTS, 0x200, 0x1200, 0x4000_0040),
        (2, b".data\0\0\0", GLOBALS, 0x200, 0x1400, 0xc000_0040),
    ] {
        let at = 0x178 + index * 40;
        bytes[at..at + 8].copy_from_slice(name);
        dword(&mut bytes, at + 8, size);
        dword(&mut bytes, at + 12, rva);
        dword(&mut bytes, at + 16, size);
        dword(&mut bytes, at + 20, raw);
        dword(&mut bytes, at + 36, flags);
    }
    dword(&mut bytes, offset(IMPORTS), 0x2040);
    dword(&mut bytes, offset(IMPORTS) + 12, 0x2080);
    dword(&mut bytes, offset(IMPORTS) + 16, IAT);
    put(&mut bytes, 0x2080, b"KERNEL32.dll\0");
    for (index, rva, name) in [
        (0, 0x2090, b"VirtualProtect\0".as_slice()),
        (1, 0x20a8, b"GetCurrentProcess\0".as_slice()),
        (2, 0x20c0, b"FlushInstructionCache\0".as_slice()),
    ] {
        dword(&mut bytes, offset(0x2040) + index * 4, rva);
        dword(&mut bytes, offset(IAT) + index * 4, rva);
        put(&mut bytes, rva + 2, name);
    }
    let main = main_code(base, TEXT, build, slot, register);
    put(&mut bytes, TEXT, &main.bytes);
    put(&mut bytes, HELPER, &helper_code(base));
    put(
        &mut bytes,
        COPY,
        &[
            0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x09, 0x8b, 0x44, 0x24, 0x04, 0x89, 0x08, 0xc3,
        ],
    );
    (bytes, main)
}

#[test]
fn build_discovery_accepts_new_constants_stack_slots_registers_and_image_bases() {
    for (base, build, slot, register) in [
        (BASE, 0xc9d8, 0x1c, 6),
        (BASE, 0xc9d9, 0x24, 6),
        (0x2800_0000, 0xcafe, 0x34, 3),
    ] {
        let (bytes, _) = fixture(base, build, slot, register);
        assert_eq!(build_word(&bytes).unwrap(), build);
    }
}

#[test]
fn non_executable_decoys_do_not_supply_or_conflict_with_a_build() {
    let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    let decoy = main_code(BASE, GLOBALS + 0x40, 0xdead, 0x24, 6);
    put(&mut bytes, GLOBALS + 0x40, &decoy.bytes);
    assert_eq!(build_word(&bytes).unwrap(), 0xc9d9);
    bytes[offset(TEXT)..offset(TEXT) + 0x200].fill(0xcc);
    assert!(build_word(&bytes).is_err());
}

#[test]
fn paired_patches_require_the_same_source_helper_and_distinct_destinations() {
    let (bytes, main) = fixture(BASE, 0xc9d9, 0x24, 6);
    let mut other_source = bytes.clone();
    other_source[offset(TEXT) + main.second_source] = 0x28;
    assert!(build_word(&other_source).is_err());
    let mut other_helper = bytes.clone();
    dword(
        &mut other_helper,
        offset(TEXT) + main.second_helper + 1,
        (HELPER + 1).wrapping_sub(TEXT + main.second_helper as u32 + 5),
    );
    assert!(build_word(&other_helper).is_err());
    let mut same_destination = bytes;
    dword(
        &mut same_destination,
        offset(TEXT) + main.second_destination,
        BASE + GLOBALS,
    );
    assert!(build_word(&same_destination).is_err());
}

#[test]
fn a_shared_helper_must_protect_copy_and_restore_the_requested_memory() {
    let (bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    for (at, replacement) in [(21, 0x20), (31, 0x56), (47, 0xf8)] {
        let mut invalid_helper = bytes.clone();
        invalid_helper[offset(HELPER) + at] = replacement;
        assert!(build_word(&invalid_helper).is_err(), "helper offset {at}");
    }
}

#[test]
fn an_unreachable_flush_or_a_source_write_on_one_branch_cannot_verify_a_patch() {
    let (mut bytes, main) = fixture(BASE, 0xc9d9, 0x24, 6);
    let guard = main
        .bytes
        .windows(3)
        .position(|bytes| bytes == [0x84, 0xc0, 0x74])
        .unwrap();
    put(&mut bytes, TEXT + guard as u32, &[0x31, 0xc0]);
    assert!(build_word(&bytes).is_err());

    let (mut bytes, main) = fixture(BASE, 0xc9d9, 0x24, 6);
    let mut changed = main.bytes.clone();
    let mut store = vec![0xc7, 0x44, 0x24, 0x20];
    store.extend_from_slice(&0xc9d9u32.to_le_bytes());
    changed.splice(guard + 4..guard + 4, store);
    changed[guard + 3] += 8;
    let second_call = main.second_helper + 8;
    dword(
        &mut changed,
        second_call + 1,
        HELPER.wrapping_sub(TEXT + second_call as u32 + 5),
    );
    put(&mut bytes, TEXT, &changed);
    assert!(build_word(&bytes).is_err());
}

#[test]
fn branches_cannot_bypass_the_first_patch_or_second_patch_arguments() {
    let (mut bytes, main) = fixture(BASE, 0xc9d9, 0x24, 6);
    let exit = main.bytes.len() - 1;
    put(
        &mut bytes,
        TEXT,
        &[0x31, 0xc0, 0x74, (exit - 4) as u8, 0x90, 0x90],
    );
    assert!(build_word(&bytes).is_err());

    let (mut bytes, main) = fixture(BASE, 0xc9d9, 0x24, 6);
    let guard = main
        .bytes
        .windows(3)
        .position(|bytes| bytes == [0x84, 0xc0, 0x74])
        .unwrap();
    bytes[offset(TEXT) + guard + 3] = (main.second_helper - guard - 4) as u8;
    assert!(build_word(&bytes).is_err());
}

#[test]
fn a_patch_helper_cannot_modify_its_source() {
    for after_flush in [false, true] {
        let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
        let mut helper = helper_code(BASE);
        let marker: &[u8] = if after_flush {
            &[0xb0, 0x01]
        } else {
            &[0x53, 0x56, 0xff, 0x15]
        };
        let at = helper
            .windows(marker.len())
            .rposition(|bytes| bytes == marker)
            .unwrap();
        let mut store = vec![0xc7, 0x07];
        store.extend_from_slice(&0xc9d8u32.to_le_bytes());
        helper.splice(at..at, store);
        put(&mut bytes, HELPER, &helper);
        assert!(build_word(&bytes).is_err(), "after_flush={after_flush}");
    }
}

#[test]
fn a_patch_copy_cannot_replace_the_discovered_version() {
    let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    put(
        &mut bytes,
        COPY,
        &[
            0x8b, 0x4c, 0x24, 0x08, 0xc7, 0x01, 0xd8, 0xc9, 0x00, 0x00, 0x8b, 0x44, 0x24, 0x04,
            0xc7, 0x00, 0xd8, 0xc9, 0x00, 0x00, 0xc3,
        ],
    );
    assert!(build_word(&bytes).is_err());
}

#[test]
fn genuine_copy_callees_can_use_words_bytes_guards_and_bounded_loops() {
    let scalar = [
        0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x09, 0x8b, 0x44, 0x24, 0x04, 0x89, 0x08, 0xc3,
    ];
    let words = [
        0x8b, 0x44, 0x24, 0x04, 0x8b, 0x4c, 0x24, 0x08, 0x66, 0x8b, 0x11, 0x66, 0x89, 0x10, 0x66,
        0x8b, 0x51, 0x02, 0x66, 0x89, 0x50, 0x02, 0xc3,
    ];
    let bytes = [
        0x56, 0x57, 0x8b, 0x74, 0x24, 0x10, 0x8b, 0x7c, 0x24, 0x0c, 0x8a, 0x06, 0x88, 0x07, 0x8a,
        0x46, 0x01, 0x88, 0x47, 0x01, 0x8a, 0x46, 0x02, 0x88, 0x47, 0x02, 0x8a, 0x46, 0x03, 0x88,
        0x47, 0x03, 0x8b, 0xc7, 0x5f, 0x5e, 0xc3,
    ];
    let counted = [
        0x56, 0x57, 0x8b, 0x74, 0x24, 0x10, 0x8b, 0x7c, 0x24, 0x0c, 0x8b, 0x4c, 0x24, 0x14, 0x8a,
        0x06, 0x88, 0x07, 0x83, 0xc6, 0x01, 0x83, 0xc7, 0x01, 0x83, 0xe9, 0x01, 0x75, 0xf1, 0x8b,
        0x44, 0x24, 0x0c, 0x5f, 0x5e, 0xc3,
    ];
    let mut guarded = vec![0x83, 0x7c, 0x24, 0x0c, 0x04, 0x75, scalar.len() as u8];
    guarded.extend_from_slice(&scalar);
    guarded.push(0xcc);
    let mut skipped_trap = vec![0xeb, 0x01, 0xcc];
    skipped_trap.extend_from_slice(&scalar);
    for (name, copy) in [
        ("scalar", scalar.as_slice()),
        ("words", words.as_slice()),
        ("bytes", bytes.as_slice()),
        ("counted", counted.as_slice()),
        ("guarded", guarded.as_slice()),
        ("skipped trap", skipped_trap.as_slice()),
    ] {
        let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
        put(&mut bytes, COPY, copy);
        assert_eq!(build_word(&bytes), Ok(0xc9d9), "{name}");
    }
}

#[test]
fn every_feasible_alignment_branch_must_copy_the_four_bytes() {
    let mut copy = vec![
        0x8b, 0x44, 0x24, 0x04, 0x8b, 0x4c, 0x24, 0x08, 0xf7, 0xc1, 0x03, 0x00, 0x00, 0x00, 0x75,
        0x05, 0x8b, 0x11, 0x89, 0x10, 0xc3,
    ];
    copy.extend_from_slice(&[
        0x8a, 0x11, 0x88, 0x10, 0x8a, 0x51, 0x01, 0x88, 0x50, 0x01, 0x8a, 0x51, 0x02, 0x88, 0x50,
        0x02, 0x8a, 0x51, 0x03, 0x88, 0x50, 0x03, 0xc3,
    ]);
    let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    put(&mut bytes, COPY, &copy);
    assert_eq!(build_word(&bytes).unwrap(), 0xc9d9);
    copy[21] = 0xc3;
    put(&mut bytes, COPY, &copy);
    assert!(build_word(&bytes).is_err());
}

#[test]
fn the_copy_must_work_for_symbolic_source_bytes_not_only_the_declared_build() {
    let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    put(
        &mut bytes,
        COPY,
        &[
            0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x11, 0x81, 0xfa, 0xd9, 0xc9, 0x00, 0x00, 0x75, 0x07,
            0x8b, 0x44, 0x24, 0x04, 0x89, 0x10, 0xc3, 0xc3,
        ],
    );
    assert_eq!(build_word(&bytes), Err(DiscoveryError::NotFound));
    put(
        &mut bytes,
        COPY,
        &[
            0x8b, 0x44, 0x24, 0x04, 0xc7, 0x00, 0xd9, 0xc9, 0x00, 0x00, 0xc3,
        ],
    );
    assert_eq!(build_word(&bytes), Err(DiscoveryError::NotFound));
}

#[test]
fn copy_callees_cannot_rewrite_source_bytes_even_if_the_final_value_matches() {
    let prefix = [
        0x8b, 0x44, 0x24, 0x04, 0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x11, 0x89, 0x10,
    ];
    for suffix in [
        vec![0x89, 0x11, 0xc3],
        vec![0xc7, 0x01, 0, 0, 0, 0, 0x89, 0x11, 0xc3],
    ] {
        let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
        put(
            &mut bytes,
            COPY,
            &[prefix.as_slice(), suffix.as_slice()].concat(),
        );
        assert_eq!(build_word(&bytes), Err(DiscoveryError::NotFound));
    }
}

#[test]
fn a_copy_callee_cannot_skip_transform_truncate_or_reorder_the_source() {
    for (name, copy) in [
        ("no copy", vec![0x8b, 0x44, 0x24, 0x04, 0xc3]),
        (
            "transformed",
            vec![
                0x8b, 0x44, 0x24, 0x04, 0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x11, 0x83, 0xf2, 0x01, 0x89,
                0x10, 0xc3,
            ],
        ),
        (
            "two bytes",
            vec![
                0x8b, 0x44, 0x24, 0x04, 0x8b, 0x4c, 0x24, 0x08, 0x66, 0x8b, 0x11, 0x66, 0x89, 0x10,
                0xc3,
            ],
        ),
        (
            "reordered",
            vec![
                0x8b, 0x44, 0x24, 0x04, 0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x11, 0x0f, 0xca, 0x89, 0x10,
                0xc3,
            ],
        ),
        (
            "outside source",
            vec![
                0x8b, 0x44, 0x24, 0x04, 0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x51, 0x04, 0x89, 0x10, 0xc3,
            ],
        ),
    ] {
        let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
        put(&mut bytes, COPY, &copy);
        assert!(build_word(&bytes).is_err(), "{name}");
    }
}

#[test]
fn copy_callees_cannot_write_outside_the_destination_or_damage_the_callers_state() {
    let scalar = [
        0x8b, 0x4c, 0x24, 0x08, 0x8b, 0x09, 0x8b, 0x44, 0x24, 0x04, 0x89, 0x08,
    ];
    let mut global_write = vec![0xc7, 0x05];
    global_write.extend_from_slice(&(BASE + GLOBALS + 8).to_le_bytes());
    global_write.extend_from_slice(&[0; 4]);
    for (name, suffix) in [
        (
            "outside destination",
            vec![0xc7, 0x40, 0x04, 0, 0, 0, 0, 0xc3],
        ),
        (
            "source write",
            vec![0x8b, 0x54, 0x24, 0x08, 0xc7, 0x02, 0, 0, 0, 0, 0xc3],
        ),
        (
            "unknown write",
            vec![0x8b, 0x54, 0x24, 0x10, 0xc7, 0x02, 0, 0, 0, 0, 0xc3],
        ),
        ("global write", [global_write, vec![0xc3]].concat()),
        ("saved register", vec![0xbb, 0, 0, 0, 0, 0xc3]),
        ("frame register", vec![0x31, 0xed, 0xc3]),
        ("callee cleanup", vec![0xc2, 0x0c, 0x00]),
        ("stack imbalance", vec![0x50, 0xc3]),
        (
            "argument write",
            vec![0xc7, 0x44, 0x24, 0x04, 0, 0, 0, 0, 0xc3],
        ),
        (
            "return address write",
            vec![0xc7, 0x04, 0x24, 0, 0, 0, 0, 0xc3],
        ),
        ("unbounded loop", vec![0xeb, 0xfe]),
        ("unknown SIMD", vec![0x66, 0x0f, 0xef, 0xc0, 0xc3]),
        ("string write", vec![0xf3, 0xab, 0xc3]),
    ] {
        let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
        put(
            &mut bytes,
            COPY,
            &[scalar.as_slice(), suffix.as_slice()].concat(),
        );
        assert!(build_word(&bytes).is_err(), "{name}");
    }
    let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    let mut calls_unknown = scalar.to_vec();
    call(&mut calls_unknown, COPY, COPY + 0x80);
    calls_unknown.push(0xc3);
    put(&mut bytes, COPY, &calls_unknown);
    put(&mut bytes, COPY + 0x80, &[0xc3]);
    assert!(build_word(&bytes).is_err());
}

#[test]
fn an_unrecognized_import_cannot_replace_a_recognized_import_slot() {
    let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    dword(&mut bytes, 0x104, 60);
    dword(&mut bytes, offset(IMPORTS) + 20, 0x20e0);
    dword(&mut bytes, offset(IMPORTS) + 32, 0x2080);
    dword(&mut bytes, offset(IMPORTS) + 36, IAT);
    dword(&mut bytes, offset(0x20e0), 0x20f0);
    put(&mut bytes, 0x20f2, b"ExitProcess\0");
    assert!(build_word(&bytes).is_err());
}

#[test]
fn overwritten_source_and_conflicting_executable_candidates_are_rejected() {
    let (mut bytes, main) = fixture(BASE, 0xc9d9, 0x24, 6);
    let mut overwritten = main.bytes.clone();
    let mut store = vec![0xc7, 0x44, 0x24, 0x20];
    store.extend_from_slice(&0xc9d8u32.to_le_bytes());
    overwritten.splice(main.second_patch..main.second_patch, store);
    let second_call = main.second_helper + 8;
    dword(
        &mut overwritten,
        second_call + 1,
        HELPER.wrapping_sub(TEXT + second_call as u32 + 5),
    );
    put(&mut bytes, TEXT, &overwritten);
    assert!(build_word(&bytes).is_err());
    for duplicate in [0xc9d8, 0xc9d9] {
        let (mut bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
        let conflict = main_code(BASE, 0x1400, duplicate, 0x24, 6);
        put(&mut bytes, 0x1400, &conflict.bytes);
        assert_eq!(
            build_word(&bytes),
            Err(DiscoveryError::Ambiguous { candidates: 2 }),
        );
    }
}

#[test]
fn malformed_pe_headers_sections_and_imports_cannot_supply_a_build() {
    let (bytes, _) = fixture(BASE, 0xc9d9, 0x24, 6);
    for length in [0, 1, 0x3f, 0x83, 0x97, 0x177, 0x1ef, 0x1000, 0x1213, 0x1400] {
        assert!(build_word(&bytes[..length]).is_err(), "length {length:#x}");
    }
    for (at, value) in [
        (0x3c, u32::MAX),
        (0x100, 0x3fff),
        (0x178 + 20, u32::MAX),
        (offset(IMPORTS), 0x21ff),
        (offset(0x2040), 0x3fff),
    ] {
        let mut broken = bytes.clone();
        dword(&mut broken, at, value);
        assert!(build_word(&broken).is_err(), "offset {at:#x}");
    }
}

#[test]
fn installed_proxy_fixtures_resolve_the_audited_builds() {
    for (variable, expected) in [("NWYD_PROXY_735", 0xc9d8), ("NWYD_PROXY_739", 0xc9d9)] {
        let Some(path) = std::env::var_os(variable) else {
            continue;
        };
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(build_word(&bytes).unwrap(), expected, "{variable}");
    }
}

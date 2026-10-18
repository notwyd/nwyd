use super::*;
use flate2::read::ZlibDecoder;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "nwyd-neural-build-test-{}-{unique}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        ));

        fs::create_dir(&path).unwrap();

        Self(path)
    }

    fn source(&self) -> PathBuf {
        self.0.join("source")
    }

    fn output(&self) -> PathBuf {
        self.0.join("output")
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn dll(seed: u8) -> Vec<u8> {
    let mut bytes = vec![0; 528];

    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&64_u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[68..70].copy_from_slice(&0x8664_u16.to_le_bytes());
    bytes[70..72].copy_from_slice(&1_u16.to_le_bytes());
    bytes[84..86].copy_from_slice(&240_u16.to_le_bytes());
    bytes[86..88].copy_from_slice(&0x2002_u16.to_le_bytes());
    bytes[88..90].copy_from_slice(&0x20b_u16.to_le_bytes());
    bytes[328..333].copy_from_slice(b".text");
    bytes[344..348].copy_from_slice(&16_u32.to_le_bytes());
    bytes[348..352].copy_from_slice(&512_u32.to_le_bytes());
    bytes[512..].fill(seed);

    bytes
}

fn populate(directory: &Directory) {
    fs::create_dir(directory.source()).unwrap();

    for (index, name) in REQUIRED.into_iter().enumerate() {
        let bytes = if name.ends_with(".dll") {
            dll(index as u8)
        } else {
            format!("Synthetic attribution for {name}\n").into_bytes()
        };

        fs::write(directory.source().join(name), bytes).unwrap();
    }
}

fn generated(directory: &Directory) -> String {
    fs::read_to_string(directory.output().join("neural_bundle.rs")).unwrap()
}

fn id(declaration: &str) -> &str {
    declaration
        .split("id: \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
}

#[test]
fn valid_payloads_round_trip_with_matching_hashes_and_a_deterministic_manifest() {
    let directory = Directory::new();

    populate(&directory);

    build(&directory.source(), &directory.output()).unwrap();

    let declaration = generated(&directory);

    assert_eq!(id(&declaration).len(), 64);
    assert_eq!(declaration.matches("BundleFile {").count(), REQUIRED.len());

    for (index, name) in REQUIRED.into_iter().enumerate() {
        let bytes = fs::read(directory.source().join(name)).unwrap();

        let compressed = fs::read(
            directory
                .output()
                .join(format!("neural_bundle_{index}.zlib")),
        )
        .unwrap();

        let mut decoded = Vec::new();

        ZlibDecoder::new(&compressed[..])
            .read_to_end(&mut decoded)
            .unwrap();

        assert_eq!(decoded, bytes);

        let hash: [u8; 32] = Sha256::digest(&decoded).into();

        assert!(declaration.contains(&format!("path: {name:?}")));
        assert!(declaration.contains(&format!("sha256: {hash:?}")));
        assert!(declaration.contains(&format!("uncompressed_len: {}", decoded.len())));
    }

    build(&directory.source(), &directory.output()).unwrap();

    assert_eq!(generated(&directory), declaration);
}

#[test]
fn payload_and_attribution_changes_create_new_bundle_ids() {
    let directory = Directory::new();

    populate(&directory);

    build(&directory.source(), &directory.output()).unwrap();

    let initial = generated(&directory);

    fs::write(directory.source().join(REQUIRED[0]), dll(99)).unwrap();

    build(&directory.source(), &directory.output()).unwrap();

    let changed_dll = generated(&directory);

    assert_ne!(id(&initial), id(&changed_dll));

    fs::write(
        directory.source().join(REQUIRED[2]),
        "Updated synthetic attribution\n",
    )
    .unwrap();

    build(&directory.source(), &directory.output()).unwrap();

    assert_ne!(id(&changed_dll), id(&generated(&directory)));
}

#[test]
fn missing_required_payloads_and_attribution_fail_clearly() {
    for name in REQUIRED {
        let directory = Directory::new();

        populate(&directory);

        fs::remove_file(directory.source().join(name)).unwrap();

        let error = build(&directory.source(), &directory.output()).unwrap_err();

        assert!(error.contains(name), "{error}");
        assert!(!directory.output().join("neural_bundle.rs").exists());
    }
}

#[test]
fn wine_and_unrelated_files_do_not_change_the_bundle_manifest_or_id() {
    let directory = Directory::new();

    populate(&directory);

    for name in OPTIONAL {
        fs::write(
            directory.source().join(name),
            "Synthetic optional license\n",
        )
        .unwrap();
    }

    build(&directory.source(), &directory.output()).unwrap();

    let declaration = generated(&directory);

    for name in OPTIONAL {
        assert!(declaration.contains(&format!("path: {name:?}")));
    }

    assert_eq!(
        declaration.matches("BundleFile {").count(),
        REQUIRED.len() + OPTIONAL.len()
    );

    fs::create_dir(directory.source().join("wine")).unwrap();

    for name in [
        "wine/nvapi64.dll",
        "wine/dxgi.dll",
        "wine/LICENSE-nvapi.txt",
        "wine/LICENSE-dxgi.txt",
        "dxgi.dll",
        "nvapi64.dll",
        "unexpected.bin",
    ] {
        fs::write(directory.source().join(name), b"must not be bundled").unwrap();

        build(&directory.source(), &directory.output()).unwrap();

        assert_eq!(generated(&directory), declaration, "after adding {name}");
    }
}

#[test]
fn malformed_non_dll_and_non_amd64_payloads_are_rejected() {
    let valid = dll(1);

    assert!(validate_dll(&valid).is_ok());

    for (range, bytes, reason) in [
        (0..2, b"XX".as_slice(), "MZ"),
        (64..68, b"XX\0\0".as_slice(), "PE signature"),
        (68..70, &[0x4c, 0x01], "AMD64"),
        (86..88, &[0x02, 0x00], "DLL"),
        (88..90, &[0x0b, 0x01], "PE32+"),
        (70..72, &[0, 0], "empty"),
        (348..352, &[0xff, 0xff, 0xff, 0xff], "section contents"),
    ] {
        let mut changed = valid.clone();

        changed[range].copy_from_slice(bytes);

        assert!(validate_dll(&changed).unwrap_err().contains(reason));
    }

    for length in [0, 2, 63, 87, 100, 327, 367, 527] {
        assert!(
            validate_dll(&valid[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }

    let directory = Directory::new();

    populate(&directory);

    fs::write(directory.source().join(REQUIRED[0]), b"not a library").unwrap();

    let error = build(&directory.source(), &directory.output()).unwrap_err();

    assert!(error.contains(REQUIRED[0]) && error.contains("MZ"));
}

#[test]
fn generated_rust_matches_the_runtime_contract() {
    let directory = Directory::new();

    populate(&directory);

    build(&directory.source(), &directory.output()).unwrap();

    let source = directory.output().join("contract.rs");

    fs::write(&source, r#"#![allow(dead_code)]
struct Bundle { id: &'static str, files: &'static [BundleFile] }
struct BundleFile { path: &'static str, compressed: &'static [u8], sha256: [u8; 32], uncompressed_len: u64 }
include!(concat!(env!("OUT_DIR"), "/neural_bundle.rs"));
"#).unwrap();

    let result = std::process::Command::new("rustc")
        .args(["--edition=2024", "--crate-type=lib"])
        .arg(&source)
        .arg("-o")
        .arg(directory.output().join("contract.rlib"))
        .env("OUT_DIR", directory.output())
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

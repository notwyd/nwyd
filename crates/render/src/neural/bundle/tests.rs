use super::*;
use flate2::{Compression, write::ZlibEncoder};

fn file(path: &'static str, data: &[u8]) -> BundleFile {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data).unwrap();
    BundleFile {
        path,
        compressed: Box::leak(encoder.finish().unwrap().into_boxed_slice()),
        sha256: Sha256::digest(data).into(),
        uncompressed_len: data.len() as u64,
    }
}

fn bundle() -> Bundle {
    Bundle {
        id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        files: Box::leak(
            vec![
                file("nwyd_neural.dll", b"synthetic adapter"),
                file("nvngx_dlssnr.dll", b"synthetic model"),
                file("NOTICE-neural-runtime.txt", b"synthetic attribution"),
            ]
            .into_boxed_slice(),
        ),
    }
}

fn root() -> TemporaryDirectory {
    temporary_directory(&std::env::temp_dir()).unwrap()
}

#[test]
fn native_cache_reuses_verified_payloads() {
    let root = root();
    let bundle = bundle();
    let native = install(&root.0, &bundle).unwrap();
    assert_eq!(
        fs::read(native.join("nwyd_neural.dll")).unwrap(),
        b"synthetic adapter"
    );
    assert_eq!(
        fs::read(native.join("NOTICE-neural-runtime.txt")).unwrap(),
        b"synthetic attribution"
    );
    assert!(!native.join("nvapi64.dll").exists());
    assert!(!native.join("dxgi.dll").exists());
    let model = native.join("nvngx_dlssnr.dll");
    let modified = fs::metadata(&model).unwrap().modified().unwrap();
    fs::write(native.join("runtime.log"), b"keep cache").unwrap();
    assert_eq!(install(&root.0, &bundle).unwrap(), native);
    assert_eq!(fs::metadata(model).unwrap().modified().unwrap(), modified);
    assert_eq!(fs::read(native.join("runtime.log")).unwrap(), b"keep cache");
}

#[test]
fn corrupted_missing_and_incompatible_cached_files_are_repaired() {
    let root = root();
    let bundle = bundle();
    let path = install(&root.0, &bundle).unwrap();
    fs::write(path.join("nvngx_dlssnr.dll"), b"corrupted model").unwrap();
    fs::remove_file(path.join("nwyd_neural.dll")).unwrap();
    fs::write(path.join("nvapi64.dll"), b"wrong platform").unwrap();
    fs::write(path.join("nvngx.dll"), b"forbidden core").unwrap();
    assert_eq!(install(&root.0, &bundle).unwrap(), path);
    assert_eq!(
        fs::read(path.join("nvngx_dlssnr.dll")).unwrap(),
        b"synthetic model"
    );
    assert_eq!(
        fs::read(path.join("nwyd_neural.dll")).unwrap(),
        b"synthetic adapter"
    );
    assert!(!path.join("nvapi64.dll").exists());
    assert!(!path.join("nvngx.dll").exists());
}

#[test]
fn compatibility_dlls_are_rejected_from_payloads_and_removed_from_cache() {
    let root = root();
    for name in [
        "wine/nvapi64.dll",
        "wine/dxgi.dll",
        "nvapi64.dll",
        "dxgi.dll",
    ] {
        let invalid = Bundle {
            files: Box::leak(
                vec![
                    file("nwyd_neural.dll", b"synthetic adapter"),
                    file("nvngx_dlssnr.dll", b"synthetic model"),
                    file(name, b"unwanted compatibility library"),
                ]
                .into_boxed_slice(),
            ),
            ..bundle()
        };
        assert!(install(&root.0, &invalid).is_err());
    }
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    let bundle = bundle();
    let path = install(&root.0, &bundle).unwrap();
    for name in ["nvapi64.dll", "dxgi.dll"] {
        fs::write(path.join(name), b"unwanted compatibility library").unwrap();
        assert_eq!(install(&root.0, &bundle).unwrap(), path);
        assert!(!path.join(name).exists());
        assert!(valid_cache(&path, &selected_files(&bundle).unwrap()));
    }
}

#[test]
fn unverified_or_oversized_payloads_never_publish_a_partial_runtime() {
    let root = root();
    for mismatch in [false, true] {
        let mut damaged = file("nvngx_dlssnr.dll", b"synthetic model");
        if mismatch {
            damaged.sha256 = [0; 32];
        } else {
            damaged.uncompressed_len = 1;
        }
        let bundle = Bundle {
            files: Box::leak(vec![file("nwyd_neural.dll", b"adapter"), damaged].into_boxed_slice()),
            ..bundle()
        };
        assert!(install(&root.0, &bundle).is_err());
        let parent = root.0.join(bundle.id);
        assert!(!parent.join("native").exists());
        assert!(
            fs::read_dir(parent)
                .unwrap()
                .all(|entry| { entry.unwrap().file_name() == "native.lock" })
        );
    }
}

#[test]
fn concurrent_startups_share_one_complete_cache() {
    let root = root();
    let bundle = bundle();
    std::thread::scope(|scope| {
        let threads = (0..8)
            .map(|_| scope.spawn(|| install(&root.0, &bundle).unwrap()))
            .collect::<Vec<_>>();
        let paths = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert!(paths.iter().all(|path| path == &paths[0]));
        assert!(valid_cache(&paths[0], &selected_files(&bundle).unwrap()));
    });
    assert!(fs::read_dir(root.0.join(bundle.id)).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".extract-")
    }));
}

#[test]
fn new_payload_versions_leave_existing_runtimes_untouched() {
    let root = root();
    let first = bundle();
    let second = Bundle {
        id: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        ..bundle()
    };
    let old = install(&root.0, &first).unwrap();
    let new = install(&root.0, &second).unwrap();
    assert_ne!(old, new);
    assert!(old.join("nwyd_neural.dll").is_file());
    assert!(new.join("nwyd_neural.dll").is_file());
}

#[test]
fn failed_publication_restores_the_existing_cache() {
    let root = root();
    let bundle = bundle();
    let existing = install(&root.0, &bundle).unwrap();
    fs::write(existing.join("runtime.log"), b"existing log").unwrap();
    assert!(publish(&root.0.join("missing-staging"), &existing).is_err());
    assert!(valid_cache(&existing, &selected_files(&bundle).unwrap()));
    assert_eq!(
        fs::read(existing.join("runtime.log")).unwrap(),
        b"existing log"
    );
}

#[test]
fn invalid_manifests_and_unwritable_cache_roots_fail_without_output() {
    let root = root();
    let invalid = Bundle {
        id: "../escape",
        ..bundle()
    };
    assert!(install(&root.0, &invalid).is_err());
    let invalid = Bundle {
        files: Box::leak(vec![file("../escape.dll", b"escape")].into_boxed_slice()),
        ..bundle()
    };
    assert!(install(&root.0, &invalid).is_err());
    let invalid = Bundle {
        files: Box::leak(vec![file("nwyd_neural.dll", b"adapter")].into_boxed_slice()),
        ..bundle()
    };
    assert!(install(&root.0, &invalid).is_err());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    let blocked = root.0.join("blocked");
    fs::write(&blocked, b"file instead of directory").unwrap();
    assert!(install(&blocked, &bundle()).is_err());
}

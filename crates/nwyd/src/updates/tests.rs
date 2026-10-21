use super::*;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const LOCAL_COMMIT: &str = "1111111111111111111111111111111111111111";
const REMOTE_COMMIT: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
const LINUX: &str = "x86_64-unknown-linux-gnu";

fn manifest() -> Value {
    json!({
        "schema_version": 1,
        "commit": REMOTE_COMMIT,
        "build_number": 12,
        "assets": {
            "x86_64-unknown-linux-gnu": "nwyd-x86_64-unknown-linux-gnu.tar.gz",
            "x86_64-pc-windows-msvc": "nwyd-x86_64-pc-windows-msvc.tar.gz",
            "aarch64-apple-darwin": "nwyd-aarch64-apple-darwin.tar.gz"
        }
    })
}

fn parse(value: &Value, commit: &str, build_number: u64, target: &str) -> Status {
    status_from_metadata(
        &serde_json::to_vec(value).unwrap(),
        commit,
        build_number,
        target,
    )
}

#[test]
fn newer_official_build_uses_the_current_platform_asset() {
    for target in [LINUX, "x86_64-pc-windows-msvc", "aarch64-apple-darwin"] {
        let Status::Available(update) = parse(&manifest(), LOCAL_COMMIT, 11, target) else {
            panic!("newer build must be available for {target}");
        };

        assert_eq!(update.commit, REMOTE_COMMIT);
        assert_eq!(update.build_number, 12);
        assert_eq!(
            update.download_url,
            format!("{DOWNLOAD_BASE}/{}", asset_for_target(target).unwrap())
        );
    }
}

#[test]
fn equal_or_older_builds_and_identical_commits_are_up_to_date() {
    for build_number in [12, 13, u64::MAX] {
        assert_eq!(
            parse(&manifest(), LOCAL_COMMIT, build_number, LINUX),
            Status::UpToDate
        );
    }

    for commit in [REMOTE_COMMIT.to_owned(), REMOTE_COMMIT.to_ascii_uppercase()] {
        assert_eq!(parse(&manifest(), &commit, 1, LINUX), Status::UpToDate);
    }
}

#[test]
fn local_builds_and_unsupported_targets_are_not_update_candidates() {
    for (commit, build_number, target) in [
        (LOCAL_COMMIT, 0, LINUX),
        ("unknown", 1, LINUX),
        ("12345678", 1, LINUX),
        (LOCAL_COMMIT, 1, "aarch64-unknown-linux-gnu"),
        (LOCAL_COMMIT, 1, "x86_64-apple-darwin"),
        (LOCAL_COMMIT, 1, "wasm32-unknown-unknown"),
    ] {
        assert_eq!(
            parse(&manifest(), commit, build_number, target),
            Status::Development
        );
    }
}

#[test]
fn invalid_schema_identity_or_build_number_is_rejected() {
    for (key, value) in [
        ("schema_version", json!(2)),
        ("schema_version", json!("1")),
        ("commit", json!("unknown")),
        ("commit", json!("abcdefab")),
        ("commit", json!("g".repeat(40))),
        ("build_number", json!(-1)),
        ("build_number", json!(0)),
        ("build_number", json!("12")),
    ] {
        let mut value_manifest = manifest();
        value_manifest[key] = value;

        assert_eq!(
            parse(&value_manifest, LOCAL_COMMIT, 1, LINUX),
            Status::Unavailable,
            "{value_manifest}"
        );
    }

    for key in ["schema_version", "commit", "build_number", "assets"] {
        let mut value = manifest();
        value.as_object_mut().unwrap().remove(key);

        assert_eq!(parse(&value, LOCAL_COMMIT, 1, LINUX), Status::Unavailable);
    }
}

#[test]
fn up_to_date_status_requires_valid_metadata_and_a_valid_platform_asset() {
    for (commit, build_number) in [(REMOTE_COMMIT, 1), (LOCAL_COMMIT, 12), (LOCAL_COMMIT, 13)] {
        for (key, value) in [
            ("schema_version", json!(2)),
            ("commit", json!("unknown")),
            ("build_number", json!(0)),
            ("assets", json!({})),
            (
                "assets",
                json!({LINUX: "https://example.invalid/client.exe"}),
            ),
        ] {
            let mut metadata = manifest();
            metadata[key] = value;

            assert_eq!(
                parse(&metadata, commit, build_number, LINUX),
                Status::Unavailable
            );
        }

        let mut bytes = serde_json::to_vec(&manifest()).unwrap();
        bytes.resize(MAX_METADATA_BYTES + 1, b' ');

        assert_eq!(
            status_from_metadata(&bytes, commit, build_number, LINUX),
            Status::Unavailable
        );
    }
}

#[test]
fn only_exact_published_asset_names_are_accepted() {
    for asset in [
        "https://example.invalid/client.exe",
        "../client.tar.gz",
        "nwyd-x86_64-unknown-linux-gnu.tar.gz?download=other",
        "nwyd-x86_64-pc-windows-msvc.tar.gz",
        "nwyd-x86_64-unknown-linux-gnu.tar.gz/other",
        "nwyd-x86_64-unknown-linux-gnu.tar.gz#other",
    ] {
        let mut value = manifest();
        value["assets"][LINUX] = json!(asset);

        assert_eq!(parse(&value, LOCAL_COMMIT, 1, LINUX), Status::Unavailable);
    }

    let mut value = manifest();
    value["assets"].as_object_mut().unwrap().remove(LINUX);

    assert_eq!(parse(&value, LOCAL_COMMIT, 1, LINUX), Status::Unavailable);
}

#[test]
fn malformed_truncated_or_oversized_metadata_is_rejected() {
    let mut bytes = serde_json::to_vec(&manifest()).unwrap();

    assert_eq!(
        status_from_metadata(&bytes[..bytes.len() - 1], LOCAL_COMMIT, 1, LINUX),
        Status::Unavailable
    );
    assert_eq!(
        status_from_metadata(b"<html>unavailable</html>", LOCAL_COMMIT, 1, LINUX),
        Status::Unavailable
    );

    bytes.resize(MAX_METADATA_BYTES + 1, b' ');

    assert_eq!(
        status_from_metadata(&bytes, LOCAL_COMMIT, 1, LINUX),
        Status::Unavailable
    );
}

#[test]
fn unsupported_builds_do_not_start_network_requests() {
    for (commit, number, target) in [
        (LOCAL_COMMIT, 0, Some(LINUX)),
        ("unknown", 1, Some(LINUX)),
        (LOCAL_COMMIT, 1, Some("unknown-target")),
        (LOCAL_COMMIT, 1, None),
    ] {
        let mut checker = Checker::start_with(commit, number, target, || {
            panic!("unsupported builds must not check for updates")
        });

        assert!(checker.receiver.is_none());
        assert_eq!(checker.poll(), &Status::Development);
    }
}

#[test]
fn worker_fetches_once_and_returns_an_available_update() {
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_calls = Arc::clone(&calls);
    let checker = Checker::start_with(LOCAL_COMMIT, 1, Some(LINUX), move || {
        worker_calls.fetch_add(1, Ordering::Relaxed);

        Ok(serde_json::to_vec(&manifest()).unwrap())
    });

    assert_eq!(checker.status, Status::Checking);

    let status = checker
        .receiver
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap();

    let Status::Available(update) = status else {
        panic!("newer build must be available");
    };

    assert_eq!(update.build_number, 12);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn failed_requests_report_unavailable() {
    let checker = Checker::start_with(LOCAL_COMMIT, 1, Some(LINUX), || {
        Err(io::Error::new(io::ErrorKind::TimedOut, "synthetic timeout"))
    });

    assert_eq!(
        checker
            .receiver
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap(),
        Status::Unavailable
    );
}

#[test]
fn successful_requests_distinguish_current_versions_from_invalid_metadata() {
    for (commit, bytes, expected) in [
        (
            REMOTE_COMMIT,
            serde_json::to_vec(&manifest()).unwrap(),
            Status::UpToDate,
        ),
        (
            LOCAL_COMMIT,
            b"invalid metadata".to_vec(),
            Status::Unavailable,
        ),
    ] {
        let checker = Checker::start_with(commit, 1, Some(LINUX), move || Ok(bytes));

        assert_eq!(
            checker
                .receiver
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn polling_is_nonblocking_and_keeps_the_completed_status() {
    for status in [
        parse(&manifest(), LOCAL_COMMIT, 1, LINUX),
        Status::UpToDate,
        Status::Unavailable,
    ] {
        let (sender, receiver) = channel();
        let mut checker = Checker {
            receiver: Some(receiver),
            status: Status::Checking,
        };

        assert_eq!(checker.poll(), &Status::Checking);
        assert!(checker.receiver.is_some());

        sender.send(status.clone()).unwrap();

        assert_eq!(checker.poll(), &status);
        assert!(checker.receiver.is_none());
        assert_eq!(checker.poll(), &status);
    }
}

#[test]
fn disconnected_workers_report_unavailable() {
    let (sender, receiver) = channel();
    let mut checker = Checker {
        receiver: Some(receiver),
        status: Status::Checking,
    };
    drop(sender);

    assert_eq!(checker.poll(), &Status::Unavailable);
    assert!(checker.receiver.is_none());
}

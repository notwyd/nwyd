use super::*;
use std::time::Instant;

fn event(receiver: &mpsc::Receiver<Value>) -> Value {
    let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();

    assert_eq!(request.as_object().unwrap().len(), 1);

    assert_eq!(request["eventList"].as_array().unwrap().len(), 1);

    request["eventList"][0].clone()
}

fn assert_lifecycle(event: &Value, name: &str, user_id: &str) {
    let session = event["sessionID"].as_str().unwrap();

    assert_eq!(session.len(), 36);

    for (i, byte) in session.bytes().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            assert_eq!(byte, b'-');
        } else {
            assert!(byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte));
        }
    }

    let timestamp = event["eventTimestamp"].as_str().unwrap();

    assert_eq!(timestamp.len(), 23);

    for (i, byte) in timestamp.bytes().enumerate() {
        match i {
            4 | 7 => assert_eq!(byte, b'-'),
            10 => assert_eq!(byte, b' '),
            13 | 16 => assert_eq!(byte, b':'),
            19 => assert_eq!(byte, b'.'),
            _ => assert!(byte.is_ascii_digit()),
        }
    }

    assert!(timestamp.ends_with(".000"));

    assert_eq!(
        event,
        &json!({
            "eventName": name,
            "userID": user_id,
            "sessionID": session,
            "eventTimestamp": timestamp,
            "eventParams": {"platform": "GLOBAL_SERVER"}
        })
    );
}

#[test]
fn default_config_uses_the_application_collector() {
    let config = Config::default();

    assert_eq!(config.base_url, "https://notwyd.app");

    assert_eq!(config.environment_key, "nwyd");

    assert_eq!(
        config.endpoint().unwrap().unwrap().as_str(),
        "https://notwyd.app/collect/api/nwyd"
    );
}

#[test]
fn overrides_preserve_defaults_when_absent_and_allow_disabling() {
    let defaults = Config::from_overrides(None, None);

    assert_eq!(defaults.base_url, Config::default().base_url);

    assert_eq!(defaults.environment_key, Config::default().environment_key);

    let config = Config::from_overrides(Some("https://example.com/prefix/".into()), None);

    assert_eq!(config.base_url, "https://example.com/prefix/");

    assert_eq!(config.environment_key, defaults.environment_key);

    let config = Config::from_overrides(None, Some("staging_1-test".into()));

    assert_eq!(config.base_url, defaults.base_url);

    assert_eq!(config.environment_key, "staging_1-test");

    let config = Config::from_overrides(Some(String::new()), None);

    assert!(config.endpoint().unwrap().is_none());

    assert!(configured(&config).unwrap().delivery.is_none());
}

#[test]
fn endpoint_construction_preserves_base_paths_and_uses_one_separator() {
    for (base, expected) in [
        (
            "https://example.com",
            "https://example.com/collect/api/key_1-test",
        ),
        (
            "https://example.com/",
            "https://example.com/collect/api/key_1-test",
        ),
        (
            "https://example.com/proxy///",
            "https://example.com/proxy/collect/api/key_1-test",
        ),
        (
            "https://example.com:8443/proxy",
            "https://example.com:8443/proxy/collect/api/key_1-test",
        ),
        (
            "http://127.0.0.1:1234/proxy",
            "http://127.0.0.1:1234/proxy/collect/api/key_1-test",
        ),
        (
            "http://[::1]:1234",
            "http://[::1]:1234/collect/api/key_1-test",
        ),
    ] {
        let config = Config {
            base_url: base.into(),
            environment_key: "key_1-test".into(),
        };

        assert_eq!(config.endpoint().unwrap().unwrap().as_str(), expected);
    }
}

#[test]
fn invalid_bases_are_rejected_before_delivery_starts() {
    for base in [
        "not a URL",
        "https://",
        "http://example.com/",
        "http://localhost/",
        "http://192.0.2.1/",
        "https://user@example.com/",
        "https://user:pass@example.com/",
        "file:///tmp/event",
        "ftp://example.com/",
        "https://example.com/?event=one",
        "https://example.com/#event",
    ] {
        let config = Config {
            base_url: base.into(),
            environment_key: "test".into(),
        };

        assert!(config.endpoint().is_err(), "{base}");

        assert!(configured(&config).is_err(), "{base}");
    }
}

#[test]
fn environment_keys_cannot_modify_the_url_structure() {
    for key in ["", ".", "..", "a/b", "a?b", "a#b", "%2F", "a b", "café"] {
        let config = Config {
            base_url: "https://example.com".into(),
            environment_key: key.into(),
        };

        assert!(config.endpoint().is_err(), "{key}");
    }
}

#[test]
fn login_and_end_emit_only_lifecycle_fields_with_one_session_across_accounts() {
    let (mut analytics, receiver) = Analytics::recording();

    for account in ["account_one", "account_two"] {
        analytics.login(account);

        analytics.ended();

        let started = event(&receiver);

        let ended = event(&receiver);

        assert_lifecycle(&started, "gameStarted", &account.to_ascii_uppercase());

        assert_lifecycle(&ended, "gameEnded", &account.to_ascii_uppercase());

        assert_eq!(started["sessionID"], analytics.session_id.as_ref());

        assert_eq!(ended["sessionID"], started["sessionID"]);
    }

    assert!(receiver.try_recv().is_err());
}

#[test]
fn cloned_handles_keep_their_account_identity_and_share_the_session() {
    let (mut analytics, receiver) = Analytics::recording();

    analytics.identify("first_account");

    let first = analytics.clone();

    analytics.identify("second_account");

    first.ended();

    analytics.ended();

    let first_event = event(&receiver);

    let second_event = event(&receiver);

    assert_eq!(first_event["userID"], "FIRST_ACCOUNT");

    assert_eq!(second_event["userID"], "SECOND_ACCOUNT");

    assert_eq!(first_event["sessionID"], second_event["sessionID"]);
}

#[test]
fn separate_delivery_services_have_distinct_session_ids() {
    let (first, _) = Analytics::recording();

    let (second, _) = Analytics::recording();

    assert_ne!(first.session_id, second.session_id);
}

#[test]
fn unidentified_sessions_suppress_events_until_an_account_is_identified() {
    let (mut analytics, receiver) = Analytics::recording();

    analytics.identify("");

    analytics.ended();

    analytics.login("");

    analytics.identify("synthetic_account");

    analytics.ended();

    let marker = event(&receiver);

    assert_eq!(marker["eventName"], "gameEnded");

    assert_eq!(marker["userID"], "SYNTHETIC_ACCOUNT");

    assert!(receiver.try_recv().is_err());

    let mut disabled = Analytics::default();

    disabled.login("synthetic_account");

    disabled.ended();

    assert!(disabled.delivery.is_none());
}

#[test]
fn serialization_keeps_styled_spaces_sorted_keys_and_escaped_newlines() {
    assert_eq!(
        styled(&json!({"text":"a\u{0b}\u{1f}\0ignored"})),
        br#"{   "text" : "a\u000B\u001F"}"#
    );

    assert_eq!(styled(&json!({"eventList":[{"eventName":"gameStarted","eventParams":{"platform":"GLOBAL_SERVER"}}]})),
        br#"{   "eventList" : [      {         "eventName" : "gameStarted",         "eventParams" : {            "platform" : "GLOBAL_SERVER"         }      }   ]}"#);

    assert_eq!(
        styled(&json!({"b":true,"a":"olá\n"})),
        "{   \"a\" : \"olá\\n\",   \"b\" : true}".as_bytes()
    );
}

#[test]
fn oversized_payloads_keep_the_truncation_boundary() {
    let (sender, receiver) = mpsc::channel();

    let mut analytics = Analytics::worker(move |body| {
        sender.send(body.to_vec()).unwrap();
    })
    .unwrap();

    analytics.identify("synthetic_account");

    analytics.event("large", json!({"text":"x".repeat(3000)}));

    let body = receiver.recv_timeout(Duration::from_secs(5)).unwrap();

    assert_eq!(body.len(), 2047);

    assert!(serde_json::from_slice::<Value>(&body).is_err());
}

#[test]
fn a_full_queue_drops_new_events_without_waiting_for_network_progress() {
    let (entered, wait_entered) = mpsc::channel();

    let (release, wait_release) = mpsc::channel();

    let (sent, received) = mpsc::channel();

    let mut first = true;

    let mut analytics = Analytics::worker(move |body| {
        if first {
            first = false;

            entered.send(()).unwrap();

            wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
        }

        sent.send(serde_json::from_slice::<Value>(body).unwrap())
            .unwrap();
    })
    .unwrap();

    analytics.identify("synthetic_account");

    analytics.event("blocked", json!({}));

    wait_entered.recv_timeout(Duration::from_secs(5)).unwrap();

    let pending = analytics.clone();

    let (submitted, submissions) = mpsc::channel();

    let producer = thread::spawn(move || {
        for sequence in 0..CAPACITY {
            pending.event("queued", json!({"sequence":sequence}));
        }

        pending.event("dropped", json!({}));

        submitted.send(()).unwrap();
    });

    let submission = submissions.recv_timeout(Duration::from_secs(2));

    release.send(()).unwrap();

    producer.join().unwrap();

    assert!(submission.is_ok());

    let requests: Vec<_> = (0..=CAPACITY)
        .map(|_| received.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();

    assert_eq!(requests[0]["eventList"][0]["eventName"], "blocked");

    for (sequence, request) in requests[1..].iter().enumerate() {
        assert_eq!(request["eventList"][0]["eventName"], "queued");

        assert_eq!(request["eventList"][0]["eventParams"]["sequence"], sequence);
    }

    drop(analytics);

    assert!(received.try_recv().is_err());
}

#[test]
fn loopback_delivery_posts_the_configured_path_headers_and_event_body_once() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();

    listener.set_nonblocking(true).unwrap();

    let address = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);

        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "analytics request timed out");

                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };

        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();

        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();

        let mut bytes = Vec::new();

        let mut byte = [0];

        while !bytes.ends_with(b"\r\n\r\n") {
            assert!(bytes.len() < 8192 && Instant::now() < deadline);

            socket.read_exact(&mut byte).unwrap();

            bytes.push(byte[0]);
        }

        let headers = String::from_utf8(bytes).unwrap().to_ascii_lowercase();

        assert!(headers.starts_with("post /proxy/collect/api/test_key http/1.1\r\n"));

        let header = |name| {
            headers.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;

                (key == name).then(|| value.trim())
            })
        };

        assert_eq!(header("user-agent"), Some("http"));

        assert_eq!(header("accept"), Some("*/*"));

        assert_eq!(header("content-type"), Some("application/json"));

        let length: usize = header("content-length").unwrap().parse().unwrap();

        assert!(length <= MAX_BYTES);

        let mut body = vec![0; length];

        socket.read_exact(&mut body).unwrap();

        assert!(body.starts_with(b"{   \"eventList\" : ["));

        let request: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(request["eventList"].as_array().unwrap().len(), 1);

        assert_eq!(request["eventList"][0]["eventName"], "gameEnded");

        assert_eq!(request["eventList"][0]["userID"], "SYNTHETIC_ACCOUNT");

        socket
            .write_all(b"HTTP/1.1 500 Rejected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();

        listener
    });

    let config = Config {
        base_url: format!("http://{address}/proxy/"),
        environment_key: "test_key".into(),
    };

    let mut analytics = configured(&config).unwrap();

    analytics.identify("synthetic_account");

    analytics.ended();

    let listener = server.join().unwrap();

    drop(analytics);

    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

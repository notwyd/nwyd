use super::*;
use crate::{RelayController, RelaySnapshot};
use futures_util::{Stream, stream};
use std::{net::SocketAddr, sync::Mutex};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{self, client::IntoClientRequest},
};

const ORIGIN: &str = "https://notwyd.app";
type TestSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct TestRelay {
    address: SocketAddr,
    token: String,
    state: SharedSnapshot,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<Result<(), String>>,
}

impl TestRelay {
    async fn new(allow_private_targets: bool) -> Self {
        let state = Arc::new(Mutex::new(RelaySnapshot {
            status: RelayStatus::Launching,
            listen_addr: None,
            active_sessions: 0,
            bytes_to_server: 0,
            bytes_from_server: 0,
            last_error: None,
        }));
        let config = RelayConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            allow_private_targets,
            ..RelayConfig::default()
        };
        let (stop, receiver) = watch::channel(false);
        let task = tokio::spawn(run(config, state.clone(), receiver));
        let address = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(address) = state.lock().unwrap().listen_addr {
                    break address;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("relay starts");
        let response = reqwest::Client::new()
            .get(format!("http://{address}/health"))
            .header(header::ORIGIN, ORIGIN)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let health: serde_json::Value = serde_json::from_str(&response).unwrap();
        let token = health["token"].as_str().unwrap().to_owned();
        Self {
            address,
            token,
            state,
            stop,
            task,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn proxy_url(&self, target: &str) -> String {
        let mut url = url::Url::parse(&self.url("/http")).unwrap();
        url.query_pairs_mut()
            .append_pair("token", &self.token)
            .append_pair("url", target);
        url.into()
    }

    async fn socket(&self, target: SocketAddr) -> TestSocket {
        let mut request = format!(
            "ws://{}/game?token={}&host={}&port={}",
            self.address,
            self.token,
            target.ip(),
            target.port()
        )
        .into_client_request()
        .unwrap();
        request
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        tokio_tungstenite::connect_async(request).await.unwrap().0
    }

    async fn stop(self) {
        self.stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

async fn echo_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let (mut read, mut write) = stream.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    (address, task)
}

async fn binary(socket: &mut TestSocket) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                tungstenite::Message::Binary(bytes) => return bytes.to_vec(),
                tungstenite::Message::Ping(bytes) => socket
                    .send(tungstenite::Message::Pong(bytes))
                    .await
                    .unwrap(),
                message => panic!("expected binary, got {message:?}"),
            }
        }
    })
    .await
    .unwrap()
}

async fn next_non_control(
    socket: &mut (impl Stream<Item = Result<tungstenite::Message, tungstenite::Error>> + Unpin),
) -> Option<Result<tungstenite::Message, tungstenite::Error>> {
    loop {
        match socket.next().await {
            Some(Ok(tungstenite::Message::Ping(_) | tungstenite::Message::Pong(_))) => {}
            message => return message,
        }
    }
}

#[tokio::test]
async fn shutdown_read_skips_control_frames_without_discarding_application_frames() {
    for expected in [
        tungstenite::Message::Close(None),
        tungstenite::Message::Binary(vec![0, 1, 255].into()),
        tungstenite::Message::Text("unexpected data".into()),
    ] {
        let mut messages = stream::iter([
            Ok(tungstenite::Message::Ping(vec![1].into())),
            Ok(tungstenite::Message::Pong(vec![2].into())),
            Ok(expected.clone()),
        ]);
        let received =
            tokio::time::timeout(Duration::from_secs(2), next_non_control(&mut messages))
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert_eq!(received, expected);
    }
}

#[tokio::test]
async fn shutdown_read_requires_more_than_control_frames_before_the_deadline() {
    let mut messages = stream::iter([
        Ok(tungstenite::Message::Ping(vec![1].into())),
        Ok(tungstenite::Message::Pong(vec![2].into())),
    ])
    .chain(stream::pending());
    assert!(
        tokio::time::timeout(Duration::ZERO, next_non_control(&mut messages))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn health_is_versioned_uncacheable_and_origin_host_restricted() {
    let relay = TestRelay::new(false).await;
    let client = reqwest::Client::new();
    let response = client
        .get(relay.url("/health"))
        .header(header::ORIGIN, ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        ORIGIN
    );
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let health: serde_json::Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    assert_eq!(health["app"], "nwyd-relay");
    assert_eq!(health["protocol"], 1);
    assert_eq!(relay.token.len(), 64);
    assert!(relay.token.bytes().all(|b| b.is_ascii_hexdigit()));
    for origin in [
        None,
        Some("https://evil.example"),
        Some("https://notwyd.app.evil.example"),
        Some("null"),
    ] {
        let mut request = client.get(relay.url("/health"));
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(
            !response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        );
    }
    let response = client
        .get(relay.url("/health"))
        .header(header::ORIGIN, ORIGIN)
        .header(header::HOST, "evil.example:8787")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    relay.stop().await;
}

#[tokio::test]
async fn localhost_browser_origins_are_only_allowed_in_debug_builds() {
    let relay = TestRelay::new(false).await;
    let client = reqwest::Client::new();
    for origin in [
        "http://localhost",
        "http://localhost:3000",
        "https://localhost:5173",
        "http://127.0.0.1:8081",
        "https://127.0.0.1:8443",
        "http://[::1]:3000",
        "https://[::1]:8443",
    ] {
        for (method, path, success) in [
            (Method::GET, "/health", StatusCode::OK),
            (Method::OPTIONS, "/http", StatusCode::NO_CONTENT),
        ] {
            let response = client
                .request(method, relay.url(path))
                .header(header::ORIGIN, origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .send()
                .await
                .unwrap();
            if cfg!(debug_assertions) {
                assert_eq!(response.status(), success, "{origin} {path}");
                assert_eq!(
                    response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
                    origin
                );
            } else {
                assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin} {path}");
                assert!(
                    !response
                        .headers()
                        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                );
            }
        }
    }
    let mut request = format!(
        "ws://{}/game?token=wrong&host=127.0.0.1&port=9",
        relay.address
    )
    .into_client_request()
    .unwrap();
    request.headers_mut().insert(
        header::ORIGIN,
        HeaderValue::from_static("http://localhost:3000"),
    );
    let error = tokio_tungstenite::connect_async(request).await.unwrap_err();
    let tungstenite::Error::Http(response) = error else {
        panic!("{error:?}");
    };
    assert_eq!(
        response.status(),
        if cfg!(debug_assertions) {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::FORBIDDEN
        }
    );
    relay.stop().await;
}

#[tokio::test]
async fn localhost_cors_exception_rejects_noncanonical_and_remote_origins() {
    let relay = TestRelay::new(false).await;
    let client = reqwest::Client::new();
    for origin in [
        "null",
        "localhost:3000",
        "ftp://localhost:3000",
        "http://localhost.evil.example:3000",
        "http://127.0.0.1.evil.example:3000",
        "http://192.168.1.10:3000",
        "http://[::2]:3000",
        "http://localhost:3000/",
        "http://localhost:3000/path",
        "http://localhost:3000?query=1",
        "http://localhost:3000#fragment",
        "http://user@localhost:3000",
        "http://localhost:3000 https://notwyd.app",
    ] {
        for (method, path) in [(Method::GET, "/health"), (Method::OPTIONS, "/http")] {
            let response = client
                .request(method, relay.url(path))
                .header(header::ORIGIN, origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin} {path}");
            assert!(
                !response
                    .headers()
                    .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            );
        }
    }
    relay.stop().await;
}

#[tokio::test]
async fn preflight_allows_only_supported_methods_and_headers() {
    let relay = TestRelay::new(false).await;
    let client = reqwest::Client::new();
    for (method, headers, expected) in [
        ("POST", "content-type", StatusCode::NO_CONTENT),
        ("DELETE", "content-type", StatusCode::FORBIDDEN),
        ("GET", "Authorization", StatusCode::FORBIDDEN),
    ] {
        let response = client
            .request(Method::OPTIONS, relay.url("/http"))
            .header(header::ORIGIN, ORIGIN)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, method)
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, headers)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            ORIGIN
        );
    }
    relay.stop().await;
}

#[tokio::test]
async fn token_and_private_destination_checks_precede_connect() {
    let relay = TestRelay::new(false).await;
    let client = reqwest::Client::new();
    let response = client
        .get(relay.url("/http?token=wrong&url=http%3A%2F%2F127.0.0.1"))
        .header(header::ORIGIN, ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = client
        .get(relay.proxy_url("http://127.0.0.1:9/private"))
        .header(header::ORIGIN, ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    for (token, expected) in [
        ("wrong", StatusCode::UNAUTHORIZED),
        (relay.token.as_str(), StatusCode::BAD_REQUEST),
    ] {
        let mut request = format!(
            "ws://{}/game?token={token}&host=127.0.0.1&port=9",
            relay.address
        )
        .into_client_request()
        .unwrap();
        request
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        let error = tokio_tungstenite::connect_async(request).await.unwrap_err();
        let tungstenite::Error::Http(response) = error else {
            panic!("{error:?}");
        };
        assert_eq!(response.status(), expected);
    }
    relay.stop().await;
}

#[tokio::test]
async fn multiple_sessions_forward_binary_independently_and_shutdown_together() {
    let relay = TestRelay::new(true).await;
    let (target, echo) = echo_server().await;
    let mut first = relay.socket(target).await;
    let mut second = relay.socket(target).await;
    first
        .send(tungstenite::Message::Binary(vec![0, 1, 255, 128].into()))
        .await
        .unwrap();
    second
        .send(tungstenite::Message::Binary(
            b"second session".to_vec().into(),
        ))
        .await
        .unwrap();
    assert_eq!(binary(&mut first).await, [0, 1, 255, 128]);
    assert_eq!(binary(&mut second).await, b"second session");
    {
        let snapshot = relay.state.lock().unwrap();
        assert_eq!(snapshot.active_sessions, 2);
        assert_eq!(snapshot.bytes_to_server, 18);
        assert_eq!(snapshot.bytes_from_server, 18);
    }
    let state = relay.state.clone();
    relay.stop().await;
    assert_eq!(state.lock().unwrap().active_sessions, 0);
    for socket in [&mut first, &mut second] {
        let closed = tokio::time::timeout(Duration::from_secs(2), next_non_control(socket))
            .await
            .unwrap();
        assert!(
            matches!(
                closed,
                Some(Ok(tungstenite::Message::Close(_))) | None | Some(Err(_))
            ),
            "expected shutdown, got {closed:?}"
        );
    }
    echo.abort();
}

#[tokio::test]
async fn server_eof_closes_browser_after_final_bytes() {
    let relay = TestRelay::new(true).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream.write_all(&[5, 6, 7]).await.unwrap();
        stream.shutdown().await.unwrap();
    });
    let mut socket = relay.socket(address).await;
    assert_eq!(binary(&mut socket).await, [5, 6, 7]);
    let closed = tokio::time::timeout(Duration::from_secs(2), next_non_control(&mut socket))
        .await
        .unwrap();
    assert!(matches!(closed, Some(Ok(tungstenite::Message::Close(_)))));
    upstream.await.unwrap();
    relay.stop().await;
}

#[tokio::test]
async fn text_frames_and_oversized_messages_close_the_session() {
    let relay = TestRelay::new(true).await;
    let (target, echo) = echo_server().await;
    for message in [
        tungstenite::Message::Text("invalid".into()),
        tungstenite::Message::Binary(vec![0; MAX_MESSAGE_BYTES + 1].into()),
    ] {
        let mut socket = relay.socket(target).await;
        let _ = socket.send(message).await;
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(Ok(message)) = socket.next().await {
                if matches!(message, tungstenite::Message::Close(_)) {
                    break;
                }
                assert!(!matches!(message, tungstenite::Message::Binary(_)));
            }
        })
        .await
        .unwrap();
    }
    relay.stop().await;
    echo.abort();
}

#[tokio::test]
async fn http_get_form_post_redirects_and_limits() {
    let relay = TestRelay::new(true).await;
    let upstream = Router::new()
        .route(
            "/body",
            any(|request: Request| async move {
                if request.method() == Method::POST {
                    (
                        StatusCode::CREATED,
                        [(header::CONTENT_TYPE, "text/plain")],
                        to_bytes(request.into_body(), MAX_HTTP_REQUEST_BYTES)
                            .await
                            .unwrap(),
                    )
                        .into_response()
                } else {
                    (StatusCode::OK, "response bytes").into_response()
                }
            }),
        )
        .route(
            "/redirect",
            get(|| async { (StatusCode::FOUND, [(header::LOCATION, "/body")]) }),
        )
        .route(
            "/bad-redirect",
            get(|| async {
                (
                    StatusCode::FOUND,
                    [(header::LOCATION, "file:///etc/passwd")],
                )
            }),
        )
        .route(
            "/large",
            get(|| async { vec![0u8; MAX_HTTP_RESPONSE_BYTES + 1] }),
        )
        .route(
            "/chunked-large",
            get(|| async {
                Body::from_stream(futures_util::stream::iter([
                    Ok::<_, std::io::Error>(vec![0u8; 3 * 1024 * 1024]),
                    Ok(vec![0u8; 3 * 1024 * 1024]),
                ]))
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let service = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let client = reqwest::Client::new();
    for path in ["/body", "/redirect"] {
        let response = client
            .get(relay.proxy_url(&format!("http://{address}{path}")))
            .header(header::ORIGIN, ORIGIN)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "response bytes");
    }
    let response = client
        .post(relay.proxy_url(&format!("http://{address}/body")))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("id=42&name=player")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.text().await.unwrap(), "id=42&name=player");
    for path in ["/large", "/chunked-large"] {
        let response = client
            .get(relay.proxy_url(&format!("http://{address}{path}")))
            .header(header::ORIGIN, ORIGIN)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }
    let response = client
        .get(relay.proxy_url(&format!("http://{address}/bad-redirect")))
        .header(header::ORIGIN, ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = client
        .post(relay.proxy_url(&format!("http://{address}/body")))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(vec![0u8; MAX_HTTP_REQUEST_BYTES + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let response = client
        .post(relay.proxy_url(&format!("http://{address}/body")))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    relay.stop().await;
    service.abort();
}

#[tokio::test]
async fn controller_reports_bind_failure_retries_and_stops() {
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = occupied.local_addr().unwrap();
    let controller = RelayController::new(RelayConfig {
        listen_addr: address,
        ..RelayConfig::default()
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(controller.snapshot().status, RelayStatus::Failed(_)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(occupied);
    controller.retry();
    tokio::time::timeout(Duration::from_secs(5), async {
        while controller.snapshot().status != RelayStatus::Ready {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(controller.snapshot().listen_addr, Some(address));
    controller.shutdown();
    tokio::time::timeout(Duration::from_secs(5), async {
        while controller.snapshot().status != RelayStatus::Stopped {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

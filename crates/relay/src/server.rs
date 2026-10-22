use crate::{
    PROTOCOL_VERSION, RelayConfig, RelayStatus,
    policy::{self, MAX_HTTP_REQUEST_BYTES, MAX_HTTP_RESPONSE_BYTES, MAX_MESSAGE_BYTES},
    state::{SharedSnapshot, io_error_message},
};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{
        Query, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, watch},
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const IO_TIMEOUT: Duration = Duration::from_secs(20);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(25);
const PEER_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone)]
struct AppState {
    config: RelayConfig,
    port: u16,
    token: Arc<str>,
    snapshot: SharedSnapshot,
    shutdown: watch::Receiver<bool>,
    game_slots: Arc<Semaphore>,
    http_slots: Arc<Semaphore>,
}

pub(crate) async fn wait_for_shutdown(mut shutdown: watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow() || shutdown.changed().await.is_err() {
            return;
        }
    }
}

pub(crate) async fn run(
    config: RelayConfig,
    snapshot: SharedSnapshot,
    shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    if !config.listen_addr.ip().is_loopback() {
        return Err("O relay deve aceitar conexões apenas em um endereço local (loopback).".into());
    }
    if config.allowed_origins.is_empty()
        || config.allowed_origins.iter().any(|origin| {
            let Ok(url) = url::Url::parse(origin) else {
                return true;
            };
            !matches!(url.scheme(), "http" | "https")
                || url.origin().ascii_serialization() != *origin
        })
    {
        return Err("Configure origens HTTP ou HTTPS exatas para o relay.".into());
    }
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .map_err(|error| {
            io_error_message(
                &format!(
                    "Não foi possível aceitar conexões em {}",
                    config.listen_addr
                ),
                &error,
            )
        })?;
    let address = listener.local_addr().map_err(|error| {
        io_error_message(
            "Não foi possível identificar o endereço local do relay",
            &error,
        )
    })?;
    let token: String = rand::random::<[u8; 32]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let state = AppState {
        config,
        port: address.port(),
        token: token.into(),
        snapshot: snapshot.clone(),
        shutdown: shutdown.clone(),
        game_slots: Arc::new(Semaphore::new(64)),
        http_slots: Arc::new(Semaphore::new(16)),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/game", get(game))
        .route("/http", any(http_proxy))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            browser_boundary,
        ))
        .with_state(state);
    {
        let mut snapshot = snapshot.lock().unwrap_or_else(|e| e.into_inner());
        snapshot.listen_addr = Some(address);
        snapshot.status = RelayStatus::Ready;
    }
    let server_shutdown = shutdown.clone();
    let mut serving = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(wait_for_shutdown(server_shutdown))
            .await
    });
    let result = tokio::select! {
        result = &mut serving => Some(result),
        _ = wait_for_shutdown(shutdown) => {
            match tokio::time::timeout(Duration::from_secs(2), &mut serving).await {
                Ok(result) => Some(result),
                Err(_) => { serving.abort(); None }
            }
        }
    };
    if let Some(result) = result {
        result
            .map_err(|_| "O servidor do relay parou inesperadamente.".to_owned())?
            .map_err(|error| io_error_message("Falha ao receber conexões no relay", &error))?;
    }
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .active_sessions
                == 0
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    Ok(())
}

fn failure(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(serde_json::json!({ "error": message.into() }))).into_response()
}

async fn browser_boundary(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host.is_some_and(|host| policy::valid_host_header(host, state.port)) {
        return failure(StatusCode::FORBIDDEN, "Endereço do relay inválido.");
    }
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let Some(origin) = origin.filter(|origin| state.config.allows_origin(origin)) else {
        return failure(
            StatusCode::FORBIDDEN,
            "Este site não tem permissão para usar o relay.",
        );
    };
    let origin = HeaderValue::from_str(origin).expect("a valid request header");
    let preflight = request.method() == Method::OPTIONS;
    let mut response = if preflight {
        let method = request
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(|v| v.to_str().ok());
        let headers = request
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
            .and_then(|v| v.to_str().ok());
        if !matches!(method, Some("GET" | "POST"))
            || headers.is_some_and(|headers| {
                headers
                    .split(',')
                    .any(|h| !h.trim().eq_ignore_ascii_case("content-type"))
            })
        {
            failure(
                StatusCode::FORBIDDEN,
                "A verificação de permissão enviada ao relay não é compatível.",
            )
        } else {
            StatusCode::NO_CONTENT.into_response()
        }
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if preflight {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST"),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Content-Type"),
        );
        headers.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
    }
    response
}

#[derive(Serialize)]
struct Health<'a> {
    app: &'static str,
    protocol: u32,
    version: &'static str,
    token: &'a str,
}

async fn health(State(state): State<AppState>) -> Response {
    Json(Health {
        app: "nwyd-relay",
        protocol: PROTOCOL_VERSION,
        version: env!("CARGO_PKG_VERSION"),
        token: &state.token,
    })
    .into_response()
}

#[derive(Deserialize)]
struct GameQuery {
    token: String,
    host: String,
    port: u16,
}

async fn game(
    State(state): State<AppState>,
    Query(query): Query<GameQuery>,
    websocket: WebSocketUpgrade,
) -> Response {
    if !policy::token_matches(&state.token, &query.token) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "O relay foi reiniciado. Conecte novamente pela página do jogo.",
        );
    }
    let Ok(permit) = state.game_slots.clone().try_acquire_owned() else {
        return failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Há sessões demais abertas. Feche uma sessão e tente novamente.",
        );
    };
    let addresses =
        match policy::resolve_target(&query.host, query.port, state.config.allow_private_targets)
            .await
        {
            Ok(addresses) => addresses,
            Err(error) => {
                state
                    .snapshot
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .last_error = Some(error.clone());
                return failure(StatusCode::BAD_REQUEST, error);
            }
        };
    let stream = match tokio::time::timeout(
        CONNECT_TIMEOUT,
        TcpStream::connect(addresses.as_slice()),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        _ => {
            let message = "Não foi possível conectar ao servidor do jogo. Verifique sua conexão e se o servidor está disponível.";
            state
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .last_error = Some(message.into());
            return failure(StatusCode::BAD_GATEWAY, message);
        }
    };
    let _ = stream.set_nodelay(true);
    websocket
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .write_buffer_size(64 * 1024)
        .max_write_buffer_size(MAX_MESSAGE_BYTES + 64 * 1024)
        .on_upgrade(move |websocket| async move {
            let _permit = permit;
            relay_game(state, websocket, stream).await;
        })
        .into_response()
}

struct SessionGuard(SharedSnapshot);
impl Drop for SessionGuard {
    fn drop(&mut self) {
        let mut snapshot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        snapshot.active_sessions = snapshot.active_sessions.saturating_sub(1);
    }
}

async fn relay_game(state: AppState, websocket: WebSocket, stream: TcpStream) {
    state
        .snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .active_sessions += 1;
    let _session = SessionGuard(state.snapshot.clone());
    let (mut ws_write, mut ws_read) = websocket.split();
    let (mut tcp_read, mut tcp_write) = stream.into_split();
    let receive = async {
        loop {
            let message = tokio::time::timeout(PEER_TIMEOUT, ws_read.next())
                .await
                .map_err(|_| "A sessão do navegador expirou.")?;
            match message {
                Some(Ok(Message::Binary(bytes))) => {
                    tokio::time::timeout(IO_TIMEOUT, tcp_write.write_all(&bytes))
                        .await
                        .map_err(|_| "O servidor do jogo parou de receber dados.")?
                        .map_err(
                            |_| "A conexão com o jogo foi encerrada durante o envio de dados.",
                        )?;
                    state
                        .snapshot
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .bytes_to_server += bytes.len() as u64;
                }
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Text(_))) => {
                    return Err("A conexão com o jogo exige mensagens binárias.");
                }
                Some(Err(_)) => {
                    return Err("A conexão com o navegador foi encerrada inesperadamente.");
                }
            }
        }
    };
    let send = async {
        let mut buffer = vec![0u8; 64 * 1024];
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let message = tokio::select! {
                bytes = tcp_read.read(&mut buffer) => {
                    let length = bytes.map_err(|_| "A conexão com o jogo foi encerrada durante o recebimento de dados.")?;
                    if length == 0 { return Ok(()); }
                    state.snapshot.lock().unwrap_or_else(|e| e.into_inner()).bytes_from_server += length as u64;
                    Message::Binary(buffer[..length].to_vec().into())
                }
                _ = heartbeat.tick() => Message::Ping(Vec::new().into()),
            };
            tokio::time::timeout(IO_TIMEOUT, ws_write.send(message))
                .await
                .map_err(|_| "O navegador parou de receber dados do jogo.")?
                .map_err(
                    |_| "A conexão com o navegador foi encerrada durante o recebimento de dados.",
                )?;
        }
    };
    let result: Result<(), &str> = tokio::select! {
        _ = wait_for_shutdown(state.shutdown.clone()) => Ok(()),
        result = receive => result,
        result = send => result,
    };
    if let Err(error) = result {
        state
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_error = Some(error.into());
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        if ws_write.close().await.is_err() {
            return;
        }
        while let Some(Ok(message)) = ws_read.next().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    })
    .await;
}

#[derive(Deserialize)]
struct HttpQuery {
    token: String,
    url: String,
}

async fn http_proxy(
    State(state): State<AppState>,
    Query(query): Query<HttpQuery>,
    request: Request,
) -> Response {
    if !policy::token_matches(&state.token, &query.token) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "O relay foi reiniciado. Conecte novamente pela página do jogo.",
        );
    }
    let Ok(_permit) = state.http_slots.clone().try_acquire_owned() else {
        return failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Há solicitações demais aos serviços do jogo. Tente novamente em instantes.",
        );
    };
    let method = request.method().clone();
    if method != Method::GET && method != Method::POST {
        return failure(
            StatusCode::METHOD_NOT_ALLOWED,
            "Apenas GET e POST de formulário são aceitos.",
        );
    }
    if method == Method::POST
        && !request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|content_type| {
                content_type.split(';').next().is_some_and(|mime| {
                    mime.trim()
                        .eq_ignore_ascii_case("application/x-www-form-urlencoded")
                })
            })
    {
        return failure(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "O conteúdo do POST deve estar no formato de formulário.",
        );
    }
    let body = match tokio::time::timeout(
        IO_TIMEOUT,
        to_bytes(request.into_body(), MAX_HTTP_REQUEST_BYTES),
    )
    .await
    {
        Ok(Ok(body)) => body,
        Ok(Err(_)) => {
            return failure(
                StatusCode::PAYLOAD_TOO_LARGE,
                "A solicitação HTTP é grande demais.",
            );
        }
        Err(_) => {
            return failure(
                StatusCode::REQUEST_TIMEOUT,
                "O tempo da solicitação HTTP esgotou.",
            );
        }
    };
    match tokio::time::timeout(
        Duration::from_secs(30),
        forward_http(&state, &query.url, method, body.to_vec()),
    )
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err((status, error))) => failure(status, error),
        Err(_) => failure(
            StatusCode::GATEWAY_TIMEOUT,
            "O serviço do jogo demorou demais para responder.",
        ),
    }
}

type ProxyError = (StatusCode, String);

async fn forward_http(
    state: &AppState,
    target: &str,
    mut method: Method,
    mut body: Vec<u8>,
) -> Result<Response, ProxyError> {
    let invalid = |message: &str| (StatusCode::BAD_REQUEST, message.to_owned());
    let upstream_error = |_| {
        (
            StatusCode::BAD_GATEWAY,
            "A solicitação ao serviço do jogo falhou.".into(),
        )
    };
    if target.len() > 8192 {
        return Err(invalid("A URL do serviço do jogo é longa demais."));
    }
    let mut target =
        url::Url::parse(target).map_err(|_| invalid("A URL do serviço do jogo é inválida."))?;
    for redirects in 0..=4 {
        if !matches!(target.scheme(), "http" | "https")
            || !target.username().is_empty()
            || target.password().is_some()
            || target.fragment().is_some()
        {
            return Err(invalid(
                "Apenas serviços do jogo HTTP ou HTTPS sem credenciais na URL são aceitos.",
            ));
        }
        let host = target
            .host_str()
            .ok_or_else(|| invalid("O endereço do serviço do jogo não foi informado."))?;
        let port = target
            .port_or_known_default()
            .ok_or_else(|| invalid("A porta do serviço do jogo não foi informada."))?;
        let addresses = policy::resolve_target(host, port, state.config.allow_private_targets)
            .await
            .map_err(|error| invalid(&error))?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &addresses)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(upstream_error)?;
        let mut request = client.request(method.clone(), target.clone());
        if method == Method::POST {
            request = request
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body.clone());
        }
        let response = request.send().await.map_err(upstream_error)?;
        let status = response.status();
        if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
            if redirects == 4 {
                return Err((
                    StatusCode::BAD_GATEWAY,
                    "O serviço do jogo redirecionou a solicitação vezes demais.".into(),
                ));
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|h| h.to_str().ok())
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_GATEWAY,
                        "O redirecionamento do serviço do jogo é inválido.".into(),
                    )
                })?;
            let next_target = target
                .join(location)
                .map_err(|_| invalid("A URL de redirecionamento do serviço do jogo é inválida."))?;
            if target.scheme() == "https" && next_target.scheme() != "https" {
                return Err(invalid(
                    "Um serviço seguro do jogo não pode redirecionar para uma URL sem segurança.",
                ));
            }
            target = next_target;
            if status == StatusCode::SEE_OTHER
                || ((status == StatusCode::MOVED_PERMANENTLY || status == StatusCode::FOUND)
                    && method == Method::POST)
            {
                method = Method::GET;
                body.clear();
            }
            continue;
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_HTTP_RESPONSE_BYTES as u64)
        {
            return Err((
                StatusCode::BAD_GATEWAY,
                "A resposta do serviço do jogo é grande demais.".into(),
            ));
        }
        let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(upstream_error)?;
            if chunk.len() > MAX_HTTP_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                return Err((
                    StatusCode::BAD_GATEWAY,
                    "A resposta do serviço do jogo é grande demais.".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let mut response = (status, Body::from(bytes)).into_response();
        if let Some(content_type) = content_type {
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, content_type);
        }
        return Ok(response);
    }
    unreachable!("redirect limit checked inside loop")
}

#[cfg(test)]
mod tests;

//! Local authenticated seat API. All core execution stays in match processes.
//! Axum0.8.9 routing/extractors; Tokio bounded connection/queue ownership.
use crate::{
    auth::CredentialHashes,
    broker::{Broker, BrokerError},
    delivery::{CSP, Delivery, OriginPolicy},
    ipc::{WorkerCommand, WorkerErrorCode, WorkerReply},
    protocol::{
        self, CommandDecoder, CommandExecutionStatus, PublicManifest, RejectionReason,
        SubmissionResult,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Path, Query, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

const REQUEST_LIMIT: usize = 64;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(25);
pub struct HostedMatch {
    pub credentials: CredentialHashes,
    pub manifest: PublicManifest,
    pub broker: Broker,
    polls: [Arc<Semaphore>; 4],
}
impl HostedMatch {
    pub fn new(
        credentials: CredentialHashes,
        manifest: PublicManifest,
        broker: Broker,
    ) -> Result<Self, String> {
        if credentials.match_id != manifest.match_id || !valid_match_id(&manifest.match_id) {
            return Err("match identity mismatch".into());
        }
        Ok(Self {
            credentials,
            manifest,
            broker,
            polls: std::array::from_fn(|_| Arc::new(Semaphore::new(1))),
        })
    }
}
struct App {
    matches: BTreeMap<String, Arc<HostedMatch>>,
    decoder: CommandDecoder,
    requests: Arc<Semaphore>,
    delivery: Delivery,
}
#[derive(Clone)]
pub struct Host(Arc<App>);
impl Host {
    pub fn new(matches: Vec<HostedMatch>) -> Result<Self, String> {
        Self::with_delivery(matches, Delivery::default())
    }
    pub fn with_delivery(matches: Vec<HostedMatch>, delivery: Delivery) -> Result<Self, String> {
        if matches.is_empty() || matches.len() > 2 {
            return Err("hosting admission requires one or two matches".into());
        }
        let mut map = BTreeMap::new();
        for item in matches {
            if map
                .insert(item.manifest.match_id.clone(), Arc::new(item))
                .is_some()
            {
                return Err("duplicate match identity".into());
            }
        }
        Ok(Self(Arc::new(App {
            matches: map,
            delivery,
            decoder: CommandDecoder::new()?,
            requests: Arc::new(Semaphore::new(REQUEST_LIMIT)),
        })))
    }
    pub fn router(self) -> Router {
        Router::new()
            .route(protocol::COMMAND_ROUTE, post(submit))
            .route(protocol::EVENTS_ROUTE, get(events))
            .route(protocol::COMMAND_STATUS_ROUTE, get(execution))
            .route(protocol::COMMAND_CANCEL_ROUTE, post(cancel))
            .route(protocol::MANIFEST_ROUTE, get(manifest))
            .route(protocol::RESUME_ROUTE, get(resume))
            .route(protocol::PRESENTATION_ROUTE, get(presentation))
            .fallback(public_asset)
            .layer(middleware::from_fn_with_state(self.clone(), bound_request))
            .with_state(self)
    }
    /// All externally served routers use the configured origin policy, including
    /// API-only operator mode. The unwrapped router is for in-process tests.
    pub fn router_at(self, address: std::net::SocketAddr) -> Result<Router, String> {
        let policy = OriginPolicy::new(address)?;
        let config = serde_json::to_string(&policy.config).map_err(|_| "invalid client config")?;
        Ok(self
            .router()
            .route(
                protocol::CLIENT_CONFIG_ROUTE,
                get(move |request: Request| {
                    let config = config.clone();
                    async move {
                        if !matches!(read_body(request.into_body(), 0).await, Ok(bytes) if bytes.is_empty()) { return failure(StatusCode::BAD_REQUEST); }
                        response(StatusCode::OK, config)
                    }
                }),
            )
            .layer(middleware::from_fn_with_state(policy, browser_boundary)))
    }
    pub fn stop(&self) {
        for item in self.0.matches.values() {
            item.broker.stop();
        }
    }
    fn authenticate(
        &self,
        id: &str,
        headers: &HeaderMap,
    ) -> Result<(Arc<HostedMatch>, u8), StatusCode> {
        if !valid_match_id(id) {
            return Err(StatusCode::NOT_FOUND);
        }
        let item = self.0.matches.get(id).ok_or(StatusCode::NOT_FOUND)?.clone();
        let mut values = headers.get_all(header::AUTHORIZATION).iter();
        let value = values
            .next()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if values.next().is_some() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let seat = item
            .credentials
            .authenticate_seat(value)
            .ok_or(StatusCode::UNAUTHORIZED)?;
        Ok((item, seat))
    }
}
fn valid_match_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_command_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn response(status: StatusCode, body: String) -> Response {
    (
        status,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        body,
    )
        .into_response()
}
fn failure(status: StatusCode) -> Response {
    response(status, "{\"error\":\"request_failed\"}".into())
}
fn json<T: Serialize>(value: &T) -> Response {
    match serde_json::to_string(value) {
        Ok(body) => response(StatusCode::OK, body),
        Err(_) => failure(StatusCode::SERVICE_UNAVAILABLE),
    }
}
fn broker_failure(error: BrokerError) -> Response {
    failure(match error {
        BrokerError::Busy => StatusCode::TOO_MANY_REQUESTS,
        BrokerError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}
fn worker_failure(code: WorkerErrorCode) -> Response {
    failure(match code {
        WorkerErrorCode::Conflict => StatusCode::CONFLICT,
        WorkerErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        WorkerErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}
async fn bound_request(State(host): State<Host>, request: Request, next: Next) -> Response {
    let Ok(_permit) = host.0.requests.clone().try_acquire_owned() else {
        return failure(StatusCode::TOO_MANY_REQUESTS);
    };
    tokio::time::timeout(REQUEST_TIMEOUT, next.run(request))
        .await
        .unwrap_or_else(|_| failure(StatusCode::SERVICE_UNAVAILABLE))
}
async fn read_body(body: Body, limit: usize) -> Result<axum::body::Bytes, StatusCode> {
    tokio::time::timeout(Duration::from_secs(5), to_bytes(body, limit))
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)
}
async fn submit(State(host): State<Host>, Path(id): Path<String>, request: Request) -> Response {
    let (item, seat) = match host.authenticate(&id, request.headers()) {
        Ok(value) => value,
        Err(status) => return failure(status),
    };
    if request.uri().query().is_some()
        || request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|v| v.split(';').next() != Some("application/json"))
    {
        return failure(StatusCode::BAD_REQUEST);
    }
    let body = match read_body(request.into_body(), protocol::MAX_COMMAND_BYTES).await {
        Ok(value) => value,
        Err(status) => return failure(status),
    };
    let command = match std::str::from_utf8(&body)
        .ok()
        .and_then(|text| host.0.decoder.decode(text).ok())
    {
        Some(command) => command,
        None => return failure(StatusCode::BAD_REQUEST),
    };
    match item
        .broker
        .call(WorkerCommand::Submit {
            seat,
            request: command,
        })
        .await
    {
        Ok(WorkerReply::Submission { result, .. }) => json(&result),
        Ok(WorkerReply::Error { code }) => worker_failure(code),
        Err(error) => broker_failure(error),
        _ => failure(StatusCode::SERVICE_UNAVAILABLE),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventQuery {
    after: String,
    limit: Option<u32>,
    wait_ms: Option<u32>,
}
async fn events(
    State(host): State<Host>,
    Path(id): Path<String>,
    Query(query): Query<EventQuery>,
    headers: HeaderMap,
) -> Response {
    let (item, seat) = match host.authenticate(&id, &headers) {
        Ok(value) => value,
        Err(status) => return failure(status),
    };
    let (after, limit, wait) = match (
        protocol::Counter::try_from(query.after),
        query.limit.unwrap_or(32),
        query.wait_ms.unwrap_or(0),
    ) {
        (Ok(after), limit, wait) if (1..=32).contains(&limit) && wait <= 20_000 => {
            (after.get(), limit, wait)
        }
        _ => return failure(StatusCode::BAD_REQUEST),
    };
    let Ok(_poll) = item.polls[usize::from(seat)].clone().try_acquire_owned() else {
        return failure(StatusCode::TOO_MANY_REQUESTS);
    };
    let end = tokio::time::Instant::now() + Duration::from_millis(u64::from(wait));
    loop {
        match item
            .broker
            .call(WorkerCommand::Page { seat, after, limit })
            .await
        {
            Ok(WorkerReply::Page { page }) => {
                if !page.entries.is_empty() || tokio::time::Instant::now() >= end {
                    return json(&page);
                }
            }
            Ok(WorkerReply::Error { code }) => return worker_failure(code),
            Err(error) => return broker_failure(error),
            _ => return failure(StatusCode::SERVICE_UNAVAILABLE),
        }
        tokio::time::sleep_until(std::cmp::min(
            end,
            tokio::time::Instant::now() + Duration::from_millis(100),
        ))
        .await;
    }
}
async fn execution(
    State(host): State<Host>,
    Path((id, command_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let (item, seat) = match host.authenticate(&id, &headers) {
        Ok(value) => value,
        Err(status) => return failure(status),
    };
    if !valid_command_id(&command_id) {
        return failure(StatusCode::BAD_REQUEST);
    }
    match item
        .broker
        .call(WorkerCommand::Execution { seat, command_id })
        .await
    {
        Ok(WorkerReply::Execution {
            execution: Some(execution),
        }) => json(&execution),
        Ok(WorkerReply::Execution { execution: None }) => failure(StatusCode::NOT_FOUND),
        Ok(WorkerReply::Error { code }) => worker_failure(code),
        Err(error) => broker_failure(error),
        _ => failure(StatusCode::SERVICE_UNAVAILABLE),
    }
}
async fn cancel(
    State(host): State<Host>,
    Path((id, command_id)): Path<(String, String)>,
    request: Request,
) -> Response {
    let (item, seat) = match host.authenticate(&id, request.headers()) {
        Ok(value) => value,
        Err(status) => return failure(status),
    };
    if !valid_command_id(&command_id) || request.uri().query().is_some() {
        return failure(StatusCode::BAD_REQUEST);
    }
    match read_body(request.into_body(), 0).await {
        Ok(body) if body.is_empty() => {}
        _ => return failure(StatusCode::BAD_REQUEST),
    }
    match item.broker.cancel(seat, &command_id).await {
        Ok(Some(WorkerReply::Submission { result, .. })) => return json(&result),
        Ok(Some(WorkerReply::Error { code })) => return worker_failure(code),
        Ok(None) => {}
        Err(error) => return broker_failure(error),
        _ => return failure(StatusCode::SERVICE_UNAVAILABLE),
    }
    // Already-sent commands keep their immutable submission result. This route
    // never infers an undo from a later execution status.
    match item
        .broker
        .call(WorkerCommand::Execution {
            seat,
            command_id: command_id.clone(),
        })
        .await
    {
        Ok(WorkerReply::Execution {
            execution: Some(execution),
        }) => {
            let result = match execution.status {
                CommandExecutionStatus::Pending
                | CommandExecutionStatus::ConsumedByRules
                | CommandExecutionStatus::DiscardedByDeparture => {
                    SubmissionResult::Accepted { command_id }
                }
                CommandExecutionStatus::Cancelled => SubmissionResult::Rejected {
                    command_id,
                    reason: RejectionReason::Cancelled,
                },
                CommandExecutionStatus::Rejected { reason } => {
                    SubmissionResult::Rejected { command_id, reason }
                }
            };
            json(&result)
        }
        Ok(WorkerReply::Execution { execution: None }) => failure(StatusCode::NOT_FOUND),
        Ok(WorkerReply::Error { code }) => worker_failure(code),
        Err(error) => broker_failure(error),
        _ => failure(StatusCode::SERVICE_UNAVAILABLE),
    }
}
async fn manifest(
    State(host): State<Host>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    match host.authenticate(&id, &headers) {
        Ok((item, _)) => json(&item.manifest),
        Err(status) => failure(status),
    }
}

async fn resume(State(host): State<Host>, Path(id): Path<String>, request: Request) -> Response {
    let (item, seat) = match host.authenticate(&id, request.headers()) {
        Ok(value) => value,
        Err(status) => return failure(status),
    };
    if request.uri().query().is_some() {
        return failure(StatusCode::BAD_REQUEST);
    }
    match read_body(request.into_body(), 0).await {
        Ok(body) if body.is_empty() => {}
        _ => return failure(StatusCode::BAD_REQUEST),
    }
    match item.broker.call(WorkerCommand::Resume { seat }).await {
        Ok(WorkerReply::Resume { resume }) => match protocol::checked_resume(&resume) {
            Ok(body) => response(StatusCode::OK, body),
            Err(_) => failure(StatusCode::SERVICE_UNAVAILABLE),
        },
        Ok(WorkerReply::Error { code }) => worker_failure(code),
        Err(error) => broker_failure(error),
        _ => failure(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn presentation(
    State(host): State<Host>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    let (item, _) = match host.authenticate(&id, request.headers()) {
        Ok(v) => v,
        Err(s) => return failure(s),
    };
    if request.uri().query().is_some() {
        return failure(StatusCode::BAD_REQUEST);
    }
    if !matches!(read_body(request.into_body(), 0).await, Ok(bytes) if bytes.is_empty()) {
        return failure(StatusCode::BAD_REQUEST);
    }
    match host.0.delivery.descriptor(&item.manifest.catalog_sha256) {
        Some(v) => json(v),
        None => failure(StatusCode::NOT_FOUND),
    }
}
async fn public_asset(State(host): State<Host>, request: Request) -> Response {
    if request.uri().query().is_some() {
        return failure(StatusCode::BAD_REQUEST);
    }
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return failure(StatusCode::METHOD_NOT_ALLOWED);
    }
    let path = request.uri().path().to_owned();
    let method = request.method().clone();
    if !matches!(read_body(request.into_body(), 0).await, Ok(bytes) if bytes.is_empty()) {
        return failure(StatusCode::BAD_REQUEST);
    }
    host.0
        .delivery
        .response(&path, &method)
        .unwrap_or_else(|| failure(StatusCode::NOT_FOUND))
}
async fn browser_boundary(
    State(policy): State<OriginPolicy>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let rejected = if !policy.permits(request.headers()) {
        Some(StatusCode::FORBIDDEN)
    } else if request.uri().authority().is_some()
        || path.len() > 512
        || path.contains('%')
        || path.contains('\\')
        || path.contains("//")
        || path.split('/').any(|p| p == "." || p == "..")
    {
        Some(StatusCode::BAD_REQUEST)
    } else if path.starts_with("/v1/") && request.method() == Method::HEAD {
        Some(StatusCode::METHOD_NOT_ALLOWED)
    } else if path == protocol::CLIENT_CONFIG_ROUTE
        && (request.uri().query().is_some() || request.method() != Method::GET)
    {
        Some(StatusCode::BAD_REQUEST)
    } else {
        None
    };
    let mut result = if let Some(status) = rejected {
        failure(status)
    } else {
        next.run(request).await
    };
    // Axum extractor/method errors also get bounded generic JSON, never echoed
    // input, cacheability, or a missing browser security policy.
    if !result.status().is_success() {
        result = failure(result.status());
    }
    let headers = result.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        CSP.parse().expect("static CSP"),
    );
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    headers.insert(
        "cross-origin-resource-policy",
        "same-origin".parse().unwrap(),
    );
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    result
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn delivery_boundary_enforces_security_and_bounded_errors_over_http() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new()
            .route("/", get(|| async { response(StatusCode::OK, "{}".into()) }))
            .route(
                "/v1/read",
                get(|| async { response(StatusCode::OK, "{}".into()) }),
            )
            .fallback(|| async { failure(StatusCode::NOT_FOUND) })
            .layer(middleware::from_fn_with_state(
                OriginPolicy::new(address).unwrap(),
                browser_boundary,
            ));
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        for (target, host, origin, status) in [
            ("/", address.to_string(), None, 200),
            ("/", "rebind.invalid".into(), None, 403),
            ("/", address.to_string(), Some("null"), 403),
            ("/", address.to_string(), Some("http://evil.example"), 403),
            ("/%2e%2e/operator.json", address.to_string(), None, 400),
            ("/operator.json", address.to_string(), None, 404),
            ("/assets/no.js", address.to_string(), None, 404),
        ] {
            let mut connection = tokio::net::TcpStream::connect(address).await.unwrap();
            let extra = origin
                .map(|v| format!("Origin: {v}\r\n"))
                .unwrap_or_default();
            connection
                .write_all(
                    format!(
                        "GET {target} HTTP/1.1\r\nHost: {host}\r\n{extra}Connection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            let mut raw = String::new();
            connection.read_to_string(&mut raw).await.unwrap();
            assert!(raw.starts_with(&format!("HTTP/1.1 {status}")), "{raw}");
            assert!(raw.contains("cache-control: no-store\r\n"));
            assert!(raw.contains("content-security-policy: default-src 'none'"));
            assert!(!raw.contains("access-control-allow-origin"));
            assert!(raw.contains("x-content-type-options: nosniff"));
            if status != 200 {
                assert!(raw.ends_with("{\"error\":\"request_failed\"}"));
            }
        }
        task.abort();
        let _ = task.await;
    }
}

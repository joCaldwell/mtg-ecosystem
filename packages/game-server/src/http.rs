//! Local authenticated seat API. All core execution stays in match processes.
//! Axum0.8.9 routing/extractors; Tokio bounded connection/queue ownership.
use crate::{
    auth::CredentialHashes,
    broker::{Broker, BrokerError},
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
    http::{HeaderMap, StatusCode, header},
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
}
#[derive(Clone)]
pub struct Host(Arc<App>);
impl Host {
    pub fn new(matches: Vec<HostedMatch>) -> Result<Self, String> {
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
            .fallback(|| async { failure(StatusCode::NOT_FOUND) })
            .layer(middleware::from_fn_with_state(self.clone(), bound_request))
            .with_state(self)
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

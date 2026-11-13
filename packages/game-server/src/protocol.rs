//! Rust DTOs are the wire authority; core projections are embedded directly.
use mtg_core::{DecisionToken, Observation, PlayerResponse};
use schemars::{JsonSchema, Schema, generate::SchemaSettings};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_COMMAND_BYTES: usize = 65_536;
pub const MAX_RESUME_BYTES: usize = 131_072;
pub const MAX_BOOTSTRAP_BYTES: usize = 4096;
pub const JS_MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

/// Shared endpoint authority for the Rust router and generated client adapter.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct RouteSpec {
    pub operation: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub response_schema: &'static str,
}
pub const COMMAND_ROUTE: &str = "/v1/matches/{match_id}/commands";
pub const EVENTS_ROUTE: &str = "/v1/matches/{match_id}/events";
pub const COMMAND_STATUS_ROUTE: &str = "/v1/matches/{match_id}/commands/{command_id}";
pub const COMMAND_CANCEL_ROUTE: &str = "/v1/matches/{match_id}/commands/{command_id}/cancel";
pub const MANIFEST_ROUTE: &str = "/v1/matches/{match_id}/manifest";
pub const RESUME_ROUTE: &str = "/v1/matches/{match_id}/resume";
pub const PRESENTATION_ROUTE: &str = "/v1/matches/{match_id}/presentation";
pub const CLIENT_CONFIG_ROUTE: &str = "/client-config.json";
pub const ACTION_QUERY_ROUTE: &str = "/v1/matches/{match_id}/actions/query";
pub const ROUTES: [RouteSpec; 9] = [
    RouteSpec {
        operation: "query_actions",
        method: "POST",
        path: ACTION_QUERY_ROUTE,
        response_schema: "action_query",
    },
    RouteSpec {
        operation: "get_client_config",
        method: "GET",
        path: CLIENT_CONFIG_ROUTE,
        response_schema: "client_config",
    },
    RouteSpec {
        operation: "get_presentation",
        method: "GET",
        path: PRESENTATION_ROUTE,
        response_schema: "presentation_descriptor",
    },
    RouteSpec {
        operation: "get_resume",
        method: "GET",
        path: RESUME_ROUTE,
        response_schema: "resume",
    },
    RouteSpec {
        operation: "get_manifest",
        method: "GET",
        path: MANIFEST_ROUTE,
        response_schema: "manifest",
    },
    RouteSpec {
        operation: "submit_command",
        method: "POST",
        path: COMMAND_ROUTE,
        response_schema: "submission",
    },
    RouteSpec {
        operation: "poll_events",
        method: "GET",
        path: EVENTS_ROUTE,
        response_schema: "stream",
    },
    RouteSpec {
        operation: "command_execution",
        method: "GET",
        path: COMMAND_STATUS_ROUTE,
        response_schema: "execution",
    },
    RouteSpec {
        operation: "cancel_command",
        method: "POST",
        path: COMMAND_CANCEL_ROUTE,
        response_schema: "submission",
    },
];

// Logical work is a conservative allowance, never a CPU or completeness promise.
pub const MAX_ACTION_REQUEST_BYTES: usize = 2048;
pub const MAX_ACTION_RESPONSE_BYTES: usize = 65_536;
pub const MAX_ACTION_WORK: u32 = 1000;
pub const MAX_ACTION_PAGE: u16 = 32;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionQueryRequest {
    Start {
        #[schemars(length(min = 1, max = 128), regex(pattern = "^[!-~]+$"))]
        token: DecisionToken,
        scope: mtg_core::ActionScope,
        #[schemars(range(min = 1, max = 32))]
        page_size: u16,
        #[schemars(range(min = 1, max = 1000))]
        work: u32,
    },
    Continue {
        #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
        cursor: String,
        #[schemars(range(min = 1, max = 1000))]
        work: u32,
    },
    Cancel {
        #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
        cursor: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionQueryResponse {
    #[schemars(range(min = 1, max = 1))]
    pub protocol_version: u32,
    pub match_id: String,
    #[schemars(range(min = 0, max = 3))]
    pub viewer: mtg_core::PlayerId,
    pub outcome: ActionQueryStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionQueryStatus {
    Complete {
        page: mtg_core::ActionPage,
    },
    Incomplete {
        page: mtg_core::ActionPage,
        #[schemars(range(min = 1, max = 1))]
        required_minimum: u64,
        #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
        cursor: String,
    },
    Stale,
    Unavailable {
        reason: ActionQueryUnavailable,
    },
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionQueryUnavailable {
    Core { reason: mtg_core::ActionUnavailable },
    Quota,
    InFlight,
    TotalWork,
    ResponseLimit,
    Executor,
}
pub fn action_request_schema() -> Schema {
    let mut value = serde_json::to_value(schema::<ActionQueryRequest>(false)).expect("schema");
    // ObjectHandle is core-owned; transport bounds this scoped input only.
    if let Some(variants) = value["definitions"]["ActionScope"]["oneOf"].as_array_mut() {
        for variant in variants {
            if let Some(object) = variant["properties"].get_mut("object") {
                object["minLength"] = 1.into();
                object["maxLength"] = 128.into();
                object["pattern"] = "^[!-~]+$".into();
            }
        }
    }
    Schema::try_from(value).expect("schema object")
}
pub fn action_query_schema() -> Schema {
    let mut value = serde_json::to_value(safe_numeric_schema(schema::<ActionQueryResponse>(true)))
        .expect("schema");
    value["definitions"]["ActionPage"]["properties"]["beginnings"]["maxItems"] =
        MAX_ACTION_PAGE.into();
    Schema::try_from(value).expect("schema object")
}
pub fn decode_action_request(json: &str) -> Result<ActionQueryRequest, String> {
    if json.len() > MAX_ACTION_REQUEST_BYTES {
        return Err("action request exceeds bound".into());
    }
    let value: ActionQueryRequest = crate::codec::exact(json)?;
    let cursor_valid = |text: &str| {
        text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    let valid = match &value {
        ActionQueryRequest::Start {
            token,
            scope,
            page_size,
            work,
        } => {
            let token = serde_json::to_value(token).map_err(|e| e.to_string())?;
            let scope = serde_json::to_value(scope).map_err(|e| e.to_string())?;
            let object = scope.get("object").and_then(Value::as_str);
            token.as_str().is_some_and(|s| {
                !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_graphic())
            }) && object.is_none_or(|s| {
                !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_graphic())
            }) && (1..=MAX_ACTION_PAGE).contains(page_size)
                && (1..=MAX_ACTION_WORK).contains(work)
        }
        ActionQueryRequest::Continue { cursor, work } => {
            cursor_valid(cursor) && (1..=MAX_ACTION_WORK).contains(work)
        }
        ActionQueryRequest::Cancel { cursor } => cursor_valid(cursor),
    };
    if !valid {
        return Err("invalid action query bounds".into());
    }
    Ok(value)
}
pub fn checked_actions(value: &ActionQueryResponse) -> Result<String, String> {
    let encoded = checked_public_json(value)?;
    if encoded.len() > MAX_ACTION_RESPONSE_BYTES {
        return Err("action response exceeds bound".into());
    }
    Ok(encoded)
}

/// Artifact location/integrity only; the offline producer owns its JSON shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PresentationDescriptor {
    #[schemars(range(min = 1, max = 1))]
    pub presentation_version: u32,
    #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
    pub catalog_sha256: String,
    #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
    pub presentation_sha256: String,
    #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
    pub presentation_schema_sha256: String,
    #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
    pub producer_sha256: String,
    #[schemars(
        length(min = 83, max = 83),
        regex(pattern = "^/presentation/[0-9a-f]{64}\\.json$")
    )]
    pub path: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    #[schemars(range(min = 1, max = 1))]
    pub client_config_version: u32,
    #[schemars(
        length(max = 64),
        regex(pattern = "^http://(127\\.0\\.0\\.1|\\[::1\\]):[1-9][0-9]{0,4}$")
    )]
    pub server_origin: String,
}
pub fn presentation_descriptor_schema() -> Schema {
    schema::<PresentationDescriptor>(true)
}
pub fn client_config_schema() -> Schema {
    schema::<ClientConfig>(true)
}

/// Only public open-decklist facts. No seed, private profile digest or state ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicManifest {
    pub protocol_version: u32,
    pub match_id: String,
    pub engine_build_id: String,
    pub server_build_id: String,
    pub catalog_sha256: String,
    pub profile: String,
    pub decks: [mtg_core::Decklist; 4],
    pub starting_player: mtg_core::PlayerId,
    pub catalog: mtg_contracts::Catalog,
}
impl PublicManifest {
    pub fn new(
        match_id: String,
        catalog: mtg_contracts::Catalog,
        config: &mtg_core::MatchConfig,
    ) -> Result<Self, String> {
        Ok(Self {
            protocol_version: PROTOCOL_VERSION,
            match_id,
            engine_build_id: mtg_core::BUILD_ID.into(),
            server_build_id: crate::SERVER_BUILD_ID.to_string(),
            catalog_sha256: mtg_contracts::digest(&catalog)?,
            profile: config.profile.clone(),
            decks: config.decks.clone(),
            starting_player: config.starting_player,
            catalog,
        })
    }
}
pub fn manifest_schema() -> Schema {
    schema::<PublicManifest>(true)
}

/// Transport counters serialize as decimal strings, separate from core counters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Counter(u64);
impl JsonSchema for Counter {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "DecimalU64Counter".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> Schema {
        // All canonical widths below20, or a20-digit value lexically <=u64::MAX.
        // Each alternative first differs from the maximum at exactly one digit.
        let maximum = u64::MAX.to_string();
        let mut alternatives = vec!["0".to_owned(), "[1-9][0-9]{0,18}".to_owned()];
        for (index, byte) in maximum.bytes().enumerate() {
            let lower = if index == 0 { b'1' } else { b'0' };
            if byte > lower {
                alternatives.push(format!(
                    "{}[{}-{}][0-9]{{{}}}",
                    &maximum[..index],
                    char::from(lower),
                    char::from(byte - 1),
                    maximum.len() - index - 1
                ));
            }
        }
        alternatives.push(maximum);
        let pattern = format!("^({})$", alternatives.join("|"));
        schemars::json_schema!({"type":"string", "pattern":pattern, "maxLength":20})
    }
}

impl Counter {
    pub fn new(value: u64) -> Self {
        Self(value)
    }
    pub fn get(&self) -> u64 {
        self.0
    }
    pub fn checked_next(&self) -> Result<Self, String> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or("transport counter exhausted".into())
    }
}
impl TryFrom<String> for Counter {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        let value: u64 = text.parse().map_err(|_| "invalid decimal counter")?;
        if value.to_string() != text {
            return Err("noncanonical decimal counter".into());
        }
        Ok(Self(value))
    }
}
impl From<Counter> for String {
    fn from(value: Counter) -> Self {
        value.0.to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub protocol_version: u32,
    #[schemars(length(min = 32, max = 32), regex(pattern = "^[0-9a-f]{32}$"))]
    pub command_id: String,
    pub expected_revision: Counter,
    pub command: CommandBody,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandBody {
    Decision {
        token: DecisionToken,
        response: PlayerResponse,
    },
    Concede,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubmissionResult {
    Accepted {
        command_id: String,
    },
    Rejected {
        command_id: String,
        reason: RejectionReason,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    StaleRevision,
    InvalidResponse,
    AlreadyFinished,
    ResourcePaused,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionStatus {
    Queued,
    Running,
    Waiting,
    Applied,
    Cancelled,
    Rejected,
    ResourcePaused,
    Faulted,
    Finished,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandExecutionStatus {
    Pending,
    ConsumedByRules,
    DiscardedByDeparture,
    Cancelled,
    Rejected { reason: RejectionReason },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandExecution {
    pub command_id: String,
    pub status: CommandExecutionStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Publication {
    Outcome { result: RulesResult },
    ViewReplaced { observation: Box<Observation> },
    Lifecycle { status: ExecutionStatus },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum RulesResult {
    Win { player: mtg_core::PlayerId },
    Draw,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StreamEntry {
    pub cursor: Counter,
    pub publication: Publication,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StreamPage {
    pub protocol_version: u32,
    pub entries: Vec<StreamEntry>,
    pub next: Counter,
    pub has_more: bool,
}

/// One coherent committed own-stream boundary. No query is run to refresh it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeatResume {
    #[schemars(range(min = 1, max = 1))]
    pub protocol_version: u32,
    pub match_id: String,
    pub viewer: mtg_core::PlayerId,
    pub engine_build_id: String,
    pub server_build_id: String,
    pub catalog_sha256: String,
    /// Last included own publication cursor; poll strictly after this value.
    pub next: Counter,
    pub state: ResumeState,
    /// Last published lifecycle, not the worker's private scheduling phase.
    pub lifecycle: Option<ExecutionStatus>,
    pub outcome: Option<RulesResult>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResumeState {
    Pending,
    Available { observation: Box<Observation> },
}
/// Operator file; never log or derive Debug for capability-bearing data.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeatBootstrap {
    #[schemars(range(min = 1, max = 1))]
    pub bootstrap_version: u32,
    #[schemars(range(min = 1, max = 1))]
    pub protocol_version: u32,
    #[schemars(
        length(max = 64),
        regex(pattern = "^http://(127\\.0\\.0\\.1|\\[::1\\]):[1-9][0-9]{0,4}$")
    )]
    pub server_origin: String,
    #[schemars(length(min = 1, max = 64), regex(pattern = "^[0-9a-f]+$"))]
    pub match_id: String,
    #[schemars(range(min = 0, max = 3))]
    pub viewer: mtg_core::PlayerId,
    #[schemars(length(min = 64, max = 64), regex(pattern = "^[0-9a-f]{64}$"))]
    pub capability: String,
    pub engine_build_id: String,
    pub server_build_id: String,
    pub catalog_sha256: String,
}
pub fn bootstrap_schema() -> Schema {
    schema::<SeatBootstrap>(false)
}
pub fn resume_schema() -> Schema {
    safe_numeric_schema(schema::<SeatResume>(true))
}

fn schema<T: JsonSchema>(outbound: bool) -> Schema {
    // Draft7 has tuple items supported by our generated TypeScript consumer.
    // https://docs.rs/schemars/1.2.3/schemars/generate/struct.SchemaSettings.html
    let settings = SchemaSettings::draft07();
    let settings = if outbound {
        settings.for_serialize()
    } else {
        settings.for_deserialize()
    };
    settings.into_generator().into_root_schema_for::<T>()
}
pub fn command_schema() -> Schema {
    schema::<CommandEnvelope>(false)
}
pub fn stream_schema() -> Schema {
    safe_numeric_schema(schema::<StreamPage>(true))
}
fn safe_numeric_schema(input: Schema) -> Schema {
    let mut schema = serde_json::to_value(input).expect("generated schema");
    fn constrain(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                let integer = fields.get("type").is_some_and(|t| {
                    t == "integer"
                        || t.as_array()
                            .is_some_and(|a| a.iter().any(|t| t == "integer"))
                });
                if integer {
                    if fields
                        .get("maximum")
                        .and_then(Value::as_f64)
                        .is_none_or(|n| n > JS_MAX_SAFE_INTEGER as f64)
                    {
                        fields.insert("maximum".into(), JS_MAX_SAFE_INTEGER.into());
                    }
                    if fields
                        .get("minimum")
                        .and_then(Value::as_f64)
                        .is_none_or(|n| n < -(JS_MAX_SAFE_INTEGER as f64))
                    {
                        fields.insert("minimum".into(), (-(JS_MAX_SAFE_INTEGER as i64)).into());
                    }
                }
                for child in fields.values_mut() {
                    constrain(child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    constrain(child);
                }
            }
            _ => {}
        }
    }
    constrain(&mut schema);
    Schema::try_from(schema).expect("generated schema object")
}
pub fn submission_schema() -> Schema {
    schema::<SubmissionResult>(true)
}
pub fn execution_schema() -> Schema {
    schema::<CommandExecution>(true)
}

/// Reused validator with no remote-reference features or request-controlled schemas.
pub struct CommandDecoder {
    validator: jsonschema::Validator,
}
impl CommandDecoder {
    pub fn new() -> Result<Self, String> {
        let schema = serde_json::to_value(command_schema()).map_err(|e| e.to_string())?;
        let validator = jsonschema::draft7::options()
            .build(&schema)
            .map_err(|e| e.to_string())?;
        Ok(Self { validator })
    }
    pub fn decode(&self, json: &str) -> Result<CommandEnvelope, String> {
        if json.len() > MAX_COMMAND_BYTES {
            return Err("command body exceeds limit".into());
        }
        // UniqueJson checks nested duplicates BEFORE Value/tagged-enum decoding.
        let value: Value = mtg_contracts::strict_read(json)?;
        self.validator
            .validate(&value)
            .map_err(|_| "command schema mismatch")?;
        let command: CommandEnvelope = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if command.protocol_version != PROTOCOL_VERSION {
            return Err("unsupported protocol version".into());
        }
        Ok(command)
    }
}
/// Existing core u64 fields really serialize as JSON numbers. Pause publication
/// before exceeding JavaScript's exact range rather than falsifying their schema.
pub fn checked_publication(value: &StreamPage) -> Result<String, String> {
    checked_public_json(value)
}
pub fn checked_resume(value: &SeatResume) -> Result<String, String> {
    let encoded = checked_public_json(value)?;
    if encoded.len() > MAX_RESUME_BYTES {
        return Err("resume exceeds bound".into());
    }
    Ok(encoded)
}
fn checked_public_json(value: &impl Serialize) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    fn check(value: &Value) -> Result<(), String> {
        match value {
            Value::Number(n)
                if n.as_u64().is_some_and(|n| n > JS_MAX_SAFE_INTEGER)
                    || n.as_i64()
                        .is_some_and(|n| n < -(JS_MAX_SAFE_INTEGER as i64)) =>
            {
                Err("core integer exceeds JavaScript safe range; resource pause required".into())
            }
            Value::Array(values) => {
                for value in values {
                    check(value)?;
                }
                Ok(())
            }
            Value::Object(values) => {
                for value in values.values() {
                    check(value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    check(&value)?;
    mtg_contracts::canonical(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn command(response: Value) -> Value {
        json!({"protocol_version":1,"command_id":"0123456789abcdef0123456789abcdef","expected_revision":"0","command":{"kind":"decision","token":"viewer-token","response":response}})
    }
    #[test]
    fn resume_schema_rejects_extra_fields_versions_and_size_is_inclusive() {
        let mut resume = SeatResume {
            protocol_version: 1,
            match_id: "a1".into(),
            viewer: mtg_core::PlayerId(0),
            engine_build_id: "e".into(),
            server_build_id: "s".into(),
            catalog_sha256: "c".into(),
            next: Counter::new(u64::MAX),
            state: ResumeState::Pending,
            lifecycle: None,
            outcome: None,
        };
        let validator = jsonschema::draft7::options()
            .build(&serde_json::to_value(resume_schema()).unwrap())
            .unwrap();
        let original = serde_json::to_value(&resume).unwrap();
        validator.validate(&original).unwrap();
        for field in ["seed", "profile_sha256", "viewer_revision", "work"] {
            let mut invalid = original.clone();
            invalid[field] = json!(0);
            assert!(!validator.is_valid(&invalid));
        }
        let mut invalid = original.clone();
        invalid["protocol_version"] = json!(2);
        assert!(!validator.is_valid(&invalid));
        let mut invalid = original;
        invalid["state"]["observation"] = json!(null);
        assert!(!validator.is_valid(&invalid));
        let length = checked_resume(&resume).unwrap().len();
        resume
            .match_id
            .push_str(&"x".repeat(MAX_RESUME_BYTES - length));
        assert_eq!(checked_resume(&resume).unwrap().len(), MAX_RESUME_BYTES);
        resume.match_id.push('x');
        assert!(checked_resume(&resume).is_err());
    }
    #[test]
    fn command_boundary_rejects_duplicates_unknown_fields_and_protocol_mismatch() {
        let decoder = CommandDecoder::new().unwrap();
        let base = command(json!({"response":"pass"}));
        decoder.decode(&base.to_string()).unwrap();
        let mut extra = base.clone();
        extra["command"]["response"]["seed"] = json!(1);
        assert!(decoder.decode(&extra.to_string()).is_err());
        let mut extra = base.clone();
        extra["command"]["extra"] = json!(1);
        assert!(decoder.decode(&extra.to_string()).is_err());
        let duplicate = base.to_string().replace(
            "\"response\":\"pass\"",
            "\"response\":\"cast\",\"response\":\"pass\"",
        );
        assert!(decoder.decode(&duplicate).is_err());
        let mut wrong = base.clone();
        wrong["protocol_version"] = json!(2);
        assert!(decoder.decode(&wrong.to_string()).is_err());
        let mut wrong = base.clone();
        wrong["command_id"] = json!("short");
        assert!(decoder.decode(&wrong.to_string()).is_err());
        let mut wrong = base.clone();
        wrong["expected_revision"] = json!(0);
        assert!(decoder.decode(&wrong.to_string()).is_err());
        let mut wrong = base.clone();
        wrong["command"]["kind"] = json!("raw_receipt");
        assert!(decoder.decode(&wrong.to_string()).is_err());
        assert!(decoder.decode(&" ".repeat(MAX_COMMAND_BYTES + 1)).is_err());
    }
    #[test]
    fn counters_are_exact_canonical_strings_and_overflow_is_explicit() {
        for n in [0, 1, JS_MAX_SAFE_INTEGER, JS_MAX_SAFE_INTEGER + 1, u64::MAX] {
            let counter = Counter::new(n);
            let encoded = serde_json::to_string(&counter).unwrap();
            assert_eq!(encoded, format!("\"{n}\""));
            assert_eq!(serde_json::from_str::<Counter>(&encoded).unwrap(), counter);
        }
        for invalid in ["01", "-0", "+1", " 1", "1 ", "1.0", "18446744073709551616"] {
            assert!(serde_json::from_value::<Counter>(json!(invalid)).is_err());
        }
        assert!(Counter::new(u64::MAX).checked_next().is_err());
        let schema = serde_json::to_value(schema::<Counter>(false)).unwrap();
        let validator = jsonschema::draft7::options().build(&schema).unwrap();
        for text in [
            "0",
            "1",
            "9999999999999999999",
            "10000000000000000000",
            "18446744073709551614",
            "18446744073709551615",
        ] {
            assert!(validator.is_valid(&json!(text)), "{text}");
        }
        for text in [
            "01",
            "-1",
            "18446744073709551616",
            "18446744073709551619",
            "18446744073709551620",
            "18446744073709552000",
            "19000000000000000000",
            "99999999999999999999",
        ] {
            assert!(!validator.is_valid(&json!(text)), "{text}");
        }
    }
    #[test]
    fn schema_and_serde_agree_on_damage_tuples_and_exact_quantities() {
        let decoder = CommandDecoder::new().unwrap();
        let base = command(json!({"response":"damage","assignments":[["h1","3"]]}));
        decoder.decode(&base.to_string()).unwrap();
        for assignments in [
            json!([["h1"]]),
            json!([["h1", "3", "extra"]]),
            json!([["h1", 3]]),
            json!([["h1", "01"]]),
        ] {
            let mut wrong = base.clone();
            wrong["command"]["response"]["assignments"] = assignments;
            assert!(decoder.decode(&wrong.to_string()).is_err());
        }
        let schema = serde_json::to_value(command_schema()).unwrap();
        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        assert!(!schema.to_string().contains("prefixItems"));
    }
    #[test]
    fn outbound_cursor_remains_exact_without_numeric_json() {
        let page = StreamPage {
            protocol_version: 1,
            entries: vec![StreamEntry {
                cursor: Counter::new(u64::MAX),
                publication: Publication::Lifecycle {
                    status: ExecutionStatus::Running,
                },
            }],
            next: Counter::new(u64::MAX),
            has_more: false,
        };
        let encoded = checked_publication(&page).unwrap();
        let value: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["next"], u64::MAX.to_string());
        let schema = serde_json::to_value(stream_schema()).unwrap();
        jsonschema::draft7::options()
            .build(&schema)
            .unwrap()
            .validate(&value)
            .unwrap();
    }
}

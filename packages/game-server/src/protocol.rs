//! Rust DTOs are the wire authority; core projections are embedded directly.
use mtg_core::{DecisionToken, Observation, PlayerResponse};
use schemars::{JsonSchema, Schema, generate::SchemaSettings};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_COMMAND_BYTES: usize = 65_536;
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
pub const ROUTES: [RouteSpec; 5] = [
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

/// Only public open-decklist facts. No seed, private profile digest or state ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicManifest {
    pub protocol_version: u32,
    pub match_id: String,
    pub engine_build_id: String,
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
    let mut schema = serde_json::to_value(schema::<StreamPage>(true)).expect("generated schema");
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

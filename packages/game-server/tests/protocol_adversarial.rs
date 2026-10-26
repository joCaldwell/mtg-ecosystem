//! Independent codec tests; these structural DTOs are not admitted game states.
use mtg_server::protocol::{
    CommandDecoder, Counter, JS_MAX_SAFE_INTEGER, MAX_COMMAND_BYTES, StreamPage,
    checked_publication, command_schema, stream_schema, submission_schema,
};
use schemars::generate::SchemaSettings;
use serde_json::{Value, json};

fn envelope(response: Value) -> Value {
    json!({
        "protocol_version":1,
        "command_id":"0123456789abcdef0123456789abcdef",
        "expected_revision":"0",
        "command":{"kind":"decision","token":"viewer-local-token","response":response}
    })
}
fn validator(schema: Value) -> jsonschema::Validator {
    jsonschema::draft7::options().build(&schema).unwrap()
}
fn canonical_counter(text: &str) -> bool {
    text.parse::<u64>().is_ok_and(|n| n.to_string() == text)
}

#[test]
fn decimal_counter_schema_matches_canonical_rust_u64_over_deterministic_u128_samples() {
    let schema = SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<Counter>();
    let schema = validator(serde_json::to_value(schema).unwrap());
    let check = |text: String| {
        let value = json!(text);
        let expected = canonical_counter(value.as_str().unwrap());
        assert_eq!(schema.is_valid(&value), expected, "schema: {value}");
        assert_eq!(
            serde_json::from_value::<Counter>(value.clone()).is_ok(),
            expected,
            "serde: {value}"
        );
    };
    for width in 0..=38_u32 {
        let n = 10_u128.pow(width);
        for value in [n.saturating_sub(1), n, n.saturating_add(1)] {
            check(value.to_string());
        }
    }
    for offset in 0..=1024_u128 {
        check((u128::from(u64::MAX) - offset).to_string());
        check((u128::from(u64::MAX) + offset).to_string());
    }
    let mut state = 0x6a09e667f3bcc908bb67ae8584caa73b_u128;
    for _ in 0..2048 {
        state = state
            .wrapping_mul(0x2360ed051fc65da44385df649fccf645)
            .wrapping_add(0x9e3779b97f4a7c15);
        check(state.to_string());
        check((state as u64).to_string());
        check(format!("0{}", state as u64));
        check((10_000_000_000_000_000_000_u128 + state % 10_000_000_000_000_000_000).to_string());
    }
    for text in [
        "", "+0", "-0", "-1", "1e0", "1.0", " 0", "0 ", "０", "١", "1\n", "\n1",
    ] {
        check(text.into());
    }
    assert!(Counter::new(u64::MAX).checked_next().is_err());
}

#[test]
fn recursive_duplicates_and_unknown_unit_or_nested_response_fields_reject() {
    let decoder = CommandDecoder::new().unwrap();
    let base = envelope(json!({"response":"pass"}));
    decoder.decode(&base.to_string()).unwrap();
    for path in [
        vec!["unreviewed"],
        vec!["command", "unreviewed"],
        vec!["command", "response", "unreviewed"],
    ] {
        let mut value = base.clone();
        let mut object = &mut value;
        for component in &path[..path.len() - 1] {
            object = &mut object[*component];
        }
        object[path[path.len() - 1]] = json!({"seed":[1,2,3]});
        assert!(decoder.decode(&value.to_string()).is_err());
    }
    let duplicates = [
        base.to_string().replace(
            "\"protocol_version\":1",
            "\"protocol_version\":1,\"protocol_version\":1",
        ),
        base.to_string().replace(
            "\"response\":\"pass\"",
            "\"response\":\"cast\",\"response\":\"pass\"",
        ),
        envelope(json!({"response":"spend_mana","spending":{"W":"1"}}))
            .to_string()
            .replace("\"W\":\"1\"", "\"W\":\"1\",\"W\":\"1\""),
    ];
    for value in duplicates {
        assert!(decoder.decode(&value).is_err(), "{value}");
    }
    let mut concede = base.clone();
    concede["command"] = json!({"kind":"concede"});
    decoder.decode(&concede.to_string()).unwrap();
    concede["command"]["token"] = json!("invented-chooser-authority");
    assert!(decoder.decode(&concede.to_string()).is_err());
}

#[test]
fn command_body_limit_is_bytes_and_deep_or_malformed_json_cannot_bypass_it() {
    let decoder = CommandDecoder::new().unwrap();
    let base = envelope(json!({"response":"pass"})).to_string();
    let padded = format!("{base}{}", " ".repeat(MAX_COMMAND_BYTES - base.len()));
    assert_eq!(padded.len(), MAX_COMMAND_BYTES);
    decoder.decode(&padded).unwrap();
    assert!(decoder.decode(&(padded + " ")).is_err());
    let mut unicode = envelope(json!({"response":"pass"}));
    unicode["command"]["token"] = "é".repeat(MAX_COMMAND_BYTES / 2).into();
    let unicode = unicode.to_string();
    assert!(unicode.chars().count() < MAX_COMMAND_BYTES && unicode.len() > MAX_COMMAND_BYTES);
    assert!(decoder.decode(&unicode).is_err());
    let deep = format!("{}0{}", "[".repeat(256), "]".repeat(256));
    assert!(deep.len() < MAX_COMMAND_BYTES && decoder.decode(&deep).is_err());
    for raw in ["null", "[]", "{}", "{", "true", "1", "{} {}"] {
        assert!(decoder.decode(raw).is_err());
    }
}

fn page(number: u64, expiry: u64) -> StreamPage {
    serde_json::from_value(json!({
        "protocol_version":1,
        "entries":[{"cursor":u64::MAX.to_string(),"publication":{"kind":"view_replaced","observation":{
            "observation_version":15,
            "view":{
                "combat":[],"viewer":0,"active":0,"turn_owner":0,
                "turn":{"number":number,"step":"precombat_main","lands_played":0},
                "live_turn_order":[0,1,2,3],"commanders":[],"priority":0,
                "players":[{"player":0,"status":"live","life":"-9007199254740992","mana":{"W":"9007199254740992"},"hand_count":0,"library_count":0}],
                "objects":[],"stack":[],
                "effects":[{"effect":"prevent_combat_damage","handle":"viewer-effect","origin":{"source":{"announced":"old-source","current":null},"oracle_id":"public-oracle","controller":0},"expires_in_cleanup_of_turn":expiry}]
            },
            "decision":{"request":"priority","token":"viewer-token","player":0}
        }}}],
        "next":u64::MAX.to_string(),"has_more":false
    })).unwrap()
}

#[test]
fn public_numeric_schema_and_publication_guard_agree_at_the_js_safe_edge() {
    let validate = validator(serde_json::to_value(stream_schema()).unwrap());
    let safe = page(JS_MAX_SAFE_INTEGER, JS_MAX_SAFE_INTEGER);
    let encoded = checked_publication(&safe).unwrap();
    let value: Value = serde_json::from_str(&encoded).unwrap();
    assert!(validate.is_valid(&value));
    assert_eq!(value["next"], u64::MAX.to_string());
    assert_eq!(
        value["entries"][0]["publication"]["observation"]["view"]["turn"]["number"],
        JS_MAX_SAFE_INTEGER
    );
    for unsafe_page in [
        page(JS_MAX_SAFE_INTEGER + 1, 1),
        page(1, JS_MAX_SAFE_INTEGER + 1),
    ] {
        let value = serde_json::to_value(&unsafe_page).unwrap();
        assert!(checked_publication(&unsafe_page).is_err());
        assert!(
            !validate.is_valid(&value),
            "schema admitted an unpublished unsafe integer"
        );
    }
}

fn walk(value: &Value, root: &Value) {
    match value {
        Value::Object(fields) => {
            if let Some(reference) = fields.get("$ref") {
                let reference = reference.as_str().unwrap();
                assert!(
                    reference.starts_with("#/"),
                    "external schema reference: {reference}"
                );
                assert!(
                    root.pointer(&reference[1..]).is_some(),
                    "unresolved local reference: {reference}"
                );
            }
            assert!(!fields.contains_key("prefixItems"));
            for child in fields.values() {
                walk(child, root);
            }
        }
        Value::Array(children) => {
            for child in children {
                walk(child, root);
            }
        }
        _ => {}
    }
}

#[test]
fn exported_schemas_are_self_contained_draft7_public_contracts() {
    for schema in [command_schema(), stream_schema(), submission_schema()] {
        let schema = serde_json::to_value(schema).unwrap();
        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        walk(&schema, &schema);
        let text = schema.to_string();
        for private in [
            "profile_digest",
            "Snapshot",
            "ValidatedResponse",
            "ObjectId",
            "Prepared",
            "ReplayJournal",
            "random_seed",
            "word_position",
        ] {
            assert!(
                !text.contains(private),
                "private transport type leaked: {private}"
            );
        }
    }
}

#[test]
fn tuple_shapes_and_exact_quantities_are_checked_before_typed_commands() {
    let decoder = CommandDecoder::new().unwrap();
    for response in [
        json!({"response":"attackers","attacks":[["a",1]]}),
        json!({"response":"blockers","blocks":[["b","a"]]}),
        json!({"response":"damage","assignments":[["a","3"]]}),
        json!({"response":"damage_allocation","allocations":[["d","3"]]}),
    ] {
        decoder.decode(&envelope(response).to_string()).unwrap();
    }
    for response in [
        json!({"response":"attackers","attacks":[["a"]]}),
        json!({"response":"blockers","blocks":[["b","a","extra"]]}),
        json!({"response":"damage","assignments":[["a",3]]}),
        json!({"response":"damage_allocation","allocations":[["d","-0"]]}),
    ] {
        assert!(decoder.decode(&envelope(response).to_string()).is_err());
    }
}

#[test]
fn outbound_optional_presence_matches_serde_serialization_not_inbound_defaults() {
    let validate = validator(serde_json::to_value(stream_schema()).unwrap());
    let mut page = serde_json::to_value(page(1, 1)).unwrap();
    page["entries"][0]["publication"]["observation"]["decision"] = json!({
        "request":"commander_return", "token":"viewer-token", "player":0,
        "object":null,"designation":"public-commander-seat-0", "destination":{"zone":"hand","owner":0}
    });
    let typed: StreamPage = serde_json::from_value(page).unwrap();
    let mut serialized: Value =
        serde_json::from_str(&checked_publication(&typed).unwrap()).unwrap();
    assert!(validate.is_valid(&serialized));
    let decision = &serialized["entries"][0]["publication"]["observation"]["decision"];
    assert!(decision.get("previous_choices").is_none());
    assert_eq!(decision.get("object"), Some(&Value::Null));
    let source = &serialized["entries"][0]["publication"]["observation"]["view"]["effects"][0]["origin"]
        ["source"];
    assert_eq!(source.get("current"), Some(&Value::Null));
    serialized["entries"][0]["publication"]["observation"]["decision"]
        .as_object_mut()
        .unwrap()
        .remove("object");
    assert!(
        !validate.is_valid(&serialized),
        "outbound omitted a field Serde always serializes"
    );
    // The shared Serde DTO accepts an omitted Option on input, demonstrating
    // why the outgoing schema must deliberately use the serialize contract.
    assert!(serde_json::from_value::<StreamPage>(serialized).is_ok());
}

fn page_with_commander_history() -> Value {
    let mut value = serde_json::to_value(page(1, 1)).unwrap();
    value["entries"][0]["publication"]["observation"]["view"]["commanders"] = json!([{
        "designation":"public-commander-seat-2", "oracle_id":"public-oracle",
        "history":{"owner":2,"color_identity":["W"],
            "command_casts":{"0":"0","1":"1","2":"2","3":"3"},
            "combat_damage":{"0":"0","1":"2","2":"20","3":"21"}}
    }]);
    value
}

#[test]
fn nonempty_commander_maps_roundtrip_publication_store_and_exact_framed_ipc() {
    use mtg_server::{
        ipc::{self, ReplyFrame, WorkerReply},
        protocol::{Counter, Publication, StreamEntry},
        store::{CommitResult, MatchStore, Transition},
    };
    let original = page_with_commander_history();
    let from_value: StreamPage = serde_json::from_value(original.clone()).unwrap();
    let from_text: StreamPage = serde_json::from_str(&original.to_string()).unwrap();
    assert_eq!(serde_json::to_value(&from_value).unwrap(), original);
    assert_eq!(from_value, from_text);
    let publication = original["entries"][0]["publication"].clone();
    let typed: Publication = mtg_server::codec::exact(&publication.to_string()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), publication);

    let directory = std::env::temp_dir().join(format!(
        "mtg-page-history-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(!directory.exists());
    let mut store = MatchStore::open(
        &directory,
        &json!({"structural_codec_fixture":1}),
        &"0".repeat(64),
    )
    .unwrap();
    let mut publications: [Vec<Publication>; 4] = std::array::from_fn(|_| vec![]);
    publications[0].push(typed.clone());
    let transition = Transition {
        expected: store.head().unwrap(),
        next_boundary: 1,
        state_sha256: "1".repeat(64),
        record: json!({"structural_codec_fixture":1}),
        command: None,
        publications,
        execution_updates: vec![],
    };
    assert!(matches!(
        store.commit(store.generation(), transition).unwrap(),
        CommitResult::Committed(_)
    ));
    let stored = store.stream_page(0, 0, 32).unwrap();
    let expected = StreamPage {
        protocol_version: 1,
        entries: vec![StreamEntry {
            cursor: Counter::new(1),
            publication: typed,
        }],
        next: Counter::new(1),
        has_more: false,
    };
    assert_eq!(stored, expected);
    let reply = ReplyFrame {
        ipc_version: ipc::IPC_VERSION,
        request_id: 7,
        reply: WorkerReply::Page {
            page: Box::new(stored),
        },
    };
    let framed = ipc::encode(&reply).unwrap();
    let mut input = framed.as_slice();
    let decoded: ReplyFrame = ipc::read_frame(&mut input).unwrap().unwrap();
    assert_eq!(decoded, reply);
    assert!(input.is_empty());
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        serde_json::from_value::<mtg_core::PlayerId>(json!("2")).is_err(),
        "fixing map keys must not broaden scalar PlayerId wire values"
    );
}

#[test]
fn commander_history_key_aliases_reject_inside_publication_and_ipc() {
    use mtg_server::ipc::{self, ReplyFrame};
    for field in ["command_casts", "combat_damage"] {
        for alias in [
            "02", "+2", "-0", "-1", "2.0", "2e0", " 2", "2 ", "256", "２", "",
        ] {
            let mut page = page_with_commander_history();
            page["entries"][0]["publication"]["observation"]["view"]["commanders"][0]["history"]
                [field]
                .as_object_mut()
                .unwrap()
                .insert(alias.into(), json!("1"));
            assert!(
                serde_json::from_value::<StreamPage>(page.clone()).is_err(),
                "{field}: {alias}"
            );
            assert!(
                serde_json::from_str::<StreamPage>(&page.to_string()).is_err(),
                "{field}: {alias}"
            );
            let reply = json!({"ipc_version":ipc::IPC_VERSION,"request_id":7,
                "reply":{"reply":"page","page":page}});
            assert!(ipc::decode::<ReplyFrame>(reply.to_string().as_bytes()).is_err());
        }
    }
    let duplicate = page_with_commander_history().to_string().replacen(
        "\"2\":\"2\"",
        "\"2\":\"2\",\"2\":\"3\"",
        1,
    );
    assert!(mtg_server::codec::exact::<StreamPage>(&duplicate).is_err());
}

#[test]
fn attachments_profile_variants_decode_strictly_through_commands_and_publications() {
    let decoder = CommandDecoder::new().unwrap();
    for response in [
        json!({"response":"legend_rule","keep":"legend"}),
        json!({"response":"mana_color","color":"C"}),
        json!({"response":"search_lands","objects":["private-search"]}),
        json!({"response":"optional_recursion","perform":true}),
        json!({"response":"trample_damage","assignments":[["blocker","1"]],"player":2,"amount":"9007199254740993"}),
        json!({"response":"activate_ability","object":"equipment","ability":0}),
    ] {
        decoder
            .decode(&envelope(response.clone()).to_string())
            .unwrap();
        let mut extra = response;
        extra["ignored"] = json!(true);
        assert!(decoder.decode(&envelope(extra).to_string()).is_err());
    }
    for response in [
        json!({"response":"legend_rule","keep":{"card":1,"incarnation":0}}),
        json!({"response":"mana_color","color":"colorless"}),
        json!({"response":"search_lands","objects":[{"oracle_id":"public"}]}),
        json!({"response":"optional_recursion","perform":1}),
        json!({"response":"trample_damage","assignments":[["b","1","extra"]],"player":2,"amount":"0"}),
        json!({"response":"trample_damage","assignments":[],"player":2,"amount":9007199254740993_u64}),
    ] {
        assert!(decoder.decode(&envelope(response).to_string()).is_err());
    }
    let validate = validator(serde_json::to_value(stream_schema()).unwrap());
    for request in [
        json!({"request":"mana_color","colors":["W","C"],"mana_ability_source":"rock"}),
        json!({"request":"search_lands","maximum":2,"candidates":[{"handle":"private-search","oracle_id":"public"}]}),
        json!({"request":"legend_rule","candidates":["legend-1","legend-2"],"previous_choices":[]}),
        json!({"request":"optional_recursion"}),
        json!({"request":"mana","cost":{"generic":"1","colored":[]},"mana_ability_source":"signet"}),
        json!({"request":"spend_mana","cost":{"generic":"1","colored":[]},"available":{"C":"1"},"mana_ability_source":"signet"}),
        json!({"request":"damage","source":"attacker","power":"3","recipients":["blocker"],"player_recipient":1,"lethal_requirements":[["blocker","1"]]}),
        json!({"request":"blockers","candidates":["blocker"],"attackers":["attacker"],"menace_attackers":["attacker"],"legal_attackers":[{"blocker":"blocker","attackers":["attacker"]}]}),
    ] {
        let mut request = request;
        request["player"] = json!(0);
        request["token"] = json!("private-token");
        let typed: mtg_core::PlayerRequest =
            mtg_server::codec::exact(&request.to_string()).unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), request);
        let mut publication = serde_json::to_value(page(1, 1)).unwrap();
        publication["entries"][0]["publication"]["observation"]["decision"] = request.clone();
        assert!(validate.is_valid(&publication));
        request["ignored"] = json!(true);
        assert!(mtg_server::codec::exact::<mtg_core::PlayerRequest>(&request.to_string()).is_err());
        publication["entries"][0]["publication"]["observation"]["decision"] = request;
        assert!(!validate.is_valid(&publication));
    }
}

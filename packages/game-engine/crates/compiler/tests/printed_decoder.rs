use mtg_compiler::ast::{
    printed::{
        Fault, PRINTED_GENERATOR_SHA256, PRINTED_SCHEMA_DESCRIPTOR, PRINTED_SCHEMA_SHA256,
        PRINTED_SCHEMA_VERSION, PrintedCard, PrintedFace,
    },
    vocab::{PRINTED_VOCABULARY_SHA256, VOCABULARY_TABLES},
};
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};
use std::{collections::BTreeSet, path::Path, process::Command};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../..");

fn identity() -> Value {
    let sha256 = mtg_contracts::sha256;
    let generator = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../scripts/generate-rust-ast.ts"
    ));
    let vocabulary = serde_json::to_vec(VOCABULARY_TABLES).unwrap();
    assert_eq!(
        sha256(PRINTED_SCHEMA_DESCRIPTOR.as_bytes()),
        PRINTED_SCHEMA_SHA256
    );
    assert_eq!(sha256(&vocabulary), PRINTED_VOCABULARY_SHA256);
    assert_eq!(sha256(generator), PRINTED_GENERATOR_SHA256);
    json!({
        "schemaVersion": PRINTED_SCHEMA_VERSION,
        "schemaSha256": PRINTED_SCHEMA_SHA256,
        "vocabularySha256": PRINTED_VOCABULARY_SHA256,
        "generatorSha256": PRINTED_GENERATOR_SHA256,
    })
}

#[test]
fn rust_recomputes_the_typescript_identity() {
    identity();
}

fn evidence() -> Value {
    json!({
        "artifactKind": "oracle-printed-evidence",
        "schemaVersion": 1,
        "faces": [{
            "sourceFaceIndex": 0,
            "status": "read",
            "original": { "bytes": 2, "sha256": "x" },
            "normalized": { "bytes": 2, "sha256": "x" },
            "edits": [],
            "lines": [{
                "sourceId": "line:0",
                "status": "lexed",
                "tokens": [{ "t": "selfref", "grammarIndex": 0, "o": [0, 2], "n": [0, 2] }],
                "n": [0, 2],
            }],
            "blocks": [{
                "status": "read",
                "lines": [0, 1],
                "n": [0, 2],
                "nodes": [{ "path": "", "basis": "recorded", "span": [[0, 0, 1]] }],
                "facts": [
                    { "fact": "quantity", "form": "x", "at": "", "field": null, "tok": [0, 0, 1], "value": null },
                    { "fact": "quantity", "form": "digits", "at": "", "field": "/count", "tok": [0, 0, 1], "value": 2 },
                ],
            }],
            "removedReminders": [],
        }],
    })
}

fn reject(raw: &str, fault: Fault, path: &str) {
    let error = PrintedCard::decode(raw).unwrap_err();
    assert_eq!((error.fault, error.path.as_str()), (fault, path), "{error}");
}

fn change(path: &str, value: Value, fault: Fault) {
    let mut input = evidence();
    *input.pointer_mut(path).unwrap() = value;
    reject(&input.to_string(), fault, path);
}

#[test]
fn field_presence_and_discriminator_faults_keep_their_pointers() {
    let input = evidence();
    assert_eq!(
        serde_json::to_value(PrintedCard::decode(&input.to_string()).unwrap()).unwrap(),
        input
    );

    for path in ["/faces/0/status", "/faces/0/blocks/0/facts/0/field"] {
        let mut missing = input.clone();
        let (parent, key) = path.rsplit_once('/').unwrap();
        missing
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key)
            .unwrap();
        reject(&missing.to_string(), Fault::Omitted, path);
    }

    change("/faces/0/status", Value::Null, Fault::Null);
    change("/faces/0/status", json!("partial"), Fault::Unknown);
    change("/faces/0/status", json!(0), Fault::Malformed);
    change(
        "/faces/0/blocks/0/facts/0/form",
        json!("other"),
        Fault::Unknown,
    );
    change("/schemaVersion", json!(2), Fault::Unknown);

    let mut present = input.clone();
    let token = &mut present["faces"][0]["lines"][0]["tokens"][0];
    token["possessive"] = json!(true);
    assert!(PrintedCard::decode(&present.to_string()).is_ok());
    present["faces"][0]["lines"][0]["tokens"][0]["possessive"] = Value::Null;
    reject(
        &present.to_string(),
        Fault::Null,
        "/faces/0/lines/0/tokens/0/possessive",
    );
    change(
        "/faces/0/blocks/0/facts/0/field",
        json!([]),
        Fault::Malformed,
    );

    let mut extra = input.clone();
    extra["faces"][0]["lines"][0]["x/~"] = Value::Null;
    reject(&extra.to_string(), Fault::Unknown, "/faces/0/lines/0/x~1~0");
    let mut conflicting = input;
    conflicting["faces"][0]["blocks"][0]["nodes"][0]["of"] = json!("/other");
    reject(
        &conflicting.to_string(),
        Fault::Unknown,
        "/faces/0/blocks/0/nodes/0/of",
    );

    for field in ["absent", "null"] {
        let missing = json!({
            "sourceFaceIndex": 1, "status": "missing-oracle-text", "field": field,
        });
        let decoded = PrintedFace::decode(&missing.to_string()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), missing);
    }
}

#[test]
fn malformed_numbers_ranges_and_json_keep_their_pointers() {
    let number_path = "/faces/0/lines/0/n/1";
    for value in [json!(-1), json!(1.5), json!(4_294_967_296_u64), json!("2")] {
        change(number_path, value, Fault::Malformed);
    }
    change(number_path, Value::Null, Fault::Null);
    for invalid in [json!([1]), json!([0, 1, 2]), json!("0..2")] {
        change("/faces/0/lines/0/n", invalid, Fault::Malformed);
    }
    let mut reversed = evidence();
    reversed["faces"][0]["lines"][0]["n"] = json!([2, 1]);
    reject(&reversed.to_string(), Fault::Malformed, number_path);
    change(
        "/faces/0/blocks/0/facts/0/tok",
        json!([0, 0]),
        Fault::Malformed,
    );
    reject("null", Fault::Null, "");
    reject(r#"{"faces":[tru]}"#, Fault::Malformed, "/faces/0");

    let input = evidence().to_string();
    let duplicate = input.replacen(
        r#""artifactKind":"#,
        r#""artifactKind":null,"artifactKind":"#,
        1,
    );
    reject(&duplicate, Fault::Malformed, "/artifactKind");
    let duplicate = input.replacen(r#""sourceId":"#, r#""sourceId":null,"source\u0049d":"#, 1);
    reject(&duplicate, Fault::Malformed, "/faces/0/lines/0/sourceId");
    let mut conflicting_run = evidence();
    conflicting_run["faces"][0]["blocks"][0]["facts"][0]["tok"] = json!([0, 2, 1]);
    reject(
        &conflicting_run.to_string(),
        Fault::Malformed,
        "/faces/0/blocks/0/facts/0/tok/2",
    );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Capture<'a> {
    source_index: u32,
    name: String,
    #[serde(borrow)]
    printed: &'a RawValue,
}

#[test]
#[ignore = "requires the frozen MTG_R5_ARCHIVE, Node, and the parser's locked dependencies"]
fn round_trips_frozen_capture_sample_and_witness_groups() {
    const WITNESS_GROUPS: &[&[&str]] = &[
        &["Boseiju, Who Shelters All"],
        &["Delighted Halfling"],
        &["Junk Diver", "Terastodon"],
        &["Secret Rendezvous", "Wedgelight Rammer"],
    ];
    let archive = std::env::var_os("MTG_R5_ARCHIVE").expect("set MTG_R5_ARCHIVE");
    let archive = Path::new(ROOT).join(archive).canonicalize().unwrap();
    let output = Command::new("node")
        .current_dir(ROOT)
        .arg("packages/oracle-parser/test/support/printed-decoder-sample.ts")
        .arg(archive)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let mut lines = stdout.lines();
    let ts_identity: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert_eq!(ts_identity, identity());

    let mut cards = 0;
    let mut faces = 0;
    let mut observed = BTreeSet::new();
    let mut previous = None;
    for line in lines {
        let capture: Capture<'_> = serde_json::from_str(line).unwrap();
        assert!(previous.is_none_or(|index| index < capture.source_index));
        previous = Some(capture.source_index);
        if WITNESS_GROUPS
            .iter()
            .any(|group| group.contains(&capture.name.as_str()))
        {
            observed.insert(capture.name.clone());
        } else {
            assert_eq!(capture.source_index % 127, 0, "{}", capture.name);
        }
        let raw = capture.printed.get();
        let decoded = PrintedCard::decode(raw)
            .unwrap_or_else(|error| panic!("{} {}: {error}", capture.source_index, capture.name));
        let expected: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            serde_json::to_value(&decoded).unwrap(),
            expected,
            "{}",
            capture.name
        );
        cards += 1;
        faces += decoded.faces.len();
    }
    assert_eq!(
        observed,
        WITNESS_GROUPS
            .iter()
            .flat_map(|group| group.iter().map(|name| name.to_string()))
            .collect()
    );
    assert_eq!((cards, faces), (273, 275));
    eprintln!(
        "Round-tripped {cards} frozen capture records ({faces} faces), including all four witness groups."
    );
}

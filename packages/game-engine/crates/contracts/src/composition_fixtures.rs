use super::*;
use serde_json::Value;

pub(super) fn stamp(d: &mut Definition, row: &Value) -> Value {
    let source = &row["source"];
    let ast = row["faces"][0]["result"]["abilities"].clone();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.name = source["name"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = row["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&ast).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = row["bindingSha256"].as_str().unwrap().into();
    d.source_labels = None;
    ast
}

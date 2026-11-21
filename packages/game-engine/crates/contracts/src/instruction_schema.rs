use serde_json::{Value, json};

pub(super) fn forbid_unbound_operation(schema: &mut Value, name: &str, operation: &str) {
    let reference = format!("#/$defs/{name}");
    schema["$defs"][name] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":operation}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":reference}}}}
    }});
}

pub(super) fn operation_free_definition(name: &str) -> Value {
    let free = json!({"$ref":format!("#/$defs/{name}")});
    json!({"properties":{
        "spell":{"anyOf":[{"type":"null"},free.clone()]},
        "triggered":{"items":{"properties":{"program":free.clone()}}},
        "activated":{"items":{"properties":{"program":free}}}
    }})
}

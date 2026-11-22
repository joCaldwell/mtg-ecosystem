use super::{Instruction, SetExpr, kir::Amount};
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

/// Selects an instruction that is one kernel verb named `verb`. A family twin
/// uses it only to recognise its own programs; a kernel node's operands are
/// described by the derived structural schema and admitted by `kir::check`,
/// never restated here.
pub(crate) fn kernel_verb(verb: &str) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb"],"properties":{"verb":{"const":verb}}}}}}})
}

/// Selects an instruction that is one kernel verb with any of these names.
pub(crate) fn kernel_verbs(verbs: &[&str]) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb"],"properties":{"verb":{"enum":verbs}}}}}}})
}

/// Selects a kernel sequence. The opponent drain pair is the only audited one
/// (`kir::drain`).
pub(crate) fn kernel_seq() -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{"required":["seq"]}}})
}

/// Selects a kernel sequence whose first step's amount is the announced X.
pub(crate) fn kernel_seq_announced_x() -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["seq"],"properties":{"seq":{"minItems":1,"prefixItems":[{"required":["do"],"properties":{
            "do":{"required":["amount"],"properties":{"amount":{"const":Amount::announced_x()}}}}}]}}}}})
}

/// Selects one kernel verb of `verbs` whose amount is the announced X
/// (CR 107.3): the mark of an announced-X program.
pub(crate) fn kernel_announced_x(verbs: &[&str]) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb","amount"],"properties":{
            "verb":{"enum":verbs},"amount":{"const":Amount::announced_x()}}}}}}})
}

/// `kernel_verb(verb)` at exactly `source_block`.
pub(crate) fn kernel_verb_at(verb: &str, source_block: u32) -> Value {
    json!({"allOf":[kernel_verb(verb),{"properties":{"source_block":{"const":source_block}}}]})
}

/// Selects a kernel `destroy` or `move` whose set is exactly one of `sets`.
pub(crate) fn kernel_set(sets: &[SetExpr]) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb","what"],"properties":{
            "verb":{"enum":["destroy","move"]},"what":{"enum":sets}}}}}}})
}

/// The exact JSON of one instruction, for a twin that pins a whole instruction.
pub(crate) fn exact(instruction: &Instruction) -> Value {
    json!({"const":instruction})
}

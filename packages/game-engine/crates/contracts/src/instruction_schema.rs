use super::{
    Instruction, PlayerExpr, SetExpr,
    kir::{Amount, DamageTo, EventPlayerRole, Ref},
};
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

/// Selects one kernel verb named `verb` whose player operand is exactly
/// `player`: the group or recipient a family is defined by.
pub(crate) fn kernel_verb_for(verb: &str, player: &PlayerExpr) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb","player"],"properties":{
            "verb":{"const":verb},"player":{"const":player}}}}}}})
}

/// "You" in any controller role (CR 109.5).
fn you() -> Value {
    json!({"required":["player","who"],"properties":{"player":{"const":"is"},
        "who":{"required":["ref"],"properties":{"ref":{"const":"controller"}}}}})
}

/// A fixed amount whose value matches `value` (a string schema).
fn fixed(value: Value) -> Value {
    json!({"required":["amount","value"],"properties":{"amount":{"const":"fixed"},"value":value}})
}

const COUNT: &str = "^(0|[1-9][0-9]*)$";

/// One kernel draw whose player and amount match these operand schemas.
fn draw(player: Value, amount: Value) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb","player","amount"],"properties":{
            "verb":{"const":"draw"},"player":player,"amount":amount}}}}}})
}

/// CR 121.1: the controller draws exactly one card.
pub(crate) fn single_draw() -> Value {
    draw(you(), fixed(json!({"const":"1"})))
}

/// CR 121.2: the controller draws a fixed count other than one, as one
/// counted sequence. `pattern` narrows the count further when a family does.
pub(crate) fn counted_draw(pattern: &str) -> Value {
    draw(
        you(),
        fixed(json!({"type":"string","pattern":pattern,"not":{"const":"1"}})),
    )
}

/// `counted_draw` for any count.
pub(crate) fn any_counted_draw() -> Value {
    counted_draw(COUNT)
}

/// A fixed draw that is not a counted controller draw: one card for the
/// controller, or any fixed count for an announced or event player.
pub(crate) fn uncounted_draw() -> Value {
    json!({"anyOf":[single_draw(), player_draw(json!({}))]})
}

/// An announced or event player draws a fixed count matching `count`.
pub(crate) fn player_draw(count: Value) -> Value {
    draw(
        json!({"required":["player"],"properties":{"player":{"const":"is"}},"not":you()}),
        fixed(json!({"allOf":[{"type":"string","pattern":COUNT},count]})),
    )
}

/// Target player `slot` draws a fixed count matching `count`.
pub(crate) fn target_draw(slot: u32, count: Value) -> Value {
    draw(
        json!({"const":PlayerExpr::target(slot)}),
        fixed(json!({"allOf":[{"type":"string","pattern":COUNT},count]})),
    )
}

/// One kernel damage verb whose recipient and amount match these schemas.
fn damage(to: Value, amount: Value) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb","to","amount"],"properties":{
            "verb":{"const":"damage"},"to":to,"amount":amount}}}}}})
}

fn positive() -> Value {
    fixed(json!({"type":"string","pattern":"^[1-9][0-9]*$"}))
}

fn target_ref(slot: Option<u32>) -> Value {
    let slot = slot.map_or(json!({}), |slot| json!({"const":slot}));
    json!({"required":["to","who"],"properties":{"to":{"const":"ref"},
        "who":{"required":["ref","slot"],"properties":{"ref":{"const":"target"},"slot":slot}}}})
}

/// CR 120.2b: a fixed positive amount of damage to an announced target
/// (`slot`, or any).
pub(crate) fn target_damage(slot: Option<u32>) -> Value {
    damage(target_ref(slot), positive())
}

/// CR 107.3, 120.2b: the announced X as damage to an announced target.
pub(crate) fn target_damage_x() -> Value {
    damage(target_ref(None), json!({"const":Amount::announced_x()}))
}

/// CR 120.2b: fixed damage to the invocation's controller.
pub(crate) fn controller_damage() -> Value {
    damage(
        json!({"required":["to","who"],"properties":{"to":{"const":"ref"},
            "who":{"required":["ref"],"properties":{"ref":{"const":"controller"}}}}}),
        positive(),
    )
}

/// CR 605.1a: fixed damage to the player who activated the mana ability.
pub(crate) fn activator_damage() -> Value {
    damage(
        json!({"const":DamageTo::Ref{who:Ref::EventPlayer{role:EventPlayerRole::ActivatingPlayer}}}),
        positive(),
    )
}

/// CR 608.2h: board-wide damage whose amount matches `amount`.
pub(crate) fn set_damage(amount: Value) -> Value {
    damage(
        json!({"not":{"required":["to"],"properties":{"to":{"const":"ref"}}}}),
        amount,
    )
}

/// Board-wide damage to exactly `to`, with a fixed amount.
pub(crate) fn set_damage_to(to: &DamageTo) -> Value {
    damage(
        json!({"const":to}),
        fixed(json!({"type":"string","pattern":COUNT})),
    )
}

/// `schema` at exactly `source_block`.
pub(crate) fn at(schema: Value, source_block: u32) -> Value {
    json!({"allOf":[schema,{"properties":{"source_block":{"const":source_block}}}]})
}

/// `schema` at a source block no greater than `maximum`.
pub(crate) fn at_most(schema: Value, maximum: u32) -> Value {
    json!({"allOf":[schema,{"required":["source_block"],"properties":{"source_block":{"maximum":maximum}}}]})
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

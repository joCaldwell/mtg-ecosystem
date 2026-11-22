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

/// "You" strictly as the spell's controller (CR 112.2, 109.5).
fn spell_you() -> Value {
    json!({ "const": PlayerExpr::you(super::kir::Context::Spell) })
}

/// A fixed amount whose value matches `value` (a string schema).
pub(crate) fn fixed(value: Value) -> Value {
    json!({"required":["amount","value"],"properties":{"amount":{"const":"fixed"},"value":value}})
}

const COUNT: &str = "^(0|[1-9][0-9]*)$";

/// One kernel draw whose player and amount match these operand schemas.
fn draw(player: Value, amount: Value) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb","player","amount"],"properties":{
            "verb":{"const":"draw"},"player":player,"amount":amount}}}}}})
}

/// One kernel verb named `verb` whose operands match `operands`, each one
/// required.
fn kernel_do(verb: &str, operands: Value) -> Value {
    let mut properties = operands.as_object().cloned().unwrap_or_default();
    let mut required = vec![json!("verb")];
    required.extend(properties.keys().map(|k| json!(k)));
    properties.insert("verb".into(), json!({ "const": verb }));
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":required,"properties":properties}}}}})
}

/// The player announced as target `slot` (any slot when `None`).
pub(crate) fn target_player(slot: Option<u32>) -> Value {
    let slot = slot.map_or(json!({}), |slot| json!({ "const": slot }));
    json!({"required":["player","who"],"properties":{"player":{"const":"is"},
        "who":{"required":["ref","slot"],"properties":{"ref":{"const":"target"},"slot":slot}}}})
}

/// The announced X (CR 107.3).
pub(crate) fn announced_x() -> Value {
    json!({ "const": Amount::announced_x() })
}

/// CR 701.17a: the controller mills a fixed count.
pub(crate) fn controller_mill() -> Value {
    kernel_do(
        "mill",
        json!({"player":you(),"amount":fixed(json!({"type":"string","pattern":COUNT}))}),
    )
}

/// CR 701.17a: the player announced as target `slot` mills a fixed count.
pub(crate) fn target_mill(slot: Option<u32>) -> Value {
    kernel_do(
        "mill",
        json!({"player":target_player(slot),"amount":fixed(json!({"type":"string","pattern":COUNT}))}),
    )
}

/// CR 701.17a, 107.3: the announced player mills the announced X.
pub(crate) fn target_mill_x() -> Value {
    kernel_do(
        "mill",
        json!({"player":target_player(None),"amount":announced_x()}),
    )
}

/// CR 701.17a: any fixed mill.
pub(crate) fn fixed_mill() -> Value {
    json!({"anyOf":[controller_mill(), target_mill(None)]})
}

/// CR 701.9a-b: one kernel discard; a chosen discard serializes no `random`.
pub(crate) fn discard(player: Value, amount: Value, random: bool) -> Value {
    let mut schema = kernel_do("discard", json!({"player":player,"amount":amount}));
    let operands = &mut schema["properties"]["node"]["properties"]["do"];
    if random {
        operands["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("random"));
        operands["properties"]["random"] = json!({ "const": true });
    } else {
        operands["not"] = json!({ "required": ["random"] });
    }
    schema
}

/// CR 701.9a: the controller discards a fixed count matching `count`.
pub(crate) fn controller_discard(count: Value) -> Value {
    discard(you(), fixed(count), false)
}

/// CR 701.9a: the controller or the damaged player discards a fixed count.
pub(crate) fn chosen_discard() -> Value {
    let event = json!({"const":PlayerExpr::event(EventPlayerRole::DamagedPlayer)});
    discard(
        json!({"anyOf":[you(), event]}),
        fixed(json!({"type":"string"})),
        false,
    )
}

/// CR 701.9a-b: the player announced as target 0 discards a positive count,
/// at random when `random`.
pub(crate) fn target_discard(random: bool) -> Value {
    discard(
        target_player(None),
        fixed(json!({"type":"string","pattern":"^[1-9][0-9]*$"})),
        random,
    )
}

/// CR 701.9b, 107.3: the announced player discards the announced X at random.
pub(crate) fn target_discard_x() -> Value {
    discard(target_player(None), announced_x(), true)
}

/// CR 701.9a: each player or each opponent discards a positive count.
pub(crate) fn group_discard() -> Value {
    discard(
        json!({"enum":[PlayerExpr::EachPlayer, PlayerExpr::EachOpponent]}),
        fixed(json!({"type":"string","pattern":"^[1-9][0-9]*$"})),
        false,
    )
}

/// CR 701.9a-b: any discard by an announced player.
pub(crate) fn any_target_discard() -> Value {
    kernel_do("discard", json!({ "player": target_player(None) }))
}

/// The exact source object (CR 113.7a).
pub(crate) fn source_object() -> Value {
    json!({ "const": SetExpr::source() })
}

/// The object announced as target `slot` (any slot when `None`).
pub(crate) fn target_object(slot: Option<u32>) -> Value {
    let slot = slot.map_or(json!({}), |slot| json!({ "const": slot }));
    json!({"required":["set","object"],"properties":{"set":{"const":"one"},
        "object":{"required":["ref","slot"],"properties":{"ref":{"const":"target"},"slot":slot}}}})
}

/// CR 611.2c, 514.2: one end-of-turn continuous change of `what`.
fn modify_until(what: Value, layer: &str, change: Value) -> Value {
    kernel_do(
        "modify_until",
        json!({"what":what,"layer":{"const":layer},"change":change,"until":{"const":"end_of_turn"}}),
    )
}

/// CR 613.4c: an end-of-turn P/T modification of `what` by amounts whose
/// fixed values match `power` and `toughness` (string schemas).
pub(crate) fn pump(what: Value, power: Value, toughness: Value) -> Value {
    modify_until(
        what,
        "power_toughness_modify",
        json!({"required":["change","power","toughness"],"properties":{
        "change":{"const":"power_toughness"},"power":fixed(power),"toughness":fixed(toughness)}}),
    )
}

/// CR 613.4c: any end-of-turn P/T modification of `what`.
pub(crate) fn any_pump(what: Value) -> Value {
    pump(what, json!({"type":"string"}), json!({"type":"string"}))
}

/// CR 613.1f: the source gains keywords matching `keywords` until end of turn.
pub(crate) fn source_keywords(keywords: Value) -> Value {
    modify_until(
        source_object(),
        "ability",
        json!({"required":["change","keywords"],"properties":{
        "change":{"const":"add_keywords"},"keywords":keywords}}),
    )
}

/// CR 111.10: "you" create predefined tokens.
pub(crate) fn predefined_tokens() -> Value {
    kernel_do(
        "create_tokens",
        json!({"controller":you(),"count":positive(),
        "token":{"required":["token"],"properties":{"token":{"const":"predefined"}}}}),
    )
}

/// CR 111.3: "you" create creature tokens; the count's fixed value matches
/// `count` (a string schema).
pub(crate) fn creature_tokens(count: Value) -> Value {
    kernel_do(
        "create_tokens",
        json!({"controller":you(),"count":fixed(count),
        "token":{"required":["token"],"properties":{"token":{"const":"creature"}}}}),
    )
}

/// CR 122.1: +1/+1 counters on `on`, by a fixed amount matching `amount`
/// (a string schema).
pub(crate) fn plus_one_counters(on: Value, amount: Value) -> Value {
    kernel_do(
        "put_counters",
        json!({"on":on,
        "counter":{"const":super::kir::CounterKind::PlusOnePlusOne},"amount":fixed(amount)}),
    )
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

/// CR 121.1-121.2: the controller draws a fixed count matching `pattern`,
/// one card included.
pub(crate) fn fixed_controller_draw(pattern: &str) -> Value {
    draw(you(), fixed(json!({"type":"string","pattern":pattern})))
}

/// CR 121.1-121.2: a spell controller draws a fixed count matching `pattern`,
/// one card included.
pub(crate) fn fixed_spell_controller_draw(pattern: &str) -> Value {
    draw(
        spell_you(),
        fixed(json!({"type":"string","pattern":pattern})),
    )
}

/// CR 701.9a: a spell controller discards a fixed count matching `count`.
pub(crate) fn spell_controller_discard(count: Value) -> Value {
    discard(spell_you(), fixed(count), false)
}

/// CR 119.3: the controller loses a fixed positive amount of life.
pub(crate) fn positive_controller_life_loss() -> Value {
    kernel_do("lose_life", json!({"player":you(),"amount":positive()}))
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

/// One kernel verb `verb` whose further operands match `operands`.
fn verb_with(verb: &str, operands: Value) -> Value {
    let mut properties = operands;
    properties["verb"] = json!({"const":verb});
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["do"],"properties":{"do":{"required":["verb"],"properties":properties}}}}})
}

/// CR 701.8: a destroy of announced target `slot`, with `regeneration`
/// constraining its regeneration prohibition when given.
pub(crate) fn target_destroy(slot: Option<u32>, regeneration: Option<bool>) -> Value {
    let mut operands = json!({"what":target_object(slot)});
    if let Some(forbidden) = regeneration {
        operands["regeneration_forbidden"] = if forbidden {
            json!({"const":true})
        } else {
            json!({"not":{"const":true}})
        };
    }
    verb_with("destroy", operands)
}

/// CR 608.2d: "you may destroy" announced target `slot`.
pub(crate) fn optional_target_destroy(slot: Option<u32>) -> Value {
    json!({"required":["op","node"],"properties":{"op":{"const":"kernel"},"node":{
        "required":["optional"],"properties":{"optional":{"required":["node"],"properties":{
            "node":{"required":["do"],"properties":{"do":{"required":["verb","what"],"properties":{
                "verb":{"const":"destroy"},"what":target_object(slot)}}}}}}}}}})
}

/// A destroy of announced target `slot`, mandatory or optional.
pub(crate) fn any_target_destroy(slot: Option<u32>) -> Value {
    json!({"anyOf":[target_destroy(slot, None), optional_target_destroy(slot)]})
}

/// CR 400.3: announced target `slot` to destination `to` (`exile`,
/// `owner_hand`, `owner_library_top`).
pub(crate) fn target_move(slot: Option<u32>, to: &str) -> Value {
    verb_with(
        "move",
        json!({"what":target_object(slot),"to":{"required":["to"],"properties":{"to":{"const":to}}}}),
    )
}

/// CR 701.26: tap (`tap`) or untap (`untap`) announced target `slot`.
pub(crate) fn target_tap(verb: &str, slot: Option<u32>) -> Value {
    verb_with(verb, json!({"what":target_object(slot)}))
}

/// CR 701.26b: untap the lands you control.
pub(crate) fn untap_lands() -> Value {
    verb_with(
        "untap",
        json!({"what":{"required":["set"],"properties":{"set":{"const":"matching"}}}}),
    )
}

/// CR 404.1: exile the graveyard of announced player `slot`.
pub(crate) fn exile_target_graveyard(slot: Option<u32>) -> Value {
    let slot = slot.map_or(json!({}), |slot| json!({"const":slot}));
    verb_with(
        "move",
        json!({"to":{"const":{"to":"exile"}},"what":{"required":["set","zone"],"properties":{
            "set":{"const":"matching"},"zone":{"required":["zone","of"],"properties":{
                "zone":{"const":"graveyard"},"of":{"properties":{"who":{"properties":{
                    "ref":{"const":"target"},"slot":slot}}}}}}}}}),
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

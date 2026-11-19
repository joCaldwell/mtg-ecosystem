//! CR 107.3a, 601.2b-c and 608.2b-c. All occurrences consume the same cast X,
//! including inside a selected mode. Damage is dealt by the resolving spell.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const BASE: &str = "/faces/0/abilities/0";
const MODAL: &str = "Choose one —\n• Draw X cards.\n• ~ deals X damage to any target.";

fn fail() -> Failure {
    unsupported(
        "announced_x_effects",
        "requires a complete bound announced-X spell",
    )
}

// The frozen parser has distinct raw shapes for a target creature and any target,
// and preserves the sentence boundary between damage and a subsequent draw.
fn expected_effects(text: &str) -> Option<Value> {
    let player = || json!({"player":"target-player","selectionId":"line:0:0"});
    let you = || json!({"player":"you"});
    let x_step = |op: &str, who: Value| json!({"effect":op,"who":who,"amount":{"amount":"x"}});
    let any = |id: &str| json!({"to":"any-target","selectionId":id});
    let creature = || {
        json!({"to":"object","ref":{"ref":"target","filter":{"types":["creature"]},
        "count":{"amount":"fixed","value":1},"selectionId":"line:0:5"}})
    };
    let damage = |to: Value| json!({"effect":"damage","source":{"ref":"self"},"amount":{"amount":"x"},"to":to});
    Some(match text {
        "~ deals X damage to any target." => json!([{"steps":[damage(any("line:0:6"))]}]),
        "~ deals X damage to target creature." => json!([{"steps":[damage(creature())]}]),
        "~ deals X damage to any target. You gain X life." => {
            json!([{"steps":[damage(any("line:0:6"))]},{"steps":[x_step("gain-life",you())]}])
        }
        "~ deals X damage to target creature and you gain X life." => {
            json!([{"steps":[damage(creature()),x_step("gain-life",you())]}])
        }
        "~ deals X damage to any target. Draw two cards, then discard a card." => json!([
            {"steps":[damage(any("line:0:6"))]},
            {"steps":[{"effect":"draw","who":you(),"amount":{"amount":"fixed","value":2}},
                {"effect":"discard","who":you(),"what":{"ref":"filter","filter":{"cls":"card"},
                    "count":{"amount":"fixed","value":1}}}]}
        ]),
        "Target player gains X life." => json!([{"steps":[x_step("gain-life",player())]}]),
        "Target player mills X cards and you gain X life." => {
            json!([{"steps":[x_step("mill",player()),x_step("gain-life",you())]}])
        }
        MODAL => json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[
            {"effects":[{"steps":[x_step("draw",you())]}]},
            {"effects":[{"steps":[damage(any("line:2:7"))]}]}
        ]}]}]),
        _ => return None,
    })
}

pub(super) fn validate(
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    kind: CardKind,
) -> Result<bool, Failure> {
    let Some((line, effects)) = result
        .lines
        .iter()
        .find_map(|line| expected_effects(&line.text).map(|effects| (line, effects)))
    else {
        return Ok(false);
    };
    if result.lines.len() != 1
        || !matches!(kind, CardKind::Instant | CardKind::Sorcery)
        || line.text == MODAL && kind != CardKind::Sorcery
        || !source["mana_cost"]
            .as_str()
            .is_some_and(|cost| cost.matches("{X}").count() == 1)
        || decode::<Value>(result.abilities.get(), "abilities")?
            != json!([{"kind":"spell","effects":effects}])
    {
        return Err(fail());
    }

    let mut graph = Bindings::new(source["oracle_id"].as_str().ok_or_else(fail)?, source);
    graph.ability(BASE, "spell", "spell-controller");
    graph.node(
        BASE,
        "announced-x",
        "announced-number",
        json!({"scope":graph.id(BASE,"ability"),
        "chooser":graph.id(BASE,"you"),"announcement":"cast","rules":["107.3a","601.2b"]}),
    );
    bind_effects(&mut graph, BASE, &effects);
    graph.validate(binding, "requires complete announced-X effect bindings")?;
    Ok(true)
}

// Walk the constructed shape only. Paths retain sentence, step and mode indices.
fn bind_effects(graph: &mut Bindings<'_>, base: &str, effects: &Value) {
    for (i, sentence) in effects.as_array().unwrap().iter().enumerate() {
        for (j, step) in sentence["steps"].as_array().unwrap().iter().enumerate() {
            let path = format!("{base}/effects/{i}/steps/{j}");
            if let Some(options) = step["options"].as_array() {
                for (m, option) in options.iter().enumerate() {
                    bind_effects(graph, &format!("{path}/options/{m}"), &option["effects"]);
                }
            } else {
                bind_step(graph, &path, step);
            }
        }
    }
}

fn bind_step(graph: &mut Bindings<'_>, path: &str, step: &Value) {
    let op = step["effect"].as_str().unwrap();
    let scope = graph.id(BASE, "ability");
    let mut inputs = vec![];
    if op == "damage" {
        let object = graph.id("/source", "object");
        graph.reference(&format!("{path}/source"), &step["source"], &object, "201.5");
        inputs.push(object);
        let to = &step["to"];
        let (field, expr) = if to["to"] == "object" {
            ("to/ref", to["ref"].clone())
        } else {
            (
                "to",
                json!({"ref":"any-target","selectionId":to["selectionId"]}),
            )
        };
        inputs.push(graph.target(
            &scope,
            &format!("{path}/{field}"),
            &expr,
            expr["selectionId"].as_str().unwrap(),
        ));
    } else {
        let who = &step["who"];
        let field = format!("{path}/who");
        let actor = if let Some(selection) = who["selectionId"].as_str() {
            graph.target(&scope, &field, who, selection)
        } else {
            let you = graph.id(BASE, "you");
            graph.reference(&field, who, &you, "109.5");
            you
        };
        if op == "discard" {
            let field = format!("{path}/what");
            let what = &step["what"];
            let query = graph.node(&field, "query", "selection", json!({"scope":scope,"expression":what,
                "cardinality":"one","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
            graph.reference(&field, what, &query, "608.2h");
            inputs.push(query);
        }
        inputs.push(actor);
    }
    if step["amount"]["amount"] == "x" {
        graph.reference(
            &format!("{path}/amount"),
            &step["amount"],
            &graph.id(BASE, "announced-x"),
            "107.3a",
        );
    }
    let event = graph.node(
        path,
        "event",
        "event",
        json!({"scope":scope,"operation":op,
        "inputs":inputs,"availability":"when-performed","rules":["608.2c"]}),
    );
    let objects = match op {
        "discard" => Some(("one", "zone-successors", "400.7j")),
        "draw" if step["amount"]["amount"] == "fixed" => Some(("many", "new-objects", "608.2h")),
        "draw" | "mill" => Some(("unknown", "new-objects", "608.2h")),
        _ => None,
    };
    if let Some((count, identity, rule)) = objects {
        graph.node(path,"objects","event-result",json!({"event":event,"result":"objects","cardinality":count,
            "availability":"if-produced","identity":identity,"description":{"cls":"card"},"rules":[rule]}));
    }
    if op != "discard" {
        graph.node(
            path,
            "quantity",
            "event-result",
            json!({"event":event,"result":"quantity","cardinality":"one",
            "availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}),
        );
    }
}

// Entered only for the validated complete source. Ordinary fixed draw and the
// choose-one wrapper use the same lowering paths as other spells.
pub(super) fn instruction(
    step: &Step,
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    source_block: u32,
) -> Result<Option<Instruction>, Failure> {
    let instruction = match step {
        Step::Damage { to, .. } => {
            let target_slot = match to {
                DamageDestination::AnyTarget { selection_id } => {
                    bind_target(program, slots, selection_id, TargetDomain::AnyDamageTarget)?
                }
                DamageDestination::Object { reference } => {
                    let target = serde_json::from_value(reference.clone()).map_err(|_| fail())?;
                    bind_creature_target(program, slots, &target)?
                }
                _ => return Err(fail()),
            };
            Instruction::DealDamageAnnouncedX {
                target_slot,
                source_block,
            }
        }
        Step::Mill { who, .. } => Instruction::MillAnnouncedX {
            target_slot: bind_target(
                program,
                slots,
                fixed_mill::target_selection(who).ok_or_else(fail)?,
                TargetDomain::AnyPlayer,
            )?,
            source_block,
        },
        Step::GainLife { who, .. } => Instruction::ChangeLifeAnnouncedX {
            recipient: life_recipient(program, slots, who, false)?,
            gain: true,
            source_block,
        },
        Step::Draw {
            amount: DrawAmount::X {},
            ..
        } => Instruction::DrawAnnouncedX {
            recipient: LifeRecipient::Controller,
            source_block,
        },
        Step::Discard { .. } => Instruction::DiscardCards {
            recipient: None,
            count: 1.into(),
            source_block,
        },
        _ => return Ok(None),
    };
    Ok(Some(instruction))
}

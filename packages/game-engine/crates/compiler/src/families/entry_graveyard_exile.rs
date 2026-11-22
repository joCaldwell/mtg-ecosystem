//! CR 603.3d, 603.6a and 608.2b/c. Only the graveyard owner is targeted.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const SELECTION: &str = "line:1:6";

fn owner() -> Value {
    json!({"player":"target-player","selectionId":SELECTION})
}

fn graveyard() -> Value {
    json!({"ref":"zone-cards","zone":"graveyard","owner":owner()})
}

pub(crate) fn lower_trigger(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    condition: Option<&SourceCondition>,
    block: usize,
) -> Option<TriggeredAbility> {
    let [
        Sentence {
            bindings,
            steps,
            condition: None,
            condition_kind: None,
        },
    ] = effects
    else {
        return None;
    };
    if block != 1
        || condition.is_some()
        || !bindings.is_empty()
        || !matches!(trigger, SourceTrigger::Enters {what: TriggerObject::This {noun}} if noun == "land" || noun == "creature")
        || !matches!(steps.as_slice(), [Step::Exile {what}] if *what == graveyard())
    {
        return None;
    }
    // The contract independently checks the entire original AST, printed source,
    // sibling abilities and the source kind before the definition can leave compile().
    Some(TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            modes: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: SELECTION.into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::AnyPlayer,
                filter: None,
            }],
            instructions: vec![Instruction::exile_target_graveyard(0, 1)],
        },
    })
}

fn graph<'a>(identity: &'a str, source: &Value, land: bool) -> Bindings<'a> {
    let mut graph = Bindings::new(identity, source);
    let source_id = graph.id("/source", "object");
    let noun = if land { "land" } else { "creature" };
    let subject = json!({"ref":"this","noun":noun});
    let first = "/faces/0/abilities/0";
    if land {
        let effect = format!("{first}/effect");
        graph.ability(first, "static", "static-source-controller");
        graph.node(&effect, "entering-controller","runtime-player",json!({
            "scope":graph.id(first,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]}));
        graph.reference(&format!("{effect}/what"), &subject, &source_id, "201.5");
    } else {
        creature_entry_return::flying_binding(&mut graph, first);
    }
    let base = "/faces/0/abilities/1";
    let trigger = format!("{base}/trigger");
    graph.ability(base, "triggered", "trigger-controller");
    graph.reference(&format!("{trigger}/what"), &subject, &source_id, "201.5");
    graph.event(base, &trigger, "trigger:enters", &source_id, false);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"types":[noun]})),
    );
    let scope = graph.id(base, "ability");
    let step = format!("{base}/effects/0/steps/0");
    let what = format!("{step}/what");
    graph.target(&scope, &format!("{what}/owner"), &owner(), SELECTION);
    let cards = graveyard();
    let query = graph.node(&what,"query","selection",json!({"scope":scope,"expression":cards,
        "cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
    graph.reference(&what, &cards, &query, "608.2h");
    graph.event(base, &step, "exile", &query, false);
    graph.node(&step,"objects","event-result",json!({"event":graph.id(&step,"event"),
        "result":"objects","cardinality":"many","availability":"if-produced","identity":"zone-successors",
        "description":{"cls":"card"},"rules":["400.7j"]}));
    graph.result(&step, "quantity", "measured-quantity", None);

    if land {
        let mana = "/faces/0/abilities/2";
        let step = format!("{mana}/effects/0/steps/0");
        graph.ability(mana, "activated", "activator");
        let you = graph.id(mana, "you");
        graph.reference(
            &format!("{mana}/costs/0/source"),
            &json!({"ref":"self"}),
            &source_id,
            "201.5",
        );
        graph.reference(
            &format!("{mana}/costs/0/payer"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
        graph.reference(
            &format!("{step}/who"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
        graph.event(mana, &step, "add-mana", &you, false);
    }
    graph
}

pub(crate) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if d.spell
        .as_ref()
        .is_some_and(|p| matches!(p.resolution, SpellResolution::ChooseRepeatedModes { .. }))
    {
        return Ok(());
    }
    if d.capabilities.contains(&Capability::EntryGraveyardExileV1) {
        graph(
            &d.oracle_id,
            source,
            d.characteristics.kind == CardKind::Land,
        )
        .validate_ordered(
            binding,
            "requires complete self-entry graveyard exile bindings",
        )?;
    } else if binding["nodes"].as_array().is_some_and(|nodes| {
        nodes.iter().any(|node| {
            node["path"] == "/faces/0/abilities/1/effects/0/steps/0/what"
                && node["node"] == "selection"
                && node["expression"]["ref"] == "zone-cards"
                && node["expression"]["zone"] == "graveyard"
                && node["expression"]["owner"]["player"] == "target-player"
        })
    }) {
        return Err(unsupported(
            "binding",
            "unconsumed targeted graveyard selection",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

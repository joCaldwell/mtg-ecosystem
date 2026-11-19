//! A fixed discard can use the announced player slot in an ordinary program.
//! CR 601.2c, 608.2b and 701.9a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn operation(effects: &[Sentence]) -> Option<(&str, u32)> {
    let [
        Sentence {
            condition: None,
            condition_kind: None,
            bindings,
            steps,
        },
    ] = effects
    else {
        return None;
    };
    let [Step::Discard { who, what }] = steps.as_slice() else {
        return None;
    };
    if !bindings.is_empty()
        || who.player != "target-player"
        || who.of.is_some()
        || what.reference != "filter"
        || what.filter != ObjectFilter::Class(CardFilter { cls: "card".into() })
        || what.count.amount != "fixed"
        || what.count.value == 0
    {
        return None;
    }
    Some((
        who.selection_id.as_deref().filter(|id| !id.is_empty())?,
        what.count.value,
    ))
}

pub(super) fn validate_bindings(
    ability: &Value,
    binding: &Value,
    identity: &str,
    block: usize,
) -> Result<(), Failure> {
    if ability["kind"] != "spell" {
        return Ok(());
    }
    let path = format!("/faces/0/abilities/{block}");
    let scope = format!("card:{identity}{path}#ability");
    for (sentence, steps) in sentences(ability) {
        for (index, step) in steps.iter().enumerate() {
            let step_path = format!("{path}/effects/{sentence}/steps/{index}");
            bind_step(step, &step_path, &scope, binding, identity)?;
            if step["effect"] == "modal"
                && let Some(options) = step["options"].as_array()
            {
                for (option, mode) in options.iter().enumerate() {
                    for (sentence, steps) in sentences(mode) {
                        for (index, child) in steps.iter().enumerate() {
                            let child_path = format!(
                                "{step_path}/options/{option}/effects/{sentence}/steps/{index}"
                            );
                            bind_step(child, &child_path, &scope, binding, identity)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn sentences(program: &Value) -> impl Iterator<Item = (usize, &Vec<Value>)> {
    program["effects"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, sentence)| Some((index, sentence["steps"].as_array()?)))
}

fn bind_step(
    step: &Value,
    path: &str,
    scope: &str,
    binding: &Value,
    identity: &str,
) -> Result<(), Failure> {
    if step["effect"] != "discard" || step["who"]["player"] != "target-player" {
        return Ok(());
    }
    let fail = || {
        unsupported(
            path,
            "discard requires its exact player selection and hand query",
        )
    };
    let who = &step["who"];
    let selection = fixed_mill::target_selection(who).ok_or_else(fail)?;
    let what = &step["what"];
    let count = what["count"]["value"]
        .as_u64()
        .filter(|&n| n > 0)
        .ok_or_else(fail)?;
    if *what
        != json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":count}})
        || binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
    {
        return Err(fail());
    }
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    let refs = binding["references"].as_array().ok_or_else(fail)?;
    if binding["counts"] != json!({"obligations":refs.len(),"bound":refs.len(),"unresolved":0})
        || refs.iter().any(|reference| reference["binding"] != "bound")
    {
        return Err(fail());
    }
    let who_path = format!("{path}/who");
    let what_path = format!("{path}/what");
    let encoded: String = selection
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let target = format!("{scope}/targets/{encoded}");
    let query = format!("card:{identity}{what_path}#query");
    let event = format!("card:{identity}{path}#event");
    let valid_scope = unique(nodes.iter().filter(|node| node["id"] == scope))
        .is_some_and(|node| node["node"] == "ability" && node["abilityKind"] == "spell");
    let matches_reference = |reference_path: &str, expression: &Value, target: &str| {
        unique(
            refs.iter()
                .filter(|reference| reference["path"] == reference_path),
        )
        .is_some_and(|reference| {
            reference["expression"] == *expression && reference["target"] == target
        })
    };
    let matches_selection = |id: &str,
                             node_path: &str,
                             expression: &Value,
                             timing: &str,
                             selection: &str,
                             cardinality: &str| {
        unique(nodes.iter().filter(|node| node["id"] == id)).is_some_and(|node| {
            node["node"] == "selection"
                && node["path"] == node_path
                && node["scope"] == scope
                && node["expression"] == *expression
                && node["timing"] == timing
                && node["selection"] == selection
                && node["cardinality"] == cardinality
        })
    };
    let valid_event = unique(nodes.iter().filter(|node| node["id"] == event)).is_some_and(|node| {
        node["node"] == "event"
            && node["path"] == path
            && node["scope"] == scope
            && node["operation"] == "discard"
            && node["inputs"] == json!([query, target])
            && node["availability"] == "when-performed"
    });
    if !valid_scope
        || !valid_event
        || !matches_reference(&who_path, who, &target)
        || !matches_reference(&what_path, what, &query)
        || !matches_selection(
            &target,
            &who_path,
            who,
            "stack-announcement",
            "target",
            "one",
        )
        || !matches_selection(
            &query,
            &what_path,
            what,
            "resolution",
            "evaluate-reference",
            if count == 1 { "one" } else { "many" },
        )
    {
        return Err(fail());
    }
    Ok(())
}

fn unique<T>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_modal_discard_lowers_and_rejects_foreign_binding() {
        let identity = "synthetic";
        let scope = "card:synthetic/faces/0/abilities/0#ability";
        let path = "/faces/0/abilities/0/effects/0/steps/0/options/1/effects/0/steps/0";
        let who_path = format!("{path}/who");
        let what_path = format!("{path}/what");
        let target = format!("{scope}/targets/pick%3Aplayer");
        let query = format!("card:{identity}{what_path}#query");
        let event = format!("card:{identity}{path}#event");
        let who = json!({"player":"target-player","selectionId":"pick:player"});
        let what =
            json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":3}});
        let discard = json!({"effect":"discard","who":who,"what":what});
        let draw =
            json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}});
        let modal = json!({"effect":"modal","count":{"exactly":1},"options":[
            {"effects":[{"steps":[draw]}]}, {"effects":[{"steps":[discard]}]}
        ]});
        let ability = json!({"kind":"spell","effects":[{"steps":[modal]}]});
        let mut binding = json!({
            "identity":identity,"fullyBound":true,"referencesBound":true,"diagnostics":[],
            "counts":{"obligations":2,"bound":2,"unresolved":0},
            "nodes":[
                {"id":scope,"node":"ability","abilityKind":"spell"},
                {"id":target,"node":"selection","path":who_path,"scope":scope,"expression":who,
                    "timing":"stack-announcement","selection":"target","cardinality":"one"},
                {"id":query,"node":"selection","path":what_path,"scope":scope,"expression":what,
                    "timing":"resolution","selection":"evaluate-reference","cardinality":"many"},
                {"id":event,"node":"event","path":path,"scope":scope,"operation":"discard",
                    "inputs":[query,target],"availability":"when-performed"}
            ],
            "references":[
                {"path":who_path,"expression":who,"binding":"bound","target":target},
                {"path":what_path,"expression":what,"binding":"bound","target":query}
            ]
        });
        validate_bindings(&ability, &binding, identity, 0).unwrap();
        let Ability::Spell { effects, .. } = serde_json::from_value(ability.clone()).unwrap()
        else {
            panic!("expected spell");
        };
        let mut program = SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![],
            modes: vec![],
        };
        append_spell_effects(
            &mut program,
            &effects,
            0,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
        )
        .unwrap();
        assert!(program.targets.is_empty());
        let discard = &program.modes[1].program;
        assert_eq!(discard.targets[0].domain, TargetDomain::AnyPlayer);
        assert_eq!(discard.targets[0].selection_id, "pick:player");
        assert_eq!(
            discard.instructions,
            [Instruction::TargetPlayerDiscardCards {
                target_slot: 0,
                count: 3.into(),
                source_block: 0,
            }]
        );
        binding["references"][0]["target"] = json!("another-player");
        assert!(validate_bindings(&ability, &binding, identity, 0).is_err());
    }
}

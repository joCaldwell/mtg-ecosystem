//! Complete finite typed-search source-land activation. CR 118.3b, 119.4,
//! 602.2/601.2h, 701.23b, 113.7a (2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn life(amount: &Value) -> Result<Quantity, Failure> {
    let bad = || {
        unsupported(
            "abilities.costs.pay-life",
            "requires an exact nonnegative fixed source integer",
        )
    };
    // Match the optional-entry source numeric boundary: floats, rounded huge
    // numbers, nulls and extra fields never become exact IR quantities.
    let value = amount["value"].as_u64().ok_or_else(bad)?;
    if *amount != json!({"amount":"fixed","value":value}) {
        return Err(bad());
    }
    value.to_string().parse().map_err(|_| bad())
}
pub(super) fn lower(
    cost: ActivationCost,
    effects: &[Sentence],
    block: usize,
) -> Result<ActivatedAbility, Failure> {
    let bad = || {
        unsupported(
            "abilities.effects.search",
            "requires one complete typed search/move/shuffle activation",
        )
    };
    let [sentence] = effects else {
        return Err(bad());
    };
    if block != 0
        || !sentence.bindings.is_empty()
        || sentence.condition.is_some()
        || sentence.condition_kind.is_some()
    {
        return Err(bad());
    }
    let instruction =
        typed_land_search::lower(&sentence.steps, source_block(block)?)?.ok_or_else(bad)?;
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost,
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            modes: vec![],
            instructions: vec![instruction],
        },
    })
}
pub(super) fn validate_raw(ability: &Ability, raw: &str) -> Result<(), Failure> {
    if !matches!(ability, Ability::Activated { costs, .. } if costs.iter().any(|c| matches!(c, SourceCost::PayLife { .. })))
    {
        return Ok(());
    }
    let value: Value = decode(raw, "abilities")?;
    let bad = || {
        unsupported(
            "abilities.effects.search",
            "unconsumed activation field or null optional operand",
        )
    };
    fn keys(value: &Value, allowed: &[&str]) -> bool {
        value
            .as_object()
            .is_some_and(|o| o.keys().all(|k| allowed.contains(&k.as_str())))
    }
    if !keys(&value, &["kind", "costs", "effects", "abilityWord"])
        || value.get("abilityWord").is_some_and(|v| !v.is_string())
        || !keys(&value["effects"][0], &["steps"])
        || !keys(
            &value["effects"][0]["steps"][0],
            &["effect", "who", "zones", "for"],
        )
        || !keys(
            &value["effects"][0]["steps"][1],
            &["effect", "what", "to", "tapped"],
        )
        || value["effects"][0]["steps"][1]
            .get("tapped")
            .is_some_and(|v| v != &json!(true))
        || !keys(
            &value["effects"][0]["steps"][2],
            &["effect", "who", "library"],
        )
    {
        return Err(bad());
    }
    Ok(())
}
// Bind all consumed operands, including implicit cost payer and source. Require
// exactly the finite graph, so duplicate/foreign/unconsumed obligations reject.
pub(super) fn bind(
    abilities: &[Ability],
    binding: &Value,
    identity: &str,
    card: &Source,
) -> Result<(), Failure> {
    if !abilities.iter().any(|a| matches!(a, Ability::Activated { costs, .. } if costs.iter().any(|c| matches!(c, SourceCost::PayLife { .. })))) {
        return Ok(());
    }
    let bad = || {
        unsupported(
            "binding.activation",
            "incomplete typed-search source-land activation binding closure",
        )
    };
    let [
        Ability::Activated {
            costs,
            effects,
            restriction: None,
            ..
        },
    ] = abilities
    else {
        return Err(bad());
    };
    let [
        SourceCost::TapSelf {},
        SourceCost::PayLife { amount },
        SourceCost::Sacrifice { what },
    ] = costs.as_slice()
    else {
        return Err(bad());
    };
    if *what != json!({"ref":"this","noun":"land"}) {
        return Err(bad());
    }
    lower(
        ActivationCost::TapPayLifeSacrificeSource {
            life: life(amount)?,
        },
        effects,
        0,
    )?;
    let [
        Step::Search { find, .. },
        Step::MoveZone { what: moved, .. },
        Step::Shuffle { .. },
    ] = effects[0].steps.as_slice()
    else {
        return Err(bad());
    };
    let path = "/faces/0/abilities/0";
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let ability = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let step = format!("{path}/effects/0/steps");
    let query = format!("{root}{step}/0/for#query");
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost}),
        json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,"abilityKind":"activated","you":you}),
        json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"activator"}),
    ];
    for (i, op) in [(1, "cost:pay-life"), (2, "cost:sacrifice")] {
        let p = format!("{path}/costs/{i}");
        let event = format!("{root}{p}#event");
        nodes.push(json!({"id":event,"path":p,"node":"event","scope":ability,"operation":op,"inputs":[],"availability":"when-performed"}));
        nodes.push(json!({"id":format!("{root}{p}#quantity"),"path":p,"node":"event-result","event":event,"result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity"}));
    }
    let cardinality = if find["count"]["value"] == 1 {
        "one"
    } else {
        "many"
    };
    nodes.push(json!({"id":query,"path":format!("{step}/0/for"),"node":"selection","scope":ability,"expression":find,"cardinality":cardinality,"timing":"resolution","selection":"evaluate-reference"}));
    for (i, op, inputs) in [
        (0, "search", vec![query.clone(), you.clone()]),
        (1, "move-zone", vec![query.clone()]),
        (2, "shuffle", vec![you.clone()]),
    ] {
        let p = format!("{step}/{i}");
        nodes.push(json!({"id":format!("{root}{p}#event"),"path":p,"node":"event","scope":ability,"operation":op,"inputs":inputs,"availability":"when-performed"}));
    }
    let mut description = find["filter"].clone();
    description["cls"] = json!("permanent");
    nodes.push(json!({"id":format!("{root}{step}/1#objects"),"path":format!("{step}/1"),"node":"event-result","event":format!("{root}{step}/1#event"),"result":"objects","cardinality":cardinality,"availability":"if-produced","identity":"zone-successors","description":description}));
    let references: Vec<_> = [
        ("costs/0/source", json!({"ref":"self"}), source.clone()),
        ("costs/0/payer", json!({"player":"you"}), you.clone()),
        ("costs/1/payer", json!({"player":"you"}), you.clone()),
        ("costs/2/payer", json!({"player":"you"}), you.clone()),
        ("costs/2/what", what.clone(), source),
        ("effects/0/steps/0/who", json!({"player":"you"}), you.clone()),
        ("effects/0/steps/0/zones/of/0/owner", json!({"player":"you"}), you.clone()),
        ("effects/0/steps/0/for", find.clone(), query.clone()),
        ("effects/0/steps/1/who", json!({"player":"you"}), you.clone()),
        ("effects/0/steps/1/what", moved.clone(), query),
        ("effects/0/steps/2/who", json!({"player":"you"}), you.clone()),
        ("effects/0/steps/2/library/owner", json!({"player":"you"}), you),
    ].into_iter().map(|(p, expression, target)| json!({"path":format!("{path}/{p}"),"expression":expression,"target":target,"binding":"bound"})).collect();
    fn exact(actual: &Value, expected: &[Value], key: &str) -> bool {
        actual.as_array().is_some_and(|a| {
            a.len() == expected.len()
                && expected.iter().all(|want| {
                    let matching: Vec<_> = a.iter().filter(|v| v[key] == want[key]).collect();
                    matching.len() == 1 && {
                        let mut got = matching[0].clone();
                        got.as_object_mut().is_some_and(|o| {
                            o.remove("rules");
                            true
                        }) && got == *want
                    }
                })
        })
    }
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["engineFeatures"] != json!([])
        || binding["counts"] != json!({"obligations":12,"bound":12,"unresolved":0})
        || !exact(&binding["nodes"], &nodes, "id")
        || !exact(&binding["references"], &references, "path")
    {
        return Err(bad());
    }
    Ok(())
}

#[cfg(test)]
#[path = "typed_fetchland_tests.rs"]
mod tests;

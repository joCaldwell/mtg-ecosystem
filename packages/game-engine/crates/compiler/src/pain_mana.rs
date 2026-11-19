//! Complete disjoint mana abilities. CR 106.1b, 109.5, 120.3, 201.5,
//! 605.1a/3b, 704.4 and 733.1 (2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    abilities: &[Ability],
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    card: &Source,
) -> Result<Option<[ManaAbility; 2]>, Failure> {
    let candidate = abilities.iter().any(|ability| {
        let Ability::Activated { effects, .. } = ability else {
            return false;
        };
        effects.iter().flat_map(|effect| &effect.steps).any(|step| {
            matches!(
                step,
                Step::AddMana {
                    mana: SourceMana::Choice { .. },
                    ..
                }
            )
        })
    });
    if !candidate {
        return Ok(None);
    }

    let bad = || unsupported("mana.family", "unsupported complete disjoint pain-mana source");
    // Exact pure source kinds; no intrinsic subtype/explicit ability mixture,
    // creature readiness, third branch, unrelated static or triggered clauses.
    let noun = match card.type_line.as_str() {
        "Land" if card.mana_cost.is_empty() => "land",
        "Artifact" if !card.mana_cost.is_empty() => "artifact",
        _ => return Err(bad()),
    };
    let abilities: Vec<Value> = decode(result.abilities.get(), "abilities")?;
    if abilities.len() != 2 || result.lines.len() != 2 {
        return Err(bad());
    }
    let choice = abilities[1]["effects"][0]["steps"][0]["mana"].clone();
    let SourceMana::Choice { options } = serde_json::from_value(choice).map_err(|_| bad())? else {
        return Err(bad());
    };
    if !(2..=5).contains(&options.len()) {
        return Err(bad());
    }
    let mut colors = Vec::new();
    for option in &options {
        let SourceMana::Fixed { symbols } = option else {
            return Err(bad());
        };
        let [color] = symbols.as_slice() else {
            return Err(bad());
        };
        if *color == Color::C || colors.contains(color) {
            return Err(bad());
        }
        colors.push(*color);
    }
    let amount = &abilities[1]["effects"][1]["steps"][0]["amount"];
    let number = amount["value"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(bad)?;
    if *amount != json!({"amount":"fixed","value":number}) {
        return Err(bad());
    }
    let damage = number.to_string().parse().map_err(|_| bad())?;
    let add = |mana| json!({"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":mana}]});
    let options = colors
        .iter()
        .map(|color| json!({"mana":"fixed","symbols":[color]}))
        .collect::<Vec<_>>();
    let expected = [
        json!({"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[add(json!({"mana":"fixed","symbols":["C"]}))]}),
        json!({"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[
            add(json!({"mana":"choice","options":options})),
            {"steps":[{"effect":"damage","source":{"ref":"this","noun":noun},
                "amount":amount,"to":{"to":"player","ref":{"player":"you"}}}]}]}),
    ];
    for ((actual, wanted), line) in abilities.iter().zip(&expected).zip(&result.lines) {
        let mirror: Value = decode(line.ability.get(), "lines.ability")?;
        if actual != wanted || mirror != *actual {
            return Err(bad());
        }
    }
    // Bind the colors and amount to the complete printed clauses. The AST
    // and its line mirror must agree with the source even after rehashing.
    let symbols = colors
        .iter()
        .map(|color| {
            let value = serde_json::to_value(color).map_err(|_| bad())?;
            Ok(format!("{{{}}}", value.as_str().ok_or_else(bad)?))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let production = if symbols.len() == 2 {
        symbols.join(" or ")
    } else {
        format!(
            "{}, or {}",
            symbols[..symbols.len() - 1].join(", "),
            symbols.last().ok_or_else(bad)?,
        )
    };
    let text = vec![
        "{T}: Add {C}.".to_owned(),
        format!("{{T}}: Add {production}. This {noun} deals {number} damage to you."),
    ];
    if source_lines(&card.oracle_text, &card.name, &card.type_line)? != text
        || result
            .lines
            .iter()
            .map(|l| normalization::literal_spelling(&l.text))
            .collect::<Vec<_>>()
            != text
    {
        return Err(invalid(
            "lines",
            "source contains unaccounted or mismatched normalized blocks",
        ));
    }
    bind(binding, identity, card, noun)?;
    let costs = PermanentActivationCosts {
        tap: true,
        mana: None,
        sacrifice: false,
        remove_source_plus_one_counters: None,
    };
    Ok(Some([
        ManaAbility {
            self_damage: None,
            restriction: None,
            costs: costs.clone(),
            production: ManaProduction::Fixed {
                symbols: vec![Color::C],
            },
            source_block: 0,
        },
        ManaAbility {
            self_damage: Some(damage),
            restriction: None,
            costs,
            production: ManaProduction::Choice { colors },
            source_block: 1,
        },
    ]))
}

fn bind(binding: &Value, identity: &str, card: &Source, noun: &str) -> Result<(), Failure> {
    let bad = || unsupported("binding.mana", "incomplete disjoint pain-mana binding closure");
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity,"rules":["201.5"]}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
            "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost,"rules":["712"]}),
    ];
    let mut references = Vec::new();
    for index in 0..2 {
        let path = format!("/faces/0/abilities/{index}");
        let ability = format!("{root}{path}#ability");
        let you = format!("{root}{path}#you");
        let mana = format!("{path}/effects/0/steps/0");
        nodes.extend([
            json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,
                "abilityKind":"activated","you":you,"rules":["113","109.5"]}),
            json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"activator","rules":["109.5"]}),
            json!({"id":format!("{root}{mana}#event"),"path":mana,"node":"event","scope":ability,
                "operation":"add-mana","inputs":[you],"availability":"when-performed","rules":["608.2c"]}),
        ]);
        references.extend([
            json!({"path":format!("{path}/costs/0/source"),"expression":{"ref":"self"},"binding":"bound","target":source,"rules":["201.5"]}),
            json!({"path":format!("{path}/costs/0/payer"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]}),
            json!({"path":format!("{mana}/who"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]}),
        ]);
        if index == 1 {
            let path = format!("{path}/effects/1/steps/0");
            let event = format!("{root}{path}#event");
            nodes.extend([
                json!({"id":event,"path":path,"node":"event","scope":ability,"operation":"damage",
                    "inputs":[source,you],"availability":"when-performed","rules":["608.2c"]}),
                json!({"id":format!("{root}{path}#quantity"),"path":path,"node":"event-result","event":event,
                    "result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}),
            ]);
            references.extend([
                json!({"path":format!("{path}/source"),"expression":{"ref":"this","noun":noun},"binding":"bound","target":source,"rules":["201.5"]}),
                json!({"path":format!("{path}/to/ref"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]}),
            ]);
        }
    }
    let mut expected = json!({"artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
        "identity":identity,"rules":{"edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
        "nodes":nodes,"references":references,"diagnostics":[],"counts":{"obligations":8,"bound":8,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true});
    let mut actual = binding.clone();
    // Node/reference ordering carries no authority. Equality still checks every
    // field, rule citation, ID, scope, dependency and cardinality, exactly once.
    for (key, by) in [("nodes", "id"), ("references", "path")] {
        for graph in [&mut actual, &mut expected] {
            let entries = graph
                .get_mut(key)
                .and_then(Value::as_array_mut)
                .ok_or_else(bad)?;
            entries.sort_by(|a, b| a[by].as_str().cmp(&b[by].as_str()));
        }
    }
    if actual != expected {
        return Err(bad());
    }
    Ok(())
}

#[cfg(test)]
#[path = "pain_mana_tests.rs"]
mod tests;

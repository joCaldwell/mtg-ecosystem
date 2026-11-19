//! Complete fixed controller scry and the closed artifact-or-creature selector.
//! CR 109.5, 601.2c, 608.2c, 701.22a-d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn validate_source_shape(raw: &str) -> Result<(), Failure> {
    let abilities: serde_json::Value = decode(raw, "abilities")?;
    for ability in abilities
        .as_array()
        .ok_or_else(|| unsupported("scry", "ordinary source required"))?
    {
        if ability["kind"] != "spell" {
            return Err(unsupported("scry", "ordinary source required"));
        }
        for sentence in ability["effects"]
            .as_array()
            .ok_or_else(|| unsupported("scry", "missing complete effects"))?
        {
            if sentence.as_object().is_none_or(|s| {
                s.keys()
                    .any(|k| !["bindings", "steps"].contains(&k.as_str()))
            }) {
                return Err(unsupported(
                    "scry",
                    "conditional or qualified sentence unsupported",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn instruction(
    amount: &serde_json::Value,
    source_block: u32,
) -> Result<Instruction, Failure> {
    Ok(Instruction::Scry {
        amount: fixed_count(amount)?,
        source_block,
    })
}
pub(super) fn fixed_count(amount: &serde_json::Value) -> Result<Quantity, Failure> {
    let count: Count = serde_json::from_value(amount.clone()).map_err(|_| {
        unsupported(
            "library count",
            "requires complete fixed nonnegative amount",
        )
    })?;
    if count.amount != "fixed" {
        return Err(unsupported(
            "library count",
            "requires fixed nonnegative amount",
        ));
    }
    Ok(Quantity::from(i64::from(count.value)))
}
pub(super) fn surveil_instruction(
    amount: &serde_json::Value,
    source_block: u32,
) -> Result<Instruction, Failure> {
    Ok(Instruction::Surveil {
        amount: fixed_count(amount)?,
        source_block,
    })
}

pub(super) fn artifact_creature_target(what: &serde_json::Value) -> bool {
    lower_permanent_filter(&what["filter"]).is_ok_and(|f| {
        f.types.len() == 2
            && f.types.contains(&CardKind::Creature)
            && f.types.contains(&CardKind::Artifact)
            && f.non_types.is_empty()
            && f.non_colors.is_empty()
            && !f.opponent
            && !f.tapped
    })
}

pub(super) fn bind_grant_target(
    p: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &serde_json::Value,
) -> Result<u32, Failure> {
    if !artifact_creature_target(what) {
        return Err(unsupported(
            "targets",
            "requires unqualified artifact-or-creature selector",
        ));
    }
    let slot = bind_permanent_target(p, slots, what)?;
    // Alternatives are commutative. Canonicalize ONLY this new closed family;
    // preserve every historical permanent-removal filter's serialized order.
    p.targets[slot as usize]
        .filter
        .as_mut()
        .ok_or_else(|| unsupported("targets", "missing permanent filter"))?
        .types
        .sort();
    Ok(slot)
}

pub(super) fn validate_bindings(
    binding: &serde_json::Value,
    identity: &str,
    raw: &str,
) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "requires complete controller-scry binding closure",
        )
    };
    let abilities: serde_json::Value = decode(raw, "abilities")?;
    let mut expected = vec![];
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    for (block, ability) in abilities.as_array().ok_or_else(fail)?.iter().enumerate() {
        if ability["kind"] != "spell" {
            return Err(fail());
        }
        let scope = format!("card:{identity}/faces/0/abilities/{block}#ability");
        let you = format!("card:{identity}/faces/0/abilities/{block}#you");
        if !nodes.iter().any(|n| {
            n["id"] == you
                && n["node"] == "runtime-player"
                && n["scope"] == scope
                && n["role"] == "spell-controller"
        }) {
            return Err(fail());
        }
        for (sentence, effect) in ability["effects"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .enumerate()
        {
            for (step, value) in effect["steps"]
                .as_array()
                .ok_or_else(fail)?
                .iter()
                .enumerate()
            {
                if value["effect"] == "mill" {
                    let who = &value["who"];
                    let target = if *who == serde_json::json!({"player":"you"}) {
                        you.clone()
                    } else {
                        let selection =
                            crate::fixed_mill::target_selection(who).ok_or_else(fail)?;
                        let encoded = selection
                            .bytes()
                            .map(|b| {
                                if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
                                    (b as char).to_string()
                                } else {
                                    format!("%{b:02X}")
                                }
                            })
                            .collect::<String>();
                        let target = format!("{scope}/targets/{encoded}");
                        if !nodes.iter().any(|n| {
                            n["id"] == target
                                && n["node"] == "selection"
                                && n["scope"] == scope
                                && n["expression"] == *who
                                && n["timing"] == "stack-announcement"
                                && n["cardinality"] == "one"
                                && n["selection"] == "target"
                        }) {
                            return Err(fail());
                        }
                        target
                    };
                    expected.push((
                        format!("/faces/0/abilities/{block}/effects/{sentence}/steps/{step}/who"),
                        who.clone(),
                        target,
                    ));
                }
                if value["effect"] == "draw" {
                    if value["who"] != serde_json::json!({"player":"you"}) {
                        return Err(fail());
                    }
                    expected.push((
                        format!("/faces/0/abilities/{block}/effects/{sentence}/steps/{step}/who"),
                        value["who"].clone(),
                        you.clone(),
                    ));
                }
            }
        }
    }
    let refs = binding["references"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != serde_json::json!([])
        || binding["counts"]
            != serde_json::json!({"obligations":expected.len(),"bound":expected.len(),"unresolved":0})
        || refs.len() != expected.len()
        || expected.iter().any(|(path, expression, target)| {
            let matches: Vec<_> = refs.iter().filter(|r| r["path"] == *path).collect();
            matches.len() != 1
                || matches[0]["expression"] != *expression
                || matches[0]["target"] != *target
                || matches[0]["binding"] != "bound"
        })
    {
        return Err(fail());
    }
    Ok(())
}

// A single fixed lookup can use the cast event's authenticated controller.
pub(super) fn validate_cast_scry(
    abilities: &[Ability],
    result: &ParseResult,
    binding: &serde_json::Value,
    identity: &str,
) -> Result<(), Failure> {
    for (block, ability) in abilities.iter().enumerate() {
        let Ability::Triggered {
            trigger,
            effects,
            condition,
            ..
        } = ability
        else {
            continue;
        };
        if !effects
            .iter()
            .flat_map(|sentence| &sentence.steps)
            .any(|step| matches!(step, Step::Scry { .. }))
        {
            continue;
        }

        let fail = || {
            unsupported(
                "triggered scry",
                "requires one source-bound controller cast lookup",
            )
        };
        let SourceTrigger::Cast {
            who,
            what,
            not_from: None,
        } = trigger
        else {
            return Err(fail());
        };
        let [
            Sentence {
                bindings,
                condition: None,
                condition_kind: None,
                steps,
            },
        ] = effects.as_slice()
        else {
            return Err(fail());
        };
        let [Step::Scry { amount }] = steps.as_slice() else {
            return Err(fail());
        };
        let TypePredicate::SpellColor { color } =
            lower_type_predicate(what, Some("spell"), true, 0)?
        else {
            return Err(fail());
        };
        let color = match color {
            Color::W => "white",
            Color::U => "blue",
            Color::B => "black",
            Color::R => "red",
            Color::G => "green",
            Color::C => return Err(fail()),
        };
        let count: Count = serde_json::from_value(amount.clone()).map_err(|_| fail())?;
        let printed = result.lines[block]
            .text
            .strip_prefix(&format!("Whenever you cast a {color} spell, scry "))
            .and_then(|text| text.strip_suffix('.'));
        if condition.is_some()
            || !bindings.is_empty()
            || who.player != "you"
            || who.selection_id.is_some()
            || who.of.is_some()
            || count.amount != "fixed"
            || !printed.is_some_and(|text| {
                normalization::matches_fixed_amount(text, i64::from(count.value))
            })
        {
            return Err(fail());
        }

        let root = format!("/faces/0/abilities/{block}");
        let scope = format!("card:{identity}{root}#ability");
        let you = format!("card:{identity}{root}#you");
        let actor = format!("{root}/trigger/who");
        let trigger_path = format!("{root}/trigger");
        let effect_path = format!("{root}/effects/0/steps/0");
        let in_block = |node: &serde_json::Value| {
            node["path"].as_str().is_some_and(|path| {
                path == root
                    || path
                        .strip_prefix(&root)
                        .is_some_and(|tail| tail.starts_with('/'))
            })
        };
        let nodes: Vec<_> = binding["nodes"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .filter(|node| in_block(node))
            .collect();
        let references: Vec<_> = binding["references"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .filter(|node| in_block(node))
            .collect();
        if nodes.len() != 5
            || references.len() != 1
            || binding["identity"] != identity
            || *references[0]
                != serde_json::json!({
                    "path":actor,"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]
                })
            || !nodes.iter().any(|node| {
                node["id"] == you
                    && node["path"] == root
                    && node["node"] == "runtime-player"
                    && node["scope"] == scope
                    && node["role"] == "trigger-controller"
            })
            || !nodes.iter().any(|node| {
                node["path"] == trigger_path
                    && node["scope"] == scope
                    && node["node"] == "event"
                    && node["operation"] == "trigger:cast"
                    && node["inputs"] == serde_json::json!([you])
            })
            || !nodes.iter().any(|node| {
                node["path"] == effect_path
                    && node["scope"] == scope
                    && node["node"] == "event"
                    && node["operation"] == "scry"
                    && node["inputs"] == serde_json::json!([])
            })
        {
            return Err(fail());
        }
    }
    Ok(())
}

#[test]
fn cast_scry_checks_printed_color_count_and_controller_binding() {
    let raw =
        include_str!("../../../tests/fixtures/ramos/3f31c423-d45c-4c3e-b7d4-ca125be0f92c.json");
    let record: Record = decode(raw, "record").unwrap();
    let mut result: ParseResult = decode(record.faces[0].result.get(), "result").unwrap();
    let abilities = read_abilities(result.abilities.get()).unwrap();
    let binding = decode(record.binding.get(), "binding").unwrap();
    validate_cast_scry(&abilities, &result, &binding, &record.oracle_identity).unwrap();
    result.lines[1].text = result.lines[1].text.replace("scry 2", "scry 5");
    assert!(validate_cast_scry(&abilities, &result, &binding, &record.oracle_identity).is_err());
}

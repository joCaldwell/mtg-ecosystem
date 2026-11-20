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

// Match both directions: an edited AST must not erase a printed lookup,
// and an injected lookup cannot borrow a different printed effect.
pub(super) fn lower_cast_opponent_damage(
    condition: &TriggerCondition,
    effects: &[Sentence],
    block: u32,
    program: &mut SpellProgram,
) -> Result<Option<Instruction>, Failure> {
    // The complete printed block, operand and scoped graph were checked by
    // validate_fixed_cast_effects before lowering any abilities.
    let TriggerCondition::SpellCast {
        caster: CasterRelation::Controller,
        spell: TypePredicate::SpellColor { .. },
    } = condition
    else {
        return Ok(None);
    };
    let [
        Sentence {
            bindings,
            steps,
            condition: None,
            condition_kind: None,
        },
    ] = effects
    else {
        return Ok(None);
    };
    let [
        Step::Damage {
            optional: None,
            amount,
            to: DamageDestination::Player { reference },
            ..
        },
    ] = steps.as_slice()
    else {
        return Ok(None);
    };
    if !bindings.is_empty() || reference["player"] != "target-opponent" {
        return Ok(None);
    }
    let selection = reference["selectionId"]
        .as_str()
        .ok_or_else(|| unsupported("damage.target", "missing bound opponent occurrence"))?;
    Ok(Some(Instruction::DealDamage {
        target_slot: bind_target(
            program,
            &mut BTreeMap::new(),
            selection,
            TargetDomain::OpponentPlayer,
        )?,
        amount: fixed_positive_value(amount)?,
        source_block: block,
    }))
}

pub(super) fn validate_fixed_cast_effects(
    abilities: &[Ability],
    result: &ParseResult,
    binding: &serde_json::Value,
    identity: &str,
    source: &serde_json::Value,
) -> Result<(), Failure> {
    use crate::bindings::Bindings;
    use serde_json::json;
    let fail = || {
        unsupported(
            "cast effect",
            "requires a complete source-bound controller cast effect",
        )
    };
    for (block, (ability, line)) in abilities.iter().zip(&result.lines).enumerate() {
        if line.text.starts_with("When this land enters, scry ")
            && matches!(ability, Ability::Triggered {trigger, ..}
                if matches!(trigger.as_ref(), SourceTrigger::Enters {what: TriggerObject::This {noun}} if noun == "land"))
        {
            // The land-entry lowerer proves the complete source and binding.
            continue;
        }
        let printed = FixedCastEffect::from_line(&line.text, "~");
        let uses_scry = matches!(ability, Ability::Triggered {effects,..}
            if effects.iter().flat_map(|sentence| &sentence.steps).any(|step| matches!(step,Step::Scry {..})));
        let uses_damage = matches!(ability, Ability::Triggered {trigger, effects,..}
            if matches!(trigger.as_ref(), SourceTrigger::Cast { .. }) && effects.iter().flat_map(|s| &s.steps).any(|step| matches!(step, Step::Damage {to: DamageDestination::Player {reference}, ..} if reference["player"] == "target-opponent")));
        if printed.is_none() && !uses_scry && !uses_damage {
            continue;
        }
        let operand = printed.ok_or_else(fail)?;
        let expected = operand.ability(block);
        if !match operand {
            FixedCastEffect::Scry(..) => uses_scry,
            FixedCastEffect::OpponentDamage(..) => uses_damage,
        } || decode::<serde_json::Value>(line.ability.get(), "triggered scry")? != expected
        {
            return Err(fail());
        }
        let root = format!("/faces/0/abilities/{block}");
        let trigger = format!("{root}/trigger");
        let effect = format!("{root}/effects/0/steps/0");
        let mut graph = Bindings::new(identity, source);
        graph.ability(&root, "triggered", "trigger-controller");
        let you = graph.id(&root, "you");
        graph.reference(
            &format!("{trigger}/who"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
        graph.event(&root, &trigger, "trigger:cast", &you, false);
        graph.result(
            &trigger,
            "objects",
            "same-objects",
            Some(&json!({"cls":"spell"})),
        );
        let (operation, inputs) = match operand {
            FixedCastEffect::Scry(..) => ("scry", vec![]),
            FixedCastEffect::OpponentDamage(..) => {
                let step = &expected["effects"][0]["steps"][0];
                let object = graph.id("/source", "object");
                graph.reference(
                    &format!("{effect}/source"),
                    &step["source"],
                    &object,
                    "201.5",
                );
                let target = graph.target(
                    &graph.id(&root, "ability"),
                    &format!("{effect}/to/ref"),
                    &step["to"]["ref"],
                    &format!("line:{block}:12"),
                );
                graph.result(&effect, "quantity", "measured-quantity", None);
                ("damage", vec![object, target])
            }
        };
        graph.node(
            &effect,
            "event",
            "event",
            json!({"scope":graph.id(&root,"ability"),
            "operation":operation,"inputs":inputs,"availability":"when-performed","rules":["608.2c"]}),
        );
        graph.validate_block(binding, &root, "incomplete cast trigger binding graph")?;
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
    let source = decode(record.source.get(), "source").unwrap();
    validate_fixed_cast_effects(
        &abilities,
        &result,
        &binding,
        &record.oracle_identity,
        &source,
    )
    .unwrap();
    // Mutate only the AST of the printed lookup. A different operator cannot
    // leave the family before its source and binding checks run.
    let mut raw_abilities: serde_json::Value = decode(result.abilities.get(), "abilities").unwrap();
    raw_abilities[1]["effects"][0]["steps"][0] = serde_json::json!({
        "effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}
    });
    let substituted = read_abilities(&raw_abilities.to_string()).unwrap();
    assert!(
        validate_fixed_cast_effects(
            &substituted,
            &result,
            &binding,
            &record.oracle_identity,
            &source
        )
        .is_err()
    );

    let root = "/faces/0/abilities/1";
    for (role, field, value) in [
        ("ability", "source", "unrelated"),
        ("runtime-player", "role", "spell-controller"),
        ("event-result", "identity", "zone-successors"),
        ("scry", "availability", "when-announced"),
        ("trigger:cast", "id", "unrelated"),
    ] {
        let mut corrupted = binding.clone();
        let row = corrupted["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| {
                node["path"].as_str().unwrap().starts_with(root)
                    && (node["node"] == role || node["operation"] == role)
            })
            .unwrap();
        row[field] = serde_json::json!(value);
        assert!(
            validate_fixed_cast_effects(
                &abilities,
                &result,
                &corrupted,
                &record.oracle_identity,
                &source
            )
            .is_err(),
            "{role}"
        );
    }
    result.lines[1].text = result.lines[1].text.replace("scry 2", "scry 5");
    assert!(
        validate_fixed_cast_effects(
            &abilities,
            &result,
            &binding,
            &record.oracle_identity,
            &source
        )
        .is_err()
    );
}

#[test]
fn complete_cast_scry_binds_definition_block_color_amount_and_source() {
    use serde_json::json;
    // Isolate one grammatical block in memory and rebase the parser paths.
    // The surrounding card's other abilities and archived file stay untouched.
    let fixture =
        include_str!("../../../tests/fixtures/ramos/3f31c423-d45c-4c3e-b7d4-ca125be0f92c.json");
    let raw = crate::tests::changed_source(fixture, |r| {
        let result = &mut r["faces"][0]["result"];
        let line = result["lines"][1].clone();
        result["lines"] = json!([line]);
        result["abilities"] = json!([line["ability"]]);
        r["source"]["oracle_text"] = line["text"].clone();
        let graph = &mut r["binding"];
        for field in ["nodes", "references"] {
            graph[field].as_array_mut().unwrap().retain(|row| {
                let path = row["path"].as_str().unwrap();
                matches!(path, "/source" | "/faces/0") || path.starts_with("/faces/0/abilities/1")
            });
            graph[field] = serde_json::from_str(
                &graph[field]
                    .to_string()
                    .replace("/faces/0/abilities/1", "/faces/0/abilities/0"),
            )
            .unwrap();
        }
        graph["counts"] = json!({"obligations":1,"bound":1,"unresolved":0});
    });
    let manifest = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
    let CompileResult::Lowered { definition } = compile(manifest, &raw) else {
        panic!("isolated color cast block must lower")
    };
    for change in ["amount", "color", "source block", "erased", "audit block"] {
        let mut d = definition.clone();
        match change {
            "erased" => d.triggered.clear(),
            "audit block" => d.source_labels.as_mut().unwrap().triggered[0] = 1,
            "color" => {
                d.triggered[0].condition = TriggerCondition::SpellCast {
                    caster: CasterRelation::Controller,
                    spell: TypePredicate::SpellColor { color: Color::W },
                }
            }
            _ => {
                let Instruction::Scry {
                    amount,
                    source_block,
                } = &mut d.triggered[0].program.instructions[0]
                else {
                    panic!("scry instruction")
                };
                if change == "amount" {
                    *amount = 1.into();
                } else {
                    *source_block = 1;
                }
            }
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{change}");
    }
}

#[test]
fn fixed_cast_opponent_damage_proves_actor_selection_event_and_omission() {
    use serde_json::json;
    let fixture =
        include_str!("../../../tests/fixtures/ramos/3f31c423-d45c-4c3e-b7d4-ca125be0f92c.json");
    let manifest = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
    let raw = crate::tests::changed_source(fixture, |r| {
        let name = "Violet Observatory";
        let line = "Whenever you cast a black spell, ~ deals 5 damage to target opponent.";
        r["source"]["name"] = json!(name);
        r["source"]["oracle_text"] = json!(line.replace('~', name));
        let result = &mut r["faces"][0]["result"];
        result["name"] = json!(name);
        let mut ast = result["abilities"][2].clone();
        ast["trigger"]["what"]["colors"] = json!(["black"]);
        ast["effects"][0]["steps"][0]["amount"]["value"] = json!(5);
        ast["effects"][0]["steps"][0]["to"]["ref"]["selectionId"] = json!("line:0:12");
        result["lines"] = json!([{"ok":true,"text":line,"ability":ast}]);
        result["abilities"] = json!([ast]);
        let graph = &mut r["binding"];
        for field in ["nodes", "references"] {
            graph[field].as_array_mut().unwrap().retain_mut(|row| {
                let path = row["path"].as_str().unwrap();
                if path == "/faces/0" {
                    row["name"] = json!(name);
                    return true;
                }
                path == "/source" || path.starts_with("/faces/0/abilities/2")
            });
            graph[field] = serde_json::from_str(
                &graph[field]
                    .to_string()
                    .replace("/abilities/2", "/abilities/0")
                    .replace("line:2:12", "line:0:12")
                    .replace("line%3A2%3A12", "line%3A0%3A12"),
            )
            .unwrap();
        }
        graph["counts"] = json!({"obligations":3,"bound":3,"unresolved":0});
    });
    let parsed = compile(manifest, &raw);
    let CompileResult::Lowered { definition } = parsed else {
        panic!("synthetic cast damage must lower: {parsed:?}")
    };
    for change in [
        "omission",
        "renamed omission",
        "amount",
        "selection",
        "condition",
    ] {
        let mut d = definition.clone();
        match change {
            "omission" => d.triggered.clear(),
            "renamed omission" => {
                d.triggered.clear();
                d.name = "Synthetic Missing Observer".into();
                d.source_labels = None;
            }
            "amount" => {
                let Instruction::DealDamage { amount, .. } =
                    &mut d.triggered[0].program.instructions[0]
                else {
                    panic!()
                };
                *amount = 1.into();
            }
            "selection" => d.triggered[0].program.targets[0].selection_id = "line:1:12".into(),
            _ => d.triggered[0].condition = TriggerCondition::SelfDies,
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{change}");
    }
    for change in [
        "actor",
        "origin",
        "scope",
        "selection",
        "timing",
        "inputs",
        "result",
        "operator",
        "count",
    ] {
        let corrupted = crate::tests::changed_source(&raw, |r| {
            if matches!(change, "operator" | "count") {
                let result = &mut r["faces"][0]["result"];
                let step = &mut result["abilities"][0]["effects"][0]["steps"][0];
                if change == "count" {
                    step["amount"]["value"] = json!(6)
                } else {
                    *step = json!({"effect":"scry","amount":{"amount":"fixed","value":5}})
                }
                result["lines"][0]["ability"] = result["abilities"][0].clone();
                return;
            }
            let (field, kind, key, value) = match change {
                "actor" => ("references", "/trigger/who", "target", json!("other")),
                "origin" => ("references", "/steps/0/source", "target", json!("other")),
                "scope" => ("nodes", "selection", "scope", json!("other")),
                "selection" => (
                    "nodes",
                    "selection",
                    "expression",
                    json!({"player":"target-player","selectionId":"line:0:12"}),
                ),
                "timing" => ("nodes", "selection", "timing", json!("resolution")),
                "inputs" => ("nodes", "damage", "inputs", json!([])),
                _ => ("nodes", "quantity", "identity", json!("same-objects")),
            };
            let row = r["binding"][field]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| {
                    r["node"] == kind
                        || r["operation"] == kind
                        || r["result"] == kind
                        || r["path"].as_str().unwrap().ends_with(kind)
                })
                .unwrap();
            row[key] = value;
        });
        assert!(
            !matches!(compile(manifest, &corrupted), CompileResult::Lowered { .. }),
            "{change}"
        );
    }
}

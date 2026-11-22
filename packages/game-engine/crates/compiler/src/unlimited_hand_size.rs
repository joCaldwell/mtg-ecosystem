//! CR 109.5, 402.2, 611.3 and 613.11: static maximum hand-size rules.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const STATIC_TEXT: &str = "You have no maximum hand size.";

fn hand_size_rule(who: &Value, change: Value) -> Value {
    json!({"who":who,"rule":"maximum-hand-size","change":change})
}

fn fixed_change(operation: &str, amount: i64) -> Value {
    json!({"operation":operation,"amount":{"amount":"fixed","value":amount}})
}

pub(super) fn lower_rule(rule: &Value) -> Result<StaticAbility, Failure> {
    let rule_error = || unsupported("static.player-rule", "unsupported complete player rule");
    let who = &rule["who"];
    let scope = if *who == json!({"player":"you"}) {
        HandSizeScope::Controller
    } else if *who == json!({"player":"each-opponent"}) {
        HandSizeScope::Opponents
    } else {
        return Err(rule_error());
    };
    let effect = if scope == HandSizeScope::Controller
        && *rule == hand_size_rule(who, json!({"operation":"none"}))
    {
        StaticEffect::UnlimitedControllerHandSize
    } else {
        let amount = rule["change"]["amount"]["value"]
            .as_i64()
            .filter(|value| *value > 0)
            .ok_or_else(rule_error)?;
        if *rule == hand_size_rule(who, fixed_change("reduce", amount)) {
            StaticEffect::ReduceMaximumHandSize {
                scope,
                amount: Quantity::from(amount),
            }
        } else if scope == HandSizeScope::Controller
            && *rule == hand_size_rule(who, fixed_change("increase", amount))
        {
            StaticEffect::IncreaseMaximumHandSize {
                amount: Quantity::from(amount),
            }
        } else {
            return Err(rule_error());
        }
    };
    Ok(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect,
    })
}

fn ability_binding(identity: &str, path: &str, kind: &str) -> Value {
    json!({
        "id":format!("card:{identity}{path}#ability"), "path":path, "node":"ability",
        "source":format!("card:{identity}/source#object"),
        "face":format!("card:{identity}/faces/0#face"), "abilityKind":kind,
        "you":format!("card:{identity}{path}#you"), "rules":["113","109.5"]
    })
}

pub(super) fn is_flash_end_step_draw_candidate(abilities: &[Ability]) -> bool {
    abilities.len() == 3
        && abilities.iter().any(|ability| {
            matches!(ability, Ability::Keywords { keywords, .. }
                if matches!(keywords.as_slice(), [keyword] if keyword.keyword == "flash"))
        }) && abilities.iter().any(|ability| {
        matches!(ability, Ability::Static { effect: SourceStaticEffect::PlayerRule { rule }, .. }
                if lower_rule(rule).is_ok_and(|ability| matches!(ability.effect,
                    StaticEffect::ReduceMaximumHandSize { scope: HandSizeScope::Opponents, .. })))
    })
}

const END_STEP_DRAW: TriggerCondition = TriggerCondition::StepBegan {
    step: BeginningStep::End,
    turn_relation: TurnRelation::Controller,
};

pub(super) fn lower_end_step_draw(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> Result<TriggeredAbility, Failure> {
    // CR 109.5, 121.2, 513.1 and 603.3a: the trigger controller draws
    // each card when this end-step ability resolves.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if intervening.is_none()
        && matches!(trigger, SourceTrigger::Phase { phase, whose }
            if phase == "end-step" && whose.as_str() == Some("your"))
        && let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        && bindings.is_empty()
        && let [
            Step::Draw {
                who,
                amount: DrawAmount::Fixed { value },
                optional: false,
            },
        ] = steps.as_slice()
        && who.player == "you"
        && who.of.is_none()
        && who.selection_id.is_none()
        && *value > 0
    {
        return Ok(TriggeredAbility {
            death_filter: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: END_STEP_DRAW,
            intervening_if: None,
            program: SpellProgram {
                additional_sacrifice: None,
                optional_additional_costs: vec![],
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::draw(
                    PlayerExpr::you(Context::Triggered),
                    Quantity::from(i64::from(*value)),
                    source_block(block)?,
                )],
                modes: vec![],
            },
        });
    }
    Err(shape_error())
}

fn shape_error() -> Failure {
    unsupported(
        "static.player-rule",
        "unsupported complete hand size source",
    )
}

fn validate_end_step_draw(
    definition: &Definition,
    labels: &SourceLabels,
    abilities: &[Value],
) -> Result<usize, Failure> {
    let [ability] = definition.triggered.as_slice() else {
        return Err(shape_error());
    };
    let Some((count, source_block)) = (match ability.program.instructions.as_slice() {
        [draw] => draw.fixed_controller_draw(),
        _ => None,
    }) else {
        return Err(shape_error());
    };
    let block = source_block as usize;
    let count = count
        .to_string()
        .parse::<i64>()
        .map_err(|_| shape_error())?;
    let text = labels
        .blocks
        .get(block)
        .map(|s| normalization::literal_spelling(s));
    let amount = text
        .as_deref()
        .and_then(|text| text.strip_prefix("At the beginning of your end step, draw "))
        .and_then(|draw| draw.strip_suffix(if count == 1 { " card." } else { " cards." }));
    if !definition.characteristics.legendary
        || !definition.characteristics.additional_types.is_empty()
        || !definition.activated.is_empty()
        || labels.triggered.as_slice() != [source_block]
        || ability.condition != END_STEP_DRAW
        || count <= 0
        || !amount
            .is_some_and(|text| (count == 1 && text == "a") || matches_fixed_amount(text, count))
        || abilities.get(block)
            != Some(&json!({
                "kind":"triggered",
                "trigger":{"trigger":"phase","phase":"end-step","whose":"your"},
                "effects":[{"steps":[{
                    "effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":count}
                }]}]
            }))
    {
        return Err(shape_error());
    }
    Ok(block)
}

pub(super) fn validate_source(
    definition: &Definition,
    labels: &SourceLabels,
    binding: &Value,
    raw_abilities: &str,
) -> Result<(), Failure> {
    let [block] = labels.statics.as_slice() else {
        return Err(shape_error());
    };
    let [static_ability] = definition.static_abilities.as_slice() else {
        return Err(shape_error());
    };
    let static_block = *block as usize;
    let abilities: Vec<Value> = decode(raw_abilities, "abilities")?;
    let raw_static = abilities.get(static_block).ok_or_else(shape_error)?;
    let text = labels
        .blocks
        .get(static_block)
        .map(|s| normalization::literal_spelling(s))
        .ok_or_else(shape_error)?;
    let who_path = format!("/faces/0/abilities/{block}/effect/rule/who");
    let base = format!("/faces/0/abilities/{block}");
    let identity = &definition.oracle_id;
    let you = json!({"player":"you"});
    let mut actors = vec![(base.clone(), base.clone(), "static-source-controller")];
    let mut extra_nodes = Vec::new();
    let mut expected = Vec::new();
    let mut trigger_block = None;
    let (who, change, target, companion_keyword) = match &static_ability.effect {
        StaticEffect::UnlimitedControllerHandSize
            if text == STATIC_TEXT && abilities.len() == 1 + definition.mana_abilities.len() =>
        {
            (
                you.clone(),
                json!({"operation":"none"}),
                format!("card:{identity}{base}#you"),
                None,
            )
        }
        StaticEffect::ReduceMaximumHandSize { scope, amount } => {
            let value = amount.to_string().parse().map_err(|_| shape_error())?;
            extra_nodes.push(ability_binding(identity, &base, "static"));
            let (prefix, who, target) = match scope {
                HandSizeScope::Controller => {
                    ("Your", you.clone(), format!("card:{identity}{base}#you"))
                }
                HandSizeScope::Opponents => {
                    let who = json!({"player":"each-opponent"});
                    let id = format!("card:{identity}{who_path}#query");
                    extra_nodes.push(json!({
                        "id":id, "path":who_path, "node":"selection",
                        "scope":format!("card:{identity}{base}#ability"), "expression":who,
                        "cardinality":"many", "timing":"continuous", "selection":"evaluate-reference",
                        "rules":["608.2h"]
                    }));
                    ("Each opponent's", who, id)
                }
            };
            let spelled = text
                .strip_prefix(&format!("{prefix} maximum hand size is reduced by "))
                .and_then(|tail| tail.strip_suffix('.'));
            if !spelled.is_some_and(|spelled| matches_fixed_amount(spelled, value))
                || !definition.mana_abilities.is_empty()
            {
                return Err(shape_error());
            }
            let keyword = if *scope == HandSizeScope::Controller {
                Some((
                    KeywordAbility::Flying,
                    "Flying",
                    "flying",
                    "702.9",
                    "static-source-controller",
                ))
            } else if !definition.triggered.is_empty() {
                let block = validate_end_step_draw(definition, labels, &abilities)?;
                trigger_block = Some(block);
                let path = format!("/faces/0/abilities/{block}");
                let trigger = format!("{path}/trigger");
                let draw = format!("{path}/effects/0/steps/0");
                let actor = format!("card:{identity}{path}#you");
                extra_nodes.push(ability_binding(identity, &path, "triggered"));
                actors.push((path.clone(), path.clone(), "trigger-controller"));
                for (event_path, operation, inputs) in [
                    (&trigger, "trigger:phase", json!([])),
                    (&draw, "draw", json!([actor])),
                ] {
                    extra_nodes.push(json!({
                        "id":format!("card:{identity}{event_path}#event"), "path":event_path,
                        "node":"event", "scope":format!("card:{identity}{path}#ability"),
                        "operation":operation, "inputs":inputs, "availability":"when-performed",
                        "rules":["608.2c"]
                    }));
                }
                expected.extend([
                    (format!("{trigger}/whose"), you.clone(), actor.clone()),
                    (format!("{draw}/who"), you.clone(), actor),
                ]);
                Some((
                    KeywordAbility::Flash,
                    "Flash",
                    "flash",
                    "702.8",
                    "prospective-caster",
                ))
            } else {
                None
            };
            (who, fixed_change("reduce", value), target, Some(keyword))
        }
        StaticEffect::IncreaseMaximumHandSize { amount } => {
            let value = amount.to_string().parse().map_err(|_| shape_error())?;
            let spelled = text
                .strip_prefix("Your maximum hand size is increased by ")
                .and_then(|tail| tail.strip_suffix('.'));
            if !spelled.is_some_and(|spelled| matches_fixed_amount(spelled, value))
                || !definition.mana_abilities.is_empty()
            {
                return Err(shape_error());
            }
            extra_nodes.push(ability_binding(identity, &base, "static"));
            (
                you.clone(),
                fixed_change("increase", value),
                format!("card:{identity}{base}#you"),
                Some(Some((
                    KeywordAbility::Defender,
                    "Defender",
                    "defender",
                    "702.3",
                    "static-source-controller",
                ))),
            )
        }
        _ => return Err(shape_error()),
    };
    if let Some(companion_keyword) = companion_keyword {
        match (abilities.len(), definition.keywords.as_slice()) {
            (1, []) if trigger_block.is_none() => {}
            (len, [keyword])
                if len == 2 + usize::from(trigger_block.is_some())
                    && definition.characteristics.kind == CardKind::Creature =>
            {
                let (expected, spelling, name, rule, role) =
                    companion_keyword.ok_or_else(shape_error)?;
                let block = (0..len)
                    .find(|index| *index != static_block && Some(*index) != trigger_block)
                    .ok_or_else(shape_error)?;
                if *keyword != expected
                    || labels
                        .blocks
                        .get(block)
                        .map(|s| normalization::literal_spelling(s))
                        .as_deref()
                        != Some(spelling)
                    || abilities[block] != json!({"kind":"keywords","keywords":[{"keyword":name}]})
                {
                    return Err(shape_error());
                }
                let path = format!("/faces/0/abilities/{block}");
                let keyword = format!("{path}/keywords/0");
                extra_nodes.push(ability_binding(identity, &path, "keywords"));
                actors.push((path.clone(), path.clone(), "static-source-controller"));
                actors.push((keyword.clone(), path.clone(), role));
                extra_nodes.push(json!({
                    "id":format!("card:{identity}{keyword}#keyword"), "path":keyword,
                    "node":"keyword-ability", "scope":format!("card:{identity}{path}#ability"),
                    "keyword":name, "abilities":["static"],
                    "you":format!("card:{identity}{keyword}#you"), "rules":[rule]
                }));
            }
            _ => return Err(shape_error()),
        }
    }
    if *raw_static
        != json!({
            "kind":"static", "effect":{"static":"player-rule", "rule":hand_size_rule(&who, change)}
        })
    {
        return Err(shape_error());
    }

    expected.push((who_path, who, target));
    if let [ability] = definition.mana_abilities.as_slice() {
        let (text, production) = match &ability.production {
            ManaProduction::Fixed { symbols } if symbols.as_slice() == [Color::C] => {
                ("{T}: Add {C}.", json!({"mana":"fixed","symbols":["C"]}))
            }
            ManaProduction::AnyColor => (
                "{T}: Add one mana of any color.",
                json!({"mana":"any-color","amount":{"amount":"fixed","value":1}}),
            ),
            _ => return Err(shape_error()),
        };
        let mana_block = ability.source_block as usize;
        if labels
            .blocks
            .get(mana_block)
            .map(|s| normalization::literal_spelling(s))
            .as_deref()
            != Some(text)
            || abilities[mana_block]
                != json!({
                    "kind":"activated", "costs":[{"cost":"tap-self"}],
                    "effects":[{"steps":[{"effect":"add-mana","who":you,"mana":production}]}]
                })
        {
            return Err(shape_error());
        }
        let base = format!("/faces/0/abilities/{mana_block}");
        let actor = format!("card:{identity}{base}#you");
        actors.push((base.clone(), base.clone(), "activator"));
        expected.extend([
            (
                format!("{base}/costs/0/source"),
                json!({"ref":"self"}),
                format!("card:{identity}/source#object"),
            ),
            (format!("{base}/costs/0/payer"), you.clone(), actor.clone()),
            (format!("{base}/effects/0/steps/0/who"), you, actor),
        ]);
    } else if !definition.mana_abilities.is_empty() {
        return Err(shape_error());
    }

    let binding_error = || unsupported("binding", "requires complete hand size binding closure");
    let nodes = binding["nodes"].as_array().ok_or_else(binding_error)?;
    let references = binding["references"].as_array().ok_or_else(binding_error)?;
    if binding["identity"] != *identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["counts"]
            != json!({"obligations":expected.len(),"bound":expected.len(),"unresolved":0})
        || references.len() != expected.len()
        || actors.iter().any(|(path, ability, role)| {
            let prefix = format!("card:{identity}{path}");
            let id = format!("{prefix}#you");
            let scope = format!("card:{identity}{ability}#ability");
            let mut matches = nodes.iter().filter(|n| n["id"] == id);
            !matches.next().is_some_and(|node| {
                node["node"] == "runtime-player"
                    && node["path"] == *path
                    && node["scope"] == scope
                    && node["role"] == *role
            }) || matches.next().is_some()
        })
        || extra_nodes.iter().any(|expected| {
            let mut matches = nodes.iter().filter(|node| node["id"] == expected["id"]);
            matches.next() != Some(expected) || matches.next().is_some()
        })
        || expected.iter().any(|(path, expression, target)| {
            let mut matches = references.iter().filter(|r| r["path"] == *path);
            !matches.next().is_some_and(|reference| {
                reference["expression"] == *expression
                    && reference["target"] == *target
                    && reference["binding"] == "bound"
            }) || matches.next().is_some()
        })
    {
        return Err(binding_error());
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn flash_end_step_draw() {
    let phase = |whose| SourceTrigger::Phase {
        phase: "end-step".into(),
        whose: json!(whose),
    };
    let draw = |value, optional| Sentence {
        condition: None,
        condition_kind: None,
        bindings: vec![],
        steps: vec![Step::Draw {
            who: PlayerSelector {
                player: "you".into(),
                selection_id: None,
                of: None,
            },
            amount: DrawAmount::Fixed { value },
            optional,
        }],
    };
    for count in [1, 7, u32::MAX] {
        let ability = lower_end_step_draw(&phase("your"), &[draw(count, false)], None, 2).unwrap();
        assert_eq!(ability.condition, END_STEP_DRAW);
        assert_eq!(
            ability.program.instructions,
            [Instruction::draw(
                PlayerExpr::you(Context::Triggered),
                Quantity::from(i64::from(count)),
                2
            )]
        );
    }
    // CR 121.1: "draw a card" is the canonical single draw. The complete
    // source check admits it like any other positive count.
    for (count, printed) in [(1, "a card"), (7, "seven cards")] {
        let ability = lower_end_step_draw(&phase("your"), &[draw(count, false)], None, 1).unwrap();
        let mut definition = crate::catalog::embedded_catalog("base")
            .unwrap()
            .definitions
            .remove(0);
        definition.characteristics.legendary = true;
        definition.characteristics.additional_types.clear();
        definition.activated.clear();
        definition.triggered = vec![ability];
        let labels = SourceLabels {
            text: String::new(),
            blocks: vec![
                "Flash".into(),
                format!("At the beginning of your end step, draw {printed}."),
            ],
            activated: vec![],
            triggered: vec![1],
            mana: vec![],
            statics: vec![],
            spell_modes: vec![],
        };
        let abilities = vec![
            json!({"kind":"keyword"}),
            json!({"kind":"triggered","trigger":{"trigger":"phase","phase":"end-step","whose":"your"},
                "effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":count}}]}]}),
        ];
        assert_eq!(
            validate_end_step_draw(&definition, &labels, &abilities).unwrap(),
            1,
            "{printed}"
        );
    }
    assert!(lower_end_step_draw(&phase("each-opponent"), &[draw(7, false)], None, 1).is_err());
    assert!(lower_end_step_draw(&phase("your"), &[draw(7, true)], None, 1).is_err());
    assert!(lower_end_step_draw(&phase("your"), &[draw(0, false)], None, 1).is_err());

    if let Ok(path) = std::env::var("MTG_FLASH_END_STEP_DRAW_ARCHIVE_PROBE") {
        let raw = std::fs::read_to_string(path).unwrap();
        let mut lines = raw.lines();
        let line = lines.next().unwrap();
        assert!(lines.next().is_none());
        let definition = match compile(fixtures::MANIFEST, line) {
            CompileResult::Lowered { definition } => definition,
            failure => panic!("{failure:?}"),
        };
        assert_eq!(definition.provenance.source_index, 35531);
        assert_eq!(definition.keywords, [KeywordAbility::Flash]);
        assert_eq!(
            definition.triggered,
            [lower_end_step_draw(&phase("your"), &[draw(7, false)], None, 1).unwrap()]
        );
        assert_eq!(
            definition.static_abilities,
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::ReduceMaximumHandSize {
                    scope: HandSizeScope::Opponents,
                    amount: Quantity::from(7),
                },
            }]
        );
        assert_eq!(
            definition.spell.as_ref().unwrap().timing,
            SpellTiming::Instant
        );

        let wrong_ast = tests::changed_source(line, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][1]["effects"][0]["steps"][0]["optional"] = json!(false);
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &wrong_ast),
            CompileResult::Unsupported { path, .. } if path == "static.player-rule"
        ));
        let wrong_text = tests::changed_source(line, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][1]["effects"][0]["steps"][0]["amount"]["value"] = json!(6);
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &wrong_text),
            CompileResult::Unsupported { path, .. } if path == "static.player-rule"
        ));
        for role in ["trigger-controller", "prospective-caster"] {
            let wrong_actor = tests::changed_source(line, |record| {
                let actor = record["binding"]["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|node| node["role"] == role)
                    .unwrap();
                actor["role"] = json!("static-source-controller");
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &wrong_actor),
                    CompileResult::Unsupported { path, .. } if path == "binding"
                ),
                "{role}"
            );
        }
    }
}

#[cfg(test)]
#[test]
fn unlimited_hand_size_source() {
    let rule =
        json!({"who":{"player":"you"},"rule":"maximum-hand-size","change":{"operation":"none"}});
    let effect = lower_rule(&rule).unwrap_or_else(|_| panic!("exact player rule must lower"));
    assert_eq!(effect.effect, StaticEffect::UnlimitedControllerHandSize);
    assert_eq!(effect.functioning_zone, FunctioningZone::Battlefield);
    for (path, value) in [
        ("/who/player", json!("target")),
        ("/rule", json!("starting-hand-size")),
        ("/change/operation", json!("set")),
        ("/who", json!({"player":"you","of":null})),
        ("/change", json!({"operation":"none","amount":null})),
    ] {
        let mut changed = rule.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(lower_rule(&changed).is_err(), "{path}");
    }

    if let Ok(path) = std::env::var("MTG_UNLIMITED_HAND_SIZE_ARCHIVE_PROBE") {
        let raw = std::fs::read_to_string(path).unwrap();
        let mut lines = raw.lines();
        for (index, kind, production) in [
            (15668, CardKind::Creature, None),
            (19713, CardKind::Artifact, None),
            (20933, CardKind::Artifact, Some(ManaProduction::AnyColor)),
            (
                23123,
                CardKind::Artifact,
                Some(ManaProduction::Fixed {
                    symbols: vec![Color::C],
                }),
            ),
            (
                29280,
                CardKind::Land,
                Some(ManaProduction::Fixed {
                    symbols: vec![Color::C],
                }),
            ),
        ] {
            let line = lines.next().unwrap();
            let definition = match compile(fixtures::MANIFEST, line) {
                CompileResult::Lowered { definition } => definition,
                failure => panic!("{index}: {failure:?}"),
            };
            assert_eq!(definition.provenance.source_index, index);
            assert_eq!(definition.characteristics.kind, kind);
            assert_eq!(definition.static_abilities, [effect.clone()]);
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::UnlimitedHandSizeV1)
            );
            assert_eq!(
                definition.mana_abilities.len(),
                usize::from(production.is_some())
            );
            assert_eq!(
                definition.mana_abilities.first().map(|a| &a.production),
                production.as_ref()
            );

            let wrong_controller = tests::changed_source(line, |record| {
                let nodes = record["binding"]["nodes"].as_array_mut().unwrap();
                let controller = nodes
                    .iter_mut()
                    .find(|node| node["role"] == "static-source-controller")
                    .unwrap();
                controller["role"] = json!("owner");
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &wrong_controller),
                    CompileResult::Unsupported { path, .. } if path == "binding"
                ),
                "{index}"
            );
        }
        assert!(lines.next().is_none());
    }
}

#[cfg(test)]
#[test]
fn reduced_hand_size() {
    use HandSizeScope::{Controller, Opponents};

    let reduction =
        |player, amount| hand_size_rule(&json!({"player":player}), fixed_change("reduce", amount));
    let static_ability = |scope, amount| StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ReduceMaximumHandSize {
            scope,
            amount: Quantity::from(amount),
        },
    };
    for (scope, player) in [(Controller, "you"), (Opponents, "each-opponent")] {
        assert_eq!(
            lower_rule(&reduction(player, 12)).unwrap(),
            static_ability(scope, 12)
        );
    }
    let rule = reduction("you", 2);
    for (path, value) in [
        ("/who", json!({"player":"you", "of":null})),
        ("/who/player", json!("each-player")),
        ("/rule", json!("starting-hand-size")),
        ("/change/operation", json!("set")),
        ("/change/amount/amount", json!("variable")),
        ("/change/amount/value", json!(0)),
        ("/change/amount/value", json!(-1)),
        ("/change/amount/value", json!(2.5)),
    ] {
        let mut changed = rule.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(lower_rule(&changed).is_err(), "{path}");
    }

    if let Ok(path) = std::env::var("MTG_REDUCED_HAND_SIZE_ARCHIVE_PROBE") {
        let raw = std::fs::read_to_string(path).unwrap();
        let mut lines = raw.lines();
        for (index, scope, amount, keywords) in [
            (17708, Opponents, 2, &[][..]),
            (20630, Controller, 3, &[KeywordAbility::Flying][..]),
            (23262, Controller, 2, &[KeywordAbility::Flying][..]),
            (31322, Opponents, 1, &[][..]),
            (34567, Controller, 4, &[KeywordAbility::Flying][..]),
        ] {
            let line = lines.next().unwrap();
            let definition = match compile(fixtures::MANIFEST, line) {
                CompileResult::Lowered { definition } => definition,
                failure => panic!("{index}: {failure:?}"),
            };
            assert_eq!(definition.provenance.source_index, index);
            assert_eq!(definition.characteristics.kind, CardKind::Creature);
            assert_eq!(definition.static_abilities, [static_ability(scope, amount)]);
            assert_eq!(definition.keywords, keywords);
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::ReducedHandSizeV1)
            );

            let wrong_actor = tests::changed_source(line, |record| {
                let nodes = record["binding"]["nodes"].as_array_mut().unwrap();
                let actor_id = nodes
                    .iter()
                    .find(|node| node["abilityKind"] == "static")
                    .unwrap()["you"]
                    .clone();
                nodes
                    .iter_mut()
                    .find(|node| node["id"] == actor_id)
                    .unwrap()["role"] = json!("owner");
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &wrong_actor),
                    CompileResult::Unsupported { path, .. } if path == "binding"
                ),
                "{index}"
            );

            let wrong_amount = tests::changed_source(line, |record| {
                let result = &mut record["faces"][0]["result"];
                let block = usize::from(!keywords.is_empty());
                result["abilities"][block]["effect"]["rule"]["change"] =
                    fixed_change("reduce", amount + 1);
                result["lines"][block]["ability"] = result["abilities"][block].clone();
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &wrong_amount),
                    CompileResult::Unsupported { path, .. } if path == "static.player-rule"
                ),
                "{index}"
            );
        }
        assert!(lines.next().is_none());
    }
}

#[cfg(test)]
#[test]
fn increased_hand_size() {
    let rule = hand_size_rule(&json!({"player":"you"}), fixed_change("increase", 1));
    let effect = StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::IncreaseMaximumHandSize {
            amount: Quantity::from(1),
        },
    };
    assert_eq!(lower_rule(&rule).unwrap(), effect);

    for (path, value) in [
        ("/who/player", json!("each-opponent")),
        ("/who/player", json!("each-player")),
        ("/who", json!({"player":"you","of":null})),
        ("/rule", json!("starting-hand-size")),
        (
            "/change",
            json!({"operation":"increase","amount":{"amount":"fixed","value":1},"duration":null}),
        ),
        ("/change/amount/amount", json!("variable")),
        ("/change/amount/value", json!(0)),
        ("/change/amount/value", json!(-1)),
        ("/change/amount/value", json!(1.5)),
    ] {
        let mut changed = rule.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(lower_rule(&changed).is_err(), "{path}");
    }

    if let Ok(path) = std::env::var("MTG_INCREASED_HAND_SIZE_ARCHIVE_PROBE") {
        let raw = std::fs::read_to_string(path).unwrap();
        let mut lines = raw.lines();
        let line = lines.next().unwrap();
        assert!(lines.next().is_none());
        let definition = match compile(fixtures::MANIFEST, line) {
            CompileResult::Lowered { definition } => definition,
            failure => panic!("{failure:?}"),
        };
        assert_eq!(definition.provenance.source_index, 23328);
        assert_eq!(definition.characteristics.kind, CardKind::Creature);
        assert_eq!(definition.static_abilities, [effect]);
        assert_eq!(definition.keywords, [KeywordAbility::Defender]);
        assert!(
            definition
                .capabilities
                .contains(&Capability::IncreasedHandSizeV1)
        );

        let wrong_amount = tests::changed_source(line, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][1]["effect"]["rule"]["change"] = fixed_change("increase", 2);
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &wrong_amount),
            CompileResult::Unsupported { path, .. } if path == "static.player-rule"
        ));

        for node_path in ["/faces/0/abilities/1", "/faces/0/abilities/0/keywords/0"] {
            let wrong_controller = tests::changed_source(line, |record| {
                let actor = record["binding"]["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|node| node["path"] == node_path && node["node"] == "runtime-player")
                    .unwrap();
                actor["role"] = json!("owner");
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &wrong_controller),
                    CompileResult::Unsupported { path, .. } if path == "binding"
                ),
                "{node_path}"
            );
        }

        let wrong_keyword_rule = tests::changed_source(line, |record| {
            let keyword = record["binding"]["nodes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|node| node["node"] == "keyword-ability")
                .unwrap();
            keyword["rules"] = json!(["702.9"]);
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &wrong_keyword_rule),
            CompileResult::Unsupported { path, .. } if path == "binding"
        ));
    }
}

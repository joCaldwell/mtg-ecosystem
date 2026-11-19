//! CR 109.5, 402.2, 611.3 and 613.11: static maximum hand-size rules.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const STATIC_TEXT: &str = "You have no maximum hand size.";

fn hand_size_rule(who: &Value, change: Value) -> Value {
    json!({"who":who,"rule":"maximum-hand-size","change":change})
}

fn reduction_change(amount: i64) -> Value {
    json!({"operation":"reduce","amount":{"amount":"fixed","value":amount}})
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
        if *rule != hand_size_rule(who, reduction_change(amount)) {
            return Err(rule_error());
        }
        StaticEffect::ReduceMaximumHandSize {
            scope,
            amount: Quantity::from(amount),
        }
    };
    Ok(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect,
    })
}

// Match the archive producer's spelled-number vocabulary and canonical decimal text.
fn matches_reduction_amount(text: &str, amount: i64) -> bool {
    const UNITS: [&str; 20] = [
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    let decimal = amount.to_string();
    if text == decimal {
        return true;
    }
    match amount as usize {
        n @ 1..=20 => text == UNITS[n - 1],
        50 => text == "fifty",
        n @ 21..=99 if n % 10 != 0 => {
            text.split_once('-') == Some((TENS[n / 10 - 2], UNITS[n % 10 - 1]))
        }
        _ => false,
    }
}

fn ability_binding(identity: &str, path: &str, kind: &str) -> Value {
    json!({
        "id":format!("card:{identity}{path}#ability"), "path":path, "node":"ability",
        "source":format!("card:{identity}/source#object"),
        "face":format!("card:{identity}/faces/0#face"), "abilityKind":kind,
        "you":format!("card:{identity}{path}#you"), "rules":["113","109.5"]
    })
}

pub(super) fn validate_source(
    definition: &Definition,
    labels: &SourceLabels,
    binding: &Value,
    raw_abilities: &str,
) -> Result<(), Failure> {
    let shape_error = || {
        unsupported(
            "static.player-rule",
            "unsupported complete hand size source",
        )
    };
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
    let (who, change, target) = match &static_ability.effect {
        StaticEffect::UnlimitedControllerHandSize
            if text == STATIC_TEXT && abilities.len() == 1 + definition.mana_abilities.len() =>
        {
            (
                you.clone(),
                json!({"operation":"none"}),
                format!("card:{identity}{base}#you"),
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
            if !spelled.is_some_and(|spelled| matches_reduction_amount(spelled, value))
                || !definition.mana_abilities.is_empty()
            {
                return Err(shape_error());
            }
            match (abilities.as_slice(), definition.keywords.as_slice()) {
                ([_], []) => {}
                ([_, _], [KeywordAbility::Flying])
                    if *scope == HandSizeScope::Controller
                        && definition.characteristics.kind == CardKind::Creature =>
                {
                    let flying = 1 - static_block;
                    if labels
                        .blocks
                        .get(flying)
                        .map(|s| normalization::literal_spelling(s))
                        .as_deref()
                        != Some("Flying")
                        || abilities[flying]
                            != json!({"kind":"keywords","keywords":[{"keyword":"flying"}]})
                    {
                        return Err(shape_error());
                    }
                    let path = format!("/faces/0/abilities/{flying}");
                    let keyword = format!("{path}/keywords/0");
                    extra_nodes.push(ability_binding(identity, &path, "keywords"));
                    actors.push((path.clone(), path.clone(), "static-source-controller"));
                    actors.push((keyword.clone(), path.clone(), "static-source-controller"));
                    extra_nodes.push(json!({
                        "id":format!("card:{identity}{keyword}#keyword"), "path":keyword,
                        "node":"keyword-ability", "scope":format!("card:{identity}{path}#ability"),
                        "keyword":"flying", "abilities":["static"],
                        "you":format!("card:{identity}{keyword}#you"), "rules":["702.9"]
                    }));
                }
                _ => return Err(shape_error()),
            }
            (who, reduction_change(value), target)
        }
        _ => return Err(shape_error()),
    };
    if *raw_static
        != json!({
            "kind":"static", "effect":{"static":"player-rule", "rule":hand_size_rule(&who, change)}
        })
    {
        return Err(shape_error());
    }

    let mut expected = vec![(who_path, who, target)];
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
        |player, amount| hand_size_rule(&json!({"player":player}), reduction_change(amount));
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
        ("/change/operation", json!("increase")),
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
                    reduction_change(amount + 1);
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

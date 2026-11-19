//! CR 109.5 and 611.3: the static rule concerns the current source controller.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const STATIC_TEXT: &str = "You have no maximum hand size.";

pub(super) fn lower_rule(rule: &Value) -> Result<StaticAbility, Failure> {
    if *rule
        != json!({"who":{"player":"you"},"rule":"maximum-hand-size","change":{"operation":"none"}})
    {
        return Err(unsupported(
            "static.player-rule",
            "unsupported complete player rule",
        ));
    }
    Ok(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::UnlimitedControllerHandSize,
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
            "unsupported complete unlimited hand size source",
        )
    };
    let [block] = labels.statics.as_slice() else {
        return Err(shape_error());
    };
    let static_block = *block as usize;
    let abilities: Vec<Value> = decode(raw_abilities, "abilities")?;
    if labels
        .blocks
        .get(static_block)
        .map(|s| normalization::literal_spelling(s))
        .as_deref()
        != Some(STATIC_TEXT)
        || abilities.len() != 1 + definition.mana_abilities.len()
    {
        return Err(shape_error());
    }

    let identity = &definition.oracle_id;
    let base = format!("/faces/0/abilities/{block}");
    let you = json!({"player":"you"});
    let mut actors = vec![(base.clone(), "static-source-controller")];
    let mut expected = vec![(
        format!("{base}/effect/rule/who"),
        you.clone(),
        format!("card:{identity}{base}#you"),
    )];
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
        actors.push((base.clone(), "activator"));
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

    let binding_error = || {
        unsupported(
            "binding",
            "requires complete unlimited hand size binding closure",
        )
    };
    let nodes = binding["nodes"].as_array().ok_or_else(binding_error)?;
    let references = binding["references"].as_array().ok_or_else(binding_error)?;
    if binding["identity"] != *identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["counts"]
            != json!({"obligations":expected.len(),"bound":expected.len(),"unresolved":0})
        || references.len() != expected.len()
        || actors.iter().any(|(path, role)| {
            let prefix = format!("card:{identity}{path}");
            let id = format!("{prefix}#you");
            let scope = format!("{prefix}#ability");
            let mut matches = nodes.iter().filter(|n| n["id"] == id);
            !matches.next().is_some_and(|node| {
                node["node"] == "runtime-player" && node["scope"] == scope && node["role"] == *role
            }) || matches.next().is_some()
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

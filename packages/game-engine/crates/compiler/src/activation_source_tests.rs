//! Compiler source-identity regressions. Synthetic source bodies were parsed
//! and reference-analyzed independently before the native API reproduced the
//! defect. The compact graph deltas below are that captured diagnostic output,
//! not runtime source proofs or edited frozen card records.
use super::*;
use serde_json::{Value, json};
const BOMBARDMENT: &str = include_str!("../../../tests/fixtures/goblin-bombardment.json");
const SALVE: &str = include_str!("../../../tests/fixtures/healing-salve.json");
const BOMBARDMENT_NODES: &str = r#"[
  {"id":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/costs/0#event","path":"/faces/0/abilities/0/costs/0","node":"event","scope":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0#ability","operation":"cost:sacrifice","inputs":[],"availability":"when-performed","rules":["608.2c"]},
  {"id":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/costs/0#quantity","path":"/faces/0/abilities/0/costs/0","node":"event-result","event":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/costs/0#event","result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]},
  {"id":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0#ability/targets/line%3A0%3A10","path":"/faces/0/abilities/0/effects/0/steps/0/to","node":"selection","scope":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0#ability","expression":{"ref":"any-target","selectionId":"line:0:10"},"cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]},
  {"id":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/effects/0/steps/0#event","path":"/faces/0/abilities/0/effects/0/steps/0","node":"event","scope":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0#ability","operation":"damage","inputs":["card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/costs/0/what#query","card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0#ability/targets/line%3A0%3A10"],"availability":"when-performed","rules":["608.2c"]},
  {"id":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/effects/0/steps/0#quantity","path":"/faces/0/abilities/0/effects/0/steps/0","node":"event-result","event":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/effects/0/steps/0#event","result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}
]"#;
const BOMBARDMENT_REFERENCES: &str = r#"[
  {"path":"/faces/0/abilities/0/effects/0/steps/0/source","expression":{"ref":"it"},"binding":"bound","target":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0/costs/0/what#query","rules":["608.2k","400.7j","608.2h"]},
  {"path":"/faces/0/abilities/0/effects/0/steps/0/to","expression":{"ref":"any-target","selectionId":"line:0:10"},"binding":"bound","target":"card:edad60c6-80de-4033-af1b-a703ac332983/faces/0/abilities/0#ability/targets/line%3A0%3A10","rules":["601.2c"]}
]"#;
const MODAL_NODES: &str = r#"[
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0#event","path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0","node":"event","scope":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability","operation":"create-token","inputs":["card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#you"],"availability":"when-performed","rules":["608.2c"]},
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0#objects","path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0","node":"event-result","event":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0#event","result":"objects","cardinality":"one","availability":"if-produced","identity":"new-objects","description":{"cls":"token","types":["creature"]},"rules":["608.2h"]},
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0#quantity","path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0","node":"event-result","event":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0#event","result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]},
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability/targets/line%3A1%3A17","path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0/to","node":"selection","scope":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability","expression":{"ref":"any-target","selectionId":"line:1:17"},"cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]},
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0#event","path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0","node":"event","scope":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability","operation":"damage","inputs":["card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0#objects","card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability/targets/line%3A1%3A17"],"availability":"when-performed","rules":["608.2c"]},
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0#quantity","path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0","node":"event-result","event":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0#event","result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]},
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability/targets/line%3A2%3A12","path":"/faces/0/abilities/0/effects/0/steps/0/options/1/effects/0/steps/0/prevention/to/0","node":"selection","scope":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability","expression":{"ref":"any-target","selectionId":"line:2:12"},"cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]},
  {"id":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/1/effects/0/steps/0#event","path":"/faces/0/abilities/0/effects/0/steps/0/options/1/effects/0/steps/0","node":"event","scope":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability","operation":"prevent-damage","inputs":[],"availability":"when-performed","rules":["608.2c"]}
]"#;
const MODAL_REFERENCES: &str = r#"[
  {"path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0/who","expression":{"player":"you"},"binding":"bound","target":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#you","rules":["109.5"]},
  {"path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0/source","expression":{"ref":"it"},"binding":"bound","target":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0/effects/0/steps/0/options/0/effects/0/steps/0#objects","rules":["608.2k","400.7j","608.2h"]},
  {"path":"/faces/0/abilities/0/effects/0/steps/0/options/0/effects/1/steps/0/to","expression":{"ref":"any-target","selectionId":"line:1:17"},"binding":"bound","target":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability/targets/line%3A1%3A17","rules":["601.2c"]},
  {"path":"/faces/0/abilities/0/effects/0/steps/0/options/1/effects/0/steps/0/prevention/to/0","expression":{"ref":"any-target","selectionId":"line:2:12"},"binding":"bound","target":"card:8da8644c-75a1-4fe9-8e94-900d948d631c/faces/0/abilities/0#ability/targets/line%3A2%3A12","rules":["601.2c"]}
]"#;
const MODAL_FEATURES: &str = r#"[
  {"feature":"damage-prevention","path":"/faces/0/abilities/0/effects/0/steps/0/options/1/effects/0/steps/0","message":"Prevention needs the engine's future damage events, source-choice eligibility, shield consumption, and prevented quantities.","rules":["609.7","615.5","615.7","615.8","615.10","615.12"]}
]"#;

fn graph_delta(
    record: &mut Value,
    node_prefix: usize,
    reference_prefix: usize,
    nodes: &str,
    references: &str,
    features: &str,
    fully_bound: bool,
) {
    let binding = &mut record["binding"];
    let original_nodes = binding["nodes"].as_array_mut().unwrap();
    original_nodes.truncate(node_prefix);
    original_nodes.extend(strict_read::<Vec<Value>>(nodes).unwrap());
    let original_references = binding["references"].as_array_mut().unwrap();
    original_references.truncate(reference_prefix);
    original_references.extend(strict_read::<Vec<Value>>(references).unwrap());
    binding["diagnostics"] = json!([]);
    binding["counts"] = json!({"obligations":4,"bound":4,"unresolved":0});
    binding["referencesBound"] = true.into();
    binding["engineFeatures"] = strict_read(features).unwrap();
    binding["fullyBound"] = fully_bound.into();
}
fn set_source_text(record: &mut Value, text: &str) {
    let lines = source_lines(
        text,
        record["source"]["name"].as_str().unwrap(),
        record["source"]["type_line"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(lines.len(), 1);
    record["source"]["oracle_text"] = text.into();
    let result = &mut record["faces"][0]["result"];
    result["lines"][0]["text"] = lines[0].clone().into();
    result["lines"][0]["ability"] = result["abilities"][0].clone();
}
fn assert_captured(record: &str, ast: &str, binding: &str, source: &str) {
    let record: Value = strict_read(record).unwrap();
    assert_eq!(
        sha256(
            canonical(&json!([record["faces"][0]["result"]["abilities"]]))
                .unwrap()
                .as_bytes()
        ),
        ast
    );
    assert_eq!(
        sha256(canonical(&record["binding"]).unwrap().as_bytes()),
        binding
    );
    assert_eq!(
        sha256(canonical(&record["source"]).unwrap().as_bytes()),
        source
    );
}
fn selected_sacrifice_mutant() -> String {
    let record = tests::changed_source(BOMBARDMENT, |record| {
        let step = &mut record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0];
        step["source"] = json!({"ref":"it"});
        step["to"]["selectionId"] = "line:0:10".into();
        set_source_text(
            record,
            "Sacrifice a creature: It deals 1 damage to any target.",
        );
        graph_delta(
            record,
            5,
            2,
            BOMBARDMENT_NODES,
            BOMBARDMENT_REFERENCES,
            "[]",
            true,
        );
    });
    assert_captured(
        &record,
        "00164264e20133282ca0780f50a45242511041c9b6bf4385d097ce8959bb75cb",
        "e77cbdbb797622b0c3d2b3a768815d15292cfdd58fc75b69be62e0b6c473d681",
        "a3d563413cc26db37b7450faa4814fb13af227c6084fdd5cfb011471722d4eb4",
    );
    record
}
fn modal_mutant(explicit_self: bool) -> String {
    let record = tests::changed_source(SALVE, |record| {
        let name = if explicit_self { "Healing Salve" } else { "It" };
        let text = format!(
            "Choose one —\n• Create a 1/1 white Soldier creature token. {name} deals 1 damage to any target.\n• Prevent the next 3 damage that would be dealt to any target this turn."
        );
        let options =
            &mut record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["options"];
        options[0]["effects"] = json!([
            {"steps":[{"effect":"create-token","count":{"amount":"fixed","value":1},"token":{
                "colors":["white"],"supertypes":[],"types":["creature"],"subtypes":["Soldier"],"keywords":[],
                "power":{"amount":"fixed","value":1},"toughness":{"amount":"fixed","value":1}}}]},
            {"steps":[{"effect":"damage","source":{"ref":if explicit_self {"self"} else {"it"}},
                "amount":{"amount":"fixed","value":1},"to":{"to":"any-target","selectionId":"line:1:17"}}]}
        ]);
        set_source_text(record, &text);
        graph_delta(
            record,
            4,
            0,
            MODAL_NODES,
            MODAL_REFERENCES,
            MODAL_FEATURES,
            false,
        );
        if explicit_self {
            let source = format!(
                "card:{}/source#object",
                record["oracleIdentity"].as_str().unwrap()
            );
            for node in record["binding"]["nodes"].as_array_mut().unwrap() {
                if node["operation"] == "damage" {
                    node["inputs"][0] = source.clone().into();
                }
            }
            for reference in record["binding"]["references"].as_array_mut().unwrap() {
                if reference["path"].as_str().unwrap().ends_with("/source") {
                    reference["expression"] = json!({"ref":"self"});
                    reference["target"] = source.clone().into();
                    reference["rules"] = json!(["201.5"]);
                }
            }
        }
    });
    if explicit_self {
        assert_captured(
            &record,
            "89fa501e8d290da0277649647a76b8da54c8e0766ac1c79ab5801824dedb8179",
            "3bfa8298f6c793ab95ef9061e0f0a9f329a7820cc3750f78d4f1f89505bcdb4e",
            "529471147a2cba252146fe2641ad6d840d82c81c586e8b7e898a5d8b84f3b673",
        );
    } else {
        assert_captured(
            &record,
            "b8cc5c494b544b6fb0d2e0125c9431b97d87bb2bff4a9060537382f2cb0f4623",
            "ed040ac10fbf38901f25c45627f807feb7bf3039bf0809e0eef64550820551d4",
            "b6dce599e6cc45f612cc8081518d14e190a0b0fc4eeb5ef4d6d68f83d13dabcd",
        );
    }
    record
}
#[test]
fn selected_sacrifice_it_cannot_become_the_activation_source() {
    let mutant = selected_sacrifice_mutant();
    let record: Value = strict_read(&mutant).unwrap();
    assert!(
        record["binding"]["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["expression"] == json!({"ref":"it"})
                && r["target"]
                    .as_str()
                    .unwrap()
                    .ends_with("/costs/0/what#query"))
    );
    let result = compile(fixtures::MANIFEST, &mutant);
    assert!(
        matches!(result, CompileResult::Unsupported { ref path, .. } if path == "abilities.effects.source"),
        "{result:?}"
    );
}
#[test]
fn modal_token_it_cannot_become_the_spell_source() {
    let mutant = modal_mutant(false);
    let record: Value = strict_read(&mutant).unwrap();
    assert!(
        record["binding"]["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["expression"] == json!({"ref":"it"})
                && r["target"]
                    .as_str()
                    .unwrap()
                    .ends_with("/options/0/effects/0/steps/0#objects"))
    );
    let result = compile(fixtures::MANIFEST, &mutant);
    assert!(
        matches!(result, CompileResult::Unsupported { ref path, .. } if path == "damage.source"),
        "{result:?}"
    );
}
#[test]
fn explicit_sources_and_complete_self_sacrifice_sources_remain_supported() {
    let catalog = embedded_catalog("sacrifice").unwrap();
    for raw in [
        BOMBARDMENT,
        include_str!("../../../tests/fixtures/mogg-fanatic.json"),
        include_str!("../../../tests/fixtures/fling.json"),
        include_str!("../../../tests/fixtures/siege-gang-commander.json"),
    ] {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("retained explicit/source-sacrifice source")
        };
        assert_eq!(
            catalog.definition_by_oracle_id(&definition.oracle_id),
            Some(&*definition)
        );
    }
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &modal_mutant(true))
    else {
        panic!("explicit named spell source in modal body")
    };
    let first = &definition.spell.as_ref().unwrap().modes[0]
        .program
        .instructions;
    assert!(
        matches!(first.as_slice(), [Instruction::CreateCreatureTokens {count:1,..}, Instruction::DealDamage {target_slot:0,amount,..}] if *amount == Quantity::from(1))
    );
    definition.validate().unwrap();
}
#[test]
fn activation_it_requires_a_first_top_level_source_sacrifice() {
    let raw: Value =
        strict_read(include_str!("../../../tests/fixtures/mogg-fanatic.json")).unwrap();
    let Ability::Activated { costs, effects, .. } =
        serde_json::from_value(raw["faces"][0]["result"]["abilities"][0].clone()).unwrap()
    else {
        panic!("activation")
    };
    assert!(lower_activation(&costs, &effects, 0, CardKind::Creature).is_ok());
    let draw = Step::Draw {
        who: PlayerSelector {
            player: "you".into(),
            selection_id: None,
            of: None,
        },
        amount: DrawAmount::Fixed { value: 1 },
        optional: false,
    };
    let mut later_step = effects.clone();
    later_step[0].steps.insert(0, draw.clone());
    let mut later_sentence = effects.clone();
    later_sentence.insert(
        0,
        Sentence {
            bindings: vec![],
            steps: vec![draw],
        },
    );
    let nested = vec![Sentence {
        bindings: vec![],
        steps: vec![Step::Modal {
            count: ModeCount { exactly: 1 },
            options: vec![
                ModeOption {
                    effects: effects.clone()
                };
                2
            ],
        }],
    }];
    for effects in [later_step, later_sentence, nested] {
        assert!(
            matches!(lower_activation(&costs, &effects, 0, CardKind::Creature), Err(Failure {unsupported:true,ref path,..}) if path == "abilities.effects.source")
        );
    }
    for (costs, kind) in [
        (vec![SourceCost::TapSelf {}], CardKind::Creature),
        (
            vec![SourceCost::Mana {
                symbols: vec!["R".into()],
            }],
            CardKind::Creature,
        ),
        (
            vec![
                SourceCost::TapSelf {},
                SourceCost::Mana {
                    symbols: vec!["R".into()],
                },
            ],
            CardKind::Artifact,
        ),
    ] {
        assert!(
            matches!(lower_activation(&costs, &effects, 0, kind), Err(Failure {unsupported:true,ref path,..}) if path == "abilities.effects.source")
        );
    }
    // A modal spell does not gain the more permissive explicit-this semantics
    // of permanent invocations; the spell must always name its own spell source.
    let mut explicit_this = effects;
    let Step::Damage { source, .. } = &mut explicit_this[0].steps[0] else {
        panic!("damage")
    };
    *source = DamageSource::This {
        noun: "creature".into(),
    };
    let nested = vec![Sentence {
        bindings: vec![],
        steps: vec![Step::Modal {
            count: ModeCount { exactly: 1 },
            options: vec![
                ModeOption {
                    effects: explicit_this
                };
                2
            ],
        }],
    }];
    assert!(
        matches!(validate_damage_sources(&nested, DamageSourceBinding::Spell), Err(Failure {unsupported:true,ref path,..}) if path == "damage.source")
    );
}

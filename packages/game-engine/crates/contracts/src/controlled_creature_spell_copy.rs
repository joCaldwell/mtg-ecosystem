//! CR 601.2c, 608.2b, 707.9, 707.10 and 707.10f (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const SELECTION: &str = "line:0:1";
const PREFIX: &str = "Copy target creature spell you control,";
const SENTENCE: &str =
    "Copy target creature spell you control, except it isn't legendary if the spell is legendary.";
const TEXT: &str = "Copy target creature spell you control, except it isn't legendary if the spell is legendary. (A copy of a creature spell becomes a token.)";
const AST_OP: &str = "copy-stack-object";
const INVALID: &str =
    "creature spell copy requires the complete ordinary instant and its exact nonlegendary copy";

const CAPS: [Capability; 4] = [
    Capability::NormalLayoutV1,
    Capability::CastInstantV1,
    Capability::StackSpellTargetV1,
    Capability::ControlledCreatureSpellCopyV1,
];

fn abilities() -> Value {
    json!([{"kind":"spell","effects":[{"steps":[{
        "effect":AST_OP,
        "what":{"ref":"target","filter":{"cls":"spell","types":["creature"],"control":"you"},
            "count":{"amount":"fixed","value":1},"selectionId":SELECTION},
        "who":{"player":"you"},
        "count":{"amount":"fixed","value":1},
        "copyExceptions":[{"change":"types","types":{"supertypes":{"operation":"remove","types":["legendary"]}}}]
    }]}]}])
}

fn instruction() -> Instruction {
    Instruction::CopyControlledCreatureSpell {
        target_slot: 0,
        source_block: 0,
    }
}

fn target() -> TargetSpec {
    TargetSpec {
        slot: 0,
        selection_id: SELECTION.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::ControlledCreatureSpell,
        filter: None,
    }
}

fn uses_copy(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::CopyControlledCreatureSpell { .. }))
        || p.targets
            .iter()
            .any(|t| t.domain == TargetDomain::ControlledCreatureSpell)
        || p.modes.iter().any(|m| uses_copy(&m.program))
}

fn has_ast_copy(node: &Value) -> bool {
    match node {
        Value::Array(items) => items.iter().any(has_ast_copy),
        Value::Object(fields) => node["effect"] == AST_OP || fields.values().any(has_ast_copy),
        _ => false,
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| uses_copy(&a.program))
        || d.triggered.iter().any(|a| uses_copy(&a.program))
        || d.static_abilities.iter().any(|a| {
            matches!(&a.effect, StaticEffect::GrantSpellCascade { program, .. } if uses_copy(program))
        })
    {
        return Err(INVALID.into());
    }
    let raw_source = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    if !d.spell.as_ref().is_some_and(uses_copy)
        && !d
            .capabilities
            .contains(&Capability::ControlledCreatureSpellCopyV1)
    {
        // A definition with either retained copy marker still has to execute
        // the whole spell, even if its instruction and capability were removed.
        let retained_text = (raw_source.contains(PREFIX) || raw_source.contains(r"\u"))
            && strict_read::<Value>(raw_source)?["oracle_text"]
                .as_str()
                .is_some_and(|text| text.starts_with(PREFIX));
        let retained_ast = (raw_ast.contains(AST_OP) || raw_ast.contains(r"\u"))
            && has_ast_copy(&strict_read(raw_ast)?);
        return if retained_text || retained_ast {
            Err(INVALID.into())
        } else {
            Ok(())
        };
    }

    let p = d.spell.as_ref().ok_or(INVALID)?;
    let c = &d.characteristics;
    let source: Value = strict_read(raw_source)?;
    if c.kind != CardKind::Instant
        || c.basic
        || c.legendary
        || c.snow
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.keywords.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || p.timing != SpellTiming::Instant
        || p.resolution != SpellResolution::Instructions
        || p.functioning_zone != FunctioningZone::Stack
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || p.targets != [target()]
        || p.instructions != [instruction()]
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["layout"] != "normal"
        || source["type_line"] != "Instant"
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|field| source.get(*field).is_some())
        || source["keywords"] != json!([])
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["oracle_text"] != TEXT
        || strict_read::<Value>(raw_ast)? != abilities()
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != TEXT
                || l.blocks != [SENTENCE]
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let has = json!({"type":"object","anyOf":[
        {"required":["instructions"],"properties":{"instructions":{"contains":{
            "required":["op"],"properties":{"op":{"const":"copy_controlled_creature_spell"}}
        }}}},
        {"required":["targets"],"properties":{"targets":{"contains":{
            "required":["domain"],"properties":{"domain":{"const":TargetDomain::ControlledCreatureSpell}}
        }}}}
    ]});
    let copy_program = json!({"type":"object","properties":{
        "timing":{"const":SpellTiming::Instant},
        "resolution":{"const":SpellResolution::Instructions},
        "functioning_zone":{"const":FunctioningZone::Stack},
        "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
        "instructions":{"const":[instruction()]},
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"selection_id":{"const":SELECTION},"chooser":{"const":Chooser::Controller},
            "stage":{"const":EvaluationStage::Announcement},"domain":{"const":TargetDomain::ControlledCreatureSpell},
            "filter":{"type":"null"}
        }}}
    }});
    // Every child is also a SpellProgram and applies the restriction recursively.
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"modes":{"items":{"properties":{"program":{"not":has}}}}},
            "if":has,"then":copy_program
        }));
    let text = json!({"pattern":format!(r#""oracle_text"\s*:\s*"{PREFIX}"#)});
    let ast = json!({"pattern":format!(r#""effect"\s*:\s*"{AST_OP}""#)});
    let forbidden = json!({"items":{"properties":{"program":{"not":has}}}});
    // Static spell grants already require the complete canonical cascade program.
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"activated":forbidden,"triggered":forbidden},
        "if":{"anyOf":[
            {"properties":{"spell":has}},
            {"properties":{"capabilities":{"contains":{"const":Capability::ControlledCreatureSpellCopyV1}}}},
            {"properties":{"provenance":{"properties":{"source_json":text}}}},
            {"properties":{"provenance":{"properties":{"abilities_json":ast}}}}
        ]},
        "then":{"required":["spell"],"properties":{
            "spell":{"allOf":[has,copy_program]},
            "provenance":{"properties":{"source_json":{"pattern":
                r#""oracle_text"\s*:\s*"Copy target creature spell you control, except it isn't legendary if the spell is legendary\. \(A copy of a creature spell becomes a token\.\)""#
            },"abilities_json":ast}},
            "characteristics":{"properties":{
                "kind":{"const":CardKind::Instant},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                "power":{"type":"null"},"toughness":{"type":"null"},
                "colors":{"items":{"not":{"const":Color::C}}},"color_identity":{"items":{"not":{"const":Color::C}}},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"keywords":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},
            "source_labels":{"properties":{
                "text":{"const":TEXT},"blocks":{"const":[SENTENCE]},"activated":{"maxItems":0},
                "triggered":{"maxItems":0},"mana":{"maxItems":0},"statics":{"maxItems":0},"spell_modes":{"maxItems":0}
            }},
            "capabilities":{"minItems":CAPS.len(),"maxItems":CAPS.len(),"uniqueItems":true,"items":{"enum":CAPS}}
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let historical: Value = strict_read(include_str!(
        "../../../tests/fixtures/ramos/ece44a82-dcf0-4439-bdd9-a09c99a6f159.json"
    ))
    .unwrap();
    let source = &historical["source"];
    let mut d = each_player_draw::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.source_labels = Some(SourceLabels {
        text: TEXT.into(),
        blocks: vec![SENTENCE.into()],
        ..SourceLabels::default()
    });
    d.characteristics.colors = vec![Color::G, Color::U];
    d.characteristics.color_identity = d.characteristics.colors.clone();
    d.characteristics.mana_cost = ManaCost {
        generic: Quantity::zero(),
        colored: d.characteristics.colors.clone(),
    };
    let p = d.spell.as_mut().unwrap();
    p.targets = vec![target()];
    p.instructions = vec![instruction()];
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}

#[test]
fn controlled_creature_spell_copy() {
    let d = witness();
    d.validate().unwrap();
    assert_eq!(d.capabilities, BTreeSet::from(CAPS));
    let wire = json!({"op":"copy_controlled_creature_spell","target_slot":0,"source_block":0});
    assert_eq!(serde_json::to_value(instruction()).unwrap(), wire);
    let mut extra = wire;
    extra["retarget"] = json!(true);
    assert!(strict_read::<Instruction>(&extra.to_string()).is_err());

    let mutations: &[fn(&mut Definition)] = &[
        |d| d.characteristics.kind = CardKind::Sorcery,
        |d| d.x_mana_cost = true,
        |d| d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::StackSpell,
        |d| d.spell.as_mut().unwrap().targets[0].selection_id = "line:0:2".into(),
        |d| {
            d.spell.as_mut().unwrap().instructions[0] = Instruction::CopyControlledCreatureSpell {
                target_slot: 1,
                source_block: 0,
            }
        },
        |d| {
            d.spell.as_mut().unwrap().instructions[0] = Instruction::CopyControlledCreatureSpell {
                target_slot: 0,
                source_block: 1,
            }
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            p.targets.clear();
            p.instructions.clear();
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            let child = p.clone();
            p.targets.clear();
            p.instructions.clear();
            p.modes = vec![
                SpellMode {
                    program: child.clone(),
                },
                SpellMode { program: child },
            ];
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            p.additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            });
        },
        |d| {
            d.activated.push(ActivatedAbility {
                functioning_zone: FunctioningZone::Battlefield,
                cost: ActivationCost::TapSource,
                program: d.spell.clone().unwrap(),
            })
        },
        |d| {
            // The opcode cannot survive an otherwise rehashed source with an altered AST.
            d.provenance.abilities_json = "[]".into();
            d.provenance.ast_sha256 = sha256(b"[[]]");
        },
        |d| {
            // The retained AST independently prevents an unrelated fallback.
            let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
            source["oracle_text"] = json!("Draw a card.");
            d.provenance.source_json = source.to_string();
            d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
            d.source_labels = None;
            d.spell = each_player_draw::witness().spell;
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut bad = d.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "mutation {index}");
    }
    for cap in &d.capabilities {
        let mut missing = d.clone();
        missing.capabilities.remove(cap);
        assert!(missing.validate().is_err(), "missing {cap:?}");
    }
    let mut spurious = d;
    spurious.capabilities.insert(Capability::FixedMillProgramV1);
    assert!(spurious.validate().is_err());
}

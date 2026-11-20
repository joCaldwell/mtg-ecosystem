//! CR 207.2d, 400.7, 508.3a, 611.2c: an attack trigger modifies its own
//! battlefield incarnation until cleanup. Flavor labels carry no rules meaning.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const BODY: &str = "Whenever this creature attacks, it gets ";
const END: &str = " until end of turn.";
const INVALID: &str = "requires one complete printed fixed self-attack pump";

#[derive(Debug, PartialEq, Eq)]
pub struct SourceAttackPump {
    pub keyword_blocks: Vec<(usize, Vec<KeywordAbility>)>,
    pub block: usize,
    pub ability: TriggeredAbility,
}

fn printed_trigger(line: &str, block: u32) -> Option<(TriggeredAbility, Value)> {
    let (label, body) = match line.split_once(" — ") {
        Some((label, body)) if !label.is_empty() && label.trim() == label => (Some(label), body),
        Some(_) => return None,
        None => (None, line),
    };
    let stats = body
        .strip_prefix(BODY)?
        .strip_suffix(END)?
        .replace('−', "-");
    let (power, toughness) = stats.split_once('/')?;
    let (power, ast_power) = fixed_creature_activation::signed_stat(power)?;
    let (toughness, ast_toughness) = fixed_creature_activation::signed_stat(toughness)?;
    let mut ast = json!({
        "kind":"triggered","trigger":{"trigger":"attacks","what":{"ref":"this","noun":"creature"}},
        "effects":[{"steps":[{"effect":"pump","what":{"ref":"it"},
            "power":ast_power,"toughness":ast_toughness,"duration":{"duration":"end-of-turn"}}]}]
    });
    if let Some(label) = label {
        ast["abilityWord"] = json!(label);
    }
    Some((
        TriggeredAbility {
            death_filter: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::CreatureAttacked {
                attacker: AttackSubject::Source {},
            },
            intervening_if: None,
            program: SpellProgram {
                additional_sacrifice: None,
                optional_additional_costs: vec![],
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::ModifySourcePowerToughness {
                    power,
                    toughness,
                    duration: Duration::UntilEndOfTurn,
                    source_block: block,
                }],
                modes: vec![],
            },
        },
        ast,
    ))
}

const INTRINSIC: [KeywordAbility; 15] = {
    use KeywordAbility::*;
    [
        Flying,
        Reach,
        Trample,
        Vigilance,
        Defender,
        FirstStrike,
        DoubleStrike,
        Flash,
        Haste,
        Menace,
        Lifelink,
        Deathtouch,
        Hexproof,
        Shroud,
        Indestructible,
    ]
};

/// Bind every printed source line to its complete archived AST. The only
/// permitted siblings are plain, independently supported intrinsic keywords.
pub fn source(text: &str, raw_ast: &str) -> Result<Option<SourceAttackPump>, String> {
    if !text.contains(BODY) {
        return Ok(None);
    }
    let pump = text.lines().enumerate().find_map(|(index, line)| {
        let source_block = u32::try_from(index).ok()?;
        printed_trigger(line, source_block).map(|spec| (index, spec))
    });
    let Some((block, (ability, expected))) = pump else {
        return Ok(None);
    };
    let ast: Vec<Value> = strict_read(raw_ast)?;
    let err = || INVALID.to_string();
    if ast.len() != text.lines().count() || ast[block] != expected {
        return Err(err());
    }
    let mut keyword_blocks = Vec::new();
    let mut seen = Vec::new();
    for (index, (printed, ast)) in text.lines().zip(ast).enumerate() {
        if index == block {
            continue;
        }
        let printed = basic_landwalk::without_reminder(printed).ok_or_else(err)?;
        let mut keywords = Vec::new();
        for word in printed.split([',', ';']) {
            let word = word.trim().to_ascii_lowercase();
            let keyword: KeywordAbility = serde_json::from_value(json!(word)).map_err(|_| err())?;
            if !INTRINSIC.contains(&keyword) || seen.contains(&keyword) {
                return Err(err());
            }
            seen.push(keyword);
            keywords.push(keyword);
        }
        let words: Vec<_> = keywords
            .iter()
            .map(|keyword| json!({"keyword":keyword}))
            .collect();
        if ast != json!({"kind":"keywords","keywords":words}) {
            return Err(err());
        }
        keyword_blocks.push((index, keywords));
    }
    Ok(Some(SourceAttackPump {
        keyword_blocks,
        block,
        ability,
    }))
}

fn has_source_pump(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::ModifySourcePowerToughness { .. }))
        || p.modes.iter().any(|mode| has_source_pump(&mode.program))
}

pub(super) fn is_attack_pump(a: &TriggeredAbility) -> bool {
    a.condition
        == (TriggerCondition::CreatureAttacked {
            attacker: AttackSubject::Source {},
        })
        && a.functioning_zone == FunctioningZone::Battlefield
        && a.death_filter.is_none()
        && a.intervening_if.is_none()
        && a.program.additional_sacrifice.is_none()
        && a.program.optional_additional_costs.is_empty()
        && a.program.timing == SpellTiming::Instant
        && a.program.resolution == SpellResolution::Instructions
        && a.program.functioning_zone == FunctioningZone::Stack
        && a.program.targets.is_empty()
        && a.program.modes.is_empty()
        && matches!(
            a.program.instructions.as_slice(),
            [Instruction::ModifySourcePowerToughness {
                duration: Duration::UntilEndOfTurn,
                ..
            }]
        )
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let has_program = d.triggered.iter().any(|a| has_source_pump(&a.program));
    let raw = &d.provenance.source_json;
    if !has_program && !raw.contains(BODY) && !raw.contains(r"\u") {
        return Ok(());
    }
    let card: Value = strict_read(raw)?;
    let text = card["oracle_text"].as_str().unwrap_or_default();
    let ast = &d.provenance.abilities_json;
    let Some(expected) = source(text, ast)? else {
        return if has_program {
            Err(INVALID.into())
        } else {
            Ok(())
        };
    };
    if d.characteristics.kind != CardKind::Creature
        || card["layout"] != "normal"
        || card["oracle_id"] != d.oracle_id
        || card["name"] != d.name
        || card["id"] != d.provenance.printing_id
        || d.triggered.as_slice() != std::slice::from_ref(&expected.ability)
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.iter().copied().eq(expected
            .keyword_blocks
            .iter()
            .flat_map(|(_, k)| k.iter().copied()))
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.triggered != [expected.block as u32]
                || source(&labels.blocks.join("\n"), ast)
                    .ok()
                    .flatten()
                    .as_ref()
                    != Some(&expected)
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let opcode =
        json!({"required":["op"],"properties":{"op":{"const":"modify_source_power_toughness"}}});
    let present = json!({"$ref":"#/$defs/SourceAttackPumpFreeProgram"});
    schema["$defs"]["SourceAttackPumpFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":opcode}},
        "modes":{"items":{"properties":{"program":present}}}
    }});
    let shape = json!({"properties":{
        "functioning_zone":{"const":"battlefield"},
        "condition":{"const":TriggerCondition::CreatureAttacked { attacker: AttackSubject::Source {} }},
        "death_filter":{"type":"null"},"intervening_if":{"type":"null"},
        "program":{"properties":{
            "timing":{"const":"instant"},"resolution":{"const":"instructions"},
            "functioning_zone":{"const":"stack"},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0},"targets":{"maxItems":0},"modes":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[
                opcode,{"properties":{"duration":{"const":Duration::UntilEndOfTurn}}}
            ]}}
        }}
    }});
    let trigger_free =
        json!({"properties":{"triggered":{"items":{"properties":{"program":present}}}}});
    // This mirrors the portable provenance check for fixed mana activations.
    // Rust additionally compares the printed magnitudes and the complete AST.
    let printed_pump = r#""oracle_text"\s*:\s*"(?:[^"]*\\n)?(?:[^"\\]+ — )?Whenever this creature attacks, it gets [+−-][0-9]+/[+−-][0-9]+ until end of turn\.(?:\\n|")"#;
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"properties":{"provenance":{"properties":{"source_json":{"pattern":printed_pump}}}}},
        "then":{"not":trigger_free}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"not":trigger_free},
        "then":{"properties":{
            "characteristics":{"properties":{"kind":{"const":"creature"}}},
            "activated":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "triggered":{"minItems":1,"maxItems":1,"items":shape},
            "keywords":{"uniqueItems":true,"items":{"enum":INTRINSIC}},
            "capabilities":{"allOf":[
                {"contains":{"const":Capability::TemporaryPowerToughnessV1}},
                {"contains":{"const":Capability::AttackTriggerV1}},
                {"contains":{"const":Capability::TriggeredAbilityV1}},
                {"not":{"contains":{"const":Capability::SacrificeV1}}}
            ]}
        }}
    }));
}

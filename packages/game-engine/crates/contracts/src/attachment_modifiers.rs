//! Signed stat and fixed-color protection grants from continuous attachments.
//! CR 301.5f, 303.4m, 611.3a, 613.1f, 613.4c, 702.11b and 702.16.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn advanced(effect: &StaticEffect) -> bool {
    matches!(effect, StaticEffect::ModifyCreatures { scope: CreatureScope::Equipped | CreatureScope::Enchanted, power, toughness, keywords, .. }
        if power.is_negative() || toughness.is_negative() || keywords.iter().any(|k| k.fixed_protection_color().is_some()))
}

pub fn grants_protection(effect: &StaticEffect) -> bool {
    matches!(effect, StaticEffect::ModifyCreatures { scope: CreatureScope::Equipped | CreatureScope::Enchanted, keywords, .. }
        if keywords.iter().any(|k| k.fixed_protection_color().is_some()))
}

fn signed(text: &str) -> Option<(Quantity, Value)> {
    let (sign, n) = if let Some(n) = text.strip_prefix('+') {
        (1, n)
    } else {
        (-1, text.strip_prefix('-')?)
    };
    let n = n
        .parse::<u32>()
        .ok()
        .filter(|n| n.to_string() == text[1..])?;
    Some((
        (i64::from(n) * sign).into(),
        json!({"sign":sign,"amount":{"amount":"fixed","value":n}}),
    ))
}

const LEGENDARY_GRANT: &str = "As long as equipped creature is legendary, it has hexproof.";

fn legendary_grant() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::EquippedLegendary,
            power: Quantity::zero(),
            toughness: Quantity::zero(),
            keywords: vec![KeywordAbility::Hexproof],
            restrictions: vec![],
        },
    }
}

pub(super) fn scope_valid(effect: &StaticEffect, equipment: bool) -> bool {
    !matches!(
        effect,
        StaticEffect::ModifyCreatures {
            scope: CreatureScope::EquippedLegendary,
            ..
        }
    ) || (equipment && *effect == legendary_grant().effect)
}

pub fn conditional_hexproof(effect: &StaticEffect) -> bool {
    *effect == legendary_grant().effect
}

fn printed_conditional(line: &str) -> Option<(StaticAbility, Value)> {
    if line != LEGENDARY_GRANT
        && line.strip_suffix(
            " (It can't be the target of spells or abilities your opponents control.)",
        ) != Some(LEGENDARY_GRANT)
    {
        return None;
    }
    Some((
        legendary_grant(),
        json!({"kind":"static","effect":{"static":"conditional",
            "condition":{"condition":"is-filter","what":{"ref":"equipped","noun":"creature"},
                "filter":{"supertypes":["legendary"]}},
            "effect":{"static":"modify","what":{"ref":"it"},"modifiers":[
                {"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"hexproof"}}]}
            ]}
        }}),
    ))
}

pub fn printed(line: &str) -> Option<(StaticAbility, Value)> {
    if let Some(grant) = printed_conditional(line) {
        return Some(grant);
    }
    let (scope, reference, tail) = if let Some(tail) = line.strip_prefix("Equipped creature ") {
        (CreatureScope::Equipped, "equipped", tail)
    } else {
        (
            CreatureScope::Enchanted,
            "enchanted",
            line.strip_prefix("Enchanted creature ")?,
        )
    };
    let tail = tail.strip_suffix('.')?;
    let (stats, protection) = if let Some(rest) = tail.strip_prefix("gets ") {
        let (stats, grant) = rest
            .split_once(" and has ")
            .map_or((rest, None), |(s, g)| (s, Some(g)));
        (Some(stats), grant)
    } else {
        (None, Some(tail.strip_prefix("has ")?))
    };
    let mut modifiers = Vec::new();
    let (power, toughness) = if let Some(pt) = stats {
        let (p, t) = pt.split_once('/')?;
        let (power, pa) = signed(p)?;
        let (toughness, ta) = signed(t)?;
        modifiers.push(json!({"modifier":"pt","power":pa,"toughness":ta}));
        (power, toughness)
    } else {
        (Quantity::zero(), Quantity::zero())
    };
    let mut keywords = Vec::new();
    if let Some(grant) = protection {
        let mut from = Vec::new();
        for color in grant.strip_prefix("protection from ")?.split(" and from ") {
            let keyword = fixed_color_protection::keyword_for_color(color)?;
            if keywords.contains(&keyword) {
                return None;
            }
            from.push(json!({"scope":"color","color":color}));
            keywords.push(keyword);
        }
        modifiers.push(json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"protection","from":from}}]}));
    }
    let effect = StaticEffect::ModifyCreatures {
        scope,
        power,
        toughness,
        keywords,
        restrictions: vec![],
    };
    if !advanced(&effect) {
        return None;
    }
    Some((
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        },
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":reference,"noun":"creature"},"modifiers":modifiers}}),
    ))
}

fn equipment_stat_grant(text: &str) -> Option<(StaticAbility, Value)> {
    let stats = text
        .strip_prefix("Equipped creature gets ")?
        .strip_suffix('.')?;
    let (p, t) = stats.split_once('/')?;
    let (power, p) = signed(p)?;
    let (toughness, t) = signed(t)?;
    Some((
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::Equipped,
                power,
                toughness,
                keywords: vec![],
                restrictions: vec![],
            },
        },
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":"equipped","noun":"creature"},
        "modifiers":[{"modifier":"pt","power":p,"toughness":t}]}}),
    ))
}

pub fn equip_ability(mana: ManaCost, source_block: u32) -> ActivatedAbility {
    ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Mana { cost: mana },
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::MainPhase,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: "equip-creature".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                filter: None,
                domain: TargetDomain::ControlledBattlefieldCreature,
            }],
            instructions: vec![Instruction::AttachEquipment {
                target_slot: 0,
                source_block,
            }],
            modes: vec![],
        },
    }
}

fn validate_conditional_equipment(d: &Definition, text: &str, ast: &[Value]) -> Option<()> {
    let [stat_line, gate_line, equip_line] = text.lines().collect::<Vec<_>>().try_into().ok()?;
    let (stats, stats_ast) = equipment_stat_grant(stat_line)?;
    let (gate, gate_ast) = printed_conditional(gate_line)?;
    let mana = equip_line.strip_prefix("Equip {")?.strip_suffix('}')?;
    let generic: Quantity = mana.parse().ok()?;
    if generic.is_negative()
        || generic.to_string() != mana
        || d.static_abilities != [stats, gate]
        || ast
            != [
                stats_ast,
                gate_ast,
                json!({"kind":"keywords","keywords":[
                    {"keyword":"equip","cost":[{"cost":"mana","symbols":[mana]}]}
                ]}),
            ]
        || !d.mana_abilities.is_empty()
        || !d.triggered.is_empty()
        || !d.keywords.is_empty()
    {
        return None;
    }
    let [activation] = d.activated.as_slice() else {
        return None;
    };
    if *activation
        != equip_ability(
            ManaCost {
                generic,
                colored: vec![],
            },
            2,
        )
        || d.source_labels.as_ref().is_some_and(|l| {
            l.blocks != [stat_line, LEGENDARY_GRANT, equip_line]
                || l.statics != [0, 1]
                || l.activated != [2]
                || !l.mana.is_empty()
                || !l.triggered.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return None;
    }
    Some(())
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let actual = d
        .static_abilities
        .iter()
        .filter(|a| advanced(&a.effect) || conditional_hexproof(&a.effect))
        .count();
    if actual == 0
        && !d.provenance.source_json.contains(" creature gets ")
        && !d.provenance.source_json.contains(LEGENDARY_GRANT)
        && !d.provenance.source_json.contains(r"\u")
        && !d
            .provenance
            .source_json
            .contains(" creature has protection from ")
    {
        return Ok(());
    }
    let fail = "requires exact printed attached creature modifier";
    let src: Value = strict_read(&d.provenance.source_json)?;
    let text = src["oracle_text"].as_str().ok_or(fail)?;
    let expected: Vec<_> = text
        .lines()
        .enumerate()
        .filter_map(|(i, line)| printed(line).map(|spec| (i, spec)))
        .collect();
    if actual == 0 && expected.is_empty() {
        return Ok(());
    }
    if actual != expected.len()
        || src["name"] != d.name
        || src["oracle_id"] != d.oracle_id
        || src["id"] != d.provenance.printing_id
        || src["layout"] != "normal"
    {
        return Err(fail.into());
    }
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    if ast.len() != text.lines().count()
        || (text.contains(LEGENDARY_GRANT)
            && validate_conditional_equipment(d, text, &ast).is_none())
    {
        return Err(fail.into());
    }
    for (block, (ability, raw)) in expected {
        let i = ast[..block]
            .iter()
            .filter(|b| b["kind"] == "static")
            .count();
        if ast[block] != raw
            || d.static_abilities.get(i) != Some(&ability)
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.statics.get(i).copied() != u32::try_from(block).ok())
        {
            return Err(fail.into());
        }
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let scope = json!({"required":["effect"],"properties":{"effect":{
        "type":"object","required":["modify_creatures"],"properties":{"modify_creatures":{
            "required":["scope"],"properties":{"scope":{"const":"equipped_legendary"}}
        }}
    }}});
    let has_scope = json!({"required":["static_abilities"],"properties":{
        "static_abilities":{"contains":scope}
    }});
    let has_print = json!({"properties":{"provenance":{"properties":{"source_json":{
        "pattern":r"As long as equipped creature is legendary, it has hexproof\."
    }}}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[has_print.clone(),has_scope]},
        "then":{"allOf":[has_print,{"required":["static_abilities"],"properties":{
            "characteristics":{"properties":{"kind":{"const":"artifact"},"subtypes":{"const":["Equipment"]}}},
            "capabilities":{"contains":{"const":Capability::AttachmentsV1}},
            "static_abilities":{"contains":{"const":legendary_grant()},"maxContains":1,"items":{"if":scope,
                "then":{"const":legendary_grant()}}}
        }}]}
    }));
}

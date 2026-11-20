//! Closed fixed-mana nonmana creature activations. CR 109.5, 115.1c, 602.2b and 608.2b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub struct FixedCreatureActivation {
    pub ability: ActivatedAbility,
    pub ast: Value,
    pub label: String,
}

fn positive(n: &str) -> Option<u32> {
    let value = n.parse::<u32>().ok()?;
    (value > 0 && value.to_string() == n).then_some(value)
}

pub fn fixed_creature_activation(
    line: &str,
    name: &str,
    block: u32,
) -> Option<FixedCreatureActivation> {
    let (printed_cost, effect) = line.split_once(": ")?;
    let mut rest = printed_cost;
    let mut cost = ManaCost {
        generic: 0.into(),
        colored: vec![],
    };
    let mut symbols = Vec::new();
    while !rest.is_empty() {
        let (symbol, tail) = rest.strip_prefix('{')?.split_once('}')?;
        match symbol {
            "W" => cost.colored.push(Color::W),
            "U" => cost.colored.push(Color::U),
            "B" => cost.colored.push(Color::B),
            "R" => cost.colored.push(Color::R),
            "G" => cost.colored.push(Color::G),
            "C" => cost.colored.push(Color::C),
            _ if symbols.is_empty() && symbol.bytes().all(|b| b.is_ascii_digit()) => {
                let generic: Quantity = symbol.parse().ok()?;
                if generic <= Quantity::zero() || generic.to_string() != symbol {
                    return None;
                }
                cost.generic = generic;
            }
            _ => return None,
        }
        symbols.push(symbol);
        rest = tail;
    }
    if symbols.is_empty() {
        return None;
    }
    let target = |offset: usize, filter: Value, domain, permanent_filter| {
        let selection_id = format!("line:{block}:{}", symbols.len() + offset);
        (
            TargetSpec {
                slot: 0,
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain,
                filter: permanent_filter,
                selection_id: selection_id.clone(),
            },
            json!({"ref":"target","filter":filter,"count":{"amount":"fixed","value":1},"selectionId":selection_id}),
        )
    };
    let fixed = |value| json!({"amount":"fixed","value":value});
    let mut targets = Vec::new();
    let mut label = line.to_owned();
    let (instruction, step) = if let Some(n) = effect
        .strip_prefix("You gain ")
        .and_then(|v| v.strip_suffix(" life."))
    {
        let n = positive(n)?;
        (
            Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: i64::from(n).into(),
                source_block: block,
            },
            json!({"effect":"gain-life","who":{"player":"you"},"amount":fixed(n)}),
        )
    } else if effect == "Destroy target artifact or enchantment." {
        let (spec, what) = target(
            2,
            json!({"allOf":[{"anyOf":[{"types":["artifact"]},{"types":["enchantment"]}]}]}),
            TargetDomain::BattlefieldPermanent,
            Some(PermanentFilter {
                types: vec![CardKind::Artifact, CardKind::Enchantment],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
            }),
        );
        targets.push(spec);
        (
            Instruction::DestroyPermanent {
                target_slot: 0,
                optional: false,
                regeneration_forbidden: false,
                source_block: block,
            },
            json!({"effect":"destroy","what":what}),
        )
    } else {
        let (subject, amount) = effect
            .strip_suffix(" damage to target creature.")?
            .rsplit_once(" deals ")?;
        if subject != name
            && name
                .split_once(',')
                .is_none_or(|(short, _)| short != subject)
        {
            return None;
        }
        let n = positive(amount)?;
        let (spec, what) = target(
            6,
            json!({"types":["creature"]}),
            TargetDomain::BattlefieldCreature,
            None,
        );
        targets.push(spec);
        label = format!("{printed_cost}: ~ deals {n} damage to target creature.");
        (
            Instruction::DealDamage {
                target_slot: 0,
                amount: i64::from(n).into(),
                source_block: block,
            },
            json!({"effect":"damage","source":{"ref":"self"},"amount":fixed(n),"to":{"to":"object","ref":what}}),
        )
    };
    Some(FixedCreatureActivation {
        label,
        ast: json!({"kind":"activated","costs":[{"cost":"mana","symbols":symbols}],"effects":[{"steps":[step]}]}),
        ability: ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost: ActivationCost::Mana { cost },
            program: SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                modes: vec![],
                targets,
                instructions: vec![instruction],
            },
        },
    })
}

/// Intrinsic static keywords whose parser role and rules reference we can prove.
pub fn fixed_activation_keyword(keyword: KeywordAbility) -> Option<(&'static str, &'static str)> {
    Some(match keyword {
        KeywordAbility::Flying => ("flying", "702.9"),
        KeywordAbility::Reach => ("reach", "702.17"),
        KeywordAbility::Trample => ("trample", "702.19"),
        KeywordAbility::Vigilance => ("vigilance", "702.20"),
        _ => return None,
    })
}

pub fn has_fixed_creature_activation(d: &Definition) -> bool {
    d.activated.iter().any(|a| {
        matches!(a.cost, ActivationCost::Mana { .. })
            && (a.program.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::GainLife { .. } | Instruction::DestroyPermanent { .. }
                )
            }) || a
                .program
                .targets
                .iter()
                .any(|t| t.domain == TargetDomain::BattlefieldCreature)
                && a.program
                    .instructions
                    .iter()
                    .any(|i| matches!(i, Instruction::DealDamage { .. })))
    })
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    let raw = &d.provenance.source_json;
    let candidate = has_fixed_creature_activation(d);
    if !candidate
        && ![
            " damage to target creature.",
            ": Destroy target artifact or enchantment.",
            ": You gain ",
            r"\u",
        ]
        .iter()
        .any(|word| raw.contains(word))
    {
        return Ok(false);
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let name = source["name"].as_str().unwrap_or("");
    let mut fixed = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let block = u32::try_from(n).map_err(|_| "activation block limit")?;
        if let Some(spec) = fixed_creature_activation(line, name, block) {
            fixed.push((block, spec))
        }
    }
    if fixed.is_empty() && !candidate {
        return Ok(false);
    }
    let error = "fixed creature activation requires every printed cost, target and effect";
    let trigger_count = usize::from(
        d.triggered.len() == 1
            && d.triggered[0].intervening_if == Some(InterveningCondition::SourceCastByController),
    );
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    let keyword_blocks = usize::from(!d.keywords.is_empty());
    let offset = keyword_blocks + trigger_count;
    if keyword_blocks == 1 {
        let mut words = Vec::new();
        for keyword in &d.keywords {
            let (word, _) = fixed_activation_keyword(*keyword).ok_or(error)?;
            if words.contains(&word) {
                return Err(error.into());
            }
            words.push(word);
        }
        let mut printed = words.join(", ");
        printed[..1].make_ascii_uppercase();
        if text.lines().next() != Some(printed.as_str())
            || ast.first()
                != Some(
                    &json!({"kind":"keywords","keywords":words.iter().map(|w|json!({"keyword":w})).collect::<Vec<_>>()}),
                )
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.blocks.first() != Some(&printed))
        {
            return Err(error.into());
        }
    }
    if let Some(raw_keywords) = source.get("keywords") {
        let actual = raw_keywords.as_array().ok_or(error)?;
        if actual.len() != d.keywords.len()
            || d.keywords.iter().any(|k| {
                fixed_activation_keyword(*k).is_none_or(|(word, _)| {
                    !actual
                        .iter()
                        .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(word)))
                })
            })
        {
            return Err(error.into());
        }
    }
    if fixed.is_empty()
        || d.characteristics.kind != CardKind::Creature
        || !d.characteristics.additional_types.is_empty()
        || source["id"] != d.provenance.printing_id
        || name != d.name
        || source["oracle_id"] != d.oracle_id
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || d.intrinsic_zone.is_some()
        || d.triggered.len() != trigger_count
        || ast.len() != text.lines().count()
        || ast.len() != offset + fixed.len()
        || d.activated.len() != fixed.len()
    {
        return Err(error.into());
    }
    for (index, (block, spec)) in fixed.iter().enumerate() {
        if *block as usize != offset + index
            || ast[*block as usize] != spec.ast
            || d.activated[index] != spec.ability
        {
            return Err(error.into());
        }
    }
    if let Some(labels) = &d.source_labels {
        if labels.text != text
            || labels.blocks.len() != ast.len()
            || labels.activated != fixed.iter().map(|(b, _)| *b).collect::<Vec<_>>()
            || labels.triggered != (keyword_blocks as u32..offset as u32).collect::<Vec<_>>()
            || !labels.statics.is_empty()
            || !labels.mana.is_empty()
            || !labels.spell_modes.is_empty()
            || fixed
                .iter()
                .any(|(b, s)| labels.blocks[*b as usize] != s.label)
        {
            return Err(error.into());
        }
    }
    Ok(true)
}

pub(super) fn extend_schema(schema: &mut Value) {
    let damage = json!({"properties":{"op":{"const":"deal_damage"},"target_slot":{"const":0},"amount":{"pattern":"^[1-9][0-9]*$"}}});
    let destroy = json!({"properties":{"op":{"const":"destroy_permanent"},"target_slot":{"const":0},"optional":{"const":false},"regeneration_forbidden":{"const":false}}});
    let gain = json!({"properties":{"op":{"const":"gain_life"},"recipient":{"const":"controller"},"amount":{"pattern":"^[1-9][0-9]*$"}}});
    let target = |domain: &str, filter: Value| {
        json!({"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"chooser":{"const":"controller"},"stage":{"const":"announcement"},"domain":{"const":domain},
            "selection_id":{"pattern":"^line:(0|[1-9][0-9]*):[1-9][0-9]*$"},"filter":filter
        }}})
    };
    let branch = |instruction: Value, targets: Value| json!({"properties":{"instructions":{"items":instruction},"targets":targets}});
    let union = target(
        "battlefield_permanent",
        json!({"type":"object","required":["types"],"properties":{
            "types":{"const":["artifact","enchantment"]},"non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}
        }}),
    );
    let program = json!({"properties":{"functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
        "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"instructions":{"minItems":1,"maxItems":1}},
        "oneOf":[branch(damage,target("battlefield_creature",json!({"type":"null"}))),branch(destroy,json!({"allOf":[union,{"items":{"required":["filter"]}}]})),branch(gain,json!({"maxItems":0}))]});
    let mana = json!({"type":"object","required":["mana"],"properties":{"mana":{"properties":{"cost":{"anyOf":[
        {"properties":{"generic":{"pattern":"^[1-9][0-9]*$"}}},{"properties":{"colored":{"minItems":1}}}
    ]}}}}});
    schema["$defs"]["FixedCreatureActivation"] = json!({"required":["cost","program","functioning_zone"],"properties":{
        "cost":mana,"program":program,"functioning_zone":{"const":"battlefield"}
    }});
    let candidate = json!({"required":["cost","program"],"properties":{"cost":{"required":["mana"],"type":"object"},"program":{"anyOf":[
        {"properties":{"instructions":{"contains":{"properties":{"op":{"enum":["destroy_permanent","gain_life"]}}}}}},
        {"properties":{"instructions":{"contains":{"properties":{"op":{"const":"deal_damage"}}}},"targets":{"contains":{"properties":{"domain":{"const":"battlefield_creature"}}}}}}
    ]}}});
    let effect_capabilities = [
        ("deal_damage",vec![Capability::CreatureTargetV1,Capability::NoncombatDamageV1]),
        ("destroy_permanent",vec![Capability::PermanentRemovalV1]),
        ("gain_life",vec![Capability::LifeGainV1]),
    ].into_iter().map(|(op,caps)|json!({
        "if":{"properties":{"activated":{"contains":{"properties":{"program":{"properties":{
            "instructions":{"contains":{"properties":{"op":{"const":op}}}}
        }}}}}}},
        "then":{"properties":{"capabilities":{"allOf":caps.into_iter().map(|cap|json!({"contains":{"const":cap}})).collect::<Vec<_>>()}}}
    })).collect::<Vec<_>>();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":candidate}}},
        "then":{"allOf":effect_capabilities,"properties":{"activated":{"items":{"$ref":"#/$defs/FixedCreatureActivation"}},
            "keywords":{"uniqueItems":true,"items":{"enum":["flying","reach","trample","vigilance"]}},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{"kind":{"const":"creature"},"additional_types":{"maxItems":0}}},
            "triggered":{"maxItems":1,"items":{"required":["intervening_if"],"properties":{"intervening_if":{"const":InterveningCondition::SourceCastByController}}}},
            "capabilities":{"allOf":[{"contains":{"const":Capability::ManaActivationCostV1}},{"contains":{"const":Capability::ActivatedAbilityV1}}]}
        }}
    }));
}

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

fn signed_stat(text: &str) -> Option<(Quantity, Value)> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    let value: u32 = digits.parse().ok()?;
    (value.to_string() == digits).then(|| {
        (
            (i64::from(value) * sign).into(),
            json!({"sign":sign,"amount":{"amount":"fixed","value":value}}),
        )
    })
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
                if generic.is_negative() || generic.to_string() != symbol {
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
    let (instruction, step) = if let Some((subject, stats)) = effect
        .strip_suffix(" until end of turn.")
        .and_then(|text| text.rsplit_once(" gets "))
    {
        let what = if subject == "This creature" {
            json!({"ref":"this","noun":"creature"})
        } else if subject == name
            || name
                .split_once(',')
                .is_some_and(|(short, _)| subject == short)
        {
            json!({"ref":"self"})
        } else {
            return None;
        };
        let stats = stats.replace('−', "-");
        let (power, toughness) = stats.split_once('/')?;
        let (power, ast_power) = signed_stat(power)?;
        let (toughness, ast_toughness) = signed_stat(toughness)?;
        let subject = if what["ref"] == "self" { "~" } else { subject };
        label = format!("{printed_cost}: {subject} gets {stats} until end of turn.");
        (
            Instruction::ModifySourcePowerToughness {
                power,
                toughness,
                duration: Duration::UntilEndOfTurn,
                source_block: block,
            },
            json!({"effect":"pump","what":what,"power":ast_power,"toughness":ast_toughness,
                "duration":{"duration":"end-of-turn"}}),
        )
    } else if let Some(n) = effect
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
                power: None,
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
    if (symbols[0] == "0" && symbols.len() != 1)
        || (symbols[0] == "0"
            && !matches!(instruction, Instruction::ModifySourcePowerToughness { .. }))
    {
        return None;
    }
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

pub fn bound_fixed_activation_keyword(
    d: &Definition,
    keyword: KeywordAbility,
) -> Option<(&'static str, &'static str)> {
    if let Some(bound) = fixed_activation_keyword(keyword) {
        return Some(bound);
    }
    if d.activated.is_empty() || !d.activated.iter().all(mana_self_pump) {
        return None;
    }
    Some(match keyword {
        KeywordAbility::Defender => ("defender", "702.3"),
        KeywordAbility::FirstStrike => ("first strike", "702.7"),
        KeywordAbility::DoubleStrike => ("double strike", "702.4"),
        KeywordAbility::Flash => ("flash", "702.8"),
        KeywordAbility::Haste => ("haste", "702.10"),
        KeywordAbility::Menace => ("menace", "702.111"),
        KeywordAbility::Lifelink => ("lifelink", "702.15"),
        KeywordAbility::Deathtouch => ("deathtouch", "702.2"),
        KeywordAbility::Hexproof => ("hexproof", "702.11"),
        KeywordAbility::Shroud => ("shroud", "702.18"),
        KeywordAbility::Indestructible => ("indestructible", "702.12"),
        _ => return None,
    })
}

pub(super) fn mana_self_pump(a: &ActivatedAbility) -> bool {
    matches!(a.cost, ActivationCost::Mana { .. })
        && matches!(
            a.program.instructions.as_slice(),
            [Instruction::ModifySourcePowerToughness {
                duration: Duration::UntilEndOfTurn,
                ..
            }]
        )
}

fn pump_keyword_reminder(keyword: KeywordAbility) -> Option<&'static str> {
    Some(match keyword {
        KeywordAbility::Defender => "(This creature can't attack.)",
        KeywordAbility::FirstStrike => {
            "(This creature deals combat damage before creatures without first strike.)"
        }
        KeywordAbility::Flying => {
            "(This creature can't be blocked except by creatures with flying or reach.)"
        }
        KeywordAbility::Reach => "(This creature can block creatures with flying.)",
        KeywordAbility::Menace => {
            "(This creature can't be blocked except by two or more creatures.)"
        }
        KeywordAbility::Lifelink => {
            "(Damage dealt by this creature also causes you to gain that much life.)"
        }
        KeywordAbility::Flash => "(You may cast this spell any time you could cast an instant.)",
        _ => return None,
    })
}

pub fn has_fixed_creature_activation(d: &Definition) -> bool {
    d.activated.iter().any(|a| {
        matches!(a.cost, ActivationCost::Mana { .. })
            && (mana_self_pump(a)
                || a.program.instructions.iter().any(|i| {
                    matches!(
                        i,
                        Instruction::GainLife { .. } | Instruction::DestroyPermanent { .. }
                    )
                })
                || a.program
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
        && !(raw.contains("}: ") && raw.contains(" until end of turn."))
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
            let (word, _) = bound_fixed_activation_keyword(d, *keyword).ok_or(error)?;
            if words.contains(&word) {
                return Err(error.into());
            }
            words.push(word);
        }
        let mut printed = words.join(", ");
        printed[..1].make_ascii_uppercase();
        let first = text.lines().next().unwrap_or("");
        let exact_reminder = match d.keywords.as_slice() {
            [keyword] if d.activated.iter().all(mana_self_pump) => pump_keyword_reminder(*keyword)
                .is_some_and(|reminder| first == format!("{printed} {reminder}")),
            _ => false,
        };
        if (first != printed && !exact_reminder)
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
                bound_fixed_activation_keyword(d, *k).is_none_or(|(word, _)| {
                    !actual
                        .iter()
                        .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(word)))
                })
            })
        {
            return Err(error.into());
        }
    }
    let type_line = source["type_line"].as_str().ok_or(error)?;
    let (head, subtypes) = type_line.split_once(" — ").unwrap_or((type_line, ""));
    let mut types = head.split_whitespace().collect::<BTreeSet<_>>();
    if !types.remove("Creature")
        || types.remove("Legendary") != d.characteristics.legendary
        || types.remove("Snow") != d.characteristics.snow
        || !types.is_empty()
        || !subtypes
            .split_whitespace()
            .eq(d.characteristics.subtypes.iter().map(String::as_str))
    {
        return Err(error.into());
    }
    if fixed.is_empty()
        || source["layout"] != "normal"
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
    let pump_op = "modify_source_power_toughness";
    let pump =
        json!({"properties":{"op":{"const":pump_op},"duration":{"const":"until_end_of_turn"}}});
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
            "types":{"const":["artifact","enchantment"]},"non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false},"power":{"type":"null"}
        }}),
    );
    let program = json!({"properties":{"functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
        "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"instructions":{"minItems":1,"maxItems":1}},
        "oneOf":[branch(damage,target("battlefield_creature",json!({"type":"null"}))),branch(destroy,json!({"allOf":[union,{"items":{"required":["filter"]}}]})),branch(gain,json!({"maxItems":0})),branch(pump.clone(),json!({"maxItems":0}))]});
    let mana = json!({"type":"object","required":["mana"],"properties":{"mana":{"properties":{"cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}},"anyOf":[
        {"properties":{"generic":{"pattern":"^[1-9][0-9]*$"}}},{"properties":{"colored":{"minItems":1}}}
    ]}}}}});
    let has_pump =
        json!({"properties":{"program":{"properties":{"instructions":{"contains":pump}}}}});
    let zero = json!({"type":"object","required":["mana"],"properties":{"mana":{"properties":{"cost":{"properties":{
        "generic":{"const":"0"},"colored":{"maxItems":0}
    }}}}}});
    schema["$defs"]["FixedCreatureActivation"] = json!({"required":["cost","program","functioning_zone"],"properties":{
        "program":program,"functioning_zone":{"const":"battlefield"}
    },"if":has_pump,"then":{"properties":{"cost":{"anyOf":[mana,zero]}}},
        "else":{"properties":{"cost":mana}}
    });
    let candidate = json!({"required":["cost","program"],"properties":{"cost":{"required":["mana"],"type":"object"},"program":{"anyOf":[
        {"properties":{"instructions":{"contains":{"properties":{"op":{"enum":["destroy_permanent","gain_life",pump_op]}}}}}},
        {"properties":{"instructions":{"contains":{"properties":{"op":{"const":"deal_damage"}}}},"targets":{"contains":{"properties":{"domain":{"const":"battlefield_creature"}}}}}}
    ]}}});
    let printed_effects = [
        ("deal_damage", r" deals [1-9][0-9]* damage to target creature\."),
        ("destroy_permanent", r"Destroy target artifact or enchantment\."),
        ("gain_life", r"You gain [1-9][0-9]* life\."),
        (pump_op, r" gets [+−-][0-9]+/[+−-][0-9]+ until end of turn\."),
    ].into_iter().map(|(op, printed)| json!({
        "if":{"properties":{"provenance":{"properties":{"source_json":{
            "pattern":format!(r#""oracle_text"\s*:\s*"(?:[^"]*\\n)?(?:\{{[WUBRGC0-9]+\}})+: [^"\\]*?{printed}(?:\\n|")"#)
        }}}}},
        "then":{"required":["activated"],"properties":{"activated":{"contains":{"properties":{"program":{"properties":{
            "instructions":{"contains":{"properties":{"op":{"const":op}}}}
        }}}}}}}
    })).collect::<Vec<_>>();
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .extend(printed_effects);
    let mana_pump = json!({"allOf":[has_pump,{"properties":{"cost":{
        "type":"object","required":["mana"]
    }}}]});
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
            "keywords":{"uniqueItems":true},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{"kind":{"const":"creature"},"additional_types":{"maxItems":0}}},
            "triggered":{"maxItems":1,"items":{"required":["intervening_if"],"properties":{"intervening_if":{"const":InterveningCondition::SourceCastByController}}}},
            "capabilities":{"allOf":[{"contains":{"const":Capability::ManaActivationCostV1}},{"contains":{"const":Capability::ActivatedAbilityV1}}]}
        }}
    }));
    let only_pump = json!({"properties":{"activated":{"minItems":1,"items":mana_pump}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":candidate}}},
        "then":{
            "if":only_pump,
            "then":{"properties":{
                "keywords":{"items":{"enum":[
                    "flying","reach","trample","vigilance","defender","first strike","double strike",
                    "flash","haste","menace","lifelink","deathtouch","hexproof","shroud","indestructible"
                ]}},
                "capabilities":{"not":{"contains":{"const":Capability::SacrificeV1}}}
            }},
            "else":{"properties":{"keywords":{
                "items":{"enum":["flying","reach","trample","vigilance"]}
            }}}
        }
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":mana_pump}}},
        "then":{"properties":{"capabilities":{"contains":{"const":Capability::TemporaryPowerToughnessV1}}}}
    }));
}

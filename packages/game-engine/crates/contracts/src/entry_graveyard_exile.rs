//! CR 400.7, 603.3d, 603.6a and 608.2b/c. The trigger targets one player.
//! The objects in that player's graveyard are determined on resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const LAND_LINES: &[&str] = &[
    "This land enters tapped.",
    "When this land enters, exile target player's graveyard.",
    "{T}: Add {B}.",
];
const CREATURE_LINES: &[&str] = &[
    "Flying",
    "When this creature enters, exile target player's graveyard.",
];
const INVALID: &str = "requires a complete ordinary self-entry graveyard exile source";
const SELECTION: &str = "line:1:6";

fn abilities(land: bool) -> Value {
    let subject = json!({"ref":"this","noun":if land { "land" } else { "creature" }});
    let trigger = json!({"kind":"triggered","trigger":{"trigger":"enters","what":subject},
        "effects":[{"steps":[{"effect":"exile","what":{"ref":"zone-cards","zone":"graveyard",
        "owner":{"player":"target-player","selectionId":SELECTION}}}]}]});
    if land {
        json!([{"kind":"static","effect":{"static":"enters-tapped","what":subject}},trigger,
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[
            {"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["B"]}}
        ]}]}])
    } else {
        json!([{"kind":"keywords","keywords":[{"keyword":"flying"}]},trigger])
    }
}

fn trigger() -> TriggeredAbility {
    TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            modes: vec![],
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: SELECTION.into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::AnyPlayer,
                filter: None,
            }],
            instructions: vec![Instruction::ExileTargetGraveyard {
                target_slot: 0,
                source_block: 1,
            }],
        },
    }
}

fn land_mana() -> ManaAbility {
    ManaAbility {
        self_damage: None,
        restriction: None,
        source_block: 2,
        costs: PermanentActivationCosts {
            tap: true,
            sacrifice: false,
            mana: None,
            remove_source_plus_one_counters: None,
        },
        production: ManaProduction::Fixed {
            symbols: vec![Color::B],
        },
    }
}

fn enters_tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.spell
        .as_ref()
        .is_some_and(|p| matches!(p.resolution, SpellResolution::ChooseRepeatedModes { .. }))
    {
        return crate::modal_targets::validate_definition(d);
    }
    let reserved = d.capabilities.contains(&Capability::EntryGraveyardExileV1);
    let raw = &d.provenance.source_json;
    let phrase = "exile target player's graveyard";
    if !reserved && !raw.contains(phrase) && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    if !reserved
        && !source["oracle_text"]
            .as_str()
            .is_some_and(|text| text.contains(phrase))
    {
        return Ok(());
    }
    let c = &d.characteristics;
    let land = c.kind == CardKind::Land;
    let lines = if land { LAND_LINES } else { CREATURE_LINES };
    let text = lines.join("\n");
    let mut cost = String::new();
    let type_line = match c.kind {
        CardKind::Land => {
            if !c.subtypes.is_empty()
                || !c.colors.is_empty()
                || c.color_identity != [Color::B]
                || source["produced_mana"] != json!(["B"])
                || !d.keywords.is_empty()
                || d.spell.is_some()
                || d.mana_abilities != [land_mana()]
                || d.static_abilities != [enters_tapped()]
            {
                return Err(INVALID.into());
            }
            "Land".to_owned()
        }
        CardKind::Creature => {
            if d.keywords != [KeywordAbility::Flying]
                || !d.static_abilities.is_empty()
                || !d.mana_abilities.is_empty()
                || d.spell.as_ref().is_none_or(|p| {
                    p.additional_sacrifice.is_some()
                        || !p.optional_additional_costs.is_empty()
                        || !p.instructions.is_empty()
                        || !p.modes.is_empty()
                        || !p.targets.is_empty()
                })
                || c.subtypes.iter().any(|s| {
                    s.is_empty()
                        || s.contains(char::is_whitespace)
                        || unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES.contains(&s.as_str())
                })
            {
                return Err(INVALID.into());
            }
            if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
                cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
            }
            for color in &c.mana_cost.colored {
                cost.push_str(&format!("{{{color:?}}}"));
            }
            if c.subtypes.is_empty() {
                "Creature".into()
            } else {
                format!("Creature — {}", c.subtypes.join(" "))
            }
        }
        _ => return Err(INVALID.into()),
    };
    // Shared validation checks ordinary permanent timing, land mana cost, P/T,
    // intrinsic zone, hashes and exact capability closure.
    if c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || d.x_mana_cost
        || !d.activated.is_empty()
        || d.triggered != [trigger()]
        || source["type_line"] != type_line
        || source["layout"] != "normal"
        || ["card_faces", "loyalty", "defense"]
            .iter()
            .any(|field| source.get(*field).is_some())
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["oracle_text"] != text
        || source["keywords"] != if land { json!([]) } else { json!(["Flying"]) }
        || source["mana_cost"] != cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source.get("power").cloned() != c.power.as_ref().map(|q| json!(q))
        || source.get("toughness").cloned() != c.toughness.as_ref().map(|q| json!(q))
        || strict_read::<Value>(&d.provenance.abilities_json)? != abilities(land)
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != lines
                || l.triggered != [1]
                || !l.activated.is_empty()
                || !l.spell_modes.is_empty()
                || !l.statics.iter().copied().eq(land.then_some(0))
                || !l.mana.iter().copied().eq(land.then_some(2))
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let op = json!({"required":["op"],"properties":{"op":{"const":"exile_target_graveyard"}}});
    let has_op = json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":op}}});
    let singleton = |item: Value| json!({"minItems":1,"maxItems":1,"items":item});
    // Each nested mode is a SpellProgram and checks the same mode edge.
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"modes":{"items":{"properties":{"program":{"not":has_op}}}}},
        "if":has_op,"then":{"properties":{
            "timing":{"const":"instant"},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":singleton(json!({"const":{"op":"exile_target_graveyard","target_slot":0,"source_block":1}})),
            "targets":singleton(json!({"properties":{"slot":{"const":0},"selection_id":{"const":SELECTION},
                "chooser":{"const":"controller"},"stage":{"const":"announcement"},"domain":{"const":"any_player"},"filter":{"type":"null"}}}))
        }}
    }));
    let trigger = json!({"required":["program"],"properties":{"program":has_op}});
    let caps = |extra: &[Capability]| {
        let mut allowed = vec![
            NormalLayoutV1,
            TriggeredAbilityV1,
            SelfEntersBattlefieldV1,
            AnyPlayerTargetV1,
            EntryGraveyardExileV1,
        ];
        allowed.extend_from_slice(extra);
        json!({"items":{"enum":allowed},"minItems":allowed.len(),"maxItems":allowed.len(),"uniqueItems":true})
    };
    let labels = |lines: &[&str], statics: &[u32], mana: &[u32]| {
        json!({"properties":{
            "text":{"const":lines.join("\n")},"blocks":{"const":lines},"triggered":{"const":[1]},
            "activated":{"maxItems":0},"spell_modes":{"maxItems":0},"statics":{"const":statics},"mana":{"const":mana}
        }})
    };
    let land = json!({"required":["static_abilities","mana_abilities","intrinsic_zone"],"properties":{
        "characteristics":{"properties":{"kind":{"const":"land"},"subtypes":{"maxItems":0},"colors":{"maxItems":0},
            "color_identity":{"const":["B"]},"power":{"type":"null"},"toughness":{"type":"null"},"mana_cost":{"const":{"generic":"0","colored":[]}}}},
        "spell":{"type":"null"},"intrinsic_zone":{"const":"battlefield"},"keywords":{"maxItems":0},
        "static_abilities":{"const":[enters_tapped()]},"mana_abilities":singleton(json!({"properties":{
            "source_block":{"const":2},"production":{"const":{"mana":"fixed","symbols":["B"]}},
            "self_damage":{"type":"null"},"restriction":{"type":"null"},"costs":{"properties":{
                "tap":{"const":true},"sacrifice":{"const":false},"mana":{"type":"null"},"remove_source_plus_one_counters":{"type":"null"}}}}})),
        "capabilities":caps(&[LandPlayV1, EntersTappedV1, PermanentManaV1]),
        "source_labels":{"allOf":[labels(LAND_LINES,&[0],&[2]),{"required":["statics"]}]},
        "provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"This land enters tapped\.\\nWhen this land enters, exile target player's graveyard\.\\n\{T\}: Add \{B\}\.""#}}}
    }});
    let excluded = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect::<Vec<_>>();
    let creature = json!({"required":["spell","keywords"],"properties":{
        "characteristics":{"required":["power","toughness"],"properties":{"kind":{"const":"creature"},"subtypes":{"items":{"pattern":"^\\S+$","not":{"enum":excluded}}},
            "power":{"type":"string"},"toughness":{"type":"string","pattern":"^[1-9][0-9]*$"}}},
        "spell":{"type":"object","properties":{"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},"resolution":{"const":"permanent"},
            "instructions":{"maxItems":0},"targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},
        "intrinsic_zone":{"type":"null"},"keywords":{"const":["flying"]},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},
        "capabilities":caps(&[BaseCreatureV1,CastCreatureV1,FlyingV1]),"source_labels":labels(CREATURE_LINES,&[],&[]),
        "provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"Flying\\nWhen this creature enters, exile target player's graveyard\.""#}}}
    }});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"spell":{"not":has_op},"activated":{"items":{"not":trigger}}},
        "if":{"not":{"required":["spell"],"properties":{"spell":{"$ref":"#/$defs/RepeatedTargetRoot"}}},"anyOf":[{"properties":{"capabilities":{"contains":{"const":EntryGraveyardExileV1}}}},
            {"required":["triggered"],"properties":{"triggered":{"contains":trigger}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"[^"]*exile target player's graveyard"#}}}}}
        ]},
        "then":{"required":["triggered"],"oneOf":[land,creature],"properties":{
            "characteristics":{"properties":{"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0}}},
            "x_mana_cost":{"const":false},"activated":{"maxItems":0},
            "triggered":singleton(json!({"allOf":[trigger],"properties":{"condition":{"const":"self_enters_battlefield"},"functioning_zone":{"const":"battlefield"},"death_filter":{"type":"null"},"intervening_if":{"type":"null"}}}))
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness(kind: CardKind) -> Definition {
    let land = kind == CardKind::Land;
    let mut d = if land {
        source_tap_damage::witness()
    } else {
        power_entry_draw::witness()
    };
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    d.name = "Synthetic entry exile".into();
    source["name"] = json!(d.name);
    if land {
        d.characteristics.color_identity = vec![Color::B];
        d.static_abilities = vec![enters_tapped()];
        d.mana_abilities = vec![land_mana()];
        source["color_identity"] = json!(["B"]);
        source["produced_mana"] = json!(["B"]);
    } else {
        d.keywords = vec![KeywordAbility::Flying];
        source["keywords"] = json!(["Flying"]);
    }
    d.triggered = vec![trigger()];
    let lines = if land { LAND_LINES } else { CREATURE_LINES };
    source["oracle_text"] = json!(lines.join("\n"));
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities(land).to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}

#[test]
fn entry_graveyard_exile() {
    use Capability::*;
    for kind in [CardKind::Land, CardKind::Creature] {
        let base = witness(kind);
        base.validate().unwrap();
        let mut expected = BTreeSet::from([
            NormalLayoutV1,
            TriggeredAbilityV1,
            SelfEntersBattlefieldV1,
            AnyPlayerTargetV1,
            EntryGraveyardExileV1,
        ]);
        expected.extend(if kind == CardKind::Land {
            [LandPlayV1, PermanentManaV1, EntersTappedV1]
        } else {
            [BaseCreatureV1, CastCreatureV1, FlyingV1]
        });
        assert_eq!(base.capabilities, expected);
        let mut missing = base.clone();
        missing.capabilities.remove(&EntryGraveyardExileV1);
        assert!(missing.validate().is_err());
        for mutation in 0..15 {
            let mut d = base.clone();
            let p = &mut d.triggered[0].program;
            match mutation {
                0 => p.targets[0].domain = TargetDomain::OpponentPlayer,
                1 => p.targets[0].selection_id = "line:2:6".into(),
                2 => {
                    p.instructions = vec![Instruction::ExileTargetGraveyard {
                        target_slot: 0,
                        source_block: 0,
                    }]
                }
                3 => p.instructions.push(p.instructions[0].clone()),
                4 => p.modes.push(SpellMode { program: p.clone() }),
                5 => d.triggered[0].condition = TriggerCondition::SelfDies,
                6 => d.triggered.clear(),
                7 => d.triggered.push(d.triggered[0].clone()),
                8 => d.characteristics.legendary = true,
                9 => d.characteristics.additional_types.push(CardKind::Artifact),
                10 => d.x_mana_cost = true,
                11 => d.spell = Some(p.clone()),
                12 => d.activated.push(ActivatedAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    cost: ActivationCost::TapSource,
                    program: p.clone(),
                }),
                13 if kind == CardKind::Land => {
                    d.mana_abilities[0].production = ManaProduction::AnyColor
                }
                13 => d.keywords.clear(),
                14 if kind == CardKind::Land => d.static_abilities.clear(),
                14 => d
                    .spell
                    .as_mut()
                    .unwrap()
                    .modes
                    .push(SpellMode { program: p.clone() }),
                _ => unreachable!(),
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "{kind:?}, mutation {mutation}");
        }
    }
}

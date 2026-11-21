//! CR 109.5, 400.3, 601.2c, 603.3d, 603.5 and 608.2b–d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) const SPELL_TEXT: &str = "Return target card from your graveyard to your hand.";
const ENTRY_TEXT: &str =
    "When this creature enters, you may return target card from your graveyard to your hand.";
const INVALID: &str = "requires the complete own-graveyard card return spell or self-entry trigger";

fn selection(entry: bool) -> &'static str {
    if entry { "line:0:8" } else { "line:0:1" }
}

pub(super) fn abilities(entry: bool) -> Value {
    let target = json!({"ref":"target","filter":{"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
        "count":{"amount":"fixed","value":1},"selectionId":selection(entry)});
    let mut step = json!({"effect":"return-to-hand","what":target,"to":"your"});
    if entry {
        step["optional"] = json!(true);
        json!([{"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"this","noun":"creature"}},
            "effects":[{"steps":[step]}]}])
    } else {
        json!([{"kind":"spell","effects":[{"steps":[step]}]}])
    }
}

fn ordinary_program(p: &SpellProgram, timing: SpellTiming, resolution: SpellResolution) -> bool {
    p.timing == timing
        && p.resolution == resolution
        && p.functioning_zone == FunctioningZone::Stack
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
}

fn program_matches(d: &Definition, entry: bool) -> bool {
    let Some(cast) = &d.spell else { return false };
    let (p, timing) = if entry {
        let [trigger] = d.triggered.as_slice() else {
            return false;
        };
        if trigger.condition != TriggerCondition::SelfEntersBattlefield
            || trigger.functioning_zone != FunctioningZone::Battlefield
            || trigger.intervening_if.is_some()
            || trigger.death_filter.is_some()
            || !ordinary_program(cast, SpellTiming::MainPhase, SpellResolution::Permanent)
            || !cast.targets.is_empty()
            || !cast.instructions.is_empty()
        {
            return false;
        }
        (&trigger.program, SpellTiming::Instant)
    } else {
        if !d.triggered.is_empty() {
            return false;
        }
        let timing = match d.characteristics.kind {
            CardKind::Instant => SpellTiming::Instant,
            CardKind::Sorcery => SpellTiming::MainPhase,
            _ => return false,
        };
        (cast, timing)
    };
    ordinary_program(p, timing, SpellResolution::Instructions)
        && matches!(p.instructions.as_slice(), [Instruction::ReturnOwnGraveyardCardToHand {
            target_slot:0, source_block:0, optional
        }] if *optional == entry)
        && matches!(p.targets.as_slice(), [t] if t.slot == 0 && t.selection_id == selection(entry)
            && t.chooser == Chooser::Controller && t.stage == EvaluationStage::Announcement
            && t.domain == TargetDomain::OwnGraveyardCard && t.filter.is_none())
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    // The shared requirements check derives the capability from either the target
    // or the instruction, including modes. Also catch an erased program on these sources.
    let reserved = d
        .capabilities
        .contains(&Capability::OwnGraveyardCardReturnV1);
    let phrase = "target card from your graveyard to your hand";
    let raw = &d.provenance.source_json;
    if !reserved && !raw.contains(phrase) && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    if !reserved
        && !source["oracle_text"]
            .as_str()
            .is_some_and(|s| s.contains(phrase))
    {
        return Ok(());
    }
    let c = &d.characteristics;
    let entry = c.kind == CardKind::Creature;
    let text = if entry { ENTRY_TEXT } else { SPELL_TEXT };
    let mut type_line = match c.kind {
        CardKind::Creature => "Creature",
        CardKind::Instant => "Instant",
        CardKind::Sorcery => "Sorcery",
        _ => return Err(INVALID.into()),
    }
    .to_owned();
    if !c.subtypes.is_empty() {
        if !entry
            || c.subtypes.iter().any(|s| {
                s.is_empty()
                    || s.contains(char::is_whitespace)
                    || unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES.contains(&s.as_str())
            })
        {
            return Err(INVALID.into());
        }
        type_line.push_str(" — ");
        type_line.push_str(&c.subtypes.join(" "));
    }
    // Mana symbols and P/T remain the normal finite characteristics. Reconcile
    // the retained source so stripping an X cost, supertype or ability cannot authorize a cast.
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    if !program_matches(d, entry)
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || source["type_line"] != type_line
        || source["layout"] != "normal"
        || ["card_faces", "loyalty", "defense"]
            .iter()
            .any(|field| source.get(*field).is_some())
        || source["keywords"] != json!([])
        || source["oracle_text"] != text
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source.get("power").cloned() != c.power.as_ref().map(|q| json!(q))
        || source.get("toughness").cloned() != c.toughness.as_ref().map(|q| json!(q))
        || strict_read::<Value>(&d.provenance.abilities_json)? != abilities(entry)
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != [text]
                || !l.triggered.iter().copied().eq(entry.then_some(0))
                || !l.activated.is_empty()
                || !l.statics.is_empty()
                || !l.mana.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

fn program_schema(entry: bool, timing: SpellTiming) -> Value {
    let mut op = json!({"properties":{
        "op":{"const":"return_own_graveyard_card_to_hand"},"target_slot":{"const":0},
        "source_block":{"const":0},"optional":{"const":entry}
    }});
    if entry {
        op["required"] = json!(["optional"]);
    }
    json!({"type":"object","properties":{
        "timing":{"const":timing},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},
        "instructions":{"minItems":1,"maxItems":1,"items":op},
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"selection_id":{"const":selection(entry)},"chooser":{"const":"controller"},
            "stage":{"const":"announcement"},"domain":{"const":"own_graveyard_card"},"filter":{"type":"null"}
        }}}
    }})
}

pub(super) fn extend_schema(s: &mut Value) {
    use Capability::*;
    let has = json!({"type":"object","anyOf":[
        {"required":["instructions"],"properties":{"instructions":{"contains":{"properties":{
            "op":{"const":"return_own_graveyard_card_to_hand"}}}}}},
        {"required":["targets"],"properties":{"targets":{"contains":{"properties":{
            "domain":{"const":"own_graveyard_card"}}}}}}
    ]});
    // SpellProgram is recursive. Checking every immediate mode edge forbids
    // either reserved operand at any depth.
    s["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"modes":{"items":{"properties":{"program":{"not":has}}}}},
        "if":has,"then":{"anyOf":[program_schema(false,SpellTiming::Instant),
            program_schema(false,SpellTiming::MainPhase),program_schema(true,SpellTiming::Instant)]}
    }));
    let caps = |allowed: &[Capability]| {
        json!({"items":{"enum":allowed},"uniqueItems":true,
        "minItems":allowed.len(),"maxItems":allowed.len()})
    };
    let source = |text: &str| {
        json!({"properties":{"source_json":{"pattern":
        format!(r#""oracle_text"\s*:\s*"{}\.""#, text.trim_end_matches('.'))}}})
    };
    let labels = |text: &str, indices: &[u32]| {
        json!({"properties":{
            "text":{"const":text},"blocks":{"const":[text]},"triggered":{"const":indices},
            "activated":{"maxItems":0},"statics":{"maxItems":0},"mana":{"maxItems":0},"spell_modes":{"maxItems":0}
        }})
    };
    let spell = |kind: CardKind, timing, cap| {
        json!({"properties":{
            "spell":program_schema(false,timing),"triggered":{"maxItems":0},
            "characteristics":{"properties":{"kind":{"const":kind},"subtypes":{"maxItems":0},
                "power":{"type":"null"},"toughness":{"type":"null"}}},
            "capabilities":caps(&[NormalLayoutV1,cap,OwnGraveyardCardReturnV1]),
            "provenance":source(SPELL_TEXT),"source_labels":labels(SPELL_TEXT,&[])
        }})
    };
    let excluded = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect::<Vec<_>>();
    let entry = json!({"required":["triggered"],"properties":{
        "spell":{"type":"object","properties":{"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},
            "resolution":{"const":"permanent"},"targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},
        "triggered":{"minItems":1,"maxItems":1,"items":{"properties":{"functioning_zone":{"const":"battlefield"},
            "condition":{"const":"self_enters_battlefield"},"intervening_if":{"type":"null"},"death_filter":{"type":"null"},
            "program":program_schema(true,SpellTiming::Instant)}}},
        "characteristics":{"required":["power","toughness"],"properties":{"kind":{"const":"creature"},
            "subtypes":{"items":{"pattern":r"^\S+$","not":{"enum":excluded}}},
            "power":{"type":"string"},"toughness":{"type":"string","pattern":"^[1-9][0-9]*$"}}},
        "capabilities":caps(&[NormalLayoutV1,BaseCreatureV1,CastCreatureV1,TriggeredAbilityV1,SelfEntersBattlefieldV1,OwnGraveyardCardReturnV1]),
        "provenance":source(ENTRY_TEXT),"source_labels":labels(ENTRY_TEXT,&[0])
    }});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"activated":{"items":{"properties":{"program":{"not":has}}}}},
        "if":{"anyOf":[
            {"properties":{"capabilities":{"contains":{"const":OwnGraveyardCardReturnV1}}}},
            {"required":["spell"],"properties":{"spell":has}},
            {"required":["triggered"],"properties":{"triggered":{"contains":{"properties":{"program":has}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"[^"]*target card from your graveyard to your hand"#}}}}}
        ]},
        "then":{"required":["spell"],"properties":{
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"activated":{"maxItems":0},
            "static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "characteristics":{"properties":{"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "additional_types":{"maxItems":0},"mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}}}
        },"oneOf":[spell(CardKind::Instant,SpellTiming::Instant,CastInstantV1),
            spell(CardKind::Sorcery,SpellTiming::MainPhase,CastSorceryV1),entry,
            {"required":["modal_land"],"properties":{
                "spell":program_schema(false,SpellTiming::MainPhase),
                "triggered":{"maxItems":0},
                "characteristics":{"properties":{"kind":{"const":"sorcery"},"subtypes":{"maxItems":0},
                    "power":{"type":"null"},"toughness":{"type":"null"}}},
                "capabilities":{"allOf":[{"contains":{"const":OwnGraveyardCardReturnV1}},
                    {"contains":{"const":CastSorceryV1}},{"contains":{"const":ModalLandFacesV1}}]},
                "provenance":source(SPELL_TEXT),"source_labels":labels(SPELL_TEXT,&[])
            }}
        ]}
    }));
}

#[cfg(test)]
pub(super) fn witness(kind: CardKind) -> Definition {
    let entry = kind == CardKind::Creature;
    let name = if entry { "Gravedigger" } else { "Raise Dead" };
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let mut d = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == name)
        .unwrap();
    let text = if entry { ENTRY_TEXT } else { SPELL_TEXT };
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    source["oracle_text"] = json!(text);
    let p = if entry {
        &mut d.triggered[0].program
    } else {
        d.spell.as_mut().unwrap()
    };
    if kind == CardKind::Instant {
        source["type_line"] = json!("Instant");
        p.timing = SpellTiming::Instant;
    }
    d.characteristics.kind = kind;
    p.targets[0].domain = TargetDomain::OwnGraveyardCard;
    p.instructions = vec![Instruction::ReturnOwnGraveyardCardToHand {
        target_slot: 0,
        optional: entry,
        source_block: 0,
    }];
    let labels = d.source_labels.as_mut().unwrap();
    labels.text = text.into();
    labels.blocks = vec![text.into()];
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities(entry).to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn own_graveyard_card_return() {
    for kind in [CardKind::Sorcery, CardKind::Instant, CardKind::Creature] {
        let d = witness(kind);
        d.validate().unwrap();
        assert!(
            d.capabilities
                .contains(&Capability::OwnGraveyardCardReturnV1)
        );
        assert!(!d.capabilities.contains(&Capability::GraveyardRecursionV1));
        let entry = kind == CardKind::Creature;
        for mutation in 0..16 {
            let mut bad = d.clone();
            let p = if entry {
                &mut bad.triggered[0].program
            } else {
                bad.spell.as_mut().unwrap()
            };
            match mutation {
                0 => p.targets[0].domain = TargetDomain::OwnGraveyardCreatureCard,
                1 => p.targets[0].slot = 1,
                2 => p.targets[0].selection_id = "line:1:1".into(),
                3 => {
                    p.instructions[0] = Instruction::ReturnOwnGraveyardCardToHand {
                        target_slot: 0,
                        optional: !entry,
                        source_block: 0,
                    }
                }
                4 => {
                    p.instructions[0] = Instruction::ReturnOwnGraveyardCardToHand {
                        target_slot: 0,
                        optional: entry,
                        source_block: 1,
                    }
                }
                5 => p.instructions.push(p.instructions[0].clone()),
                6 => {
                    p.targets[0].filter = Some(PermanentFilter {
                        types: vec![CardKind::Creature],
                        non_types: vec![],
                        non_colors: vec![],
                        opponent: false,
                        tapped: false,
                        power: None,
                    })
                }
                7 => {
                    p.additional_sacrifice = Some(SacrificeFilter {
                        types: vec![CardKind::Creature],
                        subtypes: vec![],
                    })
                }
                8 => p.modes = vec![SpellMode { program: p.clone() }],
                9 => {
                    p.instructions[0] = Instruction::ReturnOwnGraveyardCreatureToHand {
                        target_slot: 0,
                        optional: entry,
                        source_block: 0,
                    }
                }
                10 => bad.x_mana_cost = true,
                11 => bad.keywords.push(KeywordAbility::Flash),
                12 => bad.characteristics.subtypes.push("Equipment".into()),
                13 if entry => bad.triggered[0].condition = TriggerCondition::SelfDies,
                13 => {
                    p.timing = if kind == CardKind::Instant {
                        SpellTiming::MainPhase
                    } else {
                        SpellTiming::Instant
                    }
                }
                14 if entry => {
                    bad.spell.as_mut().unwrap().additional_sacrifice = Some(SacrificeFilter {
                        types: vec![CardKind::Creature],
                        subtypes: vec![],
                    })
                }
                14 => bad.characteristics.snow = true,
                _ => {
                    bad.provenance.abilities_json = "[]".into();
                    bad.provenance.ast_sha256 = sha256(b"[[]]");
                }
            }
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err(), "{kind:?} mutation {mutation}");
        }
        let mut missing = d;
        missing
            .capabilities
            .remove(&Capability::OwnGraveyardCardReturnV1);
        assert!(missing.validate().is_err());
    }
}

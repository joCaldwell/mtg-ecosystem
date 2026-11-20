//! CR 109.5, 110.2a, 400.7, 601.2c and 608.2b–c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const RETURN: &str = "Return target creature card from your graveyard to the battlefield";
const SELECTION: &str = "line:0:1";
const INVALID: &str = "graveyard battlefield return requires its complete ordinary sorcery";

fn has_return(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::ReturnOwnGraveyardCreatureToBattlefield { .. }
        )
    }) || p.modes.iter().any(|mode| has_return(&mode.program))
}

fn target() -> TargetSpec {
    TargetSpec {
        slot: 0,
        selection_id: SELECTION.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::OwnGraveyardCreatureCard,
        filter: None,
    }
}

fn source_effects(gain: Option<u32>) -> (String, Value) {
    let mut text = format!("{RETURN}.");
    let mut effects = vec![
        json!({"steps":[{"effect":"move-zone","to":{"zone":"battlefield"},
            "what":{"ref":"target","filter":{"types":["creature"],"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
                    "count":{"amount":"fixed","value":1},"selectionId":SELECTION}
        }]}),
    ];
    if let Some(value) = gain {
        text.push_str(&format!(" You gain {value} life."));
        effects.push(json!({"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":value}}]}));
    }
    (text, json!([{"kind":"spell","effects":effects}]))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.triggered.iter().any(|a| has_return(&a.program))
        || d.activated.iter().any(|a| has_return(&a.program))
    {
        return Err(INVALID.into());
    }
    let has_op = d.spell.as_ref().is_some_and(has_return);
    if !has_op && !d.provenance.source_json.contains(RETURN) {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    if !has_op
        && !source["oracle_text"]
            .as_str()
            .is_some_and(|s| s.contains(RETURN))
    {
        return Ok(());
    }
    let p = d.spell.as_ref().ok_or(INVALID)?;
    let [
        Instruction::ReturnOwnGraveyardCreatureToBattlefield {
            target_slot: 0,
            source_block: 0,
        },
        tail @ ..,
    ] = p.instructions.as_slice()
    else {
        return Err(INVALID.into());
    };
    let gain = match tail {
        [] => None,
        [
            Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount,
                source_block: 0,
            },
        ] if *amount > Quantity::zero() => {
            Some(amount.to_string().parse::<u32>().map_err(|_| INVALID)?)
        }
        _ => return Err(INVALID.into()),
    };
    let (text, abilities) = source_effects(gain);
    let c = &d.characteristics;
    if p.targets.as_slice() != [target()]
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || c.kind != CardKind::Sorcery
        || !c.subtypes.is_empty()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || source["layout"] != "normal"
        || source["type_line"] != "Sorcery"
        || ["card_faces", "loyalty", "defense", "power", "toughness"]
            .iter()
            .any(|field| source.get(*field).is_some())
        || source["keywords"] != json!([])
        || source["oracle_text"] != text
        || strict_read::<Value>(&d.provenance.abilities_json)? != abilities
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.blocks.as_slice() != [text.as_str()]
                || !labels.activated.is_empty()
                || !labels.triggered.is_empty()
                || !labels.mana.is_empty()
                || !labels.statics.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    // Reconcile the ordinary printed mana with the retained cost, including a
    // printed {0}. The shared validator checks timing, zones and characteristics.
    let mut printed = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        printed.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        printed.push_str(&format!("{{{color:?}}}"));
    }
    if source["mana_cost"] != printed {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let op = "return_own_graveyard_creature_to_battlefield";
    let has = json!({"type":"object","required":["instructions"],"properties":{
        "instructions":{"contains":{"required":["op"],"properties":{"op":{"const":op}}}}
    }});
    let cast = json!({"type":"object","properties":{
        "functioning_zone":{"const":"stack"},"timing":{"const":"main_phase"},
        "resolution":{"const":"instructions"},"additional_sacrifice":{"type":"null"},
        "optional_additional_costs":{"type":"array","maxItems":0},"modes":{"maxItems":0},
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"selection_id":{"const":SELECTION},"chooser":{"const":"controller"},
            "stage":{"const":"announcement"},"domain":{"const":"own_graveyard_creature_card"},"filter":{"type":"null"}
        }}},
        "instructions":{"minItems":1,"maxItems":2,"prefixItems":[
            {"const":{"op":op,"source_block":0,"target_slot":0}},
            {"properties":{"op":{"const":"gain_life"},"recipient":{"const":"controller"},
                           "amount":{"pattern":"^[1-9][0-9]*$"},"source_block":{"const":0}}}
        ],"items":false}
    }});
    // Each child program also uses SpellProgram, so this forbids every nested occurrence.
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"modes":{"items":{"properties":{"program":{"not":has}}}}},
            "if":has,"then":cast
        }));
    let closure = |gain| {
        let mut allowed = vec![
            NormalLayoutV1,
            CastSorceryV1,
            GraveyardRecursionV1,
            GraveyardBattlefieldReturnV1,
        ];
        if gain {
            allowed.push(LifeGainV1);
        }
        let suffix = if gain {
            r" You gain [1-9][0-9]* life\."
        } else {
            ""
        };
        let printed = format!(r#""oracle_text"\s*:\s*"{RETURN}\.{suffix}""#);
        json!({"properties":{
            "capabilities":{"items":{"enum":allowed},"uniqueItems":true,"minItems":allowed.len(),"maxItems":allowed.len()},
            "provenance":{"properties":{"source_json":{"pattern":printed}}}
        }})
    };
    let family = json!({"required":["spell"],"properties":{"spell":has}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "activated":{"items":{"properties":{"program":{"not":has}}}},
            "triggered":{"items":{"properties":{"program":{"not":has}}}}
        },
        "if":{"anyOf":[family,{"properties":{"provenance":{"properties":{
            "source_json":{"pattern":r#""oracle_text"\s*:\s*"[^"]*Return target creature card from your graveyard to the battlefield"#}
        }}}}]},
        "then":{"allOf":[family],"properties":{
            "characteristics":{"properties":{
                "kind":{"const":"sorcery"},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0},
                "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "power":{"type":"null"},"toughness":{"type":"null"},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
            }},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},"keywords":{"maxItems":0},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0}
        },"if":{"properties":{"spell":{"properties":{"instructions":{"minItems":2}}}}},
        "then":closure(true),"else":closure(false)},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":GraveyardBattlefieldReturnV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness(gain: Option<u32>) -> Definition {
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let mut d = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == "Raise Dead")
        .unwrap();
    let (text, abilities) = source_effects(gain);
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    source["oracle_text"] = json!(text);
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities.to_string();
    d.provenance.ast_sha256 = sha256(json!([abilities]).to_string().as_bytes());
    let labels = d.source_labels.as_mut().unwrap();
    labels.blocks = vec![text.clone()];
    labels.text = text;
    let p = d.spell.as_mut().unwrap();
    p.instructions = vec![Instruction::ReturnOwnGraveyardCreatureToBattlefield {
        target_slot: 0,
        source_block: 0,
    }];
    if let Some(value) = gain {
        p.instructions.push(Instruction::GainLife {
            recipient: LifeRecipient::Controller,
            amount: i64::from(value).into(),
            source_block: 0,
        });
    }
    d.capabilities = d.requirements();
    d
}

#[test]
fn graveyard_battlefield_return() {
    use Capability::*;
    for gain in [None, Some(3), Some(17)] {
        let d = witness(gain);
        d.validate().unwrap();
        let mut required = BTreeSet::from([
            NormalLayoutV1,
            CastSorceryV1,
            GraveyardRecursionV1,
            GraveyardBattlefieldReturnV1,
        ]);
        if gain.is_some() {
            required.insert(LifeGainV1);
        }
        assert_eq!(d.requirements(), required);
        for cap in required {
            let mut bad = d.clone();
            bad.capabilities.remove(&cap);
            assert!(bad.validate().is_err());
        }
    }

    let d = witness(Some(3));
    let mutations: &[fn(&mut SpellProgram)] = &[
        |p| p.targets.clear(),
        |p| p.targets[0].domain = TargetDomain::BattlefieldCreature,
        |p| p.targets[0].selection_id = "line:0:2".into(),
        |p| {
            p.instructions[0] = Instruction::ReturnOwnGraveyardCreatureToBattlefield {
                target_slot: 1,
                source_block: 0,
            }
        },
        |p| {
            p.instructions[0] = Instruction::ReturnOwnGraveyardCreatureToHand {
                target_slot: 0,
                source_block: 0,
                optional: false,
            }
        },
        |p| {
            p.instructions[1] = Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: 3.into(),
                source_block: 1,
            }
        },
        |p| {
            p.instructions[1] = Instruction::GainLife {
                recipient: LifeRecipient::EachOpponent,
                amount: 3.into(),
                source_block: 0,
            }
        },
        |p| {
            p.instructions[1] = Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: 0.into(),
                source_block: 0,
            }
        },
        |p| p.instructions.swap(0, 1),
        |p| {
            p.instructions.pop();
        },
        |p| p.instructions.push(p.instructions[1].clone()),
        |p| p.timing = SpellTiming::Instant,
        |p| {
            p.modes.push(SpellMode { program: p.clone() });
            p.targets.clear();
            p.instructions.clear();
        },
        |p| {
            p.optional_additional_costs
                .push(OptionalAdditionalManaCost {
                    identity: "extra".into(),
                    source_block: 0,
                    mana: ManaCost {
                        generic: 1.into(),
                        colored: vec![],
                    },
                })
        },
    ];
    for (i, mutate) in mutations.iter().enumerate() {
        let mut bad = d.clone();
        mutate(bad.spell.as_mut().unwrap());
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "mutation {i}");
    }
    let mut bad = d.clone();
    bad.characteristics.mana_cost.generic = 3.into();
    assert!(bad.validate().is_err());
    let mut bad = d;
    bad.characteristics.kind = CardKind::Instant;
    bad.spell.as_mut().unwrap().timing = SpellTiming::Instant;
    bad.capabilities = bad.requirements();
    assert!(bad.validate().is_err());
    let mut unrelated = controller_library::witness();
    unrelated.capabilities.insert(GraveyardBattlefieldReturnV1);
    assert!(unrelated.validate().is_err());
    assert!(strict_read::<Instruction>(r#"{"op":"return_own_graveyard_creature_to_battlefield","target_slot":0,"source_block":0,"tapped":false}"#).is_err());
}

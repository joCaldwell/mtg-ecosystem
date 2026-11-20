//! CR 109.5, 118.12a, 603.3a, 603.5 and 608.2d (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str =
    "Whenever an opponent casts a spell, you may draw a card unless that player pays ";
const INVALID: &str = "requires the complete opponent-cast optional draw and fixed caster payment";
const CAPS: [Capability; 6] = [
    Capability::NormalLayoutV1,
    Capability::CastEnchantmentV1,
    Capability::TriggeredAbilityV1,
    Capability::CastTriggerV1,
    Capability::OptionalControllerDrawV1,
    Capability::CastPayerDrawV1,
];

pub fn cast_payer_draw_trigger(generic: Quantity) -> TriggeredAbility {
    TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SpellCast {
            caster: CasterRelation::Opponent,
            spell: TypePredicate::Any {},
        },
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            modes: vec![],
            targets: vec![],
            instructions: vec![Instruction::OptionalDrawUnlessCasterPays {
                generic,
                source_block: 0,
            }],
        },
    }
}

fn abilities(generic: &Quantity) -> Value {
    json!([{"kind":"triggered","trigger":{"trigger":"cast","who":{"player":"an-opponent"},"what":{"cls":"spell"}},
        "effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1},"optional":true}],
            "conditionKind":"unless","condition":{"condition":"pays","who":{"player":"that-player"},
                "cost":[{"cost":"mana","symbols":[generic.to_string()]}]}}]}])
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    // Called after deriving exact requirements (including every mode) and checking hashes.
    // Reserve the printed clause even if the operation and capability were erased together.
    let reserved = d.capabilities.contains(&Capability::CastPayerDrawV1);
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    let may_have_payment = raw_ast.contains(r#""pays""#) || raw_ast.contains(r"\u");
    if !reserved && !may_have_payment && !raw.contains("draw a card unless") && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    if !reserved
        && !source["oracle_text"]
            .as_str()
            .is_some_and(|s| s.contains("draw a card unless"))
    {
        if may_have_payment {
            let ast: Value = strict_read(raw_ast)?;
            if ast.as_array().is_some_and(|abilities| {
                abilities.iter().any(|a| {
                    let effect = &a["effects"][0];
                    a["trigger"]["trigger"] == "cast"
                        && effect["condition"]["condition"] == "pays"
                        && effect["steps"][0]["effect"] == "draw"
                })
            }) {
                return Err(INVALID.into());
            }
        }
        return Ok(());
    }
    let [trigger] = d.triggered.as_slice() else {
        return Err(INVALID.into());
    };
    let [Instruction::OptionalDrawUnlessCasterPays { generic, .. }] =
        trigger.program.instructions.as_slice()
    else {
        return Err(INVALID.into());
    };
    let c = &d.characteristics;
    let text = format!("{PREFIX}{{{generic}}}.");
    let mut printed_cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        printed_cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        printed_cost.push_str(&format!("{{{color:?}}}"));
    }
    if *generic <= Quantity::zero()
        || *trigger != cast_payer_draw_trigger(generic.clone())
        || c.kind != CardKind::Enchantment
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !matches!(&d.spell, Some(p) if p.timing == SpellTiming::MainPhase && p.functioning_zone == FunctioningZone::Stack
            && p.resolution == SpellResolution::Permanent && p.additional_sacrifice.is_none()
            && p.optional_additional_costs.is_empty() && p.targets.is_empty() && p.instructions.is_empty() && p.modes.is_empty())
        || source["layout"] != "normal"
        || source["type_line"] != "Enchantment"
        || source["keywords"] != json!([])
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|k| source.get(*k).is_some())
        || source["oracle_text"] != text
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != printed_cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || strict_read::<Value>(&d.provenance.abilities_json)? != abilities(generic)
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != [text.as_str()]
                || l.triggered != [0]
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

pub(super) fn extend_schema(schema: &mut Value) {
    let op =
        json!({"required":["op"],"properties":{"op":{"const":"optional_draw_unless_caster_pays"}}});
    let has = json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":op}}});
    // Every recursive SpellProgram owns this restriction on its immediate mode children.
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"modes":{"items":{"properties":{"program":{"not":has}}}}}
        }));
    let program = |timing, resolution, instructions: Value| {
        json!({"type":"object","properties":{
            "functioning_zone":{"const":"stack"},"timing":{"const":timing},"resolution":{"const":resolution},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "modes":{"maxItems":0},"targets":{"maxItems":0},"instructions":instructions
        }})
    };
    let condition = TriggerCondition::SpellCast {
        caster: CasterRelation::Opponent,
        spell: TypePredicate::Any {},
    };
    let text = format!(r"{PREFIX}\{{[1-9][0-9]*\}}\.");
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"spell":{"not":has},"activated":{"items":{"properties":{"program":{"not":has}}}}},
        "if":{"anyOf":[
            {"properties":{"capabilities":{"contains":{"const":Capability::CastPayerDrawV1}}}},
            {"required":["triggered"],"properties":{"triggered":{"contains":{"properties":{"program":has}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{
                "pattern":r#""oracle_text"\s*:\s*"[^"]*draw a card unless"#}}}}}
        ]},
        "then":{"required":["triggered","spell"],"properties":{
            "spell":program(SpellTiming::MainPhase,SpellResolution::Permanent,json!({"maxItems":0})),
            "triggered":{"minItems":1,"maxItems":1,"items":{"properties":{
                "condition":{"const":condition},"death_filter":{"type":"null"},"intervening_if":{"type":"null"},
                "functioning_zone":{"const":"battlefield"},
                "program":program(SpellTiming::Instant,SpellResolution::Instructions,json!({"minItems":1,"maxItems":1,"items":op}))
            }}},
            "capabilities":{"uniqueItems":true,"minItems":CAPS.len(),"maxItems":CAPS.len(),"items":{"enum":CAPS}},
            "activated":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{"kind":{"const":"enchantment"},
                "additional_types":{"maxItems":0},"subtypes":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}},
                "colors":{"items":{"not":{"const":Color::C}}},"color_identity":{"items":{"not":{"const":Color::C}}}}},
            "source_labels":{"properties":{"text":{"pattern":format!("^{text}$")},
                "blocks":{"minItems":1,"maxItems":1,"items":{"pattern":format!("^{text}$")}},
                "triggered":{"const":[0]},"activated":{"maxItems":0},"statics":{"maxItems":0},"mana":{"maxItems":0},"spell_modes":{"maxItems":0}}},
            "provenance":{"properties":{"source_json":{"pattern":format!(r#""oracle_text"\s*:\s*"{text}""#)}}}
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Glorious Anthem")
    .unwrap();
    let generic = Quantity::from(3);
    let text = format!("{PREFIX}{{{generic}}}.");
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    d.name = "Synthetic cast payment enchantment".into();
    source["name"] = json!(d.name);
    source["oracle_text"] = json!(text);
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities(&generic).to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.source_labels = Some(SourceLabels {
        text: text.clone(),
        blocks: vec![text],
        triggered: vec![0],
        ..Default::default()
    });
    d.static_abilities.clear();
    d.triggered = vec![cast_payer_draw_trigger(generic)];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn cast_payer_draw() {
    let d = witness();
    d.validate().unwrap();
    assert_eq!(d.capabilities, BTreeSet::from(CAPS));
    let op = json!({"op":"optional_draw_unless_caster_pays","generic":"3","source_block":0});
    assert_eq!(
        serde_json::to_value(&d.triggered[0].program.instructions[0]).unwrap(),
        op
    );
    let mut foreign = op.clone();
    foreign["payer"] = json!("controller");
    assert!(serde_json::from_value::<Instruction>(foreign).is_err());
    for cap in CAPS {
        let mut bad = d.clone();
        bad.capabilities.remove(&cap);
        assert!(bad.validate().is_err());
    }
    for change in [
        (|d: &mut Definition| d.triggered.clear()) as fn(&mut Definition),
        |d| d.triggered.push(d.triggered[0].clone()),
        |d| {
            d.triggered[0].program.instructions =
                vec![Instruction::OptionalDrawCards { source_block: 0 }]
        },
        |d| {
            d.triggered[0].condition = TriggerCondition::SpellCast {
                caster: CasterRelation::AnyPlayer,
                spell: TypePredicate::Any {},
            }
        },
        |d| {
            d.triggered[0].condition = TriggerCondition::SpellCast {
                caster: CasterRelation::Opponent,
                spell: TypePredicate::Type {
                    kind: CardKind::Creature,
                },
            }
        },
        |d| d.triggered[0].functioning_zone = FunctioningZone::Stack,
        |d| d.characteristics.subtypes.push("Aura".into()),
        |d| d.spell.as_mut().unwrap().instructions = d.triggered[0].program.instructions.clone(),
        |d| {
            d.triggered[0].program.modes = vec![SpellMode {
                program: d.triggered[0].program.clone(),
            }]
        },
        |d| {
            d.triggered[0].program.instructions = vec![Instruction::OptionalDrawUnlessCasterPays {
                generic: 3.into(),
                source_block: 1,
            }]
        },
        |d| d.triggered[0] = cast_payer_draw_trigger(0.into()),
        |d| d.triggered[0] = cast_payer_draw_trigger(2.into()),
        |d| {
            d.triggered[0]
                .program
                .optional_additional_costs
                .push(OptionalAdditionalManaCost {
                    identity: "extra".into(),
                    source_block: 0,
                    mana: d.characteristics.mana_cost.clone(),
                })
        },
    ] {
        let mut bad = d.clone();
        change(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "{bad:?}");
    }
}

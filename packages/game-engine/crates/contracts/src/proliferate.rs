//! CR 603.6a and 701.34a (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const CLAUSE: &str = "Whenever a land you control enters, proliferate.";
const REMINDER: &str = " (Choose any number of permanents and/or players, then give each another counter of each kind already there.)";

pub fn landfall_ast(text: &str) -> Option<Value> {
    let (word, clause) = text.split_once(" — ").unwrap_or(("", text));
    if clause != CLAUSE && clause.strip_suffix(REMINDER) != Some(CLAUSE) {
        return None;
    }
    let mut ability = json!({"kind":"triggered","trigger":{
        "trigger":"enters","what":controlled_land_query()
    },"effects":[{"steps":[{"effect":"keyword-action","action":"proliferate","who":{"player":"you"}}]}]});
    if !word.is_empty() {
        ability["abilityWord"] = json!(word);
    }
    Some(json!([ability]))
}

pub fn landfall() -> TriggeredAbility {
    let mut trigger = controlled_land_entry_trigger(1.into(), false);
    trigger.program.instructions = vec![Instruction::Proliferate { source_block: 0 }];
    trigger
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    fn contains(p: &SpellProgram) -> bool {
        p.instructions
            .iter()
            .any(|op| matches!(op, Instruction::Proliferate { .. }))
            || p.modes.iter().any(|m| contains(&m.program))
    }
    let present = d.spell.iter().any(contains)
        || d.triggered.iter().any(|t| contains(&t.program))
        || d.activated.iter().any(|a| contains(&a.program));
    if !present
        && !d.provenance.source_json.contains(CLAUSE)
        && !d.provenance.source_json.contains(PUMP_TAIL)
        && !d.provenance.source_json.contains(r"\u")
    {
        return Ok(false);
    }
    let bad = "requires exact controlled land entry and mandatory Proliferate";
    let raw: Value = strict_read(&d.provenance.source_json)?;
    let text = raw["oracle_text"].as_str().ok_or(bad)?;
    if !present && !(text.contains(CLAUSE) || text.contains(PUMP_TAIL)) {
        return Ok(false);
    }
    let timing = if d.characteristics.kind == CardKind::Instant {
        SpellTiming::Instant
    } else {
        SpellTiming::MainPhase
    };
    if let Some((program, ast)) = pump(text, timing) {
        return validate_pump(d, &raw, text, &program, &ast);
    }
    let ast = landfall_ast(text).ok_or(bad)?;
    let c = &d.characteristics;
    if c.kind != CardKind::Creature
        || c.basic
        || c.snow
        || c.legendary
        || !c.additional_types.is_empty()
        || c.subtypes.is_empty()
        || !c.hybrid_mana.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || d.spell.as_ref() != Some(&controlled_land_entry::permanent_spell())
        || d.triggered != [landfall()]
        || raw["oracle_id"] != d.oracle_id
        || raw["id"] != d.provenance.printing_id
        || raw["name"] != d.name
        || raw["layout"] != "normal"
        || raw["type_line"] != format!("Creature — {}", c.subtypes.join(" "))
        || raw["colors"] != json!(c.colors)
        || raw["color_identity"] != json!(c.color_identity)
        || raw["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || raw["power"] != json!(c.power.as_ref().map(ToString::to_string))
        || raw["toughness"] != json!(c.toughness.as_ref().map(ToString::to_string))
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != [text.strip_suffix(REMINDER).unwrap_or(text)]
                || l.triggered != [0]
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(bad.into());
    }
    Ok(true)
}

// One targeted, fixed Layer 7c modifier followed by the nontargeted keyword
// action (CR 608.2b/c and 701.34a). State-based actions wait until the
// complete spell has finished resolving (CR 704.4).
const PUMP_HEAD: &str = "Target creature gets ";
const PUMP_TAIL: &str = " until end of turn. Proliferate.";

pub fn pump(text: &str, timing: SpellTiming) -> Option<(SpellProgram, Value)> {
    let text = text.strip_suffix(REMINDER).unwrap_or(text);
    let (power, toughness) = text
        .strip_prefix(PUMP_HEAD)?
        .strip_suffix(PUMP_TAIL)?
        .split_once('/')?;
    let fixed_negative = |printed: &str| -> Option<(Quantity, Value)> {
        let digits = printed.strip_prefix('-')?;
        let amount = digits.parse::<u32>().ok()?;
        if amount == 0 || amount.to_string() != digits {
            return None;
        }
        Some((
            Quantity::from(-i64::from(amount)),
            json!({"sign":-1,"amount":{"amount":"fixed","value":amount}}),
        ))
    };
    let (power, p) = fixed_negative(power)?;
    let (toughness, t) = fixed_negative(toughness)?;
    let selection_id = "line:0:0";
    let step = json!({"effect":"pump","what":{"ref":"target","filter":{"types":["creature"]},
        "count":{"amount":"fixed","value":1},"selectionId":selection_id},
        "power":p,"toughness":t,"duration":{"duration":"end-of-turn"}});
    let ast = json!([{"kind":"spell","effects":[{"steps":[step]},
        {"steps":[{"effect":"keyword-action","action":"proliferate","who":{"player":"you"}}]}
    ]}]);
    let program = SpellProgram {
        timing,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: selection_id.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::BattlefieldCreature,
            filter: None,
        }],
        instructions: vec![
            Instruction::pump(kir::SetExpr::target(0), power, toughness, 0),
            Instruction::Proliferate { source_block: 0 },
        ],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        modes: vec![],
    };
    Some((program, ast))
}

fn validate_pump(
    d: &Definition,
    raw: &Value,
    text: &str,
    program: &SpellProgram,
    ast: &Value,
) -> Result<bool, String> {
    let bad = "requires a complete fixed creature modifier followed by mandatory Proliferate";
    let c = &d.characteristics;
    let type_line = match c.kind {
        CardKind::Instant => "Instant",
        CardKind::Sorcery => "Sorcery",
        _ => return Err(bad.into()),
    };
    if c.basic
        || c.snow
        || c.legendary
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.triggered.is_empty()
        || !d.keywords.is_empty()
        || d.spell.as_ref() != Some(program)
        || raw["type_line"] != type_line
        || raw["name"] != d.name
        || raw["oracle_id"] != d.oracle_id
        || raw["id"] != d.provenance.printing_id
        || raw["layout"] != "normal"
        || raw["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || raw["colors"] != json!(c.colors)
        || raw["color_identity"] != json!(c.color_identity)
        || strict_read::<Value>(&d.provenance.abilities_json)? != *ast
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != [text.strip_suffix(REMINDER).unwrap_or(text)]
                || !l.triggered.is_empty()
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(bad.into());
    }
    Ok(true)
}

pub(super) fn selector() -> Value {
    json!({"properties":{"provenance":{"properties":{"source_json":{
        "pattern":r#""oracle_text"\s*:\s*"(?:[^"\\]* — )?Whenever a land you control enters, proliferate\.(?: \(Choose any number of permanents and/or players, then give each another counter of each kind already there\.\))?""#
    }}}}})
}

fn pump_selector() -> Value {
    json!({"properties":{"provenance":{"properties":{"source_json":{
        "pattern":r#""oracle_text"\s*:\s*"Target creature gets -[1-9][0-9]*/-[1-9][0-9]* until end of turn\. Proliferate\.(?: \(Choose any number of permanents and/or players, then give each another counter of each kind already there\.\))?""#
    }}}}})
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let cap = json!({"contains":{"const":ProliferateV1}});
    let caps = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        TriggeredAbilityV1,
        ControlledLandEntryV1,
        ProliferateV1,
    ];
    let (pump, _) = pump(
        "Target creature gets -1/-1 until end of turn. Proliferate.",
        SpellTiming::MainPhase,
    )
    .unwrap();
    let fixed_negative = json!({"pattern":"^-[1-9][0-9]*$"});
    let instructions = json!({"minItems":2,"maxItems":2,"prefixItems":[
        instruction_schema::at(
            instruction_schema::pump(instruction_schema::target_object(Some(0)), fixed_negative.clone(), fixed_negative),
            0,
        ),
        {"const":Instruction::Proliferate {source_block:0}}
    ]});
    let required = [
        NormalLayoutV1,
        CreatureTargetV1,
        TemporaryPowerToughnessV1,
        ProliferateV1,
    ];
    let allowed = [required.as_slice(), &[CastInstantV1, CastSorceryV1]].concat();
    let spell_shape = json!({"required":["spell"],"allOf":[{
        "if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
        "then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},
            "capabilities":{"allOf":[{"contains":{"const":CastInstantV1}},{"not":{"contains":{"const":CastSorceryV1}}}]}}},
        "else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},
            "capabilities":{"allOf":[{"contains":{"const":CastSorceryV1}},{"not":{"contains":{"const":CastInstantV1}}}]}}}
    }], "properties":{
        "capabilities":{"items":{"enum":allowed},"allOf":required.map(|c|json!({"contains":{"const":c}}))},
        "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"basic":{"const":false},
            "legendary":{"const":false},"snow":{"const":false},"hybrid_mana":{"maxItems":0},
            "additional_types":{"maxItems":0},"subtypes":{"maxItems":0}}},
        "x_mana_cost":{"const":false},"keywords":{"maxItems":0},"intrinsic_zone":{"type":"null"},
        "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},
        "triggered":{"maxItems":0},
        "spell":{"properties":{"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},
            "targets":{"const":pump.targets},"instructions":instructions}}
    }});
    instruction_schema::forbid_unbound_operation(schema, "ProliferateFreeProgram", "proliferate");
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":selector(),
        "then":{"required":["spell","triggered"],"properties":{
            "capabilities":{"items":{"enum":caps},"allOf":caps.map(|c|json!({"contains":{"const":c}}))},
            "characteristics":{"properties":{"kind":{"const":"creature"},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"hybrid_mana":{"maxItems":0},
                "additional_types":{"maxItems":0},"subtypes":{"minItems":1}}},
            "x_mana_cost":{"const":false},"keywords":{"maxItems":0},"intrinsic_zone":{"type":"null"},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},
            "spell":{"const":controlled_land_entry::permanent_spell()},"triggered":{"const":[landfall()]}
        }},
         "else":{"if":pump_selector(),"then":spell_shape,
            "else":{"allOf":[{"properties":{"capabilities":{"not":cap}}},
                instruction_schema::operation_free_definition("ProliferateFreeProgram")]}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = controlled_land_entry::witness();
    let r: Value = strict_read(include_str!(
        "../../../tests/fixtures/b45fdeab-00cc-4422-af9f-66f30a880a7c.json"
    ))
    .unwrap();
    let raw = &r["source"];
    let c = &mut d.characteristics;
    c.subtypes = vec!["Elf".into(), "Druid".into()];
    c.mana_cost = ManaCost {
        generic: 2.into(),
        colored: vec![Color::G],
    };
    c.power = Some(3.into());
    c.toughness = Some(2.into());
    d.name = raw["name"].as_str().unwrap().into();
    d.oracle_id = raw["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = raw["id"].as_str().unwrap().into();
    d.provenance.source_index = r["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(raw).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&r["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = r["bindingSha256"].as_str().unwrap().into();
    d.triggered = vec![landfall()];
    d.capabilities = d.requirements();
    d
}

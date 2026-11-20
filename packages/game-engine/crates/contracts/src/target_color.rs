//! CR 105.4, 608.2d, 611.2c, 613.1e and 613.7 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "{T}: Target ";
const SUFFIX: &str = " becomes the color of your choice until end of turn.";

pub fn source(text: &str) -> Option<(ActivatedAbility, Value)> {
    let (domain, filter, ast_filter) = match text.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)? {
        "permanent" => (
            TargetDomain::BattlefieldPermanent,
            Some(PermanentFilter {
                types: vec![],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
                power: None,
            }),
            json!({"cls":"permanent"}),
        ),
        "creature" => (
            TargetDomain::BattlefieldCreature,
            None,
            json!({"types":["creature"]}),
        ),
        _ => return None,
    };
    let selection_id = "line:0:2";
    let ast = json!([{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
        "effect":"modify-characteristics","what":{"ref":"target","filter":ast_filter,
            "count":{"amount":"fixed","value":1},"selectionId":selection_id},
        "modifiers":[{"modifier":"colors","operation":"set","colors":{
            "choice":"fresh-choice","category":"color","who":{"player":"you"},
            "when":"effect-created","quantity":"one"}}],
        "snapshot":"effect-created","duration":{"duration":"end-of-turn"}
    }]}]}]);
    Some((
        ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost: ActivationCost::TapSource,
            program: SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: selection_id.into(),
                    domain,
                    filter,
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                }],
                instructions: vec![Instruction::SetTargetColor {
                    target_slot: 0,
                    source_block: 0,
                }],
                modes: vec![],
                optional_additional_costs: vec![],
                additional_sacrifice: None,
            },
        },
        ast,
    ))
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    fn has(p: &SpellProgram) -> bool {
        p.instructions
            .iter()
            .any(|op| matches!(op, Instruction::SetTargetColor { .. }))
            || p.modes.iter().any(|mode| has(&mode.program))
    }
    let present = d.activated.iter().any(|a| has(&a.program))
        || d.spell.iter().any(has)
        || d.triggered.iter().any(|a| has(&a.program));
    if !present
        && !d.provenance.source_json.contains(SUFFIX)
        && !d.provenance.source_json.contains(r"\u")
    {
        return Ok(false);
    }
    let bad = "requires the complete single tap ability setting one target color until end of turn";
    let raw: Value = strict_read(&d.provenance.source_json)?;
    let text = raw["oracle_text"].as_str().ok_or(bad)?;
    if !present && !text.contains(SUFFIX) {
        return Ok(false);
    }
    let (ability, ast) = source(text).ok_or(bad)?;
    let c = &d.characteristics;
    let type_line = match c.kind {
        CardKind::Artifact if c.subtypes.is_empty() => "Artifact".to_string(),
        CardKind::Creature if !c.subtypes.is_empty() => {
            format!("Creature — {}", c.subtypes.join(" "))
        }
        _ => return Err(bad.into()),
    };
    if c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.mana_abilities.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.spell.as_ref() != Some(&controlled_land_entry::permanent_spell())
        || d.activated != [ability]
        || raw["type_line"] != type_line
        || raw["name"] != d.name
        || raw["id"] != d.provenance.printing_id
        || raw["oracle_id"] != d.oracle_id
        || raw["layout"] != "normal"
        || raw["colors"] != json!(c.colors)
        || raw["color_identity"] != json!(c.color_identity)
        || raw["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || (c.kind == CardKind::Creature
            && (raw["power"] != json!(c.power.as_ref().map(ToString::to_string))
                || raw["toughness"] != json!(c.toughness.as_ref().map(ToString::to_string))))
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != [text]
                || l.activated != [0]
                || !l.mana.is_empty()
                || !l.triggered.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(bad.into());
    }
    Ok(true)
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let cap = json!({"contains":{"const":TargetColorSetV1}});
    let source_matches = |noun| {
        json!({"properties":{"provenance":{"properties":{"source_json":{
            "pattern":format!(r#""oracle_text"\s*:\s*"\{{T\}}: Target {noun} becomes the color of your choice until end of turn\.""#)
        }}}}})
    };
    let all_kinds = [CardKind::Artifact, CardKind::Creature];
    let allowed = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        CastArtifactV1,
        CreatureTargetV1,
        PermanentRemovalV1,
        ActivatedAbilityV1,
        TapSourceCostV1,
        TargetColorSetV1,
    ];
    let then = |noun| {
        let (ability, _) = source(&format!("{PREFIX}{noun}{SUFFIX}")).unwrap();
        json!({"required":["spell","activated"],"properties":{
            "capabilities":{"items":{"enum":allowed},"allOf":[cap.clone(),
                {"contains":{"const":NormalLayoutV1}},{"contains":{"const":ActivatedAbilityV1}},
                {"contains":{"const":TapSourceCostV1}}]},
            "characteristics":{"properties":{"kind":{"enum":all_kinds},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},
                "hybrid_mana":{"maxItems":0}}},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "keywords":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},
            "triggered":{"maxItems":0},"activated":{"const":[ability]},
            "spell":{"const":controlled_land_entry::permanent_spell()}
        }})
    };
    instruction_schema::forbid_unbound_operation(
        schema,
        "TargetColorFreeProgram",
        "set_target_color",
    );
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":source_matches("permanent"),
            "then":then("permanent"),
            "else":{"if":source_matches("creature"),"then":then("creature"),
                "else":{"allOf":[{"properties":{"capabilities":{"not":cap}}},
                instruction_schema::operation_free_definition("TargetColorFreeProgram")]}}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = proliferate::witness();
    let r: Value = strict_read(include_str!(
        "../../../tests/fixtures/8fc72080-8d4d-427d-ae30-065bc8ee9a28.json"
    ))
    .unwrap();
    let raw = &r["source"];
    let c = &mut d.characteristics;
    c.subtypes = vec!["Merfolk".into(), "Wizard".into()];
    c.mana_cost = ManaCost {
        generic: 0.into(),
        colored: vec![Color::U],
    };
    c.colors = vec![Color::U];
    c.color_identity = vec![Color::U];
    c.power = Some(1.into());
    c.toughness = Some(1.into());
    d.name = raw["name"].as_str().unwrap().into();
    d.oracle_id = raw["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = raw["id"].as_str().unwrap().into();
    d.provenance.source_index = r["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(raw).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&r["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = r["bindingSha256"].as_str().unwrap().into();
    d.triggered.clear();
    d.activated = vec![source(raw["oracle_text"].as_str().unwrap()).unwrap().0];
    d.capabilities = d.requirements();
    d
}

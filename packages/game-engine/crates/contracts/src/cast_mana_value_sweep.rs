//! CR 202.3, 601.2i, 608.2h and 702.85c. Source-bound mana value of other casts.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const CASCADES: &str = "Cascade, cascade";
const REMINDER: &str = " (When you cast this spell, exile cards from the top of your library until you exile a nonland card that costs less. You may cast it without paying its mana cost. Put the exiled cards on the bottom of your library in a random order. Then do it again.)";
const DAMAGE: &str = " deals damage to each creature your opponents control equal to the total mana value of other spells you've cast this turn.";
const INVALID: &str =
    "requires a complete ordinary double cascade sorcery and other-casts damage sweep";

fn abilities() -> Value {
    json!([
        {"kind":"keywords","keywords":[{"keyword":"cascade"},{"keyword":"cascade"}]},
        {"kind":"spell","effects":[{"steps":[{"effect":"damage","source":{"ref":"self"},
            "amount":{"amount":"cast-mana-value-sum","who":{"player":"you"},"during":"this-turn",
                "excluding":{"ref":"this","noun":"spell"}},
            "to":{"to":"object","ref":{"ref":"each","filter":{"types":["creature"],"control":"opponent"}}}
        }]}]}
    ])
}

fn instruction() -> Instruction {
    SetDamage::instruction(
        vec![ObjectSet::OpponentCreatures.kernel(Context::Spell)],
        false,
        kir::Amount::OtherCastsManaValue {
            caster: PlayerExpr::you(Context::Spell),
        },
        1,
    )
}

fn cascade() -> TriggeredAbility {
    TriggeredAbility {
        functioning_zone: FunctioningZone::Stack,
        condition: TriggerCondition::SelfSpellCast,
        intervening_if: None,
        death_filter: None,
        program: canonical_cascade_program(0),
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    // requirements() walks spell, activation, trigger, grant and recursive mode programs.
    // Its equality with declared capabilities is checked for every definition.
    let reserved = d.capabilities.contains(&Capability::CastManaValueSweepV1);
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    let may_have_amount = raw_ast.contains("cast-mana-value-sum") || raw_ast.contains(r"\u");
    if !reserved && !may_have_amount && !raw.contains(DAMAGE) && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    if !reserved && !text.contains(DAMAGE) {
        // The source and the amount may be stripped independently from forged
        // definitions. The retained AST still obliges us to execute this operand.
        fn has_amount(node: &Value) -> bool {
            match node {
                Value::Array(items) => items.iter().any(has_amount),
                Value::Object(fields) => {
                    node["amount"] == "cast-mana-value-sum" || fields.values().any(has_amount)
                }
                _ => false,
            }
        }
        if may_have_amount && has_amount(&strict_read(raw_ast)?) {
            return Err(INVALID.into());
        }
        return Ok(());
    }

    let c = &d.characteristics;
    let p = d.spell.as_ref().ok_or(INVALID)?;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }

    if c.kind != CardKind::Sorcery
        || c.snow
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || d.x_mana_cost
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.triggered != [cascade(), cascade()]
        || p.timing != SpellTiming::MainPhase
        || p.resolution != SpellResolution::Instructions
        || p.functioning_zone != FunctioningZone::Stack
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || p.instructions != [instruction()]
        || source["layout"] != "normal"
        || source["type_line"] != "Sorcery"
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|field| source.get(*field).is_some())
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["keywords"] != json!(["Cascade"])
        || source["mana_cost"] != cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || text != format!("{CASCADES}{REMINDER}\n{}{DAMAGE}", d.name)
        || strict_read::<Value>(&d.provenance.abilities_json)? != abilities()
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != [CASCADES, &format!("~{DAMAGE}")]
                || l.triggered != [0, 0]
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

fn program_schema() -> Value {
    json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":
        instruction_schema::set_damage(json!({"required":["amount"],"properties":{"amount":{"const":"other_casts_mana_value"}}}))
    }}})
}

pub(super) fn spell_schema() -> Value {
    json!({"required":["spell"],"properties":{"spell":program_schema()}})
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let has = program_schema();
    // Every nested mode is a SpellProgram and checks the same edge.
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"modes":{"items":{"properties":{"program":{"not":has}}}}}
        }));
    let caps = [
        NormalLayoutV1,
        CastSorceryV1,
        TriggeredAbilityV1,
        CascadeV1,
        ResolutionCastV1,
        RandomBottomV1,
        BoardWipesV1,
        CastHistoryV1,
        CastManaValueSweepV1,
    ];
    let source = json!({"properties":{"provenance":{"properties":{"source_json":{
        "pattern":r#""oracle_text"\s*:\s*"[^"]* deals damage to each creature your opponents control equal to the total mana value of other spells you've cast this turn\."#
    }}}}});
    let ast = json!({"properties":{"provenance":{"properties":{"abilities_json":{
        "pattern":r#""amount"\s*:\s*"cast-mana-value-sum""#
    }}}}});
    let forbidden = json!({"items":{"properties":{"program":{"not":has}}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"triggered":forbidden,"activated":forbidden},
        "if":{"anyOf":[spell_schema(),source,ast,{"properties":{"capabilities":{"contains":{"const":CastManaValueSweepV1}}}}]},
        "then":{"required":["spell","triggered"],"properties":{
            "capabilities":{"items":{"enum":caps},"minItems":caps.len(),"maxItems":caps.len(),"uniqueItems":true},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "keywords":{"maxItems":0},"mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"static_abilities":{"maxItems":0},
            "characteristics":{"properties":{"kind":{"const":"sorcery"},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},
            "spell":{"type":"object","properties":{
                "timing":{"const":"main_phase"},"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},
                "targets":{"maxItems":0},"modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},
                "instructions":{"const":[instruction()]}
            }},
            "triggered":{"minItems":2,"maxItems":2,"items":{"properties":{"condition":{"const":"self_spell_cast"},
                "program":{"properties":{"instructions":{"const":canonical_cascade_program(0).instructions}}}
            }}},
            "source_labels":{"properties":{"blocks":{"const":[CASCADES,format!("~{DAMAGE}")]},
                "triggered":{"const":[0,0]},"statics":{"maxItems":0},"mana":{"maxItems":0},"activated":{"maxItems":0},"spell_modes":{"maxItems":0}}},
            "provenance":{"properties":{
                "source_json":{"pattern":r#""oracle_text"\s*:\s*"Cascade, cascade \(When you cast this spell,[^"]*Then do it again\.\)\\n[^"]+ deals damage to each creature your opponents control equal to the total mana value of other spells you've cast this turn\.""#},
                "abilities_json":{"pattern":r#""amount"\s*:\s*"cast-mana-value-sum""#}
            }}
        }}
    }));
}

#[cfg(test)]
pub(super) mod tests;

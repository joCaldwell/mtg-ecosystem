//! CR 400.7, 608.2b–d/h, 701.23b and 701.24a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub const TARGET_EXILE_SEARCH: &str = "Exile target creature. Its controller may search their library for a basic land card, put that card onto the battlefield tapped, then shuffle.";

pub fn exile_search_spec(text: &str, kind: CardKind) -> Option<(Value, SpellProgram)> {
    if text != TARGET_EXILE_SEARCH || kind != CardKind::Instant {
        return None;
    }
    let who = json!({"player":"controller","of":{"ref":"it"}});
    let selection = "line:0:1";
    let ast = json!([{"kind":"spell","effects":[
        {"steps":[{"effect":"exile","what":{"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":selection}}]},
        {"steps":[{"effect":"optional-group","who":who,"steps":[
            {"effect":"search","who":who,"zones":{"selection":"all","of":[{"zone":"library","owner":"their"}]},
                "for":{"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},
                    "count":{"amount":"fixed","value":1}}},
            {"effect":"move-zone","what":{"ref":"that","filter":{"cls":"card"}},"to":{"zone":"battlefield"},"tapped":true,"who":who},
            {"effect":"shuffle","who":who,"library":{"zone":"library","owner":who}}
        ]}]}
    ]}]);
    let recipient = EffectRecipient::TargetController { target_slot: 0 };
    let program = SpellProgram {
        timing: SpellTiming::Instant,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: selection.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::BattlefieldPermanent,
            filter: Some(PermanentFilter {
                types: vec![CardKind::Creature],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
                power: None,
            }),
        }],
        instructions: vec![
            Instruction::ExilePermanent {
                target_slot: 0,
                source_block: 0,
            },
            Instruction::ChooseOptional {
                chooser: recipient.clone(),
                following: 1,
                source_block: 0,
            },
            Instruction::SearchRecipientBasicLand {
                recipient,
                source_block: 0,
            },
        ],
        modes: vec![],
    };
    Some((ast, program))
}

pub(super) fn uses(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::ChooseOptional { .. } | Instruction::SearchRecipientBasicLand { .. }
        )
    }) || program.modes.iter().any(|m| uses(&m.program))
}

pub(super) fn validate(definition: &Definition) -> Result<(), String> {
    let error = "requires the complete printed optional block and its bound recipient";
    if definition.activated.iter().any(|a| uses(&a.program))
        || definition.static_abilities.iter().any(|a| matches!(&a.effect,
            StaticEffect::Bestow { program, .. } | StaticEffect::GrantSpellCascade { program, .. } if uses(program)))
    {
        return Err(error.into());
    }
    let used = definition.spell.as_ref().is_some_and(uses);
    let raw = &definition.provenance.source_json;
    if !used && !raw.contains(TARGET_EXILE_SEARCH) && !raw.contains(r"\u") {
        return Ok(());
    }
    let row: Value = strict_read(raw)?;
    let text = row["oracle_text"].as_str().unwrap_or_default();
    if !used && text != TARGET_EXILE_SEARCH {
        return Ok(());
    }
    let c = &definition.characteristics;
    let (ast, expected) = exile_search_spec(text, c.kind).ok_or(error)?;
    if definition.spell.as_ref() != Some(&expected)
        || strict_read::<Value>(&definition.provenance.abilities_json)? != ast
        || row["name"] != definition.name
        || row["oracle_id"] != definition.oracle_id
        || row["id"] != definition.provenance.printing_id
        || row["layout"] != "normal"
        || row["type_line"] != "Instant"
        || row["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || row["colors"] != json!(c.colors)
        || row["color_identity"] != json!(c.color_identity)
        || !c.hybrid_mana.is_empty()
        || definition.x_mana_cost
        || definition.intrinsic_zone.is_some()
        || !definition.activated.is_empty()
        || !definition.triggered.is_empty()
        || !definition.mana_abilities.is_empty()
        || !definition.static_abilities.is_empty()
        || !definition.keywords.is_empty()
    {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let ops = json!({"required":["op"],"properties":{"op":{"enum":["choose_optional","search_recipient_basic_land"]}}});
    let free = json!({"$ref":"#/$defs/OptionalBlockFreeProgram"});
    schema["$defs"]["OptionalBlockFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":ops}}, "modes":{"items":{"properties":{"program":free}}}
    }});
    let cap = json!({"contains":{"const":Capability::OptionalEffectBlockV1}});
    let program = exile_search_spec(TARGET_EXILE_SEARCH, CardKind::Instant)
        .unwrap()
        .1;
    let optional =
        json!({"properties":{"program":{"type":"object","not":free}},"required":["program"]});
    let scalar = json!({"required":["condition","program"],"properties":{
        "functioning_zone":{"const":"battlefield"}, "intervening_if":{"type":"null"}, "death_filter":{"type":"null"},
        "condition":{"enum":[TriggerCondition::OtherCreatureEntersBattlefield,TriggerCondition::StepBegan {
            step:BeginningStep::Upkeep, turn_relation:TurnRelation::Controller
        }]},
        "program":{"required":["instructions","targets"],"properties":{
            "functioning_zone":{"const":"stack"}, "timing":{"const":"instant"}, "resolution":{"const":"instructions"},
            "modes":{"maxItems":0}, "targets":{"maxItems":0}, "optional_additional_costs":{"maxItems":0}, "additional_sacrifice":{"type":"null"},
            "instructions":{"minItems":2,"maxItems":2,"prefixItems":[
                {"properties":{"op":{"const":"choose_optional"},"chooser":{"const":EffectRecipient::Controller},"following":{"const":1}}},
                {"properties":{"op":{"const":"gain_life"},"recipient":{"const":LifeRecipient::Controller},"amount":{"pattern":"^[1-9][0-9]*$"}}}
            ]}
        }}
    }});
    let spell = json!({"anyOf":[
        {"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
        {"required":["provenance"],"properties":{"provenance":{"required":["source_json"],
            "properties":{"source_json":{"pattern":TARGET_EXILE_SEARCH.replace('.', r"\.")}}}}}
    ]});
    let entry_trigger = json!({"const": optional_entry_search::printed(optional_entry_search::TEXT, 0, CardKind::Creature).unwrap().1});
    let entry_source = json!({"required":["provenance"],"properties":{"provenance":{"required":["source_json"],
        "properties":{"source_json":{"pattern":optional_entry_search::TEXT.replace('.', r"\.")}}}}});
    let scalar_source = json!({"required":["provenance"],"properties":{"provenance":{"required":["source_json"],
        "properties":{"source_json":{"pattern":"(Whenever another creature enters,|At the beginning of your upkeep,) you may gain [1-9][0-9]* life[.]"}}}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({
            "properties":{
                "activated":{"items":{"properties":{"program":free}}},
                "triggered":{"items":{"if":optional,"then":{"anyOf":[scalar,entry_trigger]}}},
                "static_abilities":{"items":{"properties":{"effect":{"properties":{
                    "bestow":{"properties":{"program":free}},
                    "grant_spell_cascade":{"properties":{"program":free}}
                }}}}}
            },
            "if":spell,
            "then":{"properties":{
                "characteristics":{"properties":{"kind":{"const":"instant"}}},
                "spell":{"const":program}, "activated":{"maxItems":0}, "triggered":{"maxItems":0},
                "static_abilities":{"maxItems":0}, "keywords":{"maxItems":0}, "mana_abilities":{"maxItems":0}
            },"required":["spell"]}
        }),
        json!({
            "if":{"anyOf":[spell,scalar_source,entry_source,{"required":["triggered"],"properties":{"triggered":{"contains":optional}}}]},
            "then":{"properties":{"capabilities":cap}},
            "else":{"properties":{"capabilities":{"not":cap}}}
        }),
        json!({"if":scalar_source,"then":{"required":["triggered"],"properties":{"triggered":{"contains":optional}}}}),
        json!({"if":entry_source,"then":{"required":["triggered"],"properties":{"triggered":{"contains":entry_trigger}}}})
    ]);
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .extend(optional_entry_search::entry_schema());
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/phase1-r3/d683d985-9888-4d21-8b5f-69e69ce4a03b.json"
    ))
    .unwrap();
    let mut definition = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.characteristics.kind == CardKind::Instant)
    .unwrap();
    composition_fixtures::stamp(&mut definition, &row);
    definition.characteristics.colors = vec![Color::W];
    definition.characteristics.color_identity = vec![Color::W];
    definition.characteristics.mana_cost = ManaCost {
        generic: Quantity::zero(),
        colored: vec![Color::W],
    };
    definition.spell = Some(
        exile_search_spec(TARGET_EXILE_SEARCH, CardKind::Instant)
            .unwrap()
            .1,
    );
    definition.capabilities = definition.requirements();
    definition
}

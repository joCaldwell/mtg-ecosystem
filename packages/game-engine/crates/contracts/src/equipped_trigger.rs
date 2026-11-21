//! Equipment-owned triggered blocks. CR 109.5, 301.5, 603.3, 603.10a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub const DAMAGE: &str = "Whenever equipped creature deals combat damage to a player, ";
pub const DIES: &str = "Whenever equipped creature dies, ";

pub fn is_equipped(a: &TriggeredAbility) -> bool {
    matches!(
        a.condition,
        TriggerCondition::EquippedCombatDamage | TriggerCondition::EquippedDies
    )
}

pub fn count(text: &str) -> Option<u32> {
    if text == "a" || text == "an" {
        return Some(1);
    }
    text.parse::<u32>()
        .ok()
        .filter(|n| *n > 0 && n.to_string() == text)
        .or_else(|| (1..100).find(|&n| matches_fixed_amount(text, i64::from(n))))
}

pub fn draw_count(text: &str) -> Option<u32> {
    let (n, noun) = text.split_once(' ')?;
    let count = count(n)?;
    (noun == if count == 1 { "card" } else { "cards" }).then_some(count)
}

pub fn printed(line: &str, raw: &Value, block: u32) -> Option<TriggeredAbility> {
    let (condition, body) = if let Some(body) = line.strip_prefix(DAMAGE) {
        (TriggerCondition::EquippedCombatDamage, body)
    } else {
        (TriggerCondition::EquippedDies, line.strip_prefix(DIES)?)
    };
    let (body, reminder) = body
        .split_once(" (")
        .map_or((body, None), |(b, r)| (b, Some(r)));
    let what = json!({"ref":"equipped","noun":"creature"});
    let trigger = if condition == TriggerCondition::EquippedDies {
        json!({"trigger":"dies","what":what})
    } else {
        // The archived 'any' field also occurs for the player-only printed
        // phrase. Use the independently checked printed recipient above.
        json!({"trigger":"deals-damage","what":what,"combat":true,"to":"any"})
    };
    let mut targets = Vec::new();
    let mut instructions = Vec::new();
    let mut effects = Vec::new();
    let sentences = raw["effects"].as_array()?;
    for (i, sentence) in body.strip_suffix('.')?.split(". ").enumerate() {
        let mut steps = Vec::new();
        for (j, part) in sentence.split(" and ").enumerate() {
            let (operand, instruction) = if let Some(step) = predefined_tokens::printed(part, block)
            {
                step
            } else if let Some(text) = part
                .strip_prefix("this Equipment deals ")
                .and_then(|s| s.strip_suffix(" damage to any target"))
            {
                let n = count(text)?;
                if !targets.is_empty() {
                    return None;
                }
                let id = sentences.get(i)?["steps"].get(j)?["to"]["selectionId"].as_str()?;
                let position = id
                    .strip_prefix(&format!("line:{block}:"))?
                    .parse::<u32>()
                    .ok()?;
                if id != format!("line:{block}:{position}") {
                    return None;
                }
                let spec = TargetSpec {
                    slot: 0,
                    selection_id: id.into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    domain: TargetDomain::AnyDamageTarget,
                    filter: None,
                };
                targets.push(spec);
                (
                    json!({"effect":"damage","source":{"ref":"this","noun":"equipment"},
                    "amount":{"amount":"fixed","value":n},"to":{"to":"any-target","selectionId":id}}),
                    Instruction::DealDamage {
                        target_slot: 0,
                        amount: i64::from(n).into(),
                        source_block: block,
                    },
                )
            } else if let Some(n) = part
                .strip_prefix("you gain ")
                .or_else(|| part.strip_prefix("gain "))
                .and_then(|s| s.strip_suffix(" life"))
                .and_then(count)
            {
                (
                    json!({"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":n}}),
                    Instruction::act(
                        Verb::GainLife {
                            player: PlayerExpr::you(Context::Triggered),
                            amount: Amount::fixed(i64::from(n)),
                        },
                        block,
                    ),
                )
            } else if part == "that player discards a card"
                && condition == TriggerCondition::EquippedCombatDamage
            {
                (
                    json!({"effect":"discard","who":{"player":"that-player"},"what":{"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":1}}}),
                    Instruction::DiscardCards {
                        recipient: Some(ActionRecipient::EventPlayer),
                        count: 1.into(),
                        source_block: block,
                    },
                )
            } else if part == "you untap all lands you control"
                || part == "untap all lands you control"
            {
                (
                    json!({"effect":"untap","what":{"ref":"all","filter":{"types":["land"],"control":"you"}}}),
                    Instruction::UntapControllerLands {
                        source_block: block,
                    },
                )
            } else {
                let (text, optional) = if let Some(text) = part.strip_prefix("you may draw ") {
                    (text, true)
                } else {
                    (
                        part.strip_prefix("you draw ")
                            .or_else(|| part.strip_prefix("draw "))?,
                        false,
                    )
                };
                let n = draw_count(text)?;
                let mut step = json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":n}});
                let op = if optional {
                    if n != 1 {
                        return None;
                    }
                    step["optional"] = json!(true);
                    Instruction::OptionalDrawCards {
                        source_block: block,
                    }
                } else if n <= 2 {
                    if n == 2 {
                        instructions.push(Instruction::DrawCards {
                            recipient: None,
                            count: 1.into(),
                            source_block: block,
                        });
                    }
                    Instruction::DrawCards {
                        recipient: None,
                        count: 1.into(),
                        source_block: block,
                    }
                } else {
                    return None;
                };
                (step, op)
            };
            steps.push(operand);
            instructions.push(instruction);
        }
        effects.push(json!({"steps":steps}));
    }
    if reminder.is_some_and(|r| !predefined_tokens::reminder(r, &instructions)) {
        return None;
    }
    if *raw != json!({"kind":"triggered","trigger":trigger,"effects":effects}) {
        return None;
    }
    Some(TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            timing: SpellTiming::Instant,
            functioning_zone: FunctioningZone::Stack,
            resolution: SpellResolution::Instructions,
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            modes: vec![],
            targets,
            instructions,
        },
    })
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let actual = d.triggered.iter().filter(|a| is_equipped(a)).count();
    if actual == 0
        && !d
            .provenance
            .source_json
            .contains("Whenever equipped creature ")
    {
        return Ok(());
    }
    let fail = "requires exact printed and typed equipped trigger";
    let src: Value = strict_read(&d.provenance.source_json)?;
    let text = src["oracle_text"].as_str().ok_or(fail)?;
    let clauses: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, t)| t.starts_with(DAMAGE) || t.starts_with(DIES))
        .collect();
    if actual == 0 && clauses.is_empty() {
        return Ok(());
    }
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    if actual != clauses.len()
        || ast.len() != text.lines().count()
        || d.characteristics.kind != CardKind::Artifact
        || d.characteristics.subtypes != ["Equipment"]
        || !d.mana_abilities.is_empty()
        || src["layout"] != "normal"
        || src["name"] != d.name
        || src["oracle_id"] != d.oracle_id
        || src["id"] != d.provenance.printing_id
    {
        return Err(fail.into());
    }
    for (block, line) in clauses {
        let index = ast[..block]
            .iter()
            .filter(|a| a["kind"] == "triggered")
            .count();
        let n = u32::try_from(block).map_err(|_| fail)?;
        if printed(line, &ast[block], n).as_ref() != d.triggered.get(index)
            || d.triggered.get(index).is_none()
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.triggered.get(index) != Some(&n))
        {
            return Err(fail.into());
        }
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let condition =
        json!({"enum":[TriggerCondition::EquippedCombatDamage,TriggerCondition::EquippedDies]});
    let consumes =
        json!({"contains":{"required":["condition"],"properties":{"condition":condition}}});
    let cap = json!({"contains":{"const":Capability::EquippedCreatureTriggerV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["triggered"],"properties":{"triggered":consumes}},
        "then":{"properties":{"capabilities":cap,"characteristics":{"properties":{"kind":{"const":"artifact"},"subtypes":{"const":["Equipment"]}}},
            "triggered":{"items":{"if":{"properties":{"condition":condition}},"then":{"properties":{
                "functioning_zone":{"const":"battlefield"},"intervening_if":{"type":"null"},"death_filter":{"type":"null"},
                "program":{"properties":{"timing":{"const":"instant"},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
                    "optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
                    "instructions":{"minItems":1,"items":{"anyOf":[{"properties":{"op":{"enum":["draw_cards","optional_draw_cards","deal_damage","discard_cards","untap_controller_lands","create_predefined_tokens"]}}},instruction_schema::kernel_verb("gain_life")]}}}}
            }}}}}},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/308d7868-49f9-47a3-a7b1-4b0332d610f1.json"
    ))
    .unwrap();
    let source = &row["source"];
    let ast = &row["faces"][0]["result"]["abilities"];
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Vulshok Morningstar")
    .unwrap();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = row["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(ast).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = row["bindingSha256"].as_str().unwrap().into();
    d.static_abilities.clear();
    d.source_labels = None;
    d.triggered = vec![
        printed(
            source["oracle_text"]
                .as_str()
                .unwrap()
                .lines()
                .next()
                .unwrap(),
            &ast[0],
            0,
        )
        .unwrap(),
    ];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn slice4_capability_equipped_has_a_complete_witness() {
    let d = witness();
    assert!(
        d.capabilities
            .contains(&Capability::EquippedCreatureTriggerV1)
    );
    d.validate().unwrap();
}

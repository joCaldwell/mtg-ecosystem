//! A fixed controller-life effect chosen during resolution, CR 603.5 and 608.2d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ENTRY: &str = "Whenever another creature enters, you may gain ";
const UPKEEP: &str = "At the beginning of your upkeep, you may gain ";

fn body(line: &str, kind: CardKind) -> Option<(TriggerCondition, Value, u32)> {
    let (condition, trigger, life) = if let Some(life) = line.strip_prefix(ENTRY) {
        if kind != CardKind::Creature {
            return None;
        }
        (
            TriggerCondition::OtherCreatureEntersBattlefield,
            json!({"trigger":"enters","what":{"ref":"filter","filter":{"other":true,"types":["creature"]},
                "count":{"amount":"fixed","value":1}}}),
            life,
        )
    } else if let Some(life) = line.strip_prefix(UPKEEP) {
        if !matches!(
            kind,
            CardKind::Enchantment | CardKind::Creature | CardKind::Artifact
        ) {
            return None;
        }
        (
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::Controller,
            },
            json!({"trigger":"phase","phase":"upkeep","whose":"your"}),
            life,
        )
    } else {
        return None;
    };
    let life = life.strip_suffix(" life.")?.parse::<u32>().ok()?;
    (life > 0).then_some((condition, trigger, life))
}

pub fn candidate(line: &str, kind: CardKind) -> bool {
    body(line, kind).is_some()
        || (kind == CardKind::Creature && line == optional_entry_search::TEXT)
}

pub fn printed(
    line: &str,
    raw: &Value,
    kind: CardKind,
    source_block: u32,
) -> Option<TriggeredAbility> {
    if let Some((ast, ability)) = optional_entry_search::printed(line, source_block, kind) {
        return (*raw == ast).then_some(ability);
    }
    let (condition, trigger, life) = body(line, kind)?;
    let expected = json!({"kind":"triggered","trigger":trigger,
        "effects":[{"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":life},"optional":true}]}]
    });
    (*raw == expected).then_some(TriggeredAbility {
        condition,
        functioning_zone: FunctioningZone::Battlefield,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            targets: vec![],
            modes: vec![],
            instructions: vec![
                Instruction::ChooseOptional {
                    chooser: EffectRecipient::Controller,
                    following: 1,
                    source_block,
                },
                Instruction::act(
                    Verb::GainLife {
                        player: PlayerExpr::you(Context::Triggered),
                        amount: Amount::fixed(i64::from(life)),
                    },
                    source_block,
                ),
            ],
        },
    })
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let actual = d
        .triggered
        .iter()
        .filter(|a| optional_block::uses(&a.program))
        .count();
    let raw = &d.provenance.source_json;
    const CLUE: &str = "you may gain ";
    if actual == 0
        && !raw.contains(CLUE)
        && !raw.contains(optional_entry_search::TEXT)
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let expected: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| candidate(line, kind))
        .collect();
    if actual == 0 && expected.is_empty() {
        return Ok(());
    }
    let fail = "requires the exact printed optional trigger and its controller";
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    if actual != expected.len()
        || ast.len() != text.lines().count()
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["layout"] != "normal"
    {
        return Err(fail.into());
    }
    if text.lines().any(|line| line == optional_entry_search::TEXT) {
        optional_entry_search::validate_siblings(d, &source, text, &ast)?;
    }
    for (block, line) in expected {
        let source_block = u32::try_from(block).map_err(|_| fail)?;
        let ordinal = ast[..block]
            .iter()
            .filter(|a| a["kind"] == "triggered")
            .count();
        if !printed(line, &ast[block], kind, source_block)
            .is_some_and(|a| d.triggered.get(ordinal) == Some(&a))
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.triggered.get(ordinal) != Some(&source_block))
        {
            return Err(fail.into());
        }
    }
    Ok(())
}

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

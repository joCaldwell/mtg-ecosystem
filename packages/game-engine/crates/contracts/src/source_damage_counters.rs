//! Source anaphora on a combat-damage trigger. CR 201.5, 400.7, 603.3a, 608.2h/k.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "Whenever this creature deals combat damage to a player, put ";

pub fn candidate(line: &str) -> bool {
    amount(line).is_some()
}

fn amount(line: &str) -> Option<u32> {
    let rest = line.strip_prefix(PREFIX)?;
    if rest == "a +1/+1 counter on it." {
        return Some(1);
    }
    let n = equipped_trigger::count(rest.strip_suffix(" +1/+1 counters on it.")?)?;
    (n > 1).then_some(n)
}

fn condition() -> TriggerCondition {
    TriggerCondition::DamageDealt {
        source: DamageTriggerSource::Source {},
        kind: TriggerDamageKind::Combat,
        recipient: DamageTriggerRecipient::Player,
    }
}

pub fn printed(line: &str, ast: &Value, source_block: u32) -> Option<TriggeredAbility> {
    let n = amount(line)?;
    let expected = json!({"kind":"triggered",
        "trigger":{"trigger":"deals-damage","what":{"ref":"this","noun":"creature"},"combat":true,"to":"any"},
        "effects":[{"steps":[{"effect":"put-counters","on":{"ref":"it"},
            "counter":{"type":"+1/+1"},"count":{"amount":"fixed","value":n}}]}]
    });
    (*ast == expected).then_some(TriggeredAbility {
        condition: condition(),
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
            instructions: vec![Instruction::plus_one_counters(
                kir::SetExpr::source(),
                i64::from(n).into(),
                source_block,
            )],
        },
    })
}

//! CR 201.5, 601.2c, 608.2b-c and 120.2b: the spell itself deals the fixed
//! damage to the same announced creature, subject to resolution legality.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const SUFFIX: &str = " damage to target creature.";

fn printed_amount(source: &Value) -> Option<u32> {
    let amount = source["oracle_text"]
        .as_str()?
        .strip_prefix(source["name"].as_str()?)?
        .strip_prefix(" deals ")?
        .strip_suffix(SUFFIX)?;
    let parsed: u32 = amount.parse().ok()?;
    (parsed > 0 && amount == parsed.to_string()).then_some(parsed)
}

/// Reconstruct the complete ordinary spell from its printed source and archived AST.
pub fn program(source: &Value, ast: &Value) -> Option<SpellProgram> {
    let timing = match source["type_line"].as_str()? {
        "Instant" => SpellTiming::Instant,
        "Sorcery" => SpellTiming::MainPhase,
        _ => return None,
    };
    let amount = printed_amount(source)?;
    let selection = "line:0:5";
    let expected = json!([{"kind":"spell","effects":[{"steps":[{
        "effect":"damage","source":{"ref":"self"},"amount":{"amount":"fixed","value":amount},
        "to":{"to":"object","ref":{"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":selection}}
    }]}]}]);
    if ast != &expected {
        return None;
    }
    Some(SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: selection.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::BattlefieldCreature,
            filter: None,
        }],
        instructions: vec![Instruction::damage_target(
            0,
            kir::Amount::Fixed {
                value: Quantity::from(i64::from(amount)),
            },
            0,
        )],
        modes: vec![],
    })
}

fn uses(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|instruction| {
        matches!(instruction, kernel!(Damage { to: kir::DamageTo::Ref { who: kir::Ref::Target { slot: target_slot } }, amount: fixed!(_), .. })
            if p.targets.get(*target_slot as usize)
                .is_some_and(|t| t.domain == TargetDomain::BattlefieldCreature))
    }) || p.modes.iter().any(|mode| uses(&mode.program))
}

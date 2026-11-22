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

pub(super) fn validate(d: &Definition, fixed_activations: bool) -> Result<(), String> {
    let fail = || "requires the complete ordinary fixed creature damage spell".to_string();
    if (!fixed_activations && d.activated.iter().any(|a| uses(&a.program)))
        || d.triggered.iter().any(|a| uses(&a.program))
        || d.static_abilities.iter().any(
            |a| matches!(&a.effect, StaticEffect::GrantSpellCascade { program, .. } if uses(program)),
        )
    {
        return Err(fail());
    }
    let active = d.spell.as_ref().is_some_and(uses);
    if !active && !d.provenance.source_json.contains(SUFFIX) {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    if !active && printed_amount(&source).is_none() {
        return Ok(());
    }
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let expected = program(&source, &ast).ok_or_else(fail)?;
    let c = &d.characteristics;
    let kind = match expected.timing {
        SpellTiming::Instant => CardKind::Instant,
        SpellTiming::MainPhase => CardKind::Sorcery,
    };
    if d.spell.as_ref() != Some(&expected)
        || d.x_mana_cost
        || c.kind != kind
        || !c.hybrid_mana.is_empty()
        || c.basic
        || c.legendary
        || c.snow
        || !c.subtypes.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
    {
        return Err(fail());
    }
    Ok(())
}

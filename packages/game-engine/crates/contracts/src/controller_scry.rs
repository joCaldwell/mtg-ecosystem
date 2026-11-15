//! Controller-only ordinary scry. CR 109.5, 608.2c, 701.22a-d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(crate) fn has_scry(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::Scry { .. }))
        || p.modes.iter().any(|mode| has_scry(&mode.program))
}

/// Complete nontargeted ordinary scry, alone or with one mandatory card draw.
/// Either ordering and original source-block provenance are meaningful.
pub fn controller_scry_program(p: &SpellProgram) -> bool {
    if p.additional_sacrifice.is_some()
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
    {
        return false;
    }
    let scry =
        |i: &Instruction| matches!(i, Instruction::Scry { amount, .. } if !amount.is_negative());
    let draw = |i: &Instruction| matches!(i, Instruction::DrawCards { recipient: None | Some(LifeRecipient::Controller), count, .. } if *count == Quantity::from(1));
    match p.instructions.as_slice() {
        [i] => scry(i),
        [a, b] => (scry(a) && draw(b)) || (draw(a) && scry(b)),
        _ => false,
    }
}

pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_scry(&a.program))
        || d.triggered.iter().any(|a| has_scry(&a.program))
    {
        return Err("scry requires an ordinary spell source".into());
    }
    if !d.spell.as_ref().is_some_and(has_scry) {
        return Ok(());
    }
    let c = &d.characteristics;
    if !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.additional_types.is_empty()
        || c.basic
        || c.legendary
        || c.power.is_some()
        || c.toughness.is_some()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
    {
        return Err("scry requires a complete ordinary spell definition".into());
    }
    Ok(())
}

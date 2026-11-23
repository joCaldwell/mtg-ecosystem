//! Pure sacrifice-source / controlled-creature keyword composition. No source
//! identity, quantity or individual keyword selects an activation mechanism.
use super::*;

/// CR 602.2b, 608.2h, 611.2c: pay the source, then capture one complete set at
/// resolution through the same program used by ordinary temporary set spells.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub fn sacrifice_source_creature_set_keyword_ability(
    a: &ActivatedAbility,
) -> Option<TemporarySetKeywordProgram<'_>> {
    if a.functioning_zone != FunctioningZone::Battlefield
        || a.cost != ActivationCost::SacrificeSource
        || a.program.timing != SpellTiming::Instant
    {
        return None;
    }
    let p = temporary_creature_set_keyword_program(&a.program)?;
    (*p.set == ObjectSet::ControllerCreatures && p.modifier.is_none()).then_some(p)
}

pub(super) fn definition(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Creature
        && d.characteristics.additional_types.is_empty()
        && !d.characteristics.legendary
        && !d.characteristics.basic
        && d.spell.as_ref().is_some_and(|p| {
            p.timing == SpellTiming::MainPhase
                && p.resolution == SpellResolution::Permanent
                && p.functioning_zone == FunctioningZone::Stack
                && p.targets.is_empty()
                && p.instructions.is_empty()
                && p.modes.is_empty()
                && p.additional_sacrifice.is_none()
                && p.optional_additional_costs.is_empty()
        })
        && d.intrinsic_zone.is_none()
        && !d.x_mana_cost
        && d.keywords.is_empty()
        && d.static_abilities.is_empty()
        && d.mana_abilities.is_empty()
        && d.triggered.is_empty()
        && matches!(d.activated.as_slice(), [a]
            if sacrifice_source_creature_set_keyword_ability(a).is_some())
}



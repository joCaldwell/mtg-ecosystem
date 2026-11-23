//! CR 702.29a, 602.2a-b, 701.9a (2026-09-25): fixed mana, discard
//! this hand source, draw one. No cycling observers/modifiers are admitted.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub fn hand_discard_draw_activation(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.functioning_zone == FunctioningZone::Hand
        && matches!(&a.cost, ActivationCost::DiscardSource { mana } if !mana.generic.is_negative())
        && p.functioning_zone == FunctioningZone::Stack
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.optional_additional_costs.is_empty()
        && p.additional_sacrifice.is_none()
        && p.targets.is_empty()
        && p.modes.is_empty()
        && matches!(p.instructions.as_slice(), [kernel!(Draw { player: you!(), amount: fixed!(count) })] if *count == Quantity::from(1))
}

// Complete source composition currently qualified for lowering. Neither the
// subtype count nor the mana quantity selects a mechanic or a capacity.
pub(super) fn definition(d: &Definition) -> bool {
    land_definition(d) || spell_definition(d)
}

fn land_definition(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Land
        && !d.characteristics.basic
        && !d.characteristics.subtypes.is_empty()
        && d.characteristics
            .subtypes
            .iter()
            .all(|s| basic_land_color(s).is_some())
        && d.mana_abilities.is_empty()
        && d.triggered.is_empty()
        && d.keywords.is_empty()
        && matches!(d.activated.as_slice(), [a] if hand_discard_draw_activation(a))
        && matches!(
            d.static_abilities.as_slice(),
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped
            }]
        )
}

fn spell_definition(d: &Definition) -> bool {
    matches!(
        d.characteristics.kind,
        CardKind::Instant | CardKind::Sorcery
    ) && !d.x_mana_cost
        && d.characteristics.hybrid_mana.is_empty()
        && d.mana_abilities.is_empty()
        && d.static_abilities.is_empty()
        && d.triggered.is_empty()
        && d.keywords.is_empty()
        && matches!(d.activated.as_slice(), [a] if hand_discard_draw_activation(a)
            && matches!(a.program.instructions.as_slice(), [draw @ kernel!(Draw { .. } @ 1)] if draw.uncounted_draw()))
        && d.spell.as_ref().is_some_and(|p| {
            p.resolution == SpellResolution::Instructions
                && p.modes.is_empty()
                && p.additional_sacrifice.is_none()
                && p.optional_additional_costs.is_empty()
        })
}

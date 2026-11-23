//! CR 106.1b, 109.5 and 605.1a: select one mana output and its disjoint
//! ability. The colored ability deals damage to the activator as an effect.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub fn disjoint_pain_mana_family(definition: &Definition) -> bool {
    let [safe, colored] = definition.mana_abilities.as_slice() else {
        return false;
    };
    let ManaProduction::Choice { colors } = &colored.production else {
        return false;
    };
    let characteristics = &definition.characteristics;
    let source_shape = match characteristics.kind {
        CardKind::Land => {
            definition.intrinsic_zone == Some(FunctioningZone::Battlefield)
                && definition.spell.is_none()
        }
        CardKind::Artifact => {
            definition.intrinsic_zone.is_none()
                && definition.spell.as_ref().is_some_and(|spell| {
                    spell.resolution == SpellResolution::Permanent
                        && spell.timing == SpellTiming::MainPhase
                        && spell.functioning_zone == FunctioningZone::Stack
                        && spell.instructions.is_empty()
                        && spell.targets.is_empty()
                        && spell.modes.is_empty()
                        && spell.optional_additional_costs.is_empty()
                        && spell.additional_sacrifice.is_none()
                })
        }
        _ => false,
    };
    source_shape
        && characteristics.additional_types.is_empty()
        && characteristics.subtypes.is_empty()
        && !characteristics.basic
        && !characteristics.snow
        && definition.keywords.is_empty()
        && definition.activated.is_empty()
        && definition.triggered.is_empty()
        && definition.static_abilities.is_empty()
        && !definition.x_mana_cost
        && safe.source_block == 0
        && colored.source_block == 1
        && [safe, colored].iter().all(|ability| {
            ability.costs.tap
                && ability.costs.mana.is_none()
                && !ability.costs.sacrifice
                && ability.costs.remove_source_plus_one_counters.is_none()
                && ability.restriction.is_none()
        })
        && matches!(&safe.production, ManaProduction::Fixed { symbols } if symbols.as_slice() == [Color::C])
        && safe.self_damage.is_none()
        && colored
            .self_damage
            .as_ref()
            .is_some_and(|n| n > &Quantity::zero())
        && (2..=5).contains(&colors.len())
        && colors.iter().all(|color| *color != Color::C)
        && colors.iter().collect::<BTreeSet<_>>().len() == colors.len()
}

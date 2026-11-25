//! CR 702.29a/e-f, 602.2a-b, 701.9a (2026-09-25): fixed mana, discard
//! this hand source, draw or search. Cycling observers/modifiers are unaudited.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn cycling_cost(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.functioning_zone == FunctioningZone::Hand
        && a.restriction.is_none()
        && matches!(&a.cost, ActivationCost::DiscardSource { mana }
            if !mana.generic.is_negative() && !mana.colored.contains(&Color::C))
        && p.functioning_zone == FunctioningZone::Stack
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.optional_additional_costs.is_empty()
        && p.additional_sacrifice.is_none()
        && p.targets.is_empty()
}

pub(crate) fn hand_discard_draw_activation(a: &ActivatedAbility) -> bool {
    cycling_cost(a)
        && matches!(a.program.instructions.as_slice(), [kernel!(Draw { player: you!(), amount: fixed!(count) })] if *count == Quantity::from(1))
}

pub(crate) fn typecycling_activation(a: &ActivatedAbility) -> bool {
    use kir::{Amount, Atom, Destination, Node, PlayerExpr, SearchSpec, Verb};
    let player = PlayerExpr::you(kir::Context::Activated);
    let [Instruction::Kernel {
        node: Node::Do(Verb::Search { player: searcher, search: SearchSpec {
            filter, maximum: Amount::Fixed { value }, to: Destination::OwnerHand, reveal: true,
        } }),
        source_block: search_block,
    }, Instruction::Kernel {
        node: Node::Do(Verb::Shuffle { player: shuffler }),
        source_block: shuffle_block,
    }] = a.program.instructions.as_slice() else {
        return false;
    };
    cycling_cost(a)
        && *searcher == player
        && *shuffler == player
        && search_block == shuffle_block
        && *value == Quantity::from(1)
        && match filter.0.as_slice() {
            [Atom::Type { kind: CardKind::Land }, Atom::Basic] => true,
            [Atom::Subtype { subtype }] => matches!(subtype.as_str(),
                "Plains" | "Island" | "Swamp" | "Mountain" | "Forest" | "Halfling" | "Sliver" | "Wizard"),
            _ => false,
        }
}

pub fn hand_discard_activation(a: &ActivatedAbility) -> bool {
    hand_discard_draw_activation(a) || typecycling_activation(a)
}

// Complete source composition currently qualified for lowering. Neither the
// subtype count nor the mana quantity selects a mechanic or a capacity.

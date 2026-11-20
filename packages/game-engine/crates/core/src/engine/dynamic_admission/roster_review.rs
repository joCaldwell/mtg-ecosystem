//! A closed headless qualification, pinned to both roster and compiled definitions.
//! Changes to either input require another review; normal capability checks still apply
//! to all other decks. No card identity selects executor rules or ceilings.
use super::*;
use crate::engine::capacities::{DAMAGE_LIMIT, EFFECT_LIMIT, EVENT_GROUP_LIMIT};

const COPY_DECK: (&str, &str) = (
    "fc760bcb3e02ad494005b69f3d6c200115399e2a5692782cd14d4cb6a418d3d3",
    "c818ecb0465ed7514e28100e5f2c27d1c238feed0e8b408d15ceb697f9c07763",
);
const CONTROL_DECK: (&str, &str) = (
    "fed5f2569774b2e30d475eb575884f6a88e32a3a9520ee63c0b06bd3fdc616e4",
    "584186446f00614c51143c48bff0d6a994d03a19c46be484889cbf6dc0742864",
);
pub(super) const POLICY_INPUT: [(&str, &str); 2] = [COPY_DECK, CONTROL_DECK];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Review {
    Copy,
    Control,
}

pub(super) fn seat(catalog: &Catalog, rows: &[DynamicDeckEntry]) -> Option<Review> {
    if rows.len() > DYNAMIC_MAX_DECK_ROWS || rows.iter().any(|row| !is_oracle_id(&row.oracle_id)) {
        return None;
    }
    let mut rows: Vec<_> = rows.iter().collect();
    rows.sort_unstable_by(|left, right| {
        (&left.oracle_id, left.role, left.quantity).cmp(&(
            &right.oracle_id,
            right.role,
            right.quantity,
        ))
    });
    let roster = digest(&rows).ok()?;
    let (review, expected_definitions) = match roster.as_str() {
        pin if pin == COPY_DECK.0 => (Review::Copy, COPY_DECK.1),
        pin if pin == CONTROL_DECK.0 => (Review::Control, CONTROL_DECK.1),
        _ => return None,
    };
    let definitions = rows
        .iter()
        .map(|row| catalog.definition_by_oracle_id(&row.oracle_id))
        .collect::<Option<Vec<_>>>()?;
    (digest(&definitions).ok()? == expected_definitions).then_some(review)
}

/// Account for copied sources by the closed definitions, never by native
/// physical multiplicities alone. CR 707.2/707.10 copy the defined values.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn capacity_supported(catalog: &Catalog, seats: [Option<Review>; 4]) -> bool {
    if seats
        .iter()
        .filter(|seat| **seat == Some(Review::Copy))
        .count()
        != 1
        || seats
            .iter()
            .filter(|seat| **seat == Some(Review::Control))
            .count()
            != 3
    {
        return false;
    }
    // In this projection copying flattens to an immutable definition. Broad
    // observers are cast and creature entry only. Template births and entries
    // from a typed library search have at most two members; a copied permanent
    // resolves singly. Other triggers in the projection are source-local:
    // damage, leave/tap, chapter and step-began. No copyable broad dies trigger.
    let sources = PHYSICAL_SOURCES + TOKEN_CAPACITY as usize;
    let slots_per_source = catalog
        .definitions
        .iter()
        .map(|d| d.triggered.len())
        .max()
        .unwrap_or(0);
    if slots_per_source > 4 {
        return false;
    }
    let Some(cast) = catalog.cast_trigger_fanout_bound(sources) else {
        return false;
    };
    // Include temporary Cascade grants and the shared effect/registration
    // capacity. Sum separate wave bounds conservatively for one action.
    let enter = 2 * (sources * slots_per_source + EFFECT_LIMIT + slots_per_source);
    let damage = DAMAGE_LIMIT * slots_per_source;
    let other_group = EVENT_GROUP_LIMIT * slots_per_source;
    let phase_or_combat = 2 * sources * slots_per_source;
    cast.checked_add(EFFECT_LIMIT + enter + damage + other_group + phase_or_combat)
        .is_some_and(|bound| bound < STACK_LIMIT / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_review_does_not_extend_to_a_different_mix() {
        let catalog = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![],
        };
        assert!(seat(&catalog, &[]).is_none());
        assert!(!capacity_supported(&catalog, [Some(Review::Copy); 4]));
        assert!(!capacity_supported(&catalog, [Some(Review::Control); 4]));
        assert!(!capacity_supported(
            &catalog,
            [
                Some(Review::Copy),
                Some(Review::Control),
                Some(Review::Control),
                None
            ]
        ));
    }
}

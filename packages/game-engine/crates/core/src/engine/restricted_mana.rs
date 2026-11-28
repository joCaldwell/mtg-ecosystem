//! CR 106.4, 106.6, 601.2h, 602.2b and 733 (2026-09-25). Restricted mana
//! is stored by spending predicate and color and offered by payment context.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn credit(pool: &mut Vec<RestrictedMana>, unit: &RestrictedMana) {
    match pool.binary_search_by_key(&unit.key(), RestrictedMana::key) {
        Ok(index) => pool[index].amount.add(&unit.amount),
        Err(index) => pool.insert(index, unit.clone()),
    }
}

// A finite budget for spending rules across the six mana types. Each distinct
// rule and color pair uses one bucket regardless of the amount stored.
pub(super) const MAX_RESTRICTED_BUCKETS: usize = 17 * 6;

pub(super) fn valid_buckets(pool: &[RestrictedMana]) -> bool {
    pool.len() <= MAX_RESTRICTED_BUCKETS
        && pool
            .iter()
            .all(|unit| unit.amount > Quantity::zero() && unit.restriction.valid_bucket())
        && pool.windows(2).all(|pair| pair[0].key() < pair[1].key())
}

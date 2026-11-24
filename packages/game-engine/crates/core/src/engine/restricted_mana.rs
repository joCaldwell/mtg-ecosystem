//! CR 106.4, 106.6, 601.2h, 602.2b and 733 (2026-09-25). Restricted mana
//! can be added and is kept in its own buckets; `Pay { Spend }` offers only
//! unrestricted mana, so no payment spends a bucket.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn credit(pool: &mut Vec<RestrictedMana>, unit: &RestrictedMana) {
    match pool.binary_search_by_key(&unit.key(), RestrictedMana::key) {
        Ok(index) => pool[index].amount.add(&unit.amount),
        Err(index) => pool.insert(index, unit.clone()),
    }
}

pub(super) const MAX_RESTRICTED_BUCKETS: usize = 4 * 6;

pub(super) fn valid_buckets(pool: &[RestrictedMana]) -> bool {
    pool.len() <= MAX_RESTRICTED_BUCKETS
        && pool
            .iter()
            .all(|unit| unit.amount > Quantity::zero() && unit.restriction.is_canonical())
        && pool.windows(2).all(|pair| pair[0].key() < pair[1].key())
}

//! Exact mana spending shared by the simulator policies and host search
//! rollouts: colored pips first, then generic in a fixed color order.
use mtg_contracts::{Color, ManaCost, Quantity};
use std::collections::BTreeMap;

pub use mtg_contracts::mana_spend::{payment, spend};

pub fn spend_answer(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Option<mtg_core::Answer> {
    let unrestricted = spend(cost, available)?;
    Some(mtg_core::Answer::Pay {
        pay: mtg_core::PayAnswer::Spend { unrestricted },
    })
}

/// Each restricted bucket in the decision is eligible for that purchase.
/// Prefer those buckets and retain unrestricted mana for other payments.
pub fn pool_spend_answer(cost: &ManaCost, pool: &mtg_core::ManaPool) -> Option<mtg_core::Answer> {
    Some(mtg_core::Answer::Pay {
        pay: pool.spend(cost)?,
    })
}

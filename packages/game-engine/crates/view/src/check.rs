//! Structural validation is shared by the two identity spaces.
use super::shape::*;
use mtg_contracts::Quantity;
use mtg_contracts::{Color, ManaCost};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

impl Shape {
    pub fn check(&self, answer: &Answer) -> Result<(), String> {
        match (self, answer) {
            (Self::Pick { slots, rules }, Answer::Pick { picks }) => {
                check_picks(slots, rules, picks)
            }
            (Self::Order { items, min, max }, Answer::Order { order }) => {
                if within(order.len(), *min, *max)
                    && order.iter().all(|i| items.contains(i))
                    && distinct(order.iter())
                {
                    Ok(())
                } else {
                    Err("invalid ordered selection".into())
                }
            }
            (
                Self::Distribute {
                    total,
                    buckets,
                    gates,
                },
                Answer::Distribute { amounts },
            ) => check_allocation(total, buckets, gates, amounts),
            (Self::Number { min, max }, Answer::Number { value })
                if value >= min && max.as_ref().is_none_or(|upper| value <= upper) =>
            {
                Ok(())
            }
            (
                Self::Pay {
                    step: PayStep::Produce { sources },
                    ..
                },
                Answer::Pay {
                    pay: PayAnswer::Activate { source },
                },
            ) if sources.contains(source) => Ok(()),
            (
                Self::Pay {
                    step: PayStep::Produce { .. },
                    ..
                },
                Answer::Pay {
                    pay: PayAnswer::Finish { .. },
                },
            ) => Ok(()),
            (
                Self::Pay {
                    step: PayStep::Spend,
                    cost,
                    pool,
                },
                Answer::Pay { pay },
            ) => {
                let (unrestricted, restricted) =
                    pay.spending().ok_or("expected spend allocation")?;
                subtract_selected_mana(pool.unrestricted.clone(), unrestricted)?;
                subtract_restricted_mana(pool.restricted.clone(), restricted)?;
                let mut total = unrestricted.clone();
                for unit in restricted {
                    total.entry(unit.color).or_default().add(&unit.amount);
                }
                validate_selected_mana_cost(cost, &total)
            }
            _ => Err("answer outside decision shape".into()),
        }
    }
}

pub fn subtract_restricted_mana(
    mut pool: Vec<mtg_contracts::RestrictedMana>,
    selected: &[mtg_contracts::RestrictedMana],
) -> Result<Vec<mtg_contracts::RestrictedMana>, String> {
    if selected
        .windows(2)
        .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err("restricted allocation must use distinct ordered buckets".into());
    }
    for unit in selected {
        let index = pool
            .binary_search_by_key(&unit.key(), mtg_contracts::RestrictedMana::key)
            .map_err(|_| "restricted mana bucket unavailable")?;
        if unit.amount <= Quantity::zero() || unit.amount > pool[index].amount {
            return Err("invalid restricted mana amount".into());
        }
        pool[index].amount.subtract(&unit.amount);
    }
    pool.retain(|unit| unit.amount > Quantity::zero());
    Ok(pool)
}

fn within(len: usize, min: u32, max: u32) -> bool {
    (min as usize..=max as usize).contains(&len)
}

fn distinct<'a>(mut indices: impl Iterator<Item = &'a u32>) -> bool {
    let mut seen = BTreeSet::new();
    indices.all(|i| seen.insert(*i))
}

fn check_picks(slots: &[Slot], rules: &[JointRule], picks: &[Vec<u32>]) -> Result<(), String> {
    if slots.len() != picks.len() {
        return Err("expected one selection per slot".into());
    }
    for (slot, picked) in slots.iter().zip(picks) {
        if !within(picked.len(), slot.min, slot.max)
            || !picked.iter().all(|i| slot.allowed.contains(i))
            || (!slot.repeat && !distinct(picked.iter()))
        {
            return Err("invalid slot selection".into());
        }
    }
    for rule in rules {
        let satisfied = match rule {
            JointRule::Distinct { slots } => {
                let mut seen = BTreeSet::new();
                slots.iter().all(|slot| {
                    picks
                        .get(*slot as usize)
                        .is_some_and(|picked| picked.iter().all(|i| seen.insert(*i)))
                })
            }
            JointRule::PickedAtLeast { candidates, count } => candidates.iter().all(|candidate| {
                let occurrences = picks.iter().flatten().filter(|i| *i == candidate).count();
                occurrences == 0 || occurrences >= *count as usize
            }),
            JointRule::PickedAtMost { candidates, count } => candidates.iter().all(|candidate| {
                picks.iter().flatten().filter(|i| *i == candidate).count() <= *count as usize
            }),
            JointRule::Total { min, max } => within(picks.iter().map(Vec::len).sum(), *min, *max),
        };
        if !satisfied {
            return Err("selection violates joint rule".into());
        }
    }
    Ok(())
}

fn check_allocation(
    total: &Quantity,
    buckets: &[Bucket],
    gates: &[Gate],
    amounts: &[Quantity],
) -> Result<(), String> {
    if buckets.len() != amounts.len() {
        return Err("expected one amount per bucket".into());
    }
    let mut sum = Quantity::zero();
    for (bucket, amount) in buckets.iter().zip(amounts) {
        if *amount < bucket.min || *amount > bucket.max {
            return Err("allocation outside bucket bounds".into());
        }
        sum.add(amount);
    }
    if sum != *total {
        return Err("allocation does not sum to total".into());
    }
    for gate in gates {
        let amount = amounts
            .get(gate.bucket as usize)
            .ok_or("invalid gated bucket")?;
        if *amount > Quantity::zero()
            && !gate.requires.iter().all(|(i, required)| {
                amounts
                    .get(*i as usize)
                    .is_some_and(|assigned| assigned >= required)
            })
        {
            return Err("allocation violates bucket requirements".into());
        }
    }
    Ok(())
}
pub fn subtract_selected_mana(
    mut remaining: BTreeMap<Color, Quantity>,
    spending: &BTreeMap<Color, Quantity>,
) -> Result<BTreeMap<Color, Quantity>, String> {
    for (color, amount) in spending {
        if *amount <= Quantity::zero() {
            return Err("mana spending entries must be positive".into());
        }
        let available = remaining.get_mut(color).ok_or("unavailable mana color")?;
        if *available < *amount {
            return Err("selected mana exceeds available pool".into());
        }
        available.subtract(amount);
    }
    Ok(remaining)
}

pub fn validate_selected_mana_cost(
    cost: &ManaCost,
    spending: &BTreeMap<Color, Quantity>,
) -> Result<(), String> {
    let mut total = Quantity::zero();
    for amount in spending.values() {
        total.add(amount);
    }
    let mut required = BTreeMap::<Color, Quantity>::new();
    let mut exact_total = cost.generic.clone();
    for color in &cost.colored {
        required.entry(*color).or_default().add(&Quantity::from(1));
        exact_total.add(&Quantity::from(1));
    }
    if total != exact_total
        || required
            .iter()
            .any(|(color, count)| spending.get(color).cloned().unwrap_or_default() < *count)
    {
        return Err("selected mana does not satisfy exact cost".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_spending_rejects_overpayment() {
        let cost = ManaCost {
            generic: 1.into(),
            colored: vec![],
        };
        assert!(
            validate_selected_mana_cost(&cost, &BTreeMap::from([(Color::G, 2.into())])).is_err()
        );
    }
}

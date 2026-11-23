//! Exact mana spending shared by the simulator policies and host search
//! rollouts: colored pips first, then generic in a fixed color order.
use mtg_contracts::{Color, ManaCost, Quantity};
use std::collections::BTreeMap;

pub fn spend(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Option<BTreeMap<Color, Quantity>> {
    let used = payment(cost, available)?;
    Some(
        PAYMENT_ORDER
            .into_iter()
            .zip(used)
            .filter(|(_, n)| *n > Quantity::zero())
            .collect(),
    )
}

const PAYMENT_ORDER: [Color; 6] = [Color::B, Color::G, Color::W, Color::U, Color::R, Color::C];

pub fn payment(cost: &ManaCost, available: &BTreeMap<Color, Quantity>) -> Option<[Quantity; 6]> {
    let mut used: [Quantity; 6] = Default::default();
    let one = Quantity::from(1);
    for c in &cost.colored {
        let index = match c {
            Color::B => 0,
            Color::G => 1,
            Color::W => 2,
            Color::U => 3,
            Color::R => 4,
            Color::C => 5,
        };
        let n = &mut used[index];
        n.add(&one);
        if &*n > available.get(c)? {
            return None;
        }
    }
    let mut rest = cost.generic.clone();
    for (c, already) in PAYMENT_ORDER.into_iter().zip(&mut used) {
        if rest == Quantity::zero() {
            break;
        }
        let Some(mut n) = available.get(&c).cloned() else {
            continue;
        };
        n.subtract(already);
        let n = n.min(rest.clone());
        if n > Quantity::zero() {
            rest.subtract(&n);
            already.add(&n);
        }
    }
    (rest == Quantity::zero()).then_some(used)
}

/// Plan against the complete fixed price as each printed hybrid pip is
/// selected. A life payment is available only on its printed symbol.
pub fn spend_response(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
    restricted_available: &[mtg_contracts::RestrictedMana],
) -> Option<mtg_core::PlayerResponse> {
    let mut combined = available.clone();
    for unit in restricted_available {
        combined.entry(unit.color).or_default().add(&unit.amount);
    }
    let mut spending = spend(cost, &combined)?;
    let mut restricted = vec![];
    for unit in restricted_available {
        if let Some(amount) = spending.get_mut(&unit.color) {
            let used = unit.amount.clone().min(amount.clone());
            if used > Quantity::zero() {
                amount.subtract(&used);
                restricted.push(mtg_contracts::RestrictedMana {
                    amount: used,
                    ..unit.clone()
                });
            }
        }
    }
    if restricted.is_empty() {
        return Some(mtg_core::PlayerResponse::SpendMana { spending });
    }
    spending.retain(|_, amount| *amount > Quantity::zero());
    Some(mtg_core::PlayerResponse::SpendManaAllocated {
        spending,
        restricted,
    })
}

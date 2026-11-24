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

pub fn spend_answer(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Option<mtg_core::Answer> {
    let unrestricted = spend(cost, available)?;
    Some(mtg_core::Answer::Pay {
        pay: mtg_core::PayAnswer::Spend { unrestricted },
    })
}

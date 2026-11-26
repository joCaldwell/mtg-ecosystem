//! Mana ability cells (CR 106, 605).
//!
//! A mana ability resolves immediately, without the stack and without
//! targets (CR 605.1a, 605.3b). Its production and costs are typed
//! [`ManaAbility`](crate::ManaAbility) fields; the rest of its printed effect
//! is a list of rider nodes, each admitted here under its own cell and
//! performed by the mana activation executor after the mana is added.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::{Definition, ManaAbility, ManaProduction};

fn controller(who: &Ref) -> bool {
    *who == Ref::controller(Context::Activated)
}

/// CR 605.1a, 605.3b: one rider a mana ability performs as it resolves.
/// Admitted riders read only the ability's source and controller, so they
/// need no target, event or announced value:
/// - CR 120.3a, 120.2b: "[this] deals N damage to you", N >= 1;
/// - CR 119.3: "you gain N life", N >= 1.
pub fn mana_rider(node: &Node) -> Result<Cell, KirError> {
    let positive = |amount: &Amount| {
        amount
            .as_fixed()
            .is_some_and(|value| *value >= Quantity::from(1))
    };
    match node {
        Node::Do(Verb::Damage {
            source: Ref::Source,
            to: DamageTo::Ref { who },
            amount,
        }) if controller(who) && positive(amount) => Ok(Cell::ManaRiderDamageController),
        Node::Do(Verb::GainLife {
            player: PlayerExpr::Is { who },
            amount,
        }) if controller(who) && positive(amount) => Ok(Cell::ManaRiderGainLifeController),
        _ => unaudited("mana ability rider"),
    }
}

/// The cells a printed mana ability occupies beyond the reviewed tap-only
/// fixed, any-color and commander-color surface (`PermanentManaV1`).
pub fn mana_ability_cells(d: &Definition, a: &ManaAbility) -> Result<BTreeSet<Cell>, KirError> {
    let mut cells = BTreeSet::new();
    match &a.production {
        ManaProduction::Fixed { .. }
        | ManaProduction::AnyColor
        | ManaProduction::CommanderColor => {}
        ManaProduction::Counted {
            zone,
            filter,
            factor,
            ..
        } => {
            if *factor < Quantity::from(1) {
                return unaudited("counted mana factor");
            }
            match zone {
                ZoneExpr::Battlefield | ZoneExpr::Graveyards => {
                    super::Walk::audited_filter(zone, filter)?;
                }
                ZoneExpr::Graveyard { of } if *of == PlayerExpr::you(Context::Activated) => {
                    super::Walk::audited_filter(&ZoneExpr::Graveyards, filter)?;
                }
                _ => return unaudited("counted mana zone"),
            }
            cells.insert(Cell::ManaCounted);
        }
        ManaProduction::Choose { .. } | ManaProduction::Bundles { .. } => {
            cells.insert(Cell::ManaChoose);
        }
        ManaProduction::LandsCouldProduce { .. } => {
            cells.insert(Cell::ManaLandsCouldProduce);
        }
    }
    if a.spend_restriction.is_some() {
        cells.insert(Cell::ManaSpendRestriction);
    }
    if a.costs.mana.is_some() {
        cells.insert(Cell::ManaAbilityManaCost);
    }
    if a.costs.pay_life.is_some() {
        cells.insert(Cell::ManaAbilityPayLife);
    }
    // CR 605.3a: the player chooses which of the source's mana abilities to
    // activate.
    if d.mana_abilities.len() > 1 {
        cells.insert(Cell::ManaAbilitySelection);
    }
    for rider in &a.riders {
        cells.insert(mana_rider(rider)?);
    }
    Ok(cells)
}

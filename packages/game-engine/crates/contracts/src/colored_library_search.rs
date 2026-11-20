//! CR 608.2d, 701.23b. A quality-constrained hidden search can find no card
//! for a slot. A multicolored card may fill exactly one matching slot.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::Color;
use std::collections::BTreeSet;

/// Ordered, distinct colored slots of a single library search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColoredLibrarySearch {
    slots: Vec<Color>,
}

/// An assigned slot; unassigned slots represent cards the player did not find.
/// Callers supply their own incarnation or scoped handle type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColoredSearchAssignment<T> {
    pub slot: Color,
    pub card: T,
}

impl ColoredLibrarySearch {
    pub fn new(slots: &[Color]) -> Result<Self, &'static str> {
        let distinct: BTreeSet<_> = slots.iter().copied().collect();
        if slots.is_empty() || slots.contains(&Color::C) || distinct.len() != slots.len() {
            return Err("search requires distinct colored slots");
        }
        Ok(Self {
            slots: slots.to_vec(),
        })
    }

    pub fn slots(&self) -> &[Color] {
        &self.slots
    }

    /// Validate the submitted partial allocation against current candidates.
    /// The caller verifies library membership and current card color together
    /// in `eligible`. The returned aggregate follows the printed slot order
    /// regardless of the order of the submitted assignments.
    pub fn found<'a, T: Ord>(
        &self,
        assignments: &'a [ColoredSearchAssignment<T>],
        mut eligible: impl FnMut(&T, Color) -> bool,
    ) -> Result<Vec<&'a T>, &'static str> {
        if assignments.len() > self.slots.len() {
            return Err("too many colored search assignments");
        }
        let mut occupied = vec![None; self.slots.len()];
        let mut cards = BTreeSet::new();
        for assignment in assignments {
            let index = self
                .slots
                .iter()
                .position(|color| *color == assignment.slot)
                .ok_or("foreign colored search slot")?;
            if occupied[index].is_some() || !cards.insert(&assignment.card) {
                return Err("duplicate colored search slot or card");
            }
            if !eligible(&assignment.card, assignment.slot) {
                return Err("ineligible colored search card");
            }
            occupied[index] = Some(&assignment.card);
        }
        Ok(occupied.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colored_search_preserves_partial_distinct_assignments() {
        let search = ColoredLibrarySearch::new(&[Color::R, Color::G, Color::W]).unwrap();
        let eligible = |id: &u8, slot| match *id {
            1 => matches!(slot, Color::R | Color::G),
            2 => slot == Color::R,
            _ => false,
        };
        let assign = |slot, card| ColoredSearchAssignment { slot, card };
        let selected = [assign(Color::G, 1), assign(Color::R, 2)];
        assert_eq!(search.found(&selected, eligible).unwrap(), [&2, &1]);
        assert!(search.found(&[], eligible).unwrap().is_empty());
        for bad in [
            vec![assign(Color::R, 1), assign(Color::G, 1)],
            vec![assign(Color::R, 1), assign(Color::R, 2)],
            vec![assign(Color::W, 1)],
            vec![assign(Color::U, 1)],
            vec![assign(Color::G, 3)],
        ] {
            assert!(search.found(&bad, eligible).is_err());
        }
        assert!(ColoredLibrarySearch::new(&[Color::C]).is_err());
        assert!(ColoredLibrarySearch::new(&[Color::R, Color::R]).is_err());
    }
}

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

use super::{
    CardKind, Definition, FunctioningZone, Instruction, SpellProgram, SpellResolution, SpellTiming,
    StaticEffect, strict_read,
};
use serde_json::{Value, json};

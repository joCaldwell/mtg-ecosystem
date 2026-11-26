use crate::{JointRule, Slot};
use std::collections::{BTreeMap, BTreeSet};

/// Find a legal assignment for mana-payment slots. Each object can cover one
/// slot, even when its colors make it a candidate in several.
pub fn distinct_payment_choices(
    slots: &[Slot],
    minimum: u32,
    desired: u32,
) -> Option<Vec<Vec<u32>>> {
    fn augment(
        index: usize,
        slots: &[Slot],
        owners: &mut BTreeMap<u32, usize>,
        seen: &mut BTreeSet<usize>,
    ) -> bool {
        seen.insert(index)
            && slots[index].allowed.iter().any(|&candidate| {
                if owners
                    .get(&candidate)
                    .copied()
                    .is_none_or(|other| augment(other, slots, owners, seen))
                {
                    owners.insert(candidate, index);
                    true
                } else {
                    false
                }
            })
    }

    let mut owners = BTreeMap::new();
    for (index, slot) in slots.iter().enumerate() {
        for _ in 0..slot.min {
            if !augment(index, slots, &mut owners, &mut BTreeSet::new()) {
                return None;
            }
        }
    }
    for (index, slot) in slots.iter().enumerate() {
        for _ in slot.min..slot.max {
            if owners.len() >= desired as usize
                || !augment(index, slots, &mut owners, &mut BTreeSet::new())
            {
                break;
            }
        }
    }
    if owners.len() < minimum as usize {
        return None;
    }
    let mut picks = vec![vec![]; slots.len()];
    for (object, slot) in owners {
        picks[slot].push(object);
    }
    Some(picks)
}

pub fn payment_choice_minimum(rules: &[JointRule]) -> u32 {
    rules
        .iter()
        .find_map(|rule| match rule {
            JointRule::Total { min, .. } => Some(*min),
            _ => None,
        })
        .unwrap_or_default()
}

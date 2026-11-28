//! Constraints on one hidden library search (CR 201.2b, 701.23a–b, h).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    /// A grouped search offers only legal extensions. Stopping is permitted
    /// even when there are matches (CR 701.23b). No card moves until the set
    /// is complete and the instruction can move the entire group together.
    pub(super) fn grouped_search_offer(
        &self,
        player: PlayerId,
        group: &SearchGroup,
        found: &[ObjectId],
        bounds: &mut SearchBounds,
    ) -> Result<(), String> {
        let mut trial = found.to_vec();
        let mut eligible = Vec::new();
        if found.len() < bounds.maximum {
            for &card in &bounds.candidates {
                if found.contains(&card) {
                    continue;
                }
                trial.push(card);
                if self.search_group_matches(player, group, &trial)? {
                    eligible.push(card);
                }
                trial.pop();
            }
        }
        bounds.maximum = usize::from(!eligible.is_empty());
        bounds.candidates = eligible;
        Ok(())
    }

    pub(super) fn answer_grouped_search(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        mut found: Vec<ObjectId>,
        chosen: Vec<ObjectId>,
        search: &SearchSpec,
        source_block: u32,
    ) -> Result<(), String> {
        let bounds = self.search_bounds(player, search)?;
        let stop = chosen.is_empty();
        found.extend(chosen);
        if !self.valid_search_answer(player, search, &bounds, &found)? {
            return Err("invalid grouped search answer".into());
        }
        if stop || found.len() == bounds.maximum {
            return self.complete_search(frame, player, found, source_block);
        }
        self.rules_mut().frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Search {
                player,
                source_block,
                found,
            },
        };
        Ok(())
    }

    pub(super) fn valid_search_answer(
        &self,
        player: PlayerId,
        search: &SearchSpec,
        bounds: &SearchBounds,
        found: &[ObjectId],
    ) -> Result<bool, String> {
        if !(bounds.minimum..=bounds.maximum).contains(&found.len())
            || !Self::subset(&bounds.candidates, found)
        {
            return Ok(false);
        }
        match &search.group {
            None => Ok(true),
            Some(group) => self.search_group_matches(player, group, found),
        }
    }

    fn search_group_matches(
        &self,
        player: PlayerId,
        group: &SearchGroup,
        found: &[ObjectId],
    ) -> Result<bool, String> {
        match group {
            SearchGroup::DifferentNames => {
                // Frozen catalog names are the canonical English Oracle
                // names. A split card has both names in the library (709.4a).
                let mut names = BTreeSet::new();
                for &card in found {
                    let definition = self.definition(card.card)?;
                    let second = definition
                        .back_face
                        .as_deref()
                        .filter(|_| definition.layout == FaceLayout::Split);
                    for name in std::iter::once(&definition.name).chain(second.map(|d| &d.name)) {
                        if name.is_empty() || !names.insert(name) {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
            SearchGroup::Separate { slots } => {
                let eligible = found
                    .iter()
                    .map(|&card| {
                        let facts = self.object_facts(self.current(card)?);
                        Ok(slots
                            .iter()
                            .map(|slot| crate::query::matches_pred(&facts, player, slot).is_yes())
                            .collect::<Vec<_>>())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let mut allocated = vec![None; slots.len()];
                for card in 0..found.len() {
                    let mut visited = vec![false; slots.len()];
                    if !allocate(card, &eligible, &mut allocated, &mut visited) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }
}

/// Extend the matching, moving a flexible earlier card to another slot if
/// necessary. Only existence of an assignment matters, never its order.
fn allocate(
    card: usize,
    eligible: &[Vec<bool>],
    allocated: &mut [Option<usize>],
    visited: &mut [bool],
) -> bool {
    for (slot, &matches) in eligible[card].iter().enumerate() {
        if !matches || visited[slot] {
            continue;
        }
        visited[slot] = true;
        if allocated[slot].is_none_or(|other| allocate(other, eligible, allocated, visited)) {
            allocated[slot] = Some(card);
            return true;
        }
    }
    false
}

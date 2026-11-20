//! CR 601.2b–c and 700.2: a mode's target occurrences belong to that printed
//! mode, including when two chosen modes select the same physical target.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeCount {
    OneOrMore,
    Exactly(u32),
}

impl ModeCount {
    pub fn selected(self, offered: usize, indices: &[u32]) -> Result<Vec<u32>, String> {
        if indices.is_empty()
            || indices.len() > offered
            || matches!(self, Self::Exactly(n) if usize::try_from(n).ok()!=Some(indices.len()))
        {
            return Err("wrong number of chosen modes".into());
        }
        let mut chosen = indices.to_vec();
        chosen.sort_unstable();
        if chosen.windows(2).any(|pair| pair[0] == pair[1])
            || chosen
                .last()
                .is_none_or(|index| usize::try_from(*index).map_or(true, |i| i >= offered))
        {
            return Err("unknown or repeated printed mode".into());
        }
        Ok(chosen)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeTargetOccurrence<'a> {
    pub printed_mode: u32,
    pub target: &'a TargetSpec,
}

pub fn target_occurrences<'a>(
    root: &'a SpellProgram,
    count: ModeCount,
    chosen: &[u32],
) -> Result<Vec<ModeTargetOccurrence<'a>>, String> {
    let selected = count.selected(root.modes.len(), chosen)?;
    let mut occurrences = vec![];
    for mode in selected {
        for (index, target) in root.modes[mode as usize].program.targets.iter().enumerate() {
            if usize::try_from(target.slot).ok() != Some(index) {
                return Err("noncontiguous printed target occurrences".into());
            }
            if let TargetDomain::PlayerGraveyardCards { owner_slot, .. } = target.domain {
                if usize::try_from(owner_slot)
                    .ok()
                    .and_then(|s| root.modes[mode as usize].program.targets.get(s))
                    .is_none_or(|owner| {
                        owner.slot >= target.slot || owner.domain != TargetDomain::AnyPlayer
                    })
                {
                    return Err(
                        "dependent cards lack an earlier player target in the same mode".into(),
                    );
                }
            }
            occurrences.push(ModeTargetOccurrence {
                printed_mode: mode,
                target,
            });
        }
    }
    Ok(occurrences)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_mode_count_preserves_print_order_and_distinct_dependent_occurrences() {
        let mut root = crate::graveyard_shuffle::witness().spell.unwrap();
        let nested = root.clone();
        root.targets.clear();
        root.instructions.clear();
        root.modes = vec![
            SpellMode {
                program: nested.clone(),
            },
            SpellMode {
                program: nested.clone(),
            },
            SpellMode { program: nested },
        ];
        let choose = ModeCount::Exactly(2);
        let slots = target_occurrences(&root, choose, &[2, 0]).unwrap();
        assert_eq!(
            slots
                .iter()
                .map(|s| (s.printed_mode, s.target.slot))
                .collect::<Vec<_>>(),
            [(0, 0), (0, 1), (2, 0), (2, 1)]
        );
        for invalid in [&[][..], &[0], &[0, 0], &[0, 3], &[0, 1, 2]] {
            assert!(target_occurrences(&root, choose, invalid).is_err());
        }
        root.modes[2].program.targets[1].domain = TargetDomain::PlayerGraveyardCards {
            owner_slot: 1,
            maximum: 2,
        };
        assert!(target_occurrences(&root, choose, &[0, 2]).is_err());
    }
}

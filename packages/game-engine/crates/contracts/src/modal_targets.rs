//! CR 601.2b–c and 700.2: a mode's target occurrences belong to that printed
//! mode, including when two chosen modes select the same physical target.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeCount {
    OneOrMore,
    Exactly(u32),
    ExactlyWithRepetition(u32),
}

impl ModeCount {
    pub fn selected(self, offered: usize, indices: &[u32]) -> Result<Vec<u32>, String> {
        if indices.is_empty()
            || (indices.len() > offered && !matches!(self, Self::ExactlyWithRepetition(_)))
            || matches!(self, Self::Exactly(n) | Self::ExactlyWithRepetition(n) if usize::try_from(n).ok()!=Some(indices.len()))
        {
            return Err("wrong number of chosen modes".into());
        }
        let mut chosen = indices.to_vec();
        chosen.sort_unstable();
        if (!matches!(self, Self::ExactlyWithRepetition(_))
            && chosen.windows(2).any(|pair| pair[0] == pair[1]))
            || chosen
                .last()
                .is_none_or(|index| usize::try_from(*index).map_or(true, |i| i >= offered))
        {
            return Err("unknown or repeated printed mode".into());
        }
        Ok(chosen)
    }
}

impl ModeCount {
    pub fn for_root(root: &SpellProgram) -> Option<Self> {
        match root.resolution {
            SpellResolution::ChooseModes { exactly } => Some(Self::Exactly(exactly)),
            SpellResolution::ChooseRepeatedModes { exactly } => {
                Some(Self::ExactlyWithRepetition(exactly))
            }
            SpellResolution::OneOrMoreModes => Some(Self::OneOrMore),
            _ => None,
        }
    }
}

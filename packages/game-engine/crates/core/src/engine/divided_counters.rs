//! CR 601.2c-d and 608.2b. The selected objects fix the unique division of
//! two counters: two on one creature, or one each on two distinct creatures.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CounterDivision {
    pub count: Quantity,
    pub minimum_targets: u32,
    pub maximum_targets: u32,
}

impl CounterDivision {
    pub(super) fn offered_by(program: &SpellProgram) -> Option<Self> {
        let [
            Instruction::DistributePlusOneCounters {
                count,
                minimum_targets,
                maximum_targets,
                ..
            },
        ] = program.instructions.as_slice()
        else {
            return None;
        };
        Some(Self {
            count: count.clone(),
            minimum_targets: *minimum_targets,
            maximum_targets: *maximum_targets,
        })
    }

    fn allows_count(&self, count: usize) -> bool {
        self.count == Quantity::from(2)
            && self.minimum_targets == 1
            && self.maximum_targets == 2
            && matches!(count, 1 | 2)
    }

    fn accepts(&self, objects: &[ObjectId]) -> bool {
        self.allows_count(objects.len()) && !matches!(objects, [first, second] if first == second)
    }
}

impl Game {
    pub(super) fn object_target_number_valid(program: &SpellProgram, count: usize) -> bool {
        CounterDivision::offered_by(program).map_or(count == program.targets.len(), |offer| {
            offer.allows_count(count)
        })
    }

    pub(super) fn object_target_count_valid(program: &SpellProgram, objects: &[ObjectId]) -> bool {
        CounterDivision::offered_by(program)
            .map_or(objects.len() == program.targets.len(), |offer| {
                offer.accepts(objects)
            })
    }

    pub(super) fn announced_target_count_valid(
        &self,
        program: &SpellProgram,
        entry: &StackInstance,
    ) -> bool {
        if let Some(offer) = CounterDivision::offered_by(program) {
            entry.kind.is_spell()
                && self.bound_target_count(entry) == entry.targets.len()
                && offer.accepts(&entry.targets)
        } else {
            self.bound_target_count(entry) == program.targets.len()
        }
    }

    pub(super) fn allocated_counter_targets(
        &self,
        frame: &Resolving,
    ) -> Result<Vec<(ObjectId, Quantity)>, String> {
        let program = self.program_for(&frame.spell)?;
        let offer =
            CounterDivision::offered_by(program).ok_or("missing divided counter program")?;
        if !self.announced_target_count_valid(program, &frame.spell)
            || frame.legal_targets.len() != frame.spell.targets.len()
        {
            return Err("divided counter target binding invariant".into());
        }
        let amount = if frame.spell.targets.len() == 1 {
            offer.count
        } else {
            Quantity::from(1)
        };
        // Target loss never reallocates counters from the announced division.
        Ok(frame
            .spell
            .targets
            .iter()
            .zip(&frame.legal_targets)
            .filter(|(_, legal)| **legal)
            .map(|(&target, _)| (target, amount.clone()))
            .collect())
    }
}

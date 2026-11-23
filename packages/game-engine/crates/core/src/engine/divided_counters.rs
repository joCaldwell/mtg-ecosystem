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

impl RulesTxn {
    pub(super) fn object_target_number_valid(program: &SpellProgram, count: usize) -> bool {
        count == program.targets.len()
    }

    pub(super) fn object_target_count_valid(program: &SpellProgram, objects: &[ObjectId]) -> bool {
        objects.len() == program.targets.len()
    }

    pub(super) fn announced_target_count_valid(
        &self,
        program: &SpellProgram,
        entry: &StackInstance,
    ) -> bool {
        self.bound_target_count(entry) == program.targets.len()
    }
}

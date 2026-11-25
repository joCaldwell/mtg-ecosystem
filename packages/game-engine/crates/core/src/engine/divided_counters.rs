//! CR 601.2c-d and 608.2b. The selected objects fix the unique division of
//! two counters: two on one creature, or one each on two distinct creatures.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(super) fn announced_target_count_valid(
        &self,
        program: &SpellProgram,
        entry: &StackInstance,
    ) -> bool {
        let count = self.bound_target_count(entry);
        if program.optional_target {
            return count <= program.targets.len()
                && entry.targets.iter().collect::<BTreeSet<_>>().len() == count;
        }
        count == program.targets.len()
    }
}

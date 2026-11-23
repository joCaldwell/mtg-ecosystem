//! CR 111.13, 601.2, 608.2h, 702.144, 704.5d/e, 707.9b/f, 707.10/10f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use tokens::TokenTemplate;

impl RulesTxn {
    pub(crate) fn is_spell_copy(&self, entry: &StackInstance) -> bool {
        entry.kind.is_spell()
            && entry.source.incarnation == 0
            && self.state.rules.tokens.get(&entry.source.card).is_some_and(|identity|
                matches!(identity.spec, TokenTemplate::SpellCopy { stack_id, .. } if stack_id == entry.id))
    }

    pub(super) fn stack_entry_complete(&self, entry: &StackInstance) -> bool {
        entry.paid.as_ref() == Some(&entry.total_cost) || self.is_spell_copy(entry)
    }
}

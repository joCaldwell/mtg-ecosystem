//! CR 400.7 and 707.2 (2026-09-25): every incarnation's copiable values are
//! its immutable physical (or generated token) definition.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    /// Base characteristics and intrinsic abilities of this exact incarnation,
    /// including after departure. This lookup alone does not establish presence
    /// or authorize an action. The observer/stack/event owns its ObjectId.
    pub(crate) fn copiable_definition(
        &self,
        object: impl std::borrow::Borrow<ObjectId>,
    ) -> Result<&Definition, String> {
        Ok(self.definition(object.borrow().card)?)
    }
}

//! CR 400.7 and 707.2 (2026-09-25): every incarnation's copiable values are
//! its immutable physical (or generated token) definition and, for two-part
//! cards, the face chosen for that incarnation (CR 712.8a,f, 715.3b) or the
//! face a transforming permanent currently has up (CR 712.8d-e, 712.18).
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
        let id = object.borrow();
        let definition = self.definition(id.card)?;
        // CR 712.18: a transformed permanent is the same object with its
        // other face up. Only the current incarnation has transformed.
        let face = match self.state.rules.objects.get(&id.card) {
            Some(live) if definition.transforms() && live.id == *id => live.face(),
            _ => id.face,
        };
        definition
            .face(face)
            .ok_or_else(|| "card has no selected face".into())
    }

    /// CR 113.7a, 712.18: an activated or triggered ability on the stack is
    /// the ability of the face it was activated or triggered from, even after
    /// its source transforms; a spell has the characteristics of its object.
    pub(crate) fn stack_definition(&self, entry: &StackInstance) -> Result<&Definition, String> {
        self.face_definition(entry.source.card, entry.program_face())
    }

    pub(super) fn face_definition(
        &self,
        card: CardId,
        face: CardFace,
    ) -> Result<&Definition, String> {
        self.definition(card)?
            .face(face)
            .ok_or_else(|| "card has no selected face".into())
    }
}

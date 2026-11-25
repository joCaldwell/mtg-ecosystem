//! CR 400.7 and 707.2 (2026-09-25): every incarnation's copiable values are
//! its immutable physical (or generated token) definition and, for modal
//! cards, the face chosen for that incarnation (CR 712.8a,f).
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
        self.face_definition(id.card, id.face)
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

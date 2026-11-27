//! CR 400.7 and 707.2 (2026-09-25): every incarnation's copiable values are
//! its immutable physical (or generated token) definition and, for two-part
//! cards, the face chosen for that incarnation (CR 712.8a,f, 715.3b) or the
//! face a transforming permanent currently has up (CR 712.8d-e, 712.18).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    /// Printed face for this incarnation, including after departure. A split
    /// card off the stack combines both halves in `copiable_characteristics`.
    /// This lookup alone does not establish presence
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
            // CR 708.2, 707.2: a face-down object's copiable values are the
            // face-down characteristics until it is turned face up.
            Some(live) if live.face_down && live.id == *id => {
                return Ok(face_down::face_down_definition());
            }
            Some(live) if definition.transforms() && live.id == *id => live.face(),
            _ => id.face,
        };
        definition
            .face(face)
            .ok_or_else(|| "card has no selected face".into())
    }

    /// CR 709.3b–4: an identified split card has both halves' copiable
    /// characteristics everywhere except the stack.
    pub(super) fn copiable_characteristics(
        &self,
        object: &Object,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        let definition = self.copiable_definition(object.id)?;
        Ok(
            if object.zone != Zone::Stack && definition.layout == FaceLayout::Split {
                definition.card_characteristics()
            } else {
                std::borrow::Cow::Borrowed(&definition.characteristics)
            },
        )
    }

    /// CR 113.7a, 712.18: an activated or triggered ability on the stack is
    /// the ability of the face it was activated or triggered from, even after
    /// its source transforms; a spell has the characteristics of its object.
    pub(crate) fn stack_definition(&self, entry: &StackInstance) -> Result<&Definition, String> {
        if entry.kind.is_spell() && self.is_face_down(entry.source) {
            return Ok(face_down::face_down_definition());
        }
        self.face_definition(entry.source.card, entry.program_face())
    }

    /// The definition captured facts were taken from: the face-down
    /// characteristics, or the face a transforming permanent had up.
    pub(crate) fn fact_definition(
        &self,
        source: ObjectId,
        transforms: u32,
        face_down: bool,
    ) -> Result<&Definition, String> {
        if face_down {
            return Ok(face_down::face_down_definition());
        }
        self.face_definition(source.card, face_after(source.face, transforms))
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

//! Token cells (CR 111).
use super::*;

impl Walk<'_, '_> {
    /// CR 111.1-111.4, 111.10: "you" create a positive fixed number of
    /// tokens of one exact recipe.
    pub(super) fn audit_tokens(
        controller: &PlayerExpr,
        count: &Amount,
        token: &TokenSpec,
    ) -> Result<Cell, KirError> {
        let PlayerExpr::Is {
            who: Ref::Controller { .. },
        } = controller
        else {
            return unaudited("verb");
        };
        Self::fixed_at_least(count, 1, "token count")?;
        if count.as_fixed().and_then(Quantity::to_u32).is_none() {
            return kind("token count above the supported range");
        }
        Ok(match token {
            TokenSpec::Predefined { .. } => Cell::PredefinedTokens,
            TokenSpec::Creature { spec } if !spec.additional_types.is_empty() => {
                Cell::MultiTypeCreatureTokens
            }
            TokenSpec::Creature { .. } => Cell::CreatureTokens,
        })
    }
}

/// CR 111.10: predefined artifact tokens; CR 111.3-111.4: creature tokens,
/// with the reviewed multi-type and hexproof/shroud surfaces.
pub(super) fn capabilities(token: &TokenSpec, out: &mut BTreeSet<Capability>) {
    match token {
        TokenSpec::Predefined { .. } => {
            out.insert(Capability::PredefinedArtifactTokensV1);
        }
        TokenSpec::Creature { spec } => {
            out.insert(Capability::CreatureTokensV1);
            if !spec.additional_types.is_empty() {
                out.insert(Capability::MultiTypeCreatureTokensV1);
            }
            if spec
                .keywords
                .iter()
                .any(|k| matches!(k, KeywordAbility::Hexproof | KeywordAbility::Shroud))
            {
                out.insert(Capability::AttachmentsV1);
            }
        }
    }
}

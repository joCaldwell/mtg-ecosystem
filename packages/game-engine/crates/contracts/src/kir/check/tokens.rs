//! Token cells (CR 111).
use super::*;

impl Walk<'_, '_> {
    /// CR 111.1-111.4, 111.10: the ability's controller, or the current or
    /// last-known controller of an announced permanent target, creates tokens
    /// of one exact recipe. CR 107.3, 608.2h: the count is a positive fixed
    /// number or a quantity the shared evaluator reads once as the step is
    /// performed (`audit_evaluated` has checked it); zero creates nothing.
    /// A positive fixed number of plain creature tokens for "you" keeps its
    /// reviewed cell and tiers.
    pub(super) fn audit_evaluated_tokens(
        &self,
        controller: &PlayerExpr,
        count: &Amount,
        token: &TokenSpec,
    ) -> Result<Cell, KirError> {
        let you = matches!(
            controller,
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            }
        );
        match controller {
            _ if you => {}
            PlayerExpr::ControllerOf {
                object: Ref::Target { slot },
            } if self.target_has_facts(*slot) => {}
            _ => return unaudited("token controller"),
        }
        if count.as_fixed().is_some() {
            Self::fixed_at_least(count, 1, "token count")?;
            if count.as_fixed().and_then(Quantity::to_u32).is_none() {
                return kind("token count above the supported range");
            }
            if you && matches!(token, TokenSpec::Creature { spec } if spec.additional_types.is_empty())
            {
                return Ok(Cell::CreatureTokens);
            }
        }
        Ok(Cell::Evaluated(EvaluatedVerb::CreateTokens))
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

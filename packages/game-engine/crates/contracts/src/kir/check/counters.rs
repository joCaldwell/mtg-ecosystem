//! Counter cells (CR 122).
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_player_counters(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        if !player.is_you() && *player != PlayerExpr::EachOpponent {
            return unaudited("player counter recipient");
        }
        Self::fixed_at_least(amount, 1, "player counter amount")?;
        Ok(Cell::PlayerCounters)
    }
}

/// CR 122.1.
pub(super) fn capabilities(counter: &CounterKind, amount: &Amount, out: &mut BTreeSet<Capability>) {
    match counter {
        CounterKind::PlusOnePlusOne => {
            out.insert(Capability::PlusOneCountersV1);
        }
        CounterKind::MinusOneMinusOne => {
            out.insert(Capability::MinusOneCountersV1);
        }
        CounterKind::Keyword { keyword } => {
            out.insert(Capability::KeywordCountersV1);
            out.extend(crate::intrinsic_keyword_capability(*keyword));
        }
        CounterKind::Loyalty => {
            out.insert(Capability::PlaneswalkerV1);
        }
        CounterKind::Stun => {
            out.insert(Capability::StunCountersV1);
        }
        CounterKind::Lore => {
            out.insert(Capability::SagaV1);
        }
        CounterKind::Named { .. } => {
            out.insert(Capability::NamedCountersV1);
        }
    }
    if amount.is_announced_x() {
        out.insert(Capability::AnnouncedXEffectsV1);
    }
}

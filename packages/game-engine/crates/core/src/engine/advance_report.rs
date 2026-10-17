//! Trusted host input-processing evidence, local to one advance call. It is not
//! semantic State, a player projection, or proof that a chosen action resolved.
use super::*;

/// Zero-based ordinals in the accepted decision/concession journals. Response
/// contents are deliberately absent; these identities belong to trusted hosts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoreInputIdentity {
    Decision {
        journal_ordinal: u64,
        decision_id: u64,
        player: PlayerId,
    },
    Concession {
        concession_ordinal: u64,
        player: PlayerId,
    },
}
/// Classifies the committed root rule step, not the eventual spell/ability
/// outcome. A RulesStep removal means only "consumed by rules"; departure may
/// discard an accepted choice before that choice's ordinary processing step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputProcessingCause {
    RulesStep,
    DepartureStep,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputProcessingFact {
    pub identity: CoreInputIdentity,
    pub cause: InputProcessingCause,
    /// Execution boundary AFTER the step was committed atomically.
    pub boundary: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedInputCounts {
    pub decisions: u64,
    pub concessions: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvanceReport {
    pub advance: Advance,
    /// In committed-step order. Within one step, the removed decision precedes
    /// the processed concession. No fact is retained between advance calls.
    pub processed: Vec<InputProcessingFact>,
}
impl Game {
    /// Cheap trusted journal lengths; after a successful submit/concede, its
    /// zero-based ordinal is the corresponding count minus one.
    pub fn accepted_input_counts(&self) -> Result<AcceptedInputCounts, String> {
        Ok(AcceptedInputCounts {
            decisions: u64::try_from(self.state.journal.len())
                .map_err(|_| "decision journal ordinal resource limit")?,
            concessions: u64::try_from(self.state.external_inputs.len())
                .map_err(|_| "concession journal ordinal resource limit")?,
        })
    }
    /// Same rules, atomicity and Budget units as advance. Facts describe only
    /// accepted inputs actually removed by committed steps during THIS call.
    /// A rolled-back step emits none, even if it removed inputs in its staging
    /// branch. Earlier committed facts survive a later Fault return.
    pub fn advance_with_report(&mut self, budget: Budget) -> AdvanceReport {
        let mut processed = vec![];
        let advance = self.advance_internal::<true>(budget, &mut processed);
        AdvanceReport { advance, processed }
    }
}

pub(super) struct StepInputCapture {
    decision: Option<CoreInputIdentity>,
    concession: Option<CoreInputIdentity>,
    cause: InputProcessingCause,
}
impl StepInputCapture {
    pub(super) fn capture(game: &Game) -> Self {
        // Restored/admitted states bind pending to journal.last(). Do not
        // fabricate an ordinal for malformed private test/domain states.
        let decision = game.state.pending.as_ref().and_then(|pending| {
            (game.state.journal.last() == Some(pending)).then_some(())?;
            Some(CoreInputIdentity::Decision {
                journal_ordinal: u64::try_from(game.state.journal.len().checked_sub(1)?).ok()?,
                decision_id: pending.decision_id,
                player: pending.player,
            })
        });
        // The core step first removes this queued front and applies departure.
        // Every player can concede at most once; original journal order is
        // retained even when several concessions share a decision/boundary.
        let front = game.state.pending_concessions.first().copied();
        let concession = front.and_then(|player| {
            Some(CoreInputIdentity::Concession {
                concession_ordinal: u64::try_from(
                    game.state
                        .external_inputs
                        .iter()
                        .position(|input| input.player == player)?,
                )
                .ok()?,
                player,
            })
        });
        Self {
            decision,
            concession,
            cause: if front.is_some() {
                InputProcessingCause::DepartureStep
            } else {
                InputProcessingCause::RulesStep
            },
        }
    }
    /// Called only after step, boundary increment, and visibility refresh all
    /// succeeded and the staging State was installed. CR800.4 departure
    /// revalidation can clear any nested pending choice; inspect the committed
    /// result rather than infer processing from an ordinary next request.
    pub(super) fn committed(self, game: &Game, out: &mut Vec<InputProcessingFact>) {
        if game.state.pending.is_none()
            && let Some(identity) = self.decision
        {
            out.push(InputProcessingFact {
                identity,
                cause: self.cause,
                boundary: game.state.rule_steps,
            });
        }
        if let Some(identity) = self.concession {
            out.push(InputProcessingFact {
                identity,
                cause: self.cause,
                boundary: game.state.rule_steps,
            });
        }
    }
}

#[cfg(test)]
#[path = "advance_report_tests.rs"]
mod tests;

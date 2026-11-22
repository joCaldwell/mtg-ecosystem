//! The advance loop: one atomic rule step at a time, within a budget.
use crate::engine::*;

impl Game {
    pub(crate) fn advance_internal<const REPORT: bool, const COALESCE: bool>(
        &mut self,
        mut budget: Budget,
        processed: &mut Vec<InputProcessingFact>,
    ) -> Advance {
        loop {
            if let Frame::Finished { winner } = self.state.rules.frame {
                return Advance::Finished { winner };
            }
            if let Frame::Fault { reason } = &self.state.rules.frame {
                return Advance::Fault(reason.clone());
            }
            if matches!(self.state.rules.frame, Frame::Draw) {
                return Advance::Draw;
            }
            if self.state.rules.pending.is_none() && self.state.rules.pending_concessions.is_empty()
            {
                // At most one decision is pending; only a live player can owe it.
                match self.pending_decision() {
                    Ok(Some(request)) if self.is_live(request.player()) => {
                        return Advance::AwaitingDecision(request);
                    }
                    Ok(_) => {}
                    Err(reason) => return self.enter_fault(RuleError::invariant(reason)),
                }
            }
            if !budget.consume() {
                return Advance::Suspended {
                    required_minimum: 1,
                };
            }
            // A rules step commits atomically. Resource/invariant failures retain
            // its pre-step state and become faults, never wins/losses/draws.
            let input_report = REPORT.then(|| advance_report::StepInputCapture::capture(self));
            let stepped = self.commit_rule_step(COALESCE);
            if let Err(error) = stepped {
                return self.enter_fault(error);
            }
            if let Some(report) = input_report {
                report.committed(self, processed);
            }
        }
    }
    /// Accepts a validated answer: journals it and leaves it pending for the
    /// next rule step. A receipt issued for another executor or an earlier
    /// state is illegal input and changes nothing.
    pub(in crate::engine) fn accept(
        &mut self,
        receipt: ValidatedResponse,
    ) -> Result<(), RuleError> {
        if !Arc::ptr_eq(&receipt.executor, &self.executor)
            || !Arc::ptr_eq(&receipt.state_token, &self.state_token)
            || receipt.revision != self.state.rules.revision
            || self.state.rules.pending.is_some()
        {
            return Err(RuleError::illegal("stale or foreign validation receipt"));
        }
        self.commit_input(receipt.input)
    }
    /// A failed step is never a rules outcome: the game keeps its pre-step
    /// state and stops at a fault, typed when a capacity was exhausted.
    pub(in crate::engine) fn enter_fault(&mut self, error: RuleError) -> Advance {
        self.rules_mut().frames.clear();
        self.rules_mut().typed_fault = error.fault().cloned();
        let reason = String::from(error);
        self.rules_mut().frame = Frame::Fault {
            reason: reason.clone(),
        };
        self.state_token = Arc::new(());
        Advance::Fault(reason)
    }
}

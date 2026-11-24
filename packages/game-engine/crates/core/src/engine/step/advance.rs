//! The advance loop: one atomic rule step at a time, within a budget.
use crate::engine::*;

impl Game {
    pub(crate) fn advance_internal<const REPORT: bool, const COALESCE: bool>(
        &mut self,
        mut budget: Budget,
        processed: &mut Vec<InputProcessingFact>,
    ) -> Advance {
        // Check before even looking up a pending decision: that and retention
        // metering traverse saved frames. Each subsequent step retires its own
        // links before installing its successor.
        if (!self.state.rules.frames.is_empty() || self.state.rules.frame.parent().is_some())
            && let Err(error) = self.validate_stack_links()
        {
            return self.enter_fault(error);
        }
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
                    Ok(Some(request)) if self.is_live(request.player) => {
                        return Advance::AwaitingDecision(request);
                    }
                    Ok(_) => {}
                    Err(reason) => return self.enter_fault(RuleError::invariant(reason)),
                }
            }
            // Backpressure at a rule boundary. One atomic step can append its
            // bounded event group before the next delivery opportunity.
            if self.stream_buffer.len() >= 2048 || !budget.consume() {
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
}

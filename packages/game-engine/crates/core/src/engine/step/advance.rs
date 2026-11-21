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
            let stepped = self.transact(|tx| {
                tx.in_rule_step = COALESCE;
                tx.check_history_capacity()?;
                tx.step()?;
                tx.count_rule_step()
            });
            if let Err(error) = stepped {
                return self.enter_fault(error);
            }
            if let Some(report) = input_report {
                report.committed(self, processed);
            }
        }
    }
    /// Counts the rule step this transaction executed.
    fn count_rule_step(&mut self) -> Result<(), RuleError> {
        let rules = &mut self.state.rules;
        rules.rule_steps = rules
            .rule_steps
            .checked_add(1)
            .ok_or_else(|| RuleError::resource("execution boundary resource limit"))?;
        Ok(())
    }
    /// A failed step is never a rules outcome: the game keeps its pre-step
    /// state and stops at a fault, typed when a capacity was exhausted.
    pub(in crate::engine) fn enter_fault(&mut self, error: RuleError) -> Advance {
        self.state.rules.typed_fault = error.fault().cloned();
        let reason = String::from(error);
        self.state.rules.frame = Frame::Fault {
            reason: reason.clone(),
        };
        self.state_token = Arc::new(());
        Advance::Fault(reason)
    }
}

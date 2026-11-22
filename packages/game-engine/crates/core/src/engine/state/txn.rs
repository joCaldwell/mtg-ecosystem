//! The transaction every change to a game commits through.
//!
//! A change runs on a staged copy that shares the game's tables and copies
//! only what it writes. `transact` then publishes the staged state to all
//! four viewers, checks the retained-history capacity and, only if all of
//! that succeeded, replaces the game's state and issues a new state token.
//! A failed change leaves the game exactly as it was. With
//! `debug_assertions`, a transaction that starts from a valid state must
//! also commit one (`validate_state`). Which tables changed
//! is read from table identity: a written table or object chunk is a new
//! allocation, so publication needs no separate dirty set.
use crate::engine::*;
use std::ops::Deref;

mod queries;

testkit_fields! {
/// A committed executor only lends its rules for reads. Rule routines receive
/// an owned, copy-on-write stage. Only this module installs its changes.
pub struct Game {
    committed: RulesTxn,
}

}
impl Deref for Game {
    type Target = RulesTxn;
    fn deref(&self) -> &RulesTxn {
        &self.committed
    }
}
// White-box witnesses construct and corrupt otherwise unreachable states.
#[cfg(any(test, feature = "testkit"))]
impl std::ops::DerefMut for Game {
    fn deref_mut(&mut self) -> &mut RulesTxn {
        self.committed.published = None;
        &mut self.committed
    }
}

impl Game {
    pub(in crate::engine) fn from_initial(committed: RulesTxn) -> Self {
        Self { committed }
    }
    pub fn new(prepared: Prepared) -> Self {
        Self::from_initial(RulesTxn::new(prepared))
    }
    pub fn with_trace(prepared: Prepared, mode: TraceMode) -> Self {
        Self::from_initial(RulesTxn::with_trace(prepared, mode))
    }
    pub fn fork(&self) -> Self {
        Self::from_initial(self.committed.fork())
    }
    /// A failed step is never a rules outcome: the game keeps its pre-step
    /// state and stops at a fault, typed when a capacity was exhausted.
    pub(in crate::engine) fn enter_fault(&mut self, error: RuleError) -> Advance {
        self.committed.published = None;
        self.committed.kernel_leaf = None;
        self.committed.rules_mut().frames.clear();
        self.committed.rules_mut().typed_fault = error.fault().cloned();
        let reason = String::from(error);
        self.committed.rules_mut().frame = Frame::Fault {
            reason: reason.clone(),
        };
        self.committed.state_token = Arc::new(());
        Advance::Fault(reason)
    }
}

impl RulesTxn {
    fn execute_step(&mut self, coalesce: bool) -> Result<(), RuleError> {
        self.in_rule_step = coalesce;
        self.step()?;
        let rules = self.rules_mut();
        rules.rule_steps = rules
            .rule_steps
            .checked_add(1)
            .ok_or_else(|| RuleError::resource("execution boundary resource limit"))?;
        Ok(())
    }

    fn accept_input(&mut self, input: AcceptedInput) -> Result<(), RuleError> {
        let rules = self.rules_mut();
        rules.revision = rules
            .revision
            .checked_add(1)
            .ok_or_else(|| RuleError::resource("structural revision resource limit"))?;
        rules.retained_auxiliary_charge = rules
            .retained_auxiliary_charge
            .saturating_add(stored_history::payload_charge(&input));
        rules.pending = Some(input.clone());
        self.state.audit.journal.push(input);
        Ok(())
    }
}

impl Game {
    /// Runs one rule change atomically: stage, change, publish, check the
    /// history capacity, commit. On failure nothing is committed, and a
    /// capacity fault the change recorded travels with the error.
    pub(in crate::engine) fn commit_rule_step(&mut self, coalesce: bool) -> Result<(), RuleError> {
        self.transact(true, |tx| tx.execute_step(coalesce))
    }

    /// Accepts an input atomically. Retaining an input is charged to history
    /// but never refused here: the next rule step's capacity check faults
    /// instead, so an accepted input is always journaled.
    pub(in crate::engine) fn commit_input(
        &mut self,
        input: AcceptedInput,
    ) -> Result<(), RuleError> {
        self.transact(false, |tx| tx.accept_input(input))
    }

    pub(in crate::engine) fn commit_concession(
        &mut self,
        player: PlayerId,
        revision: u64,
    ) -> Result<(), RuleError> {
        self.transact(false, |next| {
            let input = ExternalInput {
                after_decisions: u64::try_from(next.state.audit.journal.len())
                    .map_err(|_| RuleError::resource("input count resource limit"))?,
                boundary: next.state.rules.rule_steps,
                player,
            };
            let charge = stored_history::payload_charge(&input);
            next.state.audit.external_inputs.push(input);
            let rules = next.rules_mut();
            rules.retained_auxiliary_charge =
                rules.retained_auxiliary_charge.saturating_add(charge);
            rules.pending_concessions.push(player);
            rules.revision = revision;
            Ok(())
        })
    }

    fn transact<T>(
        &mut self,
        metered: bool,
        change: impl FnOnce(&mut RulesTxn) -> Result<T, RuleError>,
    ) -> Result<T, RuleError> {
        // A transaction preserves the state invariant. A state that did not
        // satisfy it beforehand (a white-box test's synthetic board) proves
        // nothing either way.
        #[cfg(all(debug_assertions, any(test, feature = "testkit")))]
        let valid_before = self.validate_state().is_ok();
        #[cfg(all(debug_assertions, not(any(test, feature = "testkit"))))]
        let valid_before = true;
        let mut staged = self.committed.fork();
        let outcome = change(&mut staged).and_then(|value| {
            staged.refresh_visibility()?;
            if metered {
                staged.check_history_capacity()?;
            }
            Ok(value)
        });
        match outcome {
            Ok(value) => {
                #[cfg(debug_assertions)]
                if valid_before && let Err(violation) = staged.validate_state() {
                    panic!("a transaction broke the state invariant: {violation}");
                }
                self.committed.state = staged.state;
                self.committed.trace = staged.trace;
                self.committed.published = staged.published;
                self.committed.kernel_leaf = staged.kernel_leaf;
                self.committed.state_token = Arc::new(());
                Ok(value)
            }
            Err(error) => Err(error.with_fault(staged.state.rules.typed_fault.clone())),
        }
    }
}

/// The rules as they stood before an event, for the triggers that look back
/// at it (CR 603.10a) and for the facts of a damage transaction the event
/// consumes. An explicit read-only capture: it shares every rules table and
/// each viewer's handles with the game, and carries no audit history, trace
/// or publication record, so taking one copies no table.
pub struct PreEvent(RulesTxn);

impl Deref for PreEvent {
    type Target = RulesTxn;
    fn deref(&self) -> &RulesTxn {
        &self.0
    }
}

impl RulesTxn {
    pub(in crate::engine) fn pre_event(&self) -> PreEvent {
        PreEvent(RulesTxn {
            opening_library_sizes: self.opening_library_sizes,
            catalog_has_land_type_grant: self.catalog_has_land_type_grant,
            catalog_has_land_reclamation: self.catalog_has_land_reclamation,
            catalog_has_nonland_color_grant: self.catalog_has_nonland_color_grant,
            catalog_has_target_color_set: self.catalog_has_target_color_set,
            catalog_has_mana_tap_observer: self.catalog_has_mana_tap_observer,
            prepared: self.prepared.clone(),
            definition_index: self.definition_index.clone(),
            state: State {
                rules: self.state.rules.clone(),
                knowledge: self.state.knowledge.clone(),
                audit: AuditState {
                    external_inputs: History::default(),
                    journal: History::default(),
                },
            },
            trace: TraceEvidence::new(TraceMode::Off),
            // A capture answers queries only; it never issues a receipt.
            executor: self.executor.clone(),
            state_token: self.state_token.clone(),
            in_rule_step: false,
            kernel_leaf: self.kernel_leaf.clone(),
            published: None,
        })
    }
}

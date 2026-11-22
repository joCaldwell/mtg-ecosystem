//! CR 701.34a, 608.2d, 101.4, 616.1, 714.2b and 800.4g/h (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// One candidate at a time encodes an arbitrary subset with bounded branching.
pub(super) type ProliferateChoices = Vec<(ObjectId, bool)>;

impl RulesTxn {
    pub(super) fn proliferate_eligible(&self, object: ObjectId) -> bool {
        // Every counter kind represented by the current game state is checked.
        // The admitted pool has no player counters.
        self.current(object).is_ok_and(|o| {
            o.present_on_battlefield()
                && (o.plus_one_counters > Quantity::zero() || o.lore_counters > Quantity::zero())
        })
    }

    pub(super) fn proliferate_request(
        &self,
        frame: &Resolving,
        choices: &ProliferateChoices,
    ) -> Option<Request> {
        // CR 701.34a/800.4h: this choice is prescribed by the keyword rule.
        let controller = self.entry(frame.stack_id).ok()?.controller;
        let player = if self.is_live(controller) {
            controller
        } else {
            self.next_live(controller)?
        };
        use std::ops::Bound::{Excluded, Unbounded};
        let after = choices
            .last()
            .map_or(Unbounded, |(object, _)| Excluded(object.card));
        let candidate = self
            .state
            .rules
            .objects
            .range((after, Unbounded))
            .map(|(_, object)| object.id)
            .find(|id| self.proliferate_eligible(*id))?;
        Some(Request::Proliferate {
            decision_id: self.state.rules.revision,
            player,
            candidate,
        })
    }

    pub(super) fn validate_proliferate_choice(
        &self,
        frame: &Resolving,
        choices: &ProliferateChoices,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if !matches!((self.proliferate_request(frame, choices), response),
            (Some(Request::Proliferate {player: chooser,candidate,..}), Response::Proliferate {object,..})
                if player == chooser && candidate == *object)
        {
            return Err("invalid proliferate choice".into());
        }
        Ok(())
    }

    pub(super) fn proliferate_step(
        &mut self,
        frame: Resolving,
        source_block: u32,
        mut choices: ProliferateChoices,
    ) -> Result<(), String> {
        if self.proliferate_request(&frame, &choices).is_some() {
            let input = self
                .rules_mut()
                .pending
                .take()
                .ok_or("missing proliferate choice")?;
            self.validate_proliferate_choice(&frame, &choices, input.player, &input.response)?;
            let Response::Proliferate { object, include } = input.response else {
                return Err("missing proliferate response".into());
            };
            choices.push((object, include));
        }
        if self.proliferate_request(&frame, &choices).is_some() {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Proliferate {
                    source_block,
                    choices,
                },
            };
            return Ok(());
        }

        let cause = self.instruction_cause(&frame, source_block)?;
        let members = choices
            .into_iter()
            .filter(|(object, include)| *include && self.proliferate_eligible(*object))
            .map(|(object, _)| {
                let o = self.current(object)?;
                let amount = Quantity::from(i64::from(o.plus_one_counters > Quantity::zero()));
                Ok(counter_placement::CounterPlacementMember {
                    object,
                    original_amount: amount.clone(),
                    amount,
                    applications: vec![],
                    lore: o.lore_counters > Quantity::zero(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.begin_counter_transaction(frame, members, cause)
    }

    pub(super) fn apply_proliferate(
        &mut self,
        id: ObjectId,
        plus_one: &Quantity,
        lore: bool,
    ) -> Result<EventOutcome, String> {
        let o = self
            .rules_mut()
            .objects
            .get_mut(&id.card)
            .ok_or("missing proliferate recipient")?;
        if o.id != id
            || !o.present_on_battlefield()
            || (*plus_one > Quantity::zero() && o.plus_one_counters <= Quantity::zero())
            || (lore && o.lore_counters <= Quantity::zero())
        {
            return Err("proliferate recipient or counter kind changed".into());
        }
        let previous_plus_one = o.plus_one_counters.clone();
        o.plus_one_counters.add(plus_one);
        let lore = lore.then(|| {
            let before = o.lore_counters.clone();
            o.lore_counters.add(&1.into());
            saga::LoreChange {
                object: id,
                controller: o.controller,
                before,
                after: o.lore_counters.clone(),
            }
        });
        Ok(EventOutcome::Proliferated {
            object: id,
            previous_plus_one,
            current_plus_one: o.plus_one_counters.clone(),
            lore,
        })
    }
}

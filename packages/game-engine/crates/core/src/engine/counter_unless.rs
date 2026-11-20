//! CR 118.3/12a, 605.3a, 608.2b/d/g and 701.6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CounterPaymentStage {
    Offer,
    Mana,
    Spend,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CounterPayment {
    target: StackId,
    pub(super) payer: PlayerId,
    cost: ManaCost,
    pub(super) stage: CounterPaymentStage,
    pub(super) receipts: Vec<ManaActivationUndo>,
}

impl Game {
    pub(super) fn begin_counter_payment(
        &mut self,
        frame: Resolving,
        target_slot: u32,
        generic: Quantity,
    ) -> Result<(), String> {
        let slot = target_slot as usize;
        let target = *frame
            .spell
            .stack_targets
            .get(slot)
            .ok_or("missing conditional counter target")?;
        if frame.legal_targets.get(slot) != Some(&true) {
            return self
                .finish_instruction(frame, InstructionOutcome::StackTargetIllegal { target });
        }
        let Some(entry) = self.state.rules.stack.iter().find(|s| s.id == target) else {
            return self.finish_instruction(
                frame,
                InstructionOutcome::StackObjectNoLongerPresent { target },
            );
        };
        self.state.rules.frame = Frame::CounterUnless {
            frame,
            payment: CounterPayment {
                target,
                payer: entry.controller,
                cost: ManaCost {
                    generic,
                    colored: vec![],
                },
                stage: CounterPaymentStage::Offer,
                receipts: vec![],
            },
        };
        Ok(())
    }

    pub(super) fn counter_payment_request(&self, payment: &CounterPayment) -> Request {
        let decision_id = self.state.rules.revision;
        let player = payment.payer;
        match payment.stage {
            CounterPaymentStage::Offer => Request::OptionalEffect {
                decision_id,
                player,
            },
            CounterPaymentStage::Mana => Request::Mana {
                decision_id,
                player,
                cost: payment.cost.clone(),
            },
            CounterPaymentStage::Spend => Request::SpendMana {
                decision_id,
                player,
                cost: payment.cost.clone(),
                available: self.state.rules.players[player.0 as usize].mana.clone(),
            },
        }
    }

    pub(super) fn validate_counter_payment_response(
        &self,
        payment: &CounterPayment,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if player != payment.payer || !self.is_live(player) {
            return Err("unauthorized resolution payment".into());
        }
        match (payment.stage, response) {
            (CounterPaymentStage::Offer, Response::OptionalEffect { .. })
            | (CounterPaymentStage::Mana, Response::FinishMana { .. }) => Ok(()),
            (CounterPaymentStage::Mana, Response::ActivateMana { object }) => {
                self.validate_mana_activation(player, *object)
            }
            (CounterPaymentStage::Spend, Response::SpendMana { spending }) => self
                .selected_payment_pool(player, &payment.cost, spending)
                .map(|_| ()),
            _ => Err("invalid resolution payment response".into()),
        }
    }

    pub(super) fn counter_payment_step(
        &mut self,
        frame: Resolving,
        mut payment: CounterPayment,
    ) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing resolution payment input")?;
        self.validate_counter_payment_response(&payment, input.player, &input.response)?;
        let (target_slot, _, source_block) = self.counter_payment_terms(&frame)?;
        match input.response {
            Response::OptionalEffect { perform: true } => payment.stage = CounterPaymentStage::Mana,
            Response::OptionalEffect { perform: false } => {
                return self.resolve_counter_instruction(frame, target_slot, source_block, None);
            }
            Response::ActivateMana { object } => {
                return self.begin_payment_mana(
                    &input,
                    object,
                    Frame::CounterUnless { frame, payment },
                );
            }
            Response::FinishMana { undo_mana } => {
                if !self.can_pay(payment.payer, &payment.cost)? {
                    if undo_mana {
                        self.undo_mana_receipts(payment.payer, &payment.receipts)?;
                    }
                    return self.resolve_counter_instruction(
                        frame,
                        target_slot,
                        source_block,
                        None,
                    );
                }
                payment.stage = CounterPaymentStage::Spend;
            }
            Response::SpendMana { spending } => {
                let actual = self.commit(
                    ProposedEvent::PayManaSelected {
                        player: payment.payer,
                        cost: payment.cost,
                        spending,
                    },
                    Self::staple_cause(&frame, source_block),
                )?;
                return self.finish_instruction(
                    frame,
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    },
                );
            }
            _ => return Err("resolution payment input invariant".into()),
        }
        self.state.rules.frame = Frame::CounterUnless { frame, payment };
        Ok(())
    }

    fn counter_payment_terms(&self, frame: &Resolving) -> Result<(u32, &Quantity, u32), String> {
        let Some(Instruction::CounterSpellUnlessPays {
            target_slot,
            generic,
            source_block,
        }) = self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("missing conditional counter instruction".into());
        };
        Ok((*target_slot, generic, *source_block))
    }

    pub(super) fn validate_counter_payment(
        &self,
        frame: &Resolving,
        payment: &CounterPayment,
    ) -> Result<(), String> {
        self.validate_resolving(frame)?;
        let (target_slot, generic, _) = self.counter_payment_terms(frame)?;
        let slot = target_slot as usize;
        let target = self
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.id == payment.target);
        if frame.spell.stack_targets.get(slot) != Some(&payment.target)
            || frame.legal_targets.get(slot) != Some(&true)
            || payment.payer.0 >= 4
            || target.is_some_and(|entry| entry.controller != payment.payer)
            || !payment.cost.colored.is_empty()
            || &payment.cost.generic != generic
            || (payment.stage == CounterPaymentStage::Offer && !payment.receipts.is_empty())
            || payment.receipts.iter().any(|r| r.player != payment.payer)
        {
            return Err("conditional counter payment binding invariant".into());
        }
        if matches!(self.state.rules.frame, Frame::CounterUnless { .. })
            && (!self.is_live(payment.payer)
                || target.is_none()
                || (payment.stage == CounterPaymentStage::Spend
                    && !self.can_pay(payment.payer, &payment.cost)?))
        {
            return Err("unavailable resolution payment".into());
        }
        Ok(())
    }

    // A paid mana ability owns its remaining production/damage even if the
    // target leaves. On return discard unpaid windows and preserve completed
    // activations. A departed resolving source alone cannot end the offer.
    pub(super) fn revalidate_counter_payment_departure(&mut self) -> Result<(), String> {
        let Frame::CounterUnless { frame, payment } =
            spell_cascade_grants::retained_effect_frame(&self.state.rules.frame)
        else {
            return Ok(());
        };
        let present = self
            .state
            .rules
            .stack
            .iter()
            .any(|s| s.id == payment.target);
        if present && self.is_live(payment.payer) {
            return Ok(());
        }
        match &self.state.rules.frame {
            Frame::ManaAbility { activation }
                if activation.stage == mana_permanents::ManaStage::Color
                    && self.is_live(activation.player) =>
            {
                return Ok(());
            }
            Frame::ManaAbility { .. }
            | Frame::ManaSourceChoice { .. }
            | Frame::CounterUnless { .. } => {}
            _ => return Ok(()),
        }
        let target = payment.target;
        let frame = frame.clone();
        self.state.rules.pending = None;
        if present {
            let (target_slot, _, source_block) = self.counter_payment_terms(&frame)?;
            return self.resolve_counter_instruction(frame, target_slot, source_block, None);
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::StackObjectNoLongerPresent { target },
        )
    }
}

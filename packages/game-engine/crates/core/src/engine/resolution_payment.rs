//! CR 118.3/118.12a, 605.3a, 608.2b/d/g and 701.6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionPaymentStage {
    Offer,
    Mana,
    Spend,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionPayment {
    purpose: PaymentPurpose,
    pub payer: PlayerId,
    cost: ManaCost,
    pub(super) stage: ResolutionPaymentStage,
    pub(super) receipts: Vec<ManaActivationUndo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PaymentPurpose {
    Counter { target_slot: u32 },
    CastPayerDraw,
}

impl Game {
    fn resolution_payment_terms<'a>(
        &'a self,
        frame: &'a Resolving,
    ) -> Result<(PaymentPurpose, &'a Quantity, u32), String> {
        match self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::CounterSpellUnlessPays {
                target_slot,
                generic,
                source_block,
            }) => Ok((
                PaymentPurpose::Counter {
                    target_slot: *target_slot,
                },
                generic,
                *source_block,
            )),
            Some(Instruction::OptionalDrawUnlessCasterPays {
                generic,
                source_block,
            }) => Ok((PaymentPurpose::CastPayerDraw, generic, *source_block)),
            _ => Err("missing resolution payment instruction".into()),
        }
    }

    // A counter binds to the controller of its legal stack target. A cast
    // trigger binds to the event caster even after that spell has left.
    fn resolution_payer(
        &self,
        frame: &Resolving,
        purpose: PaymentPurpose,
    ) -> Result<Option<PlayerId>, String> {
        match purpose {
            PaymentPurpose::Counter { target_slot } => {
                let slot = target_slot as usize;
                let target = frame
                    .spell
                    .stack_targets
                    .get(slot)
                    .ok_or("missing conditional counter target")?;
                if frame.legal_targets.get(slot) != Some(&true) {
                    return Ok(None);
                }
                Ok(self
                    .state
                    .rules
                    .stack
                    .iter()
                    .find(|s| s.id == *target)
                    .map(|s| s.controller))
            }
            PaymentPurpose::CastPayerDraw => match &frame.spell.kind {
                StackKind::Triggered {
                    witness: TriggerEventWitness::SpellCast { cast },
                    ..
                } => Ok(Some(cast.caster)),
                _ => Err("missing resolution payment cast witness".into()),
            },
        }
    }

    fn resolve_unpaid(
        &mut self,
        frame: Resolving,
        purpose: PaymentPurpose,
        source_block: u32,
    ) -> Result<(), String> {
        match purpose {
            PaymentPurpose::Counter { target_slot } => {
                self.resolve_counter_instruction(frame, target_slot, source_block, None)
            }
            PaymentPurpose::CastPayerDraw => self.offer_controller_draw(frame, source_block),
        }
    }

    pub(super) fn begin_resolution_payment(&mut self, frame: Resolving) -> Result<(), String> {
        let (purpose, generic, source_block) = self.resolution_payment_terms(&frame)?;
        let Some(payer) = self
            .resolution_payer(&frame, purpose)?
            .filter(|p| self.is_live(*p))
        else {
            return self.resolve_unpaid(frame, purpose, source_block);
        };
        let generic = generic.clone();
        self.state.rules.frame = Frame::ResolutionPayment {
            frame,
            payment: ResolutionPayment {
                purpose,
                payer,
                cost: ManaCost {
                    generic,
                    colored: vec![],
                },
                stage: ResolutionPaymentStage::Offer,
                receipts: vec![],
            },
        };
        Ok(())
    }

    pub(super) fn resolution_payment_request(&self, payment: &ResolutionPayment) -> Request {
        let decision_id = self.state.rules.revision;
        let player = payment.payer;
        match payment.stage {
            ResolutionPaymentStage::Offer => Request::OptionalEffect {
                decision_id,
                player,
            },
            ResolutionPaymentStage::Mana => Request::Mana {
                decision_id,
                player,
                cost: payment.cost.clone(),
            },
            ResolutionPaymentStage::Spend => Request::SpendMana {
                decision_id,
                player,
                cost: payment.cost.clone(),
                available: self.state.rules.players[player.0 as usize].mana.clone(),
                restricted_available: vec![],
            },
        }
    }

    pub(super) fn validate_resolution_payment_response(
        &self,
        payment: &ResolutionPayment,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if player != payment.payer || !self.is_live(player) {
            return Err("unauthorized resolution payment".into());
        }
        match (payment.stage, response) {
            (ResolutionPaymentStage::Offer, Response::OptionalEffect { .. })
            | (ResolutionPaymentStage::Mana, Response::FinishMana { .. }) => Ok(()),
            (ResolutionPaymentStage::Mana, Response::ActivateMana { object }) => {
                self.validate_mana_activation(player, *object)
            }
            (ResolutionPaymentStage::Spend, Response::SpendMana { spending }) => self
                .selected_payment_pool(player, &payment.cost, spending)
                .map(|_| ()),
            _ => Err("invalid resolution payment response".into()),
        }
    }

    pub(super) fn resolution_payment_step(
        &mut self,
        frame: Resolving,
        mut payment: ResolutionPayment,
    ) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing resolution payment input")?;
        self.validate_resolution_payment_response(&payment, input.player, &input.response)?;
        let (_, _, source_block) = self.resolution_payment_terms(&frame)?;
        match input.response {
            Response::OptionalEffect { perform: true } => {
                payment.stage = ResolutionPaymentStage::Mana
            }
            Response::OptionalEffect { perform: false } => {
                return self.resolve_unpaid(frame, payment.purpose, source_block);
            }
            Response::ActivateMana { object } => {
                return self.begin_payment_mana(
                    &input,
                    object,
                    Frame::ResolutionPayment { frame, payment },
                );
            }
            Response::FinishMana { undo_mana } => {
                if !self.can_pay(payment.payer, &payment.cost)? {
                    if undo_mana {
                        self.undo_mana_receipts(payment.payer, &payment.receipts)?;
                    }
                    return self.resolve_unpaid(frame, payment.purpose, source_block);
                }
                payment.stage = ResolutionPaymentStage::Spend;
            }
            Response::SpendMana { spending } => {
                let actual = self.commit(
                    ProposedEvent::PayManaSelected {
                        player: payment.payer,
                        cost: payment.cost,
                        spending,
                        restricted: vec![],
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
        self.state.rules.frame = Frame::ResolutionPayment { frame, payment };
        Ok(())
    }

    pub(crate) fn validate_resolution_payment(
        &self,
        frame: &Resolving,
        payment: &ResolutionPayment,
    ) -> Result<(), String> {
        self.validate_resolving(frame)?;
        let (purpose, generic, _) = self.resolution_payment_terms(frame)?;
        let payer = self.resolution_payer(frame, purpose)?;
        if matches!(purpose, PaymentPurpose::Counter { target_slot } if frame.legal_targets.get(target_slot as usize) != Some(&true))
            || payment.purpose != purpose
            || payment.payer.0 >= 4
            || payer.is_some_and(|p| p != payment.payer)
            || !payment.cost.colored.is_empty()
            || &payment.cost.generic != generic
            || (payment.stage == ResolutionPaymentStage::Offer && !payment.receipts.is_empty())
            || payment.receipts.iter().any(|r| r.player != payment.payer)
        {
            return Err("resolution payment binding invariant".into());
        }
        if matches!(self.state.rules.frame, Frame::ResolutionPayment { .. })
            && (!self.is_live(payment.payer)
                || payer.is_none()
                || (payment.stage == ResolutionPaymentStage::Spend
                    && !self.can_pay(payment.payer, &payment.cost)?))
        {
            return Err("unavailable resolution payment".into());
        }
        Ok(())
    }

    // A paid mana ability owns its remaining production/damage even if the
    // payer or counter target leaves. Keep completed activations and abandon
    // unpaid windows. Losing the resolving source alone leaves the offer intact.
    pub(super) fn revalidate_resolution_payment_departure(&mut self) -> Result<(), String> {
        let Frame::ResolutionPayment { frame, payment } =
            spell_cascade_grants::retained_effect_frame(&self.state.rules.frame)
        else {
            return Ok(());
        };
        if self.resolution_payer(frame, payment.purpose)? == Some(payment.payer)
            && self.is_live(payment.payer)
        {
            return Ok(());
        }
        match &self.state.rules.frame {
            Frame::ManaAbility { activation }
                if matches!(
                    activation.stage,
                    mana_permanents::ManaStage::Color
                        | mana_permanents::ManaStage::Combination { .. }
                ) && self.is_live(activation.player) =>
            {
                return Ok(());
            }
            Frame::ManaAbility { .. }
            | Frame::ManaSourceChoice { .. }
            | Frame::ResolutionPayment { .. } => {}
            _ => return Ok(()),
        }
        let (_, _, source_block) = self.resolution_payment_terms(frame)?;
        let purpose = payment.purpose;
        let frame = frame.clone();
        self.state.rules.pending = None;
        self.resolve_unpaid(frame, purpose, source_block)
    }
}

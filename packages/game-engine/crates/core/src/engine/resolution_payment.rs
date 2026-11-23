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
    pub(super) resume: FrameLink,
    pub payer: PlayerId,
    cost: ManaCost,
    pub(super) stage: ResolutionPaymentStage,
    pub(super) receipts: Vec<ManaActivationUndo>,
}

impl RulesTxn {
    pub(super) fn begin_kernel_payment(
        &mut self,
        task: kir::Task,
        payer: PlayerId,
        generic: Quantity,
    ) -> Result<(), String> {
        if generic.is_negative() {
            return Err("negative kernel mana payment".into());
        }
        let frame = self.kernel_root()?.clone();
        let resume = self.save_frame(Frame::Kernel {
            task: Box::new(task),
        });
        self.rules_mut().frame = Frame::ResolutionPayment {
            frame,
            payment: ResolutionPayment {
                resume,
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
            .rules_mut()
            .pending
            .take()
            .ok_or("missing resolution payment input")?;
        self.validate_resolution_payment_response(&payment, input.player, &input.response)?;
        let source_block = self.kernel_payment_terms(payment.resume)?.1;
        match input.response {
            Response::OptionalEffect { perform: true } => {
                payment.stage = ResolutionPaymentStage::Mana
            }
            Response::OptionalEffect { perform: false } => {
                return self.finish_kernel_payment(payment.resume, false);
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
                    return self.finish_kernel_payment(payment.resume, false);
                }
                payment.stage = ResolutionPaymentStage::Spend;
            }
            Response::SpendMana { spending } => {
                self.commit(
                    ProposedEvent::PayManaSelected {
                        player: payment.payer,
                        cost: payment.cost,
                        spending,
                        restricted: vec![],
                    },
                    self.instruction_cause(&frame, source_block)?,
                )?;
                return self.finish_kernel_payment(payment.resume, true);
            }
            _ => return Err("resolution payment input invariant".into()),
        }
        self.rules_mut().frame = Frame::ResolutionPayment { frame, payment };
        Ok(())
    }

    pub(crate) fn validate_resolution_payment(
        &self,
        frame: &Resolving,
        payment: &ResolutionPayment,
    ) -> Result<(), String> {
        self.validate_resolving(frame)?;
        let (payer, _) = self.kernel_payment_terms(payment.resume)?;
        if frame != self.kernel_root()?
            || payment.cost.generic.is_negative()
            || payment.payer.0 >= 4
            || payer != payment.payer
            || !payment.cost.colored.is_empty()
            || (payment.stage == ResolutionPaymentStage::Offer && !payment.receipts.is_empty())
            || payment.receipts.iter().any(|r| r.player != payment.payer)
        {
            return Err("resolution payment binding invariant".into());
        }
        if matches!(self.state.rules.frame, Frame::ResolutionPayment { .. })
            && (!self.is_live(payment.payer)
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
        let Frame::ResolutionPayment { payment, .. } =
            frame::retained_effect_frame(&self.state.rules.frames, &self.state.rules.frame)
        else {
            return Ok(());
        };
        if self.kernel_payment_terms(payment.resume)?.0 == payment.payer
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
        let resume = payment.resume;
        self.rules_mut().pending = None;
        self.finish_kernel_payment(resume, false)
    }
}

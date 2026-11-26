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
        cost: ManaCost,
    ) -> Result<(), String> {
        if cost.generic.is_negative() {
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
                cost,
                stage: ResolutionPaymentStage::Offer,
                receipts: vec![],
            },
        };
        Ok(())
    }

    pub(super) fn resolution_payment_request(
        &self,
        payment: &ResolutionPayment,
    ) -> Result<Decision<Rules>, String> {
        match payment.stage {
            ResolutionPaymentStage::Offer => Ok(self.pick_flags(
                payment.payer,
                Reason::OptionalEffect,
                &[Flag::Accept, Flag::Decline],
            )),
            ResolutionPaymentStage::Mana | ResolutionPaymentStage::Spend => self.payment_decision(
                payment.payer,
                payment.cost.clone(),
                None,
                payment.stage == ResolutionPaymentStage::Mana,
            ),
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
        let source_block = self.kernel_payment_terms(payment.resume)?.1;
        match (&input.answer, payment.stage) {
            (Answer::Pick { .. }, ResolutionPaymentStage::Offer) if input.flag(Flag::Accept)? => {
                payment.stage = ResolutionPaymentStage::Mana
            }
            (Answer::Pick { .. }, ResolutionPaymentStage::Offer) => {
                return self.finish_kernel_payment(payment.resume, None);
            }
            (
                Answer::Pay {
                    pay: PayAnswer::Activate { source },
                },
                ResolutionPaymentStage::Mana,
            ) => {
                let object = input.mana_source(*source)?;
                return self.begin_payment_mana(
                    &input,
                    object,
                    Frame::ResolutionPayment { frame, payment },
                );
            }
            (
                Answer::Pay {
                    pay:
                        PayAnswer::Finish {
                            undo_on_failure: undo_mana,
                        },
                },
                ResolutionPaymentStage::Mana,
            ) => {
                if !self.can_pay(payment.payer, &payment.cost)? {
                    if *undo_mana {
                        self.undo_mana_receipts(payment.payer, &payment.receipts)?;
                    }
                    return self.finish_kernel_payment(payment.resume, None);
                }
                payment.stage = ResolutionPaymentStage::Spend;
            }
            (
                Answer::Pay {
                    pay: PayAnswer::Spend { unrestricted },
                },
                ResolutionPaymentStage::Spend,
            ) => {
                let spending = unrestricted.clone();
                let batch = self.commit_batch(
                    self.instruction_cause(&frame, source_block)?,
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::PayManaSelected {
                            player: payment.payer,
                            cost: payment.cost,
                            spending,
                            restricted: vec![],
                        },
                    },
                )?;
                return self.finish_kernel_payment(payment.resume, Some(&batch));
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
        let (payer, _, cost) = self.kernel_payment_terms(payment.resume)?;
        if frame != self.kernel_root()?
            || payment.cost.generic.is_negative()
            || payment.cost != cost
            || payment.payer.0 >= 4
            || payer != payment.payer
            || (payment.stage == ResolutionPaymentStage::Offer && !payment.receipts.is_empty())
            || payment.receipts.iter().any(|r| r.player != payment.payer)
        {
            return Err("resolution payment binding invariant".into());
        }
        self.validate_mana_receipt_conditions(&payment.receipts)?;
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
                if activation.stage == mana_permanents::ManaStage::Color
                    && self.is_live(activation.player) =>
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
        self.finish_kernel_payment(resume, None)
    }
}

use super::*;

/// Lack of funds is an ordinary casting outcome; corrupt inputs are not.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PaymentError {
    InsufficientMana,
    Invariant(&'static str),
}
impl From<PaymentError> for String {
    fn from(error: PaymentError) -> Self {
        match error {
            PaymentError::InsufficientMana => "insufficient mana".into(),
            PaymentError::Invariant(reason) => reason.into(),
        }
    }
}

/// Successful completion is distinct from a rules inability to pay and from
/// an engine error. This receipt is produced only after every cost and the one
/// cast occurrence; a caller, rather than the payment primitive, owns resumption.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct AnnouncementCompleted {
    pub(super) stack_id: StackId,
    pub(super) controller: PlayerId,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum AnnouncementPayment {
    Completed(AnnouncementCompleted),
    Unpayable,
}

pub(super) fn phyrexian_life(choices: &[HybridPayment]) -> Quantity {
    let mut total = Quantity::zero();
    for choice in choices {
        if *choice == HybridPayment::PayTwoLife {
            total.add(&2.into());
        }
    }
    total
}

impl Game {
    pub(super) fn can_pay(&self, player: PlayerId, cost: &ManaCost) -> Result<bool, String> {
        match self.payment_pool(player, cost) {
            Ok(_) => Ok(true),
            Err(PaymentError::InsufficientMana) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn finish_mana_frame(&self, cast: Casting) -> Result<Frame, String> {
        let stack = self.top()?;
        Ok(
            if self.state.rules.players[stack.controller.0 as usize]
                .can_pay_life(&phyrexian_life(&stack.hybrid_colors))
                && self.can_pay(stack.controller, &stack.total_cost)?
            {
                Frame::SpendMana { cast }
            } else {
                Frame::Payment { cast }
            },
        )
    }

    pub(super) fn spend_mana_request(&self) -> Result<Request, String> {
        let stack = self.top()?;
        Ok(Request::SpendMana {
            decision_id: self.state.rules.revision,
            player: stack.controller,
            cost: stack.total_cost.clone(),
            available: self.state.rules.players[stack.controller.0 as usize]
                .mana
                .clone(),
        })
    }

    pub(super) fn validate_spending(
        &self,
        player: PlayerId,
        spending: &BTreeMap<Color, Quantity>,
    ) -> Result<(), String> {
        let stack = self.top()?;
        if stack.controller != player {
            return Err("mana spending controller invariant".into());
        }
        self.selected_payment_pool(player, &stack.total_cost, spending)?;
        Ok(())
    }

    // CR 601.2h / 118.3: the payer chooses which available mana pays generic
    // costs. A complete selection must satisfy every colored symbol and pay
    // exactly the total cost. Never silently spend a different color.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn selected_payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        spending: &BTreeMap<Color, Quantity>,
    ) -> Result<BTreeMap<Color, Quantity>, String> {
        if !self.is_live(player) || cost.generic.is_negative() {
            return Err("invalid mana payer/cost".into());
        }
        let mut remaining = self
            .state
            .rules
            .players
            .get(player.0 as usize)
            .ok_or("missing paying player")?
            .mana
            .clone();
        let mut total = Quantity::zero();
        for (color, amount) in spending {
            if *amount <= Quantity::zero() {
                return Err("mana spending entries must be positive".into());
            }
            let available = remaining.get_mut(color).ok_or("unavailable mana color")?;
            if *available < *amount {
                return Err("selected mana exceeds available pool".into());
            }
            available.subtract(amount);
            total.add(amount);
        }
        let mut required = BTreeMap::<Color, Quantity>::new();
        let mut exact_total = cost.generic.clone();
        for color in &cost.colored {
            required.entry(*color).or_default().add(&Quantity::from(1));
            exact_total.add(&Quantity::from(1));
        }
        if total != exact_total
            || required
                .iter()
                .any(|(color, count)| spending.get(color).cloned().unwrap_or_default() < *count)
        {
            return Err("selected mana does not satisfy exact cost".into());
        }
        Ok(remaining)
    }

    pub(super) fn spend_mana_step(&mut self, mut cast: Casting) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing spending input")?;
        let Response::SpendMana { spending } = input.response else {
            return Err("mana spending response/frame invariant".into());
        };
        self.validate_spending(input.player, &spending)?;
        cast.spending = Some(spending);
        if self.invocation_sacrifice(self.top()?).is_some() {
            self.state.rules.frame = Frame::SacrificeCost { cast };
            Ok(())
        } else {
            self.payment_step(cast)
        }
    }

    pub(super) fn payment_step(&mut self, cast: Casting) -> Result<(), String> {
        let during_resolution = !matches!(cast.completion, CastCompletion::Priority);
        match self.pay_announcement_costs(&cast)? {
            AnnouncementPayment::Completed(completed) => match cast.completion {
                CastCompletion::Priority => {
                    self.state.rules.priority = completed.controller;
                    self.state.rules.frame = Frame::Priority;
                }
                CastCompletion::HandTrigger { frame } => {
                    self.state.rules.frame = Frame::HandCast {
                        frame,
                        child: Some(completed.stack_id),
                    };
                }
                CastCompletion::Resolution {
                    permission,
                    mut progress,
                } => {
                    if completed.stack_id != cast.stack_id
                        || completed.controller != permission.caster
                    {
                        return Err("resolution payment completion identity".into());
                    }
                    match &mut progress {
                        CastOwner::LinkedPlay {
                            progress, object, ..
                        } => {
                            // Paying an additional sacrifice cost can move the
                            // granting permanent while the child is announced.
                            if let Some(issuer) = self
                                .state
                                .rules
                                .stack
                                .iter()
                                .find(|s| s.id == progress.frame.stack_id)
                            {
                                progress
                                    .frame
                                    .spell
                                    .source_facts
                                    .clone_from(&issuer.source_facts);
                            }
                            progress.complete_cast(*object, completed.stack_id);
                        }
                        CastOwner::Cascade(progress) => {
                            progress.child = Some(completed.stack_id);
                            progress.stage = cascade::CascadeStage::Randomize;
                            self.prepare_cascade_remainder(progress)?;
                        }
                        CastOwner::RevealAndCast(progress) => {
                            progress.complete_cast(permission.object, completed.stack_id)?
                        }
                    }
                    self.state.rules.frame = progress.resume();
                }
            },
            AnnouncementPayment::Unpayable => {
                let undo = cast.undo_mana;
                let reason = if self.state.rules.players[self.top()?.controller.0 as usize]
                    .can_pay_life(&phyrexian_life(&self.top()?.hybrid_colors))
                {
                    "insufficient mana"
                } else {
                    "insufficient life or prohibited life payment"
                };
                self.reverse_cast(cast, reason, undo)?;
            }
        }
        if during_resolution {
            Ok(())
        } else {
            self.stabilize()
        }
    }

    // CR 601.2h-i / 608.2g: payment and cast-trigger collection do not grant
    // priority or stabilize. The caller returns either to priority or to its
    // owned resolving continuation. Payment itself never supplies permission.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn pay_announcement_costs(
        &mut self,
        cast: &Casting,
    ) -> Result<AnnouncementPayment, String> {
        let stack = self.top()?.clone();
        self.validate_hand_cast_child(&stack)?;
        self.validate_cost_lock(&stack)?;
        self.validate_additional_cost_entry(&stack)?;
        if stack.id != cast.stack_id
            || stack.paid.is_some()
            || stack.kind.is_spell() != cast.origin.is_spell()
        {
            return Err("announcement payment binding invariant".into());
        }
        if let CastCompletion::Resolution { permission, .. } = &cast.completion
            && !self.resulting_permission_matches(permission, &stack)?
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        let cost = stack.total_cost.clone();
        if let StackKind::Activated { ability, .. } = &stack.kind
            && matches!(
                self.definition(stack.source.card)?.activated[*ability as usize].cost,
                ActivationCost::DiscardSource { .. }
            )
            && !self.hand_activation_source_payable(stack.controller, stack.source)
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        if let StackKind::Activated { ability, .. } = stack.kind
            && let ActivationCost::Composite { costs } =
                &self.definition(stack.source.card)?.activated[ability as usize].cost
            && !self.permanent_source_costs_payable(stack.source, stack.controller, costs)
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        let life = phyrexian_life(&stack.hybrid_colors);
        if !self.state.rules.players[stack.controller.0 as usize].can_pay_life(&life)
            || !self.can_pay(stack.controller, &cost)?
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        self.check_sacrifice_payment(&stack, cast)?;
        let proposed = {
            let spending = cast
                .spending
                .clone()
                .ok_or("missing explicit mana spending")?;
            self.selected_payment_pool(stack.controller, &cost, &spending)?;
            ProposedEvent::PayManaSelected {
                player: stack.controller,
                cost: cost.clone(),
                spending,
            }
        };
        self.commit(
            proposed,
            BatchCause::Payment {
                stack_id: cast.stack_id,
                player: stack.controller,
            },
        )?;
        if life > Quantity::zero() {
            self.commit(
                ProposedEvent::LoseLife {
                    player: stack.controller,
                    amount: life,
                },
                BatchCause::Payment {
                    stack_id: cast.stack_id,
                    player: stack.controller,
                },
            )?;
        }
        // CR601.2h/602.2b/702.29a: debit the complete explicit mana selection
        // before discarding precisely the preflighted source as a cost. The
        // containing rule step stages both occurrences atomically. No choice
        // of another hand card, no effect-time discard, no early cost on failure.
        if let StackKind::Activated { ability, .. } = &stack.kind
            && matches!(
                self.definition(stack.source.card)?.activated[*ability as usize].cost,
                ActivationCost::DiscardSource { .. }
            )
        {
            self.commit(
                ProposedEvent::DiscardCard {
                    object: stack.source,
                    player: stack.controller,
                },
                BatchCause::AbilityActivation {
                    stack_id: stack.id,
                    player: stack.controller,
                    source: stack.source,
                    decision_id: self.state.rules.revision,
                    ability: *ability,
                },
            )?;
        }
        if let StackKind::Activated { ability, .. } = stack.kind
            && let ActivationCost::Composite { costs } =
                self.definition(stack.source.card)?.activated[ability as usize]
                    .cost
                    .clone()
        {
            self.pay_permanent_source_costs(
                stack.source,
                stack.controller,
                &costs,
                BatchCause::AbilityActivation {
                    stack_id: stack.id,
                    player: stack.controller,
                    source: stack.source,
                    decision_id: self.state.rules.revision,
                    ability,
                },
            )?;
        }
        self.pay_sacrifice_cost(&stack, &cast.sacrifices)?;
        self.state
            .rules
            .stack
            .last_mut()
            .ok_or("missing stack")?
            .paid = Some(cost);
        if stack.kind.is_spell() {
            self.record_event(Event::SpellCast {
                stack_id: cast.stack_id,
            })?;
            self.collect_cast_triggers(&stack)?;
            self.state.rules.commanders.record_cast(
                stack.source.card,
                stack.controller,
                stack.cast_from,
            )?;
        } else if let StackKind::Activated { ability, .. } = &stack.kind {
            self.record_event(Event::AbilityActivated {
                stack_id: stack.id,
                source: stack.source,
                player: stack.controller,
                ability: *ability,
            })?;
        }
        Ok(AnnouncementPayment::Completed(AnnouncementCompleted {
            stack_id: stack.id,
            controller: stack.controller,
        }))
    }
}

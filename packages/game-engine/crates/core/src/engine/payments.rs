use super::*;

/// CR 106.6: the purchase that owns this exact payment window.
#[derive(Clone, Copy)]
pub(in crate::engine) enum ManaUse {
    Spell(ManaSpellTraits),
    Activation {
        source: Option<ManaSpellTraits>,
        battlefield: bool,
    },
    Resolution,
}

/// Lack of funds is an ordinary casting outcome; corrupt inputs are not.
#[derive(Debug, PartialEq, Eq)]
pub enum PaymentError {
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
pub struct AnnouncementCompleted {
    pub stack_id: StackId,
    pub controller: PlayerId,
}
#[derive(Debug, PartialEq, Eq)]
pub enum AnnouncementPayment {
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

impl RulesTxn {
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn can_pay(&self, player: PlayerId, cost: &ManaCost) -> Result<bool, String> {
        match self.payment_pool(player, cost) {
            Ok(_) => Ok(true),
            Err(PaymentError::InsufficientMana) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn mana_payment_pool(
        &self,
        player: PlayerId,
        use_on: ManaUse,
    ) -> Result<ManaPool, String> {
        let payer = &self.state.rules.players[player.0 as usize];
        let restricted = payer
            .restricted_mana
            .iter()
            .filter(|unit| match use_on {
                ManaUse::Spell(spell) => unit.restriction.permits_spell(spell),
                ManaUse::Activation {
                    source,
                    battlefield,
                } => source.map_or_else(
                    || unit.restriction.permits_activation(false),
                    |source| unit.restriction.permits_activation_of(source, battlefield),
                ),
                ManaUse::Resolution => unit.restriction.permits_resolution_payment(),
            })
            .cloned()
            .collect();
        Ok(ManaPool {
            unrestricted: payer.mana.clone(),
            restricted,
        })
    }

    pub(super) fn can_pay_for(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        use_on: ManaUse,
    ) -> Result<bool, String> {
        Ok(self.is_live(player)
            && mtg_contracts::mana_spend::payment(
                cost,
                &self.mana_payment_pool(player, use_on)?.available(),
            )
            .is_some())
    }

    pub(super) fn announced_mana_use(&self, stack: &StackInstance) -> Result<ManaUse, String> {
        Ok(if stack.kind.is_spell() {
            ManaUse::Spell(ManaSpellTraits::from_characteristics(
                &self.derived_characteristics_with_sources(stack.source, None)?,
            ))
        } else {
            self.activation_mana_use(stack.source, stack.source_facts.as_ref())?
        })
    }

    pub(super) fn activation_mana_use(
        &self,
        source: ObjectId,
        last: Option<&ObjectBefore>,
    ) -> Result<ManaUse, String> {
        let (source, battlefield) = match self.current(source) {
            Ok(object) => (
                Some(ManaSpellTraits::from_characteristics(
                    &self.derived_characteristics_with_sources(source, None)?,
                )),
                object.present_on_battlefield(),
            ),
            Err(_) => (
                last.map(|facts| ManaSpellTraits::from_characteristics(&facts.characteristics)),
                false,
            ),
        };
        Ok(ManaUse::Activation {
            source,
            battlefield,
        })
    }

    pub(super) fn can_pay_announced(
        &self,
        stack: &StackInstance,
        cost: &ManaCost,
    ) -> Result<bool, String> {
        self.can_pay_for(stack.controller, cost, self.announced_mana_use(stack)?)
    }

    pub(super) fn validate_announced_allocation(
        &self,
        stack: &StackInstance,
        cost: &ManaCost,
        spending: &BTreeMap<Color, Quantity>,
        restricted: &[RestrictedMana],
    ) -> Result<(), String> {
        self.validate_spending(
            stack.controller,
            cost,
            self.announced_mana_use(stack)?,
            spending,
            restricted,
        )
    }

    pub(super) fn validate_spending(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        use_on: ManaUse,
        spending: &BTreeMap<Color, Quantity>,
        restricted: &[RestrictedMana],
    ) -> Result<(), String> {
        let pool = self.mana_payment_pool(player, use_on)?;
        let pay = if restricted.is_empty() {
            PayAnswer::Spend {
                unrestricted: spending.clone(),
            }
        } else {
            PayAnswer::SpendRestricted {
                unrestricted: spending.clone(),
                restricted: restricted.to_vec(),
            }
        };
        Shape::Pay {
            cost: cost.clone(),
            pool,
            step: PayStep::Spend,
        }
        .check(&Answer::Pay { pay })
    }

    fn delve_candidates(&self, entry: &StackInstance) -> Result<Vec<ObjectId>, String> {
        if !entry.kind.is_spell()
            || !self.stack_definition(entry)?.delve
            || entry.total_cost.generic == Quantity::zero()
        {
            return Ok(vec![]);
        }
        Ok(self
            .state
            .rules
            .objects
            .values()
            .filter(|object| {
                object.zone
                    == Zone::Graveyard {
                        owner: entry.controller,
                    }
            })
            .map(|object| object.id)
            .collect())
    }

    pub(super) fn mana_due(
        &self,
        entry: &StackInstance,
        cast: &Casting,
    ) -> Result<ManaCost, String> {
        let mut cost = entry.total_cost.clone();
        if let Some(selected) = &cast.delve {
            let eligible = self.delve_candidates(entry)?;
            let count = Quantity::from(selected.len() as i64);
            if count > cost.generic
                || selected.iter().collect::<BTreeSet<_>>().len() != selected.len()
                || selected.iter().any(|id| !eligible.contains(id))
            {
                return Err("invalid delve payment".into());
            }
            cost.generic.subtract(&count);
        }
        Ok(cost)
    }

    // CR 601.2g-h and 702.66a-b: choose cards after the mana ability window
    // and keep the locked total intact. Exile only when payment commits.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn delve_decision(&self, cast: &Casting) -> Result<Option<Decision<Rules>>, String> {
        if cast.delve.is_some() {
            return Ok(None);
        }
        let entry = self.top()?;
        let candidates = self.delve_candidates(entry)?;
        if candidates.is_empty() {
            return Ok(None);
        }
        let max =
            (candidates.len() as u32).min(entry.total_cost.generic.to_u32().unwrap_or(u32::MAX));
        let mut colored = entry.total_cost.clone();
        colored.generic = Quantity::zero();
        let pool = self.mana_payment_pool(entry.controller, self.announced_mana_use(entry)?)?;
        let surplus = match Self::deduct_payment_cost(pool.available(), &colored) {
            Ok(pool) => pool.into_values().fold(Quantity::zero(), |mut sum, n| {
                sum.add(&n);
                sum
            }),
            Err(PaymentError::InsufficientMana) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut deficit = entry.total_cost.generic.clone();
        deficit.subtract(&surplus);
        let deficit = deficit.max(Quantity::zero());
        if deficit > Quantity::from(i64::from(max)) {
            return Ok(None);
        }
        Ok(Some(
            self.pick(
                entry.controller,
                Reason::ChosenObjectCost {
                    action: ChosenCostAction::ExileFromGraveyard,
                },
                candidates
                    .into_iter()
                    .map(|object| Candidate::Object { object })
                    .collect(),
                deficit
                    .to_u32()
                    .ok_or("delve deficit exceeds candidate bound")?,
                max,
            ),
        ))
    }

    pub(super) fn finish_mana_frame(&self, cast: Casting) -> Result<Frame, String> {
        let stack = self.top()?;
        Ok(
            if self.state.rules.players[stack.controller.0 as usize]
                .can_pay_life(&phyrexian_life(&stack.hybrid_colors))
                && (self.delve_decision(&cast)?.is_some()
                    || self.can_pay_announced(stack, &stack.total_cost)?)
            {
                Frame::SpendMana { cast }
            } else {
                Frame::Payment { cast }
            },
        )
    }

    // CR 601.2h / 118.3: the payer chooses which available mana pays generic
    // costs. A complete selection must satisfy every colored symbol and pay
    // exactly the total cost. Never silently spend a different color.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn selected_payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        spending: &BTreeMap<Color, Quantity>,
    ) -> Result<BTreeMap<Color, Quantity>, String> {
        if !self.is_live(player) || cost.generic.is_negative() {
            return Err("invalid mana payer/cost".into());
        }
        let pool = self
            .state
            .rules
            .players
            .get(player.0 as usize)
            .ok_or("missing paying player")?
            .mana
            .clone();
        let remaining = mtg_view::subtract_selected_mana(pool, spending)?;
        mtg_view::validate_selected_mana_cost(cost, spending)?;
        Ok(remaining)
    }

    pub(super) fn spend_mana_step(&mut self, mut cast: Casting) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing spending input")?;
        if matches!(input.answer, Answer::Pick { .. }) && self.delve_decision(&cast)?.is_some() {
            cast.delve = Some(input.objects()?);
            self.mana_due(self.top()?, &cast)?;
            self.rules_mut().frame = Frame::SpendMana { cast };
            return Ok(());
        }
        let Answer::Pay { pay } = &input.answer else {
            return Err("expected mana spending".into());
        };
        let (unrestricted, restricted) = pay.spending().ok_or("expected mana allocation")?;
        cast.spending = Some(unrestricted.clone());
        cast.spending_restricted = restricted.to_vec();
        if self.invocation_chosen_cost(self.top()?)?.is_some() {
            self.rules_mut().frame = Self::chosen_cost_frame(cast);
            Ok(())
        } else if self.invocation_sacrifice(self.top()?).is_some() {
            self.rules_mut().frame = Frame::SacrificeCost { cast };
            Ok(())
        } else {
            self.payment_step(cast)
        }
    }

    // Paying an additional sacrifice cost can move the permanent whose ability
    // owns this cast. `cast` is the step's private copy, so the owner it
    // resumes is reconciled with its live stack entry when the step commits
    // its frame lease (CR 608.2h/113.7a).
    pub(super) fn payment_step(&mut self, cast: Casting) -> Result<(), String> {
        self.payment_step_with_objects(cast, &[])
    }

    pub(super) fn payment_step_with_objects(
        &mut self,
        cast: Casting,
        selected: &[ObjectId],
    ) -> Result<(), String> {
        match self.pay_announcement_costs(&cast, selected)? {
            AnnouncementPayment::Completed(completed) => {
                self.rules_mut().priority = completed.controller;
                self.rules_mut().frame = Frame::Priority;
            }
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
        Ok(self.stabilize()?)
    }

    // CR 601.2h-i / 608.2g: payment and cast-trigger collection do not grant
    // priority or stabilize. The caller returns either to priority or to its
    // owned resolving continuation. Payment itself never supplies permission.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(crate) fn pay_announcement_costs(
        &mut self,
        cast: &Casting,
        selected: &[ObjectId],
    ) -> Result<AnnouncementPayment, String> {
        let stack = self.top()?.clone();

        self.validate_cost_lock(&stack)?;
        self.validate_additional_cost_entry(&stack)?;
        if stack.id != cast.stack_id
            || stack.paid.is_some()
            || stack.kind.is_spell() != cast.origin.is_spell()
        {
            return Err("announcement payment binding invariant".into());
        }

        let cost = self.mana_due(&stack, cast)?;
        if let StackKind::Activated { ability, .. } = &stack.kind
            && let activated = &self.stack_definition(&stack)?.activated[*ability as usize]
            && matches!(
                activated.cost,
                ActivationCost::DiscardSource { .. } | ActivationCost::ExileSource { .. }
            )
            && !self.zone_source_payable(activated.functioning_zone, stack.controller, stack.source)
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        if let StackKind::Activated { ability, .. } = stack.kind
            && let ActivationCost::Composite { costs } =
                &self.stack_definition(&stack)?.activated[ability as usize].cost
            && !self.permanent_source_costs_payable(stack.source, stack.controller, costs)
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        // CR 602.2b: a sacrifice cost that also taps its source needs that
        // source untapped under its controller when the cost is paid.
        let tap_with_sacrifice = match stack.kind {
            StackKind::Activated { ability, .. } => matches!(
                self.stack_definition(&stack)?.activated[ability as usize].cost,
                ActivationCost::SacrificePermanent { tap: true, .. }
            ),
            _ => false,
        };
        if tap_with_sacrifice
            && !self.current(stack.source).is_ok_and(|o| {
                o.present_on_battlefield() && o.controller == stack.controller && !o.tapped
            })
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        let life = phyrexian_life(&stack.hybrid_colors);
        if !self.state.rules.players[stack.controller.0 as usize].can_pay_life(&life)
            || !self.can_pay_announced(&stack, &cost)?
        {
            return Ok(AnnouncementPayment::Unpayable);
        }
        self.check_sacrifice_payment(&stack, cast)?;
        let chosen = self.chosen_payment_events(&stack, selected)?;
        let uncounterable = match self.announced_mana_use(&stack)? {
            ManaUse::Spell(spell) => cast
                .spending_restricted
                .iter()
                .any(|unit| unit.restriction.prevents_countering(spell)),
            _ => false,
        };
        let proposed = {
            let spending = cast
                .spending
                .clone()
                .ok_or("missing explicit mana spending")?;
            let restricted = cast.spending_restricted.clone();
            self.validate_announced_allocation(&stack, &cost, &spending, &restricted)?;
            ProposedEvent::PayManaSelected {
                player: stack.controller,
                cost: cost.clone(),
                spending,
                restricted,
            }
        };
        self.commit(
            proposed,
            BatchCause::Payment {
                stack_id: cast.stack_id,
                player: stack.controller,
            },
        )?;
        for object in cast.delve.iter().flatten() {
            self.commit(
                ProposedEvent::Move {
                    object: *object,
                    destination: Zone::Exile,
                },
                BatchCause::Payment {
                    stack_id: cast.stack_id,
                    player: stack.controller,
                },
            )?;
        }
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
                self.stack_definition(&stack)?.activated[*ability as usize].cost,
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
        // CR 602.2b, 601.2h, 701.13a: after mana, exile exactly the
        // preflighted source card from the zone its ability functions in.
        if let StackKind::Activated { ability, .. } = &stack.kind
            && matches!(
                self.stack_definition(&stack)?.activated[*ability as usize].cost,
                ActivationCost::ExileSource { .. }
            )
        {
            self.commit(
                ProposedEvent::Move {
                    object: stack.source,
                    destination: Zone::Exile,
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
            && let ActivationCost::Composite { costs } = self.stack_definition(&stack)?.activated
                [ability as usize]
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
        if tap_with_sacrifice && let StackKind::Activated { ability, .. } = stack.kind {
            self.commit(
                ProposedEvent::Tap {
                    object: stack.source,
                },
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
        self.commit_chosen_payment(&stack, chosen)?;
        self.finish_payment_shuffles(cast)?;
        let entry = self.rules_mut().stack.last_mut().ok_or("missing stack")?;
        entry.paid = Some(stack.total_cost.clone());
        entry.mana_uncounterable = uncounterable;
        if stack.kind.is_spell() {
            self.record_event(Event::SpellCast {
                stack_id: cast.stack_id,
            })?;
            self.finish_target_triggers(stack.id)?;
            self.collect_cast_triggers(&stack)?;
            self.rules_mut().commanders.record_cast(
                stack.source.card,
                stack.controller,
                stack.cast_from,
            )?;
        } else if let StackKind::Activated { ability, .. } = &stack.kind {
            self.record_ability_activated(stack.id, stack.source, stack.controller, *ability)?;
        }
        Ok(AnnouncementPayment::Completed(AnnouncementCompleted {
            stack_id: stack.id,
            controller: stack.controller,
        }))
    }
}

use super::*;

impl Game {
    pub(super) fn finish_mana_frame(&self, cast: Casting) -> Result<Frame, String> {
        let stack = self.top()?;
        Ok(
            if self
                .payment_pool(stack.controller, &stack.total_cost)
                .is_ok()
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
            decision_id: self.state.revision,
            player: stack.controller,
            cost: stack.total_cost.clone(),
            available: self.state.players[stack.controller.0 as usize].mana.clone(),
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
        let input = self.state.pending.take().ok_or("missing spending input")?;
        let Response::SpendMana { spending } = input.response else {
            return Err("mana spending response/frame invariant".into());
        };
        self.validate_spending(input.player, &spending)?;
        cast.spending = Some(spending);
        if self.invocation_sacrifice_filter(self.top()?).is_some() {
            self.state.frame = Frame::SacrificeCost { cast };
            Ok(())
        } else {
            self.payment_step(cast)
        }
    }

    pub(super) fn payment_step(&mut self, cast: Casting) -> Result<(), String> {
        let stack = self.top()?.clone();
        let cost = stack.total_cost.clone();
        if self.payment_pool(stack.controller, &cost).is_err() {
            let undo = cast.undo_mana;
            self.reverse_cast(cast, "insufficient mana", undo)?;
            self.stabilize()?;
            return Ok(());
        }
        self.check_sacrifice_payment(&stack, &cast)?;
        let proposed = {
            let spending = cast.spending.ok_or("missing explicit mana spending")?;
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
                    decision_id: self.state.revision,
                    ability,
                },
            )?;
        }
        self.pay_sacrifice_cost(&stack, cast.sacrifice)?;
        self.state.stack.last_mut().ok_or("missing stack")?.paid = Some(cost);
        if stack.kind.is_spell() {
            self.record_event(Event::SpellCast {
                stack_id: cast.stack_id,
            })?;
            self.state.commanders.record_cast(
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
        self.state.priority = stack.controller;
        self.state.frame = Frame::Priority;
        self.stabilize()
    }
}

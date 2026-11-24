//! Completing a resolving spell or ability, and the payment pool arithmetic
//! the announcement stages share.
use crate::engine::*;

impl RulesTxn {
    pub(in crate::engine) fn finish_spell(
        &mut self,
        id: StackId,
        resolved: bool,
    ) -> Result<(), RuleError> {
        let stack = self.entry(id)?;
        if !stack.kind.is_spell() {
            return Ok(self.finish_ability(id, resolved)?);
        }
        let (source, controller) = (stack.source, stack.controller);
        let owner = self.state.rules.cards[&source.card].owner;
        if self.current(source).is_ok() {
            let index = self
                .resolving_entry_index(id)?
                .ok_or("missing resolving spell")?;
            // CR 608.3a: a resolving permanent spell becomes a permanent;
            // targetless spells do not fail the targeted-spell legality test.
            // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
            let program = self.program_for(stack)?;
            let destination = if resolved && program.resolution == SpellResolution::Permanent {
                Zone::Battlefield
            } else {
                Zone::Graveyard { owner }
            };
            if destination == Zone::Battlefield
                && self.copiable_definition(source)?.characteristics.subtypes == ["Aura"]
            {
                let target = stack
                    .targets
                    .first()
                    .copied()
                    .ok_or("missing Aura target")?;
                self.finish_spell_aura_entry(source, controller, target, id)?;
            } else {
                self.move_object_for(
                    source,
                    destination,
                    BatchCause::SpellCompletion {
                        stack_id: id,
                        player: controller,
                        source,
                    },
                )?;
            }
            self.unstack(index)?;
        } else if self.invocation_lives(stack) || self.state.rules.stack.iter().any(|s| s.id == id)
        {
            return Err("unsupported resolving-source movement".into());
        }
        self.complete_spell_resolution(id, resolved)
    }
    pub(in crate::engine) fn complete_spell_resolution(
        &mut self,
        id: StackId,
        resolved: bool,
    ) -> Result<(), RuleError> {
        self.record_event(Event::SpellCompleted {
            stack_id: id,
            resolved,
        })?;
        self.rules_mut().frame = Frame::Priority;
        self.rules_mut().priority = self
            .state
            .rules
            .active
            .or_else(|| self.next_live(self.state.rules.turn_owner))
            .ok_or("no live player after resolution")?;
        self.stabilize()
    }
    pub(crate) fn payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
    ) -> Result<BTreeMap<Color, Quantity>, payments::PaymentError> {
        use payments::PaymentError;
        let pool = self
            .state
            .rules
            .players
            .get(player.0 as usize)
            .ok_or(PaymentError::Invariant("missing paying player"))?
            .mana
            .clone();
        Self::deduct_payment_cost(pool, cost)
    }
    pub(in crate::engine) fn deduct_payment_cost(
        mut pool: BTreeMap<Color, Quantity>,
        cost: &ManaCost,
    ) -> Result<BTreeMap<Color, Quantity>, payments::PaymentError> {
        use payments::PaymentError;
        if cost.generic.is_negative() {
            return Err(PaymentError::Invariant("negative mana cost"));
        }
        if pool.values().any(Quantity::is_negative) {
            return Err(PaymentError::Invariant("negative mana pool"));
        }
        for color in &cost.colored {
            let n = pool.entry(*color).or_default();
            if *n < Quantity::from(1) {
                return Err(PaymentError::InsufficientMana);
            }
            n.subtract(&Quantity::from(1));
        }
        let mut generic = cost.generic.clone();
        for n in pool.values_mut() {
            let spent = std::cmp::min(n.clone(), generic.clone());
            n.subtract(&spent);
            generic.subtract(&spent);
        }
        if generic > Quantity::zero() {
            return Err(PaymentError::InsufficientMana);
        }
        Ok(pool)
    }
    pub(crate) fn stabilize(&mut self) -> Result<(), RuleError> {
        if matches!(self.state.rules.frame, Frame::Priority) {
            self.schedule_stabilization(Frame::Priority, false);
        }
        Ok(())
    }
}

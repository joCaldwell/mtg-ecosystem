//! CR 106.4, 106.6, 601.2h, 602.2b and 733 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::mana_permanents::ManaStage;
use super::*;

#[derive(Clone, Copy)]
pub(super) enum ManaConsumer {
    Spell { colorless: bool },
    Activation,
}

impl ManaConsumer {
    fn permits(self, restriction: ManaSpendRestriction) -> bool {
        match restriction {
            ManaSpendRestriction::ActivatedAbility => matches!(self, Self::Activation),
            ManaSpendRestriction::ColorlessSpellOrActivatedAbility => {
                matches!(self, Self::Activation | Self::Spell { colorless: true })
            }
        }
    }
}

pub(super) fn credit(pool: &mut Vec<RestrictedMana>, unit: &RestrictedMana) {
    match pool.binary_search_by_key(&unit.key(), RestrictedMana::key) {
        Ok(index) => pool[index].amount.add(&unit.amount),
        Err(index) => pool.insert(index, unit.clone()),
    }
}

pub(super) fn debit(pool: &mut Vec<RestrictedMana>, unit: &RestrictedMana) -> Result<(), String> {
    let index = pool
        .binary_search_by_key(&unit.key(), RestrictedMana::key)
        .map_err(|_| "missing restricted mana bucket")?;
    let amount = &mut pool[index].amount;
    if unit.amount <= Quantity::zero() || *amount < unit.amount {
        return Err("restricted mana spending exceeds available pool".into());
    }
    amount.subtract(&unit.amount);
    if *amount == Quantity::zero() {
        pool.remove(index);
    }
    Ok(())
}

pub(super) const MAX_RESTRICTED_BUCKETS: usize = 2 * 6;

pub(super) fn valid_buckets(pool: &[RestrictedMana]) -> bool {
    pool.len() <= MAX_RESTRICTED_BUCKETS
        && pool.iter().all(|unit| unit.amount > Quantity::zero())
        && pool.windows(2).all(|pair| pair[0].key() < pair[1].key())
}

impl Game {
    pub(super) fn announced_mana_consumer(
        &self,
        stack: &StackInstance,
    ) -> Result<ManaConsumer, String> {
        match stack.kind {
            StackKind::Spell => {
                let colors = self
                    .characteristics(stack.source)
                    .map(|c| c.colors)
                    .or_else(|error| {
                        stack
                            .source_facts
                            .as_ref()
                            .map(|facts| facts.characteristics.colors.clone())
                            .ok_or(error)
                    })?;
                let colorless = colors.is_empty();
                Ok(ManaConsumer::Spell { colorless })
            }
            StackKind::Activated { .. } => Ok(ManaConsumer::Activation),
            _ => Err("mana allocation requires a spell or activated ability".into()),
        }
    }

    pub(super) fn allocated_payment_consumer(
        &self,
        player: PlayerId,
    ) -> Result<ManaConsumer, String> {
        match &self.state.rules.frame {
            Frame::ManaAbility { activation }
                if activation.player == player && activation.stage == ManaStage::Spend =>
            {
                Ok(ManaConsumer::Activation)
            }
            Frame::SpendMana { cast } | Frame::Payment { cast } | Frame::SacrificeCost { cast }
                if self
                    .top()
                    .is_ok_and(|stack| stack.controller == player && stack.id == cast.stack_id) =>
            {
                self.announced_mana_consumer(self.top()?)
            }
            Frame::GraveyardGroup { tx }
                if tx.payment_stack().is_some_and(|id| {
                    self.top()
                        .is_ok_and(|stack| stack.controller == player && stack.id == id)
                }) =>
            {
                self.announced_mana_consumer(self.top()?)
            }
            _ => Err("restricted mana cannot pay this cost".into()),
        }
    }

    pub(super) fn eligible_restricted(
        &self,
        player: PlayerId,
        consumer: ManaConsumer,
    ) -> Vec<RestrictedMana> {
        self.state.rules.players[player.0 as usize]
            .restricted_mana
            .iter()
            .filter(|unit| consumer.permits(unit.restriction))
            .cloned()
            .collect()
    }

    pub(super) fn can_pay_with_restricted(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        consumer: ManaConsumer,
    ) -> Result<bool, String> {
        let mut funds = self.state.rules.players[player.0 as usize].mana.clone();
        for unit in self.eligible_restricted(player, consumer) {
            funds.entry(unit.color).or_default().add(&unit.amount);
        }
        match Self::deduct_payment_cost(funds, cost) {
            Ok(_) => Ok(true),
            Err(payments::PaymentError::InsufficientMana) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub(super) fn allocated_mana_pools(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        spending: &BTreeMap<Color, Quantity>,
        restricted: &[RestrictedMana],
        consumer: ManaConsumer,
    ) -> Result<(BTreeMap<Color, Quantity>, Vec<RestrictedMana>), String> {
        if !self.is_live(player) || cost.generic.is_negative() || !valid_buckets(restricted) {
            return Err("invalid restricted mana allocation".into());
        }
        let p = &self.state.rules.players[player.0 as usize];
        let mut slots = p.restricted_mana.clone();
        let mut all_spent = spending.clone();
        for unit in restricted {
            if !consumer.permits(unit.restriction) {
                return Err("mana spend restriction excludes this cost".into());
            }
            debit(&mut slots, unit)?;
            all_spent.entry(unit.color).or_default().add(&unit.amount);
        }
        let remaining = Self::subtract_selected_mana(p.mana.clone(), spending)?;
        Self::validate_selected_mana_cost(cost, &all_spent)?;
        Ok((remaining, slots))
    }
}

//! CR 118.7/118.9d, 601.2f-h, 702.41, 903.8 (frozen 2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Counts/tax are historical facts authenticated by accepted-input replay.
//! Never validate a locked total by consulting the later battlefield.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CostLock {
    increases: Quantity,
    reducers: Vec<CountFact>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountFact {
    slot: u32,
    count: u32,
}
/// Pure construction: substitute only base, then X/additionals/increases, then
/// every distinct reduction slot, finally floor generic at zero.
fn total_cost(
    mut base: ManaCost,
    x: Option<&Quantity>,
    additionals: &[ManaCost],
    increases: &Quantity,
    reductions: &[(Quantity, u32)],
) -> ManaCost {
    if let Some(x) = x {
        base.generic.add(x);
    }
    for cost in additionals {
        base.generic.add(&cost.generic);
        base.colored.extend_from_slice(&cost.colored);
    }
    base.generic.add(increases);
    for (per_object, count) in reductions {
        let mut amount = per_object.clone();
        amount.multiply(&Quantity::from(i64::from(*count)));
        base.generic.subtract(&amount);
    }
    if base.generic.is_negative() {
        base.generic = Quantity::zero();
    }
    base
}
impl Game {
    pub(super) fn cost_lock_charge(&self) -> usize {
        let mut charge = self
            .state
            .rules
            .stack
            .iter()
            .filter_map(|s| s.cost_lock.as_ref())
            .fold(0usize, |n, lock| {
                n.saturating_add(stored_history::payload_charge(lock))
            });
        // Cascade owners are already charged in full by cascade_charge.
        if let Some(entry) = spell_cascade_grants::retained_resolving_entry(&self.state.rules.frame)
            && let Some(lock) = &entry.cost_lock
        {
            charge = charge.saturating_add(stored_history::payload_charge(lock));
        }
        charge
    }

    pub(super) fn cost_lock_inputs(
        &self,
        card: CardId,
        caster: PlayerId,
        from: Zone,
    ) -> Result<CostLock, String> {
        let increases = if matches!(from, Zone::Command { .. }) {
            if from != (Zone::Command { owner: caster }) {
                return Err("unsupported command-zone casting permission".into());
            }
            self.state.rules.commanders.tax(card, caster)?
        } else {
            Quantity::zero()
        };
        let mut reducers = vec![];
        for (slot, ability) in self.definition(card)?.static_abilities.iter().enumerate() {
            if let Some((_, selector)) = ability.effect.counted_generic_reduction() {
                let mut count = 0u32;
                for object in self.state.rules.objects.values() {
                    if object.zone == Zone::Battlefield
                        && (selector.controller == CountController::Any
                            || object.controller == caster)
                        && self
                            .definition(object.id.card)?
                            .characteristics
                            .has_type(selector.kind)
                    {
                        count = count.checked_add(1).ok_or("battlefield count overflow")?;
                    }
                }
                reducers.push(CountFact {
                    slot: u32::try_from(slot).map_err(|_| "reducer slot overflow")?,
                    count,
                });
            }
        }
        Ok(CostLock {
            increases,
            reducers,
        })
    }
    pub(super) fn construct_spell_cost(
        &self,
        card: CardId,
        alternative: Option<cascade::AlternativeCost>,
        x: Option<&Quantity>,
        selected: &[String],
        lock: &CostLock,
    ) -> Result<ManaCost, String> {
        let definition = self.definition(card)?;
        let program = definition
            .spell
            .as_ref()
            .ok_or("missing spell cost program")?;
        if lock.increases.is_negative() {
            return Err("negative locked increase".into());
        }
        let slots = definition
            .static_abilities
            .iter()
            .enumerate()
            .filter_map(|(slot, a)| a.effect.counted_generic_reduction().map(|(q, _)| (slot, q)))
            .collect::<Vec<_>>();
        if lock.reducers.len() != slots.len() {
            return Err("cost lock reducer count invariant".into());
        }
        let mut reductions = vec![];
        for ((slot, per_object), fact) in slots.into_iter().zip(&lock.reducers) {
            if usize::try_from(fact.slot).ok() != Some(slot)
                || fact.count as usize
                    > capacities::OWNED_LIMIT * 4 + capacities::TOKEN_CAPACITY as usize
            {
                return Err("cost lock reducer slot/battlefield bound invariant".into());
            }
            reductions.push((per_object, fact.count));
        }
        let mut additional = vec![];
        for (index, identity) in selected.iter().enumerate() {
            if selected[..index].contains(identity) {
                return Err("duplicate additional cost".into());
            }
            additional.push(
                program
                    .optional_additional_costs
                    .iter()
                    .find(|c| c.identity == *identity)
                    .ok_or("foreign additional cost")?
                    .mana
                    .clone(),
            );
        }
        Ok(total_cost(
            match alternative {
                None => definition.characteristics.mana_cost.clone(),
                Some(cascade::AlternativeCost::WithoutManaCost) => zero_mana_cost(),
            },
            x,
            &additional,
            &lock.increases,
            &reductions,
        ))
    }
    pub(super) fn lock_spell_cost(&mut self) -> Result<(), String> {
        let entry = self.top()?;
        if !entry.kind.is_spell() {
            return Ok(());
        }
        if entry.cost_lock.is_some() {
            return self.validate_cost_lock(entry);
        }
        let lock = self.cost_lock_inputs(entry.source.card, entry.controller, entry.cast_from)?;
        let total = self.construct_spell_cost(
            entry.source.card,
            entry.alternative_cost,
            entry.x.as_ref(),
            entry.selected_additional_costs.as_deref().unwrap_or(&[]),
            &lock,
        )?;
        let entry = self
            .state
            .rules
            .stack
            .last_mut()
            .ok_or("missing lock entry")?;
        entry.total_cost = total;
        entry.cost_lock = Some(lock);
        Ok(())
    }
    pub(super) fn validate_cost_lock(&self, entry: &StackInstance) -> Result<(), String> {
        if !entry.kind.is_spell() {
            return if entry.cost_lock.is_none() {
                Ok(())
            } else {
                Err("nonspell cost lock".into())
            };
        }
        if let Some(lock) = &entry.cost_lock {
            if matches!(self.underlying_mana_frame(&self.state.rules.frame),
                Frame::AdditionalCosts {cast} | Frame::AnnounceX {cast} | Frame::Modes {cast} | Frame::Targets {cast} | Frame::EntityTargets {cast} | Frame::StackTargets {cast} | Frame::PlayerTargets {cast} if cast.stack_id == entry.id)
            {
                return Err("spell cost locked before announcement choices".into());
            }
            if (!matches!(entry.cast_from, Zone::Command { .. })
                && lock.increases != Quantity::zero())
                || entry.total_cost
                    != self.construct_spell_cost(
                        entry.source.card,
                        entry.alternative_cost,
                        entry.x.as_ref(),
                        entry.selected_additional_costs.as_deref().unwrap_or(&[]),
                        lock,
                    )?
            {
                return Err("locked spell total invariant".into());
            }
            return Ok(());
        }
        let frame = self.underlying_mana_frame(&self.state.rules.frame);
        if entry.paid.is_none()
            && matches!(frame,
            Frame::AdditionalCosts {cast} | Frame::AnnounceX {cast} | Frame::Modes {cast} | Frame::Targets {cast} | Frame::EntityTargets {cast} | Frame::StackTargets {cast} | Frame::PlayerTargets {cast} if cast.stack_id == entry.id)
            || matches!(frame, Frame::Fault { .. } | Frame::Finished { .. })
        {
            return Ok(());
        }
        Err("missing spell cost lock".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Synthetic typed composition: no complete optional-affinity source is admitted.
    #[test]
    fn m4e_pure_cost_order_x_additionals_tax_multiple_slots_and_floor() {
        let base = ManaCost {
            generic: 2.into(),
            colored: vec![Color::W, Color::C],
        };
        let additional = ManaCost {
            generic: 3.into(),
            colored: vec![Color::U],
        };
        let result = total_cost(
            base,
            Some(&4.into()),
            &[additional],
            &2.into(),
            &[(2.into(), 3), (1.into(), 4)],
        );
        assert_eq!(
            result,
            ManaCost {
                generic: 1.into(),
                colored: vec![Color::W, Color::C, Color::U]
            }
        );
        assert_eq!(
            total_cost(
                zero_mana_cost(),
                None,
                &[ManaCost {
                    generic: 3.into(),
                    colored: vec![Color::R]
                }],
                &2.into(),
                &[(3.into(), 2)]
            ),
            ManaCost {
                generic: 0.into(),
                colored: vec![Color::R]
            }
        );
        let huge: Quantity = "999999999999999999999999999999999".parse().unwrap();
        assert_eq!(
            total_cost(zero_mana_cost(), None, &[], &0.into(), &[(huge, 656)]),
            zero_mana_cost()
        );
    }
}

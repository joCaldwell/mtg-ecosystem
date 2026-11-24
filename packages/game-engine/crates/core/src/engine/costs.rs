//! CR 118.7/118.9d, 601.2f-h, 903.8 (frozen 2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Tax is a historical fact authenticated by accepted-input replay.
//! Never validate a locked total by consulting the later battlefield.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostLock {
    increases: Quantity,
}
/// Pure construction: substitute only base, then X/additionals/increases.
fn total_cost(
    mut base: ManaCost,
    x: Option<&Quantity>,
    additionals: &[ManaCost],
    increases: &Quantity,
) -> ManaCost {
    if let Some(x) = x {
        base.generic.add(x);
    }
    for cost in additionals {
        base.generic.add(&cost.generic);
        base.colored.extend_from_slice(&cost.colored);
    }
    base.generic.add(increases);
    base
}
pub fn entry_cost_lock_charge(entry: &StackInstance) -> usize {
    entry
        .cost_lock
        .as_ref()
        .map_or(0, stored_history::payload_charge)
}
impl RulesTxn {
    pub(crate) fn cost_lock_charge(&self) -> usize {
        // Every invocation has one owner: its stack entry, or the detached
        // table while a frame still resolves it.
        let rules = &self.state.rules;
        rules
            .stack
            .iter()
            .chain(rules.detached.values())
            .fold(0usize, |n, entry| {
                n.saturating_add(entry_cost_lock_charge(entry))
            })
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
        Ok(CostLock { increases })
    }
    pub(super) fn validate_hybrid_colors(
        &self,
        card: CardId,
        colors: &[HybridPayment],
    ) -> Result<(), String> {
        let symbols = &self.definition(card)?.characteristics.hybrid_mana;
        if symbols.is_empty()
            || symbols.len() != colors.len()
            || !symbols
                .iter()
                .zip(colors)
                .all(|(s, c)| s.colors.contains(c))
        {
            return Err("hybrid announcement requires one allowed color per printed symbol".into());
        }
        Ok(())
    }
    pub(super) fn construct_spell_cost(
        &self,
        card: CardId,
        x: Option<&Quantity>,
        selected: &[String],
        hybrid: Option<&[HybridPayment]>,
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
            {
                let mut cost = definition.characteristics.mana_cost.clone();
                if let Some(colors) = hybrid {
                    self.validate_hybrid_colors(card, colors)?;
                    cost.colored
                        .extend(colors.iter().filter_map(|choice| choice.color()));
                }
                cost
            },
            x,
            &additional,
            &lock.increases,
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
        if !self
            .copiable_definition(entry.source)?
            .characteristics
            .hybrid_mana
            .is_empty()
        {
            self.validate_hybrid_colors(entry.source.card, &entry.hybrid_colors)?;
        }
        let lock = self.cost_lock_inputs(entry.source.card, entry.controller, entry.cast_from)?;
        let total = self.construct_spell_cost(
            entry.source.card,
            entry.x.as_ref(),
            entry.selected_additional_costs.as_deref().unwrap_or(&[]),
            (!entry.hybrid_colors.is_empty()).then_some(entry.hybrid_colors.as_slice()),
            &lock,
        )?;
        let entry = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing lock entry")?;
        entry.total_cost = total;
        entry.cost_lock = Some(lock);
        Ok(())
    }
    pub(crate) fn validate_cost_lock(&self, entry: &StackInstance) -> Result<(), String> {
        if self.is_spell_copy(entry) {
            return if entry.cost_lock.is_none()
                && entry.hybrid_colors.is_empty()
                && entry.paid.is_none()
                && entry.total_cost == zero_mana_cost()
                && entry.cast_from == Zone::Stack
            {
                Ok(())
            } else {
                Err("spell copy has casting or payment provenance".into())
            };
        }
        let needs_choice = entry.kind.is_spell()
            && !self
                .copiable_definition(entry.source)?
                .characteristics
                .hybrid_mana
                .is_empty();
        let choosing = matches!(self.underlying_mana_frame(&self.state.rules.frame)?,Frame::HybridCost {cast} if cast.stack_id==entry.id);
        if choosing && (!needs_choice || entry.paid.is_some())
            || (!entry.hybrid_colors.is_empty()) != (needs_choice && !choosing)
        {
            return Err("hybrid announcement binding invariant".into());
        }
        if !entry.hybrid_colors.is_empty() {
            self.validate_hybrid_colors(entry.source.card, &entry.hybrid_colors)?;
        }
        if !entry.kind.is_spell() {
            return if entry.cost_lock.is_none() {
                Ok(())
            } else {
                Err("nonspell cost lock".into())
            };
        }
        if let Some(lock) = &entry.cost_lock {
            if matches!(self.underlying_mana_frame(&self.state.rules.frame)?,
                Frame::AdditionalCosts {cast} | Frame::HybridCost {cast} | Frame::AnnounceX {cast} | Frame::Targets {cast} | Frame::EntityTargets {cast} | Frame::StackTargets {cast} | Frame::PlayerTargets {cast} if cast.stack_id == entry.id)
            {
                return Err("spell cost locked before announcement choices".into());
            }
            if (!matches!(entry.cast_from, Zone::Command { .. })
                && lock.increases != Quantity::zero())
                || entry.total_cost
                    != self.construct_spell_cost(
                        entry.source.card,
                        entry.x.as_ref(),
                        entry.selected_additional_costs.as_deref().unwrap_or(&[]),
                        (!entry.hybrid_colors.is_empty()).then_some(entry.hybrid_colors.as_slice()),
                        lock,
                    )?
            {
                return Err("locked spell total invariant".into());
            }
            return Ok(());
        }
        let frame = self.underlying_mana_frame(&self.state.rules.frame)?;
        if entry.paid.is_none()
            && matches!(frame,
            Frame::AdditionalCosts {cast} | Frame::HybridCost {cast} | Frame::AnnounceX {cast} | Frame::Targets {cast} | Frame::EntityTargets {cast} | Frame::StackTargets {cast} | Frame::PlayerTargets {cast} if cast.stack_id == entry.id)
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
    #[test]
    fn pure_cost_adds_x_additionals_and_tax_to_the_base() {
        let base = ManaCost {
            generic: 2.into(),
            colored: vec![Color::W, Color::C],
        };
        let additional = ManaCost {
            generic: 3.into(),
            colored: vec![Color::U],
        };
        assert_eq!(
            total_cost(base, Some(&4.into()), &[additional], &2.into()),
            ManaCost {
                generic: 11.into(),
                colored: vec![Color::W, Color::C, Color::U]
            }
        );
    }
}

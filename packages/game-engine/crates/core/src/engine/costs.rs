//! CR 118.7/118.9d, 601.2f-h, 903.8 (frozen 2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Tax is a historical fact authenticated by accepted-input replay.
//! Never validate a locked total by consulting the later battlefield.
use super::*;
use mtg_contracts::kir::SpellCostChange;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostLock {
    increases: Quantity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    static_changes: Vec<SpellCostChange>,
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
// CR 601.2f, 118.7a-d. Generic reductions only subtract generic
// mana. A colored or colorless reduction consumes its own requirement
// first; any excess then reduces generic mana. For these fixed symbols
// the reductions commute because both use the same zero floor.
fn apply_static_cost_changes(
    cost: &mut ManaCost,
    changes: &[SpellCostChange],
) -> Result<(), String> {
    for change in changes {
        if let SpellCostChange::Increase { mana } = change {
            if mana.generic.is_negative() {
                return Err("negative static cost increase".into());
            }
            cost.generic.add(&mana.generic);
            cost.colored.extend_from_slice(&mana.colored);
        }
    }
    for change in changes {
        if let SpellCostChange::Reduce { mana } = change {
            if mana.generic.is_negative() {
                return Err("negative static cost reduction".into());
            }
            cost.generic.subtract(&mana.generic);
            for color in &mana.colored {
                if let Some(index) = cost.colored.iter().position(|c| c == color) {
                    cost.colored.remove(index);
                } else {
                    cost.generic.subtract(&1.into());
                }
            }
        }
    }
    cost.generic = std::mem::take(&mut cost.generic).max(Quantity::zero());
    Ok(())
}

impl CostLock {
    /// CR 601.2f, 602.2b: an activation's own changes, read once as its
    /// total is determined. An activation pays no commander tax.
    pub(super) fn activation(static_changes: Vec<SpellCostChange>) -> Self {
        Self {
            increases: Quantity::zero(),
            static_changes,
        }
    }
}

/// CR 601.2f, 602.2b: the announced activation total after its locked
/// changes (no lock: the announced total itself).
pub(super) fn locked_activation_total(
    mut cost: ManaCost,
    lock: Option<&CostLock>,
) -> Result<ManaCost, String> {
    if let Some(lock) = lock {
        if lock.increases != Quantity::zero() {
            return Err("activation cost lock with tax".into());
        }
        apply_static_cost_changes(&mut cost, &lock.static_changes)?;
    }
    Ok(cost)
}

/// The locked changes are, in printed order, a subsequence of the printed
/// changes: each keeps its direction and colored symbols, and a counted one
/// a nonnegative multiple of its generic mana (replay authenticates which
/// applied and the count; CR 601.2f).
fn activation_lock_consistent(
    printed: &[mtg_contracts::ActivationCostChange],
    locked: &[SpellCostChange],
) -> bool {
    let mut printed = printed.iter();
    locked.iter().all(|locked| {
        printed.any(|p| match (&p.change, locked) {
            (SpellCostChange::Increase { mana: a }, SpellCostChange::Increase { mana: b })
            | (SpellCostChange::Reduce { mana: a }, SpellCostChange::Reduce { mana: b }) => {
                a.colored == b.colored
                    && !b.generic.is_negative()
                    && if p.multiplier.is_some() {
                        a.generic > Quantity::zero() && b.generic.is_multiple_of(&a.generic)
                    } else {
                        a.generic == b.generic
                    }
            }
            _ => false,
        })
    })
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
        source: ObjectId,
        caster: PlayerId,
        from: Zone,
    ) -> Result<CostLock, String> {
        let increases = if matches!(from, Zone::Command { .. }) {
            if from != (Zone::Command { owner: caster }) {
                return Err("unsupported command-zone casting permission".into());
            }
            self.state.rules.commanders.tax(source.card, caster)?
        } else {
            Quantity::zero()
        };
        let static_changes = self.static_cost_changes(source, caster)?;
        Ok(CostLock {
            increases,
            static_changes,
        })
    }
    /// CR 601.2f, 602.2b: the activated ability's own cost changes that
    /// apply now, each multiplied by its count, read from `source` as its
    /// statics are.
    pub(super) fn activation_cost_lock(
        &self,
        source: ObjectId,
        changes: &[mtg_contracts::ActivationCostChange],
    ) -> Result<Option<CostLock>, String> {
        if changes.is_empty() {
            return Ok(None);
        }
        let object = self.current(source)?;
        let mut locked = Vec::new();
        for change in changes {
            if !self.condition_holds(object, change.condition.as_ref())? {
                continue;
            }
            let mut applied = change.change.clone();
            if let Some(amount) = &change.multiplier {
                let count = self
                    .static_amount(object, amount)?
                    .ok_or("invalid activation cost multiplier")?;
                if count.is_negative() {
                    return Err("negative activation cost multiplier".into());
                }
                let (SpellCostChange::Increase { mana } | SpellCostChange::Reduce { mana }) =
                    &mut applied;
                mana.generic.multiply(&count);
            }
            locked.push(applied);
        }
        Ok(Some(CostLock::activation(locked)))
    }
    pub(super) fn validate_hybrid_colors(
        &self,
        source: ObjectId,
        colors: &[HybridPayment],
    ) -> Result<(), String> {
        stack_costs::validate_hybrid_choice(
            &self
                .copiable_definition(source)?
                .characteristics
                .hybrid_mana,
            colors,
        )
    }
    /// CR 107.4e-f, 601.2b, 602.2b: the hybrid symbols this entry's spell or
    /// activated ability announces.
    pub(crate) fn entry_hybrid_symbols(
        &self,
        entry: &StackInstance,
    ) -> Result<&[ColorHybridSymbol], String> {
        let definition = self.stack_definition(entry)?;
        Ok(match entry.kind {
            StackKind::Spell => &definition.characteristics.hybrid_mana,
            StackKind::Activated { ability, .. } => {
                &definition
                    .activated
                    .get(ability as usize)
                    .ok_or("missing activated hybrid definition")?
                    .hybrid_mana
            }
            _ => &[],
        })
    }
    pub(super) fn construct_spell_cost(
        &self,
        source: ObjectId,
        from: Zone,
        x: Option<&Quantity>,
        selected: &[String],
        hybrid: Option<&[HybridPayment]>,
        lock: &CostLock,
    ) -> Result<ManaCost, String> {
        let definition = self.copiable_definition(source)?;
        let program = definition
            .spell
            .as_ref()
            .ok_or("missing spell cost program")?;
        if lock.increases.is_negative() {
            return Err("negative locked increase".into());
        }
        let mut additional = vec![];
        // CR 118.9, 702.74a: an announced alternative cost replaces the
        // mana cost of a hand or command-zone cast.
        let mut alternative = None;
        for (index, identity) in selected.iter().enumerate() {
            if selected[..index].contains(identity) {
                return Err("duplicate additional cost".into());
            }
            if let Some(cost) = definition
                .alternative_cost
                .as_ref()
                .filter(|c| c.identity == *identity)
            {
                if !matches!(from, Zone::Hand { .. } | Zone::Command { .. })
                    || x.is_some()
                    || hybrid.is_some()
                {
                    return Err("alternative cost announced outside its casting route".into());
                }
                alternative = Some(cost.mana.clone());
                continue;
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
        let mut cost = total_cost(
            {
                // CR 702.34a, 118.9: the flashback cost replaces the mana cost
                // of a spell cast from its owner's graveyard.
                let mut cost = match from {
                    // CR 702.37a, c: a face-down spell pays {3} rather than
                    // its mana cost (an alternative cost, CR 118.9).
                    _ if self.is_face_down(source) => {
                        if x.is_some() || hybrid.is_some() {
                            return Err("a face-down cast announces no X or hybrid".into());
                        }
                        FaceDownCast::cast_cost()
                    }
                    // CR 702.81a: retrace pays the mana cost; its land
                    // discard is a chosen-object cost.
                    Zone::Graveyard { .. } if definition.retrace => {
                        definition.characteristics.mana_cost.clone()
                    }
                    // CR 702.138a, 118.9: the escape cost replaces it.
                    Zone::Graveyard { .. } if definition.escape.is_some() => {
                        if x.is_some() || hybrid.is_some() {
                            return Err("escape cost announces no X or hybrid".into());
                        }
                        definition
                            .escape
                            .as_ref()
                            .map(|e| e.mana.clone())
                            .ok_or("missing escape cost")?
                    }
                    Zone::Graveyard { .. } => {
                        if x.is_some() || hybrid.is_some() {
                            return Err("flashback cost announces no X or hybrid".into());
                        }
                        definition
                            .flashback
                            .clone()
                            .ok_or("graveyard cast without flashback")?
                    }
                    _ => {
                        alternative.unwrap_or_else(|| definition.characteristics.mana_cost.clone())
                    }
                };
                if let Some(colors) = hybrid {
                    self.validate_hybrid_colors(source, colors)?;
                    for choice in colors {
                        choice.add_to(&mut cost);
                    }
                }
                cost
            },
            x,
            &additional,
            &lock.increases,
        );
        apply_static_cost_changes(&mut cost, &lock.static_changes)?;
        Ok(cost)
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
            .stack_definition(entry)?
            .characteristics
            .hybrid_mana
            .is_empty()
        {
            self.validate_hybrid_colors(entry.source, &entry.hybrid_colors)?;
        }
        let lock = self.cost_lock_inputs(entry.source, entry.controller, entry.cast_from)?;
        let total = self.construct_spell_cost(
            entry.source,
            entry.cast_from,
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
        let symbols = self.entry_hybrid_symbols(entry)?;
        let needs_choice = !symbols.is_empty();
        let frame = self.underlying_mana_frame(&self.state.rules.frame)?;
        let choosing = matches!(frame, Frame::HybridCost {cast} if cast.stack_id==entry.id);
        // CR 601.2b: optional additional costs are declared before hybrid
        // symbols, so the symbols stay unchosen through that stage.
        let declaring = matches!(frame, Frame::ChooseModes {cast} | Frame::AdditionalCosts {cast} if cast.stack_id==entry.id);
        // A game can end (CR 104.2a) while a spell is still being announced;
        // its unpaid entry then keeps whatever it had announced.
        let unfinished =
            entry.paid.is_none() && matches!(frame, Frame::Fault { .. } | Frame::Finished { .. });
        if choosing && (!needs_choice || entry.paid.is_some())
            || (unfinished && !needs_choice && !entry.hybrid_colors.is_empty())
            || (!unfinished
                && (!entry.hybrid_colors.is_empty()) != (needs_choice && !choosing && !declaring))
        {
            return Err("hybrid announcement binding invariant".into());
        }
        if !entry.hybrid_colors.is_empty() {
            stack_costs::validate_hybrid_choice(symbols, &entry.hybrid_colors)?;
        }
        if !entry.kind.is_spell() {
            // CR 602.2b: an activation's hybrid symbols precede its X.
            if entry.x.is_some() && needs_choice && entry.hybrid_colors.is_empty() {
                return Err("activation announcement order invariant".into());
            }
            let changes = match entry.kind {
                StackKind::Activated { ability, .. } => {
                    &self
                        .stack_definition(entry)?
                        .activated
                        .get(ability as usize)
                        .ok_or("missing activated cost definition")?
                        .cost_changes
                }
                _ => &vec![],
            };
            return match &entry.cost_lock {
                None if changes.is_empty() => Ok(()),
                Some(lock)
                    if !changes.is_empty()
                        && lock.increases == Quantity::zero()
                        && activation_lock_consistent(changes, &lock.static_changes) =>
                {
                    Ok(())
                }
                _ => Err("activation cost lock invariant".into()),
            };
        }
        // CR 601.2b: optional additional costs, then hybrid symbols, then X are
        // announced in order, and the cost locks (CR 601.2f) only after all of
        // them. A game that ends mid-announcement keeps that prefix.
        let definition = self.stack_definition(entry)?;
        let additional_pending = !self.optional_cost_declarations(entry)?.is_empty()
            && entry.selected_additional_costs.is_none();
        let hybrid_pending = needs_choice && entry.hybrid_colors.is_empty();
        let x_pending = definition.x_mana_cost && entry.x.is_none();
        if (!entry.hybrid_colors.is_empty() && additional_pending)
            || (entry.x.is_some() && (additional_pending || hybrid_pending))
            || (entry.cost_lock.is_some() && (additional_pending || hybrid_pending || x_pending))
        {
            return Err("spell announcement order invariant".into());
        }
        if let Some(lock) = &entry.cost_lock {
            if matches!(self.underlying_mana_frame(&self.state.rules.frame)?,
                Frame::ChooseModes {cast} | Frame::AdditionalCosts {cast} | Frame::HybridCost {cast} | Frame::AnnounceX {cast} | Frame::Targets {cast} | Frame::EntityTargets {cast} | Frame::StackTargets {cast} | Frame::PlayerTargets {cast} if cast.stack_id == entry.id)
            {
                return Err("spell cost locked before announcement choices".into());
            }
            if (!matches!(entry.cast_from, Zone::Command { .. })
                && lock.increases != Quantity::zero())
                || entry.total_cost
                    != self.construct_spell_cost(
                        entry.source,
                        entry.cast_from,
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
        // Only an unpaid entry may lack its lock: payment follows the lock.
        if entry.paid.is_none()
            && (matches!(frame,
            Frame::ChooseModes {cast} | Frame::AdditionalCosts {cast} | Frame::HybridCost {cast} | Frame::AnnounceX {cast} | Frame::Targets {cast} | Frame::EntityTargets {cast} | Frame::StackTargets {cast} | Frame::PlayerTargets {cast} if cast.stack_id == entry.id)
                || matches!(frame, Frame::Fault { .. } | Frame::Finished { .. }))
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

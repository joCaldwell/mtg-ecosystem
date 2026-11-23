//! Read access shared by every rules routine: definitions, current objects,
//! the stack top and its program, and the retained-history charge.
use crate::engine::*;

impl RulesTxn {
    pub(crate) fn history_charge(&self) -> Result<usize, String> {
        // Charge actual retained allocations, independently of trace mode.
        Ok(self.state.knowledge.visibility.iter().fold(
            self.state
                .rules
                .retained_auxiliary_charge
                .saturating_add(self.kernel_charge())
                .saturating_add(self.cost_lock_charge())
                .saturating_add(self.mana_continuation_charge()?)
                .saturating_add(self.land_entry_charge())
                .saturating_add(self.replacement_charge())
                .saturating_add(self.counter_placement_charge())
                .saturating_add(self.trigger_payload_charge()),
            |n, k| n.saturating_add(k.history.retention_charge()),
        ))
    }
    pub(crate) fn check_history_capacity(&mut self) -> Result<(), RuleError> {
        let requested = self.history_charge()?;
        if requested > capacities::HISTORY_LIMIT {
            let fault = RuleFault::HistoryCapacityExceeded {
                capacity: capacities::HISTORY_LIMIT,
                requested,
            };
            return Err(RuleError::Resource {
                reason: self.capacity_error(fault.clone()),
                fault: Some(fault),
            });
        }
        Ok(())
    }
    // Dense by CardId. The row is whatever the catalog lookup itself returns, so
    // a hit below is that same lookup's answer while the Oracle IDs still agree.
    // Generated token IDs and untrusted oversized IDs stay on the lookup path.
    pub(in crate::engine) fn definition_index(
        catalog: &Catalog,
        cards: &BTreeMap<CardId, Card>,
    ) -> Arc<[u32]> {
        pub(in crate::engine) const LIMIT: usize = 1 << 16;
        let len = cards
            .keys()
            .rev()
            .map(|card| card.0 as usize)
            .find(|card| *card < LIMIT)
            .map_or(0, |card| card + 1);
        let mut index = vec![u32::MAX; len];
        let mut rows = BTreeMap::<&str, u32>::new();
        for (card, physical) in cards {
            let Some(slot) = index.get_mut(card.0 as usize) else {
                continue;
            };
            *slot = *rows.entry(&physical.oracle_id).or_insert_with(|| {
                catalog
                    .definition_by_oracle_id(&physical.oracle_id)
                    .and_then(|found| {
                        catalog
                            .definitions
                            .iter()
                            .position(|definition| std::ptr::eq(definition, found))
                    })
                    .and_then(|row| u32::try_from(row).ok())
                    .unwrap_or(u32::MAX)
            });
        }
        index.into()
    }
    /// Immutable physical card or generated token definition, independent of zone copies.
    pub(crate) fn definition(&self, card: CardId) -> Result<&Definition, RuleError> {
        if let Some(token) = self.state.rules.tokens.get(&card) {
            return Ok(&token.definition);
        }
        let physical = self
            .state
            .rules
            .cards
            .get(&card)
            .ok_or("unknown physical card")?;
        if let Some(definition) = self
            .definition_index
            .get(card.0 as usize)
            .and_then(|row| self.prepared.catalog.definitions.get(*row as usize))
            && definition.oracle_id == physical.oracle_id
        {
            return Ok(definition);
        }
        self.prepared
            .catalog
            .definition_by_oracle_id(&physical.oracle_id)
            .ok_or_else(|| "missing admitted definition".into())
    }
    pub(crate) fn current(&self, id: ObjectId) -> Result<&Object, RuleError> {
        self.state
            .rules
            .objects
            .get(&id.card)
            .filter(|o| o.id == id)
            .ok_or_else(|| "stale or absent object incarnation".into())
    }
    pub(crate) fn is_battlefield_creature(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| o.present_on_battlefield())
            && self
                .characteristics(id)
                .is_ok_and(|c| c.kind == CardKind::Creature)
    }
    pub(crate) fn top(&self) -> Result<&StackInstance, RuleError> {
        self.state
            .rules
            .stack
            .last()
            .ok_or_else(|| "missing stack frame".into())
    }
    pub(crate) fn program(&self) -> Result<&SpellProgram, RuleError> {
        let entry = self.top()?;
        if matches!(
            self.state.rules.frame,
            Frame::PlayerTargets { .. }
                | Frame::Targets { .. }
                | Frame::StackTargets { .. }
                | Frame::EntityTargets { .. }
        ) {
            if let Some(child) = self.next_mode_target(entry)? {
                return Ok(child);
            }
        }
        self.program_for(entry)
    }
    pub(crate) fn program_for<'a>(
        &'a self,
        stack: &'a StackInstance,
    ) -> Result<&'a SpellProgram, RuleError> {
        if let Some(leaf) = &self.kernel_leaf
            && leaf.invocation == stack.id
        {
            return Ok(&leaf.program);
        }
        self.original_program_for(stack)
    }

    pub(in crate::engine) fn original_program_for<'a>(
        &'a self,
        stack: &'a StackInstance,
    ) -> Result<&'a SpellProgram, RuleError> {
        if let Some(plan) = &stack.mode_subset {
            return Ok(plan.program(self.root_program_for(stack)?)?);
        }

        Ok(self.selected_program(self.root_program_for(stack)?, stack.mode)?)
    }
    pub(in crate::engine) fn root_program_for(
        &self,
        stack: &StackInstance,
    ) -> Result<&SpellProgram, RuleError> {
        let definition = self.copiable_definition(stack.source)?;
        match &stack.kind {
            StackKind::Spell if stack.alternative_cost == Some(AlternativeCost::Bestow) => {
                Ok(mtg_contracts::bestow_alternative(definition)
                    .map(|(_, program)| program)
                    .ok_or_else(|| "missing bestow cast program".to_owned())?)
            }
            StackKind::Spell => Ok(definition
                .spell
                .as_ref()
                .ok_or_else(|| "missing spell program".to_string())?),
            StackKind::Triggered { ability, .. } => match *ability {
                EffectiveTriggerAbility::DelayedChapter { chapter, .. } => {
                    Ok(self.delayed_chapter_program(stack.source, chapter)?)
                }
                EffectiveTriggerAbility::Intrinsic { index } => Ok(definition
                    .triggered
                    .get(index as usize)
                    .map(|a| &a.program)
                    .ok_or_else(|| "missing triggered program".to_string())?),
            },
            StackKind::Activated { ability, .. } => Ok(self
                .activation_definition(stack.source)?
                .activated
                .get(*ability as usize)
                .map(|ability| &ability.program)
                .ok_or_else(|| "missing activated program".to_string())?),
        }
    }
}

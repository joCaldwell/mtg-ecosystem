//! CR 615.7 finite prevention allocation across simultaneous source portions.
//! Selection chooses a shield/group; the anchor does not reserve its source.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageAllocationCandidate {
    pub damage: DamageId,
    pub amount: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleDamageAllocationCandidate {
    pub damage: DamageHandle,
    pub source: VisibleTarget,
    pub oracle_id: String,
    pub amount: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageAllocationRecord {
    pub effect: EffectKey,
    pub allocations: Vec<(DamageId, Quantity)>,
    pub application_start: u32,
    pub application_count: u32,
    pub budget_before: Quantity,
    pub budget_after: Quantity,
}
impl Game {
    pub(super) fn allocation_options(
        &self,
        tx: &DamageTransaction,
        effect: &EffectKey,
    ) -> Result<
        (
            PlayerId,
            TargetEntity,
            Quantity,
            Quantity,
            Vec<DamageAllocationCandidate>,
        ),
        String,
    > {
        let EffectKey::Resolved { id } = effect else {
            return Err("allocation effect is not finite shield".into());
        };
        let Some(ResolvedEffect {
            kind: ResolvedEffectKind::PreventNextDamage { target, remaining },
            ..
        }) = self.state.effects.iter().find(|e| e.id == *id)
        else {
            return Err("missing allocation shield".into());
        };
        if remaining <= &Quantity::zero() {
            return Err("exhausted allocation shield".into());
        }
        let candidates = tx
            .entries
            .iter()
            .filter(|d| {
                d.recipient.entity() == *target
                    && d.amount > Quantity::zero()
                    && !d.applied.contains(effect)
            })
            .map(|d| DamageAllocationCandidate {
                damage: d.id,
                amount: d.amount.clone(),
            })
            .collect::<Vec<_>>();
        let first = candidates.first().ok_or("empty allocation group")?;
        let recipient = &tx
            .entries
            .iter()
            .find(|d| d.id == first.damage)
            .ok_or("missing allocation occurrence")?
            .recipient;
        let player = self
            .recipient_chooser(recipient)
            .ok_or("departed allocation recipient")?;
        let mut total = Quantity::zero();
        for c in &candidates {
            total.add(&c.amount)
        }
        let required = std::cmp::min(remaining.clone(), total);
        Ok((player, *target, remaining.clone(), required, candidates))
    }
    pub(super) fn needs_allocation(&self, tx: &DamageTransaction, effect: &EffectKey) -> bool {
        self.prepared.config.choices_profile()
            && self
                .allocation_options(tx, effect)
                .is_ok_and(|(_, _, remaining, _, c)| remaining > Quantity::from(1) && c.len() > 1)
    }
    pub(super) fn damage_allocation_request(&self) -> Result<Option<Request>, String> {
        let Frame::DamageAllocation {
            transaction,
            effect,
        } = &self.state.frame
        else {
            return Ok(None);
        };
        let Ok((player, recipient, remaining, required, candidates)) =
            self.allocation_options(transaction, effect)
        else {
            return Ok(None);
        };
        Ok(Some(Request::DamageAllocation {
            decision_id: self.state.revision,
            player,
            recipient,
            effect: effect.clone(),
            remaining,
            required,
            candidates,
        }))
    }
    pub(super) fn validate_damage_allocation(
        &self,
        player: PlayerId,
        allocations: &[(DamageId, Quantity)],
    ) -> Result<(), String> {
        let Some(Request::DamageAllocation {
            player: expected,
            required,
            candidates,
            ..
        }) = self.damage_allocation_request()?
        else {
            return Err("no allocation choice".into());
        };
        if player != expected || allocations.len() != candidates.len() {
            return Err("wrong allocation chooser/cardinality".into());
        }
        let mut seen = BTreeSet::new();
        let mut sum = Quantity::zero();
        for (id, amount) in allocations {
            let candidate = candidates
                .iter()
                .find(|c| c.damage == *id)
                .ok_or("foreign allocation occurrence")?;
            if !seen.insert(*id) || amount.is_negative() || *amount > candidate.amount {
                return Err("invalid allocation quantity/duplicate".into());
            }
            sum.add(amount);
        }
        if sum != required {
            return Err("allocation must prevent the required next damage".into());
        }
        Ok(())
    }
    pub(super) fn damage_allocation_step(&mut self) -> Result<(), String> {
        let Frame::DamageAllocation {
            transaction,
            effect,
        } = self.state.frame.clone()
        else {
            return Err("allocation frame invariant".into());
        };
        if self.damage_allocation_request()?.is_none() {
            self.state.pending = None;
            self.state.frame = Frame::DamageEffects { transaction };
            return Ok(());
        }
        let input = self
            .state
            .pending
            .take()
            .ok_or("missing allocation vector")?;
        let Response::DamageAllocation { mut allocations } = input.response else {
            return Err("allocation response invariant".into());
        };
        self.validate_damage_allocation(input.player, &allocations)?;
        allocations.sort_by_key(|(id, _)| *id);
        self.commit(
            ProposedEvent::AllocateDamageEffect {
                transaction: transaction.id,
                effect,
                allocations,
            },
            transaction.cause,
        )?;
        let Frame::DamageAllocation { transaction, .. } = self.state.frame.clone() else {
            return Err("allocation event changed frame".into());
        };
        self.state.frame = Frame::DamageEffects { transaction };
        Ok(())
    }
    pub(super) fn apply_damage_allocation(
        &mut self,
        id: DamageTransactionId,
        effect: &EffectKey,
        allocations: &[(DamageId, Quantity)],
    ) -> Result<EventOutcome, String> {
        let Frame::DamageAllocation {
            transaction: tx,
            effect: selected,
        } = &self.state.frame
        else {
            return Err("allocation outside selected shield frame".into());
        };
        if tx.id != id || selected != effect {
            return Err("wrong allocation identity".into());
        }
        self.validate_damage_transaction(tx)?;
        if !self.needs_allocation(tx, effect) || allocations.windows(2).any(|w| w[0].0 >= w[1].0) {
            return Err("allocation requires canonical multi-source selection".into());
        }
        let (player, _, remaining, _, _) = self.allocation_options(tx, effect)?;
        self.validate_damage_allocation(player, allocations)?;
        let mut next_budget = remaining.clone();
        let mut applications = vec![];
        for (damage, prevented) in allocations {
            if *prevented == Quantity::zero() {
                continue;
            }
            let d = tx
                .entries
                .iter()
                .find(|d| d.id == *damage)
                .ok_or("missing allocated occurrence")?;
            let mut after = d.amount.clone();
            after.subtract(prevented);
            let before_budget = next_budget.clone();
            next_budget.subtract(prevented);
            applications.push(DamageApplication {
                damage: *damage,
                effect: effect.clone(),
                before: d.amount.clone(),
                after,
                prevented: prevented.clone(),
                budget_before: Some(before_budget),
                budget_after: Some(next_budget.clone()),
            });
        }
        let record = DamageAllocationRecord {
            effect: effect.clone(),
            allocations: allocations.to_vec(),
            application_start: u32::try_from(tx.applications.len())
                .map_err(|_| "allocation range limit")?,
            application_count: u32::try_from(applications.len())
                .map_err(|_| "allocation count limit")?,
            budget_before: remaining,
            budget_after: next_budget.clone(),
        };
        let EffectKey::Resolved { id } = effect else {
            return Err("nonfinite allocated effect".into());
        };
        let Some(ResolvedEffect {
            kind: ResolvedEffectKind::PreventNextDamage { remaining, .. },
            ..
        }) = self.state.effects.iter_mut().find(|e| e.id == *id)
        else {
            return Err("missing allocated shield".into());
        };
        *remaining = next_budget;
        let Frame::DamageAllocation {
            transaction: tx, ..
        } = &mut self.state.frame
        else {
            return Err("missing allocation transaction".into());
        };
        for a in &applications {
            let d = tx
                .entries
                .iter_mut()
                .find(|d| d.id == a.damage)
                .ok_or("missing allocated occurrence")?;
            d.amount = a.after.clone();
            d.applied.push(effect.clone());
        }
        tx.applications.extend(applications.clone());
        tx.allocation_groups.push(record);
        Ok(EventOutcome::DamageEffectAllocated {
            transaction: tx.id,
            effect: effect.clone(),
            allocations: allocations.to_vec(),
            applications,
        })
    }
    pub(super) fn visible_allocation_candidate(
        &self,
        viewer: PlayerId,
        c: &DamageAllocationCandidate,
    ) -> Result<VisibleDamageAllocationCandidate, String> {
        let tx = self.damage_transaction()?;
        let d = tx
            .entries
            .iter()
            .find(|d| d.id == c.damage)
            .ok_or("unknown allocation occurrence")?;
        Ok(VisibleDamageAllocationCandidate {
            damage: self.damage_handle(viewer, tx.id, c.damage)?,
            source: VisibleTarget {
                announced: d
                    .source_handles
                    .get(viewer.0 as usize)
                    .ok_or("missing allocation source reference")?
                    .clone(),
                current: self.handle_for(viewer, d.source.object).ok(),
            },
            oracle_id: self
                .state
                .cards
                .get(&d.source.object.card)
                .ok_or("missing allocation source card")?
                .oracle_id
                .clone(),
            amount: c.amount.clone(),
        })
    }
    pub(super) fn resolve_allocation_response(
        &self,
        viewer: PlayerId,
        allocations: &[(DamageHandle, Quantity)],
    ) -> Result<Vec<(DamageId, Quantity)>, String> {
        let mut result = vec![];
        for (handle, amount) in allocations {
            result.push((self.resolve_damage_handle(viewer, handle)?, amount.clone()));
        }
        self.validate_damage_allocation(viewer, &result)?;
        Ok(result)
    }
}
impl Game {
    pub(super) fn damage_handle(
        &self,
        viewer: PlayerId,
        transaction: DamageTransactionId,
        damage: DamageId,
    ) -> Result<DamageHandle, String> {
        self.state
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| {
                k.damage_handles
                    .iter()
                    .find(|(id, _)| *id == (transaction, damage))
                    .map(|(_, h)| h.clone())
            })
            .ok_or_else(|| "unknown damage handle".into())
    }
    pub(super) fn resolve_damage_handle(
        &self,
        viewer: PlayerId,
        handle: &DamageHandle,
    ) -> Result<DamageId, String> {
        let id = self.damage_transaction()?.id;
        self.state
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| {
                k.damage_handles
                    .iter()
                    .find(|((tx, _), h)| *tx == id && h == handle)
                    .map(|((_, d), _)| *d)
            })
            .ok_or_else(|| "foreign/stale damage handle".into())
    }
}
impl Game {
    pub(super) fn validate_allocation_prefix_group(
        &self,
        tx: &DamageTransaction,
        group: &DamageAllocationRecord,
        amounts: &BTreeMap<DamageId, Quantity>,
        used: &BTreeMap<DamageId, Vec<EffectKey>>,
    ) -> Result<(), String> {
        let EffectKey::Resolved { id } = group.effect else {
            return Err("nonfinite allocation prefix".into());
        };
        let effect = self
            .state
            .effects
            .iter()
            .find(|e| e.id == id)
            .ok_or("missing allocation prefix shield")?;
        let ResolvedEffectKind::PreventNextDamage { target, .. } = &effect.kind else {
            return Err("wrong allocated effect kind".into());
        };
        if group.allocations.len() > 24
            || group.budget_before <= Quantity::from(1)
            || group.budget_before > self.shield_limit(effect)?
            || group.allocations.windows(2).any(|w| w[0].0 >= w[1].0)
        {
            return Err("allocation source budget/order invariant".into());
        }
        let candidates = tx
            .entries
            .iter()
            .filter(|d| {
                d.recipient.entity() == *target
                    && amounts.get(&d.id).is_some_and(|n| *n > Quantity::zero())
                    && !used.get(&d.id).is_some_and(|u| u.contains(&group.effect))
            })
            .map(|d| d.id)
            .collect::<Vec<_>>();
        if candidates.len() < 2
            || candidates
                != group
                    .allocations
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>()
        {
            return Err("allocation prefix candidate coverage invariant".into());
        }
        let mut total = Quantity::zero();
        let mut sum = Quantity::zero();
        let mut budget = group.budget_before.clone();
        let mut expected = vec![];
        for (id, prevention) in &group.allocations {
            let before = amounts
                .get(id)
                .ok_or("unknown allocated prefix occurrence")?;
            if prevention.is_negative() || prevention > before {
                return Err("allocated prefix quantity outside original damage".into());
            }
            total.add(before);
            sum.add(prevention);
            if *prevention == Quantity::zero() {
                continue;
            }
            let mut after = before.clone();
            after.subtract(prevention);
            let previous = budget.clone();
            budget.subtract(prevention);
            expected.push(DamageApplication {
                damage: *id,
                effect: group.effect.clone(),
                before: before.clone(),
                after,
                prevented: prevention.clone(),
                budget_before: Some(previous),
                budget_after: Some(budget.clone()),
            });
        }
        if sum != std::cmp::min(group.budget_before.clone(), total)
            || budget != group.budget_after
            || expected.len() != group.application_count as usize
        {
            return Err("allocation prefix required consumption invariant".into());
        }
        let start = group.application_start as usize;
        let end = start
            .checked_add(expected.len())
            .ok_or("allocation prefix range overflow")?;
        if tx.applications.get(start..end) != Some(expected.as_slice()) {
            return Err("allocation vector/application prefix differs".into());
        }
        Ok(())
    }
}

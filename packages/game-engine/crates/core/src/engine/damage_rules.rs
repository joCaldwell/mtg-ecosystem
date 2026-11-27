//! CR 609.7a, 614.1a, 615 and 616.1: static damage rules and resolved
//! prevention effects as candidates of a prepared damage transaction.
//!
//! A static prevention or damage replacement is one candidate per live
//! static ability; a resolved prevention is one candidate per effect row.
//! Each applies at most once to each damage event, in the order the
//! affected recipient's chooser picks (CR 616.1e-f). Matching sources are
//! read from the source's damage-time facts (CR 609.7a); recipients from
//! the current battlefield. Damage that can't be prevented (CR 615.12)
//! offers no prevention candidate: no admitted prevention has an
//! additional effect, and shields are not reduced by such damage.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{
    Atom, Cmp, DamageEvent, DamageKindFilter, DamageModification, DamageRecipients, Pred,
    PreventionAmount, PreventionShield, PreventionSources, Ref, ResolvedPrevention, Scope,
    StaticRule,
};

/// CR 609.7a, 615.1: the sources whose damage a resolved prevention
/// prevents: one announced incarnation, or every source matching `filter`
/// when the damage would be dealt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sources", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreventedSources {
    One { object: ObjectId },
    Matching { filter: Pred },
}

fn kind_matches(filter: DamageKindFilter, kind: DamageKind) -> bool {
    match filter {
        DamageKindFilter::All => true,
        DamageKindFilter::Combat => kind == DamageKind::Combat,
        DamageKindFilter::Noncombat => kind == DamageKind::Noncombat,
    }
}

fn same_incarnation(left: ObjectId, right: ObjectId) -> bool {
    left.card == right.card && left.incarnation == right.incarnation
}

fn compare(left: &Quantity, cmp: Cmp, right: &Quantity) -> bool {
    match cmp {
        Cmp::Exactly => left == right,
        Cmp::AtMost => left <= right,
        Cmp::AtLeast => left >= right,
    }
}

/// CR 615.1, 615.10: the damage a static prevention prevents from one event.
pub(super) fn static_prevented(
    prevent: &PreventionAmount,
    before: &Quantity,
) -> Result<Quantity, String> {
    Ok(match prevent {
        PreventionAmount::All => before.clone(),
        PreventionAmount::UpTo { amount } => before.clone().min(amount.clone()),
        PreventionAmount::Half { round_up } => before.halved(*round_up),
    })
}

/// CR 614.1a: the replaced amount of one damage event.
pub(super) fn modified_damage(change: &DamageModification, before: &Quantity) -> Quantity {
    let mut after = before.clone();
    match change {
        DamageModification::Multiply { factor } => after.multiply(factor),
        DamageModification::Add { amount } => after.add(amount),
    }
    after
}

impl RulesTxn {
    /// The live static damage rule at `index` of a battlefield permanent's
    /// static abilities, if its condition holds (CR 604.1, 611.3a).
    pub(super) fn live_damage_rule(
        &self,
        source: ObjectId,
        index: u32,
    ) -> Option<(&Object, &StaticRule)> {
        let holder = self
            .battlefield_static_sources(None)
            .find(|o| o.id == source)?;
        self.held_damage_rule(holder, index)
            .map(|rule| (holder, rule))
    }

    fn held_damage_rule<'a>(&'a self, holder: &Object, index: u32) -> Option<&'a StaticRule> {
        // CR 113.10, 305.7: a holder without its abilities holds no rule.
        if self.printed_abilities_lost(holder.id) {
            return None;
        }
        let ability = self
            .copiable_definition(holder.id)
            .ok()?
            .static_abilities
            .get(index as usize)?;
        if ability.functioning_zone != FunctioningZone::Battlefield {
            return None;
        }
        let StaticEffect::Rule { rule, condition } = &ability.effect else {
            return None;
        };
        if condition
            .as_ref()
            .is_some_and(|c| !self.static_condition(holder, c))
        {
            return None;
        }
        matches!(
            rule,
            StaticRule::PreventDamage { .. }
                | StaticRule::ModifyDamage { .. }
                | StaticRule::DamageCantBePrevented { .. }
        )
        .then_some(rule)
    }

    fn live_damage_rules(&self) -> impl Iterator<Item = (ObjectId, u32, &Object, &StaticRule)> {
        self.battlefield_static_sources(None)
            .flat_map(move |holder| {
                let count = self
                    .copiable_definition(holder.id)
                    .map(|d| d.static_abilities.len())
                    .unwrap_or(0);
                (0..count as u32).filter_map(move |index| {
                    self.held_damage_rule(holder, index)
                        .map(|rule| (holder.id, index, holder, rule))
                })
            })
    }

    /// The candidate keys of live static preventions and damage
    /// replacements (CR 616.1).
    pub(super) fn static_damage_keys(&self) -> Vec<EffectKey> {
        self.live_damage_rules()
            .filter(|(.., rule)| {
                matches!(
                    rule,
                    StaticRule::PreventDamage { .. } | StaticRule::ModifyDamage { .. }
                )
            })
            .map(|(source, ability, ..)| EffectKey::StaticAbility { source, ability })
            .collect()
    }

    /// CR 609.7a: a damage source's damage-time facts against a filter
    /// whose "you" is `you`.
    pub(super) fn damage_fact_matches(
        &self,
        filter: &Pred,
        you: PlayerId,
        holder: Option<ObjectId>,
        fact: &ObjectBefore,
    ) -> bool {
        let c = &fact.characteristics;
        let stat = |value: &Option<Quantity>, cmp: &Cmp, bound: &mtg_contracts::kir::Amount| {
            value
                .as_ref()
                .zip(bound.as_fixed())
                .is_some_and(|(value, bound)| compare(value, *cmp, bound))
        };
        filter.0.iter().all(|atom| match atom {
            Atom::Type { kind } => c.has_type(*kind),
            Atom::NotType { kind } => !c.has_type(*kind),
            Atom::AnyType { kinds } => kinds.iter().any(|k| c.has_type(*k)),
            Atom::Subtype { subtype } => c.has_subtype(subtype),
            Atom::NotSubtype { subtype } => !c.has_subtype(subtype),
            Atom::Basic => c.basic,
            Atom::NotBasic => !c.basic,
            Atom::Legendary => c.legendary,
            Atom::NotLegendary => !c.legendary,
            Atom::Color { color } => c.colors.contains(color),
            Atom::NotColor { color } => !c.colors.contains(color),
            Atom::ControlledBy { player } => self.static_player(player, you, fact.controller),
            Atom::OwnedBy { player } => self.static_player(player, you, fact.owner),
            Atom::Token => self.state.rules.tokens.contains_key(&fact.object.card),
            Atom::NotToken => !self.state.rules.tokens.contains_key(&fact.object.card),
            Atom::Tapped => fact.tapped,
            Atom::Untapped => !fact.tapped,
            Atom::Attacking => self
                .state
                .rules
                .combat
                .iter()
                .any(|attack| same_incarnation(attack.attacker, fact.object)),
            Atom::Blocking => self
                .state
                .rules
                .blocking_creatures
                .iter()
                .any(|blocker| same_incarnation(*blocker, fact.object)),
            Atom::ManaValue { cmp, value } => value
                .as_fixed()
                .is_some_and(|bound| compare(&c.printed_mana_value(), *cmp, bound)),
            Atom::Power { cmp, value } => stat(&c.power, cmp, value),
            Atom::Toughness { cmp, value } => stat(&c.toughness, cmp, value),
            Atom::Keyword { keyword } => fact.keywords.contains(keyword),
            Atom::NotKeyword { keyword } => !fact.keywords.contains(keyword),
            Atom::IsNot { object } => {
                *object == Ref::Source
                    && holder.is_some_and(|holder| !same_incarnation(holder, fact.object))
            }
            Atom::AnyOf { alternatives } => alternatives
                .iter()
                .any(|p| self.damage_fact_matches(p, you, holder, fact)),
            Atom::Is { .. } => false,
        })
    }

    fn damage_source_in(
        &self,
        scope: &Scope,
        holder: &Object,
        d: &PreparedDamage,
    ) -> Result<bool, String> {
        Ok(match scope {
            Scope::Source => same_incarnation(d.source.object, holder.id),
            Scope::Attachment { filter, .. } => {
                holder
                    .attached_to
                    .is_some_and(|host| same_incarnation(host, d.source.object))
                    && self.damage_fact_matches(
                        filter,
                        holder.controller,
                        Some(holder.id),
                        &self.damage_source_appearance(d)?,
                    )
            }
            Scope::Matching { filter } => self.damage_fact_matches(
                filter,
                holder.controller,
                Some(holder.id),
                &self.damage_source_appearance(d)?,
            ),
        })
    }

    fn recipient_in(
        &self,
        recipients: &DamageRecipients,
        holder: &Object,
        recipient: &DamageRecipient,
    ) -> bool {
        match *recipient {
            DamageRecipient::Player { player } => recipients
                .players
                .as_ref()
                .is_some_and(|who| self.static_player(who, holder.controller, player)),
            DamageRecipient::Permanent { object } => recipients
                .permanents
                .as_ref()
                .is_some_and(|scope| self.declaration_scope(scope, holder, object)),
        }
    }

    /// CR 609.7a, 120.1: whether a static damage rule's event matches one
    /// prepared damage event now.
    fn damage_event_matches(
        &self,
        event: &DamageEvent,
        holder: &Object,
        kind: DamageKind,
        d: &PreparedDamage,
    ) -> Result<bool, String> {
        if !kind_matches(event.kind, kind) {
            return Ok(false);
        }
        if let Some(by) = &event.by
            && !self.damage_source_in(by, holder, d)?
        {
            return Ok(false);
        }
        if let Some(to) = &event.to
            && !self.recipient_in(to, holder, &d.recipient)
        {
            return Ok(false);
        }
        if let Some(scope) = &event.involving {
            let to = matches!(d.recipient, DamageRecipient::Permanent { object }
                if self.declaration_scope(scope, holder, object));
            if !to && !self.damage_source_in(scope, holder, d)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// CR 615.12: a live static says this damage can't be prevented.
    pub(super) fn damage_unpreventable(
        &self,
        kind: DamageKind,
        d: &PreparedDamage,
    ) -> Result<bool, String> {
        for (.., holder, rule) in self.live_damage_rules() {
            if let StaticRule::DamageCantBePrevented { event } = rule
                && self.damage_event_matches(event, holder, kind, d)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// CR 615.1, 615.10, 614.1a: the amount a live static prevention or
    /// damage replacement leaves of `d`, and the damage it prevented.
    pub(super) fn static_damage_application(
        &self,
        kind: DamageKind,
        d: &PreparedDamage,
        source: ObjectId,
        ability: u32,
    ) -> Result<(Quantity, Quantity), String> {
        let (holder, rule) = self
            .live_damage_rule(source, ability)
            .ok_or("static damage rule no longer applies")?;
        let event = match rule {
            StaticRule::PreventDamage { event, .. } | StaticRule::ModifyDamage { event, .. } => {
                event
            }
            _ => return Err("not a damage candidate".into()),
        };
        if !self.damage_event_matches(event, holder, kind, d)? {
            return Err("static damage rule does not match".into());
        }
        Self::static_rule_amounts(rule, &d.amount)
    }

    /// The `(after, prevented)` pair a matching static damage rule yields.
    pub(super) fn static_rule_amounts(
        rule: &StaticRule,
        before: &Quantity,
    ) -> Result<(Quantity, Quantity), String> {
        match rule {
            StaticRule::PreventDamage { prevent, .. } => {
                let prevented = static_prevented(prevent, before)?;
                if prevented <= Quantity::zero() {
                    return Err("static prevention prevents nothing".into());
                }
                let mut after = before.clone();
                after.subtract(&prevented);
                Ok((after, prevented))
            }
            StaticRule::ModifyDamage { change, .. } => {
                Ok((modified_damage(change, before), Quantity::zero()))
            }
            _ => Err("not a damage candidate".into()),
        }
    }

    /// CR 615.7: the shield's damage this transaction has already used.
    pub(super) fn shield_used(tx: &DamageTransaction, key: &EffectKey) -> Quantity {
        let mut used = Quantity::zero();
        for a in tx.applications.iter().filter(|a| a.effect == *key) {
            used.add(&a.prevented);
        }
        used
    }

    /// CR 615.1, 615.7: the `(after, prevented)` pair of a resolved
    /// prevention effect for `d`.
    pub(super) fn resolved_prevention_application(
        &self,
        tx: &DamageTransaction,
        d: &PreparedDamage,
        effect: &ResolvedEffect,
    ) -> Result<(Quantity, Quantity), String> {
        let ResolvedEffectKind::PreventDamage {
            kind,
            to,
            by,
            remaining,
        } = &effect.kind
        else {
            return Err("not a prevention effect".into());
        };
        if !kind_matches(*kind, tx.kind)
            || to.as_ref().is_some_and(|to| *to != d.recipient.entity())
        {
            return Err("prevention does not match".into());
        }
        let source = match by {
            None => true,
            Some(PreventedSources::One { object }) => same_incarnation(*object, d.source.object),
            Some(PreventedSources::Matching { filter }) => self.damage_fact_matches(
                filter,
                effect.origin.controller,
                None,
                &self.damage_source_appearance(d)?,
            ),
        };
        if !source {
            return Err("prevention does not apply".into());
        }
        let prevented = match remaining {
            None => d.amount.clone(),
            Some(remaining) => {
                let mut left = remaining.clone();
                left.subtract(&Self::shield_used(
                    tx,
                    &EffectKey::Resolved { id: effect.id },
                ));
                left.min(d.amount.clone())
            }
        };
        if prevented <= Quantity::zero() {
            return Err("prevention shield is used up".into());
        }
        let mut after = d.amount.clone();
        after.subtract(&prevented);
        Ok((after, prevented))
    }

    /// CR 615.1, 615.7, 608.2b: the prevention effect a resolving
    /// instruction creates, or none when its announced object is gone.
    pub(super) fn prevention_kind(
        &self,
        frame: &Resolving,
        prevention: &ResolvedPrevention,
    ) -> Result<Option<ResolvedEffectKind>, String> {
        let entry = self.entry(frame.stack_id)?;
        let legal = |slot: u32| frame.legal_targets.get(slot as usize) == Some(&true);
        let to = match &prevention.to {
            None => None,
            Some(Ref::Target { slot }) => {
                if !legal(*slot) {
                    return Ok(None);
                }
                let target = self.directed_damage_target(entry, *slot)?;
                let present = match target {
                    TargetEntity::Object { object } => self.is_battlefield_creature(object),
                    TargetEntity::Player { player } => self.is_live(player),
                };
                if !present {
                    return Ok(None);
                }
                Some(target)
            }
            Some(_) => return Err("unaudited shielded recipient".into()),
        };
        let by = match &prevention.by {
            None => None,
            Some(PreventionSources::One {
                object: Ref::Target { slot },
            }) => {
                let object = *entry
                    .targets
                    .get(*slot as usize)
                    .ok_or("missing prevented source target")?;
                if !legal(*slot) || !self.is_battlefield_creature(object) {
                    return Ok(None);
                }
                Some(PreventedSources::One { object })
            }
            Some(PreventionSources::Matching { filter }) => Some(PreventedSources::Matching {
                filter: filter.clone(),
            }),
            Some(_) => return Err("unaudited prevented source".into()),
        };
        Ok(Some(ResolvedEffectKind::PreventDamage {
            kind: prevention.kind,
            to,
            by,
            remaining: match &prevention.shield {
                PreventionShield::All => None,
                PreventionShield::Next { amount } => Some(amount.clone()),
            },
        }))
    }

    /// The announced object a prevention effect records handles for.
    pub(super) fn prevention_object(kind: &ResolvedEffectKind) -> Option<ObjectId> {
        match kind {
            ResolvedEffectKind::PreventDamage {
                to: Some(TargetEntity::Object { object }),
                ..
            }
            | ResolvedEffectKind::PreventDamage {
                by: Some(PreventedSources::One { object }),
                ..
            } => Some(*object),
            _ => None,
        }
    }

    pub(in crate::engine) fn kernel_prevent_damage(
        &mut self,
        frame: Resolving,
        prevention: &ResolvedPrevention,
        source_block: u32,
    ) -> Result<(), String> {
        match self.prevention_kind(&frame, prevention)? {
            Some(kind) => self.commit_resolved_effect(frame, kind, source_block),
            None => self.finish_instruction(frame, InstructionOutcome::Effect { actual: None }),
        }
    }

    /// CR 615.7: committed damage reduces each shield by what it prevented;
    /// a shield reduced to 0 is used up.
    pub(super) fn consume_prevention_shields(
        &mut self,
        tx: &DamageTransaction,
    ) -> Result<(), String> {
        let mut used: BTreeMap<EffectId, Quantity> = BTreeMap::new();
        for a in &tx.applications {
            if let EffectKey::Resolved { id } = a.effect {
                used.entry(id).or_default().add(&a.prevented);
            }
        }
        for (id, amount) in used {
            let effect = self
                .rules_mut()
                .effects
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or("missing applied prevention effect")?;
            if let ResolvedEffectKind::PreventDamage {
                remaining: Some(remaining),
                ..
            } = &mut effect.kind
            {
                remaining.subtract(&amount);
                if remaining.is_negative() {
                    return Err("prevention shield overdrawn".into());
                }
            }
        }
        self.rules_mut().effects.retain(|e| {
            !matches!(&e.kind, ResolvedEffectKind::PreventDamage { remaining: Some(r), .. }
                if *r == Quantity::zero())
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CR 615.1: halving is exact above every machine width.
    #[test]
    fn half_prevention_is_exact_for_large_damage() {
        let q = |s: &str| s.parse::<Quantity>().unwrap();
        for (before, round_up, prevented) in [
            ("4294967297", true, "2147483649"),
            ("4294967297", false, "2147483648"),
            ("18446744073709551617", true, "9223372036854775809"),
            ("18446744073709551617", false, "9223372036854775808"),
        ] {
            let amount = PreventionAmount::Half { round_up };
            assert_eq!(static_prevented(&amount, &q(before)).unwrap(), q(prevented));
        }
    }
}

//! Strategy over public IR and permitted observations. These are deliberately
//! coarse value estimates; only core decides legality and executes rules.
use super::*;
use mtg_contracts::{
    ActivationCost, CreatureRestriction, Definition, Instruction as I, KeywordAbility,
    LifeRecipient, ObjectSet, SacrificeDamageAmount, SacrificeFilter, SpellProgram,
    SpellResolution, SpellTiming, StaticEffect, TargetDomain, TargetSpec, WipeAmount,
};
use mtg_core::{PermittedView, StackHandle, VisibleEntityTarget, VisibleObject, VisibleStackEntry};

fn zero_cost() -> ManaCost {
    ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    }
}
fn mana_value(cost: &ManaCost) -> Quantity {
    let mut q = cost.generic.clone();
    q.add(&Quantity::from(cost.colored.len() as i64));
    q
}
fn program_mode(p: &SpellProgram, mode: Option<u32>) -> &SpellProgram {
    mode.and_then(|m| p.modes.get(m as usize))
        .map(|m| &m.program)
        .unwrap_or(p)
}
fn entity_matches(visible: &VisibleEntityTarget, target: &PlayerTargetEntity) -> bool {
    match (visible, target) {
        (VisibleEntityTarget::Player { player: a }, PlayerTargetEntity::Player { player: b }) => {
            a == b
        }
        (VisibleEntityTarget::Object { object: a }, PlayerTargetEntity::Object { object: b }) => {
            a.current.as_ref() == Some(b)
        }
        _ => false,
    }
}
fn blocking_pump_program(p: &SpellProgram) -> bool {
    p.modes.is_empty()
        && !p.targets.is_empty()
        && p.targets
            .iter()
            .all(|t| t.domain == TargetDomain::BlockingBattlefieldCreature)
        && !p.instructions.is_empty()
        && p.instructions.iter().all(|i| {
            matches!(i, I::ModifyPowerToughness {power, toughness, ..}
                if !power.is_negative() && !toughness.is_negative()
                    && (*power > Quantity::zero() || *toughness > Quantity::zero()))
        })
}
fn reactive_program(p: &SpellProgram) -> bool {
    blocking_pump_program(p)
        || p.additional_sacrifice.is_some()
        || p.instructions.iter().any(|i| {
            matches!(
                i,
                I::CounterStackObject { .. }
                    | I::MoveSet {
                        set: ObjectSet::AttackingCreatures,
                        ..
                    }
                    | I::PreventCombatDamage { .. }
                    | I::PreventNextDamage { .. }
            )
        })
        || p.modes.iter().any(|m| reactive_program(&m.program))
}
fn tapped_destroy_spell(d: &Definition, p: &SpellProgram) -> bool {
    d.characteristics.kind == CardKind::Instant
        && d.spell.as_ref() == Some(p)
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.modes.is_empty()
        && p.additional_sacrifice.is_none()
        && !p.targets.is_empty()
        && p.targets.iter().all(|t| {
            t.domain == TargetDomain::BattlefieldPermanent
                && t.filter.as_ref().is_some_and(|f| f.tapped)
        })
        && !p.instructions.is_empty()
        && p.instructions.iter().all(|i| {
            matches!(i, I::DestroyPermanent { target_slot, optional: false, .. }
                if p.targets.iter().any(|t| t.slot == *target_slot))
        })
}
impl Strategy<'_> {
    pub(super) fn legend_choice(
        &self,
        candidates: &[ObjectHandle],
    ) -> Result<PlayerResponse, String> {
        Ok(PlayerResponse::LegendRule {
            keep: candidates.last().ok_or("empty legend choices")?.clone(),
        })
    }

    pub(super) fn reserve_reactive_tap(&self, o: &VisibleObject) -> bool {
        definition(self.catalog, &o.oracle_id).is_ok_and(|d| {
            d.activated.iter().any(|a| {
                let tap = match &a.cost {
                    ActivationCost::TapSource => true,
                    ActivationCost::Composite { costs } => costs.tap,
                    _ => false,
                };
                tap && a.program.timing == SpellTiming::Instant && reactive_program(&a.program)
            })
        })
    }
    fn entry_program<'a>(
        &'a self,
        v: &PermittedView,
        s: &VisibleStackEntry,
    ) -> Option<(&'a Definition, &'a SpellProgram)> {
        let (d, p) = if let Some(t) = &s.triggered {
            let d = definition(self.catalog, &t.oracle_id).ok()?;
            (d, &d.triggered.get(t.ability as usize)?.program)
        } else if let Some(a) = &s.ability {
            let d = definition(self.catalog, &a.oracle_id).ok()?;
            (d, &d.activated.get(a.ability as usize)?.program)
        } else {
            let o = v
                .objects
                .iter()
                .find(|o| Some(&o.handle) == s.spell.as_ref())?;
            let d = definition(self.catalog, &o.oracle_id).ok()?;
            (d, d.spell.as_ref()?)
        };
        Some((d, program_mode(p, s.mode)))
    }
    pub(super) fn top_program<'a>(
        &'a self,
        v: &PermittedView,
    ) -> Option<(&'a Definition, &'a SpellProgram)> {
        self.entry_program(v, v.stack.last()?)
    }
    fn targetable(&self, v: &PermittedView, o: &VisibleObject) -> bool {
        !o.keywords.contains(&KeywordAbility::Shroud)
            && (o.controller == v.viewer || !o.keywords.contains(&KeywordAbility::Hexproof))
    }
    fn value(&self, o: &VisibleObject) -> Quantity {
        let Some(c) = &o.characteristics else {
            return Quantity::zero();
        };
        let mut n = mana_value(&c.mana_cost);
        n.add(&c.power.clone().unwrap_or_default().max(Quantity::zero()));
        n.add(&1.into());
        n
    }
    fn harmful_attachment(&self, d: &Definition) -> bool {
        d.static_abilities.iter().any(|a| matches!(&a.effect,
            StaticEffect::ModifyCreatures { power, toughness, restrictions, .. }
            if power.is_negative() || toughness.is_negative() || restrictions.contains(&CreatureRestriction::CantAttackOrBlock)))
    }
    pub(super) fn target_beneficial(&self, d: &Definition, p: &SpellProgram, slot: u32) -> bool {
        if d.characteristics.subtypes.iter().any(|s| s == "Aura") {
            return !self.harmful_attachment(d);
        }
        p.instructions.iter().any(|i| match i {
            I::AttachEquipment { target_slot, .. }
            | I::ReturnOwnGraveyardCreatureToHand { target_slot, .. }
            | I::PreventNextDamage { target_slot, .. } => *target_slot == slot,
            I::PutPlusOneCounters {
                on: mtg_contracts::CounterRecipient::Target { target_slot },
                ..
            } => *target_slot == slot,
            I::ModifyPowerToughness {
                target_slot,
                power,
                toughness,
                ..
            } => *target_slot == slot && !power.is_negative() && !toughness.is_negative(),
            I::GainLife {
                recipient: LifeRecipient::Target { target_slot },
                ..
            }
            | I::DrawCards {
                recipient: Some(LifeRecipient::Target { target_slot }),
                ..
            } => *target_slot == slot,
            _ => false,
        })
    }
    fn object_matches(
        &self,
        v: &PermittedView,
        o: &VisibleObject,
        t: &TargetSpec,
        authoritative_candidate: bool,
    ) -> bool {
        let Some(c) = &o.characteristics else {
            return false;
        };
        if t.domain == TargetDomain::OwnGraveyardCreatureCard {
            return o.zone == (Zone::Graveyard { owner: v.viewer }) && c.kind == CardKind::Creature;
        }
        o.zone == Zone::Battlefield
            && self.targetable(v, o)
            && match t.domain {
                TargetDomain::ControlledBattlefieldCreature => {
                    o.controller == v.viewer && c.kind == CardKind::Creature
                }
                TargetDomain::BattlefieldCreature | TargetDomain::AnyDamageTarget => {
                    c.kind == CardKind::Creature
                }
                TargetDomain::BlockingBattlefieldCreature => {
                    c.kind == CardKind::Creature
                        // Public attacker links prove some blockers, but cannot
                        // prove an orphan blocker's retained status. Once core
                        // offers a target, its candidate list supplies that proof.
                        && (authoritative_candidate
                            || v.combat.iter().any(|a| a.blockers.contains(&o.handle)))
                }
                TargetDomain::BattlefieldPermanent => true,
                _ => false,
            }
            && t.filter.as_ref().is_none_or(|f| {
                (!f.opponent || o.controller != v.viewer)
                    && (!f.tapped || o.tapped)
                    && (f.types.is_empty() || f.types.contains(&c.kind))
                    && !f.non_types.contains(&c.kind)
                    && !c.colors.iter().any(|c| f.non_colors.contains(c))
            })
    }
    pub(super) fn object_target(
        &self,
        v: &PermittedView,
        d: &Definition,
        p: &SpellProgram,
        t: &TargetSpec,
        candidates: Option<&[ObjectHandle]>,
    ) -> Option<ObjectHandle> {
        let own = self.target_beneficial(d, p, t.slot)
            || t.domain == TargetDomain::OwnGraveyardCreatureCard;
        v.objects
            .iter()
            .filter(|o| {
                candidates.is_none_or(|cs| cs.contains(&o.handle))
                    && self.object_matches(v, o, t, candidates.is_some())
                    && (o.controller == v.viewer) == own
            })
            .max_by_key(|o| (self.value(o), o.handle.0.clone()))
            .map(|o| o.handle.clone())
    }
    pub(super) fn player_target(
        &self,
        v: &PermittedView,
        candidates: &[PlayerId],
        own: bool,
    ) -> Option<PlayerId> {
        candidates
            .iter()
            .copied()
            .filter(|p| (*p == v.viewer) == own)
            .min_by_key(|p| {
                (
                    v.players
                        .iter()
                        .find(|r| r.player == *p)
                        .map(|r| r.life.clone())
                        .unwrap_or_default(),
                    *p,
                )
            })
            .or_else(|| candidates.first().copied())
    }
    pub(super) fn stack_target(
        &self,
        v: &PermittedView,
        t: &TargetSpec,
        candidates: Option<&[StackHandle]>,
    ) -> Option<StackHandle> {
        v.stack
            .iter()
            .filter(|s| {
                s.controller != v.viewer
                    && s.cast_complete
                    && s.handle
                        .as_ref()
                        .is_some_and(|h| candidates.is_none_or(|cs| cs.contains(h)))
            })
            .filter(|s| match t.domain {
                TargetDomain::StackActivatedOrTriggeredAbility => {
                    s.ability.is_some() || s.triggered.is_some()
                }
                TargetDomain::StackSpell => {
                    s.spell.is_some()
                        && t.filter.as_ref().is_none_or(|f| {
                            self.entry_program(v, s).is_some_and(|(d, _)| {
                                (f.types.is_empty() || f.types.contains(&d.characteristics.kind))
                                    && !f.non_types.contains(&d.characteristics.kind)
                                    && !d
                                        .characteristics
                                        .colors
                                        .iter()
                                        .any(|c| f.non_colors.contains(c))
                            })
                        })
                }
                _ => false,
            })
            .max_by_key(|s| {
                (
                    s.mana_value
                        .clone()
                        .or_else(|| {
                            self.entry_program(v, s)
                                .map(|(d, _)| mana_value(&d.characteristics.mana_cost))
                        })
                        .unwrap_or_default(),
                    s.handle.clone().map(|h| h.0),
                )
            })
            .and_then(|s| s.handle.clone())
    }
    fn sacrifice_objects<'a>(
        &self,
        v: &'a PermittedView,
        f: &SacrificeFilter,
    ) -> Vec<&'a VisibleObject> {
        v.objects
            .iter()
            .filter(|o| {
                o.controller == v.viewer
                    && o.zone == Zone::Battlefield
                    && o.characteristics.as_ref().is_some_and(|c| {
                        (f.types.is_empty() || f.types.contains(&c.kind))
                            && f.subtypes.iter().all(|s| c.subtypes.contains(s))
                    })
            })
            .collect()
    }
    fn sacrifice_rank(&self, v: &PermittedView, o: &VisibleObject) -> (bool, bool, bool, Quantity) {
        (
            !self.about_to_die(v, o),
            self.commander(v, o),
            o.token.is_none(),
            self.value(o),
        )
    }
    fn incoming_damage(&self, v: &PermittedView, target: &PlayerTargetEntity) -> Quantity {
        let mut damage = Quantity::zero();
        // Public, announced targets only. Do not infer unannounced modes or LKI.
        for s in v.stack.iter().filter(|s| s.cast_complete) {
            let Some((_, p)) = self.entry_program(v, s) else {
                continue;
            };
            for i in &p.instructions {
                match i {
                    I::DealDamage {
                        target_slot,
                        amount,
                        ..
                    }
                    | I::SacrificeDamage {
                        target_slot: Some(target_slot),
                        amount: SacrificeDamageAmount::Fixed { value: amount },
                        ..
                    } => {
                        if s.entity_targets
                            .get(*target_slot as usize)
                            .is_some_and(|t| entity_matches(t, target))
                        {
                            damage.add(&self.scaled_damage(v, amount));
                        }
                    }
                    I::DamageSet {
                        sets,
                        each_player,
                        amount,
                        ..
                    } => {
                        let hit = match target {
                            PlayerTargetEntity::Player { .. } => *each_player,
                            PlayerTargetEntity::Object { object } => v
                                .objects
                                .iter()
                                .find(|o| &o.handle == object)
                                .is_some_and(|o| sets.iter().any(|set| self.in_set(v, o, set))),
                        };
                        if hit {
                            damage.add(&self.scaled_damage(
                                v,
                                &self.wipe_amount(v, amount, &s.x.clone().unwrap_or_default()),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        // Priority after ordinary combat damage is too late to prepare a shield.
        if matches!(
            v.turn.step,
            TurnStep::DeclareAttackers | TurnStep::DeclareBlockers | TurnStep::FirstStrikeDamage
        ) {
            for a in &v.combat {
                let sources: Vec<_> = match target {
                    PlayerTargetEntity::Player { player }
                        if a.defender == *player && !a.blocked =>
                    {
                        vec![&a.attacker]
                    }
                    PlayerTargetEntity::Object { object } if a.blockers.contains(object) => {
                        vec![&a.attacker]
                    }
                    PlayerTargetEntity::Object { object } if &a.attacker == object => {
                        a.blockers.iter().collect()
                    }
                    _ => vec![],
                };
                for h in sources {
                    if let Some(power) = v
                        .objects
                        .iter()
                        .find(|o| &o.handle == h)
                        .and_then(|o| o.characteristics.as_ref()?.power.as_ref())
                    {
                        damage.add(&self.scaled_damage(v, &power.clone().max(Quantity::zero())));
                    }
                }
            }
        }
        damage
    }
    fn about_to_die(&self, v: &PermittedView, o: &VisibleObject) -> bool {
        let target = PlayerTargetEntity::Object {
            object: o.handle.clone(),
        };
        let mut damage = self.incoming_damage(v, &target);
        damage.add(&o.marked_damage);
        if o.characteristics
            .as_ref()
            .and_then(|c| c.toughness.as_ref())
            .is_some_and(|t| damage >= *t)
        {
            return true;
        }
        v.stack.iter().filter(|s| s.cast_complete).any(|s| {
            self.entry_program(v, s).is_some_and(|(_, p)| {
                p.instructions.iter().any(|i| match i {
                    I::DestroyTargetCreature { target_slot, .. }
                    | I::DestroyPermanent { target_slot, .. }
                    | I::ExilePermanent { target_slot, .. } => s
                        .targets
                        .get(*target_slot as usize)
                        .is_some_and(|t| t.current.as_ref() == Some(&o.handle)),
                    I::DestroyAllCreatures { .. } => self.in_set(v, o, &ObjectSet::Creatures),
                    I::MoveSet { set, .. } => self.in_set(v, o, set),
                    I::PumpSet { set, toughness, .. } if toughness.is_negative() => {
                        self.in_set(v, o, set)
                            && o.characteristics
                                .as_ref()
                                .and_then(|c| c.toughness.as_ref())
                                .is_some_and(|t| {
                                    let mut remaining = t.clone();
                                    remaining.add(toughness);
                                    remaining <= o.marked_damage
                                })
                    }
                    _ => false,
                })
            })
        })
    }
    fn sacrifice_worthwhile(&self, v: &PermittedView, p: &SpellProgram, o: &VisibleObject) -> bool {
        if o.token.is_some() || self.about_to_die(v, o) {
            return true;
        }
        let amount = if p.instructions.iter().any(|i| {
            matches!(
                i,
                I::SacrificeDamage {
                    amount: SacrificeDamageAmount::SacrificedPower,
                    ..
                }
            )
        }) {
            o.characteristics
                .as_ref()
                .and_then(|c| c.power.clone())
                .unwrap_or_default()
        } else {
            self.damage_amount(v, p)
        };
        let amount = self.scaled_damage(v, &amount);
        v.players.iter().any(|r| {
            r.player != v.viewer && v.live_turn_order.contains(&r.player) && r.life <= amount
        }) || v.objects.iter().any(|t| {
            t.controller != v.viewer
                && t.zone == Zone::Battlefield
                && self.targetable(v, t)
                && self.value(t) > self.value(o)
                && t.characteristics
                    .as_ref()
                    .and_then(|c| c.toughness.as_ref())
                    .is_some_and(|n| {
                        let mut damage = amount.clone();
                        damage.add(&t.marked_damage);
                        damage >= *n
                    })
        }) || p.instructions.iter().any(|i| match i {
            I::DestroyTargetCreature { target_slot, .. }
            | I::DestroyPermanent { target_slot, .. }
            | I::ExilePermanent { target_slot, .. } => p
                .targets
                .iter()
                .find(|t| t.slot == *target_slot)
                .is_some_and(|t| {
                    v.objects.iter().any(|other| {
                        other.controller != v.viewer
                            && self.object_matches(v, other, t, false)
                            && self.value(other) > self.value(o)
                    })
                }),
            _ => false,
        })
    }
    fn commander(&self, v: &PermittedView, o: &VisibleObject) -> bool {
        v.commanders
            .iter()
            .any(|c| c.oracle_id == o.oracle_id && c.history.owner == v.viewer)
    }
    pub(super) fn sacrifice_choice(
        &self,
        v: &PermittedView,
        candidates: &[ObjectHandle],
    ) -> Result<ObjectHandle, String> {
        v.objects
            .iter()
            .filter(|o| candidates.contains(&o.handle))
            .min_by_key(|o| {
                (
                    self.top_program(v)
                        .is_some_and(|(_, p)| !self.sacrifice_worthwhile(v, p, o)),
                    self.sacrifice_rank(v, o),
                )
            })
            .map(|o| o.handle.clone())
            .ok_or("empty sacrifice candidates".into())
    }
    fn expendable(&self, o: &VisibleObject) -> bool {
        o.token.is_some()
            || o.characteristics.as_ref().is_some_and(|c| {
                c.kind == CardKind::Creature && c.power.as_ref().is_some_and(|p| p <= &1.into())
            })
    }
    fn damage_amount(&self, v: &PermittedView, p: &SpellProgram) -> Quantity {
        p.instructions
            .iter()
            .find_map(|i| match i {
                I::DealDamage { amount, .. } => Some(amount.clone()),
                I::SacrificeDamage {
                    amount: SacrificeDamageAmount::Fixed { value },
                    ..
                } => Some(value.clone()),
                I::SacrificeDamage {
                    amount: SacrificeDamageAmount::SacrificedPower,
                    ..
                } => {
                    // Public prepayment estimate. If the sacrifice has already left,
                    // prefer a player rather than inventing hidden last-known facts.
                    p.additional_sacrifice
                        .as_ref()
                        .and_then(|f| {
                            self.sacrifice_objects(v, f).into_iter().min_by_key(|o| {
                                (
                                    !self.sacrifice_worthwhile(v, p, o),
                                    self.sacrifice_rank(v, o),
                                )
                            })
                        })
                        .and_then(|o| o.characteristics.as_ref()?.power.clone())
                }
                _ => None,
            })
            .unwrap_or_default()
    }
    fn scaled_damage(&self, v: &PermittedView, amount: &Quantity) -> Quantity {
        let mut n = amount.clone();
        for o in &v.objects {
            if o.zone == Zone::Battlefield
                && definition(self.catalog, &o.oracle_id).is_ok_and(|d| {
                    d.static_abilities
                        .iter()
                        .any(|a| a.effect == StaticEffect::DoubleDamageToPermanentOrPlayer)
                })
            {
                let copy = n.clone();
                n.add(&copy);
            }
        }
        n
    }
    pub(super) fn damage_target(
        &self,
        v: &PermittedView,
        p: &SpellProgram,
        candidates: &[PlayerTargetEntity],
    ) -> Option<PlayerTargetEntity> {
        if p.instructions
            .iter()
            .any(|i| matches!(i, I::PreventNextDamage { .. }))
        {
            return candidates
                .iter()
                .filter(|e| match e {
                    PlayerTargetEntity::Player { player } => *player == v.viewer,
                    PlayerTargetEntity::Object { object } => v
                        .objects
                        .iter()
                        .any(|o| &o.handle == object && o.controller == v.viewer),
                })
                .max_by_key(|e| self.incoming_damage(v, e))
                .cloned()
                .or_else(|| {
                    v.objects
                        .iter()
                        .filter(|o| {
                            o.controller == v.viewer
                                && candidates.contains(&PlayerTargetEntity::Object {
                                    object: o.handle.clone(),
                                })
                        })
                        .max_by_key(|o| self.value(o))
                        .map(|o| PlayerTargetEntity::Object {
                            object: o.handle.clone(),
                        })
                });
        }
        let amount = self.scaled_damage(v, &self.damage_amount(v, p));
        let player = candidates
            .iter()
            .filter_map(|e| {
                if let PlayerTargetEntity::Player { player } = e {
                    Some(*player)
                } else {
                    None
                }
            })
            .filter(|p| *p != v.viewer)
            .min_by_key(|p| {
                v.players
                    .iter()
                    .find(|r| r.player == *p)
                    .map(|r| r.life.clone())
                    .unwrap_or_default()
            });
        if let Some(player) = player
            && v.players
                .iter()
                .any(|r| r.player == player && r.life <= amount)
        {
            return Some(PlayerTargetEntity::Player { player });
        }
        if let Some(o) = v
            .objects
            .iter()
            .filter(|o| {
                o.controller != v.viewer
                    && candidates.contains(&PlayerTargetEntity::Object {
                        object: o.handle.clone(),
                    })
                    && o.characteristics
                        .as_ref()
                        .and_then(|c| c.toughness.as_ref())
                        .is_some_and(|t| {
                            let mut damage = amount.clone();
                            damage.add(&o.marked_damage);
                            damage >= *t
                        })
            })
            .max_by_key(|o| self.value(o))
        {
            return Some(PlayerTargetEntity::Object {
                object: o.handle.clone(),
            });
        }
        player
            .map(|player| PlayerTargetEntity::Player { player })
            .or_else(|| candidates.first().cloned())
    }
    fn in_set(&self, v: &PermittedView, o: &VisibleObject, set: &ObjectSet) -> bool {
        let Some(c) = &o.characteristics else {
            return false;
        };
        o.zone == Zone::Battlefield
            && match set {
                ObjectSet::Creatures => c.kind == CardKind::Creature,
                ObjectSet::NonlandPermanents => c.kind != CardKind::Land,
                ObjectSet::Enchantments => c.kind == CardKind::Enchantment,
                ObjectSet::AttackingCreatures => v.combat.iter().any(|a| a.attacker == o.handle),
                ObjectSet::OpponentCreatures => {
                    c.kind == CardKind::Creature && o.controller != v.viewer
                }
                ObjectSet::FlyingCreatures => {
                    c.kind == CardKind::Creature && o.keywords.contains(&KeywordAbility::Flying)
                }
                ObjectSet::NonflyingCreatures => {
                    c.kind == CardKind::Creature && !o.keywords.contains(&KeywordAbility::Flying)
                }
            }
    }
    fn wipe_amount(&self, v: &PermittedView, amount: &WipeAmount, x: &Quantity) -> Quantity {
        match amount {
            WipeAmount::Fixed { value } => value.clone(),
            WipeAmount::AnnouncedX => x.clone(),
            WipeAmount::CreatureCount => Quantity::from(
                v.objects
                    .iter()
                    .filter(|o| self.in_set(v, o, &ObjectSet::Creatures))
                    .count() as i64,
            ),
        }
    }
    fn wipe_score(&self, v: &PermittedView, p: &SpellProgram, x: &Quantity) -> Option<Quantity> {
        let mut found = false;
        let mut lost = std::collections::BTreeSet::new();
        let mut wins = false;
        for i in &p.instructions {
            let (sets, damage) = match i {
                I::MoveSet { set, .. } => (vec![set.clone()], None),
                I::DestroyAllCreatures { .. } => (vec![ObjectSet::Creatures], None),
                I::PumpSet { set, toughness, .. } if toughness.is_negative() => {
                    let mut q = Quantity::zero();
                    q.subtract(toughness);
                    (vec![set.clone()], Some(q))
                }
                I::DamageSet {
                    sets,
                    each_player,
                    amount,
                    ..
                } => {
                    let damage = self.scaled_damage(v, &self.wipe_amount(v, amount, x));
                    if *each_player {
                        if v.players
                            .iter()
                            .any(|r| r.player == v.viewer && r.life <= damage)
                        {
                            return Some(Quantity::from(i64::MIN));
                        }
                        wins |= v
                            .players
                            .iter()
                            .filter(|r| {
                                r.player != v.viewer && v.live_turn_order.contains(&r.player)
                            })
                            .all(|r| r.life <= damage);
                    }
                    (sets.clone(), Some(damage))
                }
                I::EachPlayerSacrifice { .. } => {
                    found = true;
                    for player in &v.live_turn_order {
                        if let Some(o) = v
                            .objects
                            .iter()
                            .filter(|o| {
                                o.controller == *player && self.in_set(v, o, &ObjectSet::Creatures)
                            })
                            .min_by_key(|o| self.value(o))
                        {
                            lost.insert(o.handle.0.clone());
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            found = true;
            for o in &v.objects {
                if sets.iter().any(|s| self.in_set(v, o, s))
                    && damage.as_ref().is_none_or(|n| {
                        o.characteristics
                            .as_ref()
                            .and_then(|c| c.toughness.as_ref())
                            .is_some_and(|t| {
                                let mut n = n.clone();
                                n.add(&o.marked_damage);
                                n >= *t
                            })
                    })
                {
                    lost.insert(o.handle.0.clone());
                }
            }
        }
        found.then(|| {
            if wins {
                Quantity::from(i64::MAX)
            } else {
                lost.iter()
                    .filter_map(|h| v.objects.iter().find(|o| o.handle.0 == *h))
                    .fold(Quantity::zero(), |mut score, o| {
                        if o.controller == v.viewer {
                            score.subtract(&self.value(o));
                        } else {
                            score.add(&self.value(o));
                        }
                        score
                    })
            }
        })
    }
    pub(super) fn choose_x(
        &self,
        v: &PermittedView,
        d: &Definition,
        p: &SpellProgram,
    ) -> Option<Quantity> {
        // A public upper estimate from production symbols and floating mana;
        // no deck-size bound or catalog fingerprint is involved.
        let upper = v
            .objects
            .iter()
            .filter(|o| o.zone == Zone::Battlefield && o.controller == v.viewer && !o.tapped)
            .map(|o| {
                definition(self.catalog, &o.oracle_id)
                    .ok()
                    .map(|d| {
                        d.mana_abilities
                            .iter()
                            .map(|a| match &a.production {
                                mtg_contracts::ManaProduction::Fixed { symbols } => symbols.len(),
                                _ => 1,
                            })
                            .max()
                            .unwrap_or(usize::from(
                                d.characteristics.kind == CardKind::Land && d.characteristics.basic,
                            ))
                    })
                    .unwrap_or(0)
            })
            .sum::<usize>()
            + v.players
                .iter()
                .find(|r| r.player == v.viewer)
                .map(|r| {
                    r.mana
                        .values()
                        .map(|n| n.to_string().parse::<usize>().unwrap_or(0))
                        .sum::<usize>()
                })
                .unwrap_or(0);
        let mut best = Quantity::zero();
        let mut chosen = None;
        for n in 1..=upper {
            let x = Quantity::from(n as i64);
            let mut cost = self.base_cost(v, d);
            cost.generic.add(&x);
            if mana_policy::plan(self.catalog, v, &cost, None).is_none() {
                break;
            }
            let score = self.wipe_score(v, p, &x).unwrap_or_default();
            if score > best {
                best = score;
                chosen = Some(x);
            }
        }
        chosen
    }
    fn base_cost(&self, v: &PermittedView, d: &Definition) -> ManaCost {
        let mut cost = d.characteristics.mana_cost.clone();
        if d.static_abilities
            .iter()
            .any(|a| a.effect == StaticEffect::ReduceGenericByCreatureCount)
        {
            cost.generic.subtract(&Quantity::from(
                v.objects
                    .iter()
                    .filter(|o| self.in_set(v, o, &ObjectSet::Creatures))
                    .count() as i64,
            ));
            cost.generic = cost.generic.max(Quantity::zero());
        }
        cost
    }
    fn program_ready(&self, v: &PermittedView, d: &Definition, p: &SpellProgram) -> bool {
        if !p.modes.is_empty() {
            return p.modes.iter().any(|m| self.program_ready(v, d, &m.program));
        }
        // Priority during these steps precedes remaining combat damage.
        // End-of-combat priority has legal blockers but no damage left to boost.
        if blocking_pump_program(p)
            && !matches!(
                v.turn.step,
                TurnStep::DeclareBlockers | TurnStep::FirstStrikeDamage
            )
        {
            return false;
        }
        if let Some(f) = &p.additional_sacrifice
            && !self
                .sacrifice_objects(v, f)
                .iter()
                .any(|o| self.sacrifice_worthwhile(v, p, o))
        {
            return false;
        }
        for i in &p.instructions {
            match i {
                I::PreventCombatDamage { .. }
                    if !v.combat.iter().any(|a| a.defender == v.viewer)
                        || !matches!(
                            v.turn.step,
                            TurnStep::DeclareAttackers
                                | TurnStep::DeclareBlockers
                                | TurnStep::FirstStrikeDamage
                        ) =>
                {
                    return false;
                }
                I::PreventNextDamage { .. }
                    if !self
                        .friendly_entities(v)
                        .iter()
                        .any(|e| self.incoming_damage(v, e) > Quantity::zero()) =>
                {
                    return false;
                }
                _ => {}
            }
        }
        let x = if p.instructions.iter().any(|i| {
            matches!(
                i,
                I::DamageSet {
                    amount: WipeAmount::AnnouncedX,
                    ..
                }
            )
        }) {
            let Some(x) = self.choose_x(v, d, p) else {
                return false;
            };
            x
        } else {
            Quantity::zero()
        };
        if self
            .wipe_score(v, p, &x)
            .is_some_and(|score| score <= Quantity::zero())
        {
            return false;
        }
        p.targets.iter().all(|t| match t.domain {
            TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility => {
                self.stack_target(v, t, None).is_some()
            }
            TargetDomain::AnyPlayer
            | TargetDomain::OpponentPlayer
            | TargetDomain::AnyDamageTarget => v.live_turn_order.iter().any(|p| *p != v.viewer),
            _ => self.object_target(v, d, p, t, None).is_some(),
        })
    }
    pub(super) fn definition_ready(&self, v: &PermittedView, d: &Definition) -> bool {
        d.spell.as_ref().is_none_or(|p| self.program_ready(v, d, p))
    }
    fn friendly_entities(&self, v: &PermittedView) -> Vec<PlayerTargetEntity> {
        std::iter::once(PlayerTargetEntity::Player { player: v.viewer })
            .chain(
                v.objects
                    .iter()
                    .filter(|o| {
                        o.controller == v.viewer
                            && o.zone == Zone::Battlefield
                            && self.targetable(v, o)
                            && o.characteristics
                                .as_ref()
                                .is_some_and(|c| c.kind == CardKind::Creature)
                    })
                    .map(|o| PlayerTargetEntity::Object {
                        object: o.handle.clone(),
                    }),
            )
            .collect()
    }
    fn reactive_ready(&self, v: &PermittedView, d: &Definition, p: &SpellProgram) -> bool {
        if !p.modes.is_empty() {
            return p
                .modes
                .iter()
                .any(|m| self.reactive_ready(v, d, &m.program));
        }
        // Only the ordinary nonmodal spell is recognized here. In particular,
        // a modal child or activated program does not acquire this new policy.
        self.program_ready(v, d, p) && (reactive_program(p) || tapped_destroy_spell(d, p))
    }
    fn cast(
        &mut self,
        v: &PermittedView,
        o: &VisibleObject,
        d: &Definition,
    ) -> Option<PlayerResponse> {
        // The prohibition is fully public: inspect current battlefield
        // controllers and their catalog static effects, using the retained
        // turn owner rather than active-player presence (CR 800.4j).
        if v.objects.iter().any(|source| {
            source.zone == Zone::Battlefield
                && source.controller == v.turn_owner
                && source.controller != v.viewer
                && definition(self.catalog, &source.oracle_id).is_ok_and(|d| {
                    d.static_abilities
                        .iter()
                        .any(|a| a.effect == StaticEffect::OpponentsCantCastDuringControllerTurn)
                })
        }) {
            return None;
        }
        if !self.definition_ready(v, d) {
            return None;
        }
        // Keep reactive modes available for their priority window rather than
        // spending the card on its unconditional mode during development.
        if let Some(p) = &d.spell
            && p.timing == SpellTiming::Instant
            && reactive_program(p)
            && !self.reactive_ready(v, d, p)
        {
            return None;
        }
        let mut cost = self.base_cost(v, d);
        if let Some(p) = &d.spell
            && p.instructions.iter().any(|i| {
                matches!(
                    i,
                    I::DamageSet {
                        amount: WipeAmount::AnnouncedX,
                        ..
                    }
                )
            })
        {
            cost.generic.add(&self.choose_x(v, d, p)?);
        }
        if o.zone == (Zone::Command { owner: v.viewer })
            && let Some(c) = v
                .commanders
                .iter()
                .find(|c| c.oracle_id == o.oracle_id && c.history.owner == v.viewer)
        {
            let tax = c
                .history
                .command_casts
                .get(&v.viewer)
                .cloned()
                .unwrap_or_default();
            cost.generic.add(&tax);
            cost.generic.add(&tax);
        }
        self.mana_plan = mana_policy::plan(self.catalog, v, &cost, None)?;
        Some(PlayerResponse::Cast {
            card: o.handle.clone(),
        })
    }
    pub(super) fn reactive_spells(
        &mut self,
        v: &PermittedView,
    ) -> Result<Option<PlayerResponse>, String> {
        for o in v
            .objects
            .iter()
            .filter(|o| o.zone == (Zone::Hand { owner: v.viewer }))
        {
            let d = definition(self.catalog, &o.oracle_id)?;
            let Some(p) = &d.spell else { continue };
            if p.timing == SpellTiming::Instant
                && self.reactive_ready(v, d, p)
                && let Some(r) = self.cast(v, o, d)
            {
                return Ok(Some(r));
            }
        }
        Ok(None)
    }
    pub(super) fn optional_effect(&self, v: &PermittedView) -> bool {
        let Some((d, p)) = self.top_program(v) else {
            return false;
        };
        let Some(s) = v.stack.last() else {
            return false;
        };
        // Optional damage uses entity targets; optional removal uses object targets.
        if p.instructions
            .iter()
            .any(|i| matches!(i, I::SacrificeDamage { .. }))
        {
            return true;
        }
        s.targets
            .iter()
            .enumerate()
            .filter_map(|(slot, t)| Some((slot, t.current.as_ref()?)))
            .any(|(slot, h)| {
                v.objects.iter().any(|o| {
                    &o.handle == h
                        && (o.controller == v.viewer) == self.target_beneficial(d, p, slot as u32)
                })
            })
    }
    pub(super) fn choose_mode(&self, v: &PermittedView, candidates: &[u32]) -> Option<u32> {
        let (d, p) = self.top_program(v)?;
        candidates
            .iter()
            .copied()
            .find(|n| {
                p.modes
                    .get(*n as usize)
                    .is_some_and(|m| self.reactive_ready(v, d, &m.program))
            })
            .or_else(|| {
                candidates.iter().copied().find(|n| {
                    p.modes
                        .get(*n as usize)
                        .is_some_and(|m| self.program_ready(v, d, &m.program))
                })
            })
            .or_else(|| candidates.first().copied())
    }
    fn activation(
        &mut self,
        v: &PermittedView,
        o: &VisibleObject,
        d: &Definition,
        ability: usize,
    ) -> Option<PlayerResponse> {
        let a = &d.activated[ability];
        if !self.program_ready(v, d, &a.program) {
            return None;
        }
        let (tap, sac, cost) = match &a.cost {
            ActivationCost::TapSource => (true, false, zero_cost()),
            ActivationCost::SacrificeSource => (false, true, zero_cost()),
            ActivationCost::Mana { cost } => (false, false, cost.clone()),
            ActivationCost::Composite { costs } => (
                costs.tap,
                costs.sacrifice,
                costs.mana.clone().unwrap_or_else(zero_cost),
            ),
            ActivationCost::SacrificePermanent { filter, mana } => {
                if !self
                    .sacrifice_objects(v, filter)
                    .iter()
                    .any(|t| self.sacrifice_worthwhile(v, &a.program, t))
                {
                    return None;
                }
                (false, false, mana.clone().unwrap_or_else(zero_cost))
            }
        };
        if tap
            && (o.tapped
                || (d.characteristics.kind == CardKind::Creature
                    && !o.attack_ready
                    && !o.keywords.contains(&KeywordAbility::Haste)))
        {
            return None;
        }
        let draw = a
            .program
            .instructions
            .iter()
            .any(|i| matches!(i, I::DrawCards { .. }));
        if draw
            && v.players
                .iter()
                .find(|r| r.player == v.viewer)
                .is_none_or(|r| r.library_count < 2)
        {
            return None;
        }
        if sac
            && !self.sacrifice_worthwhile(v, &a.program, o)
            && !self.expendable(o)
            && v.turn.number < 20
        {
            return None;
        }
        if a.program
            .instructions
            .iter()
            .any(|i| matches!(i, I::ModifySourcePowerToughness { .. }))
            && !(v.active == Some(v.viewer)
                && v.turn.step == TurnStep::PrecombatMain
                && o.attack_ready)
            && !v
                .combat
                .iter()
                .any(|a| a.attacker == o.handle || a.blockers.contains(&o.handle))
        {
            return None;
        }
        if a.program
            .instructions
            .iter()
            .any(|i| matches!(i, I::AttachEquipment { .. }))
        {
            let t = a.program.targets.first()?;
            let target = self.object_target(v, d, &a.program, t, None)?;
            if o.attached_to.as_ref() == Some(&target) {
                return None;
            }
            if let Some(current) = o
                .attached_to
                .as_ref()
                .and_then(|h| v.objects.iter().find(|o| &o.handle == h))
                && let Some(next) = v.objects.iter().find(|o| o.handle == target)
                && self.value(current) >= self.value(next)
            {
                return None;
            }
        }
        self.mana_plan = mana_policy::plan(
            self.catalog,
            v,
            &cost,
            if tap || sac { Some(&o.handle) } else { None },
        )?;
        Some(PlayerResponse::ActivateAbility {
            object: o.handle.clone(),
            ability: ability as u32,
        })
    }
    pub(super) fn priority(&mut self, observation: &Observation) -> Result<PlayerResponse, String> {
        let v = &observation.view;
        if self.policy == Policy::DrawOnly {
            return Ok(PlayerResponse::Pass);
        }
        if let Some(r) = self.reactive_spells(v)? {
            return Ok(r);
        }
        // CR 702.8: develop flash permanents in an opponent's end step.
        // Core retains all timing, target and payment authority.
        if v.active != Some(v.viewer) && v.turn.step == TurnStep::End && v.stack.is_empty() {
            for o in v
                .objects
                .iter()
                .filter(|o| o.zone == (Zone::Hand { owner: v.viewer }))
            {
                let d = definition(self.catalog, &o.oracle_id)?;
                if d.keywords.contains(&KeywordAbility::Flash)
                    && let Some(r) = self.cast(v, o, d)
                {
                    return Ok(r);
                }
            }
        }
        if v.active != Some(v.viewer)
            || !v.stack.is_empty()
            || !matches!(
                v.turn.step,
                TurnStep::PrecombatMain | TurnStep::PostcombatMain
            )
        {
            for o in v
                .objects
                .iter()
                .filter(|o| o.zone == Zone::Battlefield && o.controller == v.viewer)
            {
                // Tokens in the current public contract have no catalog abilities.
                let Ok(d) = definition(self.catalog, &o.oracle_id) else {
                    continue;
                };
                for (ability, a) in d.activated.iter().enumerate() {
                    if a.program.timing == SpellTiming::Instant
                        && (self.reactive_ready(v, d, &a.program)
                            || matches!(
                                a.cost,
                                ActivationCost::SacrificePermanent { .. }
                                    | ActivationCost::SacrificeSource
                                    | ActivationCost::Composite {
                                        costs: mtg_contracts::PermanentActivationCosts {
                                            sacrifice: true,
                                            ..
                                        }
                                    }
                            ))
                        && let Some(r) = self.activation(v, o, d, ability)
                    {
                        return Ok(r);
                    }
                }
            }
            return Ok(PlayerResponse::Pass);
        }
        if v.turn.lands_played == 0
            && let Some(o) = v.objects.iter().find(|o| {
                o.zone == (Zone::Hand { owner: v.viewer })
                    && definition(self.catalog, &o.oracle_id)
                        .is_ok_and(|d| d.characteristics.kind == CardKind::Land)
            })
        {
            return Ok(PlayerResponse::PlayLand {
                object: o.handle.clone(),
            });
        }
        // Develop mana and death/sacrifice engines before repeatedly sinking
        // mana into commander tax. This uses IR features, never deck membership.
        let engine_card = |d: &Definition| {
            !d.mana_abilities.is_empty()
                || d.triggered.iter().any(|t| {
                    matches!(
                        t.condition,
                        mtg_contracts::TriggerCondition::AnyCreatureDies
                            | mtg_contracts::TriggerCondition::SelfDies
                            | mtg_contracts::TriggerCondition::OpponentCreatureDies
                    )
                })
                || d.spell.as_ref().is_some_and(|p| {
                    p.instructions
                        .iter()
                        .any(|i| matches!(i, I::SearchBasicLands { .. }))
                })
        };
        for stage in 0..5 {
            for o in &v.objects {
                let command = o.zone == (Zone::Command { owner: v.viewer });
                if !command && o.zone != (Zone::Hand { owner: v.viewer }) {
                    continue;
                }
                let d = definition(self.catalog, &o.oracle_id)?;
                let Some(p) = &d.spell else { continue };
                let taxed = command
                    && v.commanders.iter().any(|c| {
                        c.oracle_id == o.oracle_id
                            && c.history
                                .command_casts
                                .get(&v.viewer)
                                .is_some_and(|n| *n > Quantity::zero())
                    });
                let rank = if !command && engine_card(d) {
                    0
                } else if command && !taxed {
                    1
                } else if !command && p.resolution == SpellResolution::Permanent {
                    2
                } else if command {
                    3
                } else {
                    4
                };
                if rank != stage {
                    continue;
                }
                if let Some(r) = self.cast(v, o, d) {
                    return Ok(r);
                }
            }
        }
        for o in v
            .objects
            .iter()
            .filter(|o| o.zone == Zone::Battlefield && o.controller == v.viewer)
        {
            let Ok(d) = definition(self.catalog, &o.oracle_id) else {
                continue;
            };
            for ability in 0..d.activated.len() {
                if let Some(r) = self.activation(v, o, d, ability) {
                    return Ok(r);
                }
            }
        }
        Ok(PlayerResponse::Pass)
    }
    #[cfg(test)]
    fn land_color(&self, id: &str) -> Option<Color> {
        let d = definition(self.catalog, id).ok()?;
        (d.characteristics.kind == CardKind::Land)
            .then(|| d.characteristics.color_identity.first().copied())
            .flatten()
    }
    #[cfg(test)]
    pub(super) fn funds(&self, v: &mtg_core::PermittedView) -> BTreeMap<Color, Quantity> {
        let mut funds = v
            .players
            .iter()
            .find(|p| p.player == v.viewer)
            .map(|p| p.mana.clone())
            .unwrap_or_default();
        for o in &v.objects {
            if o.zone == Zone::Battlefield
                && o.controller == v.viewer
                && !o.tapped
                && let Some(c) = self.land_color(&o.oracle_id)
            {
                funds.entry(c).or_default().add(&1.into());
            }
        }
        funds
    }
    #[cfg(test)]
    fn opposing_stack(
        &self,
        v: &PermittedView,
        ability: bool,
        candidates: Option<&[StackHandle]>,
    ) -> Option<StackHandle> {
        let t = TargetSpec {
            slot: 0,
            selection_id: "test".into(),
            chooser: mtg_contracts::Chooser::Controller,
            stage: mtg_contracts::EvaluationStage::Announcement,
            domain: if ability {
                TargetDomain::StackActivatedOrTriggeredAbility
            } else {
                TargetDomain::StackSpell
            },
            filter: None,
        };
        self.stack_target(v, &t, candidates)
    }
}
#[cfg(test)]
#[path = "generic_policy_tests.rs"]
mod tests;

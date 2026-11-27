//! Strategy over public IR and permitted observations. These are deliberately
//! coarse value estimates; only core decides legality and executes rules.
use super::*;
use mtg_contracts::{
    ActivationCost, Definition, FunctioningZone, Instruction as I, KeywordAbility, ObjectSet,
    SacrificeFilter, SpellProgram, SpellResolution, SpellTiming, StaticEffect, TargetDomain,
    TargetSpec, TriggerCondition, announced_x, fixed, kernel,
    kir::{self, Context, SetExpr},
    source_object, target_object, target_player,
};
use mtg_core::query::{self, Tri};
use std::collections::BTreeSet;

/// CR 613.4c: the kernel set and negative toughness of an end-of-turn set
/// shrink.
fn set_shrink(i: &I) -> Option<(&SetExpr, &Quantity)> {
    let what = ObjectSet::pumped(i)?;
    let (_, _, toughness, _) = i.pumped()?;
    toughness.is_negative().then_some((what, toughness))
}

fn set_damage_hits_player(
    v: &PermittedView,
    damage: &mtg_contracts::SetDamage<'_>,
    controller: PlayerId,
    event_player: Option<PlayerId>,
    player: PlayerId,
) -> bool {
    v.live_turn_order.contains(&player)
        && (damage.each_player
            || match damage.selective_players {
                Some(kir::PlayerExpr::EachOpponent) => player != controller,
                Some(kir::PlayerExpr::Is {
                    who: kir::Ref::Controller { .. },
                }) => player == controller,
                Some(kir::PlayerExpr::Is {
                    who: kir::Ref::EventPlayer { .. },
                }) => event_player == Some(player),
                _ => false,
            })
}

fn visible_event_player(
    v: &PermittedView,
    entry: &VisibleStackEntry,
    d: &Definition,
) -> Option<PlayerId> {
    let mtg_core::VisibleTriggerAbility::Intrinsic { index } = &entry.triggered.as_ref()?.ability
    else {
        return None;
    };
    match &d.triggered.get(*index as usize)?.condition {
        TriggerCondition::Event {
            pattern: kir::EventPattern::StepBegins { .. },
            ..
        } => v.active,
        TriggerCondition::Event {
            pattern: kir::EventPattern::Cast { caster, .. },
            ..
        } if caster.is_you() => Some(entry.controller),
        // A public stack row does not identify the caster of an arbitrary
        // spell that caused a trigger.
        _ => None,
    }
}
use mtg_core::{PermittedView, StackHandle, VisibleEntityTarget, VisibleObject, VisibleStackEntry};

fn zero_cost() -> ManaCost {
    ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    }
}

fn announced_x(instruction: &I) -> bool {
    matches!(
        instruction,
        kernel!(Draw {
            player: kir::PlayerExpr::Is { .. },
            amount: announced_x!()
        }) | kernel!(Damage {
            amount: announced_x!(),
            ..
        }) | kernel!(Mill {
            player: target_player!(_),
            amount: announced_x!()
        }) | kernel!(GainLife {
            amount: mtg_contracts::announced_x!(),
            ..
        }) | kernel!(LoseLife {
            amount: mtg_contracts::announced_x!(),
            ..
        })
    )
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
    !p.targets.is_empty()
        && p.targets
            .iter()
            .all(|t| t.domain == TargetDomain::BlockingBattlefieldCreature)
        && !p.instructions.is_empty()
        && p.instructions.iter().all(|i| {
            matches!(i, kernel!(ModifyUntil { what: target_object!(_), change: kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: kir::Until::EndOfTurn, .. })
                if !power.is_negative() && !toughness.is_negative()
                    && (*power > Quantity::zero() || *toughness > Quantity::zero()))
        })
}
/// The board-wide destruction or movement this policy weighs, with its set.
///
/// The policy has always weighed ordinary wipes and left three reviewed
/// nonland-destruction programs unweighed: the opponents-only wipe, the wave
/// that forbids regeneration, and the wave that first removes indestructible.
/// Those are the same kernel verb now, so the exclusion is stated here; pod
/// decisions stay exactly what they were. Widen it as a policy change.
fn scored_wipe(i: &I) -> Option<&SetExpr> {
    match i {
        kernel!(Destroy {
            what: what @ SetExpr::Matching { .. },
            regeneration_forbidden: false,
        }) if *what != ObjectSet::OpponentNonlandPermanents.kernel(Context::Spell) => Some(what),
        kernel!(Move {
            what: what @ SetExpr::Matching { .. },
            ..
        }) => Some(what),
        _ => None,
    }
}
/// CR 701.8: "destroy all creatures", which the policy has always weighed
/// as its own program, separately from the scored wipes.
fn destroys_all_creatures(i: &I) -> bool {
    matches!(i, kernel!(Destroy {
        what: SetExpr::Matching { zone: kir::ZoneExpr::Battlefield, filter },
        regeneration_forbidden: false,
    }) if *filter == kir::Pred::creatures())
}
/// The slot of one announced target the instruction destroys or exiles.
fn removal_target(i: &I) -> Option<u32> {
    match i {
        kernel!(Move {
            what: target_object!(slot),
            to: kir::Destination::Exile
        }) => Some(*slot),
        i => i.target_destroy().map(|(slot, ..)| slot),
    }
}
fn reactive_program(p: &SpellProgram) -> bool {
    blocking_pump_program(p)
        || p.additional_sacrifice.is_some()
        || p.instructions.iter().any(|i| {
            scored_wipe(i)
                .is_some_and(|what| *what == ObjectSet::AttackingCreatures.kernel(Context::Spell))
        })
}
// A sole ordinary destruction has no other mandatory benefit to preserve.
fn sole_target_destruction(p: &SpellProgram, slot: u32) -> bool {
    p.targets.len() == 1
        && matches!(p.instructions.as_slice(),
        [i] if i.target_destroy().is_some_and(|(target_slot, ..)| target_slot == slot))
}
fn tapped_destroy_program(d: &Definition, p: &SpellProgram) -> bool {
    matches!(
        d.characteristics.kind,
        CardKind::Instant | CardKind::Sorcery
    ) && d.spell.as_ref() == Some(p)
        && p.resolution == SpellResolution::Instructions
        && p.additional_sacrifice.is_none()
        && !p.targets.is_empty()
        && p.targets.iter().all(|t| {
            t.domain == TargetDomain::BattlefieldPermanent
                && t.filter.as_ref().is_some_and(|f| f.tapped)
        })
        && !p.instructions.is_empty()
        && p.instructions.iter().all(|i| {
            matches!(i, kernel!(Destroy { what: target_object!(target_slot), regeneration_forbidden: _ })
                if p.targets.iter().any(|t| t.slot == *target_slot))
        })
}
fn tapped_destroy_spell(d: &Definition, p: &SpellProgram) -> bool {
    d.characteristics.kind == CardKind::Instant
        && p.timing == SpellTiming::Instant
        && tapped_destroy_program(d, p)
}
fn defensive_untap_spell(d: &Definition, p: &SpellProgram) -> bool {
    d.characteristics.kind == CardKind::Instant
        && d.spell.as_ref() == Some(p)
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.additional_sacrifice.is_none()
        && matches!(
            p.instructions.as_slice(),
            [kernel!(Untap {
                what: target_object!(0)
            })]
        )
        && p.targets.len() == 1
        && p.targets[0].domain == TargetDomain::BattlefieldPermanent
        && p.targets[0].filter.as_ref().is_some_and(|f| {
            f.types.is_empty()
                && f.non_types.is_empty()
                && f.non_colors.is_empty()
                && !f.opponent
                && !f.tapped
        })
}
// Match the complete strict public IR contract, without card/profile identity.
fn tap_source_creature_untap(d: &Definition, p: &SpellProgram) -> bool {
    use mtg_contracts::{Chooser, EvaluationStage, FunctioningZone};
    d.characteristics.kind == CardKind::Creature
        && p.timing == SpellTiming::Instant
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && p.additional_sacrifice.is_none()
        && matches!(
            p.instructions.as_slice(),
            [kernel!(Untap {
                what: target_object!(0)
            })]
        )
        && matches!(p.targets.as_slice(), [t] if t.slot == 0
            && !t.selection_id.is_empty() && t.chooser == Chooser::Controller
            && t.stage == EvaluationStage::Announcement
            && t.domain == TargetDomain::BattlefieldPermanent
            && t.filter.as_ref().is_some_and(|f| f.types == [CardKind::Creature]
                && f.non_types.is_empty() && f.non_colors.is_empty() && !f.opponent && !f.tapped))
        && d.activated.iter().any(|a| {
            a.program == *p
                && a.functioning_zone == FunctioningZone::Battlefield
                && a.cost == ActivationCost::TapSource
        })
}
// Recognize only a complete ordinary pure creature keyword spell. Additional
// benefits, modal/activated programs and other grants retain their own policy.

// A distinct complete compound contract: the pump must not disappear into the
// pure keyword recognizer. CR 608.2c, 611.2a, 613.4c and 702.12:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

fn contains_untap(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Untap {
                what: target_object!(_)
            })
        )
    })
}
fn activated_permanent_tap_program(d: &Definition, p: &SpellProgram) -> bool {
    d.characteristics.kind == CardKind::Artifact
        && p.additional_sacrifice.is_none()
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.targets.len() == 1
        && p.targets[0].domain == TargetDomain::BattlefieldPermanent
        && matches!(
            p.instructions.as_slice(),
            [kernel!(Tap {
                what: target_object!(0)
            })]
        )
        && d.activated.iter().any(|a| {
            &a.program == p
                && matches!(&a.cost, ActivationCost::Composite { costs }
                    if costs.tap && !costs.sacrifice && costs.mana.is_some())
        })
}
fn in_viewers_hand(view: &PermittedView, object: &VisibleObject) -> bool {
    object.zone == (Zone::Hand { owner: view.viewer })
}

impl Strategy<'_> {
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
            let mtg_core::VisibleTriggerAbility::Intrinsic { index } = &t.ability else {
                return None;
            };
            (d, &d.triggered.get(*index as usize)?.program)
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
        Some((d, p))
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
        let mut n = c.printed_mana_value();
        n.add(&c.power.clone().unwrap_or_default().max(Quantity::zero()));
        n.add(&1.into());
        n
    }

    fn harmful_attachment(&self, d: &Definition) -> bool {
        use mtg_contracts::kir::{Change, CombatRequirement, Scope, StaticRule};
        d.static_abilities.iter().any(|ability| match &ability.effect {
            StaticEffect::Continuous { affects: Scope::Attachment { .. }, changes, .. } => changes.iter().any(|change| {
                matches!(change, Change::PowerToughness { power, toughness }
                    if [power, toughness].into_iter().filter_map(|n| n.as_fixed()).any(Quantity::is_negative))
            }),
            StaticEffect::Rule { rule: StaticRule::Combat {
                affects: Scope::Attachment { .. },
                requirement: CombatRequirement::CantAttackOrBlock, ..
            }, .. } => true,
            _ => false,
        })
    }
    pub(super) fn target_beneficial(&self, d: &Definition, p: &SpellProgram, slot: u32) -> bool {
        if d.characteristics.subtypes.iter().any(|s| s == "Aura") {
            return !self.harmful_attachment(d);
        }
        p.instructions.iter().any(|i| match i {
            kernel!(PutCounters {
                on: target_object!(target_slot),
                ..
            })
            | kernel!(Regenerate {
                what: target_object!(target_slot)
            }) => *target_slot == slot,
            kernel!(ModifyUntil {
                what: target_object!(target_slot),
                change: kir::Change::PowerToughness {
                    power: fixed!(power),
                    toughness: fixed!(toughness)
                },
                until: kir::Until::EndOfTurn,
                ..
            }) => *target_slot == slot && !power.is_negative() && !toughness.is_negative(),
            kernel!(GainLife {
                player: target_player!(target_slot),
                ..
            })
            | kernel!(Draw {
                player: target_player!(target_slot),
                amount: fixed!(_)
            })
            | kernel!(Draw {
                player: target_player!(target_slot),
                amount: announced_x!()
            }) => *target_slot == slot,
            _ => false,
        })
    }
    // Core decides legality; this ranks only objects that inhabit the domain
    // and satisfy the filter under the shared evaluators. An unknown answer is
    // settled by core's own candidate list, never by a guess.
    fn object_matches(
        &self,
        v: &PermittedView,
        o: &VisibleObject,
        t: &TargetSpec,
        authoritative_candidate: bool,
    ) -> bool {
        // Graveyard-group cards are chosen with their owner, never ranked here.
        if o.characteristics.is_none()
            || matches!(t.domain, TargetDomain::PlayerGraveyardCards { .. })
        {
            return false;
        }
        let facts = query::in_view(v, o);
        let settled = |answer: Tri| match answer {
            Tri::Yes => true,
            Tri::No => false,
            Tri::Unknown => authoritative_candidate,
        };
        if !settled(query::matches_target_domain(&facts, v.viewer, t.domain)) {
            return false;
        }
        if matches!(
            t.domain,
            TargetDomain::GraveyardCard { .. }
                | TargetDomain::OwnGraveyardCard
                | TargetDomain::OwnGraveyardCreatureCard
        ) {
            return true;
        }
        self.targetable(v, o)
            && t.filter
                .as_ref()
                .is_none_or(|f| settled(query::matches_permanent_filter(&facts, v.viewer, f)))
    }
    pub(super) fn object_target(
        &self,
        v: &PermittedView,
        d: &Definition,
        p: &SpellProgram,
        t: &TargetSpec,
        candidates: Option<&[ObjectHandle]>,
    ) -> Option<ObjectHandle> {
        if p.instructions.iter().any(|i| {
            matches!(
                i,
                kernel!(Untap {
                    what: target_object!(_)
                })
            )
        }) {
            if tap_source_creature_untap(d, p) {
                return v.objects.iter().filter(|o| {
                    o.controller == v.viewer && o.tapped
                        && o.characteristics.as_ref().is_some_and(|c| c.has_type(CardKind::Creature))
                        && candidates.is_none_or(|cs| cs.contains(&o.handle))
                        && self.object_matches(v, o, t, candidates.is_some())
                        // Conservatively exclude every visible catalog untapper,
                        // including self, so autonomous activations cannot cycle.
                        && definition(self.catalog, &o.oracle_id).is_ok_and(|target|
                            !target.activated.iter().any(|a| contains_untap(&a.program)))
                }).max_by_key(|o| (self.value(o), o.handle.0.as_str())).map(|o| o.handle.clone());
            }
            return self.defensive_untap_target(v, d, p, t, candidates);
        }
        self.desirable_objects(v, d, p, t, candidates)
            .max_by_key(|o| (self.value(o), o.handle.0.as_str()))
            .map(|o| o.handle.clone())
    }
    /// The objects `object_target` ranks for an ordinary target slot.
    fn desirable_objects<'v>(
        &'v self,
        v: &'v PermittedView,
        d: &Definition,
        p: &SpellProgram,
        t: &'v TargetSpec,
        candidates: Option<&'v [ObjectHandle]>,
    ) -> impl Iterator<Item = &'v VisibleObject> {
        let own = self.target_beneficial(d, p, t.slot)
            || t.domain == TargetDomain::OwnGraveyardCreatureCard;
        // A paid source-tap activation should change its target's status.
        // Ordinary tap spells may still gain value from their draw follow-up.
        let needs_untapped = activated_permanent_tap_program(d, p);
        let indestructible_useless = sole_target_destruction(p, t.slot);
        v.objects.iter().filter(move |o| {
            candidates.is_none_or(|cs| cs.contains(&o.handle))
                && (!needs_untapped || !o.tapped)
                && (!indestructible_useless
                    || !o.keywords.contains(&KeywordAbility::Indestructible))
                && self.object_matches(v, o, t, candidates.is_some())
                && (o.controller == v.viewer) == own
        })
    }
    // Priority in DeclareAttackers follows the actual attack and precedes the
    // blocker decision (CR 508.2, 509.1h, 702.9). Never infer that flight removes
    // existing blocks. Deliberately omit speculative precombat/defensive uses.

    // Deliberately narrow: inspect the next completed public invocation, or
    // unambiguous one-on-one combat with no intervening stack. No hidden facts,
    // speculative future targets, multi-block allocation or outcome search.

    fn defensive_untap_target(
        &self,
        v: &PermittedView,
        d: &Definition,
        p: &SpellProgram,
        t: &TargetSpec,
        candidates: Option<&[ObjectHandle]>,
    ) -> Option<ObjectHandle> {
        if !defensive_untap_spell(d, p) {
            return None;
        }
        let mut entries = v.stack.iter().rev();
        let mut threat = entries.next()?;
        // Target selection exposes our newly proposed spell above the threat.
        // Skip exactly that incomplete ordinary untap spell, never an unrelated
        // entry or a completed spell that may have changed the useful response.
        if !threat.cast_complete
            && threat.controller == v.viewer
            && threat.ability.is_none()
            && threat.triggered.is_none()
            && self.entry_program(v, threat).is_some_and(|(top, program)| {
                top.oracle_id == d.oracle_id && defensive_untap_spell(top, program)
            })
        {
            threat = entries.next()?;
        }
        if !threat.cast_complete
            || threat.controller == v.viewer
            || threat.ability.is_some()
            || threat.triggered.is_some()
        {
            return None;
        }
        let (source, incoming) = self.entry_program(v, threat)?;
        if !tapped_destroy_program(source, incoming) {
            return None;
        }
        incoming.instructions.iter().filter_map(|i| {
            let (target_slot, ..) = i.target_destroy()?;
            let target_slot = &target_slot;
            let handle = threat.targets.get(*target_slot as usize)?.current.as_ref()?;
            let spec = incoming.targets.get(*target_slot as usize)?;
            let filter = spec.filter.as_ref()?;
            v.objects.iter().find(|o| {
                &o.handle == handle
                    && o.controller == v.viewer && o.tapped
                    && candidates.is_none_or(|cs| cs.contains(handle))
                    && self.object_matches(v, o, t, candidates.is_some())
                    // The incoming spell must still be a threat for its own
                    // caster. Hexproof can already protect our permanent.
                    && !o.keywords.contains(&KeywordAbility::Shroud)
                    && (o.controller == threat.controller || !o.keywords.contains(&KeywordAbility::Hexproof))
                    // The incoming filter is relative to its own controller.
                    && query::matches_permanent_filter(
                        &query::in_view(v, o),
                        threat.controller,
                        filter,
                    )
                    .is_yes()
            })
        }).max_by_key(|o| (self.value(o), o.handle.0.as_str())).map(|o| o.handle.clone())
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
                (s.controller == v.viewer) == (t.domain == TargetDomain::ControlledCreatureSpell)
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
                                let mana_value = s
                                    .mana_value
                                    .clone()
                                    .unwrap_or_else(|| d.characteristics.printed_mana_value());
                                query::spell_matches_filter(&d.characteristics, &mana_value, f)
                            })
                        })
                }
                TargetDomain::ControlledCreatureSpell => {
                    s.spell.is_some()
                        && self
                            .entry_program(v, s)
                            .is_some_and(|(d, _)| d.characteristics.has_type(CardKind::Creature))
                }
                _ => false,
            })
            .max_by_key(|s| {
                (
                    s.mana_value
                        .clone()
                        .or_else(|| {
                            self.entry_program(v, s)
                                .map(|(d, _)| d.characteristics.printed_mana_value())
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
                query::matches_sacrifice_filter(&query::in_view(v, o), v.viewer, f).is_yes()
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
            let Some((d, p)) = self.entry_program(v, s) else {
                continue;
            };
            for i in &p.instructions {
                match i {
                    kernel!(Damage {
                        to: kir::DamageTo::Ref {
                            who: kir::Ref::Target { slot: target_slot }
                        },
                        amount: fixed!(amount),
                        ..
                    }) => {
                        if s.entity_targets
                            .get(*target_slot as usize)
                            .is_some_and(|t| entity_matches(t, target))
                        {
                            damage.add(amount);
                        }
                    }
                    i if let Some(set_damage) = i.set_damage() => {
                        let hit = match target {
                            PlayerTargetEntity::Player { player } => set_damage_hits_player(
                                v,
                                &set_damage,
                                s.controller,
                                visible_event_player(v, s, d),
                                *player,
                            ),
                            PlayerTargetEntity::Object { object } => v
                                .objects
                                .iter()
                                .find(|o| &o.handle == object)
                                .is_some_and(|o| {
                                    set_damage
                                        .sets
                                        .iter()
                                        .any(|set| self.in_kernel_set(v, o, set))
                                }),
                        };
                        if hit
                            && let Some(amount) = self.wipe_amount(
                                v,
                                set_damage.amount,
                                &s.x.clone().unwrap_or_default(),
                            )
                        {
                            damage.add(&amount);
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
                        damage.add(&power.clone().max(Quantity::zero()));
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
                    i if let Some(target_slot) = removal_target(i) => s
                        .targets
                        .get(target_slot as usize)
                        .is_some_and(|t| t.current.as_ref() == Some(&o.handle)),
                    i if destroys_all_creatures(i) => self.in_set(v, o, &ObjectSet::Creatures),
                    i if scored_wipe(i).is_some() => {
                        scored_wipe(i).is_some_and(|what| self.in_kernel_set(v, o, what))
                    }
                    i if set_shrink(i).is_some() => {
                        let (what, toughness) = set_shrink(i).unwrap();
                        self.in_kernel_set(v, o, what)
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
        let amount = { self.damage_amount(v, p) };
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
            i if let Some(target_slot) = removal_target(i) => p
                .targets
                .iter()
                .find(|t| t.slot == target_slot)
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
    pub(super) fn sacrifice_choices(
        &self,
        v: &PermittedView,
        candidates: &[ObjectHandle],
        count: usize,
    ) -> Result<Vec<ObjectHandle>, String> {
        let mut choices = v
            .objects
            .iter()
            .filter(|o| candidates.contains(&o.handle))
            .collect::<Vec<_>>();
        if choices.len() < count {
            return Err("sacrifice candidates missing from view".into());
        }
        let program = self.top_program(v).map(|(_, program)| program);
        choices.sort_by_cached_key(|o| {
            (
                program.is_some_and(|p| !self.sacrifice_worthwhile(v, p, o)),
                self.sacrifice_rank(v, o),
            )
        });
        Ok(choices
            .into_iter()
            .take(count)
            .map(|o| o.handle.clone())
            .collect())
    }
    fn expendable(&self, o: &VisibleObject) -> bool {
        o.token.is_some()
            || o.characteristics.as_ref().is_some_and(|c| {
                c.has_type(CardKind::Creature) && c.power.as_ref().is_some_and(|p| p <= &1.into())
            })
    }
    fn damage_amount(&self, v: &PermittedView, p: &SpellProgram) -> Quantity {
        p.instructions
            .iter()
            .find_map(|i| match i {
                kernel!(Damage {
                    to: kir::DamageTo::Ref {
                        who: kir::Ref::Target { slot: _ }
                    },
                    amount: fixed!(amount),
                    ..
                }) => Some(amount.clone()),
                kernel!(Damage {
                    to: kir::DamageTo::Ref {
                        who: kir::Ref::Target { slot: _ }
                    },
                    amount: announced_x!(),
                    ..
                }) => v.stack.last()?.x.clone(),

                _ => None,
            })
            .unwrap_or_default()
    }
    pub(super) fn damage_target(
        &self,
        v: &PermittedView,
        p: &SpellProgram,
        candidates: &[PlayerTargetEntity],
    ) -> Option<PlayerTargetEntity> {
        let amount = self.damage_amount(v, p);
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
        query::matches_object_set(&query::in_view(v, o), v.viewer, set).is_yes()
    }
    /// CR 608.2h: membership of a kernel set, as far as the view shows it.
    fn in_kernel_set(&self, v: &PermittedView, o: &VisibleObject, what: &SetExpr) -> bool {
        match what {
            SetExpr::Matching { zone, filter } => {
                query::matches_zone_pred(&query::in_view(v, o), v.viewer, zone, filter).is_yes()
            }
            _ => false,
        }
    }
    fn wipe_amount(
        &self,
        v: &PermittedView,
        amount: &kir::Amount,
        x: &Quantity,
    ) -> Option<Quantity> {
        Some(match amount {
            kir::Amount::Fixed { value } => value.clone(),
            kir::Amount::Var {
                of: kir::Ref::AnnouncedX,
            } => x.clone(),
            kir::Amount::Count { set } => Quantity::from(
                v.objects
                    .iter()
                    .filter(|o| self.in_kernel_set(v, o, set))
                    .count() as i64,
            ),
            _ => return None,
        })
    }
    fn wipe_score(&self, v: &PermittedView, p: &SpellProgram, x: &Quantity) -> Option<Quantity> {
        let mut found = false;
        let mut lost = BTreeSet::new();
        let mut wins = false;
        for i in &p.instructions {
            let (sets, damage): (Vec<SetExpr>, _) = match i {
                i if scored_wipe(i).is_some() => {
                    (scored_wipe(i).into_iter().cloned().collect(), None)
                }
                i if set_shrink(i).is_some() => {
                    let (what, toughness) = set_shrink(i).unwrap();
                    let mut q = Quantity::zero();
                    q.subtract(toughness);
                    (vec![what.clone()], Some(q))
                }
                i if let Some(set_damage) = i.set_damage() => {
                    let damage = self.wipe_amount(v, set_damage.amount, x)?;
                    let takes_damage =
                        |player| set_damage_hits_player(v, &set_damage, v.viewer, None, player);
                    if v.players
                        .iter()
                        .any(|r| r.player == v.viewer && takes_damage(r.player) && r.life <= damage)
                    {
                        return Some(Quantity::from(i64::MIN));
                    }
                    let mut opponents = v
                        .players
                        .iter()
                        .filter(|r| r.player != v.viewer && v.live_turn_order.contains(&r.player))
                        .peekable();
                    wins |= opponents.peek().is_some()
                        && opponents.all(|r| takes_damage(r.player) && r.life <= damage);
                    (set_damage.sets.into_iter().cloned().collect(), Some(damage))
                }
                kernel!(Sacrifice {
                    player: kir::PlayerExpr::EachPlayer,
                    ..
                }) => {
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
                if sets.iter().any(|s| self.in_kernel_set(v, o, s))
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
    fn casting_mana_plan(
        &self,
        view: &PermittedView,
        definition: &Definition,
        cost: ManaCost,
    ) -> Option<(
        Vec<mtg_contracts::HybridPayment>,
        std::collections::VecDeque<mana_policy::PlannedMana>,
    )> {
        if definition.convoke || definition.improvise {
            let kind = if definition.convoke {
                CardKind::Creature
            } else {
                CardKind::Artifact
            };
            let (due, reserved) = super::tap_payment_plan(view, &cost, kind);
            if let Some(plan) = mana_policy::hybrid_plan_reserved(
                self.catalog,
                view,
                due,
                &definition.characteristics.hybrid_mana,
                &reserved,
            ) {
                return Some(plan);
            }
        }
        mana_policy::hybrid_plan(
            self.catalog,
            view,
            cost,
            &definition.characteristics.hybrid_mana,
        )
    }

    pub(super) fn choose_x(
        &self,
        v: &PermittedView,
        d: &Definition,
        p: &SpellProgram,
        quote: Option<(&mtg_core::Game, &ObjectHandle)>,
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
                            .chain(&o.granted_mana)
                            .map(|a| match &a.production {
                                mtg_contracts::ManaProduction::Fixed { symbols } => symbols.len(),
                                mtg_contracts::ManaProduction::ChooseDevotion {} => {
                                    [Color::W, Color::U, Color::B, Color::R, Color::G]
                                        .into_iter()
                                        .filter_map(|color| {
                                            super::mana_policy::devotion(v, v.viewer, color)
                                                .to_u32()
                                        })
                                        .max()
                                        .unwrap_or_default()
                                        as usize
                                }
                                mtg_contracts::ManaProduction::Counted { factor, .. } => v
                                    .objects
                                    .len()
                                    .saturating_mul(factor.to_u32().unwrap_or(1) as usize),
                                mtg_contracts::ManaProduction::Bundles { options } => {
                                    options.iter().map(Vec::len).max().unwrap_or_default()
                                }
                                mtg_contracts::ManaProduction::Choose { amount, .. } => {
                                    amount.to_u32().map_or(1, |n| n as usize)
                                }
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
        // CR 601.2b-c: an exact- or up-to-X group names at most the
        // objects this policy would choose, and a "mana value X or less"
        // card needs X of at least its printed mana value.
        let counted = matches!(
            p.variable_target_count(),
            Some(kir::VariableTargetCount::ExactlyX | kir::VariableTargetCount::UpToX)
        );
        let wanted = p
            .targets
            .first()
            .filter(|_| counted)
            .map(|t| self.desirable_objects(v, d, p, t, None).count());
        let x_filtered = p
            .targets
            .first()
            .filter(|t| t.mana_value_at_most_announced_x());
        let filtered_cards: Vec<(Quantity, Quantity)> = x_filtered
            .map(|t| {
                self.desirable_objects(v, d, p, t, None)
                    .filter_map(|o| {
                        let card = definition(self.catalog, &o.oracle_id).ok()?;
                        Some((card.characteristics.printed_mana_value(), self.value(o)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut best = Quantity::zero();
        let mut chosen = None;
        // This is a bounded policy search, including on views with extended mana production.
        for n in usize::from(x_filtered.is_none())..=upper.min(64) {
            if wanted.is_some_and(|wanted| n > wanted) {
                break;
            }
            let x = Quantity::from(n as i64);
            let cost = match quote {
                Some((game, card)) => game.quote_cast_cost(v.viewer, card, Some(&x)).ok()?,
                None => {
                    let mut cost = d.characteristics.mana_cost.clone();
                    cost.generic.add(&x);
                    cost
                }
            };
            let cost = if d.delve {
                super::delve_mana_due(cost, v, quote.map(|(_, card)| card))
            } else {
                cost
            };
            if self.casting_mana_plan(v, d, cost).is_none() {
                break;
            }
            // The best card the announced X admits; the smallest such X wins.
            let filtered_score = x_filtered.map(|_| {
                filtered_cards
                    .iter()
                    .filter(|(value, _)| *value <= x)
                    .map(|(_, score)| score.clone().max(Quantity::from(1)))
                    .max()
                    .unwrap_or_default()
            });
            let score = filtered_score
                .or_else(|| self.wipe_score(v, p, &x))
                .unwrap_or_else(|| {
                    let caps_draw = p.instructions.iter().any(|instruction| {
                        matches!(
                            instruction,
                            kernel!(Draw {
                                player: kir::PlayerExpr::Is { .. },
                                amount: announced_x!()
                            })
                        )
                    });
                    if caps_draw
                        && v.players
                            .iter()
                            .find(|player| player.player == v.viewer)
                            .is_none_or(|player| n >= player.library_count as usize)
                    {
                        Quantity::zero()
                    } else if counted || p.instructions.iter().any(announced_x) {
                        x.clone()
                    } else {
                        Quantity::zero()
                    }
                });
            if score > best {
                best = score;
                chosen = Some(x);
            }
        }
        chosen
    }
    fn program_ready(&self, v: &PermittedView, d: &Definition, p: &SpellProgram) -> bool {
        self.program_ready_with_x(v, d, p, None)
    }
    /// `x` is an activation's planned X; a spell's X is chosen from its own
    /// cast quote.
    fn program_ready_with_x(
        &self,
        v: &PermittedView,
        d: &Definition,
        p: &SpellProgram,
        x: Option<&Quantity>,
    ) -> bool {
        if p.instructions.iter().any(|i| {
            matches!(
                i,
                kernel!(Untap {
                    what: target_object!(_)
                })
            )
        }) && !defensive_untap_spell(d, p)
            && !tap_source_creature_untap(d, p)
        {
            return false;
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
        let x = if let Some(x) = x {
            x.clone()
        } else if p.instructions.iter().any(announced_x) {
            let Some(x) = self.choose_x(v, d, p, None) else {
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
            TargetDomain::PlayerGraveyardCards { .. } => true, // The owning player is mandatory, cards are optional.
            TargetDomain::StackSpell
            | TargetDomain::ControlledCreatureSpell
            | TargetDomain::StackActivatedOrTriggeredAbility => {
                self.stack_target(v, t, None).is_some()
            }
            TargetDomain::PlayerOrPlaneswalker => !v.live_turn_order.is_empty(),
            TargetDomain::AnyPlayer
            | TargetDomain::OpponentPlayer
            | TargetDomain::AnyDamageTarget => v.live_turn_order.iter().any(|p| *p != v.viewer),
            _ => self.object_target(v, d, p, t, None).is_some(),
        })
    }
    pub(super) fn definition_ready(&self, v: &PermittedView, d: &Definition) -> bool {
        d.spell.as_ref().is_none_or(|p| self.program_ready(v, d, p))
    }

    fn reactive_ready(&self, v: &PermittedView, d: &Definition, p: &SpellProgram) -> bool {
        // Only the ordinary spell is recognized here. An activated program
        // does not acquire this policy.
        self.program_ready(v, d, p)
            && (reactive_program(p) || tapped_destroy_spell(d, p) || defensive_untap_spell(d, p))
    }

    fn cast(
        &mut self,
        v: &PermittedView,
        o: &VisibleObject,
        d: &Definition,
        game: Option<&mtg_core::Game>,
    ) -> Option<Action<Viewer>> {
        if self.memory.payment.failed[usize::from(v.viewer.0)].iter().any(|failed| {
            matches!(failed, PaymentAction::Cast(id, zone) if id == &d.oracle_id && *zone == o.zone)
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
        // CR 702.34a: a graveyard cast pays the flashback cost; CR 702.81a:
        // a retrace cast pays the mana cost and discards a land card.
        let flashback = o.zone == (Zone::Graveyard { owner: v.viewer }) && !d.retrace;
        // CR 702.138a: an escape cast exiles other graveyard cards.
        if o.zone == (Zone::Graveyard { owner: v.viewer })
            && let Some(escape) = &d.escape
            && v.objects
                .iter()
                .filter(|g| g.zone == o.zone && g.handle != o.handle && g.token.is_none())
                .count()
                < escape.exile_other as usize
        {
            return None;
        }
        if o.zone == (Zone::Graveyard { owner: v.viewer })
            && d.retrace
            && !v.objects.iter().any(|h| {
                in_viewers_hand(v, h)
                    && definition(self.catalog, &h.oracle_id)
                        .is_ok_and(|hd| hd.characteristics.kind == CardKind::Land)
            })
        {
            return None;
        }
        let quote = game.map(|game| (game, &o.handle));
        let x = if let Some(p) = &d.spell
            && !flashback
            && d.x_mana_cost
        {
            Some(self.choose_x(v, d, p, quote)?)
        } else {
            None
        };
        let mut cost = match quote {
            Some((game, card)) => game.quote_cast_cost(v.viewer, card, x.as_ref()).ok()?,
            // CR 601.2f, 702.81a, 702.138a: only the engine quote includes
            // the increases and reductions that lock with a retrace or
            // escape cast; an observation alone plans neither.
            None if o.zone == (Zone::Graveyard { owner: v.viewer }) && !flashback => {
                return None;
            }
            None => {
                let mut cost = match (&d.flashback, flashback) {
                    (Some(flashback), true) => flashback.clone(),
                    (None, true) => return None,
                    _ => d.characteristics.mana_cost.clone(),
                };
                if let Some(x) = &x {
                    cost.generic.add(x);
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
                cost
            }
        };
        let unchanged_cost = d.characteristics.hybrid_mana.is_empty()
            && d.spell
                .as_ref()
                .is_none_or(|p| p.additional_sacrifice.is_none());
        let due = if d.delve {
            super::delve_mana_due(cost.clone(), v, Some(&o.handle))
        } else {
            cost.clone()
        };
        let mut alternative = None;
        let (choices, plan) = match self.casting_mana_plan(v, d, due) {
            Some(plan) => plan,
            // CR 118.9, 702.74a: announce the printed alternative cost from
            // hand or the command zone when only it can be funded.
            None => {
                let printed = d.alternative_cost.as_ref().filter(|_| {
                    matches!(o.zone, Zone::Hand { .. } | Zone::Command { .. })
                        && d.characteristics.hybrid_mana.is_empty()
                        && x.is_none()
                })?;
                // Only the engine quote includes the tax, increases and
                // reductions that lock with the alternative (CR 601.2f); an
                // observation alone never plans one.
                let (game, card) = quote?;
                let (identity, alt) = game
                    .quote_alternative_cast_cost(v.viewer, card)
                    .ok()
                    .flatten()
                    .filter(|(identity, _)| *identity == printed.identity)?;
                let due = if d.delve {
                    super::delve_mana_due(alt.clone(), v, Some(&o.handle))
                } else {
                    alt.clone()
                };
                let plan = self.casting_mana_plan(v, d, due)?;
                alternative = Some(identity);
                cost = alt;
                plan
            }
        };
        self.memory.payment.announced_cost = unchanged_cost.then_some(cost);
        self.memory.hybrid_choices = choices;
        self.memory.alternative_cost = alternative;
        self.memory.planned_x = x;
        self.memory.mana_plan = plan;
        self.memory.payment.action = Some(PaymentAction::Cast(d.oracle_id.clone(), o.zone));
        Some(Action::Cast {
            card: o.handle.clone(),
        })
    }
    /// The face-down actions the pending priority decision offers.
    fn offered(observation: &Observation, action: &Action<Viewer>) -> bool {
        observation.decision.as_ref().is_some_and(|d| {
            d.candidates
                .iter()
                .any(|c| matches!(c, Candidate::Action { action: offered } if offered == action))
        })
    }

    /// CR 702.37e: turn a face-down permanent this player controls face up
    /// when the visible mana can pay its morph cost. The controller sees the
    /// card it controls (CR 708.5).
    fn turn_face_up(&mut self, observation: &Observation) -> Option<Action<Viewer>> {
        let v = &observation.view;
        for o in v
            .objects
            .iter()
            .filter(|o| o.zone == Zone::Battlefield && o.controller == v.viewer && o.face_down)
        {
            let action = Action::TurnFaceUp {
                object: o.handle.clone(),
            };
            let failed = PaymentAction::TurnFaceUp(o.handle.clone());
            if !Self::offered(observation, &action)
                || self.memory.payment.failed[usize::from(v.viewer.0)].contains(&failed)
            {
                continue;
            }
            let Some(cost) = definition(self.catalog, &o.oracle_id)
                .ok()
                .and_then(|d| d.face_down.as_ref())
                .map(|f| f.cost.clone())
            else {
                continue;
            };
            let Some(plan) = mana_policy::plan_for(self.catalog, v, &cost, None) else {
                continue;
            };
            self.memory.payment.announced_cost = Some(cost);
            self.memory.mana_plan = plan;
            self.memory.payment.action = Some(failed);
            return Some(action);
        }
        None
    }

    /// CR 702.37a: cast a morph card from hand face down when its printed
    /// cast was not chosen. Only the rules quote prices it (CR 601.2f: {3}
    /// plus tax and increases such as Sphere of Resistance, less
    /// reductions), so an observation alone plans no face-down cast.
    fn cast_face_down(
        &mut self,
        observation: &Observation,
        game: Option<&mtg_core::Game>,
    ) -> Option<Action<Viewer>> {
        let v = &observation.view;
        for o in v.objects.iter().filter(|o| in_viewers_hand(v, o)) {
            let action = Action::CastFaceDown {
                card: o.handle.clone(),
            };
            let failed = PaymentAction::CastFaceDown(o.handle.clone());
            if !Self::offered(observation, &action)
                || self.memory.payment.failed[usize::from(v.viewer.0)].contains(&failed)
            {
                continue;
            }
            let Some(cost) =
                game.and_then(|g| g.quote_face_down_cast_cost(v.viewer, &o.handle).ok())
            else {
                continue;
            };
            let Some(plan) = mana_policy::plan_for(self.catalog, v, &cost, None) else {
                continue;
            };
            self.memory.payment.announced_cost = Some(cost);
            self.memory.mana_plan = plan;
            self.memory.payment.action = Some(failed);
            return Some(action);
        }
        None
    }

    fn reactive_spells_with_game(
        &mut self,
        v: &PermittedView,
        game: Option<&mtg_core::Game>,
    ) -> Result<Option<Action<Viewer>>, String> {
        for o in v.objects.iter().filter(|o| in_viewers_hand(v, o)) {
            let d = definition(self.catalog, &o.oracle_id)?;
            let Some(p) = &d.spell else { continue };
            if p.timing == SpellTiming::Instant
                && self.reactive_ready(v, d, p)
                && let Some(r) = self.cast(v, o, d, game)
            {
                return Ok(Some(r));
            }
        }
        Ok(None)
    }
    /// Whether a land played from the viewer's hand now would enter tapped
    /// under this policy (CR 614.12): printed tapped entry; an entry life
    /// payment, which this policy always declines (no stack, so
    /// `optional_effect` is false), so a shockland counts as tapped; or an
    /// entry reveal with no other matching hand card. Conditional entry
    /// (`EntersTappedIf`) is not predicted and counts as untapped.
    fn land_enters_tapped(&self, v: &PermittedView, land: &VisibleObject) -> bool {
        use mtg_core::query::{in_view, matches_pred};
        let Ok(d) = definition(self.catalog, &land.oracle_id) else {
            return true;
        };
        d.static_abilities.iter().any(|a| match &a.effect {
            kir::StaticEffect::EntersTapped | kir::StaticEffect::PayLifeOrEnterTapped { .. } => {
                true
            }
            kir::StaticEffect::RevealOrEnterTapped { filter } => !v.objects.iter().any(|o| {
                o.handle != land.handle
                    && in_viewers_hand(v, o)
                    && matches_pred(&in_view(v, o), v.viewer, filter).is_yes()
            }),
            _ => false,
        })
    }
    pub(super) fn optional_effect(&self, v: &PermittedView, block: Option<u32>) -> bool {
        let Some(s) = v.stack.last() else {
            return false;
        };
        let Some((d, p)) = self.top_program(v) else {
            return false;
        };

        // The mana window already plans and spends the exact colored cost.
        // Open it only when it funds our conditional benefit or avoids an
        // opponent's effect.
        if p.instructions.iter().any(|instruction| {
            let I::Kernel { node, source_block } = instruction;
            if block.is_some_and(|expected| expected != *source_block) {
                return false;
            }
            let mana = match node {
                kir::Node::IfPays {
                    payment: kir::Payment::Mana { mana },
                    ..
                } if s.controller == v.viewer => mana,
                kir::Node::UnlessPays {
                    payment: kir::Payment::Mana { mana },
                    ..
                } if s.controller != v.viewer => mana,
                _ => return false,
            };
            mana_policy::plan_for(self.catalog, v, mana, None).is_some()
        }) {
            return true;
        }

        if s.controller == v.viewer
            && p.instructions.iter().any(|instruction| {
                let I::Kernel { node, source_block } = instruction;
                block == Some(*source_block)
                    && matches!(node, kir::Node::Optional { node, .. } if matches!(
                        node.as_ref(),
                        kir::Node::Do(kir::Verb::CreateTokens { .. } | kir::Verb::PutCounters {
                            on: kir::SetExpr::Matching { zone: kir::ZoneExpr::Battlefield, .. },
                            ..
                        })
                    ))
            })
        {
            return true;
        }

        let optional_slots = p
            .instructions
            .iter()
            .filter(|i| block.is_none_or(|b| i.source_block() == b))
            .filter_map(|i| i.target_destroy())
            .filter_map(|(slot, optional, _, _)| optional.then_some(slot))
            .collect::<BTreeSet<_>>();
        s.targets
            .iter()
            .enumerate()
            .filter(|(slot, _)| optional_slots.contains(&(*slot as u32)))
            .filter_map(|(slot, t)| Some((slot, t.current.as_ref()?)))
            .any(|(slot, h)| {
                v.objects.iter().any(|o| {
                    &o.handle == h
                        && (o.controller == v.viewer) == self.target_beneficial(d, p, slot as u32)
                })
            })
    }
    fn activation(
        &mut self,
        v: &PermittedView,
        o: &VisibleObject,
        d: &Definition,
        ability: usize,
    ) -> Option<Action<Viewer>> {
        let a = &d.activated[ability];
        let in_zone = match a.functioning_zone {
            FunctioningZone::Battlefield => o.zone == Zone::Battlefield && !o.phased_out,
            FunctioningZone::Hand => o.zone == (Zone::Hand { owner: v.viewer }),
            FunctioningZone::Graveyard => o.zone == (Zone::Graveyard { owner: v.viewer }),
            _ => false,
        };
        let offered = Action::ActivateAbility {
            object: o.handle.clone(),
            ability: ability as u32,
        };
        if !in_zone
            || self
                .memory
                .offered_activations
                .as_ref()
                .is_some_and(|actions| !actions.contains(&offered))
            || self.memory.payment.failed[usize::from(v.viewer.0)].iter().any(|failed| {
                matches!(failed, PaymentAction::Activate(handle, index) if handle == &o.handle && *index == ability)
            })
            || !self.program_ready_with_x(
                v,
                d,
                &a.program,
                a.x_mana_cost.then(|| Quantity::from(1)).as_ref(),
            )
        {
            return None;
        }
        let (tap, sac, cost) = match &a.cost {
            ActivationCost::TapPayLifeSacrificeSource { .. } => return None,
            ActivationCost::DiscardSource { mana } => {
                if self.library_count(v) < 2
                    || !a.program.instructions.iter().any(|i| {
                        matches!(
                            i,
                            kernel!(Draw {
                                player: kir::PlayerExpr::Is { .. },
                                amount: fixed!(_)
                            })
                        )
                    })
                {
                    return None;
                }
                (false, false, mana.clone())
            }
            ActivationCost::ExileSource { mana } => (false, false, mana.clone()),
            ActivationCost::ChooseObjects(_) => return None,
            ActivationCost::TapSource => (true, false, zero_cost()),
            ActivationCost::SacrificeSource => (false, true, zero_cost()),
            ActivationCost::Mana { cost } => (false, false, cost.clone()),
            ActivationCost::Composite { costs } => {
                if costs
                    .remove_source_plus_one_counters
                    .as_ref()
                    .is_some_and(|n| o.plus_one_counters < *n)
                    // CR 119.4, 119.8: life that cannot change pays no life;
                    // otherwise keep a margin above the printed payment.
                    || costs.pay_life.as_ref().is_some_and(|life| {
                        v.players
                            .iter()
                            .find(|r| r.player == v.viewer)
                            .is_none_or(|r| r.life <= *life || r.life_frozen_until_next_turn)
                    })
                {
                    return None;
                }
                (
                    costs.tap,
                    costs.sacrifice,
                    costs.mana.clone().unwrap_or_else(zero_cost),
                )
            }
            ActivationCost::SacrificePermanent {
                count,
                filter,
                mana,
                tap,
                another,
            } => {
                if *count != 1 {
                    return None;
                }
                if !self
                    .sacrifice_objects(v, filter)
                    .iter()
                    .filter(|t| !*another || t.handle != o.handle)
                    .any(|t| self.sacrifice_worthwhile(v, &a.program, t))
                {
                    return None;
                }
                (*tap, false, mana.clone().unwrap_or_else(zero_cost))
            }
        };
        if tap && !available_tap(o) {
            return None;
        }
        let draw = a.program.instructions.iter().any(I::uncounted_draw);
        if draw
            && v.players
                .iter()
                .find(|r| r.player == v.viewer)
                .is_none_or(|r| r.library_count < 2)
        {
            return None;
        }
        if sac
            && !matches!(a.cost, ActivationCost::TapPayLifeSacrificeSource { .. })
            && !self.sacrifice_worthwhile(v, &a.program, o)
            && !self.expendable(o)
            && v.turn.number < 20
        {
            return None;
        }
        if a.program.instructions.iter().any(|i| {
            matches!(
                i,
                kernel!(ModifyUntil {
                    what: source_object!(),
                    change: kir::Change::PowerToughness {
                        power: _,
                        toughness: _
                    },
                    until: kir::Until::EndOfTurn,
                    ..
                })
            )
        }) && !(v.active == Some(v.viewer)
            && v.turn.step == TurnStep::PrecombatMain
            && o.attack_ready)
            && !v
                .combat
                .iter()
                .any(|a| a.attacker == o.handle || a.blockers.contains(&o.handle))
        {
            return None;
        }

        let exclude = (tap || sac).then_some(&o.handle);
        // CR 107.3a, 107.4e-f, 602.2b: hybrid symbols, then X, are announced
        // as the ability is activated. The total is the engine's own cost
        // construction over those announcements.
        let hybrid = if a.hybrid_mana.is_empty() {
            vec![]
        } else {
            let reserved: Vec<&ObjectHandle> = exclude.into_iter().collect();
            mana_policy::hybrid_plan_reserved(self.catalog, v, cost, &a.hybrid_mana, &reserved)?.0
        };
        let mut x = None;
        if a.x_mana_cost {
            // The largest X this view can fund; an X of 0 does nothing.
            for n in 1..=64i64 {
                let n = Quantity::from(n);
                if mana_policy::plan_for(
                    self.catalog,
                    v,
                    &a.announced_cost(&hybrid, Some(&n)),
                    exclude,
                )
                .is_none()
                {
                    break;
                }
                x = Some(n);
            }
            if !self.program_ready_with_x(v, d, &a.program, Some(x.as_ref()?)) {
                return None;
            }
        }
        let cost = a.announced_cost(&hybrid, x.as_ref());
        self.memory.mana_plan = mana_policy::plan_for(self.catalog, v, &cost, exclude)?;
        self.memory.hybrid_choices = hybrid;
        self.memory.planned_x = x;
        self.memory.payment.exclude = (tap || sac).then(|| o.handle.clone());
        self.memory.payment.announced_cost = Some(cost);
        self.memory.payment.action = Some(PaymentAction::Activate(o.handle.clone(), ability));
        Some(Action::ActivateAbility {
            object: o.handle.clone(),
            ability: ability as u32,
        })
    }
    #[cfg(test)]
    pub(super) fn priority(&mut self, observation: &Observation) -> Result<Action<Viewer>, String> {
        self.priority_with_game(observation, None)
    }

    pub(super) fn priority_with_game(
        &mut self,
        observation: &Observation,
        game: Option<&mtg_core::Game>,
    ) -> Result<Action<Viewer>, String> {
        let v = &observation.view;
        if self.policy == Policy::DrawOnly {
            return Ok(Action::Pass);
        }
        self.memory.payment.next_priority(v);
        self.memory.offered_activations = observation.decision.as_ref().map(|d| {
            d.candidates
                .iter()
                .filter_map(|c| match c {
                    Candidate::Action {
                        action: action @ Action::ActivateAbility { .. },
                    } => Some(action.clone()),
                    _ => None,
                })
                .collect()
        });
        self.memory.mana_plan.clear();
        self.memory.mana_ability = None;
        self.memory.mana_color = None;
        self.memory.mana_bundle = None;
        self.memory.hybrid_choices.clear();
        self.memory.alternative_cost = None;
        self.memory.planned_x = None;
        if let Some(r) = self.reactive_spells_with_game(v, game)? {
            return Ok(r);
        }
        // CR 702.8: develop flash permanents in an opponent's end step.
        // Core retains all timing, target and payment authority.
        if v.active != Some(v.viewer) && v.turn.step == TurnStep::End && v.stack.is_empty() {
            for o in v.objects.iter().filter(|o| in_viewers_hand(v, o)) {
                let d = definition(self.catalog, &o.oracle_id)?;
                if d.keywords.contains(&KeywordAbility::Flash)
                    && let Some(r) = self.cast(v, o, d, game)
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
            return Ok(Action::Pass);
        }
        let land_allowance = v
            .effects
            .iter()
            .fold(Quantity::from(1), |mut allowance, effect| {
                if let mtg_core::VisibleEffect::AdditionalLandPlays {
                    origin,
                    count,
                    expires_in_cleanup_of_turn,
                    ..
                } = effect
                    && origin.controller == v.viewer
                    && *expires_in_cleanup_of_turn == v.turn.number
                {
                    allowance.add(count);
                }
                allowance
            });
        // CR 614.12: play a land that enters untapped under this policy
        // before one that enters tapped, keeping hand order otherwise.
        if Quantity::from(i64::from(v.turn.lands_played)) < land_allowance
            && let Some(o) = v
                .objects
                .iter()
                .filter(|o| {
                    in_viewers_hand(v, o)
                        && definition(self.catalog, &o.oracle_id)
                            .is_ok_and(|d| d.characteristics.kind == CardKind::Land)
                })
                .min_by_key(|o| self.land_enters_tapped(v, o))
        {
            return Ok(Action::PlayLand {
                object: o.handle.clone(),
            });
        }
        if let Some(r) = self.turn_face_up(observation) {
            return Ok(r);
        }
        // Develop mana and death/sacrifice engines before repeatedly sinking
        // mana into commander tax. This uses IR features, never deck membership.
        let engine_card = |d: &Definition| {
            !d.mana_abilities.is_empty()
                || d.triggered.iter().any(|t| {
                    // A creature dying, other than the host of an Aura
                    // or Equipment.
                    matches!(t.condition.pattern(), kir::EventPattern::Dies { object }
                        if !object.is_ref(kir::Ref::AttachedTo))
                })
        };
        for stage in 0..5 {
            for o in &v.objects {
                let command = o.zone == (Zone::Command { owner: v.viewer });
                let graveyard = o.zone == (Zone::Graveyard { owner: v.viewer });
                if !command && !graveyard && !in_viewers_hand(v, o) {
                    continue;
                }
                let d = definition(self.catalog, &o.oracle_id)?;
                if graveyard && d.flashback.is_none() && !d.retrace && d.escape.is_none() {
                    continue;
                }
                let Some(p) = &d.spell else { continue };
                let taxed = command
                    && v.commanders.iter().any(|c| {
                        c.oracle_id == o.oracle_id
                            && c.history
                                .command_casts
                                .get(&v.viewer)
                                .is_some_and(|n| *n > Quantity::zero())
                    });
                let rank = if graveyard {
                    4
                } else if !command && engine_card(d) {
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
                if let Some(r) = self.cast(v, o, d, game) {
                    return Ok(r);
                }
            }
        }
        if let Some(r) = self.cast_face_down(observation, game) {
            return Ok(r);
        }
        for o in v.objects.iter().filter(|o| {
            (o.zone == Zone::Battlefield && o.controller == v.viewer)
                || o.zone == (Zone::Hand { owner: v.viewer })
        }) {
            let Ok(d) = definition(self.catalog, &o.oracle_id) else {
                continue;
            };
            for ability in 0..d.activated.len() {
                if let Some(r) = self.activation(v, o, d, ability) {
                    return Ok(r);
                }
            }
        }
        Ok(Action::Pass)
    }

    fn library_count(&self, v: &PermittedView) -> usize {
        v.players
            .iter()
            .find(|p| p.player == v.viewer)
            .map_or(0, |p| p.library_count as usize)
    }
}
#[cfg(test)]
#[path = "generic_policy_tests.rs"]
mod tests;

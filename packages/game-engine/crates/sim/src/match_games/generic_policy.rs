//! Strategy over public IR and permitted observations. These are deliberately
//! coarse value estimates; only core decides legality and executes rules.
use super::*;
use mtg_contracts::{
    ActivationCost, CreatureRestriction, Definition, FunctioningZone, Instruction as I,
    KeywordAbility, ObjectSet, SacrificeFilter, SpellProgram, SpellResolution, SpellTiming,
    StaticEffect, TargetDomain, TargetSpec, announced_x, fixed, kernel,
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
use mtg_core::{PermittedView, StackHandle, VisibleEntityTarget, VisibleObject, VisibleStackEntry};

fn zero_cost() -> ManaCost {
    ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    }
}
fn program_mode(p: &SpellProgram, mode: Option<u32>) -> &SpellProgram {
    mode.and_then(|m| p.modes.get(m as usize))
        .map(|m| &m.program)
        .unwrap_or(p)
}

fn requires_copy_entry_choice(definition: &Definition) -> bool {
    definition
        .static_abilities
        .iter()
        .any(|ability| ability.effect == StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn)
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
    p.modes.is_empty()
        && !p.targets.is_empty()
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
        || p.modes.iter().any(|m| reactive_program(&m.program))
}
// A sole ordinary destruction has no other mandatory benefit to preserve.
fn sole_target_destruction(p: &SpellProgram, slot: u32) -> bool {
    p.modes.is_empty()
        && p.targets.len() == 1
        && matches!(p.instructions.as_slice(),
        [i] if i.target_destroy().is_some_and(|(target_slot, ..)| target_slot == slot))
}
fn tapped_destroy_program(d: &Definition, p: &SpellProgram) -> bool {
    matches!(
        d.characteristics.kind,
        CardKind::Instant | CardKind::Sorcery
    ) && d.spell.as_ref() == Some(p)
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
        && p.modes.is_empty()
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
        && p.modes.is_empty()
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
    }) || p.modes.iter().any(|m| contains_untap(&m.program))
}
fn activated_permanent_tap_program(d: &Definition, p: &SpellProgram) -> bool {
    d.characteristics.kind == CardKind::Artifact
        && p.modes.is_empty()
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
            let mtg_core::VisibleTriggerAbility::Intrinsic { index } = &t.ability else {
                return None; // Static spell grants have no policy admission.
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
        let mut n = c.printed_mana_value();
        n.add(&c.power.clone().unwrap_or_default().max(Quantity::zero()));
        n.add(&1.into());
        n
    }
    pub(super) fn best_permanent(
        &self,
        v: &PermittedView,
        candidates: &[ObjectHandle],
    ) -> Option<ObjectHandle> {
        v.objects
            .iter()
            .filter(|o| candidates.contains(&o.handle))
            .max_by_key(|o| (self.value(o), o.handle.0.as_str()))
            .map(|o| o.handle.clone())
            .or_else(|| candidates.first().cloned())
    }

    fn harmful_attachment(&self, d: &Definition) -> bool {
        d.static_abilities.iter().any(|a| a.effect == StaticEffect::ControlEnchantedPermanent
            || matches!(&a.effect,
            StaticEffect::ModifyCreatures { power, toughness, restrictions, .. }
            if power.is_negative() || toughness.is_negative() || restrictions.contains(&CreatureRestriction::CantAttackOrBlock)))
    }
    pub(super) fn target_beneficial(&self, d: &Definition, p: &SpellProgram, slot: u32) -> bool {
        if d.characteristics.subtypes.iter().any(|s| s == "Aura") {
            return !self.harmful_attachment(d);
        }
        p.instructions.iter().any(|i| match i {
            kernel!(PutCounters {
                on: target_object!(target_slot),
                ..
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
            TargetDomain::OwnGraveyardCard | TargetDomain::OwnGraveyardCreatureCard
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

        let own = self.target_beneficial(d, p, t.slot)
            || t.domain == TargetDomain::OwnGraveyardCreatureCard;
        // A paid source-tap activation should change its target's status.
        // Ordinary tap spells may still gain value from their draw follow-up.
        let needs_untapped = activated_permanent_tap_program(d, p);
        v.objects
            .iter()
            .filter(|o| {
                candidates.is_none_or(|cs| cs.contains(&o.handle))
                    && (!needs_untapped || !o.tapped)
                    && (!sole_target_destruction(p, t.slot)
                        || !o.keywords.contains(&KeywordAbility::Indestructible))
                    && self.object_matches(v, o, t, candidates.is_some())
                    && (o.controller == v.viewer) == own
            })
            .max_by_key(|o| (self.value(o), o.handle.0.as_str()))
            .map(|o| o.handle.clone())
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
            || threat.mode.is_some()
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
                                query::spell_matches_filter(&d.characteristics, f)
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
            let Some((_, p)) = self.entry_program(v, s) else {
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
                            damage.add(&self.scaled_damage(v, amount));
                        }
                    }
                    i if let Some(set_damage) = i.set_damage() => {
                        let hit = match target {
                            PlayerTargetEntity::Player { .. } => set_damage.each_player,
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
                            damage.add(&self.scaled_damage(v, &amount));
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
                    let damage = self.scaled_damage(v, &self.wipe_amount(v, set_damage.amount, x)?);
                    if set_damage.each_player {
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
                                mtg_contracts::ManaProduction::OpponentLandColors => 0,
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
        // This is a bounded policy search, including on views with extended mana production.
        for n in 1..=upper.min(64) {
            let x = Quantity::from(n as i64);
            let mut cost = self.base_cost(v, d);
            cost.generic.add(&x);
            if mana_policy::hybrid_plan(self.catalog, v, cost, &d.characteristics.hybrid_mana)
                .is_none()
            {
                break;
            }
            let score = self.wipe_score(v, p, &x).unwrap_or_else(|| {
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
                } else if p.instructions.iter().any(announced_x) {
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
            let mut ready = p
                .modes
                .iter()
                .filter(|m| self.program_ready(v, d, &m.program));
            return match p.resolution {
                SpellResolution::ChooseModes { exactly } => {
                    ready.nth(exactly.saturating_sub(1) as usize).is_some()
                }
                _ => ready.next().is_some(),
            };
        }
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
        let x = if p.instructions.iter().any(announced_x) {
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
            TargetDomain::PlayerGraveyardCards { .. } => true, // The owning player is mandatory, cards are optional.
            TargetDomain::ControlledCreatureSpell => false,
            TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility => {
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
        if !p.modes.is_empty() {
            return p
                .modes
                .iter()
                .any(|m| self.reactive_ready(v, d, &m.program));
        }
        // Only the ordinary nonmodal spell is recognized here. In particular,
        // a modal child or activated program does not acquire this new policy.
        self.program_ready(v, d, p)
            && (reactive_program(p) || tapped_destroy_spell(d, p) || defensive_untap_spell(d, p))
    }

    fn cast(
        &mut self,
        v: &PermittedView,
        o: &VisibleObject,
        d: &Definition,
    ) -> Option<PlayerResponse> {
        // The pinned core can expose a trusted EntryCopy request without a
        // permitted decision token. Avoid originating that continuation.
        if requires_copy_entry_choice(d) {
            return None;
        }
        if self.payment.failed[usize::from(v.viewer.0)].iter().any(|failed| {
            matches!(failed, PaymentAction::Cast(id, zone) if id == &d.oracle_id && *zone == o.zone)
        }) {
            return None;
        }
        // The prohibition is fully public: inspect current battlefield
        // controllers and their catalog static effects, using the retained
        // turn owner rather than active-player presence (CR 800.4j).
        if v.turn_owner != v.viewer
            && v.objects.iter().any(|source| {
                source.zone == Zone::Battlefield
                    && !source.phased_out
                    && source.controller == v.turn_owner
                    && definition(self.catalog, &source.oracle_id).is_ok_and(|d| {
                        d.static_abilities.iter().any(|a| {
                            a.effect == StaticEffect::OpponentsCantCastDuringControllerTurn
                        })
                    })
            })
        {
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
            && p.instructions.iter().any(announced_x)
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
        let unchanged_cost = d.characteristics.hybrid_mana.is_empty()
            && d.spell
                .as_ref()
                .is_none_or(|p| p.additional_sacrifice.is_none());
        let (choices, plan) = mana_policy::hybrid_plan_for(
            self.catalog,
            v,
            cost.clone(),
            &d.characteristics.hybrid_mana,
            Some(mana_policy::ManaPurpose::Cast(
                mtg_contracts::ManaSpellTraits::from_characteristics(
                    o.characteristics.as_ref().unwrap_or(&d.characteristics),
                ),
            )),
        )?;
        self.payment.announced_cost = unchanged_cost.then_some(cost);
        self.hybrid_choices = choices;
        self.mana_plan = plan;
        self.payment.action = Some(PaymentAction::Cast(d.oracle_id.clone(), o.zone));
        Some(PlayerResponse::Cast {
            card: o.handle.clone(),
        })
    }
    pub(super) fn reactive_spells(
        &mut self,
        v: &PermittedView,
    ) -> Result<Option<PlayerResponse>, String> {
        for o in v.objects.iter().filter(|o| in_viewers_hand(v, o)) {
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
    pub(super) fn optional_effect(&self, v: &PermittedView, block: Option<u32>) -> bool {
        let Some(s) = v.stack.last() else {
            return false;
        };
        let Some((d, p)) = self.top_program(v) else {
            return false;
        };

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
    pub(super) fn mode_response(
        &self,
        v: &PermittedView,
        candidates: &[u32],
        exactly: Option<u32>,
        allow_repeat: bool,
        one_or_more: bool,
    ) -> Option<PlayerResponse> {
        let first = self.choose_mode(v, candidates)?;
        if let Some(count) = exactly {
            let modes = if allow_repeat {
                let (_, root) = self.top_program(v)?;
                // Repeated direct damage can advance the game with every occurrence.
                let repeat = candidates
                    .iter()
                    .copied()
                    .find(|i| {
                        root.modes.get(*i as usize).is_some_and(|m| {
                            matches!(
                                m.program.instructions.as_slice(),
                                [kernel!(Damage {
                                    to: kir::DamageTo::Ref {
                                        who: kir::Ref::Target { slot: _ }
                                    },
                                    amount: fixed!(_),
                                    ..
                                })]
                            ) && m
                                .program
                                .targets
                                .first()
                                .is_some_and(|t| t.domain == TargetDomain::AnyDamageTarget)
                        })
                    })
                    .unwrap_or(first);
                vec![repeat; count as usize]
            } else {
                let mut available = candidates.to_vec();
                let mut chosen = Vec::new();
                for _ in 0..count {
                    let next = self.choose_mode(v, &available)?;
                    chosen.push(next);
                    available.retain(|index| *index != next);
                }
                chosen.sort_unstable();
                chosen
            };
            Some(PlayerResponse::ModeSubset { modes })
        } else if one_or_more {
            Some(PlayerResponse::ModeSubset { modes: vec![first] })
        } else {
            Some(PlayerResponse::Modes { mode: first })
        }
    }

    pub(super) fn occurrence_program<'a>(
        &'a self,
        v: &PermittedView,
        occurrences: &[TargetSpec],
    ) -> Option<(&'a Definition, &'a SpellProgram)> {
        let (d, p) = self.top_program(v)?;
        let selected = &v.stack.last()?.selected_modes;
        let mut children = selected
            .iter()
            .filter_map(|&index| p.modes.get(index as usize));
        let program = children
            .find(|child| child.program.targets == occurrences)
            .or_else(|| {
                p.modes
                    .iter()
                    .find(|child| child.program.targets == occurrences)
            })
            .map_or(p, |child| &child.program);
        Some((d, program))
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
                candidates
                    .iter()
                    .copied()
                    .filter_map(|index| {
                        let child = &p.modes.get(index as usize)?.program;
                        self.program_ready(v, d, child).then_some((index, child))
                    })
                    .min_by_key(|(index, child)| {
                        let impact = child.instructions.iter().any(|i| {
                            matches!(
                                i,
                                kernel!(CreateTokens {
                                    token: kir::TokenSpec::Creature { .. },
                                    ..
                                }) | kernel!(Draw {
                                    player: kir::PlayerExpr::Is { .. },
                                    amount: fixed!(_)
                                })
                            )
                        });
                        (!impact, *index)
                    })
                    .map(|(index, _)| index)
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
        let in_zone = match a.functioning_zone {
            FunctioningZone::Battlefield => o.zone == Zone::Battlefield && !o.phased_out,
            FunctioningZone::Hand => o.zone == (Zone::Hand { owner: v.viewer }),
            _ => false,
        };
        if !in_zone
            || self.payment.failed[usize::from(v.viewer.0)].iter().any(|failed| {
                matches!(failed, PaymentAction::Activate(handle, index) if handle == &o.handle && *index == ability)
            })
            || !self.program_ready(v, d, &a.program)
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
            ActivationCost::TapSource => (true, false, zero_cost()),
            ActivationCost::SacrificeSource => (false, true, zero_cost()),
            ActivationCost::Mana { cost } => (false, false, cost.clone()),
            ActivationCost::Composite { costs } => {
                if costs
                    .remove_source_plus_one_counters
                    .as_ref()
                    .is_some_and(|n| o.plus_one_counters < *n)
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
            } => {
                if *count != 1 {
                    return None;
                }
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

        self.mana_plan = mana_policy::plan_for(
            self.catalog,
            v,
            &cost,
            if tap || sac { Some(&o.handle) } else { None },
            Some(mana_policy::ManaPurpose::activation(v, &o.handle)),
        )?;
        self.payment.exclude = (tap || sac).then(|| o.handle.clone());
        self.payment.announced_cost = Some(cost);
        self.payment.action = Some(PaymentAction::Activate(o.handle.clone(), ability));
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
        self.payment.next_priority(v);
        self.mana_plan.clear();
        self.mana_ability = None;
        self.mana_color = None;
        self.mana_combination = None;
        self.hybrid_choices.clear();
        if let Some(r) = self.reactive_spells(v)? {
            return Ok(r);
        }
        // CR 702.8: develop flash permanents in an opponent's end step.
        // Core retains all timing, target and payment authority.
        if v.active != Some(v.viewer) && v.turn.step == TurnStep::End && v.stack.is_empty() {
            for o in v.objects.iter().filter(|o| in_viewers_hand(v, o)) {
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
        let mut land_allowance =
            v.effects
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
        let mut graveyard_lands = false;
        for object in &v.objects {
            if object.zone != Zone::Battlefield
                || object.controller != v.viewer
                || object.phased_out
            {
                continue;
            }
            let identity = object
                .copied_values
                .as_ref()
                .and_then(|copy| copy.oracle_id.as_deref())
                .unwrap_or(&object.oracle_id);
            if let Ok(definition) = definition(self.catalog, identity) {
                for ability in &definition.static_abilities {
                    match ability.effect {
                        StaticEffect::AdditionalLandPlay => land_allowance.add(&1.into()),
                        StaticEffect::PlayLandsFromOwnGraveyard => graveyard_lands = true,
                        _ => {}
                    }
                }
            }
        }
        if Quantity::from(i64::from(v.turn.lands_played)) < land_allowance
            && let Some(o) = v.objects.iter().find(|o| {
                (in_viewers_hand(v, o)
                    || (graveyard_lands && o.zone == (Zone::Graveyard { owner: v.viewer })))
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
        };
        for stage in 0..5 {
            for o in &v.objects {
                let command = o.zone == (Zone::Command { owner: v.viewer });
                if !command && !in_viewers_hand(v, o) {
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
        Ok(PlayerResponse::Pass)
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

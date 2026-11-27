//! Until-end-of-turn modification cells (CR 611.2c, 613).
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_gain_control(
        &self,
        what: &SetExpr,
        controller: &PlayerExpr,
        until: Until,
    ) -> Result<Cell, KirError> {
        if !matches!(
            until,
            Until::EndOfTurn | Until::Indefinite | Until::SourceLeaves
        ) || (until == Until::SourceLeaves && self.env.context == Context::Spell)
        {
            return unaudited("gain control duration");
        }
        if *controller != PlayerExpr::you(self.env.context)
            && !matches!(controller, PlayerExpr::Is { who: Ref::Target { slot } }
                if matches!(self.domain(*slot)?, TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer))
        {
            return unaudited("gain control recipient");
        }
        self.audit_continuous_objects(what)?;
        Ok(Cell::GainControl)
    }

    pub(super) fn audit_characteristic_set(
        &self,
        what: &SetExpr,
        values: &CharacteristicSet,
        until: Until,
    ) -> Result<Cell, KirError> {
        if until != Until::EndOfTurn
            || values == &CharacteristicSet::default()
            || values.colors.as_ref().is_some_and(|colors| {
                colors
                    .iter()
                    .enumerate()
                    .any(|(i, c)| *c == Color::C || colors[..i].contains(c))
            })
            || (!values.keywords.is_empty()
                && !crate::supported_temporary_keywords(&values.keywords))
            || values.animation.as_ref().is_some_and(|animation| {
                ((values.base_power.is_none() || values.base_toughness.is_none())
                    && !(self.env.context == Context::Activated
                        && *what
                            == (SetExpr::One {
                                object: Ref::Source,
                            })
                        && values.base_power.is_none()
                        && values.base_toughness.is_none()
                        && animation.add_artifact
                        && !animation.replace_creature_subtypes
                        && animation.creature_subtypes.is_empty()))
                    || (animation.replace_creature_subtypes
                        && animation.creature_subtypes.is_empty())
                    || animation
                        .creature_subtypes
                        .iter()
                        .enumerate()
                        .any(|(i, subtype)| {
                            !is_creature_subtype(subtype)
                                || animation.creature_subtypes[..i].contains(subtype)
                        })
            })
        {
            return unaudited("characteristic setter values or duration");
        }
        self.audit_continuous_objects(what)?;
        Ok(Cell::SetCharacteristics)
    }

    fn audit_continuous_objects(&self, what: &SetExpr) -> Result<(), KirError> {
        match what {
            SetExpr::One {
                object: Ref::Source,
            } if self.env.context != Context::Spell => Ok(()),
            SetExpr::One {
                object: Ref::Target { slot },
            } if matches!(
                self.domain(*slot)?,
                TargetDomain::BattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature
                    | TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
            ) =>
            {
                Ok(())
            }
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            } => Self::audited_filter(&ZoneExpr::Battlefield, filter),
            _ => unaudited("continuous effect objects"),
        }
    }

    /// CR 615.1, 615.7, 611.2a: "prevent all [combat] damage that would be
    /// dealt this turn", optionally only by announced or matching sources,
    /// and "prevent the next N damage that would be dealt to [an announced
    /// recipient] this turn". Matching sources are read when the damage
    /// would be dealt (CR 609.7a); an announced source or recipient is bound
    /// to the incarnation it named.
    pub(super) fn audit_resolved_prevention(
        &self,
        prevention: &ResolvedPrevention,
        until: Until,
    ) -> Result<Cell, KirError> {
        if until != Until::EndOfTurn {
            return unaudited("prevention duration");
        }
        let any_kind = matches!(
            prevention.kind,
            DamageKindFilter::All | DamageKindFilter::Combat
        );
        match (&prevention.shield, &prevention.to, &prevention.by) {
            (PreventionShield::All, None, None) if any_kind => Ok(Cell::PreventDamageThisTurn),
            (PreventionShield::All, None, Some(PreventionSources::Matching { filter }))
                if any_kind
                    && !filter.0.is_empty()
                    && damage_source_pred_valid(filter)
                    && !names_object(filter) =>
            {
                Ok(Cell::PreventDamageThisTurn)
            }
            (
                PreventionShield::All,
                None,
                Some(PreventionSources::One {
                    object: Ref::Target { slot },
                }),
            ) if any_kind && self.creature_target(*slot)? => Ok(Cell::PreventDamageThisTurn),
            (PreventionShield::Next { amount }, Some(Ref::Target { slot }), None)
                if prevention.kind == DamageKindFilter::All
                    && *amount > Quantity::zero()
                    && matches!(
                        self.domain(*slot)?,
                        TargetDomain::AnyDamageTarget | TargetDomain::BattlefieldCreature
                    ) =>
            {
                Ok(Cell::PreventionShield)
            }
            _ => unaudited("prevention shape"),
        }
    }

    pub(super) fn audit_combat_prohibition(
        &self,
        affects: &CombatProhibitionScope<Ref>,
        restriction: CombatRequirement,
        until: Until,
    ) -> Result<Cell, KirError> {
        use CombatRequirement as R;
        if until != Until::EndOfTurn {
            return unaudited("combat prohibition duration");
        }
        match (affects, restriction) {
            (
                CombatProhibitionScope::Target {
                    target: Ref::Target { slot },
                },
                R::CantAttack | R::CantBlock | R::CantAttackOrBlock | R::CantBeBlocked,
            ) if self.domain(*slot)? == TargetDomain::BattlefieldCreature => {
                Ok(Cell::CombatProhibitionUntilEndOfTurn)
            }
            // CR 508.1d, 609.4: "attacks this turn if able" and "can attack
            // this turn as though it didn't have defender" on one announced
            // creature or the ability's own source.
            (
                CombatProhibitionScope::Target {
                    target: Ref::Target { slot },
                },
                R::AttackEachCombat | R::AttackAsThoughNoDefender,
            ) if self.domain(*slot)? == TargetDomain::BattlefieldCreature => {
                Ok(Cell::CombatProhibitionUntilEndOfTurn)
            }
            (
                CombatProhibitionScope::Target {
                    target: Ref::Source,
                },
                R::AttackEachCombat | R::AttackAsThoughNoDefender,
            ) => Ok(Cell::CombatProhibitionUntilEndOfTurn),
            (
                CombatProhibitionScope::Creatures {}
                | CombatProhibitionScope::CreaturesWithoutFlying {},
                R::CantBlock,
            ) => Ok(Cell::CombatProhibitionUntilEndOfTurn),
            _ => unaudited("combat prohibition recipient or action"),
        }
    }

    pub(super) fn audit_modify(
        &self,
        what: &SetExpr,
        layer: Layer,
        change: &Change,
        until: Until,
    ) -> Result<Cell, KirError> {
        match (layer, change, until) {
            // CR 611.2c, 613.4c, 514.2: a fixed signed P/T modification of
            // the exact source, one announced creature or a board-wide set
            // until end of turn.
            (
                Layer::PowerToughnessModify,
                Change::PowerToughness { power, toughness },
                Until::EndOfTurn,
            ) if power.as_fixed().is_some() && toughness.as_fixed().is_some() => match what {
                SetExpr::One {
                    object: Ref::Source,
                } => Ok(Cell::PumpSource),
                SetExpr::One {
                    object: Ref::Target { slot },
                } => match self.domain(*slot)? {
                    TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature => Ok(Cell::PumpTarget),
                    _ if self.creature_target(*slot)? => Ok(Cell::PumpTarget),
                    _ => unaudited("pump target domain"),
                },
                // CR 613.4c: a P/T change applies to creatures; the set is
                // read through the general battlefield query and recovered
                // as a creature-only capture.
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    filter,
                } if filter.0.contains(&Atom::Type {
                    kind: CardKind::Creature,
                }) =>
                {
                    Self::audited_filter(&ZoneExpr::Battlefield, filter).map(|()| Cell::PumpSet)
                }
                _ => unaudited("pump set"),
            },
            // CR 608.2h, 611.2c, 613.4c: a P/T change by a resolution count,
            // read once as the effect begins, of the exact source of an
            // ability, one announced creature or a board-wide creature set.
            // Both counted deltas read the same count; announced X is never
            // mixed in.
            (
                Layer::PowerToughnessModify,
                change @ Change::PowerToughnessX { power, toughness },
                Until::EndOfTurn,
            ) if change.counted_x().is_some() => {
                let x = change.counted_x().expect("guarded");
                if [power, toughness].iter().any(|delta| {
                    matches!(delta, PumpDelta::X { .. })
                        || delta.counted().is_some_and(|amount| amount != x)
                }) {
                    return unaudited("mixed pump counts");
                }
                Self::audit_verb_amount(x)?;
                match what {
                    SetExpr::One {
                        object: Ref::Source,
                    } if self.env.context != Context::Spell => Ok(Cell::PumpCounted),
                    SetExpr::One {
                        object: Ref::Target { slot },
                    } if self.creature_target(*slot)? => Ok(Cell::PumpCounted),
                    SetExpr::Matching {
                        zone: ZoneExpr::Battlefield,
                        filter,
                    } if filter.0.contains(&Atom::Type {
                        kind: CardKind::Creature,
                    }) =>
                    {
                        Self::audited_filter(&ZoneExpr::Battlefield, filter)
                            .map(|()| Cell::PumpCounted)
                    }
                    _ => unaudited("counted pump recipient"),
                }
            }
            (
                Layer::PowerToughnessModify,
                Change::PowerToughnessX { power, toughness },
                Until::EndOfTurn,
            ) if matches!(power, PumpDelta::X { .. })
                || matches!(toughness, PumpDelta::X { .. }) =>
            {
                let valid = match what {
                    SetExpr::One {
                        object: Ref::Source,
                    } => self.env.context == Context::Activated,
                    SetExpr::One {
                        object: Ref::Target { slot },
                    } => self.creature_target(*slot)?,
                    _ => false,
                };
                if valid {
                    Ok(Cell::PumpAnnouncedX)
                } else {
                    unaudited("announced X pump recipient")
                }
            }
            // CR 611.2c, 613.1f, 514.2: the exact source gains keywords until
            // end of turn.
            (Layer::Ability, Change::AddKeywords { keywords }, Until::EndOfTurn)
                if *what == SetExpr::source()
                    && self.env.context == Context::Activated
                    && crate::supported_temporary_keywords(keywords) =>
            {
                Ok(Cell::SourceKeywordsUntilEndOfTurn)
            }
            (Layer::Ability, Change::AddKeywords { keywords }, Until::EndOfTurn)
                if crate::supported_temporary_keywords(keywords) =>
            {
                match what {
                    SetExpr::One {
                        object: Ref::Target { slot },
                    } if self.creature_target(*slot)? => Ok(Cell::TargetKeywordsUntilEndOfTurn),
                    SetExpr::Matching {
                        zone: ZoneExpr::Battlefield,
                        filter,
                    } => {
                        Self::audited_filter(&ZoneExpr::Battlefield, filter)?;
                        Ok(Cell::SetKeywordsUntilEndOfTurn)
                    }
                    _ => unaudited("keyword grant targets"),
                }
            }
            _ => unaudited("verb"),
        }
    }
}

/// CR 613.4c: a targeted pump names the temporary P/T surface and a
/// board-wide pump the reviewed wipe surface. The source pump is named by
/// its complete ability (`Definition::requirements`). CR 613.1f: the
/// source's own keyword grant.
pub(super) fn capabilities(what: &SetExpr, change: &Change, out: &mut BTreeSet<Capability>) {
    match (what, change) {
        (
            SetExpr::One {
                object: Ref::Target { .. },
            },
            Change::PowerToughness { .. },
        ) => {
            out.insert(Capability::TemporaryPowerToughnessV1);
        }
        (SetExpr::Matching { .. }, Change::PowerToughness { .. }) => {
            out.insert(Capability::BoardWipesV1);
        }
        (_, Change::PowerToughnessX { .. }) => {
            out.insert(Capability::ResolvedCharacteristicsV1);
        }
        (
            SetExpr::One {
                object: Ref::Source,
            },
            Change::AddKeywords { .. },
        ) => {
            out.insert(Capability::ActivatedSourceKeywordGrantV1);
        }
        _ => {}
    }
}

/// CR 615.1, 615.7: an all-damage prevention or a damage-budget shield.
pub(super) fn prevention_capabilities(
    prevention: &ResolvedPrevention,
    out: &mut BTreeSet<Capability>,
) {
    out.insert(match prevention.shield {
        PreventionShield::All => Capability::DamagePreventionV1,
        PreventionShield::Next { .. } => Capability::DamageBudgetPreventionV1,
    });
}

/// A resolved prevention outlives its stack object, so its damage-time
/// source filter has no source identity to compare: an `Is`/`IsNot`
/// reference at any depth is unaudited.
fn names_object(filter: &Pred) -> bool {
    filter.0.iter().any(|atom| match atom {
        Atom::Is { .. } | Atom::IsNot { .. } => true,
        Atom::AnyOf { alternatives } => alternatives.iter().any(names_object),
        _ => false,
    })
}

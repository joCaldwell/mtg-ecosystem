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
                values.base_power.is_none()
                    || values.base_toughness.is_none()
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

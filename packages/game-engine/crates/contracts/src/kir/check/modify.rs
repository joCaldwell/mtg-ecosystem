//! Until-end-of-turn modification cells (CR 611.2c, 613).
use super::*;

impl Walk<'_, '_> {
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
                    TargetDomain::BattlefieldCreature
                    | TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature => Ok(Cell::PumpTarget),
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
                    } if self.domain(*slot)? == TargetDomain::BattlefieldCreature => {
                        Ok(Cell::TargetKeywordsUntilEndOfTurn)
                    }
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

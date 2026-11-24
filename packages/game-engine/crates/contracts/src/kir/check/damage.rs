//! Damage cells (CR 120).
use super::*;

impl Walk<'_, '_> {
    /// CR 120.1-120.3: the resolving object's source deals damage.
    pub(super) fn audit_damage(
        &self,
        source: &Ref,
        to: &DamageTo,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        if *source != Ref::Source {
            return unaudited("damage source");
        }
        self.audit_damage_recipient(to, amount)
    }

    /// CR 120.2b: noncombat damage to one announced recipient, the
    /// controller or the mana ability's activator, or (CR 608.2h) to every
    /// creature of a board-wide set and optionally each player.
    fn audit_damage_recipient(&self, to: &DamageTo, amount: &Amount) -> Result<Cell, KirError> {
        match to {
            DamageTo::Ref {
                who: Ref::Target { slot },
            } => {
                let announced = Self::fixed_or_announced_x(amount, 1, "damage amount")?;
                match (self.domain(*slot)?, announced) {
                    (TargetDomain::AnyDamageTarget | TargetDomain::BattlefieldCreature, true) => {
                        Ok(Cell::DamageTargetX)
                    }
                    (TargetDomain::AnyDamageTarget | TargetDomain::BattlefieldCreature, false)
                    | (
                        TargetDomain::OpponentPlayer
                        | TargetDomain::PlayerOrPlaneswalker
                        | TargetDomain::BattlefieldPermanent,
                        false,
                    ) => Ok(Cell::DamageTarget),
                    _ => unaudited("damage target domain"),
                }
            }
            DamageTo::Ref {
                who: Ref::Controller { .. },
            }
            | DamageTo::Ref {
                who:
                    Ref::EventPlayer {
                        role: EventPlayerRole::ActivatingPlayer,
                    },
            } => Self::fixed_at_least(amount, 1, "damage amount").map(|()| Cell::DamageController),
            DamageTo::Ref { .. } => unaudited("damage recipient"),
            set => {
                Self::set_damage_parts(set)?;
                match amount {
                    Amount::Fixed { .. } => Self::fixed_at_least(amount, 1, "damage amount"),
                    Amount::Var {
                        of: Ref::AnnouncedX,
                    } => Ok(()),
                    Amount::Count { set } => Self::audit_verb_count(set),
                    Amount::OtherCastsManaValue { caster } if caster.is_you() => Ok(()),
                    _ => unaudited("set damage amount"),
                }
                .map(|()| match set {
                    DamageTo::Players { players } if *players != PlayerExpr::EachPlayer => {
                        Cell::DamageSelectedPlayers
                    }
                    _ => Cell::DamageSet,
                })
            }
        }
    }

    /// The parts of a set damage recipient: creature sets of the battlefield
    /// and at most one "each player", in that order.
    fn set_damage_parts(to: &DamageTo) -> Result<(), KirError> {
        let parts = match to {
            DamageTo::All { parts } => parts.as_slice(),
            part => std::slice::from_ref(part),
        };
        let mut players = false;
        for part in parts {
            match part {
                DamageTo::Objects {
                    set:
                        SetExpr::Matching {
                            zone: ZoneExpr::Battlefield,
                            filter,
                        },
                } if !players
                    && filter.0.first()
                        == Some(&Atom::Type {
                            kind: CardKind::Creature,
                        }) =>
                {
                    Self::audited_filter(&ZoneExpr::Battlefield, filter)?
                }
                DamageTo::Players {
                    players: PlayerExpr::EachPlayer,
                } if !players => players = true,
                DamageTo::Players { players: player }
                    if parts.len() == 1
                        && (*player == PlayerExpr::EachOpponent
                            || player.is_you()
                            || matches!(player, PlayerExpr::Is { who: Ref::EventPlayer { role } }
                        if Self::recipient_role(*role))) =>
                {
                    players = true;
                }
                _ => return unaudited("set damage recipient"),
            }
        }
        Ok(())
    }
}

/// CR 120.2b: directed noncombat damage; an announced X names its surface
/// too. Board-wide damage shares the reviewed wipe surface, and a
/// cast-history amount names the history surface.
pub(super) fn capabilities(to: &DamageTo, amount: &Amount, out: &mut BTreeSet<Capability>) {
    match to {
        DamageTo::Ref { .. } => {
            out.insert(Capability::NoncombatDamageV1);
            if amount.is_announced_x() {
                out.insert(Capability::AnnouncedXEffectsV1);
            }
        }
        _ => {
            out.insert(Capability::BoardWipesV1);
            if matches!(amount, Amount::OtherCastsManaValue { .. }) {
                out.extend([Capability::CastManaValueSweepV1, Capability::CastHistoryV1]);
            }
        }
    }
}

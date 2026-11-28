//! Check slots after the shared scope and sort walk.
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_evaluated(&self, verb: &Verb) -> Result<Cell, KirError> {
        let (signature, operands) = verb.evaluated().expect("evaluated verb");
        for (slot, class) in operands.classes(signature) {
            if class.is_none_or(|class| !slot.accepts.contains(&class)) {
                return unaudited(format!("unsupported {} operand", slot.field));
            }
        }
        if let Some(amount) = operands.amount() {
            self.audit_quantity(amount)?;
        }
        match operands {
            Evaluated::PlayerAmount { player, amount } => {
                self.audit_evaluated_player(player)?;
                Ok(self.player_amount_cell(signature.verb, player, amount)?)
            }
            Evaluated::Discard {
                player,
                amount,
                random,
            } => {
                self.audit_evaluated_player(player)?;
                if random && amount.is_none() {
                    return kind("random discard requires a count");
                }
                Ok(Cell::Evaluated(signature.verb))
            }
            Evaluated::Damage { source, to, amount } => {
                self.audit_evaluated_damage(source, to, amount)
            }
            Evaluated::Counters {
                on,
                counter,
                amount,
            } => self.audit_evaluated_counters(on, counter, amount),
            Evaluated::Sacrifice {
                player,
                count,
                filter,
                greatest_power,
            } => self.audit_evaluated_sacrifice(&operands, player, count, filter, greatest_power),
            Evaluated::Tokens {
                controller,
                count,
                token,
                ..
            } => self.audit_evaluated_tokens(controller, count, token),
            Evaluated::Move { what, to } => self.audit_move(what, to),
        }
    }

    /// CR 107.1, 107.1b, 608.2h: a quantity the evaluator reads once when the
    /// step is performed. Arithmetic stays signed; the verb applies its
    /// quantity rule at use.
    fn audit_quantity(&self, amount: &Amount) -> Result<(), KirError> {
        match amount {
            Amount::Fixed { value } if value.is_negative() => kind("negative quantity"),
            Amount::Fixed { .. } | Amount::Var { .. } => Ok(()),
            Amount::Count {
                set: SetExpr::Result { .. },
            } => Ok(()),
            Amount::Count {
                set:
                    SetExpr::Pool {
                        pool: Ref::StepResult { .. },
                    },
            } => Ok(()),
            Amount::Count { set } if self.audit_control_set(set).is_ok() => Ok(()),
            Amount::Count { set } => self.audit_counted_set(set),
            Amount::CountChosenCreatureType {
                set:
                    SetExpr::Matching {
                        zone: ZoneExpr::Battlefield,
                        filter,
                    },
                ..
            } => self.audited_relative_filter(filter),
            Amount::Greatest {
                set:
                    SetExpr::Matching {
                        zone: ZoneExpr::Battlefield,
                        filter,
                    },
                ..
            } => self.audited_relative_filter(filter),
            Amount::CountersOn {
                object:
                    Ref::Source
                    | Ref::EventObject { .. }
                    | Ref::CostResult { .. }
                    | Ref::BoundVar { .. },
                counter,
            } if counter.admitted() => Ok(()),
            Amount::StatOf {
                object:
                    Ref::Source
                    | Ref::Target { .. }
                    | Ref::EventObject { .. }
                    | Ref::BoundVar { .. }
                    | Ref::CostResult { .. },
                ..
            } => Ok(()),
            Amount::LifeOf { player }
                if !matches!(
                    player,
                    PlayerExpr::EachPlayer
                        | PlayerExpr::EachOpponent
                        | PlayerExpr::Is {
                            who: Ref::StepResult { .. }
                        }
                ) =>
            {
                Ok(())
            }
            Amount::Times { factor, of } if *factor >= Quantity::from(1) => self.audit_quantity(of),
            // CR 107.1a: half of an evaluated number, rounded as printed.
            Amount::Half { of, .. } => self.audit_quantity(of),
            Amount::Sum { terms } if terms.len() >= 2 => {
                terms.iter().try_for_each(|term| self.audit_quantity(term))
            }
            Amount::Difference {
                minuend,
                subtrahend,
            } => {
                self.audit_quantity(minuend)?;
                self.audit_quantity(subtrahend)
            }
            Amount::OtherCastsManaValue { caster } if caster.is_you() => Ok(()),
            amount => Self::audit_verb_amount(amount),
        }
    }

    /// CR 608.2h, 109.5: a counted battlefield set relative to one denoted
    /// player, the controller's graveyard or hand size, or an audited set.
    fn audit_counted_set(&self, set: &SetExpr) -> Result<(), KirError> {
        match set {
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            } => self.audited_relative_filter(filter),
            SetExpr::Matching {
                zone: ZoneExpr::Graveyard { of },
                filter,
            } if self.evaluated_singleton(of) => {
                self.audit_evaluated_player(of)?;
                Self::audited_filter(&ZoneExpr::Graveyards, filter)
            }
            SetExpr::Matching {
                zone: ZoneExpr::Hand { of },
                filter,
            } if self.evaluated_singleton(of) && filter.is_empty() => {
                self.audit_evaluated_player(of)
            }
            _ => Self::audit_verb_count(set),
        }
    }

    /// The battlefield atoms the evaluator reads, where a controller may be
    /// any singleton player expression it can denote.
    fn audited_relative_filter(&self, filter: &Pred) -> Result<(), KirError> {
        let mut rest = Vec::new();
        for atom in &filter.0 {
            match atom {
                Atom::ControlledBy { player } if self.evaluated_singleton(player) => {}
                Atom::IsNot {
                    object: Ref::Source,
                } => {}
                atom => rest.push(atom.clone()),
            }
        }
        Self::audit_set_filter(&ZoneExpr::Battlefield, &Pred(rest), true)
    }

    /// A player expression that denotes at most one player when read.
    fn evaluated_singleton(&self, player: &PlayerExpr) -> bool {
        match player {
            PlayerExpr::Is { who } => matches!(
                who,
                Ref::Controller { .. }
                    | Ref::Target { .. }
                    | Ref::EventPlayer { .. }
                    | Ref::BoundVar { .. }
            ),
            PlayerExpr::ControllerOf { object } | PlayerExpr::OwnerOf { object } => matches!(
                object,
                Ref::Source
                    | Ref::EventObject { .. }
                    | Ref::BoundVar { .. }
                    | Ref::Target { .. }
                    | Ref::CostResult { .. }
            ),
            PlayerExpr::EachOpponent | PlayerExpr::EachPlayer => false,
        }
    }

    fn audit_evaluated_player(&self, player: &PlayerExpr) -> Result<(), KirError> {
        if let PlayerExpr::OwnerOf { object } | PlayerExpr::ControllerOf { object } = player {
            match object {
                Ref::Source
                | Ref::EventObject { .. }
                | Ref::BoundVar { .. }
                | Ref::CostResult { .. } => {}
                Ref::Target { slot }
                    if !matches!(
                        self.domain(*slot)?,
                        TargetDomain::StackSpell
                            | TargetDomain::ControlledCreatureSpell
                            | TargetDomain::StackActivatedOrTriggeredAbility
                    ) => {}
                _ => return unaudited("player relation without object facts"),
            }
        }
        Ok(())
    }

    fn player_amount_cell(
        &self,
        verb: EvaluatedVerb,
        player: &PlayerExpr,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        // CR 121.2: the old lowering emitted two single draws for a fixed
        // count of two. The sequence still commits each attempt on its own.
        if let Some(fixed) = amount.as_fixed() {
            let target = self.player_target(player)? == Some(TargetDomain::AnyPlayer);
            match verb {
                EvaluatedVerb::Draw if amount.is_reviewed_fixed_draw() => {
                    if player.is_you() {
                        return Ok(Cell::DrawController);
                    }
                    if target {
                        return Ok(Cell::DrawTargetPlayer);
                    }
                }
                EvaluatedVerb::GainLife | EvaluatedVerb::LoseLife if *fixed > Quantity::zero() => {
                    if player.is_you() {
                        return Ok(Cell::LifeController);
                    }
                    if target {
                        return Ok(Cell::LifeTargetPlayer);
                    }
                }
                _ => {}
            }
        }
        Ok(Cell::Evaluated(verb))
    }

    /// CR 120.1-120.3, 608.2b, 608.2h: the source deals damage to every
    /// recipient in one event. A recipient that is not a player, planeswalker
    /// or creature cannot be dealt damage.
    fn audit_evaluated_damage(
        &self,
        source: &Ref,
        to: &DamageTo,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        match source {
            Ref::Source | Ref::BoundVar { .. } => {}
            Ref::Target { slot } if self.creature_target(*slot)? => {}
            Ref::Target { slot }
                if self.domain(*slot)? == TargetDomain::ControlledBattlefieldCreature => {}
            Ref::EventObject { .. } => {
                // An entry can name a noncreature. Only a creature predicate
                // proves a power read of the entering object.
                if let Amount::StatOf {
                    object: Ref::EventObject { .. },
                    stat: Stat::Power,
                } = amount
                {
                    let event = self.env.event.ok_or_else(|| {
                        KirError::Scope("event object damage without event".into())
                    })?;
                    if matches!(event.pattern(), EventPattern::Enters { object }
                        if !object.0.contains(&Atom::Type { kind: CardKind::Creature }))
                    {
                        return unaudited("entered creature's power");
                    }
                }
            }
            _ => return unaudited("damage source"),
        }
        let parts = match to {
            DamageTo::All { parts } => parts.as_slice(),
            part => std::slice::from_ref(part),
        };
        if parts.is_empty() {
            return kind("empty damage recipients");
        }
        for part in parts {
            self.audit_damage_part(part)?;
        }
        if *source == Ref::Source {
            if let DamageTo::Ref {
                who: Ref::Target { slot },
            } = to
                && amount.as_fixed().is_some_and(|n| *n >= Quantity::from(1))
                && self.reviewed_damage_target(*slot)?
            {
                return Ok(Cell::DamageTarget);
            }
            if matches!(
                to,
                DamageTo::Ref {
                    who: Ref::Controller { .. }
                        | Ref::EventPlayer {
                            role: EventPlayerRole::ActivatingPlayer
                        }
                }
            ) && amount.as_fixed().is_some_and(|n| *n >= Quantity::from(1))
            {
                return Ok(Cell::DamageController);
            }
        }
        Ok(Cell::Evaluated(EvaluatedVerb::Damage))
    }

    /// The target domains the reviewed directed damage cell covers.
    fn reviewed_damage_target(&self, slot: u32) -> Result<bool, KirError> {
        Ok(self.creature_target(slot)?
            || self.creature_or_planeswalker_filter(slot)?
            || matches!(
                self.domain(slot)?,
                TargetDomain::AnyDamageTarget
                    | TargetDomain::OpponentPlayer
                    | TargetDomain::PlayerOrPlaneswalker
            ))
    }

    /// A filtered permanent target restricted to creatures and planeswalkers.
    fn creature_or_planeswalker_filter(&self, slot: u32) -> Result<bool, KirError> {
        Ok(matches!(
            self.domain(slot)?,
            TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent
        ) && self.env.targets[slot as usize]
            .filter
            .as_ref()
            .is_some_and(|filter| {
                filter.types.contains(&CardKind::Planeswalker)
                    && filter
                        .types
                        .iter()
                        .all(|kind| matches!(kind, CardKind::Creature | CardKind::Planeswalker))
            }))
    }

    fn audit_damage_part(&self, part: &DamageTo) -> Result<(), KirError> {
        match part {
            DamageTo::Ref { who } => self.audit_damage_ref(who),
            DamageTo::Players { players } => {
                self.audit_evaluated_player(players)?;
                match players {
                    PlayerExpr::Is { who } => self.audit_damage_ref(who),
                    _ => Ok(()),
                }
            }
            DamageTo::Objects { set } => match set {
                SetExpr::One { object } => self.audit_damage_ref(object),
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    filter,
                } if damageable_set(filter) => self.audited_relative_filter(filter),
                SetExpr::Pool {
                    pool: Ref::BoundVar { .. } | Ref::StepResult { .. },
                } => Ok(()),
                _ => unaudited("set damage recipient"),
            },
            DamageTo::All { .. } => unaudited("nested damage recipients"),
        }
    }

    fn audit_damage_ref(&self, who: &Ref) -> Result<(), KirError> {
        match who {
            Ref::Target { slot } => {
                if self.reviewed_damage_target(*slot)?
                    || matches!(
                        self.domain(*slot)?,
                        TargetDomain::AnyPlayer
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::BlockingBattlefieldCreature
                            | TargetDomain::CreatureOrPlaneswalker
                    )
                {
                    Ok(())
                } else {
                    unaudited("damage target domain")
                }
            }
            // CR 120.1, 608.2c: a step or cost result is a recipient only as
            // the player who acted, or the one object a cost sacrificed;
            // an amount or a step's object set names no recipient.
            Ref::Source
            | Ref::Controller { .. }
            | Ref::EventPlayer { .. }
            | Ref::EventObject { .. }
            | Ref::BoundVar { .. }
            | Ref::StepResult {
                role: StepRole::Player,
                ..
            }
            | Ref::CostResult {
                role: StepRole::Player | StepRole::Objects,
            } => Ok(()),
            _ => unaudited("damage recipient"),
        }
    }

    /// CR 122.1, 122.6: every recipient gets the same number of counters in
    /// one placement.
    fn audit_evaluated_counters(
        &self,
        on: &SetExpr,
        counter: &CounterKind,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        if !counter.admitted() {
            return unaudited("verb");
        }
        match on {
            SetExpr::One { object } => match object {
                Ref::Source
                | Ref::EventObject { .. }
                | Ref::BoundVar { .. }
                | Ref::CostResult { .. } => {}
                Ref::Target { slot } => match self.domain(*slot)? {
                    TargetDomain::ControlledBattlefieldCreature
                    | TargetDomain::BattlefieldCreature
                    | TargetDomain::BlockingBattlefieldCreature => {}
                    _ if self.creature_target(*slot)? => {}
                    // CR 122.1: a kind without P/T or keyword meaning may sit
                    // on any permanent the target domain admits.
                    TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                        if !counter.needs_creature() => {}
                    _ => return unaudited("counter target domain"),
                },
                _ => return unaudited("counter recipient"),
            },
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            } => self.audited_relative_filter(filter)?,
            SetExpr::Pool {
                pool: Ref::BoundVar { .. } | Ref::StepResult { .. },
            } => {}
            // CR 608.2b, 608.2f: one placement on each legal member of a
            // counted creature occurrence.
            SetExpr::Targets { slots } => {
                for slot in slots {
                    if !(matches!(
                        self.domain(*slot)?,
                        TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::BattlefieldCreature
                    ) || self.creature_target(*slot)?)
                    {
                        return unaudited("counter target domain");
                    }
                }
            }
            _ => return unaudited("counter recipient"),
        }
        let fixed = amount.as_fixed().is_some_and(|n| *n >= Quantity::from(1));
        if *counter == CounterKind::PlusOnePlusOne && fixed {
            if *on == SetExpr::source() {
                return Ok(Cell::PlusOneCountersSource);
            }
            if let SetExpr::One {
                object: Ref::Target { slot },
            } = on
                && (self.domain(*slot)? == TargetDomain::ControlledBattlefieldCreature
                    || self.creature_target(*slot)?)
            {
                return Ok(Cell::PlusOneCountersTarget);
            }
        }
        Ok(Cell::Evaluated(EvaluatedVerb::PutCounters))
    }

    /// CR 701.21a: one designated permanent, or each denoted player's choice
    /// of permanents they control, collected in APNAP order (CR 101.4).
    fn audit_evaluated_sacrifice(
        &self,
        operands: &Evaluated<'_>,
        player: &PlayerExpr,
        count: &Amount,
        filter: &Pred,
        greatest_power: bool,
    ) -> Result<Cell, KirError> {
        self.audit_evaluated_player(player)?;
        match count {
            Amount::Fixed { value } if *value >= Quantity::from(1) => {}
            Amount::Fixed { .. } => return unaudited("sacrifice count"),
            _ => {}
        }
        if let Some(object) = operands.designated() {
            if count.as_fixed() != Some(&Quantity::from(1)) || greatest_power {
                return unaudited("designated sacrifice count");
            }
            if !matches!(object, Ref::Source | Ref::EventObject { .. }) {
                return unaudited("designated sacrifice object");
            }
            return Ok(Cell::Evaluated(EvaluatedVerb::Sacrifice));
        }
        for atom in &filter.0 {
            // Top-level source exclusion is evaluated by exact object ID.
            battlefield_target_atom(atom, true)?;
        }
        if greatest_power && *filter != Pred::creatures() {
            return unaudited("sacrifice greatest power");
        }
        if !greatest_power
            && count.as_fixed() == Some(&Quantity::from(1))
            && matches!(
                self.player_target(player)?,
                Some(TargetDomain::OpponentPlayer | TargetDomain::AnyPlayer)
            )
        {
            return Ok(Cell::SacrificeTargetOpponent);
        }
        Ok(Cell::Evaluated(EvaluatedVerb::Sacrifice))
    }
}

/// CR 120.3: a board set whose every member can be dealt damage.
fn damageable_set(filter: &Pred) -> bool {
    let damageable = |kind: &CardKind| matches!(kind, CardKind::Creature | CardKind::Planeswalker);
    filter.0.iter().any(|atom| match atom {
        Atom::Type { kind } => damageable(kind),
        Atom::AnyType { kinds } => !kinds.is_empty() && kinds.iter().all(damageable),
        _ => false,
    })
}

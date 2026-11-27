//! [`check`]: the single scope, type and audited-cell decision for a node,
//! and the capabilities it derives.
use super::*;
use crate::{FunctioningZone, Instruction, SpellProgram, SpellResolution, SpellTiming};

mod counter;
mod counters;
mod damage;
mod destroy;
mod discard;
mod draw;
mod evaluated;
mod keyword_actions;
mod library;
mod life;
mod mana;
mod mill;
mod modify;
mod move_zone;
mod sacrifice;
mod tap;
mod tokens;
pub use mana::{mana_ability_cells, mana_rider};

// ---------------------------------------------------------------------------
// Checking
// ---------------------------------------------------------------------------

/// Everything outside a node that its references may read.
#[derive(Clone, Copy, Debug)]
pub struct Env<'a> {
    pub context: Context,
    /// The program's announced target occurrences (CR 601.2c).
    pub targets: &'a [TargetSpec],
    /// The event of a triggered ability (CR 603.2).
    pub event: Option<&'a TriggerCondition>,
    /// CR 107.3: the object has {X} in its cost.
    pub announced_x: bool,
}

/// What a reference denotes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    Object,
    Player,
    /// CR 115.4: "any target" - a creature, player or planeswalker.
    Entity,
    StackObject,
    Number,
    Pool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KirError {
    /// A reference names something this context, event or scope lacks.
    Scope(String),
    /// A reference or literal is of the wrong kind for its operand.
    Type(String),
    /// CR 400.2: counting matching cards of a hidden zone would reveal them.
    HiddenCount,
    /// Well formed, but no audited `(verb, operand kind, context)` cell
    /// executes it.
    Unaudited(String),
}

impl fmt::Display for KirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(detail) => write!(f, "kernel reference out of scope: {detail}"),
            Self::Type(detail) => write!(f, "kernel operand type: {detail}"),
            Self::HiddenCount => f.write_str("kernel count over a filtered hidden zone"),
            Self::Unaudited(detail) => write!(f, "kernel cell is not audited: {detail}"),
        }
    }
}

impl From<KirError> for String {
    fn from(error: KirError) -> Self {
        error.to_string()
    }
}

/// What a checked node needs from its surroundings, derived by the walk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DerivedRequirements {
    pub capabilities: BTreeSet<Capability>,
    /// Target occurrences the node reads (CR 601.2c).
    pub target_slots: BTreeSet<u32>,
    pub event_player: bool,
    pub announced_x: bool,
    /// The audited cells the node occupies, for its derived tiers.
    pub cells: BTreeSet<Cell>,
}

/// The single scope-and-type checker for kernel nodes.
pub fn check(node: &Node, env: &Env<'_>) -> Result<DerivedRequirements, KirError> {
    let mut walk = Walk {
        env,
        scan: false,
        plain_leaf: false,
        bound: vec![],
        steps: Vec::new(),
        direct_sequence_member: false,
        out: DerivedRequirements::default(),
    };
    walk.node(node, false)?;
    if node.uses_continuation() {
        walk.out.capabilities.insert(Capability::KernelControlV1);
        walk.out.cells.insert(Cell::Control);
    }
    Ok(walk.out)
}

pub(super) fn audit_target_predicate(filter: &Pred, graveyard: bool) -> Result<(), KirError> {
    if filter.0.is_empty() {
        return unaudited("empty target predicate");
    }
    if graveyard {
        for atom in &filter.0 {
            if !mana_value_at_most_announced_x(atom) {
                Walk::library_card_filter(&Pred(vec![atom.clone()]))?;
            }
        }
        return Ok(());
    }
    filter
        .0
        .iter()
        .try_for_each(|atom| battlefield_target_atom(atom, true))
}

/// The evergreen keywords a battlefield target filter may test.
const TARGET_FILTER_KEYWORDS: [KeywordAbility; 17] = [
    KeywordAbility::Defender,
    KeywordAbility::Flying,
    KeywordAbility::Reach,
    KeywordAbility::Vigilance,
    KeywordAbility::Haste,
    KeywordAbility::Lifelink,
    KeywordAbility::Deathtouch,
    KeywordAbility::Indestructible,
    KeywordAbility::Trample,
    KeywordAbility::FirstStrike,
    KeywordAbility::DoubleStrike,
    KeywordAbility::Menace,
    KeywordAbility::Plainswalk,
    KeywordAbility::Islandwalk,
    KeywordAbility::Swampwalk,
    KeywordAbility::Mountainwalk,
    KeywordAbility::Forestwalk,
];

/// Whether a live permanent target may test this keyword.
pub fn target_filter_keyword(keyword: KeywordAbility) -> bool {
    TARGET_FILTER_KEYWORDS.contains(&keyword)
}

/// CR 601.2c, 608.2b: a battlefield target's characteristic, status and
/// keyword terms, evaluated on the permanent's current derived facts at
/// announcement and again at resolution (CR 110.5, 506.4, 509.1g,
/// 613.1d-f). Stat bounds are fixed. "Another" (`IsNot` the source) is a
/// top-level conjunct; controller terms use the target domain instead.
fn battlefield_target_atom(atom: &Atom, top: bool) -> Result<(), KirError> {
    match atom {
        Atom::Type { .. }
        | Atom::NotType { .. }
        | Atom::AnyType { .. }
        | Atom::Subtype { .. }
        | Atom::NotSubtype { .. }
        | Atom::Legendary
        | Atom::NotLegendary
        | Atom::Basic
        | Atom::NotBasic
        | Atom::Color { .. }
        | Atom::NotColor { .. }
        | Atom::Tapped
        | Atom::Untapped
        | Atom::Attacking
        | Atom::Blocking
        | Atom::Token
        | Atom::NotToken => Ok(()),
        Atom::Keyword { keyword } | Atom::NotKeyword { keyword }
            if target_filter_keyword(*keyword) =>
        {
            Ok(())
        }
        Atom::Power { value, .. }
        | Atom::Toughness { value, .. }
        | Atom::ManaValue { value, .. }
            if value.as_fixed().is_some() =>
        {
            Ok(())
        }
        Atom::IsNot {
            object: Ref::Source,
        } if top => Ok(()),
        Atom::AnyOf { alternatives }
            if alternatives.len() >= 2 && alternatives.iter().all(|a| !a.0.is_empty()) =>
        {
            alternatives
                .iter()
                .flat_map(|alternative| &alternative.0)
                .try_for_each(|atom| battlefield_target_atom(atom, false))
        }
        _ => unaudited("battlefield target predicate atom"),
    }
}

/// True when a target predicate excludes the targeting source ("another").
pub fn excludes_source(filter: &Pred) -> bool {
    filter.0.contains(&Atom::IsNot {
        object: Ref::Source,
    })
}

/// CR 205, 208, 202.3: a hidden card's printed characteristics, as the
/// core evaluates them for a card in a library or hand.
pub(in crate::kir) fn hidden_card_filter(filter: &Pred) -> bool {
    Walk::library_card_filter(filter).is_ok()
}

/// The target occurrences a node names, without judging the node. Program
/// validation uses this to prove every announced target is read (CR 601.2c);
/// [`check`] remains the admission decision.
pub fn target_slots(node: &Node) -> BTreeSet<u32> {
    let env = Env {
        context: Context::Spell,
        targets: &[],
        event: None,
        announced_x: false,
    };
    let mut walk = Walk {
        env: &env,
        scan: true,
        plain_leaf: false,
        bound: vec![],
        steps: Vec::new(),
        direct_sequence_member: false,
        out: DerivedRequirements::default(),
    };
    // A scan reports no scope or audit error; a malformed literal can only
    // end it early, and `check` reports that literal.
    let _ = walk.node(node, false);
    walk.out.target_slots
}

fn resolution_trigger_body(program: &SpellProgram) -> Result<&Node, KirError> {
    let [Instruction::Kernel { node, .. }] = program.instructions.as_slice() else {
        return unaudited("resolution trigger instruction count");
    };
    if program.resolution != SpellResolution::Instructions
        || program.timing != SpellTiming::Instant
        || program.functioning_zone != FunctioningZone::Stack
        || !program.optional_additional_costs.is_empty()
        || program.optional_target
        || program.additional_sacrifice.is_some()
    {
        return unaudited("resolution trigger program shell");
    }
    Ok(node)
}

struct Walk<'a, 'e> {
    env: &'a Env<'e>,
    /// Collect references only: no scope, kind or audit judgement.
    scan: bool,
    /// An ordinary leaf executor reads the invocation, without task bindings.
    plain_leaf: bool,
    /// Variables bound by enclosing `ForEach`/`Choose` nodes, with their kind.
    bound: Vec<(u32, RefKind)>,
    /// Completed steps of the innermost enclosing `Seq`.
    steps: Vec<&'static [StepRole]>,
    direct_sequence_member: bool,
    out: DerivedRequirements,
}

fn scope<T>(detail: impl Into<String>) -> Result<T, KirError> {
    Err(KirError::Scope(detail.into()))
}
fn kind<T>(detail: impl Into<String>) -> Result<T, KirError> {
    Err(KirError::Type(detail.into()))
}
fn unaudited<T>(detail: impl Into<String>) -> Result<T, KirError> {
    Err(KirError::Unaudited(detail.into()))
}
impl Walk<'_, '_> {
    fn domain(&self, slot: u32) -> Result<TargetDomain, KirError> {
        match self.env.targets.get(slot as usize) {
            Some(target) if target.slot == slot => Ok(target.domain),
            _ => scope(format!("target slot {slot} is not announced")),
        }
    }

    // The permanent filter is checked at both target boundaries (CR 601.2c,
    // 608.2b). A disjunction that includes another type cannot prove creature.
    fn batch_target_count(&self, count: u32, variable: Option<VariableTargetCount>) -> bool {
        match variable {
            None => (2..=7).contains(&count),
            Some(VariableTargetCount::Any) => count == 1,
            Some(_) => count == 1 && (self.scan || self.env.announced_x),
        }
    }

    fn creature_target(&self, slot: u32) -> Result<bool, KirError> {
        let domain = self.domain(slot)?;
        Ok(domain == TargetDomain::BattlefieldCreature
            || (matches!(
                domain,
                TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent
            ) && self.env.targets[slot as usize]
                .filter
                .as_ref()
                .is_some_and(|filter| filter.types == [CardKind::Creature])))
    }

    fn reference(&mut self, reference: &Ref) -> Result<RefKind, KirError> {
        if self.scan {
            if let Ref::Target { slot } = reference {
                self.out.target_slots.insert(*slot);
            }
            return Ok(RefKind::Object);
        }
        let found = match reference {
            Ref::Source if self.env.context == Context::Spell => {
                return unaudited("spell source as an object operand");
            }
            Ref::Source => RefKind::Object,
            Ref::Controller { role } => {
                if *role != self.env.context.controller_role() {
                    return scope(format!(
                        "controller role {role:?} in a {:?} context",
                        self.env.context
                    ));
                }
                RefKind::Player
            }
            Ref::Target { slot } => {
                let domain = self.domain(*slot)?;
                self.out.target_slots.insert(*slot);
                match domain {
                    TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::BattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature
                    | TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::CreatureOrPlaneswalker
                    | TargetDomain::OwnGraveyardCreatureCard
                    | TargetDomain::GraveyardCard { .. }
                    | TargetDomain::OwnGraveyardCard => RefKind::Object,
                    TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker => {
                        RefKind::Entity
                    }
                    TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer => RefKind::Player,
                    TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility => RefKind::StackObject,
                    TargetDomain::PlayerGraveyardCards { .. } => RefKind::Pool,
                }
            }
            Ref::EventPlayer { role } => {
                let Some(event) = self
                    .env
                    .event
                    .filter(|_| self.env.context == Context::Triggered)
                else {
                    return scope("event player outside a triggered ability");
                };
                if !event_supplies(event, *role) {
                    return scope(format!("event has no {role:?}"));
                }
                self.out.event_player = true;
                RefKind::Player
            }
            Ref::EventAmount => {
                let Some(event) = self
                    .env
                    .event
                    .filter(|_| self.env.context == Context::Triggered)
                else {
                    return scope("event amount outside a triggered ability");
                };
                if !event_supplies_amount(event) {
                    return scope("event carries no amount");
                }
                RefKind::Number
            }
            Ref::AnnouncedX => {
                if !self.env.announced_x {
                    return scope("X is not announced for this object");
                }
                self.out.announced_x = true;
                RefKind::Number
            }
            Ref::BoundVar { id } => match self.bound.iter().rev().find(|(bound, _)| bound == id) {
                Some((_, kind)) => *kind,
                None => return scope(format!("variable {id} is not bound")),
            },
            Ref::StepResult { step, role } => {
                let Some(producer) = self.steps.get(*step as usize) else {
                    return scope(format!("step {step} has no dominating projection"));
                };
                if !self.direct_sequence_member || !producer.contains(role) {
                    return scope(format!("step {step} does not supply {role:?} here"));
                }
                match role {
                    StepRole::Objects => RefKind::Pool,
                    StepRole::Player => RefKind::Player,
                    StepRole::Amount => RefKind::Number,
                }
            }
            // No audited cell reads these yet. They are in the vocabulary so
            // the binder and the checker agree on scope before the first verb
            // that needs them is audited.
            Ref::AttachedTo => return unaudited("attached-to reference"),
            Ref::EventObject { role } => {
                if self.env.event.is_none() || self.env.context != Context::Triggered {
                    return scope("event object outside a triggered ability");
                }
                if self
                    .env
                    .event
                    .and_then(|event| event.pattern().object_role())
                    != Some(*role)
                {
                    return scope(format!("event does not bind object {role:?}"));
                }
                RefKind::Object
            }
            Ref::EnteringObject => return unaudited("entering-object reference"),
            Ref::LinkedPool { .. } => return unaudited("linked pool reference"),
        };
        if self.plain_leaf {
            match reference {
                Ref::BoundVar { .. } | Ref::StepResult { .. } => {
                    return unaudited(
                        "ordinary kernel leaf reads a control binding or step result",
                    );
                }
                Ref::Target { .. } if self.env.targets.len() != 1 => {
                    return unaudited("nested leaf requires a single announced target");
                }
                _ => {}
            }
        }
        Ok(found)
    }

    fn expect(
        &mut self,
        reference: &Ref,
        wanted: &[RefKind],
        operand: &str,
    ) -> Result<RefKind, KirError> {
        let found = self.reference(reference)?;
        if self.scan || wanted.contains(&found) {
            Ok(found)
        } else {
            kind(format!("{operand} is a {found:?}"))
        }
    }

    fn player(&mut self, player: &PlayerExpr) -> Result<(), KirError> {
        match player {
            PlayerExpr::Is { who } => self.expect(who, &[RefKind::Player], "player").map(drop),
            PlayerExpr::ControllerOf { object } => self
                .expect(
                    object,
                    &[RefKind::Object, RefKind::StackObject],
                    "controlled object",
                )
                .map(drop),
            PlayerExpr::OwnerOf { object } => self
                .expect(object, &[RefKind::Object], "owned object")
                .map(drop),
            PlayerExpr::EachOpponent | PlayerExpr::EachPlayer => Ok(()),
        }
    }

    fn pred(&mut self, pred: &Pred) -> Result<(), KirError> {
        for atom in &pred.0 {
            match atom {
                Atom::Type { .. }
                | Atom::NotType { .. }
                | Atom::Legendary
                | Atom::NotLegendary
                | Atom::Basic
                | Atom::NotBasic
                | Atom::Color { .. }
                | Atom::NotColor { .. }
                | Atom::Tapped
                | Atom::Untapped
                | Atom::Attacking
                | Atom::Blocking
                | Atom::Token
                | Atom::NotToken
                | Atom::Keyword { .. }
                | Atom::NotKeyword { .. } => {}
                Atom::AnyType { kinds } => {
                    if kinds.is_empty() {
                        return kind("empty type alternatives");
                    }
                }
                Atom::Subtype { subtype } | Atom::NotSubtype { subtype } => {
                    if subtype.is_empty() {
                        return kind("empty subtype");
                    }
                }
                Atom::ControlledBy { player } | Atom::OwnedBy { player } => self.player(player)?,
                Atom::Power { value, .. }
                | Atom::Toughness { value, .. }
                | Atom::ManaValue { value, .. } => self.amount(value)?,
                Atom::IsNot { object } => {
                    self.expect(object, &[RefKind::Object], "excluded object")?;
                }
                // CR 201.5: identity with a reference is an event pattern's
                // comparison; a resolving set never names one object this way.
                Atom::Is { .. } => return unaudited("identity atom outside an event pattern"),
                Atom::AnyOf { alternatives } => {
                    if alternatives.is_empty() {
                        return kind("empty predicate alternatives");
                    }
                    for alternative in alternatives {
                        self.pred(alternative)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn zone(&mut self, zone: &ZoneExpr) -> Result<(), KirError> {
        match zone {
            ZoneExpr::Graveyard { of } | ZoneExpr::Hand { of } | ZoneExpr::Library { of } => {
                self.player(of)
            }
            ZoneExpr::Battlefield | ZoneExpr::Graveyards | ZoneExpr::Exile | ZoneExpr::Stack => {
                Ok(())
            }
        }
    }

    fn set(&mut self, set: &SetExpr) -> Result<(), KirError> {
        match set {
            SetExpr::One { object } => self
                .expect(
                    object,
                    &[RefKind::Object, RefKind::StackObject, RefKind::Entity],
                    "object",
                )
                .map(drop),
            SetExpr::Matching { zone, filter } => {
                self.zone(zone)?;
                self.pred(filter)
            }
            SetExpr::Pool { pool } => self.expect(pool, &[RefKind::Pool], "pool").map(drop),
        }
    }

    fn amount(&mut self, amount: &Amount) -> Result<(), KirError> {
        match amount {
            Amount::Fixed { .. } => Ok(()),
            Amount::Var { of } => self.expect(of, &[RefKind::Number], "amount").map(drop),
            Amount::Count { set } => {
                // CR 400.2, 402.3, 401.2: the size of a hand or library is
                // public, which cards it holds is not. A filtered count of a
                // hidden zone would publish the answer of a private question.
                if let SetExpr::Matching { zone, filter } = set
                    && zone.hidden()
                    && !filter.0.is_empty()
                {
                    return Err(KirError::HiddenCount);
                }
                self.set(set)
            }
            Amount::StatOf { object, .. } | Amount::CountersOn { object, .. } => self
                .expect(object, &[RefKind::Object], "measured object")
                .map(drop),
            Amount::Greatest { set, .. } => self.set(set),
            Amount::BlockersOf { object, .. } => self
                .expect(object, &[RefKind::Object], "blocked object")
                .map(drop),
            Amount::OpponentsAttacked {} => Ok(()),
            Amount::Times { of, .. } => self.amount(of),
            Amount::Sum { terms } => terms.iter().try_for_each(|term| self.amount(term)),
            Amount::Difference {
                minuend,
                subtrahend,
            } => {
                self.amount(minuend)?;
                self.amount(subtrahend)
            }
            Amount::Devotion { player, .. }
            | Amount::LifeOf { player }
            | Amount::OtherCastsManaValue { caster: player } => self.player(player),
            // CR 604.3: characteristic-defining values have no kernel reader.
            Amount::Distinct { .. } => kind("characteristic-defining amount outside its static"),
        }
    }

    fn damage_to(&mut self, to: &DamageTo) -> Result<(), KirError> {
        match to {
            DamageTo::Ref { who } => self
                .expect(
                    who,
                    &[RefKind::Object, RefKind::Player, RefKind::Entity],
                    "damage recipient",
                )
                .map(drop),
            DamageTo::Players { players } => self.player(players),
            DamageTo::Objects { set } => self.set(set),
            DamageTo::All { parts } => {
                if parts.is_empty() {
                    return kind("empty damage recipients");
                }
                parts.iter().try_for_each(|part| self.damage_to(part))
            }
        }
    }

    fn destination(&mut self, to: &Destination) -> Result<(), KirError> {
        match to {
            Destination::Battlefield { controller, .. } => self.player(controller),
            Destination::Exile
            | Destination::OwnerHand
            | Destination::OwnerLibraryTop
            | Destination::OwnerLibraryBottom
            | Destination::OwnerLibraryShuffled
            | Destination::OneTappedRestOwnerHand
            | Destination::OwnerGraveyard => Ok(()),
        }
    }

    fn control_player(&mut self, player: &PlayerExpr, single: bool) -> Result<(), KirError> {
        self.player(player)?;
        if self.scan {
            return Ok(());
        }
        match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. } | Ref::BoundVar { .. },
            } => Ok(()),
            PlayerExpr::Is {
                who: Ref::EventPlayer { role },
            } if Self::recipient_role(*role) => Ok(()),
            PlayerExpr::Is {
                who: Ref::Target { slot },
            } if self.env.targets[..=*slot as usize].iter().all(|target| {
                matches!(
                    target.domain,
                    TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer
                )
            }) =>
            {
                Ok(())
            }
            // CR 108.3: ownership is recorded on the physical card and
            // survives an incarnation's departure. These references always
            // supply one identity, including a singleton iteration binding.
            PlayerExpr::OwnerOf {
                object: Ref::Source | Ref::BoundVar { .. },
            } => Ok(()),
            PlayerExpr::EachPlayer | PlayerExpr::EachOpponent if !single => Ok(()),
            // Object-derived players can need an absent object's last
            // known information (CR 608.2h). The continuation has no such
            // lookup, and a target may already be illegal at resolution.
            _ => unaudited("control player expression"),
        }
    }

    fn control_set(&mut self, set: &SetExpr) -> Result<(), KirError> {
        self.set(set)?;
        self.audit_control_set(set)
    }

    fn audit_control_set(&self, set: &SetExpr) -> Result<(), KirError> {
        if self.scan {
            return Ok(());
        }
        match set {
            SetExpr::Matching {
                zone: zone @ (ZoneExpr::Battlefield | ZoneExpr::Graveyards),
                filter,
            } => Self::audited_filter(zone, filter),
            SetExpr::One {
                object: Ref::Source | Ref::BoundVar { .. },
            }
            | SetExpr::Pool {
                pool: Ref::BoundVar { .. },
            } => Ok(()),
            SetExpr::One {
                object: Ref::Target { slot },
            } if !matches!(
                self.domain(*slot)?,
                TargetDomain::AnyDamageTarget
                    | TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility
            ) =>
            {
                Ok(())
            }
            _ => unaudited("control object set"),
        }
    }

    fn control_amount(&mut self, amount: &Amount) -> Result<(), KirError> {
        self.amount(amount)?;
        if self.scan {
            return Ok(());
        }
        match amount {
            Amount::Fixed { .. } => Ok(()),
            Amount::Var {
                of: Ref::AnnouncedX,
            } => Ok(()),
            Amount::Count { set } => self.audit_control_set(set),
            // A life subject may be absent or have departed (CR 800.4i).
            // Cast-history amounts require a paid spell, including when a
            // program can also be run by a spell copy (CR 707.10).
            _ => unaudited("control amount"),
        }
    }

    fn nonnegative_control_amount(&mut self, amount: &Amount) -> Result<(), KirError> {
        self.control_amount(amount)?;
        if !self.scan && amount.as_fixed().is_some_and(Quantity::is_negative) {
            return kind("negative control amount");
        }
        Ok(())
    }

    fn condition(&mut self, condition: &Cond) -> Result<(), KirError> {
        match condition {
            Cond::Compare { left, right, .. } => {
                self.control_amount(left)?;
                self.control_amount(right)
            }
            Cond::Exists { set } => self.control_set(set),
            Cond::ActivePlayer { .. } => unaudited("active-player condition outside statics"),
            Cond::CostPaid { identity } => {
                if identity.is_empty() {
                    return kind("empty cost identity");
                }
                Ok(())
            }
            Cond::Not { condition } => self.condition(condition),
            Cond::All { conditions } => conditions.iter().try_for_each(|c| self.condition(c)),
            Cond::Satisfies {
                object: Ref::Target { slot },
                filter,
            } if self.scan
                || (matches!(
                    self.domain(*slot)?,
                    TargetDomain::GraveyardCard { .. }
                        | TargetDomain::OwnGraveyardCard
                        | TargetDomain::OwnGraveyardCreatureCard
                ) && filter.0.as_slice()
                    == [Atom::Type {
                        kind: CardKind::Creature,
                    }]) =>
            {
                self.expect(
                    &Ref::Target { slot: *slot },
                    &[RefKind::Object],
                    "graveyard card type",
                )?;
                if !self.scan {
                    self.out.cells.insert(Cell::GraveyardTargetWasCreature);
                }
                Ok(())
            }
            // CR 603.4: only an intervening "if" reads these, through
            // [`Intervening`]; no resolving control node does.
            Cond::Satisfies { .. }
            | Cond::CastBy { .. }
            | Cond::TurnHistory { .. }
            | Cond::ControlsOwnCommander { .. }
            | Cond::OpponentControlsMore { .. }
            | Cond::EventOpponentControlsMore { .. }
            | Cond::ControllerCreatureTotalPower { .. }
            | Cond::Measured { .. }
            | Cond::ControlledSinceLastUpkeep {} => {
                unaudited("intervening condition in a resolving node")
            }
        }
    }

    fn delayed(&mut self, event: &DelayedEvent) -> Result<(), KirError> {
        match event {
            DelayedEvent::NextUpkeep { of } => self.player(of),
            DelayedEvent::NextEndStepAfterExile { slot }
            | DelayedEvent::TargetLeavesAfterExilingSource { slot } => self
                .expect(
                    &Ref::Target { slot: *slot },
                    &[RefKind::Object],
                    "exile target",
                )
                .map(|_| ()),
            _ => Ok(()),
        }
    }

    /// Scope and type of every operand of a verb. Total over [`Verb`].
    fn operands(&mut self, verb: &Verb) -> Result<(), KirError> {
        match verb {
            Verb::ReturnCapturedExile {}
            | Verb::ExileCapturedReturn {}
            | Verb::SacrificeEncoreTokens {}
            | Verb::ExileMyriadTokens {}
            | Verb::ReturnDashedToHand {} => Ok(()),
            Verb::CreateMyriadToken { defender } => self.player(defender),
            Verb::Draw { player, amount }
            | Verb::GainLife { player, amount }
            | Verb::GiveCounters { player, amount, .. }
            | Verb::LoseLife { player, amount }
            | Verb::Mill { player, amount }
            | Verb::Discard { player, amount, .. }
            | Verb::ReorderLibraryTop { player, amount }
            | Verb::LookChooseHand { player, amount }
            | Verb::Scry { player, amount }
            | Verb::Surveil { player, amount } => {
                self.player(player)?;
                self.amount(amount)
            }
            Verb::Fight { first, second } => {
                self.expect(first, &[RefKind::Object], "first fighter")?;
                self.expect(second, &[RefKind::Object], "second fighter")
                    .map(|_| ())
            }
            Verb::Populate { player } | Verb::Proliferate { player } => self.player(player),
            Verb::Damage { source, to, amount } => {
                // CR 120.1: a spell can deal damage as its own source. This
                // attribution does not select the spell as a permanent.
                if *source != Ref::Source {
                    self.expect(source, &[RefKind::Object], "damage source")?;
                }
                self.damage_to(to)?;
                self.amount(amount)
            }
            Verb::Destroy { what, .. }
            | Verb::Tap { what }
            | Verb::Untap { what }
            | Verb::Regenerate { what } => self.set(what),
            // CR 701.24c: this is the resolving spell itself, rather than a
            // reference to one of the ordinary object domains.
            Verb::Move {
                what: SetExpr::One {
                    object: Ref::Source,
                },
                to: Destination::OwnerLibraryShuffled,
            } if self.env.context == Context::Spell => Ok(()),
            Verb::Move { what, to } => {
                self.set(what)?;
                self.destination(to)
            }
            Verb::RemoveLoreCounter { from } => self
                .expect(from, &[RefKind::Object], "lore removal target")
                .map(|_| ()),
            Verb::PutCounters {
                on,
                counter,
                amount,
            } => {
                if matches!(counter, CounterKind::Named { name } if name.is_empty()) {
                    return kind("empty counter name");
                }
                self.set(on)?;
                self.amount(amount)
            }
            Verb::CreateTokens {
                controller,
                count,
                token,
                ..
            } => {
                if let TokenSpec::Creature { spec } = token {
                    spec.validate().map_err(KirError::Type)?;
                }
                self.player(controller)?;
                self.amount(count)
            }
            Verb::Sacrifice {
                player,
                count,
                filter,
                ..
            } => {
                self.player(player)?;
                self.amount(count)?;
                // CR 701.21a: "sacrifice it" names one permanent, which a
                // spell's own source never is.
                if let [Atom::Is { object }] = filter.0.as_slice() {
                    return self
                        .expect(object, &[RefKind::Object], "sacrificed object")
                        .map(|_| ());
                }
                self.pred(filter)
            }
            Verb::AddMana { player, mana } => {
                self.player(player)?;
                match mana {
                    ManaSpec::Color { amount, .. } => self.amount(amount),
                    ManaSpec::AnyCombination { colors, amount } => {
                        if colors.is_empty() {
                            return kind("empty mana colors");
                        }
                        self.amount(amount)
                    }
                }
            }
            Verb::Search { player, search } => {
                self.player(player)?;
                self.pred(&search.filter)?;
                self.amount(&search.maximum)?;
                self.destination(&search.to)
            }
            Verb::Counter { what } | Verb::CopyStackObject { what } => self
                .expect(what, &[RefKind::StackObject], "stack object")
                .map(drop),
            Verb::GainControl {
                what, controller, ..
            } => {
                self.set(what)?;
                self.player(controller)
            }
            Verb::SetCharacteristics { what, .. } => self.set(what),
            Verb::Attach { what, to } => {
                self.expect(what, &[RefKind::Object], "attachment")?;
                self.expect(to, &[RefKind::Object], "attachment host")
                    .map(drop)
            }
            Verb::PreventDamage { prevention, .. } => {
                if let Some(to) = &prevention.to {
                    self.expect(
                        to,
                        &[RefKind::Object, RefKind::Player, RefKind::Entity],
                        "shielded recipient",
                    )?;
                }
                match &prevention.by {
                    Some(PreventionSources::One { object }) => self
                        .expect(object, &[RefKind::Object], "prevented damage source")
                        .map(drop),
                    Some(PreventionSources::Matching { .. }) | None => Ok(()),
                }
            }
            Verb::RestrictCombat { affects, .. } => match affects {
                CombatProhibitionScope::Target { target } => self
                    .expect(target, &[RefKind::Object], "restricted creature")
                    .map(drop),
                CombatProhibitionScope::Creatures {}
                | CombatProhibitionScope::CreaturesWithoutFlying {} => Ok(()),
            },
            Verb::ModifyUntil {
                what,
                layer,
                change,
                ..
            } => {
                // CR 613.1: the tag is the layer the change applies in.
                if *layer != change.layer() {
                    return kind(format!("{layer:?} tag on a {:?} change", change.layer()));
                }
                self.set(what)?;
                match change {
                    Change::PowerToughness { power, toughness }
                    | Change::SetPowerToughness { power, toughness } => {
                        self.amount(power)?;
                        self.amount(toughness)
                    }
                    Change::PowerToughnessX { power, toughness } => {
                        for delta in [power, toughness] {
                            match delta {
                                PumpDelta::Fixed { .. } => {}
                                PumpDelta::X { .. } => self
                                    .expect(
                                        &Ref::AnnouncedX,
                                        &[RefKind::Number],
                                        "announced pump X",
                                    )
                                    .map(drop)?,
                                PumpDelta::Counted { amount, .. } => self.amount(amount)?,
                            }
                        }
                        Ok(())
                    }
                    Change::AddKeywords { keywords } | Change::RemoveKeywords { keywords } => {
                        if keywords.is_empty() {
                            return kind("empty keyword change");
                        }
                        Ok(())
                    }
                    Change::SetColors { .. } => Ok(()),
                    Change::AddTypes { kinds } => {
                        if kinds.is_empty() {
                            return kind("empty type change");
                        }
                        Ok(())
                    }
                    Change::Controller { player } => self.player(player),
                    Change::AllCreatureTypes
                    | Change::AddCreatureSubtypes { .. }
                    | Change::SetCreatureSubtypes { .. } => kind("resolving creature-type change"),
                    // CR 205.1a, 305.7: resolving type replacement is not admitted.
                    Change::SetCardTypes { .. }
                    | Change::RemoveCardTypes { .. }
                    | Change::SetLandSubtypes { .. } => kind("resolving type replacement"),
                    // CR 113.10, 613.1f: resolving ability loss is not admitted.
                    Change::RemoveAllAbilities { .. } | Change::ForbidKeywords { .. } => {
                        kind("resolving ability loss")
                    }
                    // CR 113.10: resolving ability grants are not admitted.
                    Change::AddManaAbilities { .. } => kind("resolving mana ability grant"),
                }
            }
            Verb::CreateDelayedTrigger { event, node } => {
                self.delayed(event)?;
                self.nested(node, false)
            }
            Verb::PlayPermission { player, what, .. } => {
                self.player(player)?;
                self.set(what)
            }
            Verb::Transform { what } => self.expect(what, &[RefKind::Object], "object").map(drop),
            Verb::SetDesignation { player, .. } | Verb::Shuffle { player } => self.player(player),
            Verb::Builtin { .. } => Ok(()),
        }
    }

    /// A child node with its own step numbering.
    fn nested(&mut self, node: &Node, in_control: bool) -> Result<(), KirError> {
        let steps = std::mem::take(&mut self.steps);
        let result = self.node(node, in_control);
        self.steps = steps;
        result
    }

    fn node(&mut self, node: &Node, in_control: bool) -> Result<(), KirError> {
        match node {
            Node::Do(verb) => {
                match verb {
                    // CR 702.116a: one opponent bound by the myriad
                    // iteration; the whole ability is the reviewed builder
                    // (`TriggeredAbility::is_myriad`).
                    Verb::CreateMyriadToken { defender } if in_control => {
                        self.control_player(defender, true)?;
                        if !self.scan {
                            if self.env.context != Context::Triggered
                                || !matches!(
                                    defender,
                                    PlayerExpr::Is {
                                        who: Ref::BoundVar { .. }
                                    }
                                )
                                || !self.env.event.is_some_and(|event| {
                                    *event.pattern()
                                        == (EventPattern::Attacks {
                                            attacker: Pred::source(),
                                        })
                                })
                            {
                                return unaudited("myriad token creation");
                            }
                            self.out.cells.insert(Cell::Myriad);
                        }
                    }
                    _ => {
                        let enclosing_leaf = std::mem::replace(
                            &mut self.plain_leaf,
                            in_control && !verb.is_evaluated(),
                        );
                        let operands = self.operands(verb);
                        self.plain_leaf = enclosing_leaf;
                        operands?;
                        if !self.scan {
                            let cell = self.audit(verb)?;
                            self.out.cells.insert(cell);
                            if in_control {
                                Self::audit_nested_leaf(verb)?;
                            }
                        }
                    }
                }
                if !self.scan {
                    verb_capabilities(verb, &mut self.out.capabilities);
                }
                Ok(())
            }
            Node::Seq(nodes) => {
                let steps = std::mem::take(&mut self.steps);
                let direct = self.direct_sequence_member;
                let result = nodes.iter().try_for_each(|node| {
                    self.direct_sequence_member = matches!(node, Node::Do(_));
                    self.node(node, true)?;
                    self.steps.push(
                        step_producer(node).map_or(&[], |(signature, _)| signature.projections),
                    );
                    Ok(())
                });
                self.steps = steps;
                self.direct_sequence_member = direct;
                result
            }
            Node::If {
                condition,
                then,
                otherwise,
            } => {
                self.condition(condition)?;
                self.nested(then, true)?;
                if let Some(otherwise) = otherwise {
                    self.nested(otherwise, true)?;
                }
                Ok(())
            }
            Node::Optional {
                chooser,
                node: child,
            } => {
                if self.target_controller_chooser(chooser) {
                    // CR 608.2h, 800.4a, 800.4g: the last-known controller
                    // of a filtered target may only search or shuffle their
                    // own library. Once they leave, that library left with
                    // them, so skipping the choice deprives no live player.
                    self.player(chooser)?;
                    if !self.scan && !Self::own_library(child, chooser) {
                        return unaudited("target controller choice outside their library");
                    }
                } else {
                    self.control_player(chooser, true)?;
                }
                // CR 608.2d: "you may destroy target permanent", a choice the
                // resolving frame offers at this instruction's cursor.
                if let Some((verb, slot, regeneration_forbidden)) = node.optional_target_destroy() {
                    self.operands(verb)?;
                    if !self.scan {
                        if in_control {
                            return unaudited(
                                "nested optional target destroy uses a different choice route",
                            );
                        }
                        if regeneration_forbidden
                            || self.domain(slot)? != TargetDomain::BattlefieldPermanent
                        {
                            return unaudited("optional destroy target domain");
                        }
                        verb_capabilities(verb, &mut self.out.capabilities);
                        self.out.cells.insert(Cell::OptionalTargetDestroy);
                    }
                    return Ok(());
                }
                // CR 111.2, 608.2d: a controller may create a fixed recipe.
                // Token recovery recognizes recipes at a direct optional
                // instruction's child address.
                if let Node::Do(Verb::CreateTokens {
                    controller, count, ..
                }) = child.as_ref()
                {
                    if in_control
                        || controller != chooser
                        || !controller.is_you()
                        || count.as_fixed().is_none()
                    {
                        return unaudited(
                            "optional token creation needs a fixed controller recipe",
                        );
                    }
                    return self.nested(child, false);
                }
                self.nested(child, true)
            }
            Node::ForEach { var, over, body } => {
                let bound = match over {
                    Domain::Players { players } => {
                        self.control_player(players, false)?;
                        RefKind::Player
                    }
                    Domain::Objects { set } => {
                        self.control_set(set)?;
                        RefKind::Object
                    }
                };
                self.bound.push((*var, bound));
                let result = self.nested(body, true);
                self.bound.pop();
                result
            }
            Node::Choose {
                var,
                chooser,
                from,
                minimum,
                maximum,
                body,
            } => {
                self.control_player(chooser, true)?;
                self.control_set(from)?;
                self.nonnegative_control_amount(minimum)?;
                self.nonnegative_control_amount(maximum)?;
                if !self.scan {
                    Self::audit_choice(from, minimum, maximum)?;
                }
                self.bound.push((*var, RefKind::Pool));
                let result = self.nested(body, true);
                self.bound.pop();
                result
            }
            Node::TapTargets {
                count, variable, ..
            } => {
                // CR 115.3, 601.2c, 608.2b: one occurrence, at most N distinct
                // objects. The program admits these slots only as one group.
                if !self.batch_target_count(*count, *variable) {
                    return unaudited("tap target group bounds");
                }
                self.out.target_slots.extend(0..*count);
                if !self.scan {
                    if in_control
                        || self.env.targets.len() != *count as usize
                        || self.env.targets.iter().any(|t| {
                            !matches!(
                                t.domain,
                                TargetDomain::BattlefieldCreature
                                    | TargetDomain::BattlefieldPermanent
                                    | TargetDomain::ControlledBattlefieldPermanent
                            )
                        })
                    {
                        return unaudited("tap target group");
                    }
                    self.out.cells.insert(Cell::TapTargetGroup);
                    self.out
                        .capabilities
                        .insert(Capability::OptionalTargetGroupV1);
                }
                Ok(())
            }
            Node::SacrificeForTappedReturn { count } => {
                if !(2..=7).contains(count) {
                    return unaudited("sacrifice return group bounds");
                }
                self.out.target_slots.extend(0..*count);
                if !self.scan {
                    if in_control
                        || self.env.context != Context::Spell
                        || self.env.targets.len() != *count as usize
                        || self
                            .env
                            .targets
                            .iter()
                            .any(|t| t.domain != TargetDomain::OwnGraveyardCreatureCard)
                    {
                        return unaudited("sacrifice return target group");
                    }
                    self.out.cells.insert(Cell::SacrificeForTappedReturn);
                    self.out
                        .capabilities
                        .extend([Capability::SacrificeV1, Capability::GraveyardRecursionV1]);
                }
                Ok(())
            }
            Node::RemoveTargets {
                count, variable, ..
            } => {
                // CR 115.3, 601.2c, 608.2b: one occurrence, exactly or at most N distinct
                // objects. The program admits these slots only as one group.
                if !self.batch_target_count(*count, *variable) {
                    return unaudited("removal target group bounds");
                }
                self.out.target_slots.extend(0..*count);
                if !self.scan {
                    if in_control
                        || self.env.targets.len() != *count as usize
                        || self.env.targets.iter().any(|t| {
                            !matches!(
                                t.domain,
                                TargetDomain::BattlefieldCreature
                                    | TargetDomain::BattlefieldPermanent
                                    | TargetDomain::ControlledBattlefieldPermanent
                            )
                        })
                    {
                        return unaudited("removal target group");
                    }
                    self.out.cells.insert(Cell::RemoveTargetGroup);
                    self.out
                        .capabilities
                        .insert(Capability::TargetGroupRemovalV1);
                }
                Ok(())
            }
            Node::ChooseObject { from, action } => {
                self.control_set(from)?;
                if !self.scan {
                    if in_control
                        || !matches!(from, SetExpr::Matching { zone: ZoneExpr::Battlefield, filter }
                        if filter.0.contains(&Atom::ControlledBy {
                            player: PlayerExpr::you(self.env.context),
                        }))
                        || *action != ObjectChoiceAction::ReturnToOwnerHand
                    {
                        return unaudited("resolution object choice");
                    }
                    self.out.cells.insert(Cell::ChooseObjectOwnerHand);
                    self.out
                        .capabilities
                        .insert(Capability::ResolutionObjectChoiceV1);
                }
                Ok(())
            }
            Node::UnlessPays {
                payer,
                payment,
                node,
            } if self.counter_payment(payer, payment, node) => {
                self.player(payer)?;
                self.node(node, false)
            }
            Node::UnlessPays {
                payer,
                payment,
                node,
            }
            | Node::IfPays {
                payer,
                payment,
                node,
            } => {
                self.control_player(payer, true)?;
                match payment {
                    Payment::Generic { amount } | Payment::Life { amount } => {
                        self.nonnegative_control_amount(amount)?
                    }
                    Payment::Mana { mana: cost }
                        if !cost.generic.is_negative() && !cost.colored.is_empty() => {}
                    Payment::Mana { .. } => return unaudited("invalid fixed mana payment"),
                    Payment::Sacrifice { count: 1, filter }
                        if payer.is_you() && *filter == Pred::creatures() => {}
                    Payment::Sacrifice { .. } => return unaudited("unsupported sacrifice payment"),
                    Payment::DiscardArtifact {} => {
                        if !payer.is_you() {
                            return unaudited("object payment payer");
                        }
                    }
                }
                self.nested(node, true)
            }
            Node::Delayed { event, program } => {
                if let DelayedEvent::NextEndStepAfterExile { slot }
                | DelayedEvent::TargetLeavesAfterExilingSource { slot } = event
                {
                    self.out.target_slots.insert(*slot);
                }
                if self.scan {
                    return Ok(());
                }
                if in_control {
                    return unaudited("nested delayed trigger");
                }
                let child = resolution_trigger_body(program)?;
                match (event, child) {
                    (DelayedEvent::NextEndStepAfterExile { slot }, Node::Do(Verb::ReturnCapturedExile {}))
                        if self.creature_target(*slot)? && program.targets.is_empty() => {}
                    (DelayedEvent::TargetLeavesAfterExilingSource { slot }, Node::Do(Verb::ReturnCapturedExile {}))
                        if self.env.context == Context::Triggered
                            && self.env.event.is_some_and(|event| matches!(event.pattern(), EventPattern::Dies { object }
                                if *object == Pred::source() || *event.pattern() == EventPattern::self_dies()))
                            && self.creature_target(*slot)?
                            && program.targets.is_empty() => {}
                    // CR 702.84a: a graveyard source's own unearth ability.
                    (DelayedEvent::NextEndStepAfterUnearth, Node::Do(Verb::ExileCapturedReturn {}))
                        if self.env.context == Context::Activated && program.targets.is_empty() => {
                        self.out.cells.insert(Cell::Unearth);
                    }
                    // CR 702.116a: a myriad ability's end-of-combat exile.
                    (DelayedEvent::EndOfCombat, Node::Do(Verb::ExileMyriadTokens {}))
                        if self.env.context == Context::Triggered && program.targets.is_empty() => {
                        self.out.cells.insert(Cell::Myriad);
                    }
                    // CR 702.141a: a graveyard source's own encore ability.
                    (DelayedEvent::NextEndStepAfterEncore, Node::Do(Verb::SacrificeEncoreTokens {}))
                        if self.env.context == Context::Activated && program.targets.is_empty() => {
                        self.out.cells.insert(Cell::Encore);
                    }
                    (DelayedEvent::NextUpkeep { of }, Node::Do(Verb::Discard { player, amount, random: true }))
                        if *of == PlayerExpr::you(self.env.context) && *player == PlayerExpr::you(Context::Triggered)
                            && amount.as_fixed().is_some_and(|n| *n > Quantity::zero())
                            && program.targets.is_empty() => {
                        self.out.cells.insert(Cell::DiscardTargetRandom);
                        self.out.capabilities.insert(Capability::RandomPlayerDiscardV1);
                    }
                    (DelayedEvent::NextEndStep | DelayedEvent::NextTurnUpkeep | DelayedEvent::NextUpkeep { .. },
                        Node::Do(Verb::Draw { player, amount }))
                        if player.is_you() && amount.as_fixed().is_some_and(|n| *n > Quantity::zero())
                            && program.targets.is_empty() => {
                        if let DelayedEvent::NextUpkeep { of } = event
                            && *of != PlayerExpr::you(self.env.context)
                        {
                            return unaudited("delayed upkeep player");
                        }
                        self.resolution_trigger_child(program, child)?;
                    }
                    (DelayedEvent::NextEndStep, Node::Do(Verb::CreateTokens { .. }))
                        if program.targets.is_empty() => self.resolution_trigger_child(program, child)?,
                    _ => return unaudited("delayed trigger event and body"),
                }
                self.resolution_trigger();
                Ok(())
            }
            Node::Reflexive { payment, program } => {
                if self.scan {
                    return Ok(());
                }
                if in_control
                    || self.env.context != Context::Triggered
                    || payment.generic.is_negative()
                    || payment.generic.to_u32().is_none_or(|n| n > 32)
                    || payment.colored.len() > 32
                    || (payment.generic == Quantity::zero() && payment.colored.is_empty())
                {
                    return unaudited("reflexive trigger payment");
                }
                let child = resolution_trigger_body(program)?;
                if !matches!(
                    child,
                    Node::Do(
                        Verb::Damage { .. }
                            | Verb::PutCounters { .. }
                            | Verb::Tap { .. }
                            | Verb::Destroy { .. }
                            | Verb::Draw { .. }
                    )
                ) || program.targets.len() != 1
                    || !matches!(
                        program.targets[0].domain,
                        TargetDomain::AnyDamageTarget
                            | TargetDomain::AnyPlayer
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::BattlefieldPermanent
                    )
                {
                    return unaudited("reflexive trigger body");
                }
                self.resolution_trigger_child(program, child)?;
                self.resolution_trigger();
                Ok(())
            }
        }
    }

    fn resolution_trigger(&mut self) {
        self.out
            .capabilities
            .insert(Capability::ResolutionTriggerV1);
        self.out.cells.insert(Cell::ResolutionTrigger);
    }

    fn resolution_trigger_child(
        &mut self,
        program: &SpellProgram,
        child: &Node,
    ) -> Result<(), KirError> {
        super::tiers::validate_resolution_targets(program).map_err(KirError::Unaudited)?;
        let admitted = check(
            child,
            &Env {
                context: Context::Triggered,
                targets: &program.targets,
                event: None,
                announced_x: false,
            },
        )?;
        if admitted
            .target_slots
            .iter()
            .copied()
            .ne(0..program.targets.len() as u32)
        {
            return unaudited("resolution trigger target use");
        }
        self.out.cells.extend(admitted.cells);
        self.out.capabilities.extend(admitted.capabilities);
        Ok(())
    }

    fn audit_choice(from: &SetExpr, minimum: &Amount, maximum: &Amount) -> Result<(), KirError> {
        if let (Some(min), Some(max)) = (minimum.as_fixed(), maximum.as_fixed())
            && min > max
        {
            return kind("reversed choice bounds");
        }
        if !matches!(from, SetExpr::Matching { .. }) {
            return unaudited("choice needs a current public object set");
        }
        // CR 800.4a/g: an interactive choice stores candidates and a chooser
        // across concessions. Core cannot replace a departed chooser or
        // refresh candidates. These bounds force the empty or entire set.
        if minimum == maximum
            && (minimum.as_fixed() == Some(&Quantity::zero())
                || matches!(minimum, Amount::Count { set } if set == from))
        {
            return Ok(());
        }
        unaudited("choice can suspend with stale candidates")
    }

    fn audit_nested_leaf(verb: &Verb) -> Result<(), KirError> {
        match verb {
            Verb::Move {
                to: Destination::OwnerLibraryShuffled,
                ..
            } => unaudited("library shuffle movement needs a top-level instruction"),
            // These executors authenticate against top-level instructions
            // after the temporary leaf program has been removed.
            Verb::CreateTokens { .. }
            | Verb::ModifyUntil { .. }
            | Verb::RestrictCombat { .. }
            | Verb::PreventDamage { .. }
            | Verb::GainControl { .. }
            | Verb::SetCharacteristics { .. } => {
                unaudited("nested leaf needs persistent provenance")
            }
            Verb::Fight { .. }
            | Verb::Populate { .. }
            | Verb::Proliferate { .. }
            | Verb::GiveCounters { .. } => {
                unaudited("keyword action needs a top-level instruction")
            }
            Verb::Counter { .. } => unaudited("counter requires a direct instruction or payment"),
            Verb::Regenerate { .. } => unaudited("regeneration needs persistent provenance"),
            Verb::RemoveLoreCounter { .. } => unaudited("lore removal needs a direct instruction"),
            // The completed reorder is validated against its own fixed
            // amount at the resolving instruction's cursor.
            Verb::ReorderLibraryTop { .. } | Verb::LookChooseHand { .. } => {
                unaudited("library reorder needs a top-level instruction")
            }
            // The seeded sampler reads its instruction at the cursor.
            Verb::Search { search, .. } if search.random => {
                unaudited("random card selection needs a top-level instruction")
            }
            _ => Ok(()),
        }
    }

    /// A fixed amount of at least `minimum`.
    fn fixed_at_least(amount: &Amount, minimum: i64, what: &str) -> Result<(), KirError> {
        match amount.as_fixed() {
            Some(value) if *value >= Quantity::from(minimum) => Ok(()),
            Some(_) => kind(format!("{what} below {minimum}")),
            None => unaudited(format!("computed {what}")),
        }
    }

    /// A fixed amount of at least `minimum`, or the announced X (CR 107.3).
    /// Returns true for the announced X.
    /// The event-player roles whose player the life and draw executors read:
    /// a cast trigger's caster and a step trigger's active player.
    fn recipient_role(role: EventPlayerRole) -> bool {
        matches!(
            role,
            EventPlayerRole::Caster
                | EventPlayerRole::ActivePlayer
                | EventPlayerRole::DrawingPlayer
                | EventPlayerRole::DefendingPlayer
        )
    }

    fn fixed_or_announced_x(amount: &Amount, minimum: i64, what: &str) -> Result<bool, KirError> {
        match amount {
            Amount::Var {
                of: Ref::AnnouncedX,
            } => Ok(true),
            _ => Self::fixed_at_least(amount, minimum, what).map(|()| false),
        }
    }

    /// The target domain of a player expression that names one target.
    fn player_target(&self, player: &PlayerExpr) -> Result<Option<TargetDomain>, KirError> {
        player
            .target_slot()
            .map(|slot| self.domain(slot))
            .transpose()
    }

    /// Filtered battlefield targets save current or last-known facts.
    fn target_has_facts(&self, slot: u32) -> bool {
        self.env.targets.get(slot as usize).is_some_and(|target| {
            target.domain == TargetDomain::BattlefieldPermanent && target.filter.is_some()
        })
    }

    /// CR 608.2h: counts evaluated by the flat verb executor.
    /// CR 608.2h: a number the game state supplies as the verb reads it: a
    /// count of an audited set, the greatest power, toughness or mana value
    /// among an audited battlefield set, or the controller's devotion
    /// (CR 700.5).
    pub(super) fn audit_verb_amount(amount: &Amount) -> Result<(), KirError> {
        match amount {
            Amount::Count { set } => Self::audit_verb_count(set),
            Amount::Greatest {
                set:
                    SetExpr::Matching {
                        zone: zone @ ZoneExpr::Battlefield,
                        filter,
                    },
                ..
            } => Self::audited_filter(zone, filter),
            Amount::Devotion { player, colors }
                if player.is_you() && !colors.is_empty() && !colors.contains(&Color::C) =>
            {
                Ok(())
            }
            Amount::Times { factor, of }
                if *factor > Quantity::from(1) && !matches!(**of, Amount::Times { .. }) =>
            {
                Self::audit_verb_amount(of)
            }
            // CR 122.1, 608.2h: the source's own admitted counter kinds.
            Amount::CountersOn {
                object: Ref::Source,
                counter,
            } if counter.admitted() => Ok(()),
            // CR 702.23a: creatures blocking the ability's own source.
            // CR 702.121a: the combat's declared defenders.
            Amount::BlockersOf {
                object: Ref::Source,
                ..
            }
            | Amount::OpponentsAttacked {} => Ok(()),
            _ => unaudited("resolution count"),
        }
    }

    fn audit_verb_count(set: &SetExpr) -> Result<(), KirError> {
        match set {
            SetExpr::Matching {
                zone: zone @ (ZoneExpr::Battlefield | ZoneExpr::Graveyards),
                filter,
            } => Self::audited_filter(zone, filter),
            SetExpr::Matching {
                zone: ZoneExpr::Graveyard { of },
                filter,
            } if of.is_you() => Self::audited_filter(&ZoneExpr::Graveyards, filter),
            SetExpr::Matching {
                zone: ZoneExpr::Hand { of },
                filter,
            } if of.is_you() && filter.0.is_empty() => Ok(()),
            _ => unaudited("count set"),
        }
    }

    /// The audited cell a well-scoped verb occupies. A cell is opened only
    /// where the core interpreter executes exactly these operands in this
    /// context and recovery re-derives the same facts; each verb's cells live
    /// in its own module. Total over [`Verb`]: a verb with no module has no
    /// audited cell.
    fn audit(&self, verb: &Verb) -> Result<Cell, KirError> {
        match verb {
            Verb::GainLife { .. }
            | Verb::LoseLife { .. }
            | Verb::Draw { .. }
            | Verb::Damage { .. }
            | Verb::PutCounters { .. }
            | Verb::Sacrifice { .. } => self.audit_evaluated(verb),
            Verb::GiveCounters { player, amount, .. } => self.audit_player_counters(player, amount),
            Verb::Fight { first, second } => self.audit_fight(first, second),
            Verb::Populate { player } => Self::audit_keyword_player(player, Cell::Populate),
            Verb::Proliferate { player } => Self::audit_keyword_player(player, Cell::Proliferate),
            Verb::Destroy {
                what,
                regeneration_forbidden,
            } => self.audit_destroy(what, *regeneration_forbidden),
            Verb::Counter { what } => self.audit_counter(what),
            Verb::Regenerate { what } => self.audit_regenerate(what),
            Verb::Move { what, to } => self.audit_move(what, to),
            Verb::Tap { what } => self.audit_tap(what),
            Verb::Untap { what } => self.audit_untap(what),
            Verb::Mill { player, amount } => self.audit_mill(player, amount),
            Verb::Discard {
                player,
                amount,
                random,
            } => self.audit_discard(player, amount, *random),
            Verb::CreateTokens {
                controller,
                count,
                token,
                ..
            } => self.audit_tokens(controller, count, token),
            Verb::RemoveLoreCounter { from } => match from {
                Ref::Target { slot }
                    if matches!(
                        self.domain(*slot)?,
                        TargetDomain::BattlefieldPermanent
                            | TargetDomain::ControlledBattlefieldPermanent
                    ) =>
                {
                    Ok(Cell::RemoveLoreCounter)
                }
                _ => unaudited("lore counter removal recipient"),
            },
            Verb::ModifyUntil {
                what,
                layer,
                change,
                until,
            } => self.audit_modify(what, *layer, change, *until),
            Verb::GainControl {
                what,
                controller,
                until,
            } => self.audit_gain_control(what, controller, *until),
            Verb::SetCharacteristics {
                what,
                values,
                until,
            } => self.audit_characteristic_set(what, values, *until),
            Verb::RestrictCombat {
                affects,
                restriction,
                until,
            } => self.audit_combat_prohibition(affects, *restriction, *until),
            Verb::PreventDamage { prevention, until } => {
                self.audit_resolved_prevention(prevention, *until)
            }
            Verb::Search { player, search } => self.audit_search(player, search),
            Verb::Shuffle { player } => self.audit_shuffle(player),
            Verb::ReorderLibraryTop { player, amount } => {
                self.audit_look(player, amount, Cell::ReorderLibraryTop)
            }
            Verb::LookChooseHand { player, amount } => {
                self.audit_look(player, amount, Cell::LookChooseHand)
            }
            Verb::Scry { player, amount } => self.audit_look(player, amount, Cell::ScryController),
            Verb::Surveil { player, amount } => {
                self.audit_look(player, amount, Cell::SurveilController)
            }
            Verb::Attach {
                what: Ref::Source,
                to: Ref::Target { slot: 0 },
            } if self.env.context == Context::Activated
                && self.domain(0)? == TargetDomain::ControlledBattlefieldCreature =>
            {
                Ok(Cell::Attachment)
            }
            Verb::AddMana {
                player,
                mana: ManaSpec::Color { amount, .. },
            } if self.env.context == Context::Spell
                && *player == PlayerExpr::you(Context::Spell)
                && amount.is_resolution_count() =>
            {
                Self::audit_verb_amount(amount)?;
                Ok(Cell::SpellManaCounted)
            }
            Verb::AddMana { .. }
            | Verb::CopyStackObject { .. }
            | Verb::Attach { .. }
            | Verb::ReturnCapturedExile {}
            | Verb::ExileCapturedReturn {}
            | Verb::SacrificeEncoreTokens {}
            | Verb::CreateMyriadToken { .. }
            | Verb::ExileMyriadTokens {}
            | Verb::ReturnDashedToHand {}
            | Verb::CreateDelayedTrigger { .. }
            | Verb::PlayPermission { .. }
            | Verb::SetDesignation { .. }
            | Verb::Builtin { .. } => unaudited("verb"),
            // CR 701.27a,f: a permanent's own activated or triggered ability
            // transforms that permanent.
            Verb::Transform { what: Ref::Source }
                if matches!(self.env.context, Context::Activated | Context::Triggered) =>
            {
                Ok(Cell::TransformSource)
            }
            Verb::Transform { .. } => unaudited("transform operand"),
        }
    }

    /// The atoms the core can evaluate for a set in this zone (CR 608.2h).
    fn audited_filter(zone: &ZoneExpr, filter: &Pred) -> Result<(), KirError> {
        for atom in &filter.0 {
            match atom {
                // CR 110.5d, 506.3–4, 109.4: graveyard cards have no
                // permanent status, combat participation, or controller.
                Atom::Tapped | Atom::Untapped | Atom::Attacking | Atom::Blocking
                | Atom::ControlledBy { .. } if *zone != ZoneExpr::Battlefield => {
                    return unaudited("battlefield predicate outside the battlefield");
                }
                Atom::Type { .. }
                | Atom::NotType { .. }
                | Atom::AnyType { .. }
                | Atom::Subtype { .. }
                | Atom::NotSubtype { .. }
                | Atom::Legendary
                | Atom::NotLegendary
                | Atom::Basic
                | Atom::NotBasic
                | Atom::Color { .. }
                | Atom::NotColor { .. }
                | Atom::Tapped
                | Atom::Untapped
                | Atom::Attacking
                // CR 111.6: tokenhood is identity. Graveyard card sets
                // already exclude tokens regardless of this predicate.
                | Atom::Token
                | Atom::NotToken
                | Atom::Keyword { .. }
                | Atom::NotKeyword { .. } => {}
                Atom::ControlledBy { player } => {
                    if !player.is_you() && *player != PlayerExpr::EachOpponent {
                        return unaudited("set controller");
                    }
                }
                Atom::Power { value, .. }
                | Atom::Toughness { value, .. }
                | Atom::ManaValue { value, .. } => {
                    if value.as_fixed().is_none() {
                        return unaudited("computed set comparison");
                    }
                }
                Atom::AnyOf { alternatives } => {
                    alternatives
                        .iter()
                        .try_for_each(|alternative| Self::audited_filter(zone, alternative))?;
                }
                Atom::Blocking | Atom::OwnedBy { .. } | Atom::IsNot { .. } | Atom::Is { .. } => {
                    return unaudited("set filter atom");
                }
            }
        }
        Ok(())
    }
}

/// A verb names the reviewed runtime surface it uses; each verb's module
/// names its own. The program adds any requirements that also depend on its
/// targets (`Definition::requirements`).
fn verb_capabilities(verb: &Verb, out: &mut BTreeSet<Capability>) {
    match verb {
        Verb::GainLife { player, .. } | Verb::LoseLife { player, .. } => {
            life::capabilities(matches!(verb, Verb::GainLife { .. }), player, out)
        }
        Verb::Draw { player, amount } => draw::capabilities(player, amount, out),
        Verb::GiveCounters { .. } => {
            out.insert(Capability::PlayerCountersV1);
        }
        Verb::Fight { .. } => {
            out.extend([Capability::FightV1, Capability::NoncombatDamageV1]);
        }
        Verb::Populate { .. } => {
            out.insert(Capability::PopulateV1);
        }
        Verb::Proliferate { .. } => {
            out.insert(Capability::ProliferateV1);
        }
        Verb::Damage { to, amount, .. } => damage::capabilities(to, amount, out),
        Verb::Destroy { what, .. } => destroy::capabilities(what, out),
        Verb::Counter { .. } => {
            out.insert(Capability::CounterStackObjectV1);
        }
        Verb::Regenerate { .. } => {
            out.insert(Capability::RegenerationShieldV1);
        }
        Verb::Move { what, to } => move_zone::capabilities(what, to, out),
        Verb::Tap { what } => tap::tap_capabilities(what, out),
        Verb::Transform { .. } => {
            out.insert(Capability::TransformSourceV1);
        }
        Verb::Untap { what } => tap::untap_capabilities(what, out),
        Verb::CreateTokens { token, .. } => tokens::capabilities(token, out),
        Verb::PutCounters {
            counter, amount, ..
        } => counters::capabilities(counter, amount, out),
        Verb::RemoveLoreCounter { .. } => {
            out.insert(Capability::SagaV1);
        }
        Verb::ModifyUntil { what, change, .. } => modify::capabilities(what, change, out),
        Verb::GainControl { .. } => {
            out.insert(Capability::ResolvedControlV1);
        }
        Verb::SetCharacteristics { .. } => {
            out.insert(Capability::ResolvedCharacteristicsV1);
        }
        Verb::RestrictCombat { .. } => {
            out.insert(Capability::TemporaryCombatProhibitionV1);
        }
        Verb::PreventDamage { prevention, .. } => modify::prevention_capabilities(prevention, out),
        Verb::Mill { amount, .. } => mill::capabilities(amount, out),
        Verb::Discard { player, random, .. } => discard::capabilities(player, *random, out),
        Verb::Sacrifice { player, filter, .. } => sacrifice::capabilities(player, filter, out),
        Verb::Search { search, .. } if search.from != SelectionZone::Library => {
            out.insert(Capability::ResolutionCardChoiceV1);
        }
        Verb::Search { search, .. } if search.to == Destination::OwnerLibraryTop => {
            out.insert(Capability::SearchLibraryTopV1);
        }
        Verb::Search { search, .. } if search.to == Destination::OneTappedRestOwnerHand => {
            out.insert(Capability::SearchSplitBasicLandsV1);
        }
        Verb::ReorderLibraryTop { .. } => {
            out.insert(Capability::PrivateLibraryReorderV1);
        }
        Verb::LookChooseHand { .. } => {
            out.insert(Capability::PrivateLibrarySelectionV1);
        }
        Verb::Scry { .. }
        | Verb::Surveil { .. }
        | Verb::AddMana { .. }
        | Verb::Search { .. }
        | Verb::CopyStackObject { .. }
        | Verb::Attach { .. }
        | Verb::ReturnCapturedExile {}
        | Verb::ExileCapturedReturn {}
        | Verb::SacrificeEncoreTokens {}
        | Verb::ExileMyriadTokens {}
        | Verb::ReturnDashedToHand {}
        | Verb::CreateDelayedTrigger { .. }
        | Verb::PlayPermission { .. }
        | Verb::SetDesignation { .. }
        | Verb::Shuffle { .. }
        | Verb::Builtin { .. } => {}
        Verb::CreateMyriadToken { .. } => {
            out.insert(Capability::MyriadV1);
        }
    }
}

/// The capabilities a node derives, independent of where it resolves.
pub fn node_capabilities(node: &Node) -> BTreeSet<Capability> {
    let mut out = BTreeSet::new();
    visit_verbs(node, &mut |verb| verb_capabilities(verb, &mut out));
    if matches!(node, Node::Delayed { .. } | Node::Reflexive { .. }) {
        out.insert(Capability::ResolutionTriggerV1);
    }
    if matches!(node, Node::TapTargets { .. }) {
        out.insert(Capability::OptionalTargetGroupV1);
    }
    if matches!(node, Node::SacrificeForTappedReturn { .. }) {
        out.extend([Capability::SacrificeV1, Capability::GraveyardRecursionV1]);
    }
    if matches!(node, Node::RemoveTargets { .. }) {
        out.insert(Capability::TargetGroupRemovalV1);
    }
    if matches!(node, Node::ChooseObject { .. }) {
        out.insert(Capability::ResolutionObjectChoiceV1);
    }
    if node.uses_continuation() {
        out.insert(Capability::KernelControlV1);
    }
    out
}

/// Calls `visit` for every verb of a node, in written order.
pub fn visit_verbs<'a>(node: &'a Node, visit: &mut impl FnMut(&'a Verb)) {
    match node {
        Node::Do(verb) => {
            visit(verb);
            if let Verb::CreateDelayedTrigger { node, .. } = verb {
                visit_verbs(node, visit);
            }
        }
        Node::Seq(nodes) => nodes.iter().for_each(|node| visit_verbs(node, visit)),
        Node::If {
            then, otherwise, ..
        } => {
            visit_verbs(then, visit);
            if let Some(otherwise) = otherwise {
                visit_verbs(otherwise, visit);
            }
        }
        Node::Optional { node, .. } | Node::UnlessPays { node, .. } | Node::IfPays { node, .. } => {
            visit_verbs(node, visit)
        }
        Node::ForEach { body, .. } | Node::Choose { body, .. } => visit_verbs(body, visit),
        Node::Delayed { .. }
        | Node::Reflexive { .. }
        | Node::ChooseObject { .. }
        | Node::TapTargets { .. }
        | Node::SacrificeForTappedReturn { .. }
        | Node::RemoveTargets { .. } => {}
    }
}

/// The event-player roles an event supplies (CR 603.2).
fn event_player_roles(event: &TriggerCondition) -> &'static [EventPlayerRole] {
    match event.pattern() {
        // CR 601.2: the caster of the triggering spell.
        EventPattern::Cast { .. } => &[EventPlayerRole::Caster],
        EventPattern::Draws { .. } => &[EventPlayerRole::DrawingPlayer],
        // CR 506.2: the player the attacking creature was attacking.
        EventPattern::AttacksUnblocked { .. } | EventPattern::BecomesBlocked { .. } => {
            &[EventPlayerRole::DefendingPlayer]
        }
        // CR 500: the player whose step began.
        EventPattern::StepBegins { .. } => &[EventPlayerRole::ActivePlayer],
        _ => &[],
    }
}

/// True when `event` supplies the event player in `role` (CR 603.2). A
/// damage event's damaged player is bound only by the actions that read it,
/// so it is not the role lowerers of "that player" choose.
fn event_supplies(event: &TriggerCondition, role: EventPlayerRole) -> bool {
    event_player_roles(event).contains(&role)
        || (role == EventPlayerRole::DamagedPlayer
            && matches!(event.pattern(), EventPattern::DealsDamage { .. }))
        // CR 508.5: the player the attacking source attacks. This role is explicit in
        // annihilator. An attack alone does not bind the pronoun "that player".
        || (role == EventPlayerRole::DefendingPlayer
            && matches!(event.pattern(), EventPattern::Attacks { .. }))
}

/// CR 603.2, 608.2h: the events whose number "that much" can read: damage
/// dealt and life gained.
pub fn event_supplies_amount(event: &TriggerCondition) -> bool {
    match event.pattern() {
        EventPattern::DealsDamage { .. }
        | EventPattern::IsDealtDamage { .. }
        | EventPattern::GainsLife { .. } => true,
        _ => false,
    }
}

/// The event-player role an event supplies, for lowerers of "that player".
pub fn event_player_role(event: &TriggerCondition) -> Option<EventPlayerRole> {
    event_player_roles(event).first().copied()
}

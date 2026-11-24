//! [`check`]: the single scope, type and audited-cell decision for a node,
//! and the capabilities it derives.
use super::*;

mod counters;
mod damage;
mod destroy;
mod discard;
mod draw;
mod life;
mod mill;
mod modify;
mod move_zone;
mod sacrifice;
mod tap;
mod tokens;

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
        steps: 0,
        out: DerivedRequirements::default(),
    };
    walk.node(node, false)?;
    if node.uses_continuation() {
        walk.out.capabilities.insert(Capability::KernelControlV1);
        walk.out.cells.insert(Cell::Control);
    }
    Ok(walk.out)
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
        steps: 0,
        out: DerivedRequirements::default(),
    };
    // A scan reports no scope or audit error; a malformed literal can only
    // end it early, and `check` reports that literal.
    let _ = walk.node(node, false);
    walk.out.target_slots
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
    steps: u32,
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
/// A missing cell ends a check and is no finding of a scan.
macro_rules! missing_cell {
    ($walk:expr, $detail:expr) => {
        if $walk.scan {
            Ok(())
        } else {
            unaudited($detail)
        }
    };
}

impl Walk<'_, '_> {
    fn domain(&self, slot: u32) -> Result<TargetDomain, KirError> {
        match self.env.targets.get(slot as usize) {
            Some(target) if target.slot == slot => Ok(target.domain),
            _ => scope(format!("target slot {slot} is not announced")),
        }
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
                    | TargetDomain::OwnGraveyardCard => RefKind::Object,
                    TargetDomain::AnyDamageTarget => RefKind::Entity,
                    TargetDomain::AnyPlayer
                    | TargetDomain::OpponentPlayer
                    | TargetDomain::PlayerOrPlaneswalker => RefKind::Player,
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
                if *step >= self.steps {
                    return scope(format!("step {step} has not been performed"));
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
                return unaudited(format!("event object {role:?}"));
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
            Amount::StatOf { object, .. } => self
                .expect(object, &[RefKind::Object], "measured object")
                .map(drop),
            Amount::Devotion { player, .. }
            | Amount::LifeOf { player }
            | Amount::OtherCastsManaValue { caster: player } => self.player(player),
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
        }
    }

    fn delayed(&mut self, event: &DelayedEvent) -> Result<(), KirError> {
        match event {
            DelayedEvent::NextUpkeep { of } => self.player(of),
            DelayedEvent::NextEndStep | DelayedEvent::EndOfCombat | DelayedEvent::Event { .. } => {
                Ok(())
            }
        }
    }

    /// Scope and type of every operand of a verb. Total over [`Verb`].
    fn operands(&mut self, verb: &Verb) -> Result<(), KirError> {
        match verb {
            Verb::Draw { player, amount }
            | Verb::GainLife { player, amount }
            | Verb::LoseLife { player, amount }
            | Verb::Mill { player, amount }
            | Verb::Discard { player, amount, .. }
            | Verb::Scry { player, amount }
            | Verb::Surveil { player, amount } => {
                self.player(player)?;
                self.amount(amount)
            }
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
            Verb::Move { what, to } => {
                self.set(what)?;
                self.destination(to)
            }
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
            } => {
                self.player(player)?;
                self.amount(count)?;
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
            Verb::Attach { what, to } => {
                self.expect(what, &[RefKind::Object], "attachment")?;
                self.expect(to, &[RefKind::Object], "attachment host")
                    .map(drop)
            }
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
                    Verb::GainLife { player, amount } | Verb::LoseLife { player, amount }
                        if in_control =>
                    {
                        self.control_player(player, false)?;
                        self.nonnegative_control_amount(amount)?;
                    }
                    _ => {
                        let enclosing_leaf = std::mem::replace(&mut self.plain_leaf, in_control);
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
                // CR 119.3, 608.2c: the drain pair is one resolution step
                // with no decision between its two life events.
                if let Some(amount) = drain(node) {
                    self.amount(amount)?;
                    if let [_, Node::Do(Verb::GainLife { player, .. })] = nodes.as_slice() {
                        self.player(player)?;
                    }
                    if !self.scan {
                        if in_control {
                            return unaudited("nested drain reads a step result");
                        }
                        Self::fixed_or_announced_x(amount, 1, "drain amount")?;
                        self.out.capabilities.extend(DRAIN_CAPABILITIES);
                        self.out.cells.insert(Cell::Drain);
                    }
                    return Ok(());
                }
                let steps = std::mem::take(&mut self.steps);
                let result = nodes.iter().try_for_each(|node| {
                    self.node(node, true)?;
                    self.steps += 1;
                    Ok(())
                });
                self.steps = steps;
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
                self.control_player(chooser, true)?;
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
            Node::UnlessPays {
                payer,
                payment,
                node,
            } => {
                self.control_player(payer, true)?;
                match payment {
                    Payment::Generic { amount } | Payment::Life { amount } => {
                        self.nonnegative_control_amount(amount)?
                    }
                }
                self.nested(node, true)
            }
            Node::Delayed { event, node } => {
                self.delayed(event)?;
                self.nested_unaudited(node)?;
                missing_cell!(self, "delayed node")
            }
            Node::Reflexive { first, then } => {
                self.nested_unaudited(first)?;
                self.nested_unaudited(then)?;
                missing_cell!(self, "reflexive node")
            }
        }
    }

    /// Scope-check the child of an unsupported trigger control. Only an
    /// audited parent may rely on a successful child audit.
    fn nested_unaudited(&mut self, node: &Node) -> Result<(), KirError> {
        match self.nested(node, true) {
            Err(KirError::Unaudited(_)) => Ok(()),
            result => result,
        }
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
            // These executors authenticate against top-level instructions
            // after the temporary leaf program has been removed.
            Verb::CreateTokens { .. } | Verb::ModifyUntil { .. } => {
                unaudited("nested leaf needs persistent provenance")
            }
            Verb::Damage { to, .. } if !matches!(to, DamageTo::Ref { .. }) => {
                unaudited("nested set damage leaves top-level cursor facts")
            }
            // CR 121.2: counted draws retain attempt progress and finish
            // with an outcome that must match a top-level draw instruction.
            Verb::Draw { player, amount } if !single_draw(player, amount) => {
                unaudited("nested counted draw needs a top-level draw instruction")
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

    /// The event-player roles whose player the life and draw executors read:
    /// a cast trigger's caster and a step trigger's active player.
    fn recipient_role(role: EventPlayerRole) -> bool {
        matches!(
            role,
            EventPlayerRole::Caster | EventPlayerRole::ActivePlayer
        )
    }

    /// A fixed amount of at least `minimum`, or the announced X (CR 107.3).
    /// Returns true for the announced X.
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

    /// The audited cell a well-scoped verb occupies. A cell is opened only
    /// where the core interpreter executes exactly these operands in this
    /// context and recovery re-derives the same facts; each verb's cells live
    /// in its own module. Total over [`Verb`]: a verb with no module has no
    /// audited cell.
    fn audit(&self, verb: &Verb) -> Result<Cell, KirError> {
        match verb {
            Verb::GainLife { player, amount } | Verb::LoseLife { player, amount } => {
                self.audit_life(player, amount)
            }
            Verb::Draw { player, amount } => self.audit_draw(player, amount),
            Verb::Damage { source, to, amount } => self.audit_damage(source, to, amount),
            Verb::Destroy {
                what,
                regeneration_forbidden,
            } => self.audit_destroy(what, *regeneration_forbidden),
            Verb::Move { what, to } => self.audit_move(what, to),
            Verb::Tap { what } => self.audit_tap(what),
            Verb::Untap { what } => self.audit_untap(what),
            Verb::Mill { player, amount } => self.audit_mill(player, amount),
            Verb::Discard {
                player,
                amount,
                random,
            } => self.audit_discard(player, amount, *random),
            Verb::Sacrifice {
                player,
                count,
                filter,
            } => self.audit_sacrifice(player, count, filter),
            Verb::CreateTokens {
                controller,
                count,
                token,
            } => Self::audit_tokens(controller, count, token),
            Verb::PutCounters {
                on,
                counter,
                amount,
            } => self.audit_counters(on, counter, amount),
            Verb::ModifyUntil {
                what,
                layer,
                change,
                until,
            } => self.audit_modify(what, *layer, change, *until),
            Verb::Scry { .. }
            | Verb::Surveil { .. }
            | Verb::Regenerate { .. }
            | Verb::AddMana { .. }
            | Verb::Search { .. }
            | Verb::Counter { .. }
            | Verb::CopyStackObject { .. }
            | Verb::GainControl { .. }
            | Verb::Attach { .. }
            | Verb::CreateDelayedTrigger { .. }
            | Verb::PlayPermission { .. }
            | Verb::Transform { .. }
            | Verb::SetDesignation { .. }
            | Verb::Shuffle { .. }
            | Verb::Builtin { .. } => unaudited("verb"),
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
                Atom::Blocking | Atom::OwnedBy { .. } | Atom::IsNot { .. } => {
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
        Verb::Damage { to, amount, .. } => damage::capabilities(to, amount, out),
        Verb::Destroy { what, .. } => destroy::capabilities(what, out),
        Verb::Move { what, to } => move_zone::capabilities(what, to, out),
        Verb::Tap { what } => tap::tap_capabilities(what, out),
        Verb::Untap { what } => tap::untap_capabilities(what, out),
        Verb::CreateTokens { token, .. } => tokens::capabilities(token, out),
        Verb::PutCounters { counter, .. } => counters::capabilities(counter, out),
        Verb::ModifyUntil { what, change, .. } => modify::capabilities(what, change, out),
        Verb::Mill { amount, .. } => mill::capabilities(amount, out),
        Verb::Discard { player, random, .. } => discard::capabilities(player, *random, out),
        Verb::Sacrifice { player, .. } => sacrifice::capabilities(player, out),
        Verb::Scry { .. }
        | Verb::Surveil { .. }
        | Verb::Regenerate { .. }
        | Verb::AddMana { .. }
        | Verb::Search { .. }
        | Verb::Counter { .. }
        | Verb::CopyStackObject { .. }
        | Verb::GainControl { .. }
        | Verb::Attach { .. }
        | Verb::CreateDelayedTrigger { .. }
        | Verb::PlayPermission { .. }
        | Verb::Transform { .. }
        | Verb::SetDesignation { .. }
        | Verb::Shuffle { .. }
        | Verb::Builtin { .. } => {}
    }
}

/// CR 121.2: a draw of exactly one card by one player is a single draw.
/// Every other draw is performed as a counted sequence of single draws.
pub fn single_draw(player: &PlayerExpr, amount: &Amount) -> bool {
    matches!(player, PlayerExpr::Is { .. }) && amount.as_fixed() == Some(&Quantity::from(1))
}

/// The capabilities a node derives, independent of where it resolves.
pub fn node_capabilities(node: &Node) -> BTreeSet<Capability> {
    let mut out = BTreeSet::new();
    if drain(node).is_some() {
        out.extend(DRAIN_CAPABILITIES);
    } else {
        visit_verbs(node, &mut |verb| verb_capabilities(verb, &mut out));
        if node.uses_continuation() {
            out.insert(Capability::KernelControlV1);
        }
    }
    out
}

/// The drain pair names its own reviewed surface instead of the group life
/// executor's (CR 119.3).
const DRAIN_CAPABILITIES: [Capability; 3] = [
    Capability::OpponentLifeDrainV1,
    Capability::LifeLossV1,
    Capability::LifeGainV1,
];

/// CR 119.3, 608.2c: "each opponent loses N life. You gain life equal to the
/// life lost this way." Returns N when `node` is exactly that pair: one
/// simultaneous loss, then one gain of the total actually lost.
pub fn drain(node: &Node) -> Option<&Amount> {
    let Node::Seq(steps) = node else {
        return None;
    };
    match steps.as_slice() {
        [
            Node::Do(Verb::LoseLife {
                player: PlayerExpr::EachOpponent,
                amount,
            }),
            Node::Do(Verb::GainLife {
                player,
                amount:
                    Amount::Var {
                        of:
                            Ref::StepResult {
                                step: 0,
                                role: StepRole::Amount,
                            },
                    },
            }),
        ] if player.is_you() => Some(amount),
        _ => None,
    }
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
        Node::Optional { node, .. }
        | Node::UnlessPays { node, .. }
        | Node::Delayed { node, .. } => visit_verbs(node, visit),
        Node::ForEach { body, .. } | Node::Choose { body, .. } => visit_verbs(body, visit),
        Node::Reflexive { first, then } => {
            visit_verbs(first, visit);
            visit_verbs(then, visit);
        }
    }
}

/// The event-player roles an event supplies (CR 603.2).
fn event_player_roles(event: &TriggerCondition) -> &'static [EventPlayerRole] {
    match event {
        // CR 601.2: the caster of the triggering spell.
        TriggerCondition::SpellCast { .. } => &[EventPlayerRole::Caster],
        // CR 500: the player whose step began.
        TriggerCondition::StepBegan { .. } => &[EventPlayerRole::ActivePlayer],
        _ => &[],
    }
}

/// True when `event` supplies the event player in `role` (CR 603.2). A
/// damage event's damaged player is bound only by the actions that read it,
/// so it is not the role lowerers of "that player" choose.
fn event_supplies(event: &TriggerCondition, role: EventPlayerRole) -> bool {
    event_player_roles(event).contains(&role)
        || (role == EventPlayerRole::DamagedPlayer
            && matches!(event, TriggerCondition::DamageDealt { .. }))
}

/// The event-player role an event supplies, for lowerers of "that player".
pub fn event_player_role(event: &TriggerCondition) -> Option<EventPlayerRole> {
    event_player_roles(event).first().copied()
}

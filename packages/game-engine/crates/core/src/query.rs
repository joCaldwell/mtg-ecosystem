//! Shared object predicates for the rules core and for policies that read a
//! permitted view. The core evaluates permanent filters, board-wide sets,
//! sacrifice filters and stack-spell filters here, so a policy asking the same
//! question cannot diverge. `matches_target_domain` states the inhabitants the
//! core's own target legality selects; the core does not yet call it.
//!
//! Pinned CR 109.5, 110.5, 205.2b, 400.7, 506.4, 601.2c, 608.2b and 702.26b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//!
//! A predicate reads facts only through [`ObjectFacts`]. The core implements
//! it privately over derived state; this module implements it over the public
//! [`VisibleObject`] and [`PermittedView`]. A fact the viewer was not shown
//! yields [`Tri::Unknown`], never a guess, and nothing here reaches rules
//! state. Target legality additionally needs hexproof, shroud, protection, a
//! required filter and per-ability restrictions, which stay with the core.
use crate::{PermittedView, PlayerId, PlayerStatus, VisibleObject, Zone};
use mtg_contracts::kir::{Amount, Atom, Cmp, CounterKind, PlayerExpr, Pred, Ref, ZoneExpr};
use mtg_contracts::{
    CardKind, Characteristics, CreaturePowerFilter, KeywordAbility, ObjectSet, PermanentFilter,
    Quantity, SacrificeFilter, TargetDomain,
};
use std::borrow::Cow;

/// Three-valued answer: `Unknown` when a needed fact is hidden from the reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tri {
    Yes,
    No,
    Unknown,
}

impl Tri {
    pub fn is_yes(self) -> bool {
        self == Self::Yes
    }
    /// Kleene conjunction: a definite `No` decides it.
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::No, _) | (_, Self::No) => Self::No,
            (Self::Yes, Self::Yes) => Self::Yes,
            _ => Self::Unknown,
        }
    }
    pub fn negate(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
            Self::Unknown => Self::Unknown,
        }
    }
}

impl From<bool> for Tri {
    fn from(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }
}

/// The facts one object exposes to the predicates below. `None` and
/// [`Tri::Unknown`] mean the reader does not know the fact.
pub trait ObjectFacts {
    fn zone(&self) -> Zone;
    fn owner(&self) -> Option<PlayerId>;
    fn controller(&self) -> PlayerId;
    fn phased_out(&self) -> bool;
    fn tapped(&self) -> bool;
    fn is_token(&self) -> bool;
    /// Card types of the object's copiable values (CR 707.2).
    fn copiable(&self) -> Option<Cow<'_, Characteristics>>;
    /// Current card types, before color and power effects apply.
    fn structural(&self) -> Option<Cow<'_, Characteristics>>;
    /// Current card types and colors.
    fn current(&self) -> Option<Cow<'_, Characteristics>>;
    /// Fully derived characteristics, including power and toughness.
    fn derived(&self) -> Option<Cow<'_, Characteristics>>;
    fn has_keyword(&self, keyword: KeywordAbility) -> Tri;
    fn controller_is_live(&self) -> Tri;
    fn attacking(&self) -> Tri;
    /// CR 506.4: a blocker stays a blocking creature after its attacker leaves.
    fn blocking(&self) -> Tri;
    /// CR 122.1: known battlefield counter balances. None means hidden facts.
    fn counters(&self, _kind: Option<&CounterKind>) -> Option<Quantity> {
        None
    }
}

pub(crate) fn counter_balance<'a>(
    kind: Option<&CounterKind>,
    plus_one: &'a Quantity,
    loyalty: &'a Quantity,
    other: impl IntoIterator<Item = (&'a CounterKind, &'a Quantity)>,
) -> Quantity {
    match kind {
        Some(CounterKind::PlusOnePlusOne) => plus_one.clone(),
        Some(CounterKind::Loyalty) => loyalty.clone(),
        Some(kind) => other
            .into_iter()
            .find(|(found, _)| *found == kind)
            .map_or_else(Quantity::zero, |(_, count)| count.clone()),
        None => {
            let mut total = plus_one.clone();
            total.add(loyalty);
            for (_, count) in other {
                total.add(count);
            }
            total
        }
    }
}

/// CR 110.1/702.26b: on the battlefield and phased in.
fn present(object: &impl ObjectFacts) -> bool {
    object.zone() == Zone::Battlefield && !object.phased_out()
}

fn known(
    characteristics: Option<Cow<'_, Characteristics>>,
    test: impl FnOnce(&Characteristics) -> bool,
) -> Tri {
    characteristics.map_or(Tri::Unknown, |c| test(&c).into())
}

/// CR 205.2b, 613.1d: a creature can gain or retain other card types.
fn creature(object: &impl ObjectFacts) -> Tri {
    known(object.structural(), |c| c.has_type(CardKind::Creature))
}

/// CR 601.2c/608.2b: whether a battlefield permanent satisfies a finite
/// filter, relative to `you`. An empty type list admits any permanent type.
pub fn matches_permanent_filter(
    object: &impl ObjectFacts,
    you: PlayerId,
    filter: &PermanentFilter,
) -> Tri {
    matches_permanent_filter_with_source(object, you, filter, None)
}

/// The targeting invocation's source facts a permanent filter can read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TargetSource<'a> {
    /// The candidate is the targeting object itself (CR 201.5).
    pub is_source: bool,
    /// CR 702.134a, 608.2b: the source's derived power, current or last
    /// known; `None` when it has no power.
    pub power: Option<&'a Quantity>,
}

/// The caller can bind the source facts for a targeting invocation; without
/// them, source-relative terms are unknown.
pub(crate) fn matches_permanent_filter_with_source(
    object: &impl ObjectFacts,
    you: PlayerId,
    filter: &PermanentFilter,
    source: Option<TargetSource<'_>>,
) -> Tri {
    let is_source = source.map(|source| source.is_source);
    if !present(object)
        || (filter.opponent && object.controller() == you)
        || (filter.tapped && !object.tapped())
    {
        return Tri::No;
    }
    let mut typed = known(object.current(), |c| {
        !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
            && (filter.types.is_empty() || filter.types.iter().any(|kind| c.has_type(*kind)))
            && !filter.non_types.iter().any(|kind| c.has_type(*kind))
            && !c
                .colors
                .iter()
                .any(|color| filter.non_colors.contains(color))
    });
    if let Some(predicate) = &filter.predicate {
        typed = predicate.0.iter().fold(typed, |result, atom| {
            let matched = match (atom, is_source) {
                (
                    Atom::IsNot {
                        object: Ref::Source,
                    },
                    Some(is_source),
                ) => (!is_source).into(),
                _ => matches_atom(object, you, atom),
            };
            result.and(matched)
        });
    }
    if typed == Tri::No {
        return Tri::No;
    }
    let Some(bound) = &filter.power else {
        return typed;
    };
    let source_power = match (bound, source) {
        (CreaturePowerFilter::LessThanSource {}, None) => return typed.and(Tri::Unknown),
        (_, source) => source.and_then(|source| source.power),
    };
    typed.and(known(object.derived(), |c| {
        c.has_type(CardKind::Creature)
            && c.power.as_ref().is_some_and(|power| match bound {
                CreaturePowerFilter::AtLeast { value } => power >= value,
                CreaturePowerFilter::AtMost { value } => power <= value,
                CreaturePowerFilter::LessThanSource {} => {
                    // CR 107.2, 208.3: a known source that stopped being
                    // a creature has no power. Use zero for that number.
                    source_power.map_or_else(|| power.is_negative(), |source| power < source)
                }
            })
    }))
}

/// CR 608.2c/h: whether an object belongs to a board-wide set, relative to
/// `you`. Land exclusion uses every card type, so a creature land is a land.
pub fn matches_object_set(object: &impl ObjectFacts, you: PlayerId, set: &ObjectSet) -> Tri {
    if *set == ObjectSet::GraveyardCards {
        return (matches!(object.zone(), Zone::Graveyard { .. }) && !object.is_token()).into();
    }
    if !present(object) {
        return Tri::No;
    }
    let Some(current) = object.structural() else {
        return Tri::Unknown;
    };
    let controlled = object.controller() == you;
    match set {
        ObjectSet::GraveyardCards => Tri::No,
        ObjectSet::Artifacts => current.has_type(CardKind::Artifact).into(),
        ObjectSet::Enchantments => current.has_type(CardKind::Enchantment).into(),
        ObjectSet::Creatures => current.has_type(CardKind::Creature).into(),
        ObjectSet::NonlandPermanents => (!current.has_type(CardKind::Land)).into(),
        // CR 109.5/806.1: current control, and a player who has left has no permanents.
        ObjectSet::OpponentNonlandPermanents => Tri::from(!controlled)
            .and(object.controller_is_live())
            .and((!current.has_type(CardKind::Land)).into()),
        ObjectSet::AttackingCreatures => creature(object).and(object.attacking()),
        ObjectSet::ControllerCreatures => creature(object).and(controlled.into()),
        ObjectSet::ControllerPermanents => controlled.into(),
        ObjectSet::OpponentCreatures => creature(object).and((!controlled).into()),
        ObjectSet::FlyingCreatures => {
            creature(object).and(object.has_keyword(KeywordAbility::Flying))
        }
        ObjectSet::NonflyingCreatures => {
            creature(object).and(object.has_keyword(KeywordAbility::Flying).negate())
        }
    }
}

/// The players a kernel predicate may name without reading an invocation.
fn controlled_by(object: &impl ObjectFacts, you: PlayerId, player: &PlayerExpr) -> Tri {
    match player {
        PlayerExpr::Is {
            who: Ref::Controller { .. },
        } => (object.controller() == you).into(),
        // CR 102.2-102.3, 800.4a: an opponent who has left controls nothing.
        PlayerExpr::EachOpponent => {
            Tri::from(object.controller() != you).and(object.controller_is_live())
        }
        PlayerExpr::EachPlayer => object.controller_is_live(),
        _ => Tri::Unknown,
    }
}

fn compare(actual: Option<&Quantity>, cmp: Cmp, value: &Amount) -> bool {
    let (Some(actual), Amount::Fixed { value }) = (actual, value) else {
        return false;
    };
    compare_quantities(actual, cmp, value)
}

fn compare_quantities(actual: &Quantity, cmp: Cmp, value: &Quantity) -> bool {
    match cmp {
        Cmp::AtLeast => actual >= value,
        Cmp::AtMost => actual <= value,
        Cmp::Exactly => actual == value,
    }
}

/// One conjunct of a kernel predicate. Card types and supertypes are the
/// object's current structural ones (CR 205, 613.1d), colors its current
/// colors (CR 613.1e), power, toughness and keywords fully derived
/// (CR 613.1f, 613.4). An operand this reader cannot evaluate is `Unknown`.
pub fn matches_atom(object: &impl ObjectFacts, you: PlayerId, atom: &Atom) -> Tri {
    match atom {
        Atom::Type { kind } => known(object.structural(), |c| c.has_type(*kind)),
        Atom::NotType { kind } => known(object.structural(), |c| !c.has_type(*kind)),
        Atom::AnyType { kinds } => known(object.structural(), |c| {
            kinds.iter().any(|kind| c.has_type(*kind))
        }),
        Atom::Subtype { subtype } => known(object.derived(), |c| c.has_subtype(subtype)),
        Atom::NotSubtype { subtype } => known(object.derived(), |c| !c.has_subtype(subtype)),
        Atom::Legendary => known(object.structural(), |c| c.legendary),
        Atom::NotLegendary => known(object.structural(), |c| !c.legendary),
        Atom::Basic => known(object.structural(), |c| c.basic),
        Atom::NotBasic => known(object.structural(), |c| !c.basic),
        Atom::Color { color } => known(object.current(), |c| c.colors.contains(color)),
        Atom::NotColor { color } => known(object.current(), |c| !c.colors.contains(color)),
        Atom::ControlledBy { player } => controlled_by(object, you, player),
        Atom::OwnedBy { player } => match (object.owner(), player) {
            (
                Some(owner),
                PlayerExpr::Is {
                    who: Ref::Controller { .. },
                },
            ) => (owner == you).into(),
            (Some(owner), PlayerExpr::EachOpponent) => (owner != you).into(),
            _ => Tri::Unknown,
        },
        Atom::Tapped => object.tapped().into(),
        Atom::Untapped => (!object.tapped()).into(),
        Atom::Attacking => object.attacking(),
        Atom::Blocking => object.blocking(),
        Atom::Token => object.is_token().into(),
        Atom::NotToken => (!object.is_token()).into(),
        Atom::Keyword { keyword } => object.has_keyword(*keyword),
        Atom::NotKeyword { keyword } => object.has_keyword(*keyword).negate(),
        // CR 208.1: only a creature has power and toughness.
        Atom::Power { cmp, value } => known(object.derived(), |c| {
            c.has_type(CardKind::Creature) && compare(c.power.as_ref(), *cmp, value)
        }),
        Atom::Toughness { cmp, value } => known(object.derived(), |c| {
            c.has_type(CardKind::Creature) && compare(c.toughness.as_ref(), *cmp, value)
        }),
        Atom::ManaValue { cmp, value } => known(object.copiable(), |c| {
            compare(Some(&c.printed_mana_value()), *cmp, value)
        }),
        Atom::Counters {
            counter,
            cmp,
            value,
        } => object
            .counters(counter.as_ref())
            .map_or(Tri::Unknown, |actual| {
                compare_quantities(&actual, *cmp, value).into()
            }),
        // Identity against an invocation's reference is the caller's fact.
        Atom::Is { .. }
        | Atom::IsNot { .. }
        | Atom::SourceChosenColor {}
        | Atom::SourceChosenCreatureType {} => Tri::Unknown,
        Atom::AnyOf { alternatives } => alternatives.iter().fold(Tri::No, |found, pred| {
            match (found, matches_pred(object, you, pred)) {
                (Tri::Yes, _) | (_, Tri::Yes) => Tri::Yes,
                (Tri::No, Tri::No) => Tri::No,
                _ => Tri::Unknown,
            }
        }),
    }
}

/// CR 608.2h: whether an object satisfies every conjunct of a kernel
/// predicate, relative to `you`. Zone membership is the caller's question.
pub fn matches_pred(object: &impl ObjectFacts, you: PlayerId, pred: &Pred) -> Tri {
    pred.0.iter().fold(Tri::Yes, |all, atom| {
        all.and(matches_atom(object, you, atom))
    })
}

/// CR 110.1, 702.26b, 608.2h: whether a phased-in battlefield permanent
/// satisfies a kernel predicate.
pub fn matches_battlefield_pred(object: &impl ObjectFacts, you: PlayerId, pred: &Pred) -> Tri {
    if !present(object) {
        return Tri::No;
    }
    if object.copiable().is_none() {
        return Tri::Unknown;
    }
    matches_pred(object, you, pred)
}

/// CR 608.2h: whether an object belongs to a kernel set of one public zone.
/// CR 404.1, 111.8: a token is never a graveyard card.
pub fn matches_zone_pred(
    object: &impl ObjectFacts,
    you: PlayerId,
    zone: &ZoneExpr,
    pred: &Pred,
) -> Tri {
    match zone {
        ZoneExpr::Battlefield => matches_battlefield_pred(object, you, pred),
        ZoneExpr::Graveyard { of } if of.is_you() => {
            if object.zone() == (Zone::Graveyard { owner: you }) && !object.is_token() {
                matches_pred(object, you, pred)
            } else {
                Tri::No
            }
        }
        ZoneExpr::Graveyards => {
            if matches!(object.zone(), Zone::Graveyard { .. }) && !object.is_token() {
                matches_pred(object, you, pred)
            } else {
                Tri::No
            }
        }
        _ => Tri::Unknown,
    }
}

/// CR 701.21a: whether `you` may sacrifice a permanent under a finite filter.
pub fn matches_sacrifice_filter(
    object: &impl ObjectFacts,
    you: PlayerId,
    filter: &SacrificeFilter,
) -> Tri {
    if !present(object) || object.controller() != you {
        return Tri::No;
    }
    known(object.derived(), |c| {
        (filter.types.is_empty() || filter.types.iter().any(|kind| c.has_type(*kind)))
            && filter.subtypes.iter().all(|s| c.has_subtype(s))
    })
}

/// Whether an object inhabits a target domain, relative to `you`. Player and
/// stack domains have no object inhabitants. A `PlayerGraveyardCards` card
/// must be in the graveyard of the player bound to another slot; that binding
/// is no object fact, so a graveyard card answers `Unknown` to every reader.
pub fn matches_target_domain(
    object: &impl ObjectFacts,
    you: PlayerId,
    domain: TargetDomain,
) -> Tri {
    let battlefield_creature = || {
        if present(object) {
            creature(object)
        } else {
            Tri::No
        }
    };
    let own_graveyard_card = || {
        if object.zone() != (Zone::Graveyard { owner: you }) || object.is_token() {
            return Tri::No;
        }
        object
            .owner()
            .map_or(Tri::Unknown, |owner| (owner == you).into())
    };
    match domain {
        TargetDomain::GraveyardCard { opponents_only } => match object.zone() {
            Zone::Graveyard { owner }
                if !object.is_token() && (!opponents_only || owner != you) =>
            {
                object
                    .owner()
                    .map_or(Tri::Unknown, |actual| (owner == actual).into())
            }
            _ => Tri::No,
        },
        TargetDomain::OwnGraveyardCard => own_graveyard_card(),
        TargetDomain::OwnGraveyardCreatureCard => {
            own_graveyard_card().and(known(object.copiable(), |c| c.kind == CardKind::Creature))
        }
        TargetDomain::BattlefieldCreature => battlefield_creature(),
        TargetDomain::AnyDamageTarget | TargetDomain::CreatureOrPlaneswalker => {
            if present(object) {
                known(object.structural(), |c| {
                    c.has_type(CardKind::Creature) || c.has_type(CardKind::Planeswalker)
                })
            } else {
                Tri::No
            }
        }
        TargetDomain::PlayerOrPlaneswalker => {
            if present(object) {
                known(object.structural(), |c| c.has_type(CardKind::Planeswalker))
            } else {
                Tri::No
            }
        }
        TargetDomain::ControlledBattlefieldCreature => {
            battlefield_creature().and((object.controller() == you).into())
        }
        TargetDomain::BlockingBattlefieldCreature => battlefield_creature().and(object.blocking()),
        TargetDomain::BattlefieldPermanent => present(object).into(),
        TargetDomain::ControlledBattlefieldPermanent => {
            (present(object) && object.controller() == you).into()
        }
        TargetDomain::PlayerGraveyardCards {
            owner_slot: _,
            maximum: _,
        } => {
            if matches!(object.zone(), Zone::Graveyard { .. }) && !object.is_token() {
                Tri::Unknown
            } else {
                Tri::No
            }
        }
        TargetDomain::OpponentPlayer
        | TargetDomain::StackSpell
        | TargetDomain::ControlledCreatureSpell
        | TargetDomain::StackActivatedOrTriggeredAbility
        | TargetDomain::AnyPlayer => Tri::No,
    }
}

/// CR 202.3e, 601.2c: use the announced spell's types and stack mana value.
pub fn spell_matches_filter(
    spell: &Characteristics,
    mana_value: &Quantity,
    filter: &PermanentFilter,
) -> bool {
    (filter.types.is_empty() || filter.types.iter().any(|kind| spell.has_type(*kind)))
        && !filter.non_types.iter().any(|kind| spell.has_type(*kind))
        && filter.predicate.as_ref().is_none_or(|pred| {
            pred.0.iter().all(|atom| match atom {
                Atom::ManaValue { cmp, value } => compare(Some(mana_value), *cmp, value),
                _ => false,
            })
        })
}

/// A visible object alone. Facts that need the surrounding view are unknown;
/// use [`in_view`] to supply them.
impl ObjectFacts for VisibleObject {
    fn counters(&self, kind: Option<&CounterKind>) -> Option<Quantity> {
        (self.zone == Zone::Battlefield).then(|| {
            counter_balance(
                kind,
                &self.plus_one_counters,
                &self.loyalty_counters,
                self.counters
                    .iter()
                    .map(|stack| (&stack.kind, &stack.count)),
            )
        })
    }

    fn zone(&self) -> Zone {
        self.zone
    }
    fn owner(&self) -> Option<PlayerId> {
        Some(self.owner)
    }
    fn controller(&self) -> PlayerId {
        self.controller
    }
    fn phased_out(&self) -> bool {
        self.phased_out
    }
    fn tapped(&self) -> bool {
        self.tapped
    }
    fn is_token(&self) -> bool {
        self.token.is_some()
    }
    // A permitted view publishes a derived record. The copiable facts queried
    // from it are the mana value and the types of cards off the battlefield.
    fn copiable(&self) -> Option<Cow<'_, Characteristics>> {
        self.characteristics.as_ref().map(Cow::Borrowed)
    }
    fn structural(&self) -> Option<Cow<'_, Characteristics>> {
        self.characteristics.as_ref().map(Cow::Borrowed)
    }
    fn current(&self) -> Option<Cow<'_, Characteristics>> {
        self.characteristics.as_ref().map(Cow::Borrowed)
    }
    fn derived(&self) -> Option<Cow<'_, Characteristics>> {
        self.characteristics.as_ref().map(Cow::Borrowed)
    }
    // Publication lists all keywords on phased-in creatures and the three
    // protection/targetability keywords on other visible permanents.
    fn has_keyword(&self, keyword: KeywordAbility) -> Tri {
        if self.keywords.contains(&keyword) {
            return Tri::Yes;
        }
        let published = present(self)
            && self.characteristics.as_ref().is_some_and(|c| {
                c.has_type(CardKind::Creature)
                    || matches!(
                        keyword,
                        KeywordAbility::Hexproof
                            | KeywordAbility::Indestructible
                            | KeywordAbility::Shroud
                    )
            });
        if published { Tri::No } else { Tri::Unknown }
    }
    fn controller_is_live(&self) -> Tri {
        Tri::Unknown
    }
    fn attacking(&self) -> Tri {
        Tri::Unknown
    }
    fn blocking(&self) -> Tri {
        Tri::Unknown
    }
}

/// A visible object together with the public facts of its permitted view.
pub struct InView<'a> {
    view: &'a PermittedView,
    object: &'a VisibleObject,
    tapped: bool,
}

pub fn in_view<'a>(view: &'a PermittedView, object: &'a VisibleObject) -> InView<'a> {
    InView {
        view,
        object,
        tapped: object.tapped,
    }
}

impl InView<'_> {
    /// Evaluate a predicate after a simulated tap in a payment plan.
    pub fn after_tap(mut self) -> Self {
        self.tapped = true;
        self
    }
}

impl ObjectFacts for InView<'_> {
    fn counters(&self, kind: Option<&CounterKind>) -> Option<Quantity> {
        self.object.counters(kind)
    }

    fn zone(&self) -> Zone {
        self.object.zone()
    }
    fn owner(&self) -> Option<PlayerId> {
        self.object.owner()
    }
    fn controller(&self) -> PlayerId {
        self.object.controller
    }
    fn phased_out(&self) -> bool {
        self.object.phased_out
    }
    fn tapped(&self) -> bool {
        self.tapped
    }
    fn is_token(&self) -> bool {
        self.object.is_token()
    }
    fn copiable(&self) -> Option<Cow<'_, Characteristics>> {
        self.object.copiable()
    }
    fn structural(&self) -> Option<Cow<'_, Characteristics>> {
        self.object.structural()
    }
    fn current(&self) -> Option<Cow<'_, Characteristics>> {
        self.object.current()
    }
    fn derived(&self) -> Option<Cow<'_, Characteristics>> {
        self.object.derived()
    }
    fn has_keyword(&self, keyword: KeywordAbility) -> Tri {
        self.object.has_keyword(keyword)
    }
    fn controller_is_live(&self) -> Tri {
        self.view
            .players
            .iter()
            .find(|player| player.player == self.object.controller)
            .map_or(Tri::Unknown, |player| {
                (player.status == PlayerStatus::Live).into()
            })
    }
    fn attacking(&self) -> Tri {
        self.view
            .combat
            .iter()
            .any(|attack| attack.attacker == self.object.handle)
            .into()
    }
    // Attacker links prove their blockers. A blocker whose attacker has left
    // combat is still blocking, and the view does not show that.
    fn blocking(&self) -> Tri {
        if self
            .view
            .combat
            .iter()
            .any(|attack| attack.blockers.contains(&self.object.handle))
        {
            Tri::Yes
        } else {
            Tri::Unknown
        }
    }
}

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
use mtg_contracts::{
    CardKind, Characteristics, CreaturePowerFilter, KeywordAbility, ObjectSet, PermanentFilter,
    SacrificeFilter, TargetDomain,
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

/// The canonical execution type lists creature first (CR 205.2b).
fn creature(object: &impl ObjectFacts) -> Tri {
    known(object.structural(), |c| c.kind == CardKind::Creature)
}

/// CR 601.2c/608.2b: whether a battlefield permanent satisfies a finite
/// filter, relative to `you`. An empty type list admits any permanent type.
pub fn matches_permanent_filter(
    object: &impl ObjectFacts,
    you: PlayerId,
    filter: &PermanentFilter,
) -> Tri {
    if !present(object)
        || (filter.opponent && object.controller() == you)
        || (filter.tapped && !object.tapped())
    {
        return Tri::No;
    }
    let typed = known(object.current(), |c| {
        !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
            && (filter.types.is_empty() || filter.types.iter().any(|kind| c.has_type(*kind)))
            && !filter.non_types.iter().any(|kind| c.has_type(*kind))
            && !c
                .colors
                .iter()
                .any(|color| filter.non_colors.contains(color))
    });
    if typed == Tri::No {
        return Tri::No;
    }
    let Some(bound) = &filter.power else {
        return typed;
    };
    typed.and(known(object.derived(), |c| {
        c.has_type(CardKind::Creature)
            && c.power.as_ref().is_some_and(|power| match bound {
                CreaturePowerFilter::AtLeast { value } => power >= value,
                CreaturePowerFilter::AtMost { value } => power <= value,
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
    let Some(printed) = object.copiable() else {
        return Tri::Unknown;
    };
    let controlled = object.controller() == you;
    match set {
        ObjectSet::GraveyardCards => Tri::No,
        ObjectSet::Artifacts => printed.has_type(CardKind::Artifact).into(),
        ObjectSet::Enchantments => printed.has_type(CardKind::Enchantment).into(),
        ObjectSet::Creatures => creature(object),
        ObjectSet::NonlandPermanents => (!printed.has_type(CardKind::Land)).into(),
        // CR 109.5/806.1: current control, and a player who has left has no permanents.
        ObjectSet::OpponentNonlandPermanents => Tri::from(!controlled)
            .and(object.controller_is_live())
            .and((!printed.has_type(CardKind::Land)).into()),
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
            && filter.subtypes.iter().all(|s| c.subtypes.contains(s))
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
            known(object.derived(), |c| c.kind == CardKind::Creature)
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
        TargetDomain::OwnGraveyardCard => own_graveyard_card(),
        TargetDomain::OwnGraveyardCreatureCard => {
            own_graveyard_card().and(known(object.copiable(), |c| c.kind == CardKind::Creature))
        }
        TargetDomain::CreatureOrPlaneswalker => {
            if present(object) {
                known(object.derived(), |c| c.has_type(CardKind::Creature))
            } else {
                Tri::No
            }
        }
        TargetDomain::BattlefieldCreature | TargetDomain::AnyDamageTarget => battlefield_creature(),
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
        | TargetDomain::PlayerOrPlaneswalker
        | TargetDomain::AnyPlayer => Tri::No,
    }
}

/// CR 601.2c: a stack-spell filter names alternative card types only.
pub fn spell_matches_filter(spell: &Characteristics, filter: &PermanentFilter) -> bool {
    filter.types.iter().any(|kind| spell.has_type(*kind))
}

/// A visible object alone. Facts that need the surrounding view are unknown;
/// use [`in_view`] to supply them.
impl ObjectFacts for VisibleObject {
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
    // A permitted view publishes one derived record. Its card types equal the
    // copiable ones except that a bestowed Aura is no creature (CR 702.103b);
    // `BestowCastPlan` admits only an enchantment creature, so the artifact,
    // enchantment and land tests that read copiable types answer alike.
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
    // A view lists the keywords of a phased-in battlefield creature, and of
    // any permanent with indestructible. An absent keyword is known absent
    // only there; elsewhere the list is simply not published.
    fn has_keyword(&self, keyword: KeywordAbility) -> Tri {
        if self.keywords.contains(&keyword) {
            return Tri::Yes;
        }
        let published = self.zone == Zone::Battlefield
            && (self.keywords.contains(&KeywordAbility::Indestructible)
                || (!self.phased_out
                    && self
                        .characteristics
                        .as_ref()
                        .is_some_and(|c| c.kind == CardKind::Creature)));
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
}

pub fn in_view<'a>(view: &'a PermittedView, object: &'a VisibleObject) -> InView<'a> {
    InView { view, object }
}

impl ObjectFacts for InView<'_> {
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
        self.object.tapped
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

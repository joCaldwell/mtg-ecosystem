//! Derived capability tiers.
//!
//! Every audited kernel cell (a `(verb, operand kinds, context)` row of
//! [`check`](super::check)) and every other reviewed runtime surface carries
//! the tiers it has earned. A definition earns a tier only when every cell and
//! surface it uses has earned it: its tiers are the meet over its cells. No
//! card name, Oracle ID or profile appears here.
//!
//! | Tier | Earned when | Admits to |
//! | --- | --- | --- |
//! | [`ExecutionTier::Executes`] | the cell has an interpreter arm and a rules test | headless pods; a runtime fault is `RulesFault` |
//! | [`ExecutionTier::HeadlessQualified`] | it ran in the pod parity corpus with state validation and snapshot round trips | analytics and paired runs |
//! | [`ExecutionTier::Search`] | its hidden-information behaviour is reviewed for the sampler | `mtg_sim::search` |
//! | [`ExecutionTier::Hosting`] | every decision it raises has source labels and a client control | `packages/game-server` |
//!
//! `Executes` is the floor: a cell exists only where it executes, and an
//! admitted definition consists of such cells. The three reviews above it are
//! independent of one another (a hosting review says nothing about hidden
//! information), so a cell records each one it has passed.
use super::*;

/// One admission tier (see the module table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTier {
    Executes,
    HeadlessQualified,
    Search,
    Hosting,
}

/// The tiers a cell, surface or definition has earned. `Executes` is
/// implied by existence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tiers {
    headless_qualified: bool,
    search: bool,
    hosting: bool,
}

impl Tiers {
    /// Executes only: no review beyond the interpreter arm and rules test.
    pub const EXECUTES: Self = Self {
        headless_qualified: false,
        search: false,
        hosting: false,
    };
    /// Every review passed.
    pub const ALL: Self = Self {
        headless_qualified: true,
        search: true,
        hosting: true,
    };
    /// Reviewed for search and hosting, not yet run in the parity corpus.
    pub const SEARCH_HOSTING: Self = Self {
        headless_qualified: false,
        search: true,
        hosting: true,
    };
    /// Reviewed for hosting only.
    pub const HOSTING: Self = Self {
        headless_qualified: false,
        search: false,
        hosting: true,
    };

    pub fn admits(self, tier: ExecutionTier) -> bool {
        match tier {
            ExecutionTier::Executes => true,
            ExecutionTier::HeadlessQualified => self.headless_qualified,
            ExecutionTier::Search => self.search,
            ExecutionTier::Hosting => self.hosting,
        }
    }

    /// The tiers both sides have earned.
    pub fn meet(self, other: Self) -> Self {
        Self {
            headless_qualified: self.headless_qualified && other.headless_qualified,
            search: self.search && other.search,
            hosting: self.hosting && other.hosting,
        }
    }
}

/// An audited kernel cell: the `(verb, operand kinds, context)` row
/// [`check`](super::check) admitted a verb under. A row a wave opens is a new
/// variant here and in [`cell_tiers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cell {
    // CR 119.3: life.
    LifeController,
    LifeEventPlayer,
    LifeEachOpponent,
    LifeTargetPlayer,
    LifeTargetPlayerX,
    /// CR 119.3, 608.2c: the each-opponent drain pair.
    Drain,
    // CR 121: draw.
    DrawController,
    DrawControllerCounted,
    DrawAnnouncedX,
    DrawEachPlayer,
    DrawEventPlayer,
    DrawTargetPlayer,
    // CR 120: damage.
    DamageTarget,
    DamageTargetX,
    DamageController,
    DamageSet,
    // CR 701.8: destroy.
    DestroyTargetPermanent,
    DestroyTargetCreature,
    /// CR 608.2d: "you may destroy target permanent".
    OptionalTargetDestroy,
    DestroyAllCreatures,
    DestroySet,
    // CR 400.3, 400.7: zone moves.
    MoveTargetExile,
    MovePermanentOwnerHand,
    MoveTargetOwnerHand,
    MoveTargetOwnerLibraryTop,
    ExileTargetGraveyard,
    ExileGraveyards,
    MoveSet,
    // CR 701.26: tap and untap.
    TapTarget,
    UntapTarget,
    UntapControllerLands,
    // CR 701.17: mill.
    MillController,
    MillTargetPlayer,
    MillTargetPlayerX,
    // CR 701.9: discard.
    DiscardController,
    DiscardDamagedPlayer,
    DiscardTarget,
    DiscardTargetRandom,
    DiscardGroup,
    // CR 701.21: sacrifice.
    SacrificeDamagedPlayer,
    SacrificeEachPlayer,
    SacrificeEachOpponent,
    SacrificeTargetOpponent,
    // CR 111: tokens.
    PredefinedTokens,
    CreatureTokens,
    MultiTypeCreatureTokens,
    // CR 122: counters.
    PlusOneCountersSource,
    PlusOneCountersTarget,
    // CR 611.2c, 613: until end of turn.
    PumpSource,
    PumpTarget,
    PumpSet,
    SourceKeywordsUntilEndOfTurn,
    /// CR 608.2c-d: a control node (sequence with a decision or result,
    /// condition, choice, repetition, payment) run by the continuation.
    Control,
}

/// The tiers each kernel cell has earned. A cell's tiers are those of the
/// reviewed surfaces that name it (its verb's capabilities and the program
/// surface `Definition::requirements` names it by); a cell named only by its
/// program surface is fenced by that surface. A newly opened cell starts at
/// [`Tiers::EXECUTES`].
pub fn cell_tiers(cell: Cell) -> Tiers {
    use Cell as C;
    match cell {
        C::LifeController
        | C::LifeTargetPlayer
        | C::DrawController
        | C::SacrificeTargetOpponent
        | C::PumpTarget
        | C::PumpSource => Tiers::ALL,
        C::DamageTarget
        | C::DamageController
        | C::MoveTargetOwnerHand
        | C::MoveTargetOwnerLibraryTop
        | C::DiscardController => Tiers::SEARCH_HOSTING,
        C::DrawTargetPlayer
        | C::DestroyTargetCreature
        | C::DestroyAllCreatures
        | C::CreatureTokens
        | C::PlusOneCountersSource
        | C::PlusOneCountersTarget => Tiers::HOSTING,
        C::LifeEventPlayer
        | C::LifeEachOpponent
        | C::LifeTargetPlayerX
        | C::Drain
        | C::DrawControllerCounted
        | C::DrawAnnouncedX
        | C::DrawEachPlayer
        | C::DrawEventPlayer
        | C::DamageTargetX
        | C::DamageSet
        | C::DestroyTargetPermanent
        | C::OptionalTargetDestroy
        | C::DestroySet
        | C::MoveTargetExile
        | C::MovePermanentOwnerHand
        | C::ExileTargetGraveyard
        | C::ExileGraveyards
        | C::MoveSet
        | C::TapTarget
        | C::UntapTarget
        | C::UntapControllerLands
        | C::MillController
        | C::MillTargetPlayer
        | C::MillTargetPlayerX
        | C::DiscardDamagedPlayer
        | C::DiscardTarget
        | C::DiscardTargetRandom
        | C::DiscardGroup
        | C::SacrificeDamagedPlayer
        | C::SacrificeEachPlayer
        | C::SacrificeEachOpponent
        | C::PredefinedTokens
        | C::MultiTypeCreatureTokens
        | C::PumpSet
        | C::SourceKeywordsUntilEndOfTurn
        | C::Control => Tiers::EXECUTES,
    }
}

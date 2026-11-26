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
use crate::{ActivationCost, Capability, Definition, Instruction, SpellProgram, TriggerCondition};

mod executes;
pub(super) use executes::{mana_production_supported, validate_resolution_targets};

/// CR 113.3: the `Executes` tier of a definition: its card shell and every
/// non-kernel surface has an executor ([`executes`]) and every kernel node
/// occupies an audited cell ([`check`](super::check)).
pub(crate) fn executes(d: &Definition) -> Result<(), String> {
    executes::validate(d)
}

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cell {
    // CR 119.3: life.
    LifeController,
    LifeControllerX,
    LifeEventPlayer,
    LifeEachOpponent,
    LifeTargetPlayer,
    LifeTargetPlayerX,
    LifeTargetController,
    LifeTargetControllerPower,
    LifeGraveyardCreatureManaValue,
    LifeCounted,
    /// CR 119.3, 608.2c: the each-opponent drain pair.
    Drain,
    // CR 121: draw.
    DrawController,
    DrawControllerCounted,
    /// CR 121.2, 608.2h: "draw a card for each [filter]", counted once.
    DrawControllerPerObject,
    DrawAnnouncedX,
    DrawEachPlayer,
    DrawEventPlayer,
    DrawTargetPlayer,
    // CR 120: damage.
    DamageTarget,
    DamageTargetX,
    DamageEventObject,
    DamageController,
    DamageSet,
    DamageSelectedPlayers,
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
    MoveTargetOwnerLibraryPosition,
    ShuffleResolvingSpell,
    ShuffleTargetOwnerLibrary,
    DrawShuffledTargetOwner,
    MoveSourceOwnerZone,
    ChooseObjectOwnerHand,
    MoveOwnGraveyardTargetToHand,
    ExileGraveyardTarget,
    GraveyardTargetWasCreature,
    MoveGraveyardTargetToOwnerHand,
    MoveGraveyardTargetToOwnerLibraryTop,
    MoveOwnGraveyardCreatureToBattlefield,
    MoveGraveyardCreatureToBattlefield,
    MoveOwnGraveyardCardToBattlefield,
    ExileTargetGraveyard,
    ExileGraveyards,
    MoveSet,
    // CR 301.5 and 303.4: the attachment shell and its host.
    Attachment,
    CounterStackTarget,
    RegenerateSource,
    RegenerateTarget,
    // CR 701.26: tap and untap.
    TapTarget,
    TapTargetGroup,
    TapStatusSource,
    /// CR 115.3, 608.2b: one simultaneous removal of a counted target group.
    RemoveTargetGroup,
    SacrificeForTappedReturn,
    TapSet,
    TapTargetPlayersCreatures,
    UntapTarget,
    UntapSet,
    UntapControllerLands,
    /// CR 701.27a,f: a permanent's ability transforms that permanent.
    TransformSource,
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
    /// CR 701.21a: the source's current controller, or the ability's
    /// controller, sacrifices that exact permanent if they control it.
    SacrificeSource,
    SacrificeTargetOpponent,
    // CR 111: tokens.
    PredefinedTokens,
    Fight,
    Populate,
    PlayerCounters,
    Proliferate,
    CreatureTokens,
    MultiTypeCreatureTokens,
    TokensForTargetController,
    /// CR 608.2h: the controller creates the announced X or a resolution
    /// count of tokens, read once.
    CountedTokens,
    // CR 122: counters.
    PlusOneCountersSource,
    PlusOneCountersEventObject,
    PlusOneCountersTarget,
    PlusOneCountersSet,
    // CR 611.2c, 613: until end of turn.
    PumpSource,
    PumpTarget,
    PumpSet,
    PumpAnnouncedX,
    /// CR 608.2h: a P/T change by a resolution count, read once.
    PumpCounted,
    SourceKeywordsUntilEndOfTurn,
    TargetKeywordsUntilEndOfTurn,
    SetKeywordsUntilEndOfTurn,
    CombatProhibitionUntilEndOfTurn,
    /// CR 615.1, 611.2a: prevent all matching damage this turn.
    PreventDamageThisTurn,
    /// CR 615.7: prevent the next N damage to one announced recipient.
    PreventionShield,
    GainControl,
    SetCharacteristics,
    // CR 701.22-701.25: library search, shuffle, scry and surveil.
    /// CR 701.23: the controller searches their library.
    SearchController,
    SearchLibraryTop,
    SearchSplitBasicLands,
    /// CR 701.23, 608.2h: a target's last-known controller searches theirs.
    SearchTargetController,
    /// CR 701.24a.
    ShuffleController,
    ShuffleTargetController,
    /// CR 701.22a.
    ReorderLibraryTop,
    LookChooseHand,
    ScryController,
    /// CR 701.25a.
    SurveilController,
    /// CR 608.2c-d: a control node (sequence with a decision or result,
    /// condition, choice, repetition, payment) run by the continuation.
    Control,
    ResolutionTrigger,
    // CR 106, 605: mana abilities beyond the tap-only fixed, any-color and
    // commander-color surface.
    /// CR 106.1a: N mana of one color chosen from a printed list.
    ManaChoose,
    ManaCounted,
    ManaChooseDevotion,
    SpellManaCounted,
    /// CR 106.6: restrictions and riders carried by mana until it is spent.
    ManaSpendRestriction,
    /// CR 106.7: one mana of a type some lands could produce.
    ManaLandsCouldProduce,
    /// CR 605.3a, 601.2g-h: a mana ability with a mana cost.
    ManaAbilityManaCost,
    /// CR 118.3, 119.4: a mana ability with a life payment.
    ManaAbilityPayLife,
    /// CR 605.3a: a source with more than one printed mana ability.
    ManaAbilitySelection,
    /// CR 605.1a, 120.3a: "[this] deals N damage to you" after adding mana.
    ManaRiderDamageController,
    /// CR 605.1a, 119.3: "you gain N life" after adding mana.
    ManaRiderGainLifeController,
    // CR 118, 601.2b/f-h: casting and activation costs beyond fixed mana.
    /// CR 107.3a, 601.2b: a spell whose mana cost announces one X.
    XManaCost,
    /// CR 107.4e-f, 601.2f: two-color hybrid or Phyrexian cost symbols.
    HybridManaCost,
    /// CR 702.29a: cycling, a hand activation discarding its own source.
    Cycling,
    /// CR 702.29e-f: cycling pays for a qualified library search and shuffle.
    Typecycling,
    /// CR 602.5b-d: an activation's turn/step window or per-turn limit.
    ActivationRestriction,
    /// CR 113.6j, 602.2b, 701.13a: a graveyard activation exiling its source.
    GraveyardExileSource,
    /// CR 602.2b: fixed selections to discard, tap or exile as a cost.
    ChosenObjectCost,
    /// CR 119.4, 602.2b: a stack activation paying life.
    ActivationPayLife,
    /// CR 122.1, 602.2b: a stack activation removing +1/+1 counters from
    /// its source.
    ActivationRemoveCounters,
    /// CR 701.21a: a sacrifice cost of several permanents, or of any one
    /// permanent.
    SacrificeCostChoice,
    /// CR 602.2b: a chosen sacrifice paid with tapping the source.
    SacrificeCostTap,
    /// CR 109.1, 701.21a: "Sacrifice another ...", excluding the source.
    SacrificeCostAnother,
    /// CR 601.2b, 701.21a: a spell's additional sacrifice cost.
    SpellAdditionalSacrifice,
    /// CR 702.33a, d: a spell's kicker cost and the conditions reading it.
    Kicker,
    /// CR 702.27a: optional fixed mana and return to hand on resolution.
    Buyback,
    /// CR 702.34a: casting from the graveyard for the flashback cost, and
    /// exile as the spell leaves the stack.
    Flashback,
    /// CR 702.66: exile graveyard cards for generic mana during spell payment.
    Delve,
    /// CR 702.51: tap creatures to pay colored or generic mana costs.
    Convoke,
    /// CR 702.126: tap artifacts to pay generic mana costs.
    Improvise,
    /// CR 702.74a, 118.9: the evoke alternative cost and the entering
    /// permanent's sacrifice trigger reading it.
    Evoke,
    /// CR 702.30a: the echo upkeep trigger and the control history it reads.
    Echo,
    /// CR 702.84a: the graveyard return, haste, departure exile and the
    /// end-step exile of the returned permanent.
    Unearth,
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
        C::LifeControllerX
        | C::LifeEventPlayer
        | C::LifeEachOpponent
        | C::LifeTargetPlayerX
        | C::LifeTargetController
        | C::LifeGraveyardCreatureManaValue
        | C::LifeTargetControllerPower
        | C::LifeCounted
        | C::Drain
        | C::DrawControllerCounted
        | C::DrawControllerPerObject
        | C::DrawAnnouncedX
        | C::DrawEachPlayer
        | C::DrawEventPlayer
        | C::DamageTargetX
        | C::DamageEventObject
        | C::DamageSet
        | C::DamageSelectedPlayers
        | C::DestroyTargetPermanent
        | C::OptionalTargetDestroy
        | C::DestroySet
        | C::MoveTargetExile
        | C::MovePermanentOwnerHand
        | C::MoveSourceOwnerZone
        | C::ChooseObjectOwnerHand
        | C::GraveyardTargetWasCreature
        | C::ExileGraveyardTarget
        | C::MoveGraveyardTargetToOwnerLibraryTop
        | C::MoveGraveyardTargetToOwnerHand
        | C::MoveOwnGraveyardTargetToHand
        | C::MoveOwnGraveyardCardToBattlefield
        | C::MoveGraveyardCreatureToBattlefield
        | C::MoveOwnGraveyardCreatureToBattlefield
        | C::ExileTargetGraveyard
        | C::ExileGraveyards
        | C::MoveSet
        | C::Attachment
        | C::CounterStackTarget
        | C::RegenerateSource
        | C::RegenerateTarget
        | C::TapTarget
        | C::TapTargetGroup
        | C::SacrificeForTappedReturn
        | C::RemoveTargetGroup
        | C::TapStatusSource
        | C::TapTargetPlayersCreatures
        | C::TapSet
        | C::UntapTarget
        | C::UntapSet
        | C::UntapControllerLands
        | C::TransformSource
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
        | C::Fight
        | C::Populate
        | C::PlayerCounters
        | C::Proliferate
        | C::MultiTypeCreatureTokens
        | C::TokensForTargetController
        | C::CountedTokens
        | C::PlusOneCountersSet
        | C::PlusOneCountersEventObject
        | C::PumpSet
        | C::PumpAnnouncedX
        | C::PumpCounted
        | C::SourceKeywordsUntilEndOfTurn
        | C::TargetKeywordsUntilEndOfTurn
        | C::SetKeywordsUntilEndOfTurn
        | C::CombatProhibitionUntilEndOfTurn
        | C::PreventDamageThisTurn
        | C::PreventionShield
        | C::GainControl
        | C::SetCharacteristics
        | C::SearchController
        | C::SearchLibraryTop
        | C::SearchSplitBasicLands
        | C::MoveTargetOwnerLibraryPosition
        | C::ShuffleResolvingSpell
        | C::ShuffleTargetOwnerLibrary
        | C::DrawShuffledTargetOwner
        | C::SearchTargetController
        | C::ShuffleController
        | C::ShuffleTargetController
        | C::ReorderLibraryTop
        | C::LookChooseHand
        | C::ScryController
        | C::SurveilController
        | C::ResolutionTrigger
        | C::Control
        | C::ManaCounted
        | C::ManaChooseDevotion
        | C::SpellManaCounted
        | C::ManaChoose
        | C::ManaSpendRestriction
        | C::ManaLandsCouldProduce
        | C::ManaAbilityManaCost
        | C::ManaAbilityPayLife
        | C::ManaAbilitySelection
        | C::ManaRiderDamageController
        | C::ManaRiderGainLifeController
        | C::XManaCost
        | C::HybridManaCost
        | C::Cycling
        | C::Typecycling
        | C::ActivationRestriction
        | C::GraveyardExileSource
        | C::ChosenObjectCost
        | C::ActivationPayLife
        | C::ActivationRemoveCounters
        | C::SacrificeCostChoice
        | C::SacrificeCostTap
        | C::SacrificeCostAnother
        | C::SpellAdditionalSacrifice
        | C::Kicker
        | C::Buyback
        | C::Flashback
        | C::Delve
        | C::Convoke
        | C::Improvise
        | C::Evoke
        | C::Echo
        | C::Unearth
        | C::SacrificeSource => Tiers::EXECUTES,
    }
}

/// The tiers each reviewed non-kernel surface has earned, by the capability
/// that names it. Kernel verbs also name capabilities, so a kernel cell is
/// fenced by both its own row in [`cell_tiers`] and these. Exhaustive: a new
/// capability is reviewed here before any definition requires it.
pub fn surface_tiers(capability: Capability) -> Tiers {
    use Capability as C;
    match capability {
        C::TemporaryPowerToughnessV1
        | C::NormalLayoutV1
        | C::BasicLandManaV1
        | C::LandPlayV1
        | C::BaseCreatureV1
        | C::CommanderIdentityV1
        | C::CastInstantV1
        | C::CastCreatureV1
        | C::CreatureTargetV1
        | C::CastSorceryV1
        | C::ControllerDrawV1
        | C::OpponentTargetV1
        | C::TargetPlayerSacrificeCreatureV1
        | C::AnyPlayerTargetV1
        | C::LifeGainV1
        | C::LifeLossV1
        | C::TriggeredAbilityV1
        | C::SelfEntersBattlefieldV1
        | C::OtherCreatureEntersBattlefieldV1
        | C::CreatureDiesV1
        | C::DefenderV1
        | C::ComposedAbilityV1 => Tiers::ALL,
        C::CastEnchantmentV1
        | C::OwnerHandMoveV1
        | C::OwnerLibraryTopMoveV1
        | C::ActivatedAbilityV1
        | C::SacrificeSourceCostV1
        | C::ControllerDiscardV1
        | C::StackSpellTargetV1
        | C::StackAbilityTargetV1
        | C::CounterStackObjectV1
        | C::TapSourceCostV1
        | C::ManaActivationCostV1
        | C::AnyDamageTargetV1
        | C::NoncombatDamageV1
        | C::SelfDiesV1 => Tiers::SEARCH_HOSTING,
        C::ModalChoiceV1
        | C::ModalDoubleFacedV1
        | C::TransformingDoubleFacedV1
        | C::AdventurerV1
        | C::TransformSourceV1
        | C::GrantedManaAbilitiesV1
        | C::DamagePreventionV1
        | C::DamageBudgetPreventionV1
        | C::StaticDamageReplacementV1
        | C::UnpreventableDamageV1
        | C::CharacteristicPowerToughnessV1 => Tiers::EXECUTES,
        C::CastArtifactV1
        | C::PermanentManaV1
        | C::CompositeActivationCostV1
        | C::FlyingV1
        | C::ReachV1
        | C::VigilanceV1
        | C::HasteV1
        | C::LifelinkV1
        | C::DeathtouchV1
        | C::TrampleV1
        | C::FirstStrikeV1
        | C::DoubleStrikeV1
        | C::MenaceV1
        | C::CreatureTokensV1
        | C::PlusOneCountersV1
        | C::DestroyCreatureV1
        | C::GraveyardRecursionV1
        | C::BasicLandSearchV1
        | C::CreatureAnthemV1
        | C::TargetPlayerDrawV1
        | C::AttachmentsV1 => Tiers::HOSTING,
        C::PredefinedArtifactTokensV1
        | C::PlayerGraveyardShuffleV1
        | C::ControllerLifeFreezeV1
        | C::ProtectionFromEverythingV1
        | C::BasicLandwalkV1
        | C::EvasionKeywordsV1
        | C::FixedColorProtectionV1
        | C::SourceCombatRestrictionsV1
        | C::CastHistoryV1
        | C::CastManaValueSweepV1
        | C::CastTriggerV1
        | C::CastColorPredicateV1
        | C::CastSubtypePredicateV1
        | C::SourceTapTriggerV1
        | C::AttackTriggerV1
        | C::DamageTriggerV1
        | C::SourceDamageTriggerV2
        | C::EventPlayerDiscardV1
        | C::EventPlayerSacrificeV1
        | C::FilteredDamageTriggerV1
        | C::DamageControllerDiscardV1
        | C::EventPlayerRecipientV1
        | C::UntapControllerLandsV1
        | C::StepBeginningTriggerV1
        | C::ContinuousStaticsV1
        | C::RuleStaticsV1
        | C::TemporaryCombatProhibitionV1
        | C::ResolvedControlV1
        | C::ResolvedCharacteristicsV1
        | C::EntryPlusOneCountersV1
        | C::ConditionalEntryTappedV1
        | C::EntryLifePaymentV1
        | C::EntryRevealV1
        | C::EntersTappedV1
        | C::TypedNonbasicIntrinsicManaV1
        | C::FlashV1
        | C::EnterAsCopyOfPermanentEnteredThisTurnV1
        | C::BlockingCreatureTargetV1
        | C::TappedPermanentTargetV1
        | C::PowerFilteredRemovalV1
        | C::UntapPermanentV1
        | C::OptionalObjectTargetV1
        | C::TapPermanentV1
        | C::ActivatedTapPermanentV1
        | C::ActivatedCreatureTapV1
        | C::TapSourceDestroyV1
        | C::TapSourceUntapCreatureV1
        | C::ActivatedPermanentOwnerHandV1
        | C::ActivatedArtifactDrawV1
        | C::PermanentOwnerHandMoveV1
        | C::FixedMillProgramV1
        | C::ControllerCountedDrawV1
        | C::ControlledCreatureSpellCopyV1
        | C::PayLifeCostV1
        | C::DiscardSourceCostV1
        | C::ExileSourceCostV1
        | C::ChosenObjectCostV1
        | C::PlayerOrPlaneswalkerTargetV1
        | C::OpponentLifeDrainV1
        | C::KernelControlV1
        | C::ResolutionObjectChoiceV1
        | C::TargetCharacteristicsV1
        | C::OptionalTargetGroupV1
        | C::CountedTargetGroupV1
        | C::TargetGroupRemovalV1
        | C::EntryGraveyardExileV1
        | C::ControlledLandEntryV1
        | C::CreatureOrPlaneswalkerTargetV1
        | C::IndestructibleV1
        | C::ActivatedSourceKeywordGrantV1
        | C::FightV1
        | C::PopulateV1
        | C::PlayerCountersV1
        | C::RegenerationShieldV1
        | C::ProliferateV1
        | C::MultiTypeCreatureTokensV1
        | C::GraveyardCardTargetV1
        | C::OwnGraveyardCardReturnV1
        | C::PrivateLibraryReorderV1
        | C::SearchLibraryTopV1
        | C::SearchSplitBasicLandsV1
        | C::OwnerLibraryBottomMoveV1
        | C::OwnerLibraryShuffleMoveV1
        | C::PrivateLibrarySelectionV1
        | C::PermanentRemovalV1
        | C::SacrificeV1
        | C::FixedArtifactSacrificeCostV1
        | C::BoardWipesV1
        | C::SelfSpellProtectionV1
        | C::AnnouncedXDrawV1
        | C::EachPlayerDrawV1
        | C::AnnouncedXEffectsV1
        | C::TargetPlayerDiscardV1
        | C::RandomPlayerDiscardV1
        | C::GroupHandDiscardV1
        // CR 603.4: opened with its executor and rules tests; no review yet.
        | C::InterveningIfV1
        | C::ResolutionTriggerV1
        | C::EventTriggerV1
        | C::TriggerTurnHistoryV1
        | C::SacrificeSourceV1 => Tiers::EXECUTES,
    }
}

/// The audited kernel cells of every program of a definition. A node that
/// does not check, or a surface the runtime cannot execute, occupies no
/// cell: its error is returned, so an invalid definition can never be read
/// as one that earned a tier.
pub fn definition_cells(d: &Definition) -> Result<BTreeSet<Cell>, KirError> {
    let mut cells = BTreeSet::new();
    let mut add = |p: &SpellProgram,
                   context: Context,
                   event: Option<&TriggerCondition>|
     -> Result<(), KirError> {
        let env = Env {
            context,
            targets: &p.targets,
            event,
            // CR 107.3a: only the spell announces its own X.
            announced_x: context == Context::Spell && d.x_mana_cost,
        };
        for Instruction::Kernel { node, .. } in &p.instructions {
            cells.extend(check(node, &env)?.cells);
        }
        Ok(())
    };
    for p in d.spell_programs() {
        add(p, Context::Spell, None)?;
    }
    for a in &d.activated {
        for p in a.programs() {
            add(p, Context::Activated, None)?;
        }
    }
    for a in &d.triggered {
        for p in a.programs() {
            add(p, Context::Triggered, Some(&a.condition))?;
        }
    }
    for a in &d.mana_abilities {
        cells.extend(mana_ability_cells(d, a)?);
        if a.condition.is_some() {
            cells.insert(Cell::ActivationRestriction);
        }
    }
    // CR 113.10a, 605.3a: a granted mana ability carries its own production
    // and cost cells, and its recipient may then have several to choose from.
    for s in &d.static_abilities {
        if let StaticEffect::Continuous { changes, .. } = &s.effect {
            for change in changes {
                if let Change::AddManaAbilities { abilities } = change {
                    for a in abilities {
                        cells.extend(mana_ability_cells(d, a)?);
                    }
                    cells.insert(Cell::ManaAbilitySelection);
                }
            }
        }
    }
    // A surface with no executor (for example a life payment on a stack
    // activation's composite cost) earns no tier, so no cell qualifies it.
    executes::surfaces(d).map_err(KirError::Unaudited)?;
    if d.characteristics.is_attachment() {
        cells.insert(Cell::Attachment);
    }
    if d.x_mana_cost {
        cells.insert(Cell::XManaCost);
    }
    if !d.characteristics.hybrid_mana.is_empty() {
        cells.insert(Cell::HybridManaCost);
    }
    if d.improvise {
        cells.insert(Cell::Improvise);
    }
    if d.convoke {
        cells.insert(Cell::Convoke);
    }
    if d.delve {
        cells.insert(Cell::Delve);
    }
    if d.flashback.is_some() {
        cells.insert(Cell::Flashback);
    }
    if d.alternative_cost.is_some() {
        cells.insert(Cell::Evoke);
    }
    if d.triggered.iter().any(|t| {
        matches!(
            t.condition.intervening_if(),
            Some(Cond::ControlledSinceLastUpkeep {})
        )
    }) {
        cells.insert(Cell::Echo);
    }
    if let Some(p) = &d.spell {
        if p.additional_sacrifice.is_some() {
            cells.insert(Cell::SpellAdditionalSacrifice);
        }
        for cost in &p.optional_additional_costs {
            cells.insert(if cost.identity == crate::BUYBACK_COST {
                Cell::Buyback
            } else {
                Cell::Kicker
            });
        }
    }
    for a in &d.activated {
        if crate::cycling::typecycling_activation(a) {
            cells.insert(Cell::Typecycling);
        } else if crate::cycling::hand_discard_draw_activation(a) {
            cells.insert(Cell::Cycling);
        }
        if a.restriction.is_some() {
            cells.insert(Cell::ActivationRestriction);
        }
        if matches!(a.cost, ActivationCost::ExileSource { .. }) {
            cells.insert(Cell::GraveyardExileSource);
        }
        match &a.cost {
            ActivationCost::ChooseObjects(_) => {
                cells.insert(Cell::ChosenObjectCost);
            }
            ActivationCost::Composite { costs } => {
                if costs.pay_life.is_some() {
                    cells.insert(Cell::ActivationPayLife);
                }
                if costs.remove_source_plus_one_counters.is_some() {
                    cells.insert(Cell::ActivationRemoveCounters);
                }
            }
            ActivationCost::SacrificePermanent {
                count,
                filter,
                tap,
                another,
                ..
            } => {
                if *count != 1 || (filter.types.is_empty() && filter.subtypes.is_empty()) {
                    cells.insert(Cell::SacrificeCostChoice);
                }
                if *tap {
                    cells.insert(Cell::SacrificeCostTap);
                }
                if *another {
                    cells.insert(Cell::SacrificeCostAnother);
                }
            }
            _ => {}
        }
    }
    if let Some(back) = &d.back_face {
        cells.extend(definition_cells(back)?);
    }
    Ok(cells)
}

/// What keeps a definition below `tier`: the surfaces (by capability) and
/// kernel cells that have not earned it. Both empty means the definition
/// earns the tier, the meet over its cells. A node that does not check is
/// an error, never a qualified cell.
pub fn unqualified(
    d: &Definition,
    tier: ExecutionTier,
) -> Result<(Vec<Capability>, Vec<Cell>), KirError> {
    Ok((
        d.requirements()
            .into_iter()
            .filter(|c| !surface_tiers(*c).admits(tier))
            .collect(),
        definition_cells(d)?
            .into_iter()
            .filter(|c| !cell_tiers(*c).admits(tier))
            .collect(),
    ))
}

/// The tiers a definition has earned: the meet over its surfaces and cells,
/// or the check error of a node that occupies no cell.
pub fn definition_tiers(d: &Definition) -> Result<Tiers, KirError> {
    Ok(d.requirements()
        .into_iter()
        .map(surface_tiers)
        .chain(definition_cells(d)?.into_iter().map(cell_tiers))
        .fold(Tiers::ALL, Tiers::meet))
}

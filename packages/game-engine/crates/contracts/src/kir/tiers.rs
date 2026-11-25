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
use crate::{Capability, Definition, Instruction, SpellProgram, TriggerCondition};

mod executes;

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
    LifeCounted,
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
    ExileTargetGraveyard,
    ExileGraveyards,
    MoveSet,
    // CR 701.26: tap and untap.
    TapTarget,
    TapSet,
    UntapTarget,
    UntapSet,
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
    TokensForTargetController,
    // CR 122: counters.
    PlusOneCountersSource,
    PlusOneCountersTarget,
    // CR 611.2c, 613: until end of turn.
    PumpSource,
    PumpTarget,
    PumpSet,
    SourceKeywordsUntilEndOfTurn,
    TargetKeywordsUntilEndOfTurn,
    SetKeywordsUntilEndOfTurn,
    /// CR 608.2c-d: a control node (sequence with a decision or result,
    /// condition, choice, repetition, payment) run by the continuation.
    Control,
    // CR 106, 605: mana abilities beyond the tap-only fixed, any-color and
    // commander-color surface.
    /// CR 106.1a: N mana of one color chosen from a printed list.
    ManaChoose,
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
        | C::LifeTargetControllerPower
        | C::LifeCounted
        | C::Drain
        | C::DrawControllerCounted
        | C::DrawAnnouncedX
        | C::DrawEachPlayer
        | C::DrawEventPlayer
        | C::DamageTargetX
        | C::DamageSet
        | C::DamageSelectedPlayers
        | C::DestroyTargetPermanent
        | C::OptionalTargetDestroy
        | C::DestroySet
        | C::MoveTargetExile
        | C::MovePermanentOwnerHand
        | C::ExileTargetGraveyard
        | C::ExileGraveyards
        | C::MoveSet
        | C::TapTarget
        | C::TapSet
        | C::UntapTarget
        | C::UntapSet
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
        | C::TokensForTargetController
        | C::PumpSet
        | C::SourceKeywordsUntilEndOfTurn
        | C::TargetKeywordsUntilEndOfTurn
        | C::SetKeywordsUntilEndOfTurn
        | C::Control
        | C::ManaChoose
        | C::ManaLandsCouldProduce
        | C::ManaAbilityManaCost
        | C::ManaAbilityPayLife
        | C::ManaAbilitySelection
        | C::ManaRiderDamageController
        | C::ManaRiderGainLifeController => Tiers::EXECUTES,
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
        | C::CombatDamagePreventionV1
        | C::DamageBudgetPreventionV1
        | C::StaticDamageReplacementV1
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
        | C::SelfDiesV1
        | C::InterveningIfV1
        | C::ModalChoiceV1 => Tiers::SEARCH_HOSTING,
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
        | C::EntryPlusOneCountersV1
        | C::ConditionalEntryTappedV1
        | C::EntryLifePaymentV1
        | C::EntersTappedV1
        | C::TypedNonbasicIntrinsicManaV1
        | C::FlashV1
        | C::EnterAsCopyOfPermanentEnteredThisTurnV1
        | C::BlockingCreatureTargetV1
        | C::TappedPermanentTargetV1
        | C::PowerFilteredRemovalV1
        | C::UntapPermanentV1
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
        | C::PlayerOrPlaneswalkerTargetV1
        | C::OpponentLifeDrainV1
        | C::KernelControlV1
        | C::EntryGraveyardExileV1
        | C::ControlledLandEntryV1
        | C::CreatureOrPlaneswalkerTargetV1
        | C::IndestructibleV1
        | C::ActivatedSourceKeywordGrantV1
        | C::MultiTypeCreatureTokensV1
        | C::OwnGraveyardCardReturnV1
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
        | C::GroupHandDiscardV1 => Tiers::EXECUTES,
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
            announced_x: false,
        };
        for Instruction::Kernel { node, .. } in &p.instructions {
            cells.extend(check(node, &env)?.cells);
        }
        Ok(())
    };
    if let Some(p) = &d.spell {
        add(p, Context::Spell, None)?;
    }
    for a in &d.activated {
        add(&a.program, Context::Activated, None)?;
    }
    for a in &d.triggered {
        add(&a.program, Context::Triggered, Some(&a.condition))?;
    }
    for a in &d.mana_abilities {
        cells.extend(mana_ability_cells(d, a)?);
    }
    // A surface with no executor (for example a life payment on a stack
    // activation's composite cost) earns no tier, so no cell qualifies it.
    executes::surfaces(d).map_err(KirError::Unaudited)?;
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

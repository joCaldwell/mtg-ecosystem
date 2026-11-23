//! Executable Card IR contracts. No rules execution or parser dependency.
mod builders;
mod mana_combination;
pub use mana_combination::{MANA_COMBINATION_COLORS, validate_mana_combination};
pub mod commander_modal_keyword;

mod bestow;
pub use bestow::{BestowCastPlan, BestowForm, bestow_alternative};
pub mod hand_cast_cascade;
pub mod library_top_mana;
mod unspent_mana;

mod opening_placement;
pub use opening_placement::opening_placement_ability;

mod colored_library_search;
pub use colored_library_search::{ColoredLibrarySearch, ColoredSearchAssignment};
mod color_hybrid_cost;
pub use color_hybrid_cost::{ColorHybridSymbol, HybridPayment, color_hybrid_cost};
mod checkland_entry;
mod cycling;
pub mod paid_restricted_land;

pub use cycling::hand_discard_draw_activation;

mod hybrid_filter_mana;
mod mana_life_gain;
mod mana_spend;

pub use mana_spend::{ManaSpellSubtype, ManaSpellTraits, ManaSpendRestriction, RestrictedMana};
mod listed_mana_choice;
mod pain_mana;
pub use hybrid_filter_mana::hybrid_filter_colors;
pub use pain_mana::disjoint_pain_mana_family;
pub mod basic_landwalk;
mod cast_entry;
pub mod cast_subtype;
pub mod combat_window;

mod entry_copy;
pub mod modal_land;
mod saga;
pub mod transform_face;
pub use saga::saga_damage_target;
pub mod fixed_color_protection;
mod fixed_creature_activation;
pub mod fixed_creature_damage;
pub use fixed_creature_activation::fixed_activation_keyword;
mod cast_mana_value_sweep;
mod cast_payer_draw;
mod controlled_creature_spell_copy;
pub mod demonstrate;

mod successor_tuck;
pub use successor_tuck::{LibraryPosition, SourceMoveOrigin, TriggerSuccessorZone};
mod fetched_land_untap;

mod typed_land_search;
pub use typed_land_search::{BasicLandType, TypedLandSearch};
mod build_input;
pub use build_input::engine_build_input;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
mod activated_artifact_draw;
pub mod activated_creature_tap;
pub mod activated_source_keyword;
pub use activated_artifact_draw::activated_artifact_draw_ability;

mod artifact_modal;

mod controlled_land_entry;
mod land_reclamation;

mod entry_graveyard_exile;

mod land_entry_gain_life;
mod land_entry_scry;
pub mod proliferate;
pub mod target_color;

mod land_sacrifice_draw;
mod opponent_count_entry;
mod other_land_entry;
mod reveal_land_entry;
mod set_exile;

mod hand_cast;

mod targeted_game_loss;

pub mod ability_sequence;
pub mod attachment_modifiers;
mod controller_creature_pump;
mod controller_library;
pub mod creature_planeswalker_return;
pub mod equipped_trigger;
pub mod impulse_play;
pub mod intrinsic_keyword_blocks;
pub mod mana_tap_observer;
pub use mana_tap_observer::ManaTapFilter;
pub mod optional_entry_search;
pub mod optional_trigger;
pub mod permanent_entry;
pub mod source_damage_counters;
pub use impulse_play::PlayWindow;
pub mod resolution_mana;
pub use resolution_mana::ManaResolutionQuantity;
pub mod predefined_tokens;
pub use predefined_tokens::PredefinedToken;
mod fixed_mill;
mod graveyard_battlefield_return;
pub mod graveyard_shuffle;
mod group_discard;
pub mod modal_targets;
pub mod scoped_creature_stats;
pub mod scoped_static;

mod nonland_exile_life;

pub mod linked_hideaway;
mod source_dies_nonland_destroy;
pub mod source_regeneration;

mod same_color_mana;
pub mod self_graveyard_shuffle;
mod self_spell_protection;
pub mod source_attack_pump;
pub mod source_combat_restrictions;
pub use controller_library::matches_fixed_amount;

use kir::{Amount, Context, Destination, PlayerExpr, SetExpr, Verb};

use serde::{Deserialize, Serialize, de};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

mod archives;
pub use archives::*;
pub mod composed_spell;
pub mod composition;
pub mod ordinary_sequence;
mod own_graveyard_card_return;

mod power_filtered_removal;

pub mod bulk_catalog;
pub mod kir;
mod strict_io;
pub use strict_io::{JsonByteProof, strict_read_seekable, strict_read_seekable_verified};

/// The sole active Card IR schema accepted and emitted by the engine.
pub const CURRENT_IR_VERSION: u32 = 1;

fn current_ir_version_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({"type":"integer", "const":CURRENT_IR_VERSION})
}

pub const RULES_EDITION: &str = "2026-09-25";
pub const RULES_SHA256: &str = "8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca";
pub const SCENARIO_PROFILE: &str = "land-return-departure-scenario-v2";

mod quantity;
pub use quantity::Quantity;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum Color {
    W,
    U,
    B,
    R,
    G,
    C,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManaCost {
    pub generic: Quantity,
    pub colored: Vec<Color>,
}
impl ManaCost {
    fn printed_fixed_cost(&self) -> String {
        let mut text = String::new();
        if self.generic != Quantity::zero() || self.colored.is_empty() {
            text.push_str(&format!("{{{}}}", self.generic));
        }
        for color in &self.colored {
            text.push_str(&format!("{{{color:?}}}"));
        }
        text
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Land,
    Creature,
    Instant,
    Sorcery,
    Enchantment,
    Artifact,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Characteristics {
    /// Canonical execution type: creature first, then land, artifact, enchantment.
    pub kind: CardKind,
    /// Other simultaneous card types; empty preserves existing catalog bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_types: Vec<CardKind>,
    pub subtypes: Vec<String>,
    pub basic: bool,
    pub legendary: bool,
    /// CR 205.4g; omitted for nonsnow characteristics.
    #[serde(default, skip_serializing_if = "is_false")]
    pub snow: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hybrid_mana: Vec<ColorHybridSymbol>,
    pub mana_cost: ManaCost,
    pub colors: Vec<Color>,
    pub color_identity: Vec<Color>,
    pub power: Option<Quantity>,
    pub toughness: Option<Quantity>,
}
fn is_false(value: &bool) -> bool {
    !*value
}
impl Characteristics {
    /// Each colored hybrid or Phyrexian symbol counts once (CR 107.4e–f).
    pub fn colored_mana_symbol_count(&self) -> Quantity {
        let fixed = self
            .mana_cost
            .colored
            .iter()
            .filter(|color| **color != Color::C)
            .count();
        Quantity::from(fixed as i64 + self.hybrid_mana.len() as i64)
    }

    pub fn printed_mana_value(&self) -> Quantity {
        let mut value = self.mana_cost.generic.clone();
        value.add(&Quantity::from(
            (self.mana_cost.colored.len() + self.hybrid_mana.len()) as i64,
        ));
        value
    }

    pub fn has_type(&self, kind: CardKind) -> bool {
        self.kind == kind || self.additional_types.contains(&kind)
    }
}

/// Rule-bearing subtype boundaries from CR 205.3g–k, 301.5–7, 303.4/7,
/// 709.5, 714–720. Other subtype labels do not themselves supply abilities.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const UNSUPPORTED_RULES_SUBTYPES: &[&str] = &[
    "Saga",
    "Class",
    "Case",
    "Vehicle",
    "Curse",
    "Spacecraft",
    "Attraction",
    "Room",
    "Contraption",
    "Fortification",
    "Role",
    "Adventure",
    "Omen",
];
pub fn unsupported_rules_subtype(subtype: &str) -> bool {
    UNSUPPORTED_RULES_SUBTYPES.contains(&subtype)
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ComposedOrdinarySpellV1,
    EquippedCreatureTriggerV1,
    ResolutionManaV1,
    PredefinedArtifactTokensV1,
    FilteredStaticModifiersV1,
    PlayerGraveyardShuffleV1,
    OpeningPlacementV1,
    ExileResolvingSpellV1,
    ControllerProtectionUntilNextTurnV1,
    ControllerLifeFreezeV1,
    ControlledPermanentPhasingV1,
    TargetedPhasingV1,
    CountedSelfGenericCostReductionV1,
    OptionalAdditionalManaCostV1,
    ResolutionCastV1,
    CombatHandCastV1,
    CascadeV1,
    LookTopMayRevealForManaV1,
    RevealAndCastV1,
    DemonstrateV1,
    ProtectionFromEverythingV1,
    SelfGraveyardRevealShuffleV1,
    BasicLandwalkV1,
    FixedColorProtectionV1,
    SourceCombatRestrictionsV1,
    StaticSpellCascadeGrantV1,
    CascadeLandEntryV1,
    HandCastCascadeGrantV1,
    HideawayV1,
    LinkedExilePlayV1,
    CastHistoryV1,
    CastManaValueSweepV1,
    RandomBottomV1,
    CastTriggerV1,
    CastOriginTriggerV1,
    CastColorPredicateV1,
    CastSubtypePredicateV1,
    TemporaryCombatRestrictionV1,
    OptionalEffectBlockV1,
    ManaTapTriggerV1,
    LibraryPlayWindowV1,
    CastSpellColorCountersV1,
    CounterRemovalManaV1,
    ListedManaChoiceV1,
    SameColorManaV1,
    DisjointPainManaV1,
    HybridFilterManaV1,
    ManaLifeGainV1,
    RestrictedManaV1,
    SourceTapTriggerV1,
    ControllerCreaturePlusOneCounterReplacementV1,
    AttackTriggerV1,
    DamageTriggerV1,
    SourceDamageTriggerV2,
    OptionalControllerDrawV1,
    CastPayerDrawV1,
    EventPlayerDiscardV1,
    EventPlayerSacrificeV1,
    FilteredDamageTriggerV1,
    DamageControllerDiscardV1,
    DamageQuantityDrawV1,
    SnowSupertypeV1,
    AttackingCreatureKeywordsV1,
    OpponentCastProhibitionV1,
    UnlimitedHandSizeV1,
    ReducedHandSizeV1,
    IncreasedHandSizeV1,
    GroupedCreatureDamageTriggerV1,
    EventPlayerRecipientV1,
    SourceStatusConditionV1,
    CastEntryConditionV1,
    UntapControllerLandsV1,
    StepBeginningTriggerV1,
    EntersTappedV1,
    ConditionalBasicLandEntryV1,
    ConditionalOtherLandEntryV1,
    ConditionalOpponentCountEntryV1,
    RevealLandEntryV1,
    ConditionalLandTypeEntryV1,
    OptionalLifeLandEntryV1,
    TypedNonbasicIntrinsicManaV1,
    UnspentManaConversionV1,
    ControllerLandGrantedManaV1,
    ControllerLandBasicTypesV1,
    ControllerNonlandColorsV1,
    FlashV1,
    EnterAsCopyOfPermanentEnteredThisTurnV1,
    CastArtifactV1,
    PermanentManaV1,
    OpponentLandManaV1,
    DistinctPermanentColorsV1,
    CompositeActivationCostV1,
    CastEnchantmentV1,
    AuraControlV1,
    BestowV1,
    TemporaryPowerToughnessV1,
    SourceRegenerationV1,
    CombatDamagePreventionV1,
    DamageBudgetPreventionV1,
    StaticDamageReplacementV1,
    ControlledCreatureDamageReplacementV1,
    SagaChaptersV1,
    ModalLandFacesV1,
    TransformV1,
    DelayedControllerCreatureEntryV1,
    DelayedControllerSpellCastV1,
    NormalLayoutV1,
    ColorHybridCostV1,
    PhyrexianCostV1,
    GreatestPowerV1,
    BasicLandManaV1,
    LandPlayV1,
    BaseCreatureV1,
    CommanderIdentityV1,
    CastInstantV1,
    CastCreatureV1,
    CreatureTargetV1,
    BlockingCreatureTargetV1,
    TappedPermanentTargetV1,
    PowerFilteredRemovalV1,
    UntapPermanentV1,
    TapPermanentV1,
    ActivatedTapPermanentV1,
    ActivatedCreatureTapV1,
    TapSourceDestroyV1,
    TapSourceUntapCreatureV1,
    ActivatedSpellCounterV1,
    ActivatedPermanentOwnerHandV1,
    ActivatedArtifactDrawV1,
    OwnerHandMoveV1,
    PermanentOwnerHandMoveV1,
    CastSorceryV1,
    OwnerLibraryTopMoveV1,
    ActivatedAbilityV1,
    SacrificeSourceCostV1,
    ControllerDrawV1,
    ControllerScryV1,
    ControllerSurveilV1,
    FixedMillProgramV1,
    ControllerLibrarySelectionV1,
    ControllerLibraryOrderV1,
    ControllerHandReturnV1,
    ControllerDrawDiscardV1,
    ControllerCountedDrawV1,
    ControllerDrawLandAllowanceV1,
    TriggeredSuccessorLibraryTuckV1,
    ControllerDiscardV1,
    OpponentTargetV1,
    TargetPlayerSacrificeCreatureV1,
    StackSpellTargetV1,
    StackAbilityTargetV1,
    CounterStackObjectV1,
    CounterSpellControllerMillV1,
    CounterUnlessGenericV1,
    ControlledCreatureSpellCopyV1,
    TapSourceCostV1,
    PayLifeCostV1,
    ManaActivationCostV1,
    HandActivationV1,
    DiscardSourceCostV1,
    AnyDamageTargetV1,
    RemoveIndestructibleV1,
    PlayerOrPlaneswalkerTargetV1,
    AnyPlayerTargetV1,
    NoncombatDamageV1,
    LifeGainV1,
    LifeLossV1,
    OpponentLifeDrainV1,
    KernelControlV1,
    TriggeredAbilityV1,
    SelfEntersBattlefieldV1,
    OtherCreatureEntersBattlefieldV1,
    PowerFilteredCreatureEntryV1,
    CreatureEntryReturnV1,
    EntryGraveyardExileV1,
    LandEntryGainLifeV1,
    LandEntryScryV1,
    ControlledLandEntryV1,
    LandReclamationV1,
    LandSacrificeDrawV1,
    CreatureDiesV1,
    SelfDiesV1,
    InterveningIfV1,
    CreatureOrPlaneswalkerTargetV1,
    FixedTargetModesV1,
    RepeatedTargetModesV1,
    ModalChoiceV1,
    CommanderModalKeywordGrantV1,
    NonemptyModeSubsetV1,
    DefenderV1,
    FlyingV1,
    ReachV1,
    VigilanceV1,
    HasteV1,
    LifelinkV1,
    DeathtouchV1,
    IndestructibleV1,
    StaticPermanentIndestructibleGrantV1,
    TemporaryIndestructibleSpellV1,
    TemporaryFlyingSpellV1,
    TemporaryDeathtouchSpellV1,
    CompoundIndestructibleSpellV1,
    TemporaryCreatureKeywordProgramV1,
    TemporaryPermanentKeywordProgramV1,
    TemporaryCreatureSetKeywordProgramV1,
    TemporaryPermanentSetKeywordProgramV1,
    ActivatedSourceKeywordGrantV1,
    TrampleV1,
    FirstStrikeV1,
    DoubleStrikeV1,
    MenaceV1,
    CreatureTokensV1,
    MultiTypeCreatureTokensV1,
    PlusOneCountersV1,
    ProliferateV1,
    TargetColorSetV1,
    DividedPlusOneCountersV1,
    DestroyCreatureV1,
    GraveyardRecursionV1,
    GraveyardBattlefieldReturnV1,
    OwnGraveyardCardReturnV1,
    BasicLandSearchV1,
    BasicLandFetchV1,
    BasicLandSplitSearchV1,
    ColoredLibrarySearchV1,
    TypedLandSearchV1,
    CreatureAnthemV1,
    ScopedCreatureStatsV1,
    TargetPlayerDrawV1,
    AttachmentsV1,
    PermanentRemovalV1,
    SacrificeV1,
    FixedArtifactSacrificeCostV1,
    BoardWipesV1,
    SelfSpellProtectionV1,
    AnnouncedXDrawV1,
    EachPlayerDrawV1,
    AnnouncedXEffectsV1,
    TargetedCountedDrawV1,
    TargetPlayerDiscardV1,
    RandomPlayerDiscardV1,
    GroupHandDiscardV1,
    TargetedGameLossV1,
    // Admitted by `composition`, not by a whole-card family. A plain comment:
    // a documented variant would change the published enum schema shape.
    ComposedAbilityV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunctioningZone {
    Hand,
    Battlefield,
    Stack,
    /// Reserved for the source card's own graveyard-move replacement.
    Any,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Chooser {
    Controller,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStage {
    Announcement,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetDomain {
    CreatureOrPlaneswalker,
    PlayerGraveyardCards {
        owner_slot: u32,
        maximum: u8,
    },
    BattlefieldPermanent,
    ControlledBattlefieldPermanent,
    ControlledBattlefieldCreature,
    BattlefieldCreature,
    /// CR 509.1g: a current blocking creature; ordinary spell pumps only.
    BlockingBattlefieldCreature,
    OwnGraveyardCreatureCard,
    OwnGraveyardCard,
    OpponentPlayer,
    StackSpell,
    ControlledCreatureSpell,
    StackActivatedOrTriggeredAbility,
    /// CR 115.4: creatures, players, planeswalkers and battles. The closed
    /// scenario admits only creature/player inhabitants, not a narrower domain.
    AnyDamageTarget,
    PlayerOrPlaneswalker,
    AnyPlayer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    pub slot: u32,
    pub selection_id: String,
    pub chooser: Chooser,
    pub stage: EvaluationStage,
    pub domain: TargetDomain,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<PermanentFilter>,
}
/// Finite CR 115 filters; an empty type set means any permanent. Types are
/// alternatives; exclusions, controller and tapped restrictions are intersections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermanentFilter {
    pub types: Vec<CardKind>,
    pub non_types: Vec<CardKind>,
    pub non_colors: Vec<Color>,
    pub opponent: bool,
    /// Require the current battlefield incarnation to be tapped (CR 110.5,
    /// 601.2c, 608.2b). False imposes no tap-status restriction.
    #[serde(default, skip_serializing_if = "is_false")]
    pub tapped: bool,
    /// Derived creature power at target selection and resolution (CR 601.2c, 608.2b).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "power_filtered_removal::deserialize_power"
    )]
    #[schemars(with = "CreaturePowerFilter")]
    pub power: Option<CreaturePowerFilter>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "comparison", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreaturePowerFilter {
    AtLeast { value: Quantity },
    AtMost { value: Quantity },
}
/// Closed non-targeted sets, evaluated as an instruction is performed (CR 608.2h).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectSet {
    ControllerCreatures,
    ControllerPermanents,
    Creatures,
    NonlandPermanents,
    OpponentNonlandPermanents,
    Enchantments,
    Artifacts,
    GraveyardCards,
    AttackingCreatures,
    OpponentCreatures,
    FlyingCreatures,
    NonflyingCreatures,
}
impl ObjectSet {
    /// True when `what` is this set's kernel set in some context: "you" is
    /// the spell's or the ability's controller (CR 109.5).
    pub fn denotes(&self, what: &SetExpr) -> bool {
        [Context::Spell, Context::Activated]
            .into_iter()
            .any(|context| *what == self.kernel(context))
    }
    /// The kernel set of an end-of-turn set pump (CR 613.4c).
    pub fn pumped(instruction: &Instruction) -> Option<&SetExpr> {
        instruction
            .pumped()
            .map(|(what, ..)| what)
            .filter(|what| matches!(what, SetExpr::Matching { .. }))
    }
    /// The kernel set this closed set denotes for "you" in `context`. Kernel
    /// verbs take a general filter; the instructions that still carry an
    /// `ObjectSet` name one of these.
    pub fn kernel(&self, context: Context) -> SetExpr {
        use kir::Atom;
        let kind = |kind| Atom::Type { kind };
        let controlled = |player| Atom::ControlledBy { player };
        let you = || controlled(PlayerExpr::you(context));
        let opponents = || controlled(PlayerExpr::EachOpponent);
        let flying = KeywordAbility::Flying;
        let atoms = match self {
            Self::GraveyardCards => return SetExpr::graveyards(),
            Self::ControllerCreatures => vec![kind(CardKind::Creature), you()],
            Self::ControllerPermanents => vec![you()],
            Self::Creatures => vec![kind(CardKind::Creature)],
            Self::NonlandPermanents => return SetExpr::battlefield(kir::Pred::nonland_permanents()),
            Self::OpponentNonlandPermanents => vec![
                Atom::NotType {
                    kind: CardKind::Land,
                },
                opponents(),
            ],
            Self::Enchantments => vec![kind(CardKind::Enchantment)],
            Self::Artifacts => vec![kind(CardKind::Artifact)],
            Self::AttackingCreatures => vec![kind(CardKind::Creature), Atom::Attacking],
            Self::OpponentCreatures => vec![kind(CardKind::Creature), opponents()],
            Self::FlyingCreatures => {
                vec![kind(CardKind::Creature), Atom::Keyword { keyword: flying }]
            }
            Self::NonflyingCreatures => vec![
                kind(CardKind::Creature),
                Atom::NotKeyword { keyword: flying },
            ],
        };
        SetExpr::battlefield(kir::Pred(atoms))
    }
}
/// The affected seats are evaluated relative to the invocation controller at resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupDiscardScope {
    EachPlayer,
    EachOpponent,
}
impl GroupDiscardScope {
    /// The kernel players the scope denotes (CR 102.3).
    pub fn player(self) -> kir::PlayerExpr {
        match self {
            Self::EachPlayer => kir::PlayerExpr::EachPlayer,
            Self::EachOpponent => kir::PlayerExpr::EachOpponent,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectRecipient {
    Controller,
    TargetController { target_slot: u32 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetAmount {
    Fixed {
        amount: Quantity,
    },
    Attribute {
        target_slot: u32,
        attribute: TargetAttribute,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetAttribute {
    Power,
    ManaValue,
}
/// Controller means the frozen invocation controller (CR 109.5), not the
/// current controller of its physical source. Target refers to AnyPlayer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LifeRecipient {
    /// Frozen optional actor supplied by the trigger event; never the turn owner.
    EventPlayer,
    EachOpponent,
    Controller,
    Target {
        target_slot: u32,
    },
}
impl LifeRecipient {
    /// The kernel player this recipient denotes for "you" in `context`. An
    /// event player needs the role its event supplies and is not converted.
    pub fn kernel(&self, context: Context) -> Option<PlayerExpr> {
        match self {
            Self::Controller => Some(PlayerExpr::you(context)),
            Self::Target { target_slot } => Some(PlayerExpr::target(*target_slot)),
            Self::EachOpponent => Some(PlayerExpr::EachOpponent),
            Self::EventPlayer => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LibraryRemainder {
    Bottom,
    Graveyard,
}
impl<'de> Deserialize<'de> for LibraryRemainder {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "bottom" => Ok(Self::Bottom),
            "graveyard" => Ok(Self::Graveyard),
            _ => Err(serde::de::Error::custom(
                "unsupported library remainder destination",
            )),
        }
    }
}
/// Resolution actions use a separate binding domain from draw/life. The event
/// player is the frozen damaged player of a source-damage trigger (CR 603.2c).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionRecipient {
    Controller,
    EventPlayer,
}
impl<'de> Deserialize<'de> for ActionRecipient {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Match the schema's string vocabulary; serde's externally tagged
        // unit-object alternative is not an executable recipient encoding.
        match String::deserialize(deserializer)?.as_str() {
            "controller" => Ok(Self::Controller),
            "event_player" => Ok(Self::EventPlayer),
            _ => Err(serde::de::Error::custom("unsupported action recipient")),
        }
    }
}
/// CR 111.3–4: precisely defined creature tokens, never copied card definitions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatureToken {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_types: Vec<CardKind>,
    pub colors: Vec<Color>,
    pub subtypes: Vec<String>,
    pub power: Quantity,
    pub toughness: Quantity,
    pub keywords: Vec<KeywordAbility>,
}
impl CreatureToken {
    pub fn characteristics(&self) -> Characteristics {
        Characteristics {
            kind: CardKind::Creature,
            hybrid_mana: vec![],
            additional_types: self.additional_types.clone(),
            subtypes: self.subtypes.clone(),
            basic: false,
            legendary: false,
            snow: false,
            mana_cost: ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
            colors: self.colors.clone(),
            color_identity: self.colors.clone(),
            power: Some(self.power.clone()),
            toughness: Some(self.toughness.clone()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.additional_types.as_slice(),
            [] | [CardKind::Artifact] | [CardKind::Enchantment]
        ) || (!self.additional_types.is_empty()
            && self.subtypes.iter().any(|s| {
                unsupported_rules_subtype(s) || matches!(s.as_str(), "Aura" | "Equipment")
            }))
            || self
                .keywords
                .contains(&KeywordAbility::ProtectionFromEachColor)
            || self
                .keywords
                .contains(&KeywordAbility::ProtectionFromEverything)
            || self
                .keywords
                .iter()
                .any(|k| k.basic_landwalk_type().is_some() || k.fixed_protection_color().is_some())
            || self.keywords.contains(&KeywordAbility::Flash)
            || self.keywords.contains(&KeywordAbility::Indestructible)
            || self.colors.contains(&Color::C)
            || self.colors.iter().collect::<BTreeSet<_>>().len() != self.colors.len()
            || self.subtypes.is_empty()
            || self
                .subtypes
                .iter()
                .any(|s| s.is_empty() || !s.chars().all(|c| c.is_ascii_alphabetic() || c == '-'))
            || self.subtypes.iter().collect::<BTreeSet<_>>().len() != self.subtypes.len()
            || self
                .keywords
                .iter()
                .enumerate()
                .any(|(i, k)| self.keywords[..i].contains(k))
        {
            return Err("unsupported creature token characteristics".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "recipient", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterRecipient {
    Source,
    Target { target_slot: u32 },
}
/// Fixed signed power/toughness changes belonging to one complete creature effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatureStatModifier {
    pub power: Quantity,
    pub toughness: Quantity,
}
// Tagged operations are deliberate; unknown variants and fields reject.
// https://serde.rs/enum-representations.html
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Instruction {
    /// One Kernel IR node. `kir::check` is its whole admission decision and
    /// the core kernel interpreter its only executor.
    Kernel { node: kir::Node, source_block: u32 },
}
/// The parts of a board-wide damage instruction ([`Instruction::set_damage`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetDamage<'a> {
    pub sets: Vec<&'a SetExpr>,
    pub each_player: bool,
    pub amount: &'a kir::Amount,
    pub source_block: u32,
}
impl SetDamage<'_> {
    /// The board-wide damage instruction of these parts.
    pub fn instruction(
        sets: Vec<SetExpr>,
        each_player: bool,
        amount: kir::Amount,
        source_block: u32,
    ) -> Instruction {
        let mut parts: Vec<_> = sets
            .into_iter()
            .map(|set| kir::DamageTo::Objects { set })
            .collect();
        if each_player {
            parts.push(kir::DamageTo::Players {
                players: kir::PlayerExpr::EachPlayer,
            });
        }
        let to = if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            kir::DamageTo::All { parts }
        };
        Instruction::damage(to, amount, source_block)
    }
}
impl Instruction {
    /// One kernel verb as an instruction.
    pub fn act(verb: kir::Verb, source_block: u32) -> Self {
        Self::Kernel {
            node: kir::Node::Do(verb),
            source_block,
        }
    }
    /// CR 119.3: each opponent loses `amount`, then "you" in `context` gain
    /// the total life actually lost.
    pub fn drain(context: kir::Context, amount: kir::Amount, source_block: u32) -> Self {
        Self::Kernel {
            node: kir::Node::drain(context, amount),
            source_block,
        }
    }
    /// CR 121.1: `player` draws `count` cards.
    pub fn draw(player: kir::PlayerExpr, count: Quantity, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Draw {
                player,
                amount: kir::Amount::Fixed { value: count },
            },
            source_block,
        )
    }
    /// The player, count and source block of a fixed draw by one player.
    pub fn fixed_draw(&self) -> Option<(&kir::PlayerExpr, &Quantity, u32)> {
        match self {
            kernel!(Draw { player: player @ kir::PlayerExpr::Is { .. }, amount: fixed!(count) } @ block) => {
                Some((player, count, *block))
            }
            _ => None,
        }
    }
    /// CR 121.1: one card drawn by one player, the single draw.
    pub fn single_draw(&self) -> bool {
        self.fixed_draw()
            .is_some_and(|(_, count, _)| *count == Quantity::from(1))
    }
    /// A fixed draw that is not a counted controller draw: one card for the
    /// controller, or any fixed count for an announced or event player.
    pub fn uncounted_draw(&self) -> bool {
        self.fixed_draw()
            .is_some_and(|(player, count, _)| !player.is_you() || *count == Quantity::from(1))
    }
    /// Any fixed draw by the controller, one card included (CR 121.1-121.2).
    /// `Draw(you, Fixed(1))` is the one canonical single draw; families that
    /// also admit counts above one read it through this accessor.
    pub fn fixed_controller_draw(&self) -> Option<(&Quantity, u32)> {
        self.fixed_draw()
            .filter(|(player, ..)| player.is_you())
            .map(|(_, count, block)| (count, block))
    }
    /// A fixed controller draw of any count other than one: the counted
    /// controller draw of the hand-program families (CR 121.2).
    pub fn counted_controller_draw(&self) -> Option<(&Quantity, u32)> {
        self.fixed_draw()
            .filter(|(player, count, _)| player.is_you() && **count != Quantity::from(1))
            .map(|(_, count, block)| (count, block))
    }
    /// CR 111.1: "you" in `context` create `count` tokens of `token`.
    pub fn create_tokens(
        context: kir::Context,
        count: u32,
        token: kir::TokenSpec,
        source_block: u32,
    ) -> Self {
        Self::act(
            kir::Verb::CreateTokens {
                controller: kir::PlayerExpr::you(context),
                count: kir::Amount::fixed(i64::from(count)),
                token,
            },
            source_block,
        )
    }
    /// The recipe and count of a kernel token creation.
    pub fn created_tokens(&self) -> Option<(&kir::TokenSpec, u32)> {
        match self {
            kernel!(CreateTokens {
                count: fixed!(count),
                token,
                ..
            }) => Some((token, count.to_u32()?)),
            _ => None,
        }
    }
    /// The creature recipe of a kernel creature-token creation.
    pub fn creature_token(&self) -> Option<&CreatureToken> {
        match self.created_tokens()? {
            (kir::TokenSpec::Creature { spec }, _) => Some(spec),
            _ => None,
        }
    }
    /// CR 122.1: put `amount` +1/+1 counters on `on`.
    pub fn plus_one_counters(on: kir::SetExpr, amount: Quantity, source_block: u32) -> Self {
        Self::act(
            kir::Verb::PutCounters {
                on,
                counter: kir::CounterKind::PlusOnePlusOne,
                amount: kir::Amount::Fixed { value: amount },
            },
            source_block,
        )
    }
    /// CR 611.2c, 613.4c, 514.2: `what` gets +`power`/+`toughness` until end
    /// of turn.
    pub fn pump(
        what: kir::SetExpr,
        power: Quantity,
        toughness: Quantity,
        source_block: u32,
    ) -> Self {
        Self::act(
            kir::Verb::ModifyUntil {
                what,
                layer: kir::Layer::PowerToughnessModify,
                change: kir::Change::PowerToughness {
                    power: kir::Amount::Fixed { value: power },
                    toughness: kir::Amount::Fixed { value: toughness },
                },
                until: kir::Until::EndOfTurn,
            },
            source_block,
        )
    }
    /// The set, power and toughness of a kernel end-of-turn pump.
    pub fn pumped(&self) -> Option<(&kir::SetExpr, &Quantity, &Quantity, u32)> {
        match self {
            kernel!(ModifyUntil {
                what,
                change: kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) },
                until: kir::Until::EndOfTurn,
                ..
            } @ block) => Some((what, power, toughness, *block)),
            _ => None,
        }
    }
    /// CR 611.2c, 613.1f, 514.2: the source gains `keywords` until end of
    /// turn.
    pub fn grant_source_keywords(keywords: Vec<KeywordAbility>, source_block: u32) -> Self {
        Self::act(
            kir::Verb::ModifyUntil {
                what: kir::SetExpr::source(),
                layer: kir::Layer::Ability,
                change: kir::Change::AddKeywords { keywords },
                until: kir::Until::EndOfTurn,
            },
            source_block,
        )
    }
    /// The keywords the source grants itself until end of turn.
    pub fn source_keywords(&self) -> Option<&[KeywordAbility]> {
        match self {
            kernel!(ModifyUntil {
                what: source_object!(),
                change: kir::Change::AddKeywords { keywords },
                ..
            }) => Some(keywords),
            _ => None,
        }
    }
    /// CR 701.17a: `player` mills `amount` cards.
    pub fn mill(player: kir::PlayerExpr, amount: kir::Amount, source_block: u32) -> Self {
        Self::act(kir::Verb::Mill { player, amount }, source_block)
    }
    /// CR 701.9a-b: `player` discards `amount` cards, at random when `random`.
    pub fn discard(
        player: kir::PlayerExpr,
        amount: kir::Amount,
        random: bool,
        source_block: u32,
    ) -> Self {
        Self::act(
            kir::Verb::Discard {
                player,
                amount,
                random,
            },
            source_block,
        )
    }
    /// CR 701.21a: `player` sacrifices one creature.
    pub fn sacrifice_creature(player: kir::PlayerExpr, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Sacrifice {
                player,
                count: kir::Amount::fixed(1),
                filter: kir::Pred::creatures(),
            },
            source_block,
        )
    }
    /// The player, count and source block of a chosen fixed discard by one
    /// player: the controller, the damaged player or one announced player.
    pub fn chosen_discard(&self) -> Option<(&kir::PlayerExpr, &Quantity, u32)> {
        match self {
            kernel!(Discard { player: player @ kir::PlayerExpr::Is { .. }, amount: fixed!(count), random: false } @ block) => {
                Some((player, count, *block))
            }
            _ => None,
        }
    }
    /// The count of the controller's own fixed discard.
    pub fn controller_discard(&self) -> Option<&Quantity> {
        self.chosen_discard()
            .filter(|(player, ..)| player.is_you())
            .map(|(_, count, _)| count)
    }
    /// CR 701.8: destroy the target of `slot` (CR 701.19c when
    /// `regeneration_forbidden`).
    pub fn destroy_target(slot: u32, regeneration_forbidden: bool, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Destroy {
                what: kir::SetExpr::target(slot),
                regeneration_forbidden,
            },
            source_block,
        )
    }
    /// CR 608.2d: "you may destroy" the target of `slot`.
    pub fn optional_destroy_target(context: kir::Context, slot: u32, source_block: u32) -> Self {
        Self::Kernel {
            node: kir::Node::Optional {
                chooser: kir::PlayerExpr::you(context),
                node: Box::new(kir::Node::Do(kir::Verb::Destroy {
                    what: kir::SetExpr::target(slot),
                    regeneration_forbidden: false,
                })),
            },
            source_block,
        }
    }
    /// The slot, optionality, regeneration prohibition and block of a destroy
    /// of one announced target.
    pub fn target_destroy(&self) -> Option<(u32, bool, bool, u32)> {
        let Self::Kernel { node, source_block } = self else {
            return None;
        };
        if let Some((_, slot, regeneration_forbidden)) = node.optional_target_destroy() {
            return Some((slot, true, regeneration_forbidden, *source_block));
        }
        match node {
            kir::Node::Do(kir::Verb::Destroy {
                what: target_object!(slot),
                regeneration_forbidden,
            }) => Some((*slot, false, *regeneration_forbidden, *source_block)),
            _ => None,
        }
    }
    /// CR 400.3: move the target of `slot` to `to`.
    pub fn move_target(slot: u32, to: kir::Destination, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Move {
                what: kir::SetExpr::target(slot),
                to,
            },
            source_block,
        )
    }
    /// CR 701.26a: tap the target of `slot`.
    pub fn tap_target(slot: u32, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Tap {
                what: kir::SetExpr::target(slot),
            },
            source_block,
        )
    }
    /// CR 701.26b: untap the target of `slot`.
    pub fn untap_target(slot: u32, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Untap {
                what: kir::SetExpr::target(slot),
            },
            source_block,
        )
    }
    /// CR 701.26b: untap every land "you" in `context` control.
    pub fn untap_lands_you_control(context: kir::Context, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Untap {
                what: kir::SetExpr::lands_you_control(context),
            },
            source_block,
        )
    }
    /// CR 404.1: exile the graveyard of the target player of `slot`.
    pub fn exile_target_graveyard(slot: u32, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Move {
                what: kir::SetExpr::graveyard_of(kir::PlayerExpr::target(slot)),
                to: kir::Destination::Exile,
            },
            source_block,
        )
    }
    /// CR 701.8: destroy every creature.
    pub fn destroy_all_creatures(source_block: u32) -> Self {
        Self::act(
            kir::Verb::Destroy {
                what: kir::SetExpr::battlefield(kir::Pred::creatures()),
                regeneration_forbidden: false,
            },
            source_block,
        )
    }
    /// CR 120.2b: the resolving object's source deals `amount` damage `to`.
    pub fn damage(to: kir::DamageTo, amount: kir::Amount, source_block: u32) -> Self {
        Self::act(
            kir::Verb::Damage {
                source: kir::Ref::Source,
                to,
                amount,
            },
            source_block,
        )
    }
    /// CR 120.2b: damage to the target of `slot`.
    pub fn damage_target(slot: u32, amount: kir::Amount, source_block: u32) -> Self {
        Self::damage(
            kir::DamageTo::Ref {
                who: kir::Ref::Target { slot },
            },
            amount,
            source_block,
        )
    }
    /// The slot, amount and block of damage to one announced target.
    pub fn directed_damage(&self) -> Option<(u32, &kir::Amount, u32)> {
        match self {
            kernel!(Damage {
                to: kir::DamageTo::Ref {
                    who: kir::Ref::Target { slot },
                },
                amount,
                ..
            } @ block) => Some((*slot, amount, *block)),
            _ => None,
        }
    }
    /// CR 608.2h: damage to every member of board-wide creature sets and,
    /// optionally, each player.
    pub fn set_damage(&self) -> Option<SetDamage<'_>> {
        let kernel!(Damage { to, amount, .. } @ source_block) = self else {
            return None;
        };
        let parts = match to {
            kir::DamageTo::Ref { .. } => return None,
            kir::DamageTo::All { parts } => parts.as_slice(),
            part => std::slice::from_ref(part),
        };
        let mut damage = SetDamage {
            sets: vec![],
            each_player: false,
            amount,
            source_block: *source_block,
        };
        for part in parts {
            match part {
                kir::DamageTo::Objects { set } => damage.sets.push(set),
                kir::DamageTo::Players {
                    players: kir::PlayerExpr::EachPlayer,
                } => damage.each_player = true,
                _ => return None,
            }
        }
        Some(damage)
    }
    /// CR 107.3, 119.3: `player` gains or loses the announced X.
    pub fn announced_x_life(player: kir::PlayerExpr, gain: bool, source_block: u32) -> Self {
        let amount = kir::Amount::announced_x();
        Self::act(
            if gain {
                kir::Verb::GainLife { player, amount }
            } else {
                kir::Verb::LoseLife { player, amount }
            },
            source_block,
        )
    }
    /// CR 119.3: the amount of an opponent drain pair ([`kir::drain`]).
    pub fn drain_amount(&self) -> Option<&kir::Amount> {
        match self {
            Self::Kernel { node, .. } => kir::drain(node),
        }
    }
    /// The verb of a single-verb kernel instruction.
    pub fn verb(&self) -> Option<&kir::Verb> {
        match self {
            Self::Kernel { node, .. } => node.verb(),
        }
    }
    pub fn verb_mut(&mut self) -> Option<&mut kir::Verb> {
        match self {
            Self::Kernel {
                node: kir::Node::Do(verb),
                ..
            } => Some(verb),
            _ => None,
        }
    }
    pub fn source_block(&self) -> u32 {
        let Self::Kernel { source_block, .. } = self;
        *source_block
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
pub enum SacrificeDamageAmount {
    Fixed { value: Quantity },
    SacrificedPower,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SacrificeFilter {
    /// A union of permanent types; empty means any permanent type.
    pub types: Vec<CardKind>,
    pub subtypes: Vec<String>,
}
impl SacrificeFilter {
    pub fn validate(&self) -> Result<(), String> {
        if (self.types.is_empty() && self.subtypes.is_empty())
            || self
                .types
                .iter()
                .any(|k| !matches!(k, CardKind::Creature | CardKind::Artifact))
            || self
                .types
                .iter()
                .enumerate()
                .any(|(n, k)| self.types[..n].contains(k))
            || self.subtypes.iter().any(|s| s != "Goblin")
            || self.subtypes.len() > 1
        {
            return Err("unsupported sacrifice permanent filter".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeathFilter {
    pub controlled: bool,
    pub other: bool,
    pub non_token: bool,
    pub non_subtypes: Vec<String>,
}
/// Fixed-turn effects expire in the atomic cleanup action (CR 514.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Duration {
    UntilEndOfTurn,
}
/// CR 614.1a/614.5: positive placement quantities only; removal is a cost.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterPlacementModification {
    Add { amount: Quantity },
    Multiply { factor: Quantity },
}
impl CounterPlacementModification {
    pub fn validate(&self) -> Result<(), String> {
        let value = match self {
            Self::Add { amount } => amount,
            Self::Multiply { factor } => factor,
        };
        if *value <= Quantity::zero() {
            return Err("counter placement parameter must be positive".into());
        }
        Ok(())
    }
    pub fn apply(&self, value: &Quantity) -> Quantity {
        let mut result = value.clone();
        match self {
            Self::Add { amount } => result.add(amount),
            Self::Multiply { factor } => result.multiply(factor),
        }
        result
    }
}
/// Disjoint predicates on a completed spell; history is global to its caster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "eligibility", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpellCascadeEligibility {
    ManaValueAtLeast { minimum: Quantity },
    FirstCastEachTurn {},
    FirstCastFromExileEachTurn {},
}
impl SpellCascadeEligibility {
    pub fn requires_history(&self) -> bool {
        !matches!(self, Self::ManaValueAtLeast { .. })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandSizeScope {
    Controller,
    Opponents,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StaticEffect {
    AdditionalLandPlay,
    PlayLandsFromOwnGraveyard,
    KeepUnspentManaAsBlack,
    Bestow {
        cost: ManaCost,
        program: SpellProgram,
    },
    GrantControllerLandsAnyColorMana,
    MayBeginOnBattlefield,
    GrantControllerLandsAllBasicTypes,
    ControllerNonlandsAllColors,
    /// CR 109.5/113.10/601.2i/611.3/613.1f/702.85: each vector slot
    /// grants one independent cascade to this controller's qualifying cast.
    /// The program is exactly canonical_cascade_program, never arbitrary code.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    GrantSpellCascade {
        eligibility: SpellCascadeEligibility,
        program: SpellProgram,
    },
    /// CR 109.5/110.2a/608.2d/702.85b: this battlefield source's controller
    /// may put one of their resolving cascade's still-exiled lands onto the
    /// battlefield tapped before the cascade cast decision.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    CascadeLandEntry,
    /// CR 400.7, 614.12a, 707.2, 707.5–6: before entry, the entering
    /// controller may choose a current battlefield incarnation that entered
    /// this turn and apply its copiable values to the entering object.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    EnterAsCopyOfPermanentEnteredThisTurn,
    /// CR 109.5/303.4m/611.3/613.1b: the current source controller controls
    /// its enchanted permanent; this is not a frozen invocation recipient.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    ControlEnchantedPermanent,
    /// CR 101.2/109.5/601.3: opponents cannot cast during this controller's turn.
    OpponentsCantCastDuringControllerTurn,
    /// CR 109.5, 611.3, 613.11, 514.1: the battlefield source's current
    /// controller has no maximum hand size.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    UnlimitedControllerHandSize,
    /// CR 109.5, 611.3, 613.11, 514.1: subtract from the maximum hand size
    /// of the current controller or that controller's opponents.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    ReduceMaximumHandSize {
        scope: HandSizeScope,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
    },
    /// CR 109.5, 611.3, 613.11, 514.1: add to the maximum hand size
    /// of the current controller of this battlefield source.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    IncreaseMaximumHandSize {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
    },
    EntersTapped,
    /// CR 109.5/205.4c/614.12: count prior basic lands of the entering controller.
    EntersTappedUnlessControllerBasicLands {
        minimum: Quantity,
    },
    /// CR 614.12: compare the entering controller's other lands in the common prior state.
    EntersTappedUnlessControllerOtherLands {
        comparison: LandCountComparison,
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        threshold: Quantity,
    },
    EntersTappedUnlessOpponentCount {
        minimum: Quantity,
    },
    EntersTappedUnlessRevealLandTypes {
        subtypes: [BasicLandType; 2],
    },
    EntersTappedUnlessControllerLandType {
        subtype: BasicLandType,
    },
    EntersTappedUnlessControllerLandTypes {
        subtypes: [BasicLandType; 2],
    },
    /// CR 118.4/614.12: intended controller may pay this exact entry life cost.
    EntersTappedUnlessPayLife {
        amount: Quantity,
    },
    ReduceGenericByCreatureCount,
    ReduceSelfGenericCost {
        per_object: Quantity,
        count: BattlefieldCount,
    },
    /// CR 101.2/113.6: the source spell on the stack can't be countered.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    CantBeCountered,
    /// CR 113.6b/614.1a/701.24c: replace a move of this card from any zone to
    /// a graveyard by revealing it and shuffling it into its owner's library.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    SelfGraveyardRevealShuffle,
    /// CR 109.5/611.3/613.1f/702.12: the current controller's other
    /// battlefield permanents have indestructible while this source is present.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    GrantOtherControlledPermanentsIndestructible,
    ModifyCreatures {
        scope: CreatureScope,
        power: Quantity,
        toughness: Quantity,
        keywords: Vec<KeywordAbility>,
        restrictions: Vec<CreatureRestriction>,
    },
    ModifySourceByDistinctPermanentColors {
        scope: PermanentColorScope,
        power_per_color: Quantity,
        toughness_per_color: Quantity,
    },
    ControllerCreatureAnthem {
        power: Quantity,
        toughness: Quantity,
    },
    /// A continuously reselected controller-side layer 7c modifier.
    ControllerScopedCreatureStats {
        filter: ControllerCreatureStatFilter,
        other: bool,
        power: Quantity,
        toughness: Quantity,
    },
    ControllerCreaturePlusOneCounterReplacement {
        modification: CounterPlacementModification,
    },
    DoubleDamageToPermanentOrPlayer,
    /// Double damage from creatures controlled by this battlefield source's controller.
    DoubleControllerCreatureDamageToPermanentOrPlayer,
}
/// Restrictions evaluated on the affected creature's earlier-layer characteristics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "selector", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControllerCreatureStatFilter {
    Any,
    Artifact,
    Subtypes { subtypes: Vec<String> },
}

/// Inclusive comparison for a common-prior land count (CR 614.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LandCountComparison {
    AtLeast,
    AtMost,
}

/// Count current battlefield types and control, never ownership (CR 702.41).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BattlefieldCount {
    pub kind: CardKind,
    pub controller: CountController,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CountController {
    Any,
    Caster,
}
/// CR 109.5/110.1: battlefield permanents controlled by the static source's
/// current controller or by the player who activated the mana ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermanentColorScope {
    Controller,
}
impl StaticEffect {
    /// Preserve the canonical legacy IR while sharing one arithmetic path.
    pub fn counted_generic_reduction(&self) -> Option<(Quantity, BattlefieldCount)> {
        match self {
            Self::ReduceGenericByCreatureCount => Some((
                1.into(),
                BattlefieldCount {
                    kind: CardKind::Creature,
                    controller: CountController::Any,
                },
            )),
            Self::ReduceSelfGenericCost { per_object, count } => {
                Some((per_object.clone(), count.clone()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaticAbility {
    pub functioning_zone: FunctioningZone,
    pub effect: StaticEffect,
}
/// Static scopes in the finite attachment pool (CR 611.3a, 613.1f/4c).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CreatureScope {
    Equipped,
    EquippedLegendary,
    Enchanted,
    Controller,
    /// Dynamic attacking creatures controlled by this battlefield source.
    AttackingController,
    ControlledSubtype {
        subtype: scoped_static::Subtype,
        creatures_only: bool,
        other: bool,
    },
    Source,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CreatureRestriction {
    CantBlock,
    CantBeBlocked,
    CantAttackOrBlock,
    CanBlockOnlyFlying,
}
/// Supported announcement timing; permissions are checked by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpellTiming {
    Instant,
    MainPhase,
}
/// Permanent resolution is distinct from an empty instruction program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpellResolution {
    ChooseModes { exactly: u32 },
    ChooseRepeatedModes { exactly: u32 },
    OneOrMoreModes,
    Instructions,
    Permanent,
}
/// CR 702.33a/e: one source-bound optional fixed mana declaration. The
/// identity names the exact archived keyword; conditions cannot link by position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OptionalAdditionalManaCost {
    pub identity: String,
    pub source_block: u32,
    pub mana: ManaCost,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpellProgram {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_additional_costs: Vec<OptionalAdditionalManaCost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_sacrifice: Option<SacrificeFilter>,
    pub timing: SpellTiming,
    pub resolution: SpellResolution,
    pub functioning_zone: FunctioningZone,
    pub targets: Vec<TargetSpec>,
    pub instructions: Vec<Instruction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<SpellMode>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpellMode {
    pub program: SpellProgram,
}
fn single_sacrifice() -> u32 {
    1
}
fn single_sacrifice_count(n: &u32) -> bool {
    *n == 1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ActivationCost {
    /// Fixed mana and this exact hand source, committed as one complete cost.
    DiscardSource {
        mana: ManaCost,
    },
    SacrificePermanent {
        #[serde(
            default = "single_sacrifice",
            skip_serializing_if = "single_sacrifice_count"
        )]
        #[schemars(range(min = 1))]
        count: u32,
        filter: SacrificeFilter,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mana: Option<ManaCost>,
    },
    SacrificeSource,
    TapSource,
    /// One complete nonmana cost; life payment is loss of life, never damage.
    TapPayLifeSacrificeSource {
        life: Quantity,
    },
    Mana {
        cost: ManaCost,
    },
    Composite {
        costs: PermanentActivationCosts,
    },
}
/// Finite permanent costs; costs are committed only after complete payment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermanentActivationCosts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove_source_plus_one_counters: Option<Quantity>,
    pub tap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<ManaCost>,
    pub sacrifice: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mana", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaProduction {
    RestrictedFixed {
        symbols: Vec<Color>,
        spend: ManaSpendRestriction,
    },
    FixedWithLifeGain {
        symbols: Vec<Color>,
        life: Quantity,
    },
    HybridFilter {
        colors: [Color; 2],
    },
    GreatestControlledPowerCombination,
    Fixed {
        symbols: Vec<Color>,
    },
    AnyColor,
    AnyOneColor {
        amount: SingleColorManaAmount,
    },
    CommanderColor,
    /// Add one mana of a color an opponent's land could produce.
    OpponentLandColors,
    /// Add one mana of every distinct color in the selected battlefield set.
    EachPermanentColor {
        scope: PermanentColorScope,
    },
    /// Add one mana of the selected color.
    Choice {
        colors: Vec<Color>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SingleColorManaAmount {
    Fixed { quantity: Quantity },
    ControllerEnchantments,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManaAbility {
    /// Damage dealt by the source to the activating player after producing mana.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_damage: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restriction: Option<ManaActivationRestriction>,
    pub costs: PermanentActivationCosts,
    pub production: ManaProduction,
    pub source_block: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ManaActivationRestriction {
    OnceEachTurn,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivatedAbility {
    pub functioning_zone: FunctioningZone,
    pub cost: ActivationCost,
    pub program: SpellProgram,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CascadeThreshold {
    SourceSpellManaValue,
}
/// Mandatory ordinary event patterns. ETB patterns inspect the event's
/// resulting battlefield, including the entering source itself (CR 603.6a).
/// Dies inspects the battlefield immediately before the actual graveyard move
/// (CR 603.10/700.4), including a source that dies simultaneously.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TriggerCondition {
    ControlledLandEntersBattlefield,
    DelayedControllerCreatureEntry,
    DelayedControllerSpellCast,
    SagaChapter {
        number: std::num::NonZeroU32,
    },
    /// CR 702.85a and 702.144a: functions on the casting spell itself, on the stack.
    SelfSpellCast,
    /// One occurrence per damaged player and simultaneous combat-damage event.
    GroupedDamageDealt {
        source: DamageTriggerSource,
        kind: TriggerDamageKind,
        recipient: DamageTriggerRecipient,
        source_power: DamageSourcePowerPredicate,
    },
    DamageDealt {
        source: DamageTriggerSource,
        kind: TriggerDamageKind,
        recipient: DamageTriggerRecipient,
    },
    CreatureAttacked {
        attacker: AttackSubject,
    },
    SpellCast {
        caster: CasterRelation,
        #[schemars(schema_with = "cast_spell_predicate_schema")]
        spell: TypePredicate,
    },
    SpellCastWithOrigin {
        caster: CasterRelation,
        spell: TypePredicate,
        origin: CastOriginPredicate,
    },
    StepBegan {
        step: BeginningStep,
        turn_relation: TurnRelation,
    },
    OpponentCreatureDies,
    SelfEntersBattlefield,
    EquippedCombatDamage,
    EquippedDies,
    /// The exact source incarnation actually changes from untapped to tapped.
    SelfBecomesTapped,
    ManaTapped {
        filter: ManaTapFilter,
    },
    /// Excludes the exact source incarnation, never every copy of its card.
    OtherCreatureEntersBattlefield,
    /// CR 603.6a/603.10: read the entering creature's derived battlefield power
    /// and controller immediately after the completed entry event.
    ControlledCreatureEntersWithPower {
        minimum_power: Quantity,
        other: bool,
    },
    /// The disjoint union of this creature and all other creatures dying.
    AnyCreatureDies,
    SelfDies,
    SelfAuraToGraveyard,
    SelfMoved {
        from: SourceMoveOrigin,
        to: TriggerSuccessorZone,
    },
}
/// Candidate-local event-time comparison; no printed-source pronoun binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DamageSourcePowerPredicate {
    GreaterThanBase,
}
impl<'de> Deserialize<'de> for DamageSourcePowerPredicate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "greater_than_base" => Ok(Self::GreaterThanBase),
            _ => Err(serde::de::Error::custom(
                "unsupported damage source power predicate",
            )),
        }
    }
}
/// CR 120.2/120.7/603.2c: independent source, kind and recipient dimensions.
/// Source means the observer's exact incarnation; CreatureFilter separately
/// matches actual creature damage sources and requires FilteredDamageTriggerV1.
/// DamageTriggerV1 requires combat/player; V2 permits any kind or opponents.
/// These predicates alone supply no event-player or event-quantity binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "subject", rename_all = "snake_case", deny_unknown_fields)]
pub enum DamageTriggerSource {
    Source {},
    /// A creature dealing damage, with additional type membership and a
    /// controller relation to the observing ability's controller (CR 603.10).
    CreatureFilter {
        controller: DamageSourceRelation,
        types: TypePredicate,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DamageSourceRelation {
    Controller,
}
impl<'de> Deserialize<'de> for DamageSourceRelation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "controller" => Ok(Self::Controller),
            _ => Err(serde::de::Error::custom(
                "unsupported damage source relation",
            )),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TriggerDamageKind {
    Combat,
    Any,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DamageTriggerRecipient {
    Player,
    OpponentPlayer,
}
/// Ordinary per-creature declaration predicates (CR 508.3a/603.2c).
/// Controller relation and full type membership are independent of ownership.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "subject", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttackSubject {
    Source {},
    Filter {
        controller: AttackerRelation,
        types: TypePredicate,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttackerRelation {
    Controller,
    AnyPlayer,
    Opponent,
}
/// CR 603.3a: caster relation is independent of the source's controller/owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CasterRelation {
    Controller,
    AnyPlayer,
    Opponent,
}
/// CR 601.2a: compare the zone the spell left during announcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "predicate", rename_all = "snake_case", deny_unknown_fields)]
pub enum CastOriginPredicate {
    Outside { zone: CastOriginZone },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CastOriginZone {
    ControllerHand,
}
impl<'de> Deserialize<'de> for CastOriginZone {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "controller_hand" => Ok(Self::ControllerHand),
            _ => Err(serde::de::Error::custom("unsupported cast origin zone")),
        }
    }
}
/// Source ObjectFilter fields are conjunctive; explicit anyOf retains OR.
/// Type membership includes additional types. Spell color uses frozen characteristics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "predicate", rename_all = "snake_case", deny_unknown_fields)]
pub enum TypePredicate {
    #[schemars(skip)]
    SpellColor {
        color: Color,
    },
    // An empty struct also enforces serde unknown-field rejection here.
    Any {},
    Type {
        kind: CardKind,
    },
    NotType {
        kind: CardKind,
    },
    Subtype {
        #[schemars(regex(pattern = "^[A-Z][A-Za-z -]{0,63}$"))]
        subtype: String,
    },
    All {
        #[schemars(length(min = 1, max = 32))]
        terms: Vec<TypePredicate>,
    },
    AnyOf {
        #[schemars(length(min = 1, max = 32))]
        terms: Vec<TypePredicate>,
    },
}
fn cast_spell_predicate_schema(generator: &mut SchemaGenerator) -> Schema {
    json_schema!({"oneOf":[generator.subschema_for::<TypePredicate>(), {
        "type":"object", "required":["predicate","color"], "additionalProperties":false,
        "properties":{"predicate":{"const":"spell_color"}, "color":{"enum":["W","U","B","R","G"]}}
    }]})
}

impl TypePredicate {
    pub fn matches(&self, c: &Characteristics) -> bool {
        match self {
            Self::Any {} => true,
            Self::SpellColor { color } => c.colors.contains(color),
            Self::Type { kind } => c.has_type(*kind),
            Self::NotType { kind } => !c.has_type(*kind),
            Self::Subtype { subtype } => c.subtypes.contains(subtype),
            Self::All { terms } => terms.iter().all(|t| t.matches(c)),
            Self::AnyOf { terms } => terms.iter().any(|t| t.matches(c)),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.validate_type_shape(false)
    }

    fn validate_type_shape(&self, cast: bool) -> Result<(), String> {
        fn visit(p: &TypePredicate, depth: usize, remaining: &mut usize, cast: bool) -> bool {
            if depth > 8 || *remaining == 0 {
                return false;
            }
            *remaining -= 1;
            match p {
                TypePredicate::SpellColor { .. } => false,
                TypePredicate::Subtype { subtype } => {
                    cast && cast_subtype::subtype_spelling(subtype)
                }
                TypePredicate::All { terms } | TypePredicate::AnyOf { terms } => {
                    !terms.is_empty() && terms.iter().all(|t| visit(t, depth + 1, remaining, cast))
                }
                _ => true,
            }
        }
        if visit(self, 0, &mut 32, cast) {
            Ok(())
        } else {
            Err("type predicate structural bound".into())
        }
    }
}
/// Exact step predicates; phases, delayed and conditional boundaries are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BeginningStep {
    PrecombatMain,
    Upkeep,
    Draw,
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnRelation {
    Controller,
    AnyPlayer,
    Opponent,
}
impl TriggerCondition {
    /// Stable reporting vocabulary, independent of the serde representation.
    pub fn reporting_key(&self) -> &'static str {
        match self {
            Self::StepBegan { .. } => "step_began",
            Self::CreatureAttacked { .. } => "creature_attacked",
            Self::DamageDealt { .. } => "damage_dealt",
            Self::GroupedDamageDealt { .. } => "grouped_damage_dealt",
            Self::DelayedControllerCreatureEntry => "delayed_controller_creature_entry",
            Self::DelayedControllerSpellCast => "delayed_controller_spell_cast",
            Self::SagaChapter { .. } => "saga_chapter",
            Self::SelfSpellCast => "self_spell_cast",
            Self::SpellCast { .. } | Self::SpellCastWithOrigin { .. } => "spell_cast",
            Self::OpponentCreatureDies => "opponent_creature_dies",
            Self::SelfEntersBattlefield => "self_enters_battlefield",
            Self::EquippedCombatDamage => "equipped_combat_damage",
            Self::EquippedDies => "equipped_dies",
            Self::OtherCreatureEntersBattlefield => "other_creature_enters_battlefield",
            Self::ControlledCreatureEntersWithPower { .. } => {
                "controlled_creature_enters_with_power"
            }
            Self::AnyCreatureDies => "any_creature_dies",
            Self::SelfDies => "self_dies",
            Self::SelfBecomesTapped => "self_becomes_tapped",
            Self::ManaTapped { .. } => "mana_tapped",
            Self::SelfAuraToGraveyard => "self_aura_to_graveyard",
            Self::SelfMoved { .. } => "self_moved",
            Self::ControlledLandEntersBattlefield => "controlled_land_enters_battlefield",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum InterveningCondition {
    ControllerHandEmpty,
    SourceCastByController,
    SourceStatus { status: TapStatus },
    ControllerHasCreatureWithPower { minimum_power: Quantity },
}
/// CR 110.5: only battlefield permanents have tapped/untapped status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TapStatus {
    Tapped,
    Untapped,
}
impl TapStatus {
    pub fn matches(self, tapped: bool) -> bool {
        tapped == matches!(self, Self::Tapped)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TriggeredAbility {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_filter: Option<DeathFilter>,
    pub functioning_zone: FunctioningZone,
    pub condition: TriggerCondition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intervening_if: Option<InterveningCondition>,
    /// This mandatory stack program is placed by the rules, never cast or
    /// activated. Its existing Instant shape carries no timing permission.
    pub program: SpellProgram,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceProvenance {
    pub printing_id: String,
    pub source_index: u32,
    pub source_sha256: String,
    pub ast_sha256: String,
    pub binding_sha256: String,
    pub archive_manifest_sha256: String,
    pub parser_sha256: String,
    /// Exact compact archive subvalues, not executable text.
    pub source_json: String,
    pub abilities_json: String,
}
/// CR702.3: defender is a static prohibition on declaring this creature as
/// an attacker. Power and continuous-control eligibility do not override it.
/// Exact normalized source blocks and compiler index bindings, never executable text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceLabels {
    pub text: String,
    pub blocks: Vec<String>,
    pub activated: Vec<u32>,
    pub triggered: Vec<u32>,
    pub mana: Vec<u32>,
    #[serde(default)]
    pub statics: Vec<u32>,
    pub spell_modes: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeywordAbility {
    Flash,
    Hexproof,
    Shroud,
    Defender,
    Flying,
    Reach,
    Vigilance,
    Haste,
    Lifelink,
    Deathtouch,
    Indestructible,
    Trample,
    #[serde(rename = "first strike")]
    FirstStrike,
    #[serde(rename = "double strike")]
    DoubleStrike,
    Menace,
    Plainswalk,
    Islandwalk,
    Swampwalk,
    Mountainwalk,
    Forestwalk,
    ProtectionFromWhite,
    ProtectionFromBlue,
    ProtectionFromBlack,
    ProtectionFromRed,
    ProtectionFromGreen,
    ProtectionFromEachColor,
    // CR 702.16j: intrinsic battlefield protection from every source.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    ProtectionFromEverything,
}
impl<'de> Deserialize<'de> for KeywordAbility {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // The executable schema publishes string keywords. Do not accept serde's
        // alternative externally-tagged unit-map spelling as an IR vocabulary.
        match String::deserialize(d)?.as_str() {
            "flash" => Ok(Self::Flash),
            "defender" => Ok(Self::Defender),
            "flying" => Ok(Self::Flying),
            "reach" => Ok(Self::Reach),
            "vigilance" => Ok(Self::Vigilance),
            "haste" => Ok(Self::Haste),
            "lifelink" => Ok(Self::Lifelink),
            "deathtouch" => Ok(Self::Deathtouch),
            "indestructible" => Ok(Self::Indestructible),
            "trample" => Ok(Self::Trample),
            "first strike" => Ok(Self::FirstStrike),
            "double strike" => Ok(Self::DoubleStrike),
            "menace" => Ok(Self::Menace),
            "plainswalk" => Ok(Self::Plainswalk),
            "islandwalk" => Ok(Self::Islandwalk),
            "swampwalk" => Ok(Self::Swampwalk),
            "mountainwalk" => Ok(Self::Mountainwalk),
            "forestwalk" => Ok(Self::Forestwalk),
            "protection_from_white" => Ok(Self::ProtectionFromWhite),
            "protection_from_blue" => Ok(Self::ProtectionFromBlue),
            "protection_from_black" => Ok(Self::ProtectionFromBlack),
            "protection_from_red" => Ok(Self::ProtectionFromRed),
            "protection_from_green" => Ok(Self::ProtectionFromGreen),
            "protection_from_each_color" => Ok(Self::ProtectionFromEachColor),
            "protection_from_everything" => Ok(Self::ProtectionFromEverything),
            "hexproof" => Ok(Self::Hexproof),
            "shroud" => Ok(Self::Shroud),

            other => Err(de::Error::unknown_variant(
                other,
                &[
                    "flash",
                    "defender",
                    "flying",
                    "reach",
                    "vigilance",
                    "haste",
                    "lifelink",
                    "deathtouch",
                    "indestructible",
                    "trample",
                    "first strike",
                    "double strike",
                    "menace",
                    "plainswalk",
                    "islandwalk",
                    "swampwalk",
                    "mountainwalk",
                    "forestwalk",
                    "protection_from_white",
                    "protection_from_blue",
                    "protection_from_black",
                    "protection_from_red",
                    "protection_from_green",
                    "protection_from_each_color",
                    "protection_from_everything",
                    "hexproof",
                    "shroud",
                ],
            )),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal_land: Option<Box<modal_land::ModalLand>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_faces: Option<Box<transform_face::TransformFaces>>,
    /// Offline compiler-owned labels; absent on historical IR and synthetic definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_labels: Option<SourceLabels>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub x_mana_cost: bool,
    pub oracle_id: String,
    pub name: String,
    pub characteristics: Characteristics,
    pub intrinsic_zone: Option<FunctioningZone>,
    pub spell: Option<SpellProgram>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activated: Vec<ActivatedAbility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mana_abilities: Vec<ManaAbility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggered: Vec<TriggeredAbility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub static_abilities: Vec<StaticAbility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<KeywordAbility>,
    pub capabilities: BTreeSet<Capability>,
    pub provenance: SourceProvenance,
    /// True only when `composition` admitted every ability of this definition.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub composed: bool,
}
// Shared live creature-modifier boundary, including attachments. Keep family
// recognition and Definition validation on the same keyword/restriction rules.
// CR 611.3a-c: Controller modifiers use live source presence/current control
// for both supported creature and non-Aura enchantment sources.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
fn static_creature_modification_valid(
    kind: CardKind,
    equipment: bool,
    aura: bool,
    effect: &StaticEffect,
) -> bool {
    let StaticEffect::ModifyCreatures {
        scope,
        power,
        toughness,
        keywords,
        restrictions,
    } = effect
    else {
        return false;
    };
    let attached = matches!(scope, CreatureScope::Equipped | CreatureScope::Enchanted);
    attachment_modifiers::scope_valid(effect, equipment)
        && !(!attached && (power.is_negative() || toughness.is_negative())
            || (power == &Quantity::zero()
                && toughness == &Quantity::zero()
                && keywords.is_empty()
                && restrictions.is_empty())
            || (*scope == CreatureScope::Source
                && (kind != CardKind::Creature
                    || *power != Quantity::zero()
                    || *toughness != Quantity::zero()
                    || !keywords.is_empty()
                    || (restrictions != &[CreatureRestriction::CantBlock]
                        && source_combat_restrictions::restriction(effect).is_none())))
            || (*scope != CreatureScope::Source
                && restrictions.contains(&CreatureRestriction::CanBlockOnlyFlying))
            || (*scope == CreatureScope::AttackingController
                && (!matches!(
                    kind,
                    CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
                ) || *power != Quantity::zero()
                    || *toughness != Quantity::zero()
                    || keywords != &[KeywordAbility::Deathtouch]
                    || !restrictions.is_empty()))
            || (*scope == CreatureScope::Equipped && !equipment)
            || (*scope == CreatureScope::Enchanted && !aura)
            || (*scope == CreatureScope::Controller
                && (!matches!(kind, CardKind::Creature | CardKind::Enchantment) || aura))
            || keywords.contains(&KeywordAbility::ProtectionFromEachColor)
            || keywords.contains(&KeywordAbility::ProtectionFromEverything)
            || keywords.iter().any(|k| {
                k.basic_landwalk_type().is_some()
                    || (!attached && k.fixed_protection_color().is_some())
            })
            || keywords.contains(&KeywordAbility::Flash)
            || (keywords.contains(&KeywordAbility::Indestructible)
                && (!equipment
                    || *scope != CreatureScope::Equipped
                    || *power != Quantity::zero()
                    || *toughness != Quantity::zero()
                    || keywords != &[KeywordAbility::Indestructible]
                    || !restrictions.is_empty()))
            || keywords
                .iter()
                .enumerate()
                .any(|(n, k)| keywords[..n].contains(k))
            || restrictions
                .iter()
                .enumerate()
                .any(|(n, k)| restrictions[..n].contains(k)))
}

/// Nonserialized target domain of a complete admitted Aura-control definition.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AuraControlDomain {
    Creature,
    Artifact,
    Land,
    Permanent,
}
impl AuraControlDomain {
    /// CR 110.4: only permanent kinds represented by this engine are admitted.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub fn accepts(self, characteristics: &Characteristics) -> bool {
        match self {
            Self::Creature => characteristics.has_type(CardKind::Creature),
            Self::Artifact => characteristics.has_type(CardKind::Artifact),
            Self::Land => characteristics.has_type(CardKind::Land),
            Self::Permanent => [
                CardKind::Creature,
                CardKind::Artifact,
                CardKind::Land,
                CardKind::Enchantment,
            ]
            .into_iter()
            .any(|kind| characteristics.has_type(kind)),
        }
    }
}

/// Complete Aura-control shape, without recursive validation, capabilities or
/// source identity dispatch. Only creature control composes with a modifier.
/// CR 303.4a/m, 109.5, 611.3; explicit filters preserve target validation.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub fn aura_control_domain(definition: &Definition) -> Option<AuraControlDomain> {
    let c = &definition.characteristics;
    let p = definition.spell.as_ref()?;
    let [target] = p.targets.as_slice() else {
        return None;
    };
    let target_type = match (&target.domain, &target.filter) {
        (TargetDomain::BattlefieldCreature, None) => AuraControlDomain::Creature,
        (TargetDomain::BattlefieldPermanent, Some(f))
            if matches!(
                f.types.as_slice(),
                [] | [CardKind::Artifact] | [CardKind::Land]
            ) && f.non_types.is_empty()
                && f.non_colors.is_empty()
                && !f.opponent
                && f.power.is_none()
                && !f.tapped =>
        {
            match f.types.as_slice() {
                [] => AuraControlDomain::Permanent,
                [CardKind::Artifact] => AuraControlDomain::Artifact,
                [CardKind::Land] => AuraControlDomain::Land,
                _ => unreachable!(),
            }
        }
        _ => return None,
    };
    let valid = c.kind == CardKind::Enchantment
        && c.additional_types.is_empty()
        && c.subtypes == ["Aura"]
        && !c.basic
        && !c.legendary
        && !c.snow
        && c.power.is_none()
        && c.toughness.is_none()
        && !c.mana_cost.generic.is_negative()
        && !definition.x_mana_cost
        && definition.intrinsic_zone.is_none()
        && definition.keywords.is_empty()
        && definition.activated.is_empty()
        && definition.mana_abilities.is_empty()
        && definition.triggered.is_empty()
        && (1..=2).contains(&definition.static_abilities.len())
        && definition
            .static_abilities
            .iter()
            .filter(|a| a.effect == StaticEffect::ControlEnchantedPermanent)
            .count()
            == 1
        && definition.static_abilities.iter().all(|a| {
            a.functioning_zone == FunctioningZone::Battlefield
                && (a.effect == StaticEffect::ControlEnchantedPermanent
                    || (target_type == AuraControlDomain::Creature
                        && matches!(
                            a.effect,
                            StaticEffect::ModifyCreatures {
                                scope: CreatureScope::Enchanted,
                                ..
                            }
                        )
                        && !attachment_modifiers::advanced(&a.effect)
                        && static_creature_modification_valid(c.kind, false, true, &a.effect)))
        })
        && p.timing == SpellTiming::MainPhase
        && p.resolution == SpellResolution::Permanent
        && p.functioning_zone == FunctioningZone::Stack
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.instructions.is_empty()
        && p.modes.is_empty()
        && target.slot == 0
        && !target.selection_id.is_empty()
        && target.chooser == Chooser::Controller
        && target.stage == EvaluationStage::Announcement;
    valid.then_some(target_type)
}
/// Whether this definition has the complete narrow Aura-control shape.
pub fn aura_control_source(definition: &Definition) -> bool {
    aura_control_domain(definition).is_some()
}

/// The sole narrow cascade program shared by intrinsic and granted instances.
impl Definition {
    pub fn requirements(&self) -> BTreeSet<Capability> {
        let mut set = BTreeSet::from([Capability::NormalLayoutV1, Capability::ComposedAbilityV1]);
        match self.characteristics.kind {
            CardKind::Land => {
                set.insert(Capability::LandPlayV1);
                if self.characteristics.basic {
                    set.insert(Capability::BasicLandManaV1);
                } else if !self.characteristics.subtypes.is_empty()
                    && !listed_mana_choice::descriptive_land_subtype(self)
                {
                    set.insert(Capability::TypedNonbasicIntrinsicManaV1);
                }
            }
            CardKind::Creature => {
                set.extend([Capability::BaseCreatureV1, Capability::CastCreatureV1]);
                if self.characteristics.legendary {
                    set.insert(Capability::CommanderIdentityV1);
                }
            }
            CardKind::Artifact => {
                set.insert(Capability::CastArtifactV1);
            }
            CardKind::Enchantment => {
                set.insert(Capability::CastEnchantmentV1);
            }
            CardKind::Instant | CardKind::Sorcery => {
                set.insert(if self.characteristics.kind == CardKind::Instant {
                    Capability::CastInstantV1
                } else {
                    Capability::CastSorceryV1
                });
            }
        }
        for kind in &self.characteristics.additional_types {
            match kind {
                CardKind::Artifact if self.characteristics.kind != CardKind::Land => {
                    set.insert(Capability::CastArtifactV1);
                }
                CardKind::Enchantment => {
                    set.insert(Capability::CastEnchantmentV1);
                }
                _ => {}
            }
        }
        if let Some(program) = &self.spell {
            add_program_requirements(&mut set, program);
        }
        if !self.mana_abilities.is_empty() {
            set.insert(Capability::PermanentManaV1);
        }
        for ability in &self.activated {
            set.insert(Capability::ActivatedAbilityV1);
            if activated_artifact_draw_ability(ability) {
                set.insert(Capability::ActivatedArtifactDrawV1);
            }
            if has_untap(&ability.program) {
                set.insert(Capability::TapSourceUntapCreatureV1);
            }
            if has_tap(&ability.program) {
                set.insert(if self.characteristics.kind == CardKind::Creature {
                    Capability::ActivatedCreatureTapV1
                } else {
                    Capability::ActivatedTapPermanentV1
                });
            }
            if has_permanent_owner_hand(&ability.program) {
                set.insert(Capability::ActivatedPermanentOwnerHandV1);
            }
            if ability.cost == ActivationCost::TapSource && has_destroy_permanent(&ability.program)
            {
                set.insert(Capability::TapSourceDestroyV1);
            }
            if matches!(
                ability.cost,
                ActivationCost::SacrificePermanent { count: 2.., .. }
            ) {
                set.insert(Capability::FixedArtifactSacrificeCostV1);
            }
            set.insert(match ability.cost {
                ActivationCost::DiscardSource { .. } => Capability::DiscardSourceCostV1,
                ActivationCost::SacrificePermanent { .. } => Capability::SacrificeV1,
                ActivationCost::SacrificeSource => Capability::SacrificeSourceCostV1,
                ActivationCost::TapSource => Capability::TapSourceCostV1,
                ActivationCost::TapPayLifeSacrificeSource { .. } => Capability::PayLifeCostV1,
                ActivationCost::Mana { .. } => Capability::ManaActivationCostV1,
                ActivationCost::Composite { .. } => Capability::CompositeActivationCostV1,
            });
            if matches!(
                ability.cost,
                ActivationCost::TapPayLifeSacrificeSource { .. }
            ) {
                set.extend([
                    Capability::TapSourceCostV1,
                    Capability::SacrificeSourceCostV1,
                ]);
            }
            add_program_requirements(&mut set, &ability.program);
        }
        for ability in &self.triggered {
            if ability.death_filter.is_some()
                || (ability.condition == TriggerCondition::SelfDies
                    && ability.program.instructions.iter().any(|i| {
                        matches!(
                            i,
                            kernel!(CreateTokens {
                                token: kir::TokenSpec::Creature { .. },
                                ..
                            })
                        )
                    }))
            {
                set.insert(Capability::SacrificeV1);
            }
            set.insert(Capability::TriggeredAbilityV1);
            if matches!(ability.condition, TriggerCondition::DamageDealt { .. })
                && has_controller_discard(&ability.program)
            {
                set.insert(Capability::DamageControllerDiscardV1);
            }
            if matches!(
                ability.condition,
                TriggerCondition::DamageDealt {
                    source: DamageTriggerSource::CreatureFilter { .. },
                    ..
                }
            ) {
                set.insert(Capability::FilteredDamageTriggerV1);
            }
            let trigger_capability = match ability.condition {
                TriggerCondition::GroupedDamageDealt { .. } => {
                    Capability::GroupedCreatureDamageTriggerV1
                }
                TriggerCondition::StepBegan { .. } => Capability::StepBeginningTriggerV1,
                TriggerCondition::DelayedControllerCreatureEntry => {
                    Capability::DelayedControllerCreatureEntryV1
                }
                TriggerCondition::DelayedControllerSpellCast => {
                    Capability::DelayedControllerSpellCastV1
                }
                TriggerCondition::SagaChapter { .. } => Capability::SagaChaptersV1,
                TriggerCondition::SpellCast {
                    spell: TypePredicate::SpellColor { .. },
                    ..
                } => {
                    set.insert(Capability::CastColorPredicateV1);
                    Capability::CastTriggerV1
                }
                TriggerCondition::SpellCast { ref spell, .. }
                    if cast_subtype::restricted(spell) =>
                {
                    set.insert(Capability::CastSubtypePredicateV1);
                    Capability::CastTriggerV1
                }
                TriggerCondition::SelfSpellCast | TriggerCondition::SpellCast { .. } => {
                    Capability::CastTriggerV1
                }
                TriggerCondition::SpellCastWithOrigin { .. } => {
                    set.insert(Capability::CastOriginTriggerV1);
                    Capability::CastTriggerV1
                }
                TriggerCondition::CreatureAttacked { .. } => Capability::AttackTriggerV1,
                TriggerCondition::DamageDealt {
                    kind: TriggerDamageKind::Combat,
                    recipient: DamageTriggerRecipient::Player,
                    ..
                } => Capability::DamageTriggerV1,
                TriggerCondition::DamageDealt { .. } => Capability::SourceDamageTriggerV2,
                TriggerCondition::OpponentCreatureDies => Capability::BoardWipesV1,
                TriggerCondition::SelfEntersBattlefield => Capability::SelfEntersBattlefieldV1,
                TriggerCondition::EquippedCombatDamage | TriggerCondition::EquippedDies => {
                    Capability::EquippedCreatureTriggerV1
                }
                TriggerCondition::ControlledLandEntersBattlefield => {
                    Capability::ControlledLandEntryV1
                }
                TriggerCondition::SelfBecomesTapped => Capability::SourceTapTriggerV1,
                TriggerCondition::ManaTapped { .. } => Capability::ManaTapTriggerV1,
                TriggerCondition::OtherCreatureEntersBattlefield => {
                    Capability::OtherCreatureEntersBattlefieldV1
                }
                TriggerCondition::ControlledCreatureEntersWithPower { .. } => {
                    Capability::PowerFilteredCreatureEntryV1
                }
                TriggerCondition::AnyCreatureDies => Capability::CreatureDiesV1,
                TriggerCondition::SelfDies => Capability::SelfDiesV1,
                TriggerCondition::SelfAuraToGraveyard => Capability::AttachmentsV1,
                TriggerCondition::SelfMoved { .. } => Capability::TriggeredSuccessorLibraryTuckV1,
            };
            set.insert(trigger_capability);
            if ability.intervening_if == Some(InterveningCondition::SourceCastByController) {
                set.insert(Capability::CastEntryConditionV1);
            }
            if ability.intervening_if.is_some() {
                set.insert(Capability::InterveningIfV1);
            }
            if matches!(
                ability.intervening_if,
                Some(InterveningCondition::SourceStatus { .. })
            ) {
                set.insert(Capability::SourceStatusConditionV1);
            }
            add_program_requirements(&mut set, &ability.program);
        }
        for keyword in &self.keywords {
            set.extend(intrinsic_keyword_capability(*keyword));
        }
        for a in &self.static_abilities {
            if matches!(&a.effect, StaticEffect::ModifyCreatures { keywords, .. }
                if keywords.contains(&KeywordAbility::Indestructible))
            {
                set.insert(Capability::IndestructibleV1);
            }
            if let StaticEffect::ModifyCreatures {
                scope: CreatureScope::ControlledSubtype { .. },
                keywords,
                ..
            } = &a.effect
            {
                set.extend(
                    keywords
                        .iter()
                        .filter_map(|k| intrinsic_keyword_capability(*k)),
                );
            }
            if attachment_modifiers::grants_protection(&a.effect) {
                set.insert(Capability::FixedColorProtectionV1);
            }
            let static_capability = match a.effect {
                StaticEffect::GrantSpellCascade { .. } => Capability::StaticSpellCascadeGrantV1,
                StaticEffect::Bestow { .. } => Capability::BestowV1,
                StaticEffect::CascadeLandEntry => Capability::CascadeLandEntryV1,
                StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn => {
                    Capability::EnterAsCopyOfPermanentEnteredThisTurnV1
                }
                StaticEffect::ControlEnchantedPermanent => Capability::AuraControlV1,
                StaticEffect::OpponentsCantCastDuringControllerTurn => {
                    Capability::OpponentCastProhibitionV1
                }
                StaticEffect::ControllerCreaturePlusOneCounterReplacement { .. } => {
                    Capability::ControllerCreaturePlusOneCounterReplacementV1
                }
                StaticEffect::UnlimitedControllerHandSize => Capability::UnlimitedHandSizeV1,
                StaticEffect::ReduceMaximumHandSize { .. } => Capability::ReducedHandSizeV1,
                StaticEffect::IncreaseMaximumHandSize { .. } => Capability::IncreasedHandSizeV1,
                StaticEffect::KeepUnspentManaAsBlack => Capability::UnspentManaConversionV1,
                StaticEffect::GrantControllerLandsAnyColorMana => {
                    Capability::ControllerLandGrantedManaV1
                }
                StaticEffect::ControllerNonlandsAllColors => Capability::ControllerNonlandColorsV1,
                StaticEffect::MayBeginOnBattlefield => Capability::OpeningPlacementV1,
                StaticEffect::GrantControllerLandsAllBasicTypes => {
                    Capability::ControllerLandBasicTypesV1
                }
                StaticEffect::AdditionalLandPlay | StaticEffect::PlayLandsFromOwnGraveyard => {
                    Capability::LandReclamationV1
                }
                StaticEffect::EntersTapped => Capability::EntersTappedV1,
                StaticEffect::EntersTappedUnlessPayLife { .. } => {
                    Capability::OptionalLifeLandEntryV1
                }
                StaticEffect::EntersTappedUnlessControllerBasicLands { .. } => {
                    Capability::ConditionalBasicLandEntryV1
                }
                StaticEffect::EntersTappedUnlessControllerOtherLands { .. } => {
                    Capability::ConditionalOtherLandEntryV1
                }
                StaticEffect::EntersTappedUnlessOpponentCount { .. } => {
                    Capability::ConditionalOpponentCountEntryV1
                }
                StaticEffect::EntersTappedUnlessRevealLandTypes { .. } => {
                    Capability::RevealLandEntryV1
                }
                StaticEffect::EntersTappedUnlessControllerLandType { .. } => {
                    Capability::RestrictedManaV1
                }
                StaticEffect::EntersTappedUnlessControllerLandTypes { .. } => {
                    Capability::ConditionalLandTypeEntryV1
                }
                StaticEffect::ReduceSelfGenericCost { .. } => {
                    Capability::CountedSelfGenericCostReductionV1
                }
                StaticEffect::ReduceGenericByCreatureCount => Capability::BoardWipesV1,
                StaticEffect::CantBeCountered => self_spell_protection::capability(self),
                StaticEffect::SelfGraveyardRevealShuffle => {
                    Capability::SelfGraveyardRevealShuffleV1
                }
                StaticEffect::GrantOtherControlledPermanentsIndestructible => {
                    set.insert(Capability::IndestructibleV1);
                    Capability::StaticPermanentIndestructibleGrantV1
                }
                StaticEffect::DoubleDamageToPermanentOrPlayer => {
                    Capability::StaticDamageReplacementV1
                }
                StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer => {
                    Capability::ControlledCreatureDamageReplacementV1
                }
                StaticEffect::ControllerCreatureAnthem { .. } => Capability::CreatureAnthemV1,
                StaticEffect::ControllerScopedCreatureStats { .. } => {
                    Capability::ScopedCreatureStatsV1
                }
                StaticEffect::ModifySourceByDistinctPermanentColors { .. } => {
                    Capability::DistinctPermanentColorsV1
                }
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::AttackingController,
                    ..
                } => Capability::AttackingCreatureKeywordsV1,
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::Source,
                    ..
                } if source_combat_restrictions::restriction(&a.effect).is_some() => {
                    Capability::SourceCombatRestrictionsV1
                }
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::Source,
                    ..
                } => Capability::SacrificeV1,
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::ControlledSubtype { .. },
                    ..
                } => Capability::FilteredStaticModifiersV1,
                StaticEffect::ModifyCreatures { .. } => Capability::AttachmentsV1,
            };
            set.insert(static_capability);
        }
        set
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.composed {
            return Err("definition requires complete Archive R4 composition".into());
        }
        composition::validate(self)
    }
}
fn validate_permanent_costs(c: &PermanentActivationCosts) -> Result<(), String> {
    if c.mana
        .as_ref()
        .is_some_and(|m| m.generic.is_negative() || m.colored.contains(&Color::C))
    {
        return Err("unsupported permanent mana cost".into());
    }
    Ok(())
}
pub fn supported_temporary_keywords(keywords: &[KeywordAbility]) -> bool {
    !keywords.is_empty()
        && keywords.iter().enumerate().all(|(index, keyword)| {
            matches!(
                keyword,
                KeywordAbility::Flying
                    | KeywordAbility::DoubleStrike
                    | KeywordAbility::Vigilance
                    | KeywordAbility::Reach
                    | KeywordAbility::Deathtouch
                    | KeywordAbility::Indestructible
                    | KeywordAbility::Lifelink
                    | KeywordAbility::Hexproof
                    | KeywordAbility::Haste
                    | KeywordAbility::Trample
                    | KeywordAbility::FirstStrike
            ) && !keywords[..index].contains(keyword)
        })
}
/// Borrowed parameters of one complete ordinary targeted temporary keyword program.
/// CR 701.8: a destroy of one announced permanent (the permanent-removal
/// surface), as distinct from a destroy of one announced creature.
fn permanent_destroy(program: &SpellProgram, instruction: &Instruction) -> bool {
    target_destroy_domain(program, instruction) == Some(TargetDomain::BattlefieldPermanent)
}
fn target_destroy_domain(
    program: &SpellProgram,
    instruction: &Instruction,
) -> Option<TargetDomain> {
    let (slot, ..) = instruction.target_destroy()?;
    program.targets.get(slot as usize).map(|t| t.domain)
}
fn has_destroy_permanent(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| permanent_destroy(program, i))
        || program
            .modes
            .iter()
            .any(|mode| has_destroy_permanent(&mode.program))
}
fn has_permanent_owner_hand(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|instruction| match instruction {
            kernel!(Move {
                what: target_object!(target_slot),
                to: kir::Destination::OwnerHand
            }) => program
                .targets
                .get(*target_slot as usize)
                .is_some_and(|target| target.domain == TargetDomain::BattlefieldPermanent),
            _ => false,
        })
        || program
            .modes
            .iter()
            .any(|mode| has_permanent_owner_hand(&mode.program))
}
fn has_tap(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Tap {
                what: target_object!(_)
            })
        )
    }) || program.modes.iter().any(|m| has_tap(&m.program))
}
fn has_untap(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Untap {
                what: target_object!(_)
            })
        )
    }) || program.modes.iter().any(|m| has_untap(&m.program))
}
fn has_controller_discard(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| i.controller_discard().is_some())
        || program
            .modes
            .iter()
            .any(|m| has_controller_discard(&m.program))
}
// Intrinsic keyword capability closure is shared by Rust and exported schema.
fn intrinsic_keyword_capability(keyword: KeywordAbility) -> Option<Capability> {
    Some(match keyword {
        KeywordAbility::ProtectionFromEachColor => return None,
        KeywordAbility::ProtectionFromEverything => Capability::ProtectionFromEverythingV1,
        KeywordAbility::ProtectionFromWhite
        | KeywordAbility::ProtectionFromBlue
        | KeywordAbility::ProtectionFromBlack
        | KeywordAbility::ProtectionFromRed
        | KeywordAbility::ProtectionFromGreen => Capability::FixedColorProtectionV1,
        KeywordAbility::Flash => Capability::FlashV1,
        KeywordAbility::Defender => Capability::DefenderV1,
        KeywordAbility::Flying => Capability::FlyingV1,
        KeywordAbility::Reach => Capability::ReachV1,
        KeywordAbility::Vigilance => Capability::VigilanceV1,
        KeywordAbility::Haste => Capability::HasteV1,
        KeywordAbility::Lifelink => Capability::LifelinkV1,
        KeywordAbility::Deathtouch => Capability::DeathtouchV1,
        KeywordAbility::Indestructible => Capability::IndestructibleV1,
        KeywordAbility::Trample => Capability::TrampleV1,
        KeywordAbility::FirstStrike => Capability::FirstStrikeV1,
        KeywordAbility::DoubleStrike => Capability::DoubleStrikeV1,
        KeywordAbility::Menace => Capability::MenaceV1,
        KeywordAbility::Plainswalk
        | KeywordAbility::Islandwalk
        | KeywordAbility::Swampwalk
        | KeywordAbility::Mountainwalk
        | KeywordAbility::Forestwalk => Capability::BasicLandwalkV1,
        KeywordAbility::Hexproof | KeywordAbility::Shroud => Capability::AttachmentsV1,
    })
}
// One primitive mapping for targeted and set grants, including schema closure.
fn temporary_keyword_primitive_capability(keyword: KeywordAbility) -> Option<Capability> {
    Some(match keyword {
        KeywordAbility::Flying => Capability::FlyingV1,
        KeywordAbility::Reach => Capability::ReachV1,
        KeywordAbility::Vigilance => Capability::VigilanceV1,
        KeywordAbility::DoubleStrike => Capability::DoubleStrikeV1,
        KeywordAbility::Deathtouch => Capability::DeathtouchV1,
        KeywordAbility::Indestructible => Capability::IndestructibleV1,
        KeywordAbility::Lifelink => Capability::LifelinkV1,
        KeywordAbility::Hexproof => Capability::AttachmentsV1,
        KeywordAbility::Haste => Capability::HasteV1,
        KeywordAbility::Trample => Capability::TrampleV1,
        KeywordAbility::FirstStrike => Capability::FirstStrikeV1,
        _ => return None,
    })
}
fn add_program_requirements(set: &mut BTreeSet<Capability>, program: &SpellProgram) {
    for target in &program.targets {
        if target.filter.is_some() {
            set.insert(Capability::PermanentRemovalV1);
        }
        if target
            .filter
            .as_ref()
            .is_some_and(|filter| filter.power.is_some())
        {
            set.insert(Capability::PowerFilteredRemovalV1);
        }
        if target.filter.as_ref().is_some_and(|filter| filter.tapped) {
            set.insert(Capability::TappedPermanentTargetV1);
        }
        set.insert(match target.domain {
            TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent => {
                Capability::PermanentRemovalV1
            }
            TargetDomain::CreatureOrPlaneswalker => Capability::CreatureOrPlaneswalkerTargetV1,
            TargetDomain::BattlefieldCreature => Capability::CreatureTargetV1,
            TargetDomain::BlockingBattlefieldCreature => Capability::BlockingCreatureTargetV1,
            TargetDomain::ControlledBattlefieldCreature => Capability::AttachmentsV1,
            TargetDomain::OwnGraveyardCreatureCard => Capability::GraveyardRecursionV1,
            TargetDomain::OwnGraveyardCard => Capability::OwnGraveyardCardReturnV1,
            TargetDomain::PlayerGraveyardCards { .. } => Capability::PlayerGraveyardShuffleV1,
            TargetDomain::OpponentPlayer => Capability::OpponentTargetV1,
            TargetDomain::StackSpell | TargetDomain::ControlledCreatureSpell => {
                Capability::StackSpellTargetV1
            }
            TargetDomain::StackActivatedOrTriggeredAbility => Capability::StackAbilityTargetV1,
            TargetDomain::AnyDamageTarget => Capability::AnyDamageTargetV1,
            TargetDomain::PlayerOrPlaneswalker => Capability::PlayerOrPlaneswalkerTargetV1,
            TargetDomain::AnyPlayer => Capability::AnyPlayerTargetV1,
        });
    }
    for instruction in &program.instructions {
        let Instruction::Kernel { node, .. } = instruction;
        set.extend(kir::node_capabilities(node));
        add_kernel_leaf_requirements(set, &program.targets, node);
    }
}
fn add_kernel_leaf_requirements(
    set: &mut BTreeSet<Capability>,
    targets: &[TargetSpec],
    node: &kir::Node,
) {
    let target_domain = |slot: u32| targets.get(slot as usize).map(|target| target.domain);
    kir::visit_verbs(node, &mut |verb| {
        let capability = match verb {
            Verb::Draw {
                player: player @ kir::PlayerExpr::Is { .. },
                amount: fixed!(count),
            } => {
                if !player.is_you() {
                    Capability::TargetPlayerDrawV1
                } else if *count == Quantity::from(1) {
                    Capability::ControllerDrawV1
                } else {
                    Capability::ControllerCountedDrawV1
                }
            }
            Verb::Discard { player, .. } if player.is_you() => Capability::ControllerDiscardV1,
            Verb::Destroy {
                what: target_object!(slot),
                ..
            } => match target_domain(*slot) {
                Some(TargetDomain::BattlefieldCreature) => Capability::DestroyCreatureV1,
                Some(TargetDomain::BattlefieldPermanent) => Capability::PermanentRemovalV1,
                _ => return,
            },
            Verb::Move {
                what: target_object!(slot),
                to: kir::Destination::OwnerHand,
            } if target_domain(*slot) == Some(TargetDomain::BattlefieldPermanent) => {
                Capability::PermanentOwnerHandMoveV1
            }
            Verb::GainLife { player, amount } | Verb::LoseLife { player, amount }
                if player.is_target() && amount.is_announced_x() =>
            {
                Capability::AnnouncedXEffectsV1
            }
            _ => return,
        };
        set.insert(capability);
    });
}

pub fn basic_land_color(subtype: &str) -> Option<Color> {
    BasicLandType::from_subtype(subtype).map(BasicLandType::color)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    #[schemars(schema_with = "current_ir_version_schema")]
    pub schema_version: u32,
    pub rules_edition: String,
    pub rules_sha256: String,
    pub definitions: Vec<Definition>,
}
impl Catalog {
    /// Each physical source contributes its native observer/self slots and its
    /// static grant slots once per cast, never once per recipient permanent.
    /// Tokens/copies with abilities are outside this physical-source proof.
    pub fn cast_trigger_fanout_bound(&self, physical_sources: usize) -> Option<usize> {
        self.definitions
            .iter()
            .try_fold(0usize, |max, d| {
                let grants = d
                    .static_abilities
                    .iter()
                    .filter(|a| matches!(a.effect, StaticEffect::GrantSpellCascade { .. }))
                    .count();
                d.triggered
                    .len()
                    .checked_add(grants)
                    .map(|slots| max.max(slots))
            })?
            .checked_mul(physical_sources)
    }

    /// Look up a definition in a validated, canonically ordered catalog.
    ///
    /// `read` and `validate` establish unique, increasing Oracle identities.
    /// Callers that mutate the public definition table must validate it again
    /// before lookup. The sorted table is its own index: lookup allocates no
    /// cache and retains no references that can become stale after replacement.
    pub fn definition_by_oracle_id(&self, oracle_id: &str) -> Option<&Definition> {
        self.definitions
            .binary_search_by(|definition| definition.oracle_id.as_str().cmp(oracle_id))
            .ok()
            .map(|index| &self.definitions[index])
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != CURRENT_IR_VERSION
            || self.rules_edition != RULES_EDITION
            || self.rules_sha256 != RULES_SHA256
        {
            return Err("unsupported IR version or rules snapshot".into());
        }
        for d in &self.definitions {
            d.validate()?;
        }
        if !self
            .definitions
            .windows(2)
            .all(|w| w[0].oracle_id < w[1].oracle_id)
        {
            return Err("catalog is not canonically ordered".into());
        }
        Ok(())
    }
    pub fn read(json: &str) -> Result<Self, String> {
        let c: Self = strict_read(json)?;
        c.validate()?;
        Ok(c)
    }
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Incremental byte hashing without retaining the input or performing OS I/O.
/// This sink does not canonicalize JSON: callers must emit the same ordered
/// bytes as `canonical` when they need an equivalent semantic digest.
pub struct Sha256Sink(Sha256);
impl Sha256Sink {
    pub fn new() -> Self {
        Self(Sha256::new())
    }
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    pub fn finalize_hex(self) -> String {
        format!("{:x}", self.0.finalize())
    }
}
impl Default for Sha256Sink {
    fn default() -> Self {
        Self::new()
    }
}
impl std::io::Write for Sha256Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Card IR canonicalization: UTF-8 compact serde_json Value (sorted object keys),
/// ordered arrays, exact quantities as strings. Separate from JS archive hashes.
pub fn canonical<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_value(value)
        .and_then(|v| serde_json::to_string(&v))
        .map_err(|e| e.to_string())
}
pub fn digest<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(sha256(canonical(value)?.as_bytes()))
}
/// Structural schema of the executable wire types. Semantic validation uses the KIR checker.
pub fn catalog_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(Catalog)).expect("schema is JSON")
}

pub fn strict_read<T: de::DeserializeOwned>(json: &str) -> Result<T, String> {
    let _: UniqueJson = serde_json::from_str(json).map_err(|e| e.to_string())?;
    serde_json::from_str(json).map_err(|e| e.to_string())
}
/// Reject duplicates before tagged enums or maps could discard a repeated field.
struct UniqueJson;
impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = UniqueJson;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON without duplicate object keys")
            }
            fn visit_map<M: de::MapAccess<'de>>(self, mut m: M) -> Result<UniqueJson, M::Error> {
                let mut keys = BTreeSet::new();
                while let Some(k) = m.next_key::<String>()? {
                    if !keys.insert(k) {
                        return Err(de::Error::custom("duplicate JSON key"));
                    }
                    m.next_value::<UniqueJson>()?;
                }
                Ok(UniqueJson)
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut a: A) -> Result<UniqueJson, A::Error> {
                while a.next_element::<UniqueJson>()?.is_some() {}
                Ok(UniqueJson)
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_unit<E: de::Error>(self) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
        }
        d.deserialize_any(Visitor)
    }
}

#[cfg(test)]
mod catalog_lookup_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_ir_authority_matches_schema_and_validation() {
        for schema in [
            catalog_schema(),
            serde_json::to_value(schemars::schema_for!(Catalog)).unwrap(),
        ] {
            assert_eq!(
                schema["properties"]["schema_version"]["const"],
                CURRENT_IR_VERSION
            );
        }
        for version in [0, CURRENT_IR_VERSION, 2, 33, u32::MAX] {
            let catalog = Catalog {
                schema_version: version,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![],
            };
            assert_eq!(catalog.validate().is_ok(), version == CURRENT_IR_VERSION);
        }
    }

    #[test]
    fn incremental_sha256_matches_reference_across_byte_fragment_boundaries() {
        use std::io::Write;
        let bytes = "前半🦀\n\"quoted\"\\suffix".repeat(13).into_bytes();
        let expected = sha256(&bytes);
        // Include splits inside UTF-8 scalars and across SHA-256 block edges.
        for split in 0..=bytes.len() {
            let mut sink = Sha256Sink::new();
            sink.update(&bytes[..split]);
            assert_eq!(sink.write(&[]).unwrap(), 0);
            sink.write_all(&bytes[split..]).unwrap();
            sink.flush().unwrap();
            assert_eq!(sink.finalize_hex(), expected, "split {split}");
        }
        for width in [1, 3, 63, 64, 65, 127] {
            let mut sink = Sha256Sink::default();
            for chunk in bytes.chunks(width) {
                sink.write_all(chunk).unwrap();
            }
            assert_eq!(sink.finalize_hex(), expected, "chunk width {width}");
        }
    }
    #[test]
    fn incremental_sha256_empty_and_canonical_json_match_reference() {
        let mut empty = Sha256Sink::new();
        empty.update(&[]);
        assert_eq!(empty.finalize_hex(), sha256(b""));
        let value = serde_json::json!({
            "z": ["é\u{0301}", "🦀", "\n\t\\\"", ""],
            "a": {"quantity": Quantity::from(-2), "none": null},
        });
        let reference = canonical(&value).unwrap();
        let mut sink = Sha256Sink::new();
        serde_json::to_writer(&mut sink, &value).unwrap();
        assert_eq!(sink.finalize_hex(), digest(&value).unwrap());
        let mut fragmented = Sha256Sink::new();
        for byte in reference.as_bytes() {
            fragmented.update(std::slice::from_ref(byte));
        }
        assert_eq!(fragmented.finalize_hex(), sha256(reference.as_bytes()));
    }
    #[test]
    fn quantities_are_exact_canonical_strings() {
        let mut q: Quantity = "999999999999999999999999999999999999999".parse().unwrap();
        q.add(&Quantity::from(1));
        assert_eq!(q.to_string(), "1000000000000000000000000000000000000000");
        assert_eq!(
            serde_json::from_str::<Quantity>("\"-42\"").unwrap(),
            Quantity::from(-42)
        );
        for input in ["1", "\"01\"", "\"-0\"", "\"+1\"", "\"1.0\""] {
            assert!(serde_json::from_str::<Quantity>(input).is_err());
        }
    }
    #[test]
    fn rejects_duplicates_even_in_nested_untyped_json() {
        assert!(strict_read::<serde_json::Value>(r#"{"x":[{"a":1,"a":2}]}"#).is_err());
    }
}

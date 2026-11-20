//! Executable Card IR contracts. No rules execution or parser dependency.
mod mana_combination;
pub use mana_combination::{MANA_COMBINATION_COLORS, validate_mana_combination};
pub mod commander_modal_keyword;
pub use commander_modal_keyword::akromas_will_program;
mod bestow;
pub use bestow::{BestowCastPlan, BestowForm, bestow_alternative, canonical_bestow_program};
mod cascade_land_entry;
mod granted_land_mana;
pub mod hand_cast_cascade;
pub mod library_top_mana;
mod unspent_mana;
pub use granted_land_mana::{
    granted_land_mana_printed, granted_land_mana_source, granted_land_mana_static,
};
mod opening_placement;
pub use opening_placement::{opening_placement_ability, opening_placement_ast, three_static_ast};
mod controller_land_types;
pub use controller_land_types::{controller_land_type_ast, controller_nonland_colors_ast};
mod colored_library_search;
pub use colored_library_search::{
    ColoredLibrarySearch, ColoredSearchAssignment, colored_library_program, colored_library_source,
};
mod color_hybrid_cost;
pub use color_hybrid_cost::{ColorHybridSymbol, HybridPayment, color_hybrid_cost};
mod conditional_lands;
mod cycling;
pub use cycling::hand_discard_draw_activation;
mod counted_costs;
mod distinct_permanent_colors;
mod opponent_mana;
pub use opponent_mana::opponent_land_mana_family;
mod listed_mana_choice;
mod pain_mana;
pub use pain_mana::disjoint_pain_mana_family;
pub mod basic_landwalk;
mod cast_entry;
mod entry_copy;
mod saga;
pub use saga::{ChapterEffect, LifeChapter, chapter_lines, life_chapters};
mod fixed_creature_activation;
pub mod fixed_creature_damage;
pub use fixed_creature_activation::{
    FixedCreatureActivation, bound_fixed_activation_keyword, fixed_activation_keyword,
    fixed_creature_activation, has_fixed_creature_activation,
};
mod cast_mana_value_sweep;
mod cast_origin;
mod cast_payer_draw;
mod controlled_creature_spell_copy;
mod creative_technique;
pub use cast_payer_draw::cast_payer_draw_trigger;
mod successor_tuck;
pub use successor_tuck::{LibraryPosition, SourceMoveOrigin, TriggerSuccessorZone};
mod typed_land_search;
pub use typed_land_search::{BasicLandType, TypedLandSearch};
/// Engine build identity excludes card/profile data and hosting code. Retained
/// evidence still carries those files with their own hashes; matches/snapshots
/// separately bind the exact catalog digest. Used by both the core build and
/// the independent native source-closure verifier.
pub fn engine_build_input(path: &str) -> bool {
    matches!(path, "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml")
        || (path.starts_with("packages/game-engine/crates/")
            && !path
                .split('/')
                .any(|p| matches!(p, "generated" | "target" | "node_modules" | ".git"))
            && (path.ends_with(".rs") || path.ends_with("/Cargo.toml")))
}
use num_bigint::BigInt;
use num_traits::Zero;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
mod activated_artifact_draw;
pub mod activated_source_keyword;
pub use activated_artifact_draw::activated_artifact_draw_ability;
mod controller_counted_draw;
pub use controller_counted_draw::controller_counted_draw_program;
mod announced_x_draw;
mod artifact_modal;
mod controlled_phasing;
pub use controlled_phasing::controller_window_instructions;
mod counter_unless_generic;
mod each_player_draw;
mod entry_graveyard_exile;
mod land_entry_gain_life;
mod land_sacrifice_draw;
mod other_land_entry;
mod set_exile;
pub use set_exile::{SetExileClause, set_exile_modes};
mod hand_cast;
pub use hand_cast::{combat_hand_cast_ast, combat_hand_cast_trigger};
mod opponent_life_drain;
mod targeted_counted_draw;
mod targeted_discard;
mod targeted_game_loss;
pub use artifact_modal::BoundArtifactMode;
pub use targeted_counted_draw::targeted_counted_draw_program;
pub use targeted_game_loss::{ArtifactLossSource, artifact_loss_activation};
mod controller_creature_pump;
mod controller_library;
mod counter_spell_controller_mill;
pub mod creature_planeswalker_return;
mod draw_discard;
mod fixed_mill;
mod graveyard_battlefield_return;
pub mod graveyard_shuffle;
mod group_discard;
mod hand_return;
pub mod modal_targets;
pub mod repeated_target_modes;
pub use draw_discard::controller_draw_discard_program;
mod library_order;
pub use hand_return::controller_hand_return_program;
mod keyword_removal_wipe;
mod library_selection;
mod nonland_exile_life;
pub use keyword_removal_wipe::{keyword_removal_wipe_program, keyword_removal_wipe_source};
pub mod linked_hideaway;
mod opponent_nonland_destroy;
mod source_dies_nonland_destroy;
pub use source_dies_nonland_destroy::{death_sweep_abilities, death_sweep_source_block};
pub mod self_graveyard_shuffle;
mod self_spell_protection;
pub mod source_combat_restrictions;
pub use controller_library::{
    FixedCastEffect, cast_scry_ability_ast, controller_scry_program, controller_surveil_program,
    matches_fixed_amount, printed_cast_scry,
};
pub use fixed_mill::fixed_mill_program;
pub use library_order::controller_library_order_program;
pub use library_selection::controller_library_selection_program;
use serde::{Deserialize, Serialize, de};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt, str::FromStr};

mod archives;
mod optional_costs;
mod source_tap_damage;
mod static_controller;
pub use archives::*;
mod creature_entry_return;
mod divided_plus_one_counters;
mod flash_end_step_draw;
mod greatest_power;
mod hand_size_adjustments;
mod own_graveyard_card_return;
mod paid_counted_draw;
mod power_entry_draw;
pub use greatest_power::{greatest_power_ast, greatest_power_programs, greatest_power_source};
mod power_filtered_removal;
pub use divided_plus_one_counters::divided_plus_one_counters_modal_program;
mod strict_io;
pub use strict_io::strict_read_seekable;
mod unlimited_hand_size;

/// The sole active Card IR schema accepted and emitted by the engine.
pub const CURRENT_IR_VERSION: u32 = 1;

fn current_ir_version_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({"type":"integer", "const":CURRENT_IR_VERSION})
}

pub const RULES_EDITION: &str = "2026-09-25";
pub const RULES_SHA256: &str = "8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca";
pub const SCENARIO_PROFILE: &str = "land-return-departure-scenario-v2";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Quantity(BigInt);
impl Quantity {
    pub fn zero() -> Self {
        Self(BigInt::zero())
    }
    pub fn is_negative(&self) -> bool {
        self.0 < BigInt::zero()
    }
    pub fn add(&mut self, other: &Self) {
        self.0 += &other.0;
    }
    pub fn multiply(&mut self, other: &Self) {
        self.0 *= &other.0;
    }
    pub fn subtract(&mut self, other: &Self) {
        self.0 -= &other.0;
    }
}
impl From<i64> for Quantity {
    fn from(value: i64) -> Self {
        Self(BigInt::from(value))
    }
}
impl FromStr for Quantity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let value = BigInt::from_str(s).map_err(|_| "invalid exact quantity".to_string())?;
        if value.to_string() != s {
            return Err("quantity must use canonical decimal spelling".into());
        }
        Ok(Self(value))
    }
}
impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Serialize for Quantity {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}
impl JsonSchema for Quantity {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ExactQuantity".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({"type":"string","pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)$"})
    }
}

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
    CastSpellColorCountersV1,
    CounterRemovalManaV1,
    ListedManaChoiceV1,
    DisjointPainManaV1,
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
    CombatDamagePreventionV1,
    DamageBudgetPreventionV1,
    StaticDamageReplacementV1,
    ControlledCreatureDamageReplacementV1,
    SagaChaptersV1,
    DelayedControllerCreatureEntryV1,
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
    TriggeredAbilityV1,
    SelfEntersBattlefieldV1,
    OtherCreatureEntersBattlefieldV1,
    PowerFilteredCreatureEntryV1,
    CreatureEntryReturnV1,
    EntryGraveyardExileV1,
    LandEntryGainLifeV1,
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
    DividedPlusOneCountersV1,
    DestroyCreatureV1,
    GraveyardRecursionV1,
    GraveyardBattlefieldReturnV1,
    OwnGraveyardCardReturnV1,
    BasicLandSearchV1,
    BasicLandSplitSearchV1,
    ColoredLibrarySearchV1,
    TypedLandSearchV1,
    CreatureAnthemV1,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SetMove {
    Destroy,
    Exile,
    Hand,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
pub enum WipeAmount {
    Fixed {
        value: Quantity,
    },
    AnnouncedX,
    CreatureCount,
    /// Sum of the controller's other completed casts this turn, at resolution.
    #[serde(deserialize_with = "cast_mana_value_sweep::empty_amount")]
    OtherCastsManaValue,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
pub enum OpponentDrainAmount {
    Fixed {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        value: Quantity,
    },
    AnnouncedX,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
pub enum RandomDiscardCount {
    Fixed {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        value: Quantity,
    },
    AnnouncedX,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
pub enum DrawEachCount {
    Fixed {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        value: Quantity,
    },
    AnnouncedX,
}
/// The affected seats are evaluated relative to the invocation controller at resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupDiscardScope {
    EachPlayer,
    EachOpponent,
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
                .any(|k| k.basic_landwalk_type().is_some())
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
    LookTopMayRevealForMana {
        minimum_symbols: Quantity,
        mana: Quantity,
        source_block: u32,
    },
    RegisterControllerCreatureEntryThisTurn {
        program: Box<SpellProgram>,
        source_block: u32,
    },
    Hideaway {
        count: u32,
        source_block: u32,
    },
    PlayHideawayLinked {
        hideaway_block: u32,
        minimum_total_power: Quantity,
        source_block: u32,
    },
    GrantHandCastCascade {
        source_block: u32,
    },
    OfferHandCastWithoutManaCost {
        source_block: u32,
    },
    /// CR 702.85a: threshold comes from the originating spell, never paid cost.
    Cascade {
        threshold: CascadeThreshold,
        source_block: u32,
    },
    /// Shuffle, reveal through the first nonland, exile it, random-bottom the
    /// remaining revealed cards, then optionally cast the exile successor for free.
    RevealAndCast {
        source_block: u32,
    },
    /// CR 702.144a: optional self-copy followed by a chosen opponent's copy.
    Demonstrate {
        source_block: u32,
    },
    DrawControllerCards {
        count: Quantity,
        source_block: u32,
    },
    GrantLandPlaysThisTurn {
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        count: Quantity,
        source_block: u32,
    },
    PutHandOnLibrary {
        count: Quantity,
        source_block: u32,
    },
    OrderLibraryTop {
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        amount: Quantity,
        source_block: u32,
    },
    ShufflePlayerGraveyard {
        source_block: u32,
    },
    OptionalShuffleLibrary {
        source_block: u32,
    },
    SelectLibraryCard {
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        amount: Quantity,
        remainder: LibraryRemainder,
        source_block: u32,
    },
    /// CR 701.22: only the invocation controller looks and orders the prefix.
    MillCards {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipient: Option<LifeRecipient>,
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        count: Quantity,
        source_block: u32,
    },
    /// CR 701.22/701.25: the invocation controller looks and partitions the prefix.
    Surveil {
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        amount: Quantity,
        source_block: u32,
    },
    Scry {
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        amount: Quantity,
        source_block: u32,
    },
    ExileResolvingSpell {
        source_block: u32,
    },
    ProtectControllerUntilNextTurn {
        source_block: u32,
    },
    FreezeControllerLifeUntilNextTurn {
        source_block: u32,
    },
    PhaseOutControllerPermanents {
        source_block: u32,
    },
    PhaseOutTargetPermanent {
        target_slot: u32,
        source_block: u32,
    },
    MoveSet {
        set: ObjectSet,
        action: SetMove,
        source_block: u32,
    },
    RemoveOpponentNonlandIndestructible {
        source_block: u32,
    },
    DestroySet {
        set: ObjectSet,
        /// CR 701.19c: a prohibition keeps regeneration replacement effects
        /// from applying to this destruction.
        #[serde(default, skip_serializing_if = "is_false")]
        regeneration_forbidden: bool,
        source_block: u32,
    },
    PumpSet {
        set: ObjectSet,
        power: Quantity,
        toughness: Quantity,
        source_block: u32,
    },
    DamageSet {
        sets: Vec<ObjectSet>,
        each_player: bool,
        amount: WipeAmount,
        source_block: u32,
    },
    GainLifeDestroyed {
        source_block: u32,
    },
    ExileDamagedThisTurn {
        source_block: u32,
    },
    EachPlayerSacrifice {
        source_block: u32,
    },
    LoseLifeDeathController {
        amount: Quantity,
        source_block: u32,
    },
    SacrificeDamage {
        target_slot: Option<u32>,
        recipient: Option<LifeRecipient>,
        amount: SacrificeDamageAmount,
        optional: bool,
        source_block: u32,
    },
    EachOpponentSacrifice {
        source_block: u32,
    },
    ModifySourcePowerToughness {
        power: Quantity,
        toughness: Quantity,
        duration: Duration,
        source_block: u32,
    },
    DestroyPermanent {
        target_slot: u32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        optional: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        regeneration_forbidden: bool,
        source_block: u32,
    },
    ExilePermanent {
        target_slot: u32,
        source_block: u32,
    },
    UntapPermanent {
        target_slot: u32,
        source_block: u32,
    },
    TapPermanent {
        target_slot: u32,
        source_block: u32,
    },
    ChangeLifeFromTarget {
        recipient: EffectRecipient,
        amount: TargetAmount,
        gain: bool,
        source_block: u32,
    },
    CreateTokensFor {
        recipient: EffectRecipient,
        count: u32,
        token: CreatureToken,
        source_block: u32,
    },
    DamageTargetController {
        target_slot: u32,
        amount: Quantity,
        source_block: u32,
    },
    AttachEquipment {
        target_slot: u32,
        source_block: u32,
    },
    ReturnAuraFromGraveyard {
        source_block: u32,
    },
    DestroyTargetCreature {
        target_slot: u32,
        source_block: u32,
    },
    DestroyAllCreatures {
        source_block: u32,
    },
    ReturnOwnGraveyardCreatureToBattlefield {
        target_slot: u32,
        source_block: u32,
    },
    ReturnOwnGraveyardCreatureToHand {
        target_slot: u32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        optional: bool,
        source_block: u32,
    },
    ReturnOwnGraveyardCardToHand {
        target_slot: u32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        optional: bool,
        source_block: u32,
    },
    /// Own-library optional zero-to-two basics, reveal, one tapped/remaining hand, shuffle.
    SearchBasicLandsSplit {
        source_block: u32,
    },
    SearchColoredLibrary {
        colors: Vec<Color>,
        source_block: u32,
    },
    SearchTypedLands {
        search: TypedLandSearch,
        source_block: u32,
    },
    SearchBasicLands {
        maximum: u32,
        source_block: u32,
    },
    CreateCreatureTokens {
        count: u32,
        token: CreatureToken,
        source_block: u32,
    },
    PutSourcePlusOneCountersForCastColors {
        source_block: u32,
    },
    /// CR 601.2c–d. Slot zero is one variable target occurrence. Announce its
    /// distinct objects and positive shares summing to count before paying costs.
    DistributePlusOneCounters {
        count: Quantity,
        minimum_targets: u32,
        maximum_targets: u32,
        source_block: u32,
    },
    PutPlusOneCounters {
        on: CounterRecipient,
        amount: Quantity,
        source_block: u32,
    },
    /// CR 611.2c, 613.5–6: one captured set receives all parts of this
    /// continuous effect, including an optional fixed signed P/T modifier.
    GrantKeywordsSet {
        set: ObjectSet,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
        duration: Duration,
        source_block: u32,
    },
    /// CR 400.7, 611.2c: capture the activating incarnation until cleanup.
    GrantSourceKeywords {
        keywords: Vec<KeywordAbility>,
        source_block: u32,
    },
    /// CR 611.2c, 613.5–6: all properties belong to one captured effect.
    GrantKeywords {
        target_slot: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
        duration: Duration,
        source_block: u32,
    },
    ModifyPowerToughness {
        target_slot: u32,
        power: Quantity,
        toughness: Quantity,
        duration: Duration,
        source_block: u32,
    },
    PreventCombatDamage {
        duration: Duration,
        source_block: u32,
    },
    PreventNextDamage {
        target_slot: u32,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        duration: Duration,
        source_block: u32,
    },
    ExileTargetGraveyard {
        target_slot: u32,
        source_block: u32,
    },
    ReturnEntryTargetCreature {
        target_slot: u32,
        optional: bool,
        source_block: u32,
    },
    ReturnToOwnerHand {
        target_slot: u32,
        source_block: u32,
    },
    MoveTriggeredSuccessor {
        expected_zone: TriggerSuccessorZone,
        destination: TriggerSuccessorZone,
        position: LibraryPosition,
        source_block: u32,
    },
    PutOnTopOwnerLibrary {
        target_slot: u32,
        source_block: u32,
    },
    LoseGameTargetPlayer {
        #[schemars(range(max = 0))]
        target_slot: u32,
        #[schemars(range(min = 1, max = 1))]
        source_block: u32,
    },
    TargetPlayerSacrificeCreature {
        target_slot: u32,
        source_block: u32,
    },
    /// One creature chosen by the frozen damage recipient (CR 701.21a).
    SacrificeCreature {
        recipient: ActionRecipient,
        source_block: u32,
    },
    /// CR 702.33d/e, 121.2: declared kicker gates a fixed quantity of
    /// ordinary sequential controller draws, without a resolution-time option.
    DrawCardsIfAdditionalCost {
        cost_identity: String,
        count: Quantity,
        source_block: u32,
    },
    DrawCards {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipient: Option<LifeRecipient>,
        count: Quantity,
        source_block: u32,
    },
    /// CR 121.2c: each player finishes their draws before the next player in turn order.
    DrawEachPlayer {
        count: DrawEachCount,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    DrawAnnouncedX {
        #[serde(deserialize_with = "announced_x_draw::deserialize_recipient")]
        #[schemars(with = "LifeRecipient")]
        recipient: LifeRecipient,
        source_block: u32,
    },
    DealDamageAnnouncedX {
        #[schemars(range(max = 0))]
        target_slot: u32,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    MillAnnouncedX {
        #[schemars(range(max = 0))]
        target_slot: u32,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    ChangeLifeAnnouncedX {
        #[serde(deserialize_with = "announced_x_draw::deserialize_recipient")]
        #[schemars(with = "LifeRecipient")]
        recipient: LifeRecipient,
        gain: bool,
        source_block: u32,
    },
    /// CR 118.12a and 603.5: the event caster may pay before the controller may draw.
    OptionalDrawUnlessCasterPays {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        generic: Quantity,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    UntapControllerLands {
        source_block: u32,
    },
    /// CR 603.5/608.2d: the controller chooses at resolution whether to draw
    /// one card, including when their library is empty.
    OptionalDrawForSoleGreatestEntrant {
        source_block: u32,
    },
    OptionalDrawCards {
        source_block: u32,
    },
    /// Draw once per unit of the triggering actual damage amount (CR 121.2).
    DrawDamageAmount {
        source_block: u32,
    },
    DiscardPlayerSet {
        scope: GroupDiscardScope,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        count: Quantity,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    DiscardTargetPlayer {
        #[schemars(range(max = 0))]
        target_slot: u32,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        count: Quantity,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    DiscardTargetPlayerAtRandom {
        #[schemars(range(max = 0))]
        target_slot: u32,
        count: RandomDiscardCount,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    DiscardCards {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipient: Option<ActionRecipient>,
        count: Quantity,
        source_block: u32,
    },
    /// CR 608.2b/c/h, 701.6 and 701.17. Mill the target spell's controller
    /// even if the legal target cannot be countered.
    CounterSpellAndMillController {
        #[schemars(range(max = 0))]
        target_slot: u32,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        count: Quantity,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    /// CR 118.12a: the targeted spell's controller may pay during resolution.
    CounterSpellUnlessPays {
        #[schemars(range(max = 0))]
        target_slot: u32,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        generic: Quantity,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    CounterStackObject {
        target_slot: u32,
        source_block: u32,
    },
    /// Copy one legal controlled creature spell as nonlegendary, with its
    /// original decisions. CR 608.2b, 707.9, 707.10 and 707.10f.
    CopyControlledCreatureSpell {
        #[schemars(range(max = 0))]
        target_slot: u32,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    /// Noncombat damage from this invocation's physical source (CR 120.2b).
    DealDamage {
        target_slot: u32,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
    /// Untargeted noncombat damage from the source to the trigger's controller.
    DealDamageToController {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
    GainLife {
        recipient: LifeRecipient,
        #[schemars(with = "String", regex(pattern = r"^(0|[1-9][0-9]*)$"))]
        amount: Quantity,
        source_block: u32,
    },
    /// Each opponent loses the amount, then the controller gains the total life actually lost.
    DrainOpponents {
        amount: OpponentDrainAmount,
        #[schemars(range(max = 0))]
        source_block: u32,
    },
    LoseLife {
        recipient: LifeRecipient,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
}
impl Instruction {
    /// Compiler-owned complete source block for this instruction.
    pub fn source_block(&self) -> u32 {
        match self {
            Self::LookTopMayRevealForMana { source_block, .. }
            | Self::RegisterControllerCreatureEntryThisTurn { source_block, .. }
            | Self::Hideaway { source_block, .. }
            | Self::PlayHideawayLinked { source_block, .. }
            | Self::GrantHandCastCascade { source_block }
            | Self::OfferHandCastWithoutManaCost { source_block }
            | Self::Cascade { source_block, .. }
            | Self::RevealAndCast { source_block }
            | Self::Demonstrate { source_block }
            | Self::SearchBasicLandsSplit { source_block, .. }
            | Self::SearchColoredLibrary { source_block, .. }
            | Self::SearchTypedLands { source_block, .. }
            | Self::PutSourcePlusOneCountersForCastColors { source_block, .. }
            | Self::DrawCardsIfAdditionalCost { source_block, .. } => *source_block,
            Self::DrawControllerCards { source_block, .. }
            | Self::GrantLandPlaysThisTurn { source_block, .. }
            | Self::PutHandOnLibrary { source_block, .. } => *source_block,
            Self::OrderLibraryTop { source_block, .. }
            | Self::ShufflePlayerGraveyard { source_block }
            | Self::OptionalShuffleLibrary { source_block }
            | Self::SelectLibraryCard { source_block, .. }
            | Self::Scry { source_block, .. }
            | Self::Surveil { source_block, .. }
            | Self::MillCards { source_block, .. } => *source_block,
            Self::ExileResolvingSpell { source_block }
            | Self::ProtectControllerUntilNextTurn { source_block }
            | Self::FreezeControllerLifeUntilNextTurn { source_block }
            | Self::PhaseOutControllerPermanents { source_block }
            | Self::PhaseOutTargetPermanent { source_block, .. }
            | Self::MoveSet { source_block, .. } => *source_block,
            Self::RemoveOpponentNonlandIndestructible { source_block } => *source_block,
            Self::DestroySet { source_block, .. } => *source_block,
            Self::PumpSet { source_block, .. } => *source_block,
            Self::DamageSet { source_block, .. } => *source_block,
            Self::GainLifeDestroyed { source_block, .. } => *source_block,
            Self::ExileDamagedThisTurn { source_block, .. } => *source_block,
            Self::EachPlayerSacrifice { source_block, .. } => *source_block,
            Self::LoseLifeDeathController { source_block, .. } => *source_block,
            Self::SacrificeDamage { source_block, .. } => *source_block,
            Self::EachOpponentSacrifice { source_block, .. } => *source_block,
            Self::ModifySourcePowerToughness { source_block, .. } => *source_block,
            Self::DestroyPermanent { source_block, .. } => *source_block,
            Self::ExilePermanent { source_block, .. } => *source_block,
            Self::UntapPermanent { source_block, .. } => *source_block,
            Self::TapPermanent { source_block, .. } => *source_block,
            Self::ChangeLifeFromTarget { source_block, .. } => *source_block,
            Self::CreateTokensFor { source_block, .. } => *source_block,
            Self::DamageTargetController { source_block, .. } => *source_block,
            Self::AttachEquipment { source_block, .. } => *source_block,
            Self::ReturnAuraFromGraveyard { source_block, .. } => *source_block,
            Self::DestroyTargetCreature { source_block, .. } => *source_block,
            Self::DestroyAllCreatures { source_block, .. } => *source_block,
            Self::ReturnOwnGraveyardCreatureToBattlefield { source_block, .. }
            | Self::ReturnOwnGraveyardCreatureToHand { source_block, .. } => *source_block,
            Self::ReturnOwnGraveyardCardToHand { source_block, .. } => *source_block,
            Self::SearchBasicLands { source_block, .. } => *source_block,
            Self::CreateCreatureTokens { source_block, .. } => *source_block,
            Self::DistributePlusOneCounters { source_block, .. }
            | Self::PutPlusOneCounters { source_block, .. } => *source_block,
            Self::GrantKeywords { source_block, .. }
            | Self::GrantSourceKeywords { source_block, .. } => *source_block,
            Self::GrantKeywordsSet { source_block, .. } => *source_block,
            Self::ModifyPowerToughness { source_block, .. } => *source_block,
            Self::PreventCombatDamage { source_block, .. } => *source_block,
            Self::PreventNextDamage { source_block, .. } => *source_block,
            Self::ExileTargetGraveyard { source_block, .. }
            | Self::ReturnEntryTargetCreature { source_block, .. }
            | Self::ReturnToOwnerHand { source_block, .. } => *source_block,
            Self::MoveTriggeredSuccessor { source_block, .. }
            | Self::PutOnTopOwnerLibrary { source_block, .. } => *source_block,
            Self::LoseGameTargetPlayer { source_block, .. }
            | Self::TargetPlayerSacrificeCreature { source_block, .. } => *source_block,
            Self::SacrificeCreature { source_block, .. } => *source_block,
            Self::DrawEachPlayer { source_block, .. }
            | Self::DrawAnnouncedX { source_block, .. }
            | Self::DealDamageAnnouncedX { source_block, .. }
            | Self::MillAnnouncedX { source_block, .. }
            | Self::ChangeLifeAnnouncedX { source_block, .. }
            | Self::DrawCards { source_block, .. } => *source_block,
            Self::UntapControllerLands { source_block } => *source_block,
            Self::OptionalDrawForSoleGreatestEntrant { source_block }
            | Self::OptionalDrawCards { source_block, .. }
            | Self::OptionalDrawUnlessCasterPays { source_block, .. } => *source_block,
            Self::DrawDamageAmount { source_block, .. } => *source_block,
            Self::DiscardPlayerSet { source_block, .. }
            | Self::DiscardTargetPlayer { source_block, .. }
            | Self::DiscardTargetPlayerAtRandom { source_block, .. }
            | Self::DiscardCards { source_block, .. } => *source_block,
            Self::CounterSpellAndMillController { source_block, .. }
            | Self::CounterSpellUnlessPays { source_block, .. }
            | Self::CounterStackObject { source_block, .. }
            | Self::CopyControlledCreatureSpell { source_block, .. } => *source_block,
            Self::DealDamage { source_block, .. }
            | Self::DealDamageToController { source_block, .. } => *source_block,
            Self::GainLife { source_block, .. } => *source_block,
            Self::DrainOpponents { source_block, .. } | Self::LoseLife { source_block, .. } => {
                *source_block
            }
        }
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
    ControllerCreaturePlusOneCounterReplacement {
        modification: CounterPlacementModification,
    },
    DoubleDamageToPermanentOrPlayer,
    /// Double damage from creatures controlled by this battlefield source's controller.
    DoubleControllerCreatureDamageToPermanentOrPlayer,
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
#[serde(rename_all = "snake_case")]
pub enum CreatureScope {
    Equipped,
    Enchanted,
    Controller,
    /// Dynamic attacking creatures controlled by this battlefield source.
    AttackingController,
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
fn fixed_artifact_sacrifice_effect(p: &SpellProgram) -> bool {
    if p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
    {
        return false;
    }
    match (p.targets.as_slice(), p.instructions.as_slice()) {
        (
            [],
            [
                Instruction::DrawCards {
                    count,
                    recipient: None,
                    ..
                },
            ],
        ) => *count == Quantity::from(1),
        (
            [target],
            [
                Instruction::DealDamage {
                    target_slot: 0,
                    amount,
                    ..
                },
            ],
        ) => {
            target.slot == 0
                && matches!(
                    target.domain,
                    TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker
                )
                && target.stage == EvaluationStage::Announcement
                && target.chooser == Chooser::Controller
                && target.filter.is_none()
                && *amount > Quantity::zero()
        }
        (
            [],
            [
                Instruction::GainLife {
                    recipient: LifeRecipient::Controller,
                    amount,
                    ..
                },
            ],
        ) => *amount > Quantity::zero(),
        (
            [t],
            [
                Instruction::ModifyPowerToughness {
                    target_slot: 0,
                    duration: Duration::UntilEndOfTurn,
                    ..
                },
            ],
        ) => {
            t.slot == 0
                && t.domain == TargetDomain::BattlefieldCreature
                && t.stage == EvaluationStage::Announcement
                && t.chooser == Chooser::Controller
                && t.filter.is_none()
        }
        _ => false,
    }
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
    GreatestControlledPowerCombination,
    Fixed {
        symbols: Vec<Color>,
    },
    AnyColor,
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
    DelayedControllerCreatureEntry,
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
    /// The exact source incarnation actually changes from untapped to tapped.
    SelfBecomesTapped,
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
            Self::All { terms } => terms.iter().all(|t| t.matches(c)),
            Self::AnyOf { terms } => terms.iter().any(|t| t.matches(c)),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        fn visit(p: &TypePredicate, depth: usize, remaining: &mut usize) -> bool {
            if depth > 8 || *remaining == 0 {
                return false;
            }
            *remaining -= 1;
            match p {
                TypePredicate::SpellColor { .. } => false,
                TypePredicate::All { terms } | TypePredicate::AnyOf { terms } => {
                    !terms.is_empty() && terms.iter().all(|t| visit(t, depth + 1, remaining))
                }
                _ => true,
            }
        }
        if visit(self, 0, &mut 32) {
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
            Self::SagaChapter { .. } => "saga_chapter",
            Self::SelfSpellCast => "self_spell_cast",
            Self::SpellCast { .. } | Self::SpellCastWithOrigin { .. } => "spell_cast",
            Self::OpponentCreatureDies => "opponent_creature_dies",
            Self::SelfEntersBattlefield => "self_enters_battlefield",
            Self::OtherCreatureEntersBattlefield => "other_creature_enters_battlefield",
            Self::ControlledCreatureEntersWithPower { .. } => {
                "controlled_creature_enters_with_power"
            }
            Self::AnyCreatureDies => "any_creature_dies",
            Self::SelfDies => "self_dies",
            Self::SelfBecomesTapped => "self_becomes_tapped",
            Self::SelfAuraToGraveyard => "self_aura_to_graveyard",
            Self::SelfMoved { .. } => "self_moved",
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
    !(power.is_negative()
        || toughness.is_negative()
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
        || keywords.iter().any(|k| k.basic_landwalk_type().is_some())
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
pub fn canonical_cascade_program(source_block: u32) -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![Instruction::Cascade {
            threshold: CascadeThreshold::SourceSpellManaValue,
            source_block,
        }],
        modes: vec![],
    }
}
pub fn is_canonical_cascade_program(program: &SpellProgram) -> bool {
    matches!(program.instructions.as_slice(), [Instruction::Cascade { source_block, .. }]
        if *program == canonical_cascade_program(*source_block))
}
impl Definition {
    pub fn requirements(&self) -> BTreeSet<Capability> {
        let mut set = BTreeSet::from([
            if self
                .triggered
                .iter()
                .any(|t| matches!(t.condition, TriggerCondition::SagaChapter { .. }))
            {
                Capability::SagaChaptersV1
            } else {
                Capability::NormalLayoutV1
            },
        ]);
        if !self.characteristics.hybrid_mana.is_empty() {
            set.insert(Capability::ColorHybridCostV1);
            if self
                .characteristics
                .hybrid_mana
                .iter()
                .any(|s| s.colors.contains(&HybridPayment::PayTwoLife))
            {
                set.insert(Capability::PhyrexianCostV1);
            }
        }
        if self
            .mana_abilities
            .iter()
            .any(|a| a.production == ManaProduction::GreatestControlledPowerCombination)
        {
            set.insert(Capability::GreatestPowerV1);
        }
        if self.characteristics.snow {
            set.insert(Capability::SnowSupertypeV1);
        }

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
        if self
            .characteristics
            .subtypes
            .iter()
            .any(|s| s == "Equipment" || s == "Aura")
        {
            set.insert(Capability::AttachmentsV1);
        }
        if let Some(program) = &self.spell {
            add_program_requirements(&mut set, program);
        }
        if !self.mana_abilities.is_empty() {
            set.insert(Capability::PermanentManaV1);
        }
        if opponent_land_mana_family(self) {
            set.insert(Capability::OpponentLandManaV1);
        }
        if self.mana_abilities.iter().any(|ability| {
            matches!(
                ability.production,
                ManaProduction::EachPermanentColor { .. }
            )
        }) {
            set.insert(Capability::DistinctPermanentColorsV1);
        }
        if listed_mana_choice::present(self) {
            set.insert(Capability::ListedManaChoiceV1);
        }
        if disjoint_pain_mana_family(self) {
            set.extend([
                Capability::DisjointPainManaV1,
                Capability::NoncombatDamageV1,
            ]);
        }
        if self
            .mana_abilities
            .iter()
            .any(|a| a.costs.remove_source_plus_one_counters.is_some() || a.restriction.is_some())
        {
            set.insert(Capability::CounterRemovalManaV1);
        }
        for ability in &self.activated {
            set.insert(Capability::ActivatedAbilityV1);
            if ability.functioning_zone == FunctioningZone::Hand {
                set.insert(Capability::HandActivationV1);
            }
            if matches!(ability.cost, ActivationCost::DiscardSource { .. }) {
                set.insert(Capability::ManaActivationCostV1);
            }
            if activated_artifact_draw_ability(ability) {
                set.insert(Capability::ActivatedArtifactDrawV1);
            }
            if has_untap(&ability.program) {
                set.insert(Capability::TapSourceUntapCreatureV1);
            }
            if has_tap(&ability.program) {
                set.insert(Capability::ActivatedTapPermanentV1);
            }
            if has_counter_stack_object(&ability.program) {
                set.insert(Capability::ActivatedSpellCounterV1);
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
            if ability.cost == ActivationCost::SacrificeSource
                && ability
                    .program
                    .instructions
                    .iter()
                    .any(|i| matches!(i, Instruction::SearchBasicLands { .. }))
            {
                set.insert(Capability::SacrificeV1);
            }
            add_program_requirements_for_activation(
                &mut set,
                &ability.program,
                fixed_creature_activation::mana_self_pump(ability),
            );
        }
        if land_sacrifice_draw::present(self) {
            set.insert(Capability::LandSacrificeDrawV1);
        }
        if land_entry_gain_life::present(self) {
            set.insert(Capability::LandEntryGainLifeV1);
        }
        for ability in &self.triggered {
            if ability.death_filter.is_some()
                || (ability.condition == TriggerCondition::SelfDies
                    && ability
                        .program
                        .instructions
                        .iter()
                        .any(|i| matches!(i, Instruction::CreateCreatureTokens { .. })))
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
                TriggerCondition::SagaChapter { .. } => Capability::SagaChaptersV1,
                TriggerCondition::SelfSpellCast
                    if is_canonical_cascade_program(&ability.program) =>
                {
                    Capability::CascadeV1
                }
                TriggerCondition::SpellCast {
                    spell: TypePredicate::SpellColor { .. },
                    ..
                } => {
                    set.insert(Capability::CastColorPredicateV1);
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
                TriggerCondition::SelfBecomesTapped => Capability::SourceTapTriggerV1,
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
            if let StaticEffect::ModifyCreatures {
                scope: CreatureScope::Enchanted,
                keywords,
                ..
            } = &a.effect
                && bestow_alternative(self).is_some()
            {
                set.extend(
                    keywords
                        .iter()
                        .filter_map(|k| intrinsic_keyword_capability(*k)),
                );
            }
            if let StaticEffect::Bestow { program, .. } = &a.effect {
                add_program_requirements(&mut set, program);
            }
            if let StaticEffect::GrantSpellCascade {
                eligibility,
                program,
            } = &a.effect
            {
                add_program_requirements(&mut set, program);
                if eligibility.requires_history() {
                    set.insert(Capability::CastHistoryV1);
                }
            }
            if matches!(&a.effect, StaticEffect::ModifyCreatures { keywords, .. }
                if keywords.contains(&KeywordAbility::Indestructible))
            {
                set.insert(Capability::IndestructibleV1);
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
                StaticEffect::ModifyCreatures { .. } => Capability::AttachmentsV1,
            };
            set.insert(static_capability);
        }
        set
    }
    pub fn validate(&self) -> Result<(), String> {
        saga::validate(self)?;
        let fixed_activations = fixed_creature_activation::validate(self)?;
        cast_entry::validate_source(self, fixed_activations)?;
        optional_costs::validate_definition(self)?;
        typed_land_search::validate_definition(self)?;
        activated_source_keyword::validate(self)?;
        conditional_lands::validate_definition(self)?;
        cascade_land_entry::validate_definition(self)?;
        entry_copy::validate_definition(self)?;
        let has_distinct_permanent_colors = distinct_permanent_colors::validate_definition(self)?;
        // Every occurrence retains its own u32 ability index, including repeated
        // keywords in one source block (CR 702.85c). No per-card instance cap.
        u32::try_from(self.triggered.len()).map_err(|_| "trigger ability index limit")?;
        u32::try_from(self.static_abilities.len()).map_err(|_| "static ability index limit")?;
        controller_library::validate_definition(self)?;
        counter_spell_controller_mill::validate_definition(self)?;
        fixed_mill::validate_definition(self)?;
        announced_x_draw::validate_definition(self)?;
        targeted_counted_draw::validate_definition(self)?;
        targeted_discard::validate_definition(self)?;
        artifact_modal::validate(self)?;
        set_exile::validate(self)?;
        each_player_draw::validate_definition(self)?;
        group_discard::validate_definition(self)?;
        opponent_life_drain::validate_definition(self)?;
        targeted_game_loss::validate_definition(self)?;
        controlled_phasing::validate(self)?;
        counter_unless_generic::validate_definition(self)?;
        controlled_creature_spell_copy::validate_definition(self)?;
        creative_technique::validate_definition(self)?;
        self_graveyard_shuffle::validate_definition(self)?;
        library_selection::validate_definition(self)?;
        library_order::validate_definition(self)?;
        hand_return::validate_definition(self)?;
        unlimited_hand_size::validate_definition(self)?;
        hand_size_adjustments::validate_definition(self)?;
        power_entry_draw::validate_definition(self)?;
        greatest_power::validate(self)?;
        power_filtered_removal::validate_definition(self)?;
        creature_entry_return::validate_definition(self)?;
        entry_graveyard_exile::validate_definition(self)?;
        land_entry_gain_life::validate(self)?;
        other_land_entry::validate(self)?;
        let land_sacrifice_draw = land_sacrifice_draw::validate(self)?;
        bestow::validate(self)?;
        hand_cast::validate(self)?;
        cast_mana_value_sweep::validate_definition(self)?;
        hand_cast_cascade::validate_definition(self)?;
        linked_hideaway::validate_definition(self)?;
        if self
            .static_abilities
            .iter()
            .any(|a| a.effect == StaticEffect::ControlEnchantedPermanent)
            && !aura_control_source(self)
        {
            return Err("unsupported complete Aura control family".into());
        }
        if self.spell.as_ref().is_some_and(|p| {
            temporary_set_keyword_program(p).is_some()
                || temporary_permanent_keyword_program(p).is_some()
        }) && (!self.keywords.is_empty()
            || !self.static_abilities.is_empty()
            || !self.activated.is_empty()
            || !self.triggered.is_empty()
            || !self.mana_abilities.is_empty()
            || self.x_mana_cost)
        {
            return Err("set keyword grant requires a complete ordinary spell definition".into());
        }
        if self
            .activated
            .iter()
            .any(|a| has_keyword_grant(&a.program) && !sacrifice_set::definition(self))
            || self.triggered.iter().any(|a| has_keyword_grant(&a.program))
            || self.spell.as_ref().is_some_and(|p| {
                has_keyword_grant(p)
                    && (!matches!(
                        self.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    ) || !(ordinary_keyword_grant(p) || akromas_will_program(p)))
            })
        {
            return Err("unsupported complete temporary keyword definition".into());
        }
        if self
            .triggered
            .iter()
            .any(|a| has_permanent_owner_hand(&a.program))
            || self.spell.as_ref().is_some_and(|p| {
                has_permanent_owner_hand(p)
                    && (!matches!(
                        self.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    ) || !p.modes.is_empty())
            })
        {
            return Err("permanent owner-hand movement requires an ordinary nonmodal spell".into());
        }
        if self.triggered.iter().any(|a| has_tap(&a.program))
            || self.spell.as_ref().is_some_and(|p| {
                has_tap(p)
                    && (!matches!(
                        self.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    ) || !ordinary_tap_program(p))
            })
        {
            return Err("tap requires an ordinary nonmodal spell".into());
        }
        if self
            .activated
            .iter()
            .any(|a| has_untap(&a.program) && !tap_source_untap_ability(a))
            || self.triggered.iter().any(|a| has_untap(&a.program))
            || self.spell.as_ref().is_some_and(|p| {
                has_untap(p)
                    && (!matches!(
                        self.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    ) || !ordinary_untap_program(p))
            })
        {
            return Err("untap requires an ordinary nonmodal spell".into());
        }
        if self
            .activated
            .iter()
            .any(|a| has_blocking_creature_target(&a.program))
            || self
                .triggered
                .iter()
                .any(|a| has_blocking_creature_target(&a.program))
            || self.spell.as_ref().is_some_and(|p| {
                has_blocking_creature_target(p)
                    && (!matches!(
                        self.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    ) || !p.modes.is_empty())
            })
        {
            return Err("blocking creature targets require an ordinary nonmodal spell pump".into());
        }
        if self.characteristics.snow
            && (self.characteristics.kind != CardKind::Creature || !self.mana_abilities.is_empty())
        {
            return Err("snow support requires a non-mana creature".into());
        }
        if self.spell.as_ref().is_some_and(has_damage_quantity_draw)
            || self
                .activated
                .iter()
                .any(|a| has_damage_quantity_draw(&a.program))
            || self.triggered.iter().any(|a| {
                has_damage_quantity_draw(&a.program)
                    && !matches!(a.condition, TriggerCondition::DamageDealt { .. })
            })
        {
            return Err("damage-quantity draw requires a bound damage trigger".into());
        }
        if self.triggered.iter().any(|a| {
            has_controller_discard(&a.program)
                && !matches!(a.condition, TriggerCondition::DamageDealt { .. })
        }) {
            return Err("controller discard trigger requires a damage trigger".into());
        }
        fn damage_recipient_action(p: &SpellProgram) -> bool {
            p.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::DiscardCards {
                        recipient: Some(ActionRecipient::EventPlayer),
                        ..
                    } | Instruction::SacrificeCreature { .. }
                )
            }) || p.modes.iter().any(|m| damage_recipient_action(&m.program))
        }
        if self.spell.as_ref().is_some_and(damage_recipient_action)
            || self
                .activated
                .iter()
                .any(|a| damage_recipient_action(&a.program))
            || self.triggered.iter().any(|a| {
                damage_recipient_action(&a.program)
                    && !matches!(a.condition, TriggerCondition::DamageDealt { .. })
            })
        {
            return Err("event-player discard/sacrifice requires a bound damage trigger".into());
        }
        fn cast_colors(p: &SpellProgram) -> bool {
            p.instructions
                .iter()
                .any(|i| matches!(i, Instruction::PutSourcePlusOneCountersForCastColors { .. }))
                || p.modes.iter().any(|m| cast_colors(&m.program))
        }
        if self.spell.as_ref().is_some_and(cast_colors)
            || self.activated.iter().any(|a| cast_colors(&a.program))
            || self.triggered.iter().any(|a| {
                cast_colors(&a.program)
                    && (self.characteristics.kind != CardKind::Creature
                        || a.condition
                            != (TriggerCondition::SpellCast {
                                caster: CasterRelation::Controller,
                                spell: TypePredicate::Any {},
                            })
                        || a.intervening_if.is_some()
                        || !a.program.targets.is_empty()
                        || !a.program.modes.is_empty()
                        || !matches!(
                            a.program.instructions.as_slice(),
                            [Instruction::PutSourcePlusOneCountersForCastColors { .. }]
                        ))
            })
        {
            return Err(
                "cast-color counters require a standalone bound controller cast trigger".into(),
            );
        }
        fn event_player(p: &SpellProgram) -> bool {
            p.instructions.iter().any(Instruction::uses_event_player)
                || p.modes.iter().any(|m| event_player(&m.program))
        }
        if self.spell.as_ref().is_some_and(event_player)
            || self.activated.iter().any(|a| event_player(&a.program))
            || self.triggered.iter().any(|a| {
                event_player(&a.program)
                    && !matches!(
                        a.condition,
                        TriggerCondition::StepBegan { .. } | TriggerCondition::SpellCast { .. }
                    )
            })
        {
            return Err(
                "event-player recipient requires a bound step trigger or cast trigger".into(),
            );
        }
        fn new_ops(p: &SpellProgram) -> bool {
            p.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::CreateCreatureTokens { .. }
                        | Instruction::PutPlusOneCounters { .. }
                )
            }) || p.modes.iter().any(|m| new_ops(&m.program))
        }
        if self.activated.iter().any(|a| {
            new_ops(&a.program) && !matches!(a.cost, ActivationCost::SacrificePermanent { .. })
        }) || self.triggered.iter().any(|a| {
            new_ops(&a.program)
                && a.condition != TriggerCondition::SelfEntersBattlefield
                && a.condition != TriggerCondition::SelfDies
                && !matches!(
                    a.condition,
                    TriggerCondition::StepBegan { .. } | TriggerCondition::SpellCast { .. }
                )
                && !matches!(
                    a.condition,
                    TriggerCondition::CreatureAttacked { .. }
                        | TriggerCondition::DamageDealt { .. }
                )
                && a.death_filter.is_none()
        }) {
            return Err(
                "tokens/counters require a supported spell, activation or event trigger".into(),
            );
        }
        // A spell's own source is the spell on the stack, never a creature; a
        // "this creature" counter recipient exists only in self-enters triggers.
        fn source_counters(p: &SpellProgram) -> bool {
            p.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::PutPlusOneCounters {
                        on: CounterRecipient::Source,
                        ..
                    }
                )
            }) || p.modes.iter().any(|m| source_counters(&m.program))
        }
        if self.spell.as_ref().is_some_and(source_counters) {
            return Err("source counter recipient requires a self-enters trigger".into());
        }
        if self.spell.as_ref().is_some_and(|p| {
            p.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::AttachEquipment { .. }
                        | Instruction::ReturnAuraFromGraveyard { .. }
                )
            })
        }) {
            return Err("attachment operations are not spell instructions".into());
        }
        fn has_cascade(p: &SpellProgram) -> bool {
            p.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Cascade { .. }))
                || p.modes.iter().any(|m| has_cascade(&m.program))
        }
        if self.spell.as_ref().is_some_and(has_cascade)
            || self.activated.iter().any(|a| has_cascade(&a.program))
        {
            return Err("cascade is only a self-cast triggered program".into());
        }
        if self
            .spell
            .as_ref()
            .is_some_and(successor_tuck::contains_move)
            || self
                .activated
                .iter()
                .any(|a| successor_tuck::contains_move(&a.program))
        {
            return Err("successor movement requires its matching self-move trigger".into());
        }
        // Sacrifice programs admit enchantment programs and creature statics.
        // Other programs retain their narrower kind constraints.
        let opening_composition = opening_placement::validate_composition(self)?;
        if !opening_composition {
            opening_placement::validate(self)?;
            controller_land_types::validate(self)?;
        }
        let sacrifice = self.capabilities.contains(&Capability::SacrificeV1);
        let wipes = self.capabilities.contains(&Capability::BoardWipesV1);
        if self.x_mana_cost
            && !matches!(
                self.characteristics.kind,
                CardKind::Instant | CardKind::Sorcery
            )
        {
            return Err("X requires a spell".into());
        }
        if self.x_mana_cost
            != self.spell.as_ref().is_some_and(|p| {
                announced_x_draw::has_x_instruction(p)
                    || p.instructions.iter().any(|i| {
                        matches!(
                            i,
                            Instruction::DamageSet {
                                amount: WipeAmount::AnnouncedX,
                                ..
                            } | Instruction::DrawEachPlayer {
                                count: DrawEachCount::AnnouncedX,
                                ..
                            } | Instruction::DrainOpponents {
                                amount: OpponentDrainAmount::AnnouncedX,
                                ..
                            } | Instruction::DiscardTargetPlayerAtRandom {
                                count: RandomDiscardCount::AnnouncedX,
                                ..
                            }
                        )
                    })
            })
        {
            return Err("unbound mana X".into());
        }
        let c = &self.characteristics;
        // Combined types use one canonical execution type and complete type
        // membership everywhere filters apply. No land-creature casting path
        // or mixed spell/permanent type is implemented yet.
        if c.additional_types
            .iter()
            .any(|k| !matches!(k, CardKind::Artifact | CardKind::Enchantment) || *k == c.kind)
            || c.additional_types.windows(2).any(|w| w[0] >= w[1])
            || (!c.additional_types.is_empty()
                && !matches!(
                    c.kind,
                    CardKind::Creature | CardKind::Land | CardKind::Artifact
                ))
        {
            return Err("unsupported or noncanonical combined card types".into());
        }
        for subtype in &c.subtypes {
            if unsupported_rules_subtype(subtype)
                && !(subtype == "Saga" && self.capabilities.contains(&Capability::SagaChaptersV1))
            {
                return Err(format!("subtype:{}", subtype.to_lowercase()));
            }
        }
        let equipment = c.kind == CardKind::Artifact && c.subtypes == ["Equipment"];
        let aura = c.kind == CardKind::Enchantment && c.subtypes == ["Aura"];
        if (c.subtypes.iter().any(|s| s == "Equipment") && !equipment)
            || (c.subtypes.iter().any(|s| s == "Aura") && !aura)
        {
            return Err("unsupported attachment subtype combination".into());
        }
        if self
            .keywords
            .iter()
            .any(|k| !matches!(k, KeywordAbility::Flash | KeywordAbility::Indestructible))
            && c.kind != CardKind::Creature
        {
            return Err("keywords require creature characteristics".into());
        }
        if self
            .keywords
            .contains(&KeywordAbility::ProtectionFromEachColor)
        {
            return Err("protection from each color requires the commander modal grant".into());
        }
        if self.keywords.contains(&KeywordAbility::Flash) && c.kind == CardKind::Land {
            return Err("flash on lands is outside supported casting semantics".into());
        }
        if self.keywords.contains(&KeywordAbility::Indestructible)
            && !matches!(
                c.kind,
                CardKind::Creature | CardKind::Artifact | CardKind::Enchantment | CardKind::Land
            )
        {
            return Err("indestructible requires permanent characteristics".into());
        }
        let cast_timing =
            if c.kind == CardKind::Instant || self.keywords.contains(&KeywordAbility::Flash) {
                SpellTiming::Instant
            } else {
                SpellTiming::MainPhase
            };
        if self.oracle_id.is_empty()
            || self.name.is_empty()
            || c.mana_cost.generic.is_negative()
            || c.colors.contains(&Color::C)
            || c.color_identity.contains(&Color::C)
            || self.capabilities != self.requirements()
        {
            return Err("invalid identity, characteristics, or capability closure".into());
        }
        if !(accepted_provenance(
            &self.provenance.archive_manifest_sha256,
            &self.provenance.parser_sha256,
        ) || library_top_mana::authenticated_reparse(&self.provenance))
            || sha256(self.provenance.source_json.as_bytes()) != self.provenance.source_sha256
            || sha256(format!("[{}]", self.provenance.abilities_json).as_bytes())
                != self.provenance.ast_sha256
        {
            return Err("invalid source/AST provenance".into());
        }
        basic_landwalk::validate_definition(self)?;
        source_combat_restrictions::validate_definition(self)?;
        creature_planeswalker_return::validate(self)?;
        fixed_creature_damage::validate(self, fixed_activations)?;
        controller_creature_pump::validate(self)?;
        cycling::validate_spell_source(self)?;
        modal_targets::validate_definition(self)?;
        graveyard_shuffle::validate_definition(self)?;
        graveyard_battlefield_return::validate_definition(self)?;
        nonland_exile_life::validate_definition(self)?;
        colored_library_search::validate_definition(self)?;
        own_graveyard_card_return::validate_definition(self)?;
        commander_modal_keyword::validate_definition(self)?;
        cast_payer_draw::validate_definition(self)?;
        self_spell_protection::validate_definition(self)?;
        let library_top = library_top_mana::validate(self)?;
        if !library_top {
            unspent_mana::validate(self)?;
        }
        granted_land_mana::validate_definition(self)?;
        color_hybrid_cost::validate_definition(self, opening_composition, library_top)?;
        keyword_removal_wipe::validate_definition(self)?;
        opponent_nonland_destroy::validate_definition(self)?;
        source_tap_damage::validate_definition(self)?;
        source_dies_nonland_destroy::validate_definition(self)?;
        if let Some(labels) = &self.source_labels {
            let source: serde_json::Value = strict_read(&self.provenance.source_json)?;
            if source["oracle_text"].as_str() != Some(labels.text.as_str())
                || labels.blocks.iter().any(|s| s.is_empty())
                || labels
                    .activated
                    .iter()
                    .chain(&labels.triggered)
                    .chain(&labels.mana)
                    .chain(&labels.statics)
                    .any(|b| *b as usize >= labels.blocks.len())
            {
                return Err("invalid source label provenance".into());
            }
        }
        if self
            .mana_abilities
            .iter()
            .any(|ability| ability.production == ManaProduction::OpponentLandColors)
            && !opponent_land_mana_family(self)
        {
            return Err("unsupported complete opponent-land mana family".into());
        }
        if self.mana_abilities.len() > 1
            || self
                .mana_abilities
                .iter()
                .any(|ability| ability.self_damage.is_some())
        {
            if !disjoint_pain_mana_family(self) {
                return Err("unsupported complete disjoint pain-mana family".into());
            }
        } else {
            listed_mana_choice::validate(self)?;
        }
        for a in &self.mana_abilities {
            if !matches!(
                c.kind,
                CardKind::Creature | CardKind::Artifact | CardKind::Land
            ) || (!a.costs.tap && a.costs.remove_source_plus_one_counters.is_none())
                || matches!(&a.production, ManaProduction::Fixed { symbols } if symbols.is_empty())
            {
                return Err("unsupported permanent mana ability".into());
            }
            // Reversing a sacrificed mana source (CR 733) must go through the
            // ordinary zone-move path; until it does, no such ability is admitted.
            if a.costs.sacrifice {
                return Err("sacrifice mana costs are not admitted".into());
            }
            validate_permanent_costs(&a.costs)?;
            if a.costs.remove_source_plus_one_counters.is_some() || a.restriction.is_some() {
                if c.kind != CardKind::Creature
                    || a.costs.tap
                    || a.costs.mana.is_some()
                    || a.costs
                        .remove_source_plus_one_counters
                        .as_ref()
                        .is_none_or(|n| *n <= Quantity::zero())
                    || a.restriction != Some(ManaActivationRestriction::OnceEachTurn)
                    || !matches!(&a.production, ManaProduction::Fixed { symbols } if !symbols.is_empty())
                {
                    return Err("unsupported counter-removal mana/restriction shape".into());
                }
            }
        }
        if !self.activated.is_empty()
            && !matches!(c.kind, CardKind::Creature | CardKind::Artifact)
            && !(sacrifice && c.kind == CardKind::Enchantment)
            && !typed_land_search::activation_definition(self)
            && !self.capabilities.contains(&Capability::LinkedExilePlayV1)
            && !cycling::definition(self)
            && !paid_counted_draw::definition_matches(self)
            && !land_sacrifice_draw
        {
            return Err("activated programs require supported creature characteristics".into());
        }
        for ability in &self.activated {
            if let ActivationCost::SacrificePermanent {
                count,
                filter,
                mana,
            } = &ability.cost
                && *count != 1
            {
                let p = &ability.program;
                if *count < 2
                    || filter.types != [CardKind::Artifact]
                    || !filter.subtypes.is_empty()
                    || mana
                        .as_ref()
                        .is_some_and(|m| m.generic.is_negative() || m.colored.contains(&Color::C))
                    || !matches!(c.kind, CardKind::Creature | CardKind::Artifact)
                    || equipment
                    || ability.functioning_zone != FunctioningZone::Battlefield
                    || p.functioning_zone != FunctioningZone::Stack
                    || p.timing != SpellTiming::Instant
                    || p.resolution != SpellResolution::Instructions
                    || p.additional_sacrifice.is_some()
                    || !p.optional_additional_costs.is_empty()
                    || if p.modes.is_empty() {
                        !p.targets.is_empty()
                            || !matches!(p.instructions.as_slice(), [Instruction::DrawCards { .. }])
                            || !fixed_artifact_sacrifice_effect(p)
                    } else {
                        u32::try_from(p.modes.len()).is_err()
                            || !p
                                .modes
                                .iter()
                                .all(|m| fixed_artifact_sacrifice_effect(&m.program))
                    }
                {
                    return Err("unsupported fixed artifact sacrifice activation".into());
                }
                validate_instruction_program(p)?;
                continue;
            }
            // Each complete cost, target and instruction was reconstructed from its printed block.
            if fixed_activations {
                continue;
            }
            if ability.functioning_zone == FunctioningZone::Hand
                || matches!(ability.cost, ActivationCost::DiscardSource { .. })
            {
                if !cycling::definition(self) {
                    return Err("unsupported complete hand discard-source activation".into());
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if has_keyword_grant(&ability.program) {
                if !sacrifice_set::definition(self) {
                    return Err(
                        "unsupported complete sacrifice-source set keyword definition".into(),
                    );
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if activated_source_keyword::has_grant(&ability.program) {
                if c.kind != CardKind::Creature
                    || activated_source_keyword::keywords(ability).is_none()
                {
                    return Err("unsupported complete source keyword activation".into());
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if typed_land_search::activation(ability) || *ability == linked_hideaway::play() {
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if matches!(&ability.cost, ActivationCost::Composite { costs } if costs.remove_source_plus_one_counters.is_some())
            {
                return Err("counter removal requires mana ability".into());
            }
            if land_sacrifice_draw {
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if paid_counted_draw::ability_matches(ability) {
                // The complete source is checked by hand_return::validate_definition.
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if artifact_loss_activation(ability) {
                // The complete artifact and source-bound operands were checked above.
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if activated_artifact_draw_ability(ability) {
                activated_artifact_draw::validate_source(self)?;
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if has_untap(&ability.program) {
                if c.kind != CardKind::Creature || !tap_source_untap_ability(ability) {
                    return Err("tap-source untap requires a creature, pure source tap and one creature target".into());
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if has_permanent_owner_hand(&ability.program) {
                if c.kind != CardKind::Creature || !activated_permanent_owner_hand_ability(ability)
                {
                    return Err("activated permanent return requires a creature, fixed mana plus source tap, and one unqualified permanent target".into());
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if has_counter_stack_object(&ability.program) {
                if c.kind != CardKind::Creature || !activated_spell_counter_ability(ability) {
                    return Err("activated counter requires a creature, fixed mana plus source tap, and one unqualified spell target".into());
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if ability.cost == ActivationCost::TapSource && has_destroy_permanent(&ability.program)
            {
                if c.kind != CardKind::Creature || !tap_source_destroy_ability(ability) {
                    return Err("tap-source destruction requires a creature and one mandatory tapped-creature target".into());
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if has_tap(&ability.program) {
                if c.kind != CardKind::Artifact || !activated_permanent_tap_ability(ability) {
                    return Err("activated tap requires an artifact, fixed mana plus source tap, and one artifact/creature/land target".into());
                }
                validate_instruction_program(&ability.program)?;
                continue;
            }
            if let ActivationCost::SacrificePermanent { filter, mana, .. } = &ability.cost {
                filter.validate()?;
                if mana
                    .as_ref()
                    .is_some_and(|m| m.generic.is_negative() || m.colored.contains(&Color::C))
                {
                    return Err("invalid sacrifice mana cost".into());
                }
            }
            if equipment {
                let p = &ability.program;
                if ability.functioning_zone != FunctioningZone::Battlefield
                    || !matches!(&ability.cost, ActivationCost::Mana { cost } if !cost.generic.is_negative() && !cost.colored.contains(&Color::C))
                    || p.functioning_zone != FunctioningZone::Stack
                    || p.timing != SpellTiming::MainPhase
                    || p.resolution != SpellResolution::Instructions
                    || !p.modes.is_empty()
                    || !matches!(p.targets.as_slice(), [t] if t.domain == TargetDomain::ControlledBattlefieldCreature)
                    || !matches!(
                        p.instructions.as_slice(),
                        [Instruction::AttachEquipment { target_slot: 0, .. }]
                    )
                {
                    return Err("unsupported equip ability".into());
                }
                validate_instruction_program(p)?;
                continue;
            }
            if ability.program.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::AttachEquipment { .. }
                        | Instruction::ReturnAuraFromGraveyard { .. }
                )
            }) {
                return Err("attachment instruction requires equip or Aura return trigger".into());
            }
            if c.kind == CardKind::Artifact
                && !equipment
                && !matches!(ability.cost, ActivationCost::Composite { .. })
            {
                return Err("artifact nonmana abilities require sacrifice-to-draw costs".into());
            }
            if let ActivationCost::Composite { costs } = &ability.cost {
                validate_permanent_costs(costs)?;
                if costs.remove_source_plus_one_counters.is_some() {
                    return Err("counter costs require a mana ability".into());
                }
                if !costs.sacrifice
                    || !((c.kind == CardKind::Artifact
                        && matches!(ability.program.instructions.as_slice(), [Instruction::DrawCards {count, recipient:None, ..}] if *count == Quantity::from(1)))
                        || (c.kind == CardKind::Creature
                            && !costs.tap
                            && costs.mana.is_some()
                            && matches!(
                                ability.program.instructions.as_slice(),
                                [Instruction::DestroyPermanent {
                                    optional: false,
                                    ..
                                }]
                            )))
                {
                    return Err(
                        "composite nonmana activation requires artifact sacrifice and draw".into(),
                    );
                }
            }
            let p = &ability.program;
            if matches!(&ability.cost, ActivationCost::Mana { cost } if cost.generic.is_negative())
                || ability.functioning_zone != FunctioningZone::Battlefield
                || p.functioning_zone != FunctioningZone::Stack
                || p.timing != SpellTiming::Instant
                || p.resolution != SpellResolution::Instructions
                || p.instructions.is_empty()
                || !p.modes.is_empty()
                || if matches!(
                    ability.cost,
                    ActivationCost::SacrificePermanent { .. } | ActivationCost::SacrificeSource
                ) && p.instructions.iter().all(|i| {
                    matches!(
                        i,
                        Instruction::SacrificeDamage {
                            optional: false,
                            ..
                        } | Instruction::PutPlusOneCounters {
                            on: CounterRecipient::Source,
                            ..
                        } | Instruction::ModifySourcePowerToughness { .. }
                            | Instruction::SearchBasicLands { maximum: 1, .. }
                    )
                }) {
                    false
                } else if p.targets.is_empty() {
                    p.instructions.iter().any(|instruction| {
                        !matches!(instruction, Instruction::OptionalDrawCards { .. })
                            && !matches!(instruction,
                                Instruction::DrawCards {count,..}|Instruction::DiscardCards {count,..} if *count==Quantity::from(1))
                    })
                } else if matches!(ability.cost, ActivationCost::Composite { .. })
                    && matches!(
                        p.instructions.as_slice(),
                        [Instruction::DestroyPermanent {
                            optional: false,
                            ..
                        }]
                    )
                {
                    p.targets.len() != 1
                        || p.targets[0].domain != TargetDomain::BattlefieldPermanent
                } else {
                    p.targets
                        .iter()
                        .any(|target| target.domain != TargetDomain::AnyDamageTarget)
                        || p.instructions.iter().any(|instruction| {
                            !matches!(
                                instruction,
                                Instruction::DealDamage { .. }
                                    | Instruction::PreventNextDamage { .. }
                            )
                        })
                }
            {
                return Err("unsupported activated cost/program shape".into());
            }
            validate_instruction_program(p)?;
        }
        if !self.triggered.is_empty()
            && !match c.kind {
                CardKind::Creature | CardKind::Artifact | CardKind::Enchantment => true,
                CardKind::Instant | CardKind::Sorcery => self
                    .triggered
                    .iter()
                    .all(|a| a.condition == TriggerCondition::SelfSpellCast),
                CardKind::Land => self.triggered.iter().all(|a| {
                    a.condition == TriggerCondition::SelfBecomesTapped
                        || (a.condition == TriggerCondition::SelfEntersBattlefield
                            && (self
                                .capabilities
                                .contains(&Capability::EntryGraveyardExileV1)
                                || self.capabilities.contains(&Capability::LandEntryGainLifeV1)
                                || self.capabilities.contains(&Capability::HideawayV1)))
                }),
            }
        {
            return Err(
                "trigger programs require a supported permanent or exact stack self-cast source"
                    .into(),
            );
        }
        for ability in &self.triggered {
            if let TriggerCondition::GroupedDamageDealt {
                source,
                kind,
                recipient,
                source_power,
            } = &ability.condition
            {
                if *source
                    != (DamageTriggerSource::CreatureFilter {
                        controller: DamageSourceRelation::Controller,
                        types: TypePredicate::Type {
                            kind: CardKind::Creature,
                        },
                    })
                    || *kind != TriggerDamageKind::Combat
                    || *recipient != DamageTriggerRecipient::Player
                    || *source_power != DamageSourcePowerPredicate::GreaterThanBase
                    || ability.death_filter.is_some()
                    || ability.intervening_if.is_some()
                    || !ability.program.targets.is_empty()
                    || !ability.program.modes.is_empty()
                    || !matches!(ability.program.instructions.as_slice(),
                        [Instruction::DrawCards {recipient:None|Some(LifeRecipient::Controller),count,..}] if *count == Quantity::from(1))
                {
                    return Err("grouped damage requires controlled creatures above base power, combat player damage and one mandatory controller draw".into());
                }
            }
            if matches!(
                ability.condition,
                TriggerCondition::DamageDealt {
                    source: DamageTriggerSource::Source {},
                    ..
                }
            ) && !c.has_type(CardKind::Creature)
            {
                return Err("self damage trigger requires a creature source".into());
            }
            if let TriggerCondition::DamageDealt {
                source: DamageTriggerSource::CreatureFilter { types, .. },
                ..
            } = &ability.condition
            {
                types.validate()?;
            }
            if let TriggerCondition::CreatureAttacked { attacker } = &ability.condition {
                match attacker {
                    AttackSubject::Source {} if !c.has_type(CardKind::Creature) => {
                        return Err("self attack trigger requires a creature source".into());
                    }
                    AttackSubject::Filter { types, .. } => types.validate()?,
                    _ => (),
                }
            }
            if let TriggerCondition::SpellCast { spell, .. } = &ability.condition {
                match spell {
                    TypePredicate::SpellColor { color: Color::C } => {
                        return Err("invalid spell color".into());
                    }
                    TypePredicate::SpellColor { .. } => (),
                    _ => spell.validate()?,
                }
            }
            if let Some(f) = &ability.death_filter
                && (ability.condition != TriggerCondition::AnyCreatureDies
                    || !f.controlled
                    || f.non_subtypes.iter().any(|s| s != "Spirit")
                    || f.non_subtypes.len() > 1)
            {
                return Err("invalid filtered dies trigger".into());
            }
            let p = &ability.program;
            if ability.intervening_if == Some(InterveningCondition::SourceCastByController)
                && (ability.condition != TriggerCondition::SelfEntersBattlefield
                    || c.kind != CardKind::Creature)
            {
                return Err(
                    "cast-entry condition requires the entering creature's own trigger".into(),
                );
            }
            if matches!(ability.condition, TriggerCondition::SagaChapter { .. })
                || matches!(
                    p.instructions.as_slice(),
                    [Instruction::OfferHandCastWithoutManaCost { .. }]
                )
            {
                // The whole definition gate binds this one operation to the source's own combat event.
                validate_instruction_program(p)?;
                continue;
            }
            if ability.condition == TriggerCondition::SelfBecomesTapped {
                // The complete source and trigger shape was checked above.
                validate_instruction_program(p)?;
                continue;
            }
            if matches!(
                ability.condition,
                TriggerCondition::SpellCastWithOrigin { .. }
            ) {
                cast_origin::validate(ability)?;
            }
            if matches!(ability.condition, TriggerCondition::SelfMoved { .. }) {
                successor_tuck::validate(c, ability)?;
                validate_instruction_program(p)?;
                continue;
            }
            if ability.condition == TriggerCondition::SelfSpellCast {
                if ability.functioning_zone != FunctioningZone::Stack
                    || ability.intervening_if.is_some()
                    || ability.death_filter.is_some()
                    || (p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty())
                    || p.timing != SpellTiming::Instant
                    || p.functioning_zone != FunctioningZone::Stack
                    || p.resolution != SpellResolution::Instructions
                    || !p.targets.is_empty()
                    || !p.modes.is_empty()
                    || !(is_canonical_cascade_program(p)
                        || creative_technique::is_demonstrate_program(p))
                {
                    return Err("unsupported self-cast trigger shape".into());
                }
                validate_instruction_program(p)?;
                continue;
            }
            if p.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Cascade { .. }))
            {
                return Err("cascade requires its stack-functioning self-cast trigger".into());
            }
            if aura {
                if ability.condition != TriggerCondition::SelfAuraToGraveyard
                    || ability.intervening_if.is_some()
                    || ability.functioning_zone != FunctioningZone::Battlefield
                    || p.timing != SpellTiming::Instant
                    || p.functioning_zone != FunctioningZone::Stack
                    || p.resolution != SpellResolution::Instructions
                    || !p.targets.is_empty()
                    || !p.modes.is_empty()
                    || !matches!(
                        p.instructions.as_slice(),
                        [Instruction::ReturnAuraFromGraveyard { .. }]
                    )
                {
                    return Err("unsupported Aura return trigger".into());
                }
                validate_instruction_program(p)?;
                continue;
            }
            if p.instructions
                .iter()
                .any(|i| matches!(i, Instruction::DestroyPermanent { .. }))
                && ability.condition != TriggerCondition::SelfEntersBattlefield
                && !matches!(
                    ability.condition,
                    TriggerCondition::StepBegan { .. }
                        | TriggerCondition::SpellCast { .. }
                        | TriggerCondition::CreatureAttacked { .. }
                        | TriggerCondition::DamageDealt { .. }
                )
            {
                return Err("targeted removal requires a supported entry or step trigger".into());
            }
            if matches!(
                ability.condition,
                TriggerCondition::StepBegan { .. }
                    | TriggerCondition::SpellCast { .. }
                    | TriggerCondition::CreatureAttacked { .. }
                    | TriggerCondition::DamageDealt { .. }
            ) && p.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::DestroyPermanent { optional: true, .. }
                        | Instruction::ReturnOwnGraveyardCreatureToHand { optional: true, .. }
                        | Instruction::SacrificeDamage { optional: true, .. }
                        | Instruction::LoseLifeDeathController { .. }
                )
            }) {
                return Err(
                    "step/cast/attack/damage triggers require mandatory programs without death-event recipients"
                        .into(),
                );
            }
            if ability.condition == TriggerCondition::SelfAuraToGraveyard {
                return Err("Aura trigger requires Aura".into());
            }

            if ability.functioning_zone != FunctioningZone::Battlefield
                || (p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty())
                || p.functioning_zone != FunctioningZone::Stack
                || p.timing != SpellTiming::Instant
                || p.resolution != SpellResolution::Instructions
                || p.instructions.is_empty()
                || !p.modes.is_empty()
                || (ability.intervening_if.is_some()
                    && ability.condition != TriggerCondition::SelfDies
                    && !matches!(ability.condition, TriggerCondition::StepBegan { .. })
                    && !(ability.condition == TriggerCondition::SelfEntersBattlefield
                        && matches!(
                            ability.intervening_if,
                            Some(InterveningCondition::ControllerHasCreatureWithPower { .. })
                                | Some(InterveningCondition::SourceCastByController)
                        )))
                || (matches!(
                    ability.intervening_if,
                    Some(InterveningCondition::SourceStatus { .. })
                ) && !matches!(ability.condition, TriggerCondition::StepBegan { .. }))
                || p.targets.iter().any(|target| {
                    !matches!(
                        target.domain,
                        TargetDomain::AnyPlayer
                            | TargetDomain::OpponentPlayer
                            | TargetDomain::AnyDamageTarget
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                            | TargetDomain::OwnGraveyardCard
                    )
                })
                || p.instructions.iter().any(|instruction| {
                    !matches!(
                        instruction,
                        Instruction::LookTopMayRevealForMana { .. }
                            | Instruction::PumpSet { .. }
                            | Instruction::DealDamage { .. }
                            | Instruction::LoseLifeDeathController { .. }
                            | Instruction::SacrificeDamage { .. }
                            | Instruction::EachOpponentSacrifice { .. }
                            | Instruction::ReturnOwnGraveyardCreatureToHand { .. }
                            | Instruction::ReturnOwnGraveyardCardToHand { .. }
                            | Instruction::ExileTargetGraveyard { .. }
                            | Instruction::ReturnEntryTargetCreature { .. }
                            | Instruction::DiscardPlayerSet { .. }
                            | Instruction::DestroyPermanent { .. }
                            | Instruction::DestroySet { .. }
                            | Instruction::CreateCreatureTokens { .. }
                            | Instruction::PutSourcePlusOneCountersForCastColors { .. }
                            | Instruction::PutPlusOneCounters { .. }
                            | Instruction::DrawCards { .. }
                            | Instruction::DrawControllerCards { .. }
                            | Instruction::UntapControllerLands { .. }
                            | Instruction::OptionalDrawForSoleGreatestEntrant { .. }
                            | Instruction::OptionalDrawCards { .. }
                            | Instruction::OptionalDrawUnlessCasterPays { .. }
                            | Instruction::DrawDamageAmount { .. }
                            | Instruction::GrantHandCastCascade { .. }
                            | Instruction::Hideaway { .. }
                            | Instruction::Scry { .. }
                            | Instruction::DiscardCards { .. }
                            | Instruction::SacrificeCreature { .. }
                            | Instruction::ModifyPowerToughness { .. }
                            | Instruction::GainLife { .. }
                            | Instruction::LoseLife { .. }
                    )
                })
            {
                return Err("unsupported mandatory triggered program shape".into());
            }
            validate_instruction_program(p)?;
        }
        if self.static_abilities.iter().any(|a| {
            !matches!(
                a.effect,
                StaticEffect::GrantSpellCascade { .. }
                    | StaticEffect::CascadeLandEntry
                    | StaticEffect::KeepUnspentManaAsBlack
                    | StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn
                    | StaticEffect::GrantControllerLandsAnyColorMana
                    | StaticEffect::ReduceSelfGenericCost { .. }
                    | StaticEffect::CantBeCountered
                    | StaticEffect::SelfGraveyardRevealShuffle
                    | StaticEffect::GrantOtherControlledPermanentsIndestructible
                    | StaticEffect::EntersTappedUnlessControllerBasicLands { .. }
                    | StaticEffect::EntersTappedUnlessControllerOtherLands { .. }
                    | StaticEffect::EntersTappedUnlessPayLife { .. }
                    | StaticEffect::ModifySourceByDistinctPermanentColors { .. }
            ) && a.effect != StaticEffect::EntersTapped
                && a.effect != StaticEffect::OpponentsCantCastDuringControllerTurn
                && a.effect != StaticEffect::UnlimitedControllerHandSize
                && !matches!(
                    a.effect,
                    StaticEffect::ReduceMaximumHandSize { .. }
                        | StaticEffect::IncreaseMaximumHandSize { .. }
                        | StaticEffect::ControllerCreaturePlusOneCounterReplacement { .. }
                )
                && a.effect != StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
                && source_combat_restrictions::restriction(&a.effect).is_none()
                && !matches!(
                    a.effect,
                    StaticEffect::ModifyCreatures {
                        scope: CreatureScope::AttackingController | CreatureScope::Controller,
                        ..
                    }
                )
        }) && bestow_alternative(self).is_none()
            && c.kind != CardKind::Enchantment
            && !equipment
            && !(sacrifice && c.kind == CardKind::Creature)
            && !(wipes && matches!(c.kind, CardKind::Instant | CardKind::Sorcery))
        {
            return Err("static damage replacement requires a supported enchantment".into());
        }
        if self.static_abilities.iter().any(|a| {
            if a.effect == StaticEffect::MayBeginOnBattlefield {
                a.functioning_zone != FunctioningZone::Hand
            } else if a.effect == StaticEffect::SelfGraveyardRevealShuffle {
                a.functioning_zone != FunctioningZone::Any
            } else if matches!(
                a.effect,
                StaticEffect::ReduceSelfGenericCost { .. } | StaticEffect::Bestow { .. }
            ) {
                a.functioning_zone != FunctioningZone::Stack
            } else if matches!(
                a.effect,
                StaticEffect::ReduceGenericByCreatureCount | StaticEffect::CantBeCountered
            ) {
                a.functioning_zone != FunctioningZone::Stack
                    || !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
            } else {
                a.functioning_zone != FunctioningZone::Battlefield
            }
        }) {
            return Err("static damage replacement must function on battlefield".into());
        }
        for a in &self.static_abilities {
            if a.effect == StaticEffect::GrantOtherControlledPermanentsIndestructible {
                if c.kind != CardKind::Creature
                    || self.static_abilities.len() != 1
                    || !self.activated.is_empty()
                    || !self.mana_abilities.is_empty()
                    || !self.triggered.is_empty()
                {
                    return Err("indestructible grant requires an ordinary creature source".into());
                }
                continue;
            }
            if let StaticEffect::ReduceSelfGenericCost { per_object, count } = &a.effect {
                if *per_object <= Quantity::zero()
                    || count.kind != CardKind::Artifact
                    || count.controller != CountController::Caster
                    || self.spell.is_none()
                    || equipment
                    || aura
                {
                    return Err("unsupported counted self generic cost reduction".into());
                }
                continue;
            }
            if let StaticEffect::GrantSpellCascade {
                eligibility,
                program,
            } = &a.effect
            {
                if matches!(eligibility, SpellCascadeEligibility::ManaValueAtLeast { minimum } if minimum.is_negative())
                    || !matches!(
                        c.kind,
                        CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
                    )
                    || equipment
                    || aura
                    || !is_canonical_cascade_program(program)
                {
                    return Err("unsupported static spell cascade grant".into());
                }
                validate_instruction_program(program)?;
                continue;
            }
            if let StaticEffect::ControllerCreaturePlusOneCounterReplacement { modification } =
                &a.effect
            {
                modification.validate()?;
                if !matches!(
                    c.kind,
                    CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
                ) {
                    return Err("counter replacement requires a battlefield permanent".into());
                }
                continue;
            }
            if a.effect == StaticEffect::ControlEnchantedPermanent {
                // The whole definition was checked by aura_control_source above.
                continue;
            }
            if a.effect == StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer {
                if !matches!(
                    c.kind,
                    CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
                ) {
                    return Err(
                        "controlled creature damage replacement requires a battlefield permanent"
                            .into(),
                    );
                }
                continue;
            }
            if a.effect == StaticEffect::OpponentsCantCastDuringControllerTurn {
                if !matches!(
                    c.kind,
                    CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
                ) {
                    return Err(
                        "opponent casting prohibition requires a battlefield permanent".into(),
                    );
                }
                continue;
            }
            if matches!(
                a.effect,
                StaticEffect::EntersTapped
                    | StaticEffect::EntersTappedUnlessControllerBasicLands { .. }
                    | StaticEffect::EntersTappedUnlessControllerOtherLands { .. }
                    | StaticEffect::EntersTappedUnlessPayLife { .. }
            ) {
                if matches!(c.kind, CardKind::Instant | CardKind::Sorcery) {
                    return Err("entry replacement requires a permanent".into());
                }
                continue;
            }
            if matches!(a.effect, StaticEffect::ModifyCreatures { .. }) {
                if !static_creature_modification_valid(
                    c.kind,
                    equipment,
                    aura || bestow_alternative(self).is_some(),
                    &a.effect,
                ) {
                    return Err("invalid static creature modification".into());
                }
            } else if equipment || aura {
                return Err("unsupported attachment static effect".into());
            }
            if let StaticEffect::ControllerCreatureAnthem { power, toughness } = &a.effect
                && (*power <= Quantity::zero() || power != toughness)
            {
                return Err("anthem must give fixed positive +N/+N".into());
            }
        }
        match c.kind {
            CardKind::Enchantment | CardKind::Artifact => {
                let p = self.spell.as_ref().ok_or("missing enchantment spell")?;
                if c.basic
                    || c.power.is_some()
                    || c.toughness.is_some()
                    || self.intrinsic_zone.is_some()
                    || p.functioning_zone != FunctioningZone::Stack
                    || p.timing != cast_timing
                    || p.resolution != SpellResolution::Permanent
                    || if aura && !aura_control_source(self) {
                        !matches!(p.targets.as_slice(), [t] if t.domain == TargetDomain::BattlefieldCreature && t.slot == 0 && !t.selection_id.is_empty() && t.chooser == Chooser::Controller && t.stage == EvaluationStage::Announcement)
                    } else if !aura {
                        !p.targets.is_empty()
                    } else {
                        false
                    }
                    || !p.instructions.is_empty()
                    || !p.modes.is_empty()
                {
                    return Err("unsupported enchantment characteristics or spell".into());
                }
            }
            CardKind::Land => {
                if if c.basic {
                    c.subtypes.len() != 1
                        || basic_land_color(&c.subtypes[0]).is_none()
                        || !self.mana_abilities.is_empty()
                } else {
                    if c.subtypes.is_empty() || listed_mana_choice::descriptive_land_subtype(self) {
                        self.mana_abilities.is_empty()
                            && !typed_land_search::activation_definition(self)
                    } else {
                        !self.mana_abilities.is_empty()
                            || c.subtypes.iter().any(|s| basic_land_color(s).is_none())
                            || c.subtypes.iter().collect::<BTreeSet<_>>().len() != c.subtypes.len()
                            || !c.additional_types.is_empty()
                    }
                } || c.mana_cost.generic != Quantity::zero()
                    || !c.mana_cost.colored.is_empty()
                    || c.power.is_some()
                    || c.toughness.is_some()
                    || self.spell.is_some()
                    || self.intrinsic_zone != Some(FunctioningZone::Battlefield)
                {
                    return Err("unsupported land characteristics".into());
                }
            }
            CardKind::Creature => {
                if c.basic
                    || c.power.is_none()
                    || c.toughness
                        .as_ref()
                        .is_none_or(|n| n <= &Quantity::zero() && !has_distinct_permanent_colors)
                    || self.intrinsic_zone.is_some()
                {
                    return Err("unsupported creature characteristics".into());
                }
                let p = self
                    .spell
                    .as_ref()
                    .ok_or("missing creature spell program")?;
                if p.functioning_zone != FunctioningZone::Stack
                    || p.timing != cast_timing
                    || p.resolution != SpellResolution::Permanent
                    || !p.targets.is_empty()
                    || !p.instructions.is_empty()
                    || !p.modes.is_empty()
                {
                    return Err("unsupported creature spell semantics".into());
                }
            }
            CardKind::Instant | CardKind::Sorcery => {
                if c.legendary
                    || c.basic
                    || c.power.is_some()
                    || c.toughness.is_some()
                    || self.intrinsic_zone.is_some()
                {
                    return Err("unsupported instant/sorcery characteristics".into());
                }
                let p = self.spell.as_ref().ok_or("missing spell program")?;
                if p.functioning_zone != FunctioningZone::Stack
                    || p.timing != cast_timing
                    || !matches!(
                        p.resolution,
                        SpellResolution::Instructions
                            | SpellResolution::OneOrMoreModes
                            | SpellResolution::ChooseModes { .. }
                            | SpellResolution::ChooseRepeatedModes { .. }
                    )
                    || (p.instructions.is_empty() && p.modes.is_empty())
                {
                    return Err("invalid spell functioning zone or empty program".into());
                }
                // The closed modal grant was validated as a whole, including its protected child.
                if !akromas_will_program(p) {
                    validate_instruction_program(p)?;
                }
            }
        }
        Ok(())
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
fn ordinary_tap_program(p: &SpellProgram) -> bool {
    (p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        && p.modes.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && p.targets.len() == 1
        && p.targets[0].domain == TargetDomain::BattlefieldCreature
        && p.targets[0].filter.is_none()
        && matches!(
            p.instructions.first(),
            Some(Instruction::TapPermanent { target_slot: 0, .. })
        )
        && (p.instructions.len() == 1
            || (p.instructions.len() == 2
                && matches!(&p.instructions[1], Instruction::DrawCards { recipient: None, count, .. } if *count == Quantity::from(1))))
}
fn activated_permanent_tap_program(p: &SpellProgram) -> bool {
    (p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        && p.modes.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && matches!(
            p.instructions.as_slice(),
            [Instruction::TapPermanent { target_slot: 0, .. }]
        )
        && p.targets.len() == 1
        && p.targets[0].domain == TargetDomain::BattlefieldPermanent
        && p.targets[0].filter.as_ref().is_some_and(|f| {
            f.types.len() == 3
                && [CardKind::Artifact, CardKind::Creature, CardKind::Land]
                    .iter()
                    .all(|kind| f.types.contains(kind))
                && f.non_types.is_empty()
                && f.non_colors.is_empty()
                && !f.opponent
                && f.power.is_none()
                && !f.tapped
        })
}
fn activated_permanent_tap_ability(a: &ActivatedAbility) -> bool {
    a.functioning_zone == FunctioningZone::Battlefield
        && matches!(&a.cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.as_ref().is_some_and(|m|
                !m.generic.is_negative() && !m.colored.contains(&Color::C)))
        && activated_permanent_tap_program(&a.program)
}
// Pinned CR302.6,602.2b,608.2b,701.26b: the contextual definition guard
// admits only this pure source-tap activation; ordinary spells stay unqualified.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
fn untap_program(p: &SpellProgram, types: &[CardKind]) -> bool {
    p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && (p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        && p.modes.is_empty()
        && matches!(
            p.instructions.as_slice(),
            [Instruction::UntapPermanent { target_slot: 0, .. }]
        )
        && matches!(p.targets.as_slice(), [t] if t.slot == 0
            && !t.selection_id.is_empty() && t.chooser == Chooser::Controller
            && t.stage == EvaluationStage::Announcement
            && t.domain == TargetDomain::BattlefieldPermanent
            && t.filter.as_ref().is_some_and(|f| f.types == types && f.non_types.is_empty()
                && f.non_colors.is_empty() && !f.opponent && f.power.is_none() && !f.tapped))
}
fn ordinary_untap_program(p: &SpellProgram) -> bool {
    untap_program(p, &[])
}
fn tap_source_untap_program(p: &SpellProgram) -> bool {
    p.timing == SpellTiming::Instant && untap_program(p, &[CardKind::Creature])
}
fn tap_source_untap_ability(a: &ActivatedAbility) -> bool {
    a.functioning_zone == FunctioningZone::Battlefield
        && a.cost == ActivationCost::TapSource
        && tap_source_untap_program(&a.program)
}
fn tap_source_destroy_ability(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.cost == ActivationCost::TapSource
        && a.functioning_zone == FunctioningZone::Battlefield
        && p.timing == SpellTiming::Instant
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && (p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        && p.modes.is_empty()
        && matches!(
            p.instructions.as_slice(),
            [Instruction::DestroyPermanent {
                target_slot: 0,
                optional: false,
                regeneration_forbidden: false,
                ..
            }]
        )
        && p.targets.len() == 1
        && p.targets[0].domain == TargetDomain::BattlefieldPermanent
        && p.targets[0].filter.as_ref().is_some_and(|f| {
            f.types == [CardKind::Creature]
                && f.power.is_none()
                && f.tapped
                && !f.opponent
                && f.non_types.is_empty()
                && f.non_colors.is_empty()
        })
}
fn activated_spell_counter_ability(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.functioning_zone == FunctioningZone::Battlefield
        && matches!(&a.cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.as_ref().is_some_and(|m|
                !m.generic.is_negative() && !m.colored.contains(&Color::C)))
        && p.timing == SpellTiming::Instant
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && (p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        && p.modes.is_empty()
        && matches!(
            p.instructions.as_slice(),
            [Instruction::CounterStackObject { target_slot: 0, .. }]
        )
        && p.targets.len() == 1
        && p.targets[0].domain == TargetDomain::StackSpell
        && p.targets[0].filter.is_none()
}
fn activated_permanent_owner_hand_ability(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.functioning_zone == FunctioningZone::Battlefield
        && matches!(&a.cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.as_ref().is_some_and(|m|
                !m.generic.is_negative() && !m.colored.contains(&Color::C)))
        && p.timing == SpellTiming::Instant
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && (p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        && p.modes.is_empty()
        && matches!(
            p.instructions.as_slice(),
            [Instruction::ReturnToOwnerHand { target_slot: 0, .. }]
        )
        && p.targets.len() == 1
        && p.targets[0].domain == TargetDomain::BattlefieldPermanent
        && p.targets[0].filter.as_ref().is_some_and(|f| {
            f.types.is_empty()
                && f.non_types.is_empty()
                && f.non_colors.is_empty()
                && !f.opponent
                && f.power.is_none()
                && !f.tapped
        })
}
fn has_keyword_grant(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::GrantKeywords { .. } | Instruction::GrantKeywordsSet { .. }
        )
    }) || p.modes.iter().any(|m| has_keyword_grant(&m.program))
}
fn ordinary_keyword_grant(p: &SpellProgram) -> bool {
    temporary_targeted_keyword_program(p).is_some() || temporary_set_keyword_program(p).is_some()
}
/// Reviewed temporary-grant vocabulary. Order is retained, but repetitions and
/// keywords outside this nonempty family are unsupported.
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
#[derive(Clone, Copy, Debug)]
pub struct TemporaryTargetedKeywordProgram<'a> {
    pub modifier: Option<(&'a Quantity, &'a Quantity)>,
    pub keywords: &'a [KeywordAbility],
    pub controller_life: Option<&'a Quantity>,
    pub controller_scry: Option<&'a Quantity>,
}
/// Recognize one complete grant on one captured creature target. The caller
/// checks definition kind/context. Pure grants retain source-block provenance;
/// a grant with a fixed signed stat modifier belongs entirely to source block 0.
/// CR 611.2c, 613.5–6:
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub fn temporary_creature_keyword_program(
    p: &SpellProgram,
) -> Option<TemporaryTargetedKeywordProgram<'_>> {
    if p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || !matches!(p.targets.as_slice(), [t] if t.slot == 0
            && !t.selection_id.is_empty() && t.chooser == Chooser::Controller
            && t.stage == EvaluationStage::Announcement
            && matches!(t.domain, TargetDomain::BattlefieldCreature
                | TargetDomain::ControlledBattlefieldCreature) && t.filter.is_none())
    {
        return None;
    }
    let [
        Instruction::GrantKeywords {
            target_slot: 0,
            modifier,
            keywords,
            duration: Duration::UntilEndOfTurn,
            source_block,
        },
    ] = p.instructions.as_slice()
    else {
        return None;
    };
    if modifier.is_some() && *source_block != 0 {
        return None;
    }
    let modifier = modifier.as_ref().map(|m| (&m.power, &m.toughness));
    supported_temporary_keywords(keywords).then_some(TemporaryTargetedKeywordProgram {
        modifier,
        keywords,
        controller_life: None,
        controller_scry: None,
    })
}
/// Complete scalar union; narrow wrappers preserve their own target policy.
pub fn temporary_targeted_keyword_program(
    p: &SpellProgram,
) -> Option<TemporaryTargetedKeywordProgram<'_>> {
    temporary_creature_keyword_program(p).or_else(|| temporary_permanent_keyword_program(p))
}
/// One controlled permanent with optional fixed controller life gain, or one
/// artifact-or-creature target with optional fixed controller scry.
/// CR 109.5, 608.2b/c, 611.2c:
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub fn temporary_permanent_keyword_program(
    p: &SpellProgram,
) -> Option<TemporaryTargetedKeywordProgram<'_>> {
    let controlled = p.targets.first().is_some_and(|t| {
        t.domain == TargetDomain::ControlledBattlefieldPermanent
            && t.filter.as_ref().is_some_and(|f| {
                f.types.is_empty()
                    && f.non_types.is_empty()
                    && f.non_colors.is_empty()
                    && !f.opponent
                    && f.power.is_none()
                    && !f.tapped
            })
    });
    let artifact_or_creature = p.targets.first().is_some_and(|t| {
        t.domain == TargetDomain::BattlefieldPermanent
            && t.filter.as_ref().is_some_and(|f| {
                f.types == [CardKind::Creature, CardKind::Artifact]
                    && f.non_types.is_empty()
                    && f.non_colors.is_empty()
                    && !f.opponent
                    && f.power.is_none()
                    && !f.tapped
            })
    });
    if p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || !matches!(p.targets.as_slice(), [t] if t.slot == 0 && !t.selection_id.is_empty()
            && t.chooser == Chooser::Controller && t.stage == EvaluationStage::Announcement
            && (controlled || artifact_or_creature))
    {
        return None;
    }
    let (grant, controller_life, controller_scry) = match p.instructions.as_slice() {
        [grant] => (grant, None, None),
        [
            grant,
            Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount,
                source_block: 0,
            },
        ] if controlled && !amount.is_negative() => (grant, Some(amount), None),
        [
            grant,
            Instruction::Scry {
                amount,
                source_block: 0,
            },
        ] if artifact_or_creature && !amount.is_negative() => (grant, None, Some(amount)),
        _ => return None,
    };
    let Instruction::GrantKeywords {
        target_slot: 0,
        modifier: None,
        keywords,
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    } = grant
    else {
        return None;
    };
    supported_temporary_keywords(keywords).then_some(TemporaryTargetedKeywordProgram {
        modifier: None,
        keywords,
        controller_life,
        controller_scry,
    })
}
/// Borrowed parameters of the complete ordinary temporary set-keyword family.
#[derive(Clone, Copy, Debug)]
pub struct TemporarySetKeywordProgram<'a> {
    pub set: &'a ObjectSet,
    pub modifier: Option<(&'a Quantity, &'a Quantity)>,
    pub keywords: &'a [KeywordAbility],
}
/// CR 608.2c/h, 611.2c, 613.5–6: one instruction creates the complete effect.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub fn temporary_set_keyword_program(p: &SpellProgram) -> Option<TemporarySetKeywordProgram<'_>> {
    if p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
    {
        return None;
    }
    let [
        Instruction::GrantKeywordsSet {
            set,
            modifier,
            keywords,
            duration: Duration::UntilEndOfTurn,
            source_block: 0,
        },
    ] = p.instructions.as_slice()
    else {
        return None;
    };
    let modifier = modifier.as_ref().map(|m| (&m.power, &m.toughness));
    ((matches!(
        set,
        ObjectSet::ControllerCreatures | ObjectSet::AttackingCreatures
    ) || (*set == ObjectSet::ControllerPermanents && modifier.is_none()))
        && supported_temporary_keywords(keywords))
    .then_some(TemporarySetKeywordProgram {
        set,
        modifier,
        keywords,
    })
}
/// Creature-only view of the shared complete ordinary set-keyword family.
pub fn temporary_creature_set_keyword_program(
    p: &SpellProgram,
) -> Option<TemporarySetKeywordProgram<'_>> {
    let program = temporary_set_keyword_program(p)?;
    matches!(
        program.set,
        ObjectSet::ControllerCreatures | ObjectSet::AttackingCreatures
    )
    .then_some(program)
}
/// Permanent-only view; a permanent grant never carries a stat modifier.
pub fn temporary_permanent_set_keyword_program(
    p: &SpellProgram,
) -> Option<TemporarySetKeywordProgram<'_>> {
    let program = temporary_set_keyword_program(p)?;
    (*program.set == ObjectSet::ControllerPermanents).then_some(program)
}
/// Narrow reviewed subset retained for existing protection policy callers.
/// Quantities remain exact signed parameters of the shared program descriptor.
pub fn compound_indestructible_parameters(p: &SpellProgram) -> Option<(&Quantity, &Quantity)> {
    let program = temporary_creature_keyword_program(p)?;
    if p.targets[0].domain == TargetDomain::BattlefieldCreature
        && program.keywords == [KeywordAbility::Indestructible]
    {
        program.modifier
    } else {
        None
    }
}
fn temporary_creature_keyword_capability(p: &SpellProgram) -> Capability {
    // Legacy subset labels describe only the original unfiltered target policy.
    if !matches!(p.targets.as_slice(), [t] if t.domain == TargetDomain::BattlefieldCreature) {
        return Capability::TemporaryCreatureKeywordProgramV1;
    }
    match temporary_creature_keyword_program(p) {
        Some(TemporaryTargetedKeywordProgram {
            modifier: None,
            keywords,
            ..
        }) => match keywords {
            [KeywordAbility::Flying] => Capability::TemporaryFlyingSpellV1,
            [KeywordAbility::Deathtouch] => Capability::TemporaryDeathtouchSpellV1,
            [KeywordAbility::Indestructible] => Capability::TemporaryIndestructibleSpellV1,
            _ => Capability::TemporaryCreatureKeywordProgramV1,
        },
        Some(TemporaryTargetedKeywordProgram {
            modifier: Some(_),
            keywords,
            ..
        }) if keywords == [KeywordAbility::Indestructible] => {
            Capability::CompoundIndestructibleSpellV1
        }
        _ => Capability::TemporaryCreatureKeywordProgramV1,
    }
}
fn validate_instruction_program(p: &SpellProgram) -> Result<(), String> {
    if p.functioning_zone == FunctioningZone::Any {
        return Err("all-zone functioning is reserved for the self graveyard replacement".into());
    }
    optional_costs::validate_program(p)?;
    typed_land_search::validate_program(p)?;
    if p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::Cascade { .. }))
        && ((p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty())
            || !p.targets.is_empty()
            || !p.modes.is_empty()
            || p.timing != SpellTiming::Instant
            || p.resolution != SpellResolution::Instructions
            || p.functioning_zone != FunctioningZone::Stack
            || !matches!(p.instructions.as_slice(), [Instruction::Cascade { .. }]))
    {
        return Err("cascade requires one complete targetless program".into());
    }
    if targeted_counted_draw::has_counted_draw(p) && !targeted_counted_draw_program(p) {
        return Err("unsupported complete targeted counted draw".into());
    }
    if hand_return::has_hand_program(p)
        && !controller_hand_return_program(p)
        && !controller_draw_discard_program(p)
        && !controller_counted_draw_program(p)
        && !hand_return::controller_draw_land_allowance_program(p)
        && !flash_end_step_draw::trigger_program(p)
        && !paid_counted_draw::program_matches(p)
    {
        return Err("unsupported complete counted controller draw program".into());
    }
    if library_order::has_order(p) && !controller_library_order_program(p) {
        return Err("unsupported complete controller library ordering program".into());
    }
    if library_selection::has_selection(p) && !controller_library_selection_program(p) {
        return Err("unsupported complete library selection program".into());
    }
    if fixed_mill::has_mill(p) && !fixed_mill_program(p) {
        return Err("unsupported complete fixed mill program".into());
    }
    // Controller selectors require complete ordinary temporary-effect families.
    fn controller_set(p: &SpellProgram) -> bool {
        p.instructions.iter().any(|i| match i {
            Instruction::MoveSet { set, .. }
            | Instruction::PumpSet { set, .. }
            | Instruction::GrantKeywordsSet { set, .. } => matches!(
                set,
                ObjectSet::ControllerCreatures | ObjectSet::ControllerPermanents
            ),
            Instruction::DamageSet { sets, .. } => sets.iter().any(|set| {
                matches!(
                    set,
                    ObjectSet::ControllerCreatures | ObjectSet::ControllerPermanents
                )
            }),
            _ => false,
        }) || p.modes.iter().any(|m| controller_set(&m.program))
    }
    if p.targets
        .iter()
        .any(|t| t.domain == TargetDomain::ControlledBattlefieldPermanent)
        && temporary_permanent_keyword_program(p).is_none()
    {
        return Err(
            "controlled permanent targets require complete temporary keyword program".into(),
        );
    }
    if controller_set(p)
        && temporary_set_keyword_program(p).is_none()
        && !controller_creature_pump::program(p)
    {
        return Err("controller set requires complete temporary keyword family".into());
    }

    if controller_library::has_look(p)
        && !controller_scry_program(p)
        && !controller_surveil_program(p)
        && !temporary_permanent_keyword_program(p).is_some_and(|d| d.controller_scry.is_some())
    {
        return Err("unsupported complete controller scry program".into());
    }
    if has_keyword_grant(p) && !ordinary_keyword_grant(p) {
        return Err("unsupported temporary keyword grant program".into());
    }
    if has_permanent_owner_hand(p)
        && ((p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty())
            || !p.modes.is_empty()
            || p.functioning_zone != FunctioningZone::Stack
            || p.resolution != SpellResolution::Instructions
            || p.targets.len() != 1
            || p.targets[0].domain != TargetDomain::BattlefieldPermanent
            || p.targets[0].filter.as_ref().is_none_or(|f| {
                !f.types.is_empty()
                    || !matches!(f.non_types.as_slice(), [] | [CardKind::Land])
                    || !f.non_colors.is_empty()
                    || f.opponent
                    || f.power.is_some()
                    || f.tapped
            })
            || !matches!(
                p.instructions.as_slice(),
                [Instruction::ReturnToOwnerHand { target_slot: 0, .. }]
            ))
    {
        return Err("permanent owner-hand movement requires one unqualified or nonland permanent target and one instruction".into());
    }
    if has_tap(p) && !ordinary_tap_program(p) && !activated_permanent_tap_program(p) {
        return Err("tap requires one creature target, then at most one controller draw".into());
    }
    if has_untap(p) && !ordinary_untap_program(p) && !tap_source_untap_program(p) {
        return Err("untap requires one ordinary permanent or activated creature target and one instruction".into());
    }
    if p.targets
        .iter()
        .any(|t| t.domain == TargetDomain::BlockingBattlefieldCreature)
        && ((p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty())
            || !p.modes.is_empty()
            || p.functioning_zone != FunctioningZone::Stack
            || p.resolution != SpellResolution::Instructions
            || p.targets.iter().any(|t| {
                t.domain != TargetDomain::BlockingBattlefieldCreature || t.filter.is_some()
            })
            || p.instructions.is_empty()
            || p.instructions.iter().any(|i| {
                !matches!(
                    i,
                    Instruction::ModifyPowerToughness {
                        duration: Duration::UntilEndOfTurn,
                        ..
                    }
                )
            }))
    {
        return Err("blocking creature targets require a complete ordinary pump program".into());
    }
    if let Some(f) = &p.additional_sacrifice {
        f.validate()?;
        if p.resolution != SpellResolution::Instructions || !p.modes.is_empty() {
            return Err("unsupported additional sacrifice program".into());
        }
    }
    if !p.modes.is_empty() {
        if p.modes.len() < 2
            || !p.targets.is_empty()
            || !p.instructions.is_empty()
            || !matches!(
                p.resolution,
                SpellResolution::Instructions
                    | SpellResolution::OneOrMoreModes
                    | SpellResolution::ChooseModes { .. }
                    | SpellResolution::ChooseRepeatedModes { .. }
            )
        {
            return Err("modal root must choose exactly one of multiple complete programs".into());
        }
        for mode in &p.modes {
            let child = &mode.program;
            if !child.modes.is_empty()
                || child.timing != p.timing
                || child.functioning_zone != p.functioning_zone
                || child.resolution != SpellResolution::Instructions
                || child.instructions.is_empty()
            {
                return Err("nested, empty, or inconsistent modal program".into());
            }
            validate_instruction_program(child)?;
        }
        return Ok(());
    }
    let mut ids = BTreeSet::new();
    for (index, target) in p.targets.iter().enumerate() {
        if usize::try_from(target.slot).ok() != Some(index)
            || target.selection_id.is_empty()
            || !ids.insert(&target.selection_id)
        {
            return Err("unbound, duplicate, or noncontiguous target occurrence".into());
        }
    }
    if p.targets
        .windows(2)
        .any(|targets| targets[0].domain != targets[1].domain)
    {
        if graveyard_shuffle::offered_by(p).is_none() {
            return Err("mixed target domains are unsupported".into());
        }
    }
    // Requests expose one common candidate set, and execution uses that common
    // predicate. Distinct per-slot filters need a different request contract;
    // accepting them here would apply the first slot's restriction to all slots.
    if p.targets
        .windows(2)
        .any(|targets| targets[0].filter != targets[1].filter)
    {
        return Err("mixed target filters are unsupported".into());
    }
    for t in &p.targets {
        if let Some(f) = &t.filter {
            if !matches!(
                t.domain,
                TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::StackSpell
            ) || (t.domain == TargetDomain::StackSpell
                && (f.types.is_empty()
                    || !f.non_types.is_empty()
                    || !f.non_colors.is_empty()
                    || f.opponent
                    || f.power.is_some()
                    || f.tapped))
                || f.types
                    .iter()
                    .enumerate()
                    .any(|(i, k)| f.types[..i].contains(k))
                || f.non_types
                    .iter()
                    .enumerate()
                    .any(|(i, k)| f.non_types[..i].contains(k))
                || f.non_colors.contains(&Color::C)
                || f.non_colors
                    .iter()
                    .enumerate()
                    .any(|(i, k)| f.non_colors[..i].contains(k))
                || (t.domain == TargetDomain::BattlefieldPermanent
                    && f.types
                        .iter()
                        .chain(&f.non_types)
                        .any(|k| matches!(k, CardKind::Instant | CardKind::Sorcery)))
            {
                return Err("unsupported permanent/spell target filter".into());
            }
        } else if matches!(
            t.domain,
            TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent
        ) {
            return Err("permanent target requires explicit filter".into());
        }
    }
    let mut used = BTreeSet::new();
    if graveyard_shuffle::offered_by(p).is_some() {
        used.extend([0, 1]);
    }
    fn reference(p: &SpellProgram, slot: u32) -> Result<(u32, TargetDomain), String> {
        let t = p.targets.get(slot as usize).ok_or("unbound LKI target")?;
        if !matches!(
            t.domain,
            TargetDomain::BattlefieldPermanent | TargetDomain::StackSpell
        ) {
            return Err("unsupported LKI target domain".into());
        }
        Ok((slot, t.domain))
    }
    for instruction in &p.instructions {
        let binding = match instruction {
            Instruction::ExileResolvingSpell { .. }
            | Instruction::ProtectControllerUntilNextTurn { .. }
            | Instruction::FreezeControllerLifeUntilNextTurn { .. }
            | Instruction::PhaseOutControllerPermanents { .. }
            | Instruction::MoveSet { .. }
            | Instruction::RemoveOpponentNonlandIndestructible { .. }
            | Instruction::DestroySet { .. }
            | Instruction::PumpSet { .. }
            | Instruction::GrantKeywordsSet { .. }
            | Instruction::EachPlayerSacrifice { .. }
            | Instruction::DrawEachPlayer { .. }
            | Instruction::UntapControllerLands { .. }
            | Instruction::OptionalDrawForSoleGreatestEntrant { .. }
            | Instruction::OptionalDrawCards { .. }
            | Instruction::OptionalDrawUnlessCasterPays { .. }
            | Instruction::DrawDamageAmount { .. } => None,
            Instruction::DamageSet {
                sets,
                each_player,
                amount,
                ..
            } => {
                if sets.is_empty() && !each_player {
                    return Err("empty damage set".into());
                }
                if let WipeAmount::Fixed { value } = amount
                    && value <= &Quantity::zero()
                {
                    return Err("invalid set damage".into());
                }
                None
            }
            Instruction::GainLifeDestroyed { .. } => {
                if !p
                    .instructions
                    .iter()
                    .take_while(|i| !std::ptr::eq(*i, instruction))
                    .any(|i| {
                        matches!(
                            i,
                            Instruction::DestroyAllCreatures { .. }
                                | Instruction::MoveSet {
                                    action: SetMove::Destroy,
                                    ..
                                }
                        )
                    })
                {
                    return Err("destroy result requires preceding destroy".into());
                }
                None
            }
            Instruction::ExileDamagedThisTurn { .. } => {
                if !matches!(p.instructions.first(), Some(Instruction::DamageSet { .. })) {
                    return Err("damage history requires preceding damage".into());
                }
                None
            }
            Instruction::LoseLifeDeathController { amount, .. } => {
                if amount <= &Quantity::zero() {
                    return Err("invalid death loss".into());
                }
                None
            }

            Instruction::SacrificeDamage {
                target_slot,
                recipient,
                amount,
                optional,
                ..
            } => {
                if target_slot.is_some() == recipient.is_some()
                    || recipient.as_ref().is_some_and(|r| {
                        matches!(r, LifeRecipient::Target { .. } | LifeRecipient::EventPlayer)
                    })
                    || (*optional && target_slot.is_none())
                {
                    return Err("invalid sacrifice damage recipient".into());
                }
                match amount {
                    SacrificeDamageAmount::Fixed { value } if *value > Quantity::zero() => {}
                    SacrificeDamageAmount::SacrificedPower
                        if p.additional_sacrifice.as_ref().is_some_and(|f| {
                            f.types == [CardKind::Creature] && f.subtypes.is_empty()
                        }) => {}
                    _ => return Err("invalid sacrifice damage amount".into()),
                }
                target_slot.map(|slot| (slot, TargetDomain::AnyDamageTarget))
            }
            Instruction::EachOpponentSacrifice { .. }
            | Instruction::DrainOpponents { .. }
            | Instruction::ModifySourcePowerToughness { .. } => None,
            Instruction::DestroyPermanent {
                target_slot,
                optional,
                ..
            } => {
                if *optional && p.timing != SpellTiming::Instant {
                    return Err("optional destroy requires a triggered program".into());
                }
                Some((*target_slot, TargetDomain::BattlefieldPermanent))
            }
            Instruction::ExileTargetGraveyard { target_slot, .. } => {
                Some((*target_slot, TargetDomain::AnyPlayer))
            }
            Instruction::ReturnEntryTargetCreature { target_slot, .. }
            | Instruction::ExilePermanent { target_slot, .. }
            | Instruction::PhaseOutTargetPermanent { target_slot, .. }
            | Instruction::UntapPermanent { target_slot, .. } => {
                Some((*target_slot, TargetDomain::BattlefieldPermanent))
            }
            Instruction::TapPermanent { target_slot, .. } => {
                let domain = p
                    .targets
                    .get(*target_slot as usize)
                    .ok_or("unbound tap target")?
                    .domain;
                if !matches!(
                    domain,
                    TargetDomain::BattlefieldCreature | TargetDomain::BattlefieldPermanent
                ) {
                    return Err("unsupported tap target domain".into());
                }
                Some((*target_slot, domain))
            }
            Instruction::ChangeLifeFromTarget {
                recipient, amount, ..
            } => {
                if let EffectRecipient::TargetController { target_slot } = recipient {
                    used.insert(reference(p, *target_slot)?.0);
                }
                match amount {
                    TargetAmount::Fixed { amount } if *amount > Quantity::zero() => {}
                    TargetAmount::Attribute {
                        target_slot,
                        attribute,
                    } => {
                        let (_, domain) = reference(p, *target_slot)?;
                        if domain != TargetDomain::BattlefieldPermanent
                            || (*attribute == TargetAttribute::Power
                                && p.targets[*target_slot as usize]
                                    .filter
                                    .as_ref()
                                    .is_none_or(|f| f.types != [CardKind::Creature]))
                        {
                            return Err("unsupported target attribute".into());
                        }
                        used.insert(*target_slot);
                    }
                    _ => return Err("invalid target life amount".into()),
                }
                None
            }
            Instruction::CreateTokensFor {
                recipient,
                count,
                token,
                ..
            } => {
                if *count == 0 {
                    return Err("token count must be positive".into());
                }
                token.validate()?;
                match recipient {
                    EffectRecipient::Controller => None,
                    EffectRecipient::TargetController { target_slot } => {
                        Some(reference(p, *target_slot)?)
                    }
                }
            }
            Instruction::DamageTargetController {
                target_slot,
                amount,
                ..
            } => {
                if *amount <= Quantity::zero() {
                    return Err("damage quantity must be positive".into());
                }
                Some(reference(p, *target_slot)?)
            }
            Instruction::AttachEquipment { target_slot, .. } => {
                Some((*target_slot, TargetDomain::ControlledBattlefieldCreature))
            }
            Instruction::ReturnAuraFromGraveyard { .. } => None,
            Instruction::DestroyTargetCreature { target_slot, .. } => {
                Some((*target_slot, TargetDomain::BattlefieldCreature))
            }
            Instruction::DestroyAllCreatures { .. } => None,
            Instruction::ReturnOwnGraveyardCreatureToBattlefield { target_slot, .. }
            | Instruction::ReturnOwnGraveyardCreatureToHand { target_slot, .. } => {
                Some((*target_slot, TargetDomain::OwnGraveyardCreatureCard))
            }
            Instruction::ReturnOwnGraveyardCardToHand { target_slot, .. } => {
                Some((*target_slot, TargetDomain::OwnGraveyardCard))
            }
            Instruction::SearchColoredLibrary { colors, .. } => {
                ColoredLibrarySearch::new(colors)?;
                None
            }
            Instruction::SearchTypedLands { search, .. } => {
                search.validate()?;
                None
            }
            Instruction::SearchBasicLandsSplit { .. } => {
                if p.instructions.len() != 1
                    || !p.targets.is_empty()
                    || !p.modes.is_empty()
                    || p.resolution != SpellResolution::Instructions
                {
                    return Err("unsupported basic land split search program".into());
                }
                None
            }
            Instruction::SearchBasicLands { maximum, .. } => {
                if *maximum == 0 || *maximum > 2 {
                    return Err("unsupported basic land search maximum".into());
                }
                None
            }
            Instruction::CreateCreatureTokens { count, token, .. } => {
                if *count == 0 {
                    return Err("token count must be positive".into());
                }
                token.validate()?;
                None
            }
            Instruction::RegisterControllerCreatureEntryThisTurn { .. }
            | Instruction::Hideaway { .. }
            | Instruction::PlayHideawayLinked { .. }
            | Instruction::GrantHandCastCascade { .. }
            | Instruction::OfferHandCastWithoutManaCost { .. }
            | Instruction::Cascade { .. }
            | Instruction::Demonstrate { .. }
            | Instruction::LookTopMayRevealForMana { .. }
            | Instruction::RevealAndCast { .. }
            | Instruction::PutSourcePlusOneCountersForCastColors { .. } => None,
            Instruction::DistributePlusOneCounters { .. } => {
                Some((0, TargetDomain::BattlefieldCreature))
            }
            Instruction::PutPlusOneCounters { on, amount, .. } => {
                if *amount <= Quantity::zero() {
                    return Err("counter amount must be positive".into());
                }
                match on {
                    CounterRecipient::Source => None,
                    CounterRecipient::Target { target_slot } => {
                        Some((*target_slot, TargetDomain::BattlefieldCreature))
                    }
                }
            }
            Instruction::GrantSourceKeywords { .. } => None,
            Instruction::GrantKeywords { target_slot, .. } => {
                // The complete family guard above restricts this to its two domains.
                Some((*target_slot, p.targets[0].domain))
            }
            Instruction::ModifyPowerToughness { target_slot, .. } => {
                let domain = p
                    .targets
                    .get(*target_slot as usize)
                    .map(|target| target.domain)
                    .ok_or("unbound creature pump target")?;
                if !matches!(
                    domain,
                    TargetDomain::BattlefieldCreature | TargetDomain::BlockingBattlefieldCreature
                ) && !(domain == TargetDomain::ControlledBattlefieldCreature
                    && temporary_creature_keyword_program(p).is_some())
                {
                    return Err("unsupported creature pump target domain".into());
                }
                Some((*target_slot, domain))
            }
            Instruction::PreventNextDamage {
                target_slot,
                amount,
                ..
            } => {
                if *amount <= Quantity::zero() {
                    return Err("prevention budget must be positive".into());
                }
                Some((*target_slot, TargetDomain::AnyDamageTarget))
            }
            Instruction::PreventCombatDamage { .. } => None,
            Instruction::ReturnToOwnerHand { target_slot, .. } => {
                let domain = p
                    .targets
                    .get(*target_slot as usize)
                    .ok_or("unbound owner-hand target")?
                    .domain;
                if !matches!(
                    domain,
                    TargetDomain::CreatureOrPlaneswalker
                        | TargetDomain::BattlefieldCreature
                        | TargetDomain::BattlefieldPermanent
                ) {
                    return Err("unsupported owner-hand target domain".into());
                }
                Some((*target_slot, domain))
            }
            Instruction::MoveTriggeredSuccessor { .. } => None,
            Instruction::PutOnTopOwnerLibrary { target_slot, .. } => {
                Some((*target_slot, TargetDomain::BattlefieldCreature))
            }
            Instruction::SacrificeCreature { recipient, .. } => {
                if *recipient != ActionRecipient::EventPlayer {
                    return Err("controller sacrifice is outside supported action semantics".into());
                }
                None
            }
            Instruction::LoseGameTargetPlayer { target_slot, .. } => {
                Some((*target_slot, TargetDomain::AnyPlayer))
            }
            Instruction::TargetPlayerSacrificeCreature { target_slot, .. } => {
                Some((*target_slot, TargetDomain::OpponentPlayer))
            }
            Instruction::CounterSpellAndMillController { target_slot, .. }
            | Instruction::CounterSpellUnlessPays { target_slot, .. } => {
                Some((*target_slot, TargetDomain::StackSpell))
            }
            Instruction::CopyControlledCreatureSpell { target_slot, .. } => {
                Some((*target_slot, TargetDomain::ControlledCreatureSpell))
            }
            Instruction::CounterStackObject { target_slot, .. } => {
                let domain = p
                    .targets
                    .get(*target_slot as usize)
                    .map(|target| target.domain)
                    .filter(|domain| {
                        matches!(
                            domain,
                            TargetDomain::StackSpell
                                | TargetDomain::StackActivatedOrTriggeredAbility
                        )
                    })
                    .ok_or("counter requires a stack target domain")?;
                Some((*target_slot, domain))
            }
            Instruction::Scry { amount, .. } | Instruction::Surveil { amount, .. } => {
                if amount.is_negative() {
                    return Err("scry amount must be nonnegative".into());
                }
                None
            }
            Instruction::DealDamage {
                target_slot,
                amount,
                ..
            } => {
                if *amount <= Quantity::zero() {
                    return Err("damage quantity must be positive".into());
                }
                match p.targets.get(*target_slot as usize).map(|t| t.domain) {
                    Some(
                        domain @ (TargetDomain::AnyDamageTarget
                        | TargetDomain::OpponentPlayer
                        | TargetDomain::PlayerOrPlaneswalker
                        | TargetDomain::BattlefieldCreature),
                    ) => Some((*target_slot, domain)),
                    _ => return Err("unbound fixed damage target".into()),
                }
            }
            Instruction::DealDamageToController { amount, .. } => {
                if *amount <= Quantity::zero() {
                    return Err("damage quantity must be positive".into());
                }
                None
            }
            Instruction::DealDamageAnnouncedX { target_slot, .. } => {
                let domain = p.targets.get(*target_slot as usize).map(|t| t.domain);
                match domain {
                    Some(
                        d @ (TargetDomain::AnyDamageTarget | TargetDomain::BattlefieldCreature),
                    ) => Some((*target_slot, d)),
                    _ => return Err("unbound announced X damage target".into()),
                }
            }
            Instruction::MillAnnouncedX { target_slot, .. } => {
                Some((*target_slot, TargetDomain::AnyPlayer))
            }
            Instruction::GainLife {
                recipient, amount, ..
            }
            | Instruction::LoseLife {
                recipient, amount, ..
            } => {
                if *amount <= Quantity::zero()
                    && !(amount == &Quantity::zero()
                        && matches!(
                            instruction,
                            Instruction::GainLife {
                                recipient: LifeRecipient::Controller,
                                ..
                            }
                        )
                        && temporary_permanent_keyword_program(p).is_some())
                {
                    return Err("life quantity must be positive".into());
                }
                match recipient {
                    LifeRecipient::Controller
                    | LifeRecipient::EachOpponent
                    | LifeRecipient::EventPlayer => None,
                    LifeRecipient::Target { target_slot } => {
                        Some((*target_slot, TargetDomain::AnyPlayer))
                    }
                }
            }
            Instruction::DrawCardsIfAdditionalCost {
                count,
                cost_identity,
                ..
            } => {
                if count <= &Quantity::zero()
                    || !p
                        .optional_additional_costs
                        .iter()
                        .any(|c| c.identity == *cost_identity)
                {
                    return Err("unbound optional-cost draw/quantity".into());
                }
                None
            }
            Instruction::DrawControllerCards { .. }
            | Instruction::GrantLandPlaysThisTurn { .. }
            | Instruction::PutHandOnLibrary { .. }
            | Instruction::OrderLibraryTop { .. }
            | Instruction::ShufflePlayerGraveyard { .. }
            | Instruction::OptionalShuffleLibrary { .. }
            | Instruction::SelectLibraryCard { .. } => None,
            Instruction::MillCards {
                count, recipient, ..
            } => {
                if count.is_negative() {
                    return Err("negative mill count".into());
                }
                match recipient {
                    None => None,
                    Some(LifeRecipient::Target { target_slot }) => {
                        Some((*target_slot, TargetDomain::AnyPlayer))
                    }
                    _ => return Err("unsupported mill recipient".into()),
                }
            }
            Instruction::DrawAnnouncedX { recipient, .. }
            | Instruction::ChangeLifeAnnouncedX { recipient, .. } => match recipient {
                LifeRecipient::Controller => None,
                LifeRecipient::Target { target_slot } => {
                    Some((*target_slot, TargetDomain::AnyPlayer))
                }
                _ => return Err("invalid announced X recipient".into()),
            },
            Instruction::DrawCards { recipient, .. } => match recipient {
                Some(LifeRecipient::EachOpponent) => {
                    return Err("unsupported each-opponent draw".into());
                }
                Some(LifeRecipient::Target { target_slot }) => {
                    Some((*target_slot, TargetDomain::AnyPlayer))
                }
                _ => None,
            },
            Instruction::DiscardPlayerSet { .. } => None,
            Instruction::DiscardTargetPlayer { target_slot, .. }
            | Instruction::DiscardTargetPlayerAtRandom { target_slot, .. } => {
                // The complete discard family already verified this player target.
                Some((
                    *target_slot,
                    p.targets
                        .get(*target_slot as usize)
                        .ok_or("unbound discard target")?
                        .domain,
                ))
            }
            Instruction::DiscardCards { count, .. } => {
                if *count != Quantity::from(1) && !controller_draw_discard_program(p) {
                    return Err("unsupported draw/discard cardinality".into());
                }
                None
            }
        };
        if let Some((slot, domain)) = binding {
            if p.targets
                .get(slot as usize)
                .is_none_or(|target| target.domain != domain)
            {
                return Err("unbound target slot/domain".into());
            }
            used.insert(slot);
        }
    }
    if used.len() != p.targets.len() {
        return Err("unused target occurrence".into());
    }
    Ok(())
}
impl Instruction {
    pub fn uses_event_player(&self) -> bool {
        matches!(
            self,
            Self::DrawCards {
                recipient: Some(LifeRecipient::EventPlayer),
                ..
            } | Self::GainLife {
                recipient: LifeRecipient::EventPlayer,
                ..
            } | Self::LoseLife {
                recipient: LifeRecipient::EventPlayer,
                ..
            }
        )
    }
}
fn has_blocking_creature_target(program: &SpellProgram) -> bool {
    program
        .targets
        .iter()
        .any(|t| t.domain == TargetDomain::BlockingBattlefieldCreature)
        || program
            .modes
            .iter()
            .any(|m| has_blocking_creature_target(&m.program))
}
fn has_destroy_permanent(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::DestroyPermanent { .. }))
        || program
            .modes
            .iter()
            .any(|mode| has_destroy_permanent(&mode.program))
}
fn has_counter_stack_object(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::CounterStackObject { .. }))
        || program
            .modes
            .iter()
            .any(|mode| has_counter_stack_object(&mode.program))
}
fn has_permanent_owner_hand(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|instruction| match instruction {
            Instruction::ReturnToOwnerHand { target_slot, .. } => program
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
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::TapPermanent { .. }))
        || program.modes.iter().any(|m| has_tap(&m.program))
}
fn has_untap(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::UntapPermanent { .. }))
        || program.modes.iter().any(|m| has_untap(&m.program))
}
fn has_damage_quantity_draw(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::DrawDamageAmount { .. }))
        || program
            .modes
            .iter()
            .any(|m| has_damage_quantity_draw(&m.program))
}
fn has_controller_discard(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::DiscardCards {
                recipient: None | Some(ActionRecipient::Controller),
                ..
            }
        )
    }) || program
        .modes
        .iter()
        .any(|m| has_controller_discard(&m.program))
}
// Intrinsic keyword capability closure is shared by Rust and exported schema.
fn intrinsic_keyword_capability(keyword: KeywordAbility) -> Option<Capability> {
    Some(match keyword {
        KeywordAbility::ProtectionFromEachColor => return None,
        KeywordAbility::ProtectionFromEverything => Capability::ProtectionFromEverythingV1,
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
    add_program_requirements_for_activation(set, program, false);
}

fn add_program_requirements_for_activation(
    set: &mut BTreeSet<Capability>,
    program: &SpellProgram,
    mana_self_pump: bool,
) {
    if !program.optional_additional_costs.is_empty() {
        set.insert(Capability::OptionalAdditionalManaCostV1);
    }
    if program.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::PutSourcePlusOneCountersForCastColors { .. }
                | Instruction::DistributePlusOneCounters { .. }
        )
    }) {
        set.insert(Capability::PlusOneCountersV1);
    }
    if has_permanent_owner_hand(program) {
        // Existing creature-bounce and permanent-target capabilities alone do
        // not authorize this new composition in hosted/search profiles.
        set.insert(Capability::PermanentOwnerHandMoveV1);
    }
    if program.additional_sacrifice.is_some()
        || program.instructions.iter().any(|i| {
            matches!(
                i,
                Instruction::GainLife {
                    recipient: LifeRecipient::EachOpponent,
                    ..
                } | Instruction::LoseLife {
                    recipient: LifeRecipient::EachOpponent,
                    ..
                }
            )
        })
    {
        set.insert(Capability::SacrificeV1);
    }
    if !program.modes.is_empty() {
        set.insert(Capability::ModalChoiceV1);
        if matches!(
            program.resolution,
            SpellResolution::ChooseRepeatedModes { .. }
        ) {
            set.insert(Capability::RepeatedTargetModesV1);
        }
        if matches!(program.resolution, SpellResolution::ChooseModes { .. }) {
            set.insert(Capability::FixedTargetModesV1);
        }
        if akromas_will_program(program) {
            set.insert(Capability::CommanderModalKeywordGrantV1);
        }
        if program.resolution == SpellResolution::OneOrMoreModes {
            set.insert(Capability::NonemptyModeSubsetV1);
        }
        for mode in &program.modes {
            add_program_requirements(set, &mode.program);
        }
    }
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
    if matches!(
        program.instructions.as_slice(),
        [Instruction::ChangeLifeAnnouncedX {
            recipient: LifeRecipient::Target { target_slot: 0 },
            gain: true,
            source_block: 0
        }]
    ) {
        set.insert(Capability::AnnouncedXEffectsV1);
    }
    for instruction in &program.instructions {
        if matches!(
            instruction,
            Instruction::CounterSpellAndMillController { .. }
        ) {
            set.insert(Capability::CounterStackObjectV1);
        }
        if matches!(instruction, Instruction::DealDamageAnnouncedX { .. }) {
            set.insert(Capability::NoncombatDamageV1);
        }
        if matches!(
            instruction,
            Instruction::OptionalDrawUnlessCasterPays { .. }
        ) {
            set.insert(Capability::OptionalControllerDrawV1);
        }
        if instruction.uses_event_player() {
            set.insert(Capability::EventPlayerRecipientV1);
        }
        if matches!(instruction, Instruction::CreateCreatureTokens { token, .. }
            | Instruction::CreateTokensFor { token, .. } if !token.additional_types.is_empty())
        {
            set.insert(Capability::MultiTypeCreatureTokensV1);
        }
        if let Instruction::CreateCreatureTokens { token, .. } = instruction
            && token
                .keywords
                .iter()
                .any(|k| matches!(k, KeywordAbility::Hexproof | KeywordAbility::Shroud))
        {
            set.insert(Capability::AttachmentsV1);
        }
        if let Instruction::GrantKeywords { keywords, .. }
        | Instruction::GrantKeywordsSet { keywords, .. }
        | Instruction::GrantSourceKeywords { keywords, .. } = instruction
        {
            for keyword in keywords {
                if let Some(capability) = temporary_keyword_primitive_capability(*keyword) {
                    set.insert(capability);
                }
            }
        }
        if matches!(
            instruction,
            Instruction::OfferHandCastWithoutManaCost { .. }
        ) {
            set.insert(Capability::ResolutionCastV1);
        }
        if matches!(
            instruction,
            Instruction::DamageSet {
                amount: WipeAmount::OtherCastsManaValue,
                ..
            }
        ) {
            set.extend([Capability::CastManaValueSweepV1, Capability::CastHistoryV1]);
        }
        if let Instruction::RegisterControllerCreatureEntryThisTurn { program, .. } = instruction {
            add_program_requirements(set, program);
        }
        if matches!(
            instruction,
            Instruction::Cascade { .. } | Instruction::RevealAndCast { .. }
        ) {
            set.insert(Capability::ResolutionCastV1);
            set.insert(Capability::RandomBottomV1);
        }

        if matches!(
            instruction,
            Instruction::GrantKeywords {
                modifier: Some(_),
                ..
            }
        ) {
            set.insert(Capability::TemporaryPowerToughnessV1);
        }
        if matches!(instruction, Instruction::GrantKeywordsSet { .. }) {
            set.insert(Capability::BoardWipesV1);
        }
        if matches!(instruction, Instruction::CounterSpellUnlessPays { .. }) {
            set.insert(Capability::CounterStackObjectV1);
        }
        if matches!(instruction, Instruction::DrainOpponents { .. }) {
            set.extend([Capability::LifeLossV1, Capability::LifeGainV1]);
        }
        if matches!(instruction, Instruction::Hideaway { .. }) {
            set.insert(Capability::RandomBottomV1);
        }
        if matches!(instruction, Instruction::PlayHideawayLinked { .. }) {
            set.insert(Capability::ResolutionCastV1);
        }
        if matches!(instruction, Instruction::GrantHandCastCascade { .. }) {
            set.extend([
                Capability::CascadeV1,
                Capability::ResolutionCastV1,
                Capability::RandomBottomV1,
            ]);
        }
        set.insert(match instruction {
            Instruction::ExileResolvingSpell { .. } => Capability::ExileResolvingSpellV1,
            Instruction::ProtectControllerUntilNextTurn { .. } => {
                Capability::ControllerProtectionUntilNextTurnV1
            }
            Instruction::FreezeControllerLifeUntilNextTurn { .. } => {
                Capability::ControllerLifeFreezeV1
            }
            Instruction::PhaseOutControllerPermanents { .. } => {
                Capability::ControlledPermanentPhasingV1
            }
            Instruction::PhaseOutTargetPermanent { .. } => Capability::TargetedPhasingV1,
            Instruction::RemoveOpponentNonlandIndestructible { .. } => {
                Capability::RemoveIndestructibleV1
            }
            Instruction::RegisterControllerCreatureEntryThisTurn { .. } => {
                Capability::DelayedControllerCreatureEntryV1
            }
            Instruction::Hideaway { .. } => Capability::HideawayV1,
            Instruction::PlayHideawayLinked { .. } => Capability::LinkedExilePlayV1,
            Instruction::GrantHandCastCascade { .. } => Capability::HandCastCascadeGrantV1,
            Instruction::OfferHandCastWithoutManaCost { .. } => Capability::CombatHandCastV1,
            Instruction::Cascade { .. } => Capability::CascadeV1,
            Instruction::LookTopMayRevealForMana { .. } => Capability::LookTopMayRevealForManaV1,
            Instruction::RevealAndCast { .. } => Capability::RevealAndCastV1,
            Instruction::Demonstrate { .. } => Capability::DemonstrateV1,
            Instruction::GrantKeywordsSet {
                set: ObjectSet::ControllerPermanents,
                ..
            } => Capability::TemporaryPermanentSetKeywordProgramV1,
            Instruction::GrantKeywordsSet {
                set: ObjectSet::ControllerCreatures | ObjectSet::AttackingCreatures,
                ..
            } => Capability::TemporaryCreatureSetKeywordProgramV1,
            Instruction::GrantKeywordsSet { .. } => Capability::BoardWipesV1,
            Instruction::MoveSet { .. }
            | Instruction::DestroySet { .. }
            | Instruction::PumpSet { .. }
            | Instruction::DamageSet { .. }
            | Instruction::GainLifeDestroyed { .. }
            | Instruction::ExileDamagedThisTurn { .. }
            | Instruction::EachPlayerSacrifice { .. }
            | Instruction::LoseLifeDeathController { .. } => Capability::BoardWipesV1,
            Instruction::ModifySourcePowerToughness { .. } if mana_self_pump => {
                Capability::TemporaryPowerToughnessV1
            }
            Instruction::SacrificeDamage { .. }
            | Instruction::EachOpponentSacrifice { .. }
            | Instruction::ModifySourcePowerToughness { .. } => Capability::SacrificeV1,
            Instruction::DestroyPermanent { .. }
            | Instruction::ExilePermanent { .. }
            | Instruction::ChangeLifeFromTarget { .. }
            | Instruction::CreateTokensFor { .. }
            | Instruction::DamageTargetController { .. } => Capability::PermanentRemovalV1,
            Instruction::AttachEquipment { .. } | Instruction::ReturnAuraFromGraveyard { .. } => {
                Capability::AttachmentsV1
            }
            Instruction::DestroyTargetCreature { .. } | Instruction::DestroyAllCreatures { .. } => {
                Capability::DestroyCreatureV1
            }
            Instruction::ReturnOwnGraveyardCreatureToBattlefield { .. } => {
                Capability::GraveyardBattlefieldReturnV1
            }
            Instruction::ReturnOwnGraveyardCreatureToHand { .. } => {
                Capability::GraveyardRecursionV1
            }
            Instruction::ReturnOwnGraveyardCardToHand { .. } => {
                Capability::OwnGraveyardCardReturnV1
            }
            Instruction::SearchColoredLibrary { .. } => Capability::ColoredLibrarySearchV1,
            Instruction::SearchTypedLands { .. } => Capability::TypedLandSearchV1,
            Instruction::SearchBasicLandsSplit { .. } => Capability::BasicLandSplitSearchV1,
            Instruction::SearchBasicLands { .. } => Capability::BasicLandSearchV1,
            Instruction::DealDamageAnnouncedX { .. } | Instruction::MillAnnouncedX { .. } => {
                Capability::AnnouncedXEffectsV1
            }
            Instruction::DrawAnnouncedX { .. } => Capability::AnnouncedXDrawV1,
            Instruction::DrawEachPlayer { .. } => Capability::EachPlayerDrawV1,
            Instruction::DiscardPlayerSet { .. } => Capability::GroupHandDiscardV1,
            Instruction::DiscardTargetPlayer { .. } => Capability::TargetPlayerDiscardV1,
            Instruction::DiscardTargetPlayerAtRandom { .. } => Capability::RandomPlayerDiscardV1,
            Instruction::LoseGameTargetPlayer { .. } => Capability::TargetedGameLossV1,
            Instruction::ChangeLifeAnnouncedX { gain: true, .. } => Capability::LifeGainV1,
            Instruction::ChangeLifeAnnouncedX { gain: false, .. } => Capability::LifeLossV1,
            Instruction::DrawCards { .. } if targeted_counted_draw_program(program) => {
                Capability::TargetedCountedDrawV1
            }
            Instruction::DrawCards {
                recipient: Some(_), ..
            } => Capability::TargetPlayerDrawV1,
            Instruction::CreateCreatureTokens { .. } => Capability::CreatureTokensV1,
            Instruction::PutSourcePlusOneCountersForCastColors { .. } => {
                Capability::CastSpellColorCountersV1
            }
            Instruction::DistributePlusOneCounters { .. } => Capability::DividedPlusOneCountersV1,
            Instruction::PutPlusOneCounters { .. } => Capability::PlusOneCountersV1,
            Instruction::GrantSourceKeywords { .. } => Capability::ActivatedSourceKeywordGrantV1,
            Instruction::GrantKeywords { .. } => {
                if temporary_permanent_keyword_program(program).is_some() {
                    Capability::TemporaryPermanentKeywordProgramV1
                } else {
                    temporary_creature_keyword_capability(program)
                }
            }
            Instruction::ModifyPowerToughness { .. } => Capability::TemporaryPowerToughnessV1,
            Instruction::PreventCombatDamage { .. } => Capability::CombatDamagePreventionV1,
            Instruction::PreventNextDamage { .. } => Capability::DamageBudgetPreventionV1,
            Instruction::ExileTargetGraveyard { .. } => Capability::EntryGraveyardExileV1,
            Instruction::ReturnEntryTargetCreature { .. } => Capability::CreatureEntryReturnV1,
            Instruction::ReturnToOwnerHand { .. } => Capability::OwnerHandMoveV1,
            Instruction::PutOnTopOwnerLibrary { .. } => Capability::OwnerLibraryTopMoveV1,
            Instruction::MoveTriggeredSuccessor { .. } => {
                Capability::TriggeredSuccessorLibraryTuckV1
            }
            Instruction::TargetPlayerSacrificeCreature { .. } => {
                Capability::TargetPlayerSacrificeCreatureV1
            }
            Instruction::SacrificeCreature { .. } => Capability::EventPlayerSacrificeV1,
            Instruction::DrawCards { .. } | Instruction::DrawCardsIfAdditionalCost { .. } => {
                Capability::ControllerDrawV1
            }
            Instruction::DrawControllerCards { .. } | Instruction::PutHandOnLibrary { .. } => {
                if controller_counted_draw_program(program)
                    || controller_counted_draw::standalone_controller_draw(program).is_some()
                {
                    Capability::ControllerCountedDrawV1
                } else if controller_draw_discard_program(program) {
                    Capability::ControllerDrawDiscardV1
                } else if hand_return::controller_draw_land_allowance_program(program) {
                    Capability::ControllerDrawLandAllowanceV1
                } else {
                    Capability::ControllerHandReturnV1
                }
            }
            Instruction::GrantLandPlaysThisTurn { .. } => Capability::ControllerDrawLandAllowanceV1,
            Instruction::UntapControllerLands { .. } => Capability::UntapControllerLandsV1,
            Instruction::OptionalDrawForSoleGreatestEntrant { .. } => Capability::GreatestPowerV1,
            Instruction::OptionalDrawCards { .. } => Capability::OptionalControllerDrawV1,
            Instruction::OptionalDrawUnlessCasterPays { .. } => Capability::CastPayerDrawV1,
            Instruction::DrawDamageAmount { .. } => Capability::DamageQuantityDrawV1,
            Instruction::UntapPermanent { .. } => Capability::UntapPermanentV1,
            Instruction::TapPermanent { .. } => Capability::TapPermanentV1,
            Instruction::DiscardCards {
                recipient: Some(ActionRecipient::EventPlayer),
                ..
            } => Capability::EventPlayerDiscardV1,
            Instruction::DiscardCards { .. } => {
                if controller_draw_discard_program(program) {
                    Capability::ControllerDrawDiscardV1
                } else {
                    Capability::ControllerDiscardV1
                }
            }
            Instruction::CounterSpellAndMillController { .. } => {
                Capability::CounterSpellControllerMillV1
            }
            Instruction::CounterStackObject { .. } => Capability::CounterStackObjectV1,
            Instruction::CounterSpellUnlessPays { .. } => Capability::CounterUnlessGenericV1,
            Instruction::CopyControlledCreatureSpell { .. } => {
                Capability::ControlledCreatureSpellCopyV1
            }
            Instruction::DealDamage { .. } | Instruction::DealDamageToController { .. } => {
                Capability::NoncombatDamageV1
            }
            Instruction::Scry { .. } => Capability::ControllerScryV1,
            Instruction::Surveil { .. } => Capability::ControllerSurveilV1,
            Instruction::MillCards { .. } => Capability::FixedMillProgramV1,
            Instruction::SelectLibraryCard { .. } => Capability::ControllerLibrarySelectionV1,
            Instruction::ShufflePlayerGraveyard { .. } => Capability::PlayerGraveyardShuffleV1,
            Instruction::OrderLibraryTop { .. } | Instruction::OptionalShuffleLibrary { .. } => {
                Capability::ControllerLibraryOrderV1
            }
            Instruction::GainLife { .. } => Capability::LifeGainV1,
            Instruction::DrainOpponents { .. } => Capability::OpponentLifeDrainV1,
            Instruction::LoseLife { .. } => Capability::LifeLossV1,
        });
    }
}
pub fn basic_land_color(subtype: &str) -> Option<Color> {
    match subtype {
        "Plains" => Some(Color::W),
        "Island" => Some(Color::U),
        "Swamp" => Some(Color::B),
        "Mountain" => Some(Color::R),
        "Forest" => Some(Color::G),
        _ => None,
    }
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
        let targets_union = self.definitions.iter().any(|d| {
            d.capabilities
                .contains(&Capability::PlayerOrPlaneswalkerTargetV1)
        });
        let mut ids = BTreeSet::new();
        for d in &self.definitions {
            d.validate()?;
            if targets_union
                && std::iter::once(d.characteristics.kind)
                    .chain(d.characteristics.additional_types.iter().copied())
                    .any(artifact_modal::requires_planeswalker_target_support)
            {
                return Err(
                    "planeswalker objects require executable loyalty and damage rules".into(),
                );
            }
            if d.triggered.iter().any(|a| {
                a.program
                    .instructions
                    .iter()
                    .any(|i| matches!(i, Instruction::LoseLifeDeathController { .. }))
                    && a.condition != TriggerCondition::OpponentCreatureDies
            }) {
                return Err("unbound death controller".into());
            }
            if !ids.insert(&d.oracle_id) {
                return Err("duplicate Oracle identity".into());
            }
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
fn counter_placement_schema(schema: &mut serde_json::Value) {
    let effect = serde_json::json!({"type":"object","required":["controller_creature_plus_one_counter_replacement"]});
    schema["$defs"]["CounterPlacementModification"]["allOf"] = serde_json::json!([{"properties": {
        "amount":{"pattern":"^[1-9][0-9]*$"}, "factor":{"pattern":"^[1-9][0-9]*$"}
    }}]);
    schema["$defs"]["StaticAbility"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":{"required":["effect"],"properties":{"effect":effect.clone()}},
            "then":{"properties":{"functioning_zone":{"const":"battlefield"}}}
        }));
    schema["$defs"]["Definition"].as_object_mut().unwrap().entry("allOf").or_insert_with(||serde_json::json!([])).as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{"effect":effect}}}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["creature","artifact","enchantment"]}}},"capabilities":{"contains":{"const":Capability::ControllerCreaturePlusOneCounterReplacementV1}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerCreaturePlusOneCounterReplacementV1}}}}}
    }));
}
// CR 602.2b, 601.2h, 701.21a. Only an artifact set and a single controller
// draw are executable in this cost family. Source admission is separate.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
fn fixed_artifact_sacrifice_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let counted = json!({"required":["cost"],"properties":{"cost":{"type":"object",
        "required":["sacrifice_permanent"],"properties":{"sacrifice_permanent":{
            "required":["count"],"properties":{"count":{"minimum":2}}
        }}
    }}});
    let draw = json!({"properties":{"targets":{"maxItems":0}, "instructions":{"minItems":1,"maxItems":1,
        "items":{"properties":{"op":{"const":"draw_cards"},"recipient":{"type":"null"},"count":{"const":"1"}}}}}});
    let damage = json!({"properties":{
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"domain":{"enum":["any_damage_target","player_or_planeswalker"]},"stage":{"const":"announcement"},
            "chooser":{"const":"controller"},"filter":{"type":"null"},"selection_id":{"minLength":1}}}},
        "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{
            "op":{"const":"deal_damage"},"target_slot":{"const":0},"amount":{"pattern":"^[1-9][0-9]*$"}}}}
    }});
    let life = json!({"properties":{"targets":{"maxItems":0},"instructions":{"minItems":1,"maxItems":1,"items":{
        "properties":{"op":{"const":"gain_life"},"recipient":{"const":"controller"},"amount":{"pattern":"^[1-9][0-9]*$"}}}}}});
    let pump = json!({"properties":{"targets":{"minItems":1,"maxItems":1,"items":{"properties":{
        "slot":{"const":0},"domain":{"const":"battlefield_creature"},"stage":{"const":"announcement"},
        "chooser":{"const":"controller"},"filter":{"type":"null"},"selection_id":{"pattern":"^line:(0|[1-9][0-9]*):1$"}}}},
        "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"modify_power_toughness"},
        "target_slot":{"const":0},"duration":{"const":"until_end_of_turn"}}}}}});
    let child = json!({"anyOf":[draw,damage,life,pump],"properties":{
        "modes":{"maxItems":0},"timing":{"const":"instant"},
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}
    }});
    let ability = json!({"if":counted,"then":{"properties":{
        "functioning_zone":{"const":"battlefield"},
        "cost":{"properties":{"sacrifice_permanent":{"properties":{
            "filter":{"const":{"types":["artifact"],"subtypes":[]}},
            "mana":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"},"colored":{"items":{"enum":["W","U","B","R","G"]}}}}
        }}}},
        "program":{"properties":{"functioning_zone":{"const":"stack"},"timing":{"const":"instant"},
            "resolution":{"const":"instructions"},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0}},
            "if":{"required":["modes"],"properties":{"modes":{"minItems":1}}},
            "then":{"properties":{"modes":{"minItems":2,"maxItems":u32::MAX,"items":{"properties":{"program":child}}}}},
            "else":draw
        }
    }}});
    let narrow_target = json!({"required":["domain"],"properties":{"domain":{"const":TargetDomain::PlayerOrPlaneswalker}}});
    let free = json!({"$ref":"#/$defs/PlayerOrPlaneswalkerFreeProgram"});
    schema["$defs"]["PlayerOrPlaneswalkerFreeProgram"] = json!({"properties":{
        "targets":{"items":{"not":narrow_target}},"modes":{"items":{"properties":{"program":free}}}
    }});
    let has_union = json!({"required":["activated"],"properties":{"activated":{"contains":{"required":["program"],
        "properties":{"program":{"not":free}}}}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"spell":{"if":{"type":"object"},"then":free},"triggered":{"items":{"properties":{"program":free}}},
            "activated":{"items":{"if":{"not":counted},"then":{"properties":{"program":free}}}}},
        "if":has_union,"then":{"properties":{"capabilities":{"allOf":[{"contains":{"const":Capability::PlayerOrPlaneswalkerTargetV1}},
            {"contains":{"const":Capability::FixedArtifactSacrificeCostV1}},{"contains":{"const":Capability::NoncombatDamageV1}}]}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::PlayerOrPlaneswalkerTargetV1}}}}}
    }));
    let contains_cap = json!({"contains":{"const":Capability::FixedArtifactSacrificeCostV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"activated":{"items":ability}},
        "if":{"required":["activated"],"properties":{"activated":{"contains":counted}}},
        "then":{"properties":{
            "characteristics":{"properties":{"kind":{"enum":["creature","artifact"]},"subtypes":{"not":{"contains":{"const":"Equipment"}}}}},
            "capabilities":contains_cap
        }},
        "else":{"properties":{"capabilities":{"not":contains_cap}}}
    }));
}
fn cascade_schema(schema: &mut serde_json::Value) {
    schema["$defs"]["Definition"]["properties"]["triggered"]["maxItems"] =
        serde_json::json!(u32::MAX);
    let cascade = serde_json::json!({"required":["op"],"properties":{"op":{"const":"cascade"}}});
    let self_condition = serde_json::json!({"required":["condition"],"properties":{"condition":{"const":"self_spell_cast"}}});
    let ability = serde_json::json!({"properties":{
        "functioning_zone":{"const":"stack"},"intervening_if":{"type":"null"},"death_filter":{"type":"null"},
        "program":{"properties":{"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},"timing":{"const":"instant"},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"targets":{"maxItems":0},"modes":{"maxItems":0},"instructions":{"minItems":1,"maxItems":1,"items":cascade}}}
    }});
    schema["$defs"]["CascadeFreeProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"not":cascade}},"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/CascadeFreeProgram"}}}}
    }});
    schema["$defs"]["TriggeredAbility"].as_object_mut().unwrap().entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap().push(serde_json::json!({
        "if":self_condition,"then":{"anyOf":[ability,creative_technique::demonstrate_trigger_schema()]},
        "else":{"properties":{"functioning_zone":{"const":"battlefield"},"program":{"$ref":"#/$defs/CascadeFreeProgram"}}}
    }));
    let grant_effect = serde_json::json!({"type":"object","required":["grant_spell_cascade"]});
    let grant = serde_json::json!({"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{"effect":grant_effect}}}}});
    schema["$defs"]["Definition"]["properties"]["static_abilities"]["maxItems"] =
        serde_json::json!(u32::MAX);
    schema["$defs"]["StaticEffect"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .unwrap()
        .push(
            serde_json::json!({"properties":{"grant_spell_cascade":{"properties":{
                "eligibility":{"properties":{"minimum":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"}}},
                "program":ability["properties"]["program"]
            }}}}),
        );
    schema["$defs"]["StaticAbility"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({"if":{"required":["effect"],"properties":{"effect":grant_effect}},"then":{"properties":{"functioning_zone":{"const":"battlefield"}}}}));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":grant,"then":{"properties":{
            "capabilities":{"contains":{"const":"static_spell_cascade_grant_v1"}},
            "characteristics":{"properties":{"kind":{"enum":["creature","artifact","enchantment"]},"subtypes":{"not":{"contains":{"enum":["Equipment","Aura"]}}}}}
        }},"else":{"properties":{"capabilities":{"not":{"contains":{"const":"static_spell_cascade_grant_v1"}}}}}
    }));
    let history = serde_json::json!({"required":["static_abilities"],"properties":{"static_abilities":{"contains":{
        "required":["effect"],"properties":{"effect":{"type":"object","required":["grant_spell_cascade"],"properties":{"grant_spell_cascade":{
            "required":["eligibility"],"properties":{"eligibility":{"required":["eligibility"],"properties":{"eligibility":{"enum":["first_cast_each_turn","first_cast_from_exile_each_turn"]}}}}
        }}}}
    }}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"anyOf":[history,cast_mana_value_sweep::spell_schema()]},"then":{"properties":{"capabilities":{"contains":{"const":"cast_history_v1"}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":"cast_history_v1"}}}}}
    }));
    let cascade_trigger = serde_json::json!({"allOf":[self_condition,ability]});
    let present = serde_json::json!({"anyOf":[grant,{"required":["triggered"],"properties":{"triggered":{"contains":{"anyOf":[cascade_trigger,{"const":hand_cast_cascade::hand_cast_cascade_ability()}]}}}}]});
    let requirements = [Capability::ResolutionCastV1, Capability::RandomBottomV1];
    let yes: Vec<_> = requirements
        .iter()
        .map(|cap| serde_json::json!({"contains":{"const":cap}}))
        .collect();
    let no: Vec<_> = requirements
        .iter()
        .map(|cap| {
            let absent = serde_json::json!({"not":{"contains":{"const":cap}}});
            if *cap == Capability::ResolutionCastV1 {
                serde_json::json!({"if":{"contains":{"const":Capability::CombatHandCastV1}},
                "then":{"contains":{"const":cap}},"else":absent})
            } else {
                absent
            }
        })
        .collect();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        serde_json::json!({"properties":{"spell":{"anyOf":[{"type":"null"},{"$ref":"#/$defs/CascadeFreeProgram"}]},"activated":{"items":{"properties":{"program":{"$ref":"#/$defs/CascadeFreeProgram"}}}}}}),
        serde_json::json!({"if":present,
            "then":{"properties":{"capabilities":{"contains":{"const":Capability::CascadeV1}}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CascadeV1}}}}}
        }),
        serde_json::json!({"if":{"anyOf":[present,creative_technique::reveal_spell_schema(),linked_hideaway::capability_schema()]},
            "then":{"properties":{"capabilities":{"allOf":yes}}},"else":{"properties":{"capabilities":{"allOf":no}}}
        })
    ]);
}
fn extend_multitype_token_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let additional_types =
        json!({"required":["additional_types"],"properties":{"additional_types":{"minItems":1}}});
    let excluded_subtypes: Vec<_> = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .copied()
        .chain(["Aura", "Equipment"])
        .collect();
    schema["$defs"]["CreatureToken"]["allOf"] = json!([{
        "if":additional_types,"then":{"properties":{"subtypes":{"items":{"not":{"enum":excluded_subtypes}}}}}
    }]);
    let free = json!({"$ref":"#/$defs/SingleTypeTokenProgram"});
    schema["$defs"]["SingleTypeTokenProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["token"],"properties":{"token":additional_types}}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    let no_multitype = json!({"properties":{"program":free}});
    let cap = json!({"contains":{"const":Capability::MultiTypeCreatureTokensV1}});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"allOf":[
                {"properties":{"spell":{"anyOf":[{"type":"null"},free]}}},
                {"properties":{"activated":{"items":no_multitype}}},
                {"properties":{"triggered":{"items":no_multitype}}}
            ]},
            "then":{"properties":{"capabilities":{"not":cap}}},
            "else":{"properties":{"capabilities":cap}}
        }));
}

pub fn catalog_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Catalog)).expect("schema is JSON");
    counter_placement_schema(&mut schema);
    let intrinsic_keywords = schema["$defs"]["KeywordAbility"]["enum"]
        .as_array()
        .unwrap()
        .clone();
    for value in intrinsic_keywords {
        let keyword: KeywordAbility = serde_json::from_value(value.clone()).unwrap();
        let Some(capability) = intrinsic_keyword_capability(keyword) else {
            schema["$defs"]["Definition"]["properties"]["keywords"]["items"] = serde_json::json!({"allOf":[{"$ref":"#/$defs/KeywordAbility"},{"not":{"const":value}}]});
            continue;
        };
        schema["$defs"]["Definition"].as_object_mut().unwrap()
            .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
            .push(serde_json::json!({
                "if":{"required":["keywords"],"properties":{"keywords":{"contains":{"const":value}}}},
                "then":{"properties":{"capabilities":{"contains":{"const":capability}}}}
            }));
    }

    // CR 702.12/702.6: preserve the bounded Equipment modifier and the
    // separate battlefield grant to other controlled permanents.
    schema["$defs"]["CreatureToken"]["properties"]["additional_types"] = serde_json::json!({"type":"array","maxItems":1,"items":{"enum":["artifact","enchantment"]}});
    extend_multitype_token_schema(&mut schema);
    schema["$defs"]["CreatureToken"]["properties"]["keywords"]["items"] = serde_json::json!({"allOf":[{"$ref":"#/$defs/KeywordAbility"},{"not":{"enum":["flash","indestructible","protection_from_each_color","protection_from_everything"]}}]});
    let grant = serde_json::json!({"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{"effect":{"type":"object","required":["modify_creatures"],"properties":{"modify_creatures":{"required":["keywords"],"properties":{"keywords":{"contains":{"const":"indestructible"}}}}}}}}}}});
    let permanent_grant_effect = serde_json::json!({
        "const": StaticEffect::GrantOtherControlledPermanentsIndestructible
    });
    let permanent_grant = serde_json::json!({"required":["static_abilities"],"properties":{
        "static_abilities":{"contains":{"required":["effect"],"properties":{"effect":permanent_grant_effect}}}
    }});
    let temporary_keyword = |keyword: &str| serde_json::json!({"required":["spell"],"properties":{"spell":{"type":"object","required":["instructions"],"properties":{"instructions":{"contains":{"required":["op","keywords"],"properties":{"op":{"const":"grant_keywords"},"keywords":{"contains":{"const":keyword}}}}}}}}});
    let temporary_set_keyword = |keyword: KeywordAbility| {
        let program = serde_json::json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":{"required":["op","keywords"],"properties":{"op":{"const":"grant_keywords_set"},"keywords":{"contains":{"const":keyword}}}}}}});
        serde_json::json!({"anyOf":[
            {"required":["spell"],"properties":{"spell":program}},
            {"required":["activated"],"properties":{"activated":{"contains":{"required":["program"],"properties":{"program":program}}}}}
        ]})
    };
    let intrinsic = serde_json::json!({"required":["keywords"],"properties":{"keywords":{"contains":{"const":"indestructible"}}}});
    let self_indestructible =
        activated_source_keyword::schema_presence(Some(KeywordAbility::Indestructible));
    schema["$defs"]["StaticAbility"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({
            "if":{"required":["effect"],"properties":{"effect":{"type":"object","required":["modify_creatures"],"properties":{"modify_creatures":{"required":["keywords"],"properties":{"keywords":{"contains":{"const":"indestructible"}}}}}}}},
            "then":{"properties":{"functioning_zone":{"const":"battlefield"}}}
        }));
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":{"required":["effect"],"properties":{"effect":permanent_grant_effect}},
            "then":{"properties":{"functioning_zone":{"const":"battlefield"}}}
        }));
    schema["$defs"]["StaticEffect"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({"properties":{"modify_creatures":{
            "properties":{"keywords":{"items":{"not":{"enum":["flash","protection_from_each_color","protection_from_everything"]}}}},
            "if":{"required":["keywords"],"properties":{"keywords":{"contains":{"const":"indestructible"}}}},
            "then":{"properties":{"scope":{"const":"equipped"},"power":{"const":"0"},"toughness":{"const":"0"},"keywords":{"const":["indestructible"]},"restrictions":{"const":[]}}}
        }}}));
    schema["$defs"]["Definition"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .extend([
            serde_json::json!({"if":{"anyOf":[intrinsic.clone(),grant.clone(),permanent_grant.clone(),temporary_keyword("indestructible"),temporary_set_keyword(KeywordAbility::Indestructible),self_indestructible,{"required":["spell"],"properties":{"spell":{"$ref":"#/$defs/CommanderModalKeywordProgram"}}}]},"then":{"properties":{"capabilities":{"contains":{"const":"indestructible_v1"}}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":"indestructible_v1"}}}}}}),
            serde_json::json!({"if":permanent_grant,"then":{"properties":{
                "characteristics":{"properties":{"kind":{"const":"creature"}}},
                "static_abilities":{"maxItems":1},"activated":{"maxItems":0},
                "mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},
                "capabilities":{"contains":{"const":Capability::StaticPermanentIndestructibleGrantV1}}
            }},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::StaticPermanentIndestructibleGrantV1}}}}}}),
            serde_json::json!({"if":intrinsic,"then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["creature","artifact","enchantment","land"]}}}}}}),
            serde_json::json!({"if":grant,"then":{"properties":{"characteristics":{"properties":{"kind":{"const":"artifact"},"subtypes":{"const":["Equipment"]}}}}}}),
            serde_json::json!({"if":{"properties":{"characteristics":{"properties":{"kind":{"not":{"const":"creature"}}}}}},"then":{"properties":{"keywords":{"items":{"enum":["flash","indestructible"]}}}}}),
            serde_json::json!({"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"land"}}}}},"then":{"properties":{"keywords":{"items":{"not":{"const":"flash"}}}}}})
        ]);

    basic_landwalk::extend_schema(&mut schema);
    source_combat_restrictions::extend_schema(&mut schema);
    activated_source_keyword::extend_schema(&mut schema);
    schema["$defs"]["StaticEffect"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({"properties":{"modify_creatures":{
            "if":{"properties":{"scope":{"const":"attacking_controller"}},"required":["scope"]},
            "then":{"properties":{"power":{"const":"0"},"toughness":{"const":"0"},"keywords":{"const":["deathtouch"]},"restrictions":{"const":[]}}}
        }}}));
    schema["$defs"]["Definition"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({
            "if":{"properties":{"characteristics":{"properties":{"snow":{"const":true}},"required":["snow"]}},"required":["characteristics"]},
            "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},"mana_abilities":{"maxItems":0}}}
        }));
    schema["$defs"]["StaticAbility"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({
            "if":{"properties":{"effect":{"enum":[StaticEffect::OpponentsCantCastDuringControllerTurn,StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer]}},"required":["effect"]},
            "then":{"properties":{"functioning_zone":{"const":"battlefield"}}}
        }));
    schema["$defs"]["Definition"].as_object_mut().unwrap()
        .get_mut("allOf").unwrap().as_array_mut().unwrap()
        .push(serde_json::json!({
            "if":{"properties":{"static_abilities":{"contains":{"properties":{"effect":{"enum":[StaticEffect::OpponentsCantCastDuringControllerTurn,StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer]}},"required":["effect"]}}},"required":["static_abilities"]},
            "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["creature","artifact","enchantment"]}}}}}
        }));
    schema["$defs"]["TriggerCondition"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({"properties":{"grouped_damage_dealt":{"properties":{
            "source":{"const":{"subject":"creature_filter","controller":"controller","types":{"predicate":"type","kind":"creature"}}},
            "kind":{"const":"combat"},"recipient":{"const":"player"},"source_power":{"const":"greater_than_base"}
        }}}}));
    schema["$defs"]["TriggeredAbility"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({
            "if":{"properties":{"condition":{"type":"object","required":["grouped_damage_dealt"]}},"required":["condition"]},
            "then":{"properties":{
                "death_filter":{"type":"null"},"intervening_if":{"type":"null"},"functioning_zone":{"const":"battlefield"},
                "program":{"properties":{"additional_sacrifice":{"type":"null"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},
                    "targets":{"maxItems":0},"modes":{"maxItems":0},
                    "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"draw_cards"},"count":{"const":"1"},"recipient":{"enum":[null,"controller"]}}}}
                }}
            }}
        }));
    // ActionRecipient is shared with controller discards, but this new opcode
    // admits only the frozen damage recipient. Keep schema and Rust aligned.
    schema["$defs"]["Instruction"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":{"properties":{"op":{"const":"sacrifice_creature"}},"required":["op"]},
            "then":{"properties":{"recipient":{"const":"event_player"}}}
        }));
    schema["$defs"]["SpellProgram"]["allOf"] = serde_json::json!([{
        "if":{"required":["modes"],"properties":{"modes":{"minItems":1}}},
        "then":{"properties":{
            "targets":{"maxItems":0},"instructions":{"maxItems":0},
            "resolution":{"anyOf":[{"enum":[SpellResolution::Instructions, SpellResolution::OneOrMoreModes]},{"type":"object","required":["choose_modes"],"properties":{"choose_modes":{"properties":{"exactly":{"minimum":2,"maximum":3}}}}},{"const":SpellResolution::ChooseRepeatedModes{exactly:5}}]},
            "modes":{"minItems":2,"items":{"properties":{"program":{"properties":{
                "modes":{"maxItems":0},"instructions":{"minItems":1},
                "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}
            }}}}}
        }}
    }]);
    // This finite domain is a root spell-pump predicate, not a general
    // creature filter that may silently reach other opcodes or contexts.
    let blocking = serde_json::json!({"targets":{"contains":{"properties":{"domain":{"const":"blocking_battlefield_creature"}},"required":["domain"]}}});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["targets"],"properties":blocking},
        "then":{"properties":{
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "targets":{"items":{"properties":{"domain":{"const":"blocking_battlefield_creature"},"filter":{"type":"null"}}}},
            "instructions":{"minItems":1,"items":{"properties":{"op":{"const":"modify_power_toughness"},"duration":{"const":"until_end_of_turn"}}}}
        }}
    }));
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "properties":{"modes":{"items":{"properties":{"program":{"properties":{
            "targets":{"items":{"properties":{"domain":{"not":{"const":"blocking_battlefield_creature"}}}}}
        }}}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","required":["targets"],"properties":blocking}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "capabilities":{"contains":{"const":Capability::BlockingCreatureTargetV1}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "properties":{
            "activated":{"items":{"properties":{"program":{"properties":{"targets":{"items":{"properties":{"domain":{"not":{"const":"blocking_battlefield_creature"}}}}}}}}}},
            "triggered":{"items":{"properties":{"program":{"properties":{"targets":{"items":{"properties":{"domain":{"not":{"const":"blocking_battlefield_creature"}}}}}}}}}}
        }
    }));
    // Keep the legacy creature-bounce opcode, fencing its new permanent domain
    // independently and recursively without prohibiting old creature programs.
    let permanent_owner_hand = serde_json::json!({
        "targets":{"contains":{"required":["domain"],"properties":{"domain":{"const":"battlefield_permanent"}}}},
        "instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"return_to_owner_hand"}}}}
    });
    schema["$defs"]["PermanentOwnerHandFreeProgram"] = serde_json::json!({
        "not":{"required":["targets","instructions"],"properties":permanent_owner_hand},
        "properties":{"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PermanentOwnerHandFreeProgram"}}}}}
    });
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["targets","instructions"],"properties":permanent_owner_hand},
        "then":{"properties":{
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"return_to_owner_hand"},"target_slot":{"const":0}}}},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "domain":{"const":"battlefield_permanent"},"filter":{"type":"object","properties":{
                    "types":{"maxItems":0},"non_types":{"enum":[[],["land"]]},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}
                }}
            }}}
        }}
    }));
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "properties":{"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PermanentOwnerHandFreeProgram"}}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","required":["targets","instructions"],"properties":permanent_owner_hand}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"capabilities":{"allOf":[
            {"contains":{"const":Capability::PermanentOwnerHandMoveV1}},
            {"contains":{"const":Capability::OwnerHandMoveV1}},
            {"contains":{"const":Capability::PermanentRemovalV1}}
        ]}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::PermanentOwnerHandMoveV1}}}},
        "then":{"anyOf":[{"required":["spell"],"properties":{
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "spell":{"type":"object","required":["targets","instructions"],"properties":permanent_owner_hand}
        }},{"$ref":"#/$defs/ActivatedPermanentOwnerHandDefinition"}]}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({"properties":{
        "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/PermanentOwnerHandFreeProgram"}}}}
    }}));
    // A creature tap may compose with the existing mandatory draw-one effect.
    // No other execution context or instruction composition is admitted.
    schema["$defs"]["TapFreeProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"tap_permanent"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/TapFreeProgram"}}}}
    }});
    let tap = serde_json::json!({"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"tap_permanent"}}}}});
    schema["$defs"]["OrdinaryTapProgram"] = serde_json::json!({"properties":{
        "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
        "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
        "instructions":{"minItems":1,"maxItems":2,"prefixItems":[
            {"properties":{"op":{"const":"tap_permanent"},"target_slot":{"const":0}}},
            {"properties":{"op":{"const":"draw_cards"},"recipient":{"type":"null"},"count":{"const":"1"}}}
        ],"items":false},
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
            "domain":{"const":"battlefield_creature"},"filter":{"type":"null"}
        }}}
    }});
    schema["$defs"]["ActivatedPermanentTapProgram"] = serde_json::json!({"required":["targets","instructions"],"properties":{
        "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
        "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
        "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"tap_permanent"},"target_slot":{"const":0}}}},
        "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
            "slot":{"const":0},"domain":{"const":"battlefield_permanent"},
            "filter":{"type":"object","required":["types"],"properties":{
                "types":{"minItems":3,"maxItems":3,"uniqueItems":true,"items":{"enum":["artifact","creature","land"]}},
                "non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}
            }}
        }}}
    }});
    schema["$defs"]["ActivatedPermanentTapAbility"] = serde_json::json!({"required":["program","cost"],"properties":{
        "functioning_zone":{"const":"battlefield"},
        "cost":{"type":"object","required":["composite"],"properties":{"composite":{"type":"object","required":["costs"],"properties":{"costs":{"type":"object","required":["mana"],"properties":{
            "tap":{"const":true},"sacrifice":{"const":false},
            "mana":{"type":"object","properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"},"colored":{"items":{"enum":["W","U","B","R","G"]}}}}
        }}}}}},
        "program":{"$ref":"#/$defs/ActivatedPermanentTapProgram"}
    }});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["instructions"],"properties":tap},
        "then":{"anyOf":[{"$ref":"#/$defs/OrdinaryTapProgram"},{"$ref":"#/$defs/ActivatedPermanentTapProgram"}]}
    }));
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "properties":{"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/TapFreeProgram"}}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","required":["instructions"],"properties":tap}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "spell":{"$ref":"#/$defs/OrdinaryTapProgram"},"capabilities":{"contains":{"const":Capability::TapPermanentV1}}}}
    }));
    schema["$defs"]["ActivatedAbility"].as_object_mut().unwrap().entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["program"],"properties":{"program":{"required":["instructions"],"properties":tap}}},
        "then":{"$ref":"#/$defs/ActivatedPermanentTapAbility"}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":{"required":["program"],"properties":{"program":{"required":["instructions"],"properties":tap}}}}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"artifact"}}},"capabilities":{"allOf":[
            {"contains":{"const":Capability::ActivatedTapPermanentV1}}, {"contains":{"const":Capability::TapPermanentV1}},
            {"contains":{"const":Capability::ActivatedAbilityV1}}, {"contains":{"const":Capability::CompositeActivationCostV1}},
            {"contains":{"const":Capability::PermanentRemovalV1}}, {"contains":{"const":Capability::CastArtifactV1}}
        ]}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::ActivatedTapPermanentV1}}}},
        "then":{"required":["activated"],"properties":{"characteristics":{"properties":{"kind":{"const":"artifact"}}},
            "activated":{"contains":{"$ref":"#/$defs/ActivatedPermanentTapAbility"}}
        }}
    }));
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"properties":{
            "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/TapFreeProgram"}}}}
        }}));
    // Only source-tap destruction acquires this contextual capability. Other
    // ordinary and sacrifice-cost destruction programs retain their contracts.
    schema["$defs"]["DestroyPermanentFreeProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"destroy_permanent"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/DestroyPermanentFreeProgram"}}}}
    }});
    schema["$defs"]["TapSourceDestroyAbility"] = serde_json::json!({"required":["cost","program"],"properties":{
        "cost":{"const":"tap_source"},"functioning_zone":{"const":"battlefield"},
        "program":{"required":["targets","instructions"],"properties":{
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{
                "op":{"const":"destroy_permanent"},"target_slot":{"const":0},
                "optional":{"const":false},"regeneration_forbidden":{"const":false}
            }}},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "slot":{"const":0},"domain":{"const":"battlefield_permanent"},
                "filter":{"type":"object","required":["types","tapped"],"properties":{
                    "types":{"const":["creature"]},"tapped":{"const":true},
                    "non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false}
                }}
            }}}
        }}
    }});
    let tap_destroy = serde_json::json!({"required":["cost","program"],"properties":{
        "cost":{"const":"tap_source"},"program":{"not":{"$ref":"#/$defs/DestroyPermanentFreeProgram"}}
    }});
    schema["$defs"]["ActivatedAbility"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":tap_destroy,"then":{"$ref":"#/$defs/TapSourceDestroyAbility"}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":tap_destroy}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},"capabilities":{"allOf":[
            {"contains":{"const":Capability::TapSourceDestroyV1}}, {"contains":{"const":Capability::TappedPermanentTargetV1}},
            {"contains":{"const":Capability::PermanentRemovalV1}}, {"contains":{"const":Capability::TapSourceCostV1}},
            {"contains":{"const":Capability::ActivatedAbilityV1}}, {"contains":{"const":Capability::BaseCreatureV1}},
            {"contains":{"const":Capability::CastCreatureV1}}, {"contains":{"const":Capability::NormalLayoutV1}}
        ]}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::TapSourceDestroyV1}}}},
        "then":{"required":["activated"],"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},
            "activated":{"contains":{"$ref":"#/$defs/TapSourceDestroyAbility"}}
        }}
    }));
    schema["$defs"]["CounterStackObjectFreeProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"counter_stack_object"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/CounterStackObjectFreeProgram"}}}}
    }});
    // The composite cost is the same already-reviewed external representation
    // used by activated permanent tapping: {composite:{costs:{...}}}.
    let fixed_mana_source_tap_cost =
        schema["$defs"]["ActivatedPermanentTapAbility"]["properties"]["cost"].clone();
    schema["$defs"]["ActivatedSpellCounterAbility"] = serde_json::json!({"required":["cost","program"],"properties":{
        "cost":fixed_mana_source_tap_cost,"functioning_zone":{"const":"battlefield"},
        "program":{"required":["targets","instructions"],"properties":{
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"counter_stack_object"},"target_slot":{"const":0}}}},
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
                "slot":{"const":0},"domain":{"const":"stack_spell"},"filter":{"type":"null"}
            }}}
        }}
    }});
    let activated_counter = serde_json::json!({"required":["program"],"properties":{
        "program":{"not":{"$ref":"#/$defs/CounterStackObjectFreeProgram"}}
    }});
    schema["$defs"]["ActivatedAbility"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":activated_counter,"then":{"$ref":"#/$defs/ActivatedSpellCounterAbility"}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":activated_counter}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},"capabilities":{"allOf":[
            {"contains":{"const":Capability::ActivatedSpellCounterV1}}, {"contains":{"const":Capability::CounterStackObjectV1}},
            {"contains":{"const":Capability::StackSpellTargetV1}}, {"contains":{"const":Capability::CompositeActivationCostV1}},
            {"contains":{"const":Capability::ActivatedAbilityV1}}, {"contains":{"const":Capability::BaseCreatureV1}},
            {"contains":{"const":Capability::CastCreatureV1}}, {"contains":{"const":Capability::NormalLayoutV1}}
        ]}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::ActivatedSpellCounterV1}}}},
        "then":{"required":["activated"],"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},
            "activated":{"contains":{"$ref":"#/$defs/ActivatedSpellCounterAbility"}}
        }}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({"properties":{
        "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/CounterStackObjectFreeProgram"}}}}
    }}));
    schema["$defs"]["ActivatedPermanentOwnerHandAbility"] = serde_json::json!({"required":["cost","program"],"properties":{
        "cost":fixed_mana_source_tap_cost,"functioning_zone":{"const":"battlefield"},
        "program":{"required":["targets","instructions"],"properties":{
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"return_to_owner_hand"},"target_slot":{"const":0}}}},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "slot":{"const":0},"domain":{"const":"battlefield_permanent"},
                "filter":{"type":"object","properties":{"types":{"maxItems":0},"non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}}}
            }}}
        }}
    }});
    schema["$defs"]["ActivatedPermanentOwnerHandDefinition"] = serde_json::json!({"required":["activated"],"properties":{
        "characteristics":{"properties":{"kind":{"const":"creature"}}},
        "activated":{"contains":{"$ref":"#/$defs/ActivatedPermanentOwnerHandAbility"}}
    }});
    let activated_return = serde_json::json!({"required":["program"],"properties":{
        "program":{"not":{"$ref":"#/$defs/PermanentOwnerHandFreeProgram"}}
    }});
    schema["$defs"]["ActivatedAbility"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":activated_return,"then":{"$ref":"#/$defs/ActivatedPermanentOwnerHandAbility"}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":activated_return}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},"capabilities":{"allOf":[
            {"contains":{"const":Capability::ActivatedPermanentOwnerHandV1}}, {"contains":{"const":Capability::PermanentOwnerHandMoveV1}},
            {"contains":{"const":Capability::OwnerHandMoveV1}}, {"contains":{"const":Capability::PermanentRemovalV1}},
            {"contains":{"const":Capability::CompositeActivationCostV1}}, {"contains":{"const":Capability::ActivatedAbilityV1}},
            {"contains":{"const":Capability::BaseCreatureV1}}, {"contains":{"const":Capability::CastCreatureV1}},
            {"contains":{"const":Capability::NormalLayoutV1}}
        ]}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::ActivatedPermanentOwnerHandV1}}}},
        "then":{"$ref":"#/$defs/ActivatedPermanentOwnerHandDefinition"}
    }));
    // Untap has two disjoint complete contexts; modes and triggers stay excluded.
    schema["$defs"]["UntapFreeProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"untap_permanent"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/UntapFreeProgram"}}}}
    }});
    let untap = serde_json::json!({"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"untap_permanent"}}}}});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["instructions"],"properties":untap},
        "then":{"properties":{
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"untap_permanent"},"target_slot":{"const":0}}}},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "domain":{"const":"battlefield_permanent"},
                "slot":{"const":0},"selection_id":{"minLength":1},"chooser":{"const":"controller"},"stage":{"const":"announcement"},
                "filter":{"type":"object","properties":{"types":{"enum":[[],["creature"]]},"non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}}}
            }}}
        }}
    }));
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "properties":{"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/UntapFreeProgram"}}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","required":["instructions"],"properties":untap}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"spell":{"properties":{"targets":{"items":{"properties":{"filter":{"properties":{"types":{"maxItems":0}}}}}}}},"capabilities":{"contains":{"const":Capability::UntapPermanentV1}}}}
    }));
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"properties":{
            "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/UntapFreeProgram"}}}}
        }}));
    schema["$defs"]["TapSourceUntapAbility"] = serde_json::json!({"required":["cost","program"],"properties":{
        "cost":{"const":"tap_source"},"functioning_zone":{"const":"battlefield"},
        "program":{"required":["instructions","targets"],"properties":{
            "timing":{"const":"instant"},"modes":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"untap_permanent"}}}},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "filter":{"type":"object","required":["types"],"properties":{"types":{"const":["creature"]}}}
            }}}
        }}
    }});
    let activated_untap = serde_json::json!({"required":["program"],"properties":{"program":{"not":{"$ref":"#/$defs/UntapFreeProgram"}}}});
    schema["$defs"]["ActivatedAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":activated_untap,"then":{"$ref":"#/$defs/TapSourceUntapAbility"}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":activated_untap}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},"capabilities":{"allOf":[
            {"contains":{"const":Capability::TapSourceUntapCreatureV1}}, {"contains":{"const":Capability::UntapPermanentV1}},
            {"contains":{"const":Capability::PermanentRemovalV1}}, {"contains":{"const":Capability::TapSourceCostV1}},
            {"contains":{"const":Capability::ActivatedAbilityV1}}, {"contains":{"const":Capability::BaseCreatureV1}},
            {"contains":{"const":Capability::CastCreatureV1}}, {"contains":{"const":Capability::NormalLayoutV1}}
        ]}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::TapSourceUntapCreatureV1}}}},
        "then":{"required":["activated"],"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},
            "activated":{"contains":{"$ref":"#/$defs/TapSourceUntapAbility"}}
        }}
    }));
    // Match requirements(): legendary creatures carry Commander identity even
    // when this particular configuration does not designate them commanders.
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["characteristics"],"properties":{"characteristics":{
            "required":["kind","legendary"],"properties":{"kind":{"const":"creature"},"legendary":{"const":true}}
        }}},
        "then":{"properties":{"capabilities":{"contains":{"const":Capability::CommanderIdentityV1}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::CommanderIdentityV1}}}},
        "then":{"properties":{"characteristics":{"required":["kind","legendary"],
            "properties":{"kind":{"const":"creature"},"legendary":{"const":true}}
        }}}
    }));
    schema["$defs"]["PureKeywordGrantInstructions"] = serde_json::json!({
        "minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"grant_keywords"},"target_slot":{"const":0},"modifier":{"type":"null"},"keywords":{"enum":[["indestructible"],["flying"],["deathtouch"]]},"duration":{"const":"until_end_of_turn"}}}
    });
    schema["$defs"]["CompoundIndestructibleInstructions"] = serde_json::json!({
        "minItems":1,"maxItems":1,"items":{"required":["modifier"],"properties":{
            "op":{"const":"grant_keywords"},"target_slot":{"const":0},
            "modifier":{"$ref":"#/$defs/CreatureStatModifier"},
            "keywords":{"const":["indestructible"]},"duration":{"const":"until_end_of_turn"},"source_block":{"const":0}
        }}
    });
    schema["$defs"]["TemporaryCreatureKeywords"] = serde_json::json!({
        "type":"array","minItems":1,"uniqueItems":true,
        "items":{"enum":[KeywordAbility::Flying,KeywordAbility::DoubleStrike,KeywordAbility::Vigilance,KeywordAbility::Reach,KeywordAbility::Deathtouch,KeywordAbility::Indestructible,KeywordAbility::Lifelink,KeywordAbility::Hexproof,KeywordAbility::Haste,KeywordAbility::Trample,KeywordAbility::FirstStrike]}
    });
    schema["$defs"]["TemporaryCreatureKeywordInstructions"] = serde_json::json!({
        "minItems":1,"maxItems":1,"items":{"properties":{
            "op":{"const":"grant_keywords"},"target_slot":{"const":0},
            "modifier":{"anyOf":[{"type":"null"},{"$ref":"#/$defs/CreatureStatModifier"}]},
            "keywords":{"$ref":"#/$defs/TemporaryCreatureKeywords"},"duration":{"const":"until_end_of_turn"}
        },"if":{"required":["modifier"],"properties":{"modifier":{"type":"object"}}},
        "then":{"properties":{"source_block":{"const":0}}}}
    });
    // Fence this opcode recursively; it never becomes an activation, trigger,
    // mode, mixed program or general keyword-grant facility.
    schema["$defs"]["KeywordGrantFreeProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"grant_keywords"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/KeywordGrantFreeProgram"}}}}
    }});
    schema["$defs"]["TemporaryPermanentKeywordProgram"] = serde_json::json!({
        "required":["targets","instructions"],"properties":{
            "optional_additional_costs":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "slot":{"const":0},"selection_id":{"minLength":1},"chooser":{"const":"controller"},
                "stage":{"const":"announcement"},"domain":{"const":"controlled_battlefield_permanent"},
                "filter":{"type":"object","required":["types","non_types","non_colors","opponent"],"properties":{
                    "types":{"const":[]},"non_types":{"const":[]},"non_colors":{"const":[]},"opponent":{"const":false},"tapped":{"const":false}
                }}
            }}},
            "instructions":{"minItems":1,"maxItems":2,"prefixItems":[
                {"properties":{"op":{"const":"grant_keywords"},"target_slot":{"const":0},"modifier":{"type":"null"},
                    "keywords":{"$ref":"#/$defs/TemporaryCreatureKeywords"},"duration":{"const":"until_end_of_turn"},"source_block":{"const":0}}},
                {"properties":{"op":{"const":"gain_life"},"recipient":{"const":"controller"},
                    "amount":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}}
            ]}
        }
    });
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["instructions"],"properties":{"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"grant_keywords"}}}}}},
        "then":{"anyOf":[{"$ref":"#/$defs/TemporaryPermanentKeywordProgram"},{"properties":{
            "optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{"slot":{"const":0},"selection_id":{"minLength":1},"chooser":{"const":"controller"},"stage":{"const":"announcement"},"domain":{"enum":["battlefield_creature","controlled_battlefield_creature"]},"filter":{"type":"null"}}}},
            "instructions":{"$ref":"#/$defs/TemporaryCreatureKeywordInstructions"}
        }}]}
    }));
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["instructions","targets"],"properties":{
            "instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"modify_power_toughness"}}}},
            "targets":{"contains":{"required":["domain"],"properties":{"domain":{"const":"controlled_battlefield_creature"}}}}
        }},
        "then":{"properties":{"instructions":{"$ref":"#/$defs/TemporaryCreatureKeywordInstructions"}}}
    }));
    let plain_grant_target = serde_json::json!({"properties":{"spell":{"properties":{
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":"battlefield_creature"}}}}
    }}}});
    for (keyword, capability, ordinary) in [
        (
            "deathtouch",
            Capability::TemporaryDeathtouchSpellV1,
            Capability::DeathtouchV1,
        ),
        (
            "indestructible",
            Capability::TemporaryIndestructibleSpellV1,
            Capability::IndestructibleV1,
        ),
        (
            "flying",
            Capability::TemporaryFlyingSpellV1,
            Capability::FlyingV1,
        ),
    ] {
        schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
            "if":{"allOf":[plain_grant_target,temporary_keyword(keyword),{"properties":{"spell":{"properties":{"instructions":{"$ref":"#/$defs/PureKeywordGrantInstructions"}}}}}]},"then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"capabilities":{"allOf":[{"contains":{"const":capability}},{"contains":{"const":ordinary}}]}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":capability}}}}}
        }));
    }
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","required":["instructions"],"properties":{"targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":"battlefield_creature"}}}},"instructions":{"$ref":"#/$defs/CompoundIndestructibleInstructions"}}}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"capabilities":{"allOf":[
            {"contains":{"const":Capability::CompoundIndestructibleSpellV1}},
            {"contains":{"const":Capability::CreatureTargetV1}},
            {"contains":{"const":Capability::TemporaryPowerToughnessV1}},
            {"contains":{"const":Capability::IndestructibleV1}}
        ]}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CompoundIndestructibleSpellV1}}}}}
    }));
    let family_grant = serde_json::json!({"required":["spell"],"properties":{"spell":{
        "type":"object","required":["instructions","targets"],"properties":{
            "instructions":{"$ref":"#/$defs/TemporaryCreatureKeywordInstructions"},
            "targets":{"items":{"properties":{"domain":{"enum":["battlefield_creature","controlled_battlefield_creature"]}}}}
        },"not":{"properties":{
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":"battlefield_creature"}}}},
            "instructions":{"anyOf":[{"$ref":"#/$defs/PureKeywordGrantInstructions"},{"$ref":"#/$defs/CompoundIndestructibleInstructions"}]}
        }}
    }}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":family_grant,
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "capabilities":{"contains":{"const":Capability::TemporaryCreatureKeywordProgramV1}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::TemporaryCreatureKeywordProgramV1}}}}}
    }));
    let any_grant = serde_json::json!({"required":["spell"],"properties":{"spell":{
        "type":"object","required":["instructions"],"properties":{"instructions":{
            "contains":{"required":["op"],"properties":{"op":{"const":"grant_keywords"}}}
        }}
    }}});
    for (keyword, capability) in [
        ("flying", Capability::FlyingV1),
        ("reach", Capability::ReachV1),
        ("vigilance", Capability::VigilanceV1),
        ("double strike", Capability::DoubleStrikeV1),
        ("deathtouch", Capability::DeathtouchV1),
        ("indestructible", Capability::IndestructibleV1),
        ("lifelink", Capability::LifelinkV1),
        ("haste", Capability::HasteV1),
        ("trample", Capability::TrampleV1),
        ("first strike", Capability::FirstStrikeV1),
    ] {
        schema["$defs"]["Definition"]["allOf"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "if":any_grant,"then":{
                    "if":temporary_keyword(keyword),
                    "then":{"properties":{"capabilities":{"contains":{"const":capability}}}},
                    "else":{"properties":{"capabilities":{"not":{"contains":{"const":capability}}}}}
                }
            }));
    }
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":any_grant,"then":{"allOf":[
            {"properties":{"capabilities":{"contains":{"const":Capability::NormalLayoutV1}}}},
            {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
             "then":{"properties":{"capabilities":{"allOf":[{"contains":{"const":Capability::CastInstantV1}},{"not":{"contains":{"const":Capability::CastSorceryV1}}}]}}},
             "else":{"properties":{"capabilities":{"allOf":[{"contains":{"const":Capability::CastSorceryV1}},{"not":{"contains":{"const":Capability::CastInstantV1}}}]}}}}
        ]}
    }));
    // Other creature-keyword mechanics cannot be spare capabilities on an
    // ordinary grant spell; Flash retains its independent intrinsic meaning.
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":any_grant,"then":{"properties":{"capabilities":{"items":{"not":{"enum":[
                Capability::DefenderV1,
                Capability::MenaceV1
            ]}}}}}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":any_grant,"then":{"if":{"properties":{"spell":{"properties":{"instructions":{"contains":{"required":["modifier"],"properties":{"modifier":{"type":"object"}}}}}}}},
        "then":{"properties":{"capabilities":{"contains":{"const":Capability::TemporaryPowerToughnessV1}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::TemporaryPowerToughnessV1}}}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":any_grant,"then":{"allOf":[
            {"if":plain_grant_target,
             "then":{"properties":{"capabilities":{"contains":{"const":Capability::CreatureTargetV1}}}},
             "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CreatureTargetV1}}}}}},
            {"if":{"anyOf":[{"properties":{"spell":{"properties":{"targets":{"contains":{"properties":{"domain":{"const":"controlled_battlefield_creature"}}}}}}}},temporary_keyword("hexproof")]},
             "then":{"properties":{"capabilities":{"contains":{"const":Capability::AttachmentsV1}}}},
             "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::AttachmentsV1}}}}}}
        ]}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({"properties":{
        "activated":{"items":{"anyOf":[{"properties":{"program":{"$ref":"#/$defs/KeywordGrantFreeProgram"}}},{"$ref":"#/$defs/SacrificeSourceCreatureSetKeywordAbility"}]}},
        "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/KeywordGrantFreeProgram"}}}},
        "spell":{"anyOf":[{"$ref":"#/$defs/CommanderModalKeywordProgram"},{"properties":{"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/KeywordGrantFreeProgram"}}}}}}]}
    }}));
    // Optional fields deserialize null as None. Only a non-null cost/restriction
    // selects this family; a selected counter cost must then be a positive quantity.
    let counter_mana = serde_json::json!({"anyOf":[
        {"required":["restriction"],"properties":{"restriction":{"type":"string"}}},
        {"properties":{"costs":{"required":["remove_source_plus_one_counters"],"properties":{"remove_source_plus_one_counters":{"type":"string"}}}}}
    ]});
    schema["$defs"]["ManaAbility"]["allOf"] = serde_json::json!([{
        "if":counter_mana,
        "then":{"required":["restriction"],"properties":{
            "restriction":{"const":"once_each_turn"},
            "costs":{"required":["remove_source_plus_one_counters"],"properties":{
                "remove_source_plus_one_counters":{"type":"string","pattern":"^[1-9][0-9]*$"},"tap":{"const":false},"sacrifice":{"const":false},"mana":{"type":"null"}}},
            "production":{"properties":{"mana":{"const":"fixed"},"symbols":{"minItems":1}}}
        }},"else":{"properties":{"costs":{"properties":{"tap":{"const":true},"sacrifice":{"const":false}}}}}
    }]);
    schema["$defs"]["CastColorFreeProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"put_source_plus_one_counters_for_cast_colors"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/CastColorFreeProgram"}}}}
    }});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({"properties": {"spell": {"$ref": "#/$defs/CastColorFreeProgram"}, "activated": {"items": {"properties": {"program": {"$ref": "#/$defs/CastColorFreeProgram"}, "cost": {"properties": {"composite": {"properties": {"costs": {"not": {"required": ["remove_source_plus_one_counters"],"properties":{"remove_source_plus_one_counters":{"type":"string"}}}}}}}}}}}}}));
    let cast_colors = serde_json::json!({"required":["program"],"properties":{"program":{"required":["instructions"],"properties":{"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"put_source_plus_one_counters_for_cast_colors"}}}}}}}});
    let cast_color_condition = TriggerCondition::SpellCast {
        caster: CasterRelation::Controller,
        spell: TypePredicate::Any {},
    };
    schema["$defs"]["TriggeredAbility"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":cast_colors,"then":{"properties":{
            "condition":{"const":cast_color_condition},"intervening_if":{"type":"null"},
            "program":{"properties":{"targets":{"maxItems":0},"modes":{"maxItems":0},"instructions":{"minItems":1,"maxItems":1}}}
        }}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["mana_abilities"],"properties":{"mana_abilities":{"contains":counter_mana}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},"capabilities":{"allOf":[{"contains":{"const":Capability::CounterRemovalManaV1}},{"contains":{"const":Capability::PermanentManaV1}}]}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CounterRemovalManaV1}}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["triggered"],"properties":{"triggered":{"contains":cast_colors}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},"capabilities":{"allOf":[{"contains":{"const":Capability::CastSpellColorCountersV1}},{"contains":{"const":Capability::PlusOneCountersV1}},{"contains":{"const":Capability::CastTriggerV1}}]}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CastSpellColorCountersV1}}}}}
    }));
    cascade_schema(&mut schema);

    let permanent_target = serde_json::json!({"required":["targets"],"properties":{"targets":{"contains":{
        "required":["domain"],"properties":{"domain":{"const":"controlled_battlefield_permanent"}}
    }}}});
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":permanent_target,"then":{"$ref":"#/$defs/TemporaryPermanentKeywordProgram"}
        }));
    let permanent_grant = serde_json::json!({"required":["spell"],"properties":{"spell":{
        "type":"object","$ref":"#/$defs/TemporaryPermanentKeywordProgram"
    }}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":permanent_grant,"then":{"properties":{
            "keywords":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
            "static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},
            "intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},
                "legendary":{"const":false},"basic":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"},"additional_types":{"maxItems":0}}},
            "capabilities":{"allOf":[{"contains":{"const":Capability::TemporaryPermanentKeywordProgramV1}},
                {"contains":{"const":Capability::PermanentRemovalV1}}]}
        }},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::TemporaryPermanentKeywordProgramV1}}}}}
    }));
    // For this ordinary family, capability closure is exactly derived from the
    // kind, target primitive, ordered keyword vector and presence of the tail.
    let mut permanent_caps = vec![
        Capability::NormalLayoutV1,
        Capability::CastInstantV1,
        Capability::CastSorceryV1,
        Capability::TemporaryPermanentKeywordProgramV1,
        Capability::PermanentRemovalV1,
        Capability::LifeGainV1,
        Capability::ControllerScryV1,
    ];
    for keyword in [
        KeywordAbility::Flying,
        KeywordAbility::DoubleStrike,
        KeywordAbility::Vigilance,
        KeywordAbility::Reach,
        KeywordAbility::Deathtouch,
        KeywordAbility::Indestructible,
        KeywordAbility::Lifelink,
        KeywordAbility::Hexproof,
        KeywordAbility::Haste,
        KeywordAbility::Trample,
        KeywordAbility::FirstStrike,
    ] {
        permanent_caps.push(temporary_keyword_primitive_capability(keyword).unwrap());
    }
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":permanent_grant,"then":{"allOf":[
            {"properties":{"capabilities":{"items":{"enum":permanent_caps},"contains":{"const":Capability::NormalLayoutV1}}}},
            {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
             "then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},
             "else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}},
            {"if":{"properties":{"spell":{"properties":{"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"gain_life"}}}}}}}},
             "then":{"properties":{"capabilities":{"contains":{"const":Capability::LifeGainV1}}}},
             "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::LifeGainV1}}}}}}
        ]}
    }));
    // CR 119.9: only the exact new complete grant family admits a zero tail.
    // Generic gain/loss programs retain their strictly positive amount contract.
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "if":{"not":{"$ref":"#/$defs/TemporaryPermanentKeywordProgram"}},
            "then":{"properties":{"instructions":{"items":{
                "if":{"required":["op"],"properties":{"op":{"enum":["gain_life","lose_life"]}}},
                "then":{"properties":{"amount":{"type":"string","pattern":"^[1-9][0-9]*$"}}}
            }}}}
        }));
    // The set family has its own non-targeted shape; targeted any_grant above
    // deliberately continues to match only GrantKeywords.
    let set_grant_op =
        serde_json::json!({"required":["op"],"properties":{"op":{"const":"grant_keywords_set"}}});
    let set_grant_program = serde_json::json!({"required":["instructions"],"properties":{"instructions":{"contains":set_grant_op}}});
    schema["$defs"]["CreatureStatModifier"] = serde_json::json!({
        "type":"object","additionalProperties":false,"required":["power","toughness"],
        "properties":{"power":{"$ref":"#/$defs/ExactQuantity"},"toughness":{"$ref":"#/$defs/ExactQuantity"}}
    });
    let mut set_shapes = Vec::new();
    for set in [
        ObjectSet::ControllerCreatures,
        ObjectSet::AttackingCreatures,
        ObjectSet::ControllerPermanents,
    ] {
        let modifier = if set == ObjectSet::ControllerPermanents {
            serde_json::json!({"type":"null"})
        } else {
            serde_json::json!({"anyOf":[{"type":"null"},{"$ref":"#/$defs/CreatureStatModifier"}]})
        };
        let grant = serde_json::json!({"properties":{
            "op":{"const":"grant_keywords_set"},"set":{"const":set},
            "modifier":modifier,
            "keywords":{"$ref":"#/$defs/TemporaryCreatureKeywords"},
            "duration":{"const":"until_end_of_turn"},"source_block":{"const":0}
        }});
        set_shapes.push(serde_json::json!({"minItems":1,"maxItems":1,"items":grant}));
    }
    schema["$defs"]["TemporarySetKeywordInstructions"] = serde_json::json!({"anyOf":set_shapes});
    let set_family_program = serde_json::json!({"properties":{
        "optional_additional_costs":{"maxItems":0},
        "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},"targets":{"maxItems":0},
        "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
        "instructions":{"$ref":"#/$defs/TemporarySetKeywordInstructions"}
    }});
    let controller_set = serde_json::json!({"anyOf":[
        {"required":["set"],"properties":{"set":{"enum":[ObjectSet::ControllerCreatures,ObjectSet::ControllerPermanents]}}},
        {"required":["sets"],"properties":{"sets":{"contains":{"enum":[ObjectSet::ControllerCreatures,ObjectSet::ControllerPermanents]}}}}
    ]});
    let controller_pump_program = controller_creature_pump::extend_schema(&mut schema);
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"anyOf":[set_grant_program,{"required":["instructions"],"properties":{"instructions":{"contains":controller_set}}}]},
        "then":{"anyOf":[set_family_program,{"$ref":"#/$defs/CommanderModalKeywordChild"},controller_pump_program]}
    }));
    // Recursion fences every mode, trigger and activation as well as top-level spells.
    schema["$defs"]["KeywordGrantFreeProgram"]["properties"]["instructions"]["items"]["not"]["properties"]
        ["op"] = serde_json::json!({"enum":["grant_keywords","grant_keywords_set"]});
    let any_set_grant = serde_json::json!({"required":["spell"],"properties":{"spell":{"type":"object","allOf":[set_grant_program]}}});
    let creature_cap = Capability::TemporaryCreatureSetKeywordProgramV1;
    let permanent_cap = Capability::TemporaryPermanentSetKeywordProgramV1;
    let permanent_set = serde_json::json!({"properties":{"spell":{"properties":{"instructions":{"contains":{
        "required":["set"],"properties":{"set":{"const":ObjectSet::ControllerPermanents}}
    }}}}}});
    let mut allowed_caps = vec![
        Capability::NormalLayoutV1,
        Capability::CastInstantV1,
        Capability::CastSorceryV1,
        Capability::BoardWipesV1,
        creature_cap,
        permanent_cap,
    ];
    let mut exact_caps = Vec::new();
    for keyword in [
        KeywordAbility::Flying,
        KeywordAbility::DoubleStrike,
        KeywordAbility::Vigilance,
        KeywordAbility::Reach,
        KeywordAbility::Deathtouch,
        KeywordAbility::Indestructible,
        KeywordAbility::Lifelink,
        KeywordAbility::Hexproof,
        KeywordAbility::Haste,
        KeywordAbility::Trample,
        KeywordAbility::FirstStrike,
    ] {
        let cap = temporary_keyword_primitive_capability(keyword).unwrap();
        allowed_caps.push(cap);
        let contains = temporary_set_keyword(keyword);
        exact_caps.push(serde_json::json!({"if":contains,
            "then":{"properties":{"capabilities":{"contains":{"const":cap}}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
        }));
    }
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":any_set_grant,"then":{"allOf":[
            {"properties":{
                "spell":set_family_program,
                "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},
                "mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},
                "intrinsic_zone":{"type":"null"},
                "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},
                    "legendary":{"const":false},"basic":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"},"additional_types":{"maxItems":0}}},
                "capabilities":{"items":{"enum":allowed_caps},"allOf":[
                    {"contains":{"const":Capability::BoardWipesV1}},
                    {"contains":{"const":Capability::NormalLayoutV1}}
                ]}
            }},
            {"allOf":exact_caps},
            {"if":permanent_set,
             "then":{"properties":{"capabilities":{"allOf":[{"contains":{"const":permanent_cap}},{"not":{"contains":{"const":creature_cap}}}]}}},
             "else":{"properties":{"capabilities":{"allOf":[{"contains":{"const":creature_cap}},{"not":{"contains":{"const":permanent_cap}}}]}}}},
            {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
             "then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"allOf":[{"contains":{"const":Capability::CastInstantV1}},{"not":{"contains":{"const":Capability::CastSorceryV1}}}]}}},
             "else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"allOf":[{"contains":{"const":Capability::CastSorceryV1}},{"not":{"contains":{"const":Capability::CastInstantV1}}}]}}}}
        ]},"else":{"if":{"$ref":"#/$defs/HasActivatedSetKeyword"},
            "then":{"$ref":"#/$defs/SacrificeSourceCreatureSetKeywordDefinition"},
            "else":{"if":{"not":{"required":["spell"],"properties":{"spell":{"$ref":"#/$defs/CommanderModalKeywordProgram"}}}},"then":{"properties":{"capabilities":{"items":{"not":{"enum":[creature_cap,permanent_cap]}}}}}}}
    }));
    sacrifice_set::schema(&mut schema);
    // Keep the Aura-control operation and capability paired with its complete
    // executable shape. This addition does not narrow other attachment families.
    // Derive the scoped keyword vocabulary through the shared static-layer
    // validator, rather than maintaining another attachment keyword allowlist.
    let keyword_schema = serde_json::to_value(schemars::schema_for!(KeywordAbility)).unwrap();
    let aura_keywords: Vec<KeywordAbility> =
        serde_json::from_value::<Vec<KeywordAbility>>(keyword_schema["enum"].clone())
            .unwrap()
            .into_iter()
            .filter(|keyword| {
                static_creature_modification_valid(
                    CardKind::Enchantment,
                    false,
                    true,
                    &StaticEffect::ModifyCreatures {
                        scope: CreatureScope::Enchanted,
                        power: Quantity::zero(),
                        toughness: Quantity::zero(),
                        keywords: vec![*keyword],
                        restrictions: vec![],
                    },
                )
            })
            .collect();
    schema["$defs"]["AuraControlModifier"] = serde_json::json!({
        "required":["functioning_zone","effect"],"properties":{
            "functioning_zone":{"const":"battlefield"},
            "effect":{"type":"object","required":["modify_creatures"],"properties":{
                "modify_creatures":{
                    "required":["scope","power","toughness","keywords","restrictions"],
                    "properties":{
                        "scope":{"const":"enchanted"},
                        "power":{"pattern":"^(0|[1-9][0-9]*)$"},
                        "toughness":{"pattern":"^(0|[1-9][0-9]*)$"},
                        "keywords":{"uniqueItems":true,"items":{"enum":aura_keywords}},
                        "restrictions":{"uniqueItems":true}
                    },
                    "anyOf":[
                        {"properties":{"power":{"not":{"const":"0"}}}},
                        {"properties":{"toughness":{"not":{"const":"0"}}}},
                        {"properties":{"keywords":{"minItems":1}}},
                        {"properties":{"restrictions":{"minItems":1}}}
                    ]
                }
            }}
        }
    });
    let aura_control_static = serde_json::json!({"const":{
        "functioning_zone":"battlefield","effect":"control_enchanted_permanent"
    }});
    schema["$defs"]["AuraControlDefinition"] = serde_json::json!({
        "required":["spell","static_abilities","capabilities"],"properties":{
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{
                "kind":{"const":"enchantment"},"additional_types":{"maxItems":0},
                "subtypes":{"const":["Aura"]},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},
                "power":{"type":"null"},"toughness":{"type":"null"},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
            }},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "triggered":{"maxItems":0},"keywords":{"maxItems":0},
            "static_abilities":{"minItems":1,"maxItems":2,
                "contains":aura_control_static,"minContains":1,"maxContains":1,
                "items":{"anyOf":[aura_control_static,{"$ref":"#/$defs/AuraControlModifier"}]}
            },
            "spell":{"type":"object","properties":{
                "timing":{"const":"main_phase"},"resolution":{"const":"permanent"},
                "functioning_zone":{"const":"stack"},"additional_sacrifice":{"type":"null"},
                "optional_additional_costs":{"maxItems":0},
                "instructions":{"maxItems":0},"modes":{"maxItems":0},
                "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
                    "slot":{"const":0},"selection_id":{"minLength":1},
                    "chooser":{"const":"controller"},"stage":{"const":"announcement"},
                    "domain":{"const":"battlefield_creature"},"filter":{"type":"null"}
                }}}
            }},
            "capabilities":{"minItems":5,"maxItems":5,"uniqueItems":true,"items":{"enum":[
                Capability::NormalLayoutV1,Capability::CastEnchantmentV1,
                Capability::CreatureTargetV1,Capability::AttachmentsV1,Capability::AuraControlV1
            ]}}
        }
    });
    let creature_control = schema["$defs"]["AuraControlDefinition"].clone();
    let mut control_shapes = vec![creature_control.clone()];
    for types in [vec![CardKind::Artifact], vec![CardKind::Land], vec![]] {
        let mut shape = creature_control.clone();
        shape["properties"]["static_abilities"] = serde_json::json!({
            "minItems":1,"maxItems":1,"items":aura_control_static
        });
        let target = &mut shape["properties"]["spell"]["properties"]["targets"]["items"];
        target["required"] = serde_json::json!(["filter"]);
        target["properties"]["domain"] = serde_json::json!({"const":"battlefield_permanent"});
        target["properties"]["filter"] = serde_json::json!({"type":"object",
        "required":["types","non_types","non_colors","opponent"],"properties":{
            "types":{"const":types},"non_types":{"maxItems":0},
            "non_colors":{"maxItems":0},"opponent":{"const":false},
            "tapped":{"const":false}
        }});
        shape["properties"]["capabilities"]["items"] = serde_json::json!({"enum":[
            Capability::NormalLayoutV1,Capability::CastEnchantmentV1,
            Capability::PermanentRemovalV1,Capability::AttachmentsV1,Capability::AuraControlV1
        ]});
        control_shapes.push(shape);
    }
    schema["$defs"]["AuraControlDefinition"] = serde_json::json!({"oneOf":control_shapes});
    let aura_control = serde_json::json!({"required":["static_abilities"],"properties":{
        "static_abilities":{"contains":{"required":["effect"],"properties":{
            "effect":{"const":StaticEffect::ControlEnchantedPermanent}
        }}}
    }});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .extend([
            serde_json::json!({"if":aura_control,"then":{"$ref":"#/$defs/AuraControlDefinition"}}),
            serde_json::json!({"if":{"required":["capabilities"],"properties":{
            "capabilities":{"contains":{"const":Capability::AuraControlV1}}
        }},"then":{"$ref":"#/$defs/AuraControlDefinition"}}),
        ]);
    static_controller::schema(&mut schema);
    optional_costs::schema(&mut schema);
    counted_costs::schema(&mut schema);
    fixed_artifact_sacrifice_schema(&mut schema);
    creature_planeswalker_return::schema(&mut schema);
    modal_targets::schema(&mut schema);
    repeated_target_modes::schema(&mut schema);
    graveyard_shuffle::schema(&mut schema);
    typed_land_search::schema(&mut schema);
    controller_library::extend_schema(&mut schema);
    opponent_mana::extend_schema(&mut schema);
    distinct_permanent_colors::extend_schema(&mut schema);
    pain_mana::extend_schema(&mut schema);
    listed_mana_choice::extend_schema(&mut schema);
    source_tap_damage::extend_schema(&mut schema);
    conditional_lands::schema(&mut schema);
    other_land_entry::extend_schema(&mut schema);
    cascade_land_entry::extend_schema(&mut schema);
    entry_copy::extend_schema(&mut schema);
    counter_spell_controller_mill::extend_schema(&mut schema);
    fixed_mill::extend_schema(&mut schema);
    announced_x_draw::extend_schema(&mut schema);
    targeted_counted_draw::extend_schema(&mut schema);
    targeted_discard::extend_schema(&mut schema);
    set_exile::extend_schema(&mut schema);
    each_player_draw::extend_schema(&mut schema);
    group_discard::extend_schema(&mut schema);
    opponent_life_drain::extend_schema(&mut schema);
    targeted_game_loss::extend_schema(&mut schema);
    controlled_phasing::schema(&mut schema);
    counter_unless_generic::extend_schema(&mut schema);
    controlled_creature_spell_copy::extend_schema(&mut schema);
    creative_technique::extend_schema(&mut schema);
    self_graveyard_shuffle::extend_schema(&mut schema);
    library_selection::extend_schema(&mut schema);
    library_order::extend_schema(&mut schema);
    controller_counted_draw::extend_schema(&mut schema);
    draw_discard::extend_schema(&mut schema);
    hand_return::extend_schema(&mut schema);
    graveyard_battlefield_return::extend_schema(&mut schema);
    nonland_exile_life::extend_schema(&mut schema);
    self_spell_protection::extend_schema(&mut schema);
    unspent_mana::extend_schema(&mut schema);
    granted_land_mana::extend_schema(&mut schema);
    keyword_removal_wipe::extend_schema(&mut schema);
    opponent_nonland_destroy::extend_schema(&mut schema);
    source_dies_nonland_destroy::extend_schema(&mut schema);
    cycling::schema(&mut schema);
    successor_tuck::extend_schema(&mut schema);
    fixed_creature_activation::extend_schema(&mut schema);
    cast_entry::extend_schema(&mut schema);
    let color_trigger = serde_json::json!({"required":["condition"],"properties":{"condition":{
        "type":"object","required":["spell_cast"],"properties":{"spell_cast":{"properties":{
            "spell":{"properties":{"predicate":{"const":"spell_color"}}}
        }}}
    }}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["triggered"],"properties":{"triggered":{"contains":color_trigger}}},
        "then":{"properties":{"capabilities":{"contains":{"const":Capability::CastColorPredicateV1}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CastColorPredicateV1}}}}}
    }));
    cast_origin::extend_schema(&mut schema);
    cast_payer_draw::extend_schema(&mut schema);
    activated_artifact_draw::extend_schema(&mut schema);
    unlimited_hand_size::extend_schema(&mut schema);
    hand_size_adjustments::extend_schema(&mut schema);
    flash_end_step_draw::extend_schema(&mut schema);
    paid_counted_draw::extend_schema(&mut schema);
    power_entry_draw::extend_schema(&mut schema);
    greatest_power::extend_schema(&mut schema);
    commander_modal_keyword::extend_schema(&mut schema);
    divided_plus_one_counters::extend_schema(&mut schema);
    power_filtered_removal::extend_schema(&mut schema);
    creature_entry_return::extend_schema(&mut schema);
    entry_graveyard_exile::extend_schema(&mut schema);
    land_entry_gain_life::extend_schema(&mut schema);
    land_sacrifice_draw::extend_schema(&mut schema);
    saga::extend_schema(&mut schema);
    hand_cast::extend_schema(&mut schema);
    cast_mana_value_sweep::extend_schema(&mut schema);
    hand_cast_cascade::extend_schema(&mut schema);
    linked_hideaway::extend_schema(&mut schema);
    own_graveyard_card_return::extend_schema(&mut schema);
    for (effect, cap, pattern) in [
        (
            StaticEffect::GrantControllerLandsAllBasicTypes,
            Capability::ControllerLandBasicTypesV1,
            r#""oracle_text"\s*:\s*"Lands you control are (?:all basic land types|every basic land type) in addition to their other types\.""#,
        ),
        (
            StaticEffect::ControllerNonlandsAllColors,
            Capability::ControllerNonlandColorsV1,
            r#""oracle_text"\s*:\s*"Nonland permanents you control are all colors\.""#,
        ),
    ] {
        let grant =
            serde_json::json!({"required":["effect"],"properties":{"effect":{"const":effect}}});
        schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
            "if":{"anyOf":[{"required":["static_abilities"],"properties":{"static_abilities":{"contains":grant}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":pattern}}}}}]},
        "then":{"required":["static_abilities"],"properties":{"characteristics":{"properties":{"kind":{"const":"enchantment"},"basic":{"const":false},"snow":{"const":false},"legendary":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0}}},
            "static_abilities":{"const":[StaticAbility{functioning_zone:FunctioningZone::Battlefield,effect:effect}]},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "capabilities":{"contains":{"const":cap}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
    }));
        opening_placement::permit_composition(&mut schema);
    }
    bestow::extend_schema(&mut schema);
    colored_library_search::extend_schema(&mut schema);
    opening_placement::schema(&mut schema);
    opening_placement::permit_composition(&mut schema);
    color_hybrid_cost::extend_schema(&mut schema);
    opening_placement::permit_composition(&mut schema);
    opening_placement::require_composition(&mut schema);
    library_top_mana::extend_schema(&mut schema);
    schema
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
mod tapped_target_tests;

#[cfg(test)]
mod untap_tests;

#[cfg(test)]
mod tap_tests;

#[cfg(test)]
mod activated_permanent_tap_tests;

#[cfg(test)]
mod tap_source_destroy_tests;
#[cfg(test)]
mod tap_source_untap_tests;

#[cfg(test)]
mod activated_spell_counter_tests;

#[cfg(test)]
mod activated_permanent_return_tests;

#[cfg(test)]
mod basic_land_split_tests;

#[cfg(test)]
mod permanent_owner_hand_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn build_identity_covers_all_engine_source_but_not_fixture_or_host_data() {
        for path in [
            "Cargo.lock",
            "Cargo.toml",
            "rust-toolchain.toml",
            "packages/game-engine/crates/sim/src/new_module.rs",
            "packages/game-engine/crates/core/build.rs",
            "packages/game-engine/crates/compiler/Cargo.toml",
        ] {
            assert!(engine_build_input(path), "{path}");
        }
        for path in [
            "packages/game-engine/tests/fixtures/catalogs/base.json",
            "packages/game-server/src/main.rs",
            "packages/game-engine/crates/core/generated/export.rs",
            "packages/game-engine/crates/core/README.md",
        ] {
            assert!(!engine_build_input(path), "{path}");
        }
    }

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
    fn every_capability_has_a_complete_current_witness() {
        use Capability::*;
        let expected = [
            PlayerGraveyardShuffleV1,
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
            EnterAsCopyOfPermanentEnteredThisTurnV1,
            SourceCombatRestrictionsV1,
            StaticSpellCascadeGrantV1,
            CascadeLandEntryV1,
            HandCastCascadeGrantV1,
            HideawayV1,
            LinkedExilePlayV1,
            CastHistoryV1,
            CastManaValueSweepV1,
            RandomBottomV1,
            SagaChaptersV1,
            DelayedControllerCreatureEntryV1,
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
            PlayerOrPlaneswalkerTargetV1,
            RemoveIndestructibleV1,
            AnyPlayerTargetV1,
            NoncombatDamageV1,
            LifeGainV1,
            LifeLossV1,
            OpponentLifeDrainV1,
            TriggeredAbilityV1,
            SelfEntersBattlefieldV1,
            OtherCreatureEntersBattlefieldV1,
            PowerFilteredCreatureEntryV1,
            CreatureEntryReturnV1,
            EntryGraveyardExileV1,
            LandEntryGainLifeV1,
            LandSacrificeDrawV1,
            CreatureDiesV1,
            CastEnchantmentV1,
            AuraControlV1,
            BestowV1,
            TemporaryPowerToughnessV1,
            CombatDamagePreventionV1,
            DamageBudgetPreventionV1,
            StaticDamageReplacementV1,
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
            DividedPlusOneCountersV1,
            DestroyCreatureV1,
            GraveyardRecursionV1,
            GraveyardBattlefieldReturnV1,
            OwnGraveyardCardReturnV1,
            BasicLandSearchV1,
            BasicLandSplitSearchV1,
            ColoredLibrarySearchV1,
            TypedLandSearchV1,
            CreatureAnthemV1,
            TargetPlayerDrawV1,
            TargetedCountedDrawV1,
            TargetPlayerDiscardV1,
            RandomPlayerDiscardV1,
            GroupHandDiscardV1,
            TargetedGameLossV1,
            CastArtifactV1,
            PermanentManaV1,
            OpponentLandManaV1,
            DistinctPermanentColorsV1,
            CompositeActivationCostV1,
            AttachmentsV1,
            PermanentRemovalV1,
            SacrificeV1,
            FixedArtifactSacrificeCostV1,
            BoardWipesV1,
            SelfSpellProtectionV1,
            AnnouncedXDrawV1,
            EachPlayerDrawV1,
            AnnouncedXEffectsV1,
            FlashV1,
            EntersTappedV1,
            ConditionalBasicLandEntryV1,
            ConditionalOtherLandEntryV1,
            OptionalLifeLandEntryV1,
            TypedNonbasicIntrinsicManaV1,
            ControllerLandBasicTypesV1,
            UnspentManaConversionV1,
            ControllerLandGrantedManaV1,
            OpeningPlacementV1,
            ControllerNonlandColorsV1,
            StepBeginningTriggerV1,
            EventPlayerRecipientV1,
            SourceStatusConditionV1,
            CastEntryConditionV1,
            UntapControllerLandsV1,
            CastTriggerV1,
            CastOriginTriggerV1,
            CastColorPredicateV1,
            CastSpellColorCountersV1,
            CounterRemovalManaV1,
            ListedManaChoiceV1,
            DisjointPainManaV1,
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
            ControlledCreatureDamageReplacementV1,
        ];
        // Enumerate the public vocabulary, so a new capability cannot silently
        // escape this contract. Check full program-bearing definitions rather
        // than capability-only records that fail independent shape checks.
        let schema = serde_json::to_value(schemars::schema_for!(Capability)).unwrap();
        let vocabulary: BTreeSet<Capability> =
            serde_json::from_value::<Vec<Capability>>(schema["enum"].clone())
                .unwrap()
                .into_iter()
                .collect();
        let covered: BTreeSet<Capability> = expected.into_iter().collect();
        assert_eq!(covered, vocabulary);
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalogs");
        let mut definitions = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                path.extension()
                    .is_some_and(|e| e == "json")
                    .then_some(path)
            })
            .flat_map(|path| {
                let manifest: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                let catalog: Catalog = serde_json::from_slice(
                    &std::fs::read(root.join(manifest["expected_ir"].as_str().unwrap())).unwrap(),
                )
                .unwrap();
                catalog.definitions
            })
            .collect::<Vec<_>>();
        let mut creator = definitions
            .iter()
            .find(|d| {
                d.spell.as_ref().is_some_and(|p| {
                    matches!(
                        p.instructions.first(),
                        Some(Instruction::CreateCreatureTokens { .. })
                    )
                })
            })
            .unwrap()
            .clone();
        creator.source_labels = None;
        if let Instruction::CreateCreatureTokens { token, .. } =
            &mut creator.spell.as_mut().unwrap().instructions[0]
        {
            token.colors.clear();
            token.keywords.clear();
            token.additional_types = vec![CardKind::Artifact];
            token.subtypes = vec!["Latticekin".into()];
            token.power = 3.into();
            token.toughness = 7.into();
        }
        creator.capabilities = creator.requirements();
        let mut menu = definitions
            .iter()
            .find(|d| {
                d.characteristics.kind == CardKind::Sorcery
                    && !d.x_mana_cost
                    && d.keywords.is_empty()
                    && d.triggered.is_empty()
                    && d.static_abilities.is_empty()
                    && d.characteristics.subtypes.is_empty()
            })
            .unwrap()
            .clone();
        let record: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/synthetic-open-mode-catalog.json"
        ))
        .unwrap();
        let source = &record["source"];
        menu.source_labels = None;
        assert_eq!(source["mana_cost"], "{0}");
        menu.characteristics.mana_cost = ManaCost {
            generic: 0.into(),
            colored: vec![],
        };
        menu.name = source["name"].as_str().unwrap().into();
        menu.oracle_id = source["oracle_id"].as_str().unwrap().into();
        menu.provenance.printing_id = source["id"].as_str().unwrap().into();
        menu.provenance.source_json = canonical(source).unwrap();
        menu.provenance.source_sha256 = sha256(menu.provenance.source_json.as_bytes());
        menu.provenance.abilities_json =
            canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
        menu.provenance.ast_sha256 =
            sha256(format!("[{}]", menu.provenance.abilities_json).as_bytes());
        let timing = if source["type_line"] == "Sorcery" {
            menu.characteristics.kind = CardKind::Sorcery;
            SpellTiming::MainPhase
        } else {
            menu.characteristics.kind = CardKind::Instant;
            SpellTiming::Instant
        };
        menu.spell = Some(
            set_exile_modes(source["oracle_text"].as_str().unwrap(), timing)
                .unwrap()
                .2,
        );
        menu.capabilities = menu.requirements();
        definitions.push(menu);
        definitions.push(creator);
        // Synthetic complete typed control; exact frozen source is compiled
        // independently in compiler::ramos_m4e_tests.
        let mut counted = definitions
            .iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap()
            .clone();
        let mut multi = counted.clone();
        multi.source_labels = None;
        multi.activated.push(ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost: ActivationCost::SacrificePermanent {
                count: 2,
                filter: SacrificeFilter {
                    types: vec![CardKind::Artifact],
                    subtypes: vec![],
                },
                mana: None,
            },
            program: SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                instructions: vec![Instruction::DrawCards {
                    count: 1.into(),
                    recipient: None,
                    source_block: 0,
                }],
                targets: vec![],
                modes: vec![],
                additional_sacrifice: None,
                optional_additional_costs: vec![],
            },
        });
        multi.capabilities = multi.requirements();
        definitions.push(multi.clone());
        let text = "Sacrifice three artifacts: Choose one —\n• ~ deals 2 damage to target player or planeswalker.\n• Draw a card.";
        let bound = BoundArtifactMode::parse(text, 0, 0).unwrap();
        multi.name = "Synthetic Boundary Reader".into();
        multi.activated = vec![bound.ability];
        multi.provenance.abilities_json = canonical(&[bound.ast]).unwrap();
        multi.provenance.ast_sha256 =
            sha256(format!("[{}]", multi.provenance.abilities_json).as_bytes());
        multi.provenance.source_json=canonical(&serde_json::json!({"name":multi.name,"oracle_id":multi.oracle_id,"id":multi.provenance.printing_id,
            "oracle_text":text.replace('~',&multi.name)})).unwrap();
        multi.provenance.source_sha256 = sha256(multi.provenance.source_json.as_bytes());
        multi.capabilities = multi.requirements();
        definitions.push(multi);
        let mut permanent_grant = counted.clone();
        permanent_grant.source_labels = None;
        permanent_grant.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantOtherControlledPermanentsIndestructible,
        }];
        permanent_grant.capabilities = permanent_grant.requirements();
        definitions.push(permanent_grant);
        let mut distinct_colors = counted.clone();
        distinct_colors.source_labels = None;
        distinct_colors.characteristics.toughness = Some(Quantity::zero());
        distinct_colors.keywords = vec![KeywordAbility::Vigilance];
        let scope = PermanentColorScope::Controller;
        distinct_colors.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifySourceByDistinctPermanentColors {
                scope,
                power_per_color: 2.into(),
                toughness_per_color: 3.into(),
            },
        }];
        distinct_colors.mana_abilities = vec![ManaAbility {
            self_damage: None,
            restriction: None,
            costs: PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap: true,
                mana: None,
                sacrifice: false,
            },
            production: ManaProduction::EachPermanentColor { scope },
            source_block: 2,
        }];
        distinct_colors.capabilities = distinct_colors.requirements();
        definitions.push(distinct_colors);
        definitions.push(successor_tuck::witness(counted.clone()));
        let mut origin_draw = counted.clone();
        origin_draw.source_labels = None;
        origin_draw.triggered = vec![TriggeredAbility {
            death_filter: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::SpellCastWithOrigin {
                caster: CasterRelation::Controller,
                spell: TypePredicate::Any {},
                origin: CastOriginPredicate::Outside {
                    zone: CastOriginZone::ControllerHand,
                },
            },
            intervening_if: None,
            program: SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 0,
                }],
                modes: vec![],
            },
        }];
        origin_draw.capabilities = origin_draw.requirements();
        let mut color_cast = origin_draw.clone();
        color_cast.triggered[0].condition = TriggerCondition::SpellCast {
            caster: CasterRelation::Controller,
            spell: TypePredicate::SpellColor { color: Color::U },
        };
        color_cast.capabilities = color_cast.requirements();
        let mut cast_entry = color_cast.clone();
        cast_entry.triggered[0].condition = TriggerCondition::SelfEntersBattlefield;
        cast_entry.triggered[0].intervening_if = Some(InterveningCondition::SourceCastByController);
        let synthetic: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/synthetic-cast-entry.json"
        ))
        .unwrap();
        cast_entry.provenance.printing_id = synthetic["source"]["id"].as_str().unwrap().into();
        cast_entry.name = synthetic["source"]["name"].as_str().unwrap().into();
        cast_entry.oracle_id = synthetic["oracleIdentity"].as_str().unwrap().into();
        cast_entry.provenance.source_json = canonical(&synthetic["source"]).unwrap();
        cast_entry.provenance.abilities_json =
            canonical(&synthetic["faces"][0]["result"]["abilities"]).unwrap();
        cast_entry.provenance.source_sha256 = sha256(cast_entry.provenance.source_json.as_bytes());
        cast_entry.provenance.ast_sha256 =
            sha256(format!("[{}]", cast_entry.provenance.abilities_json).as_bytes());
        cast_entry.triggered[0].program.instructions =
            vec![Instruction::OptionalDrawCards { source_block: 0 }];
        cast_entry.capabilities = cast_entry.requirements();
        let mut land_entry = cast_entry.clone();
        let land: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/synthetic-cast-land-untap.json"
        ))
        .unwrap();
        land_entry.provenance.printing_id = land["source"]["id"].as_str().unwrap().into();
        land_entry.name = land["source"]["name"].as_str().unwrap().into();
        land_entry.oracle_id = land["oracleIdentity"].as_str().unwrap().into();
        land_entry.provenance.source_json = canonical(&land["source"]).unwrap();
        land_entry.provenance.abilities_json =
            canonical(&land["faces"][0]["result"]["abilities"]).unwrap();
        land_entry.provenance.source_sha256 = sha256(land_entry.provenance.source_json.as_bytes());
        land_entry.provenance.ast_sha256 =
            sha256(format!("[{}]", land_entry.provenance.abilities_json).as_bytes());
        land_entry.triggered[0].program.instructions =
            vec![Instruction::UntapControllerLands { source_block: 0 }];
        land_entry.capabilities = land_entry.requirements();
        definitions.push(land_entry);
        definitions.push(cast_entry);
        definitions.push(color_cast);
        definitions.push(origin_draw);
        counted.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Stack,
            effect: StaticEffect::ReduceSelfGenericCost {
                per_object: 1.into(),
                count: BattlefieldCount {
                    kind: CardKind::Artifact,
                    controller: CountController::Caster,
                },
            },
        }];
        counted.capabilities = counted.requirements();
        definitions.push(counted);
        let mut static_grant = definitions
            .iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap()
            .clone();
        static_grant.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantSpellCascade {
                eligibility: SpellCascadeEligibility::FirstCastEachTurn {},
                program: canonical_cascade_program(0),
            },
        }];
        static_grant.capabilities = static_grant.requirements();
        definitions.push(static_grant);
        let mut optional = definitions
            .iter()
            .find(|d| d.name == "Naturalize")
            .unwrap()
            .clone();
        let identity = format!(
            "card:{}/faces/0/abilities/0/keywords/0#keyword",
            optional.oracle_id
        );
        let p = optional.spell.as_mut().unwrap();
        p.optional_additional_costs = vec![OptionalAdditionalManaCost {
            identity: identity.clone(),
            source_block: 0,
            mana: ManaCost {
                generic: 2.into(),
                colored: vec![Color::U],
            },
        }];
        p.instructions.push(Instruction::DrawCardsIfAdditionalCost {
            cost_identity: identity,
            count: 2.into(),
            source_block: 1,
        });
        optional.capabilities = optional.requirements();
        definitions.push(optional);

        definitions.push(creature_set_keyword_tests::grant(
            ObjectSet::ControllerCreatures,
            &[KeywordAbility::Haste],
            false,
        ));
        definitions.push(creature_set_keyword_tests::grant(
            ObjectSet::ControllerPermanents,
            &[KeywordAbility::Hexproof, KeywordAbility::Indestructible],
            false,
        ));
        let mut blocking = definitions
            .iter()
            .find(|d| d.name == "Giant Growth")
            .unwrap()
            .clone();
        blocking.spell.as_mut().unwrap().targets[0].domain =
            TargetDomain::BlockingBattlefieldCreature;
        blocking.capabilities = blocking.requirements();
        definitions.push(blocking);
        // Complete synthetic IR witness for the independently source-qualified
        // tapped predicate; compiler tests exercise the exact new card records.
        let mut tapped = definitions
            .iter()
            .find(|d| d.name == "Naturalize")
            .unwrap()
            .clone();
        tapped.spell.as_mut().unwrap().targets[0]
            .filter
            .as_mut()
            .unwrap()
            .tapped = true;
        tapped.capabilities = tapped.requirements();
        definitions.push(tapped);
        // Complete typed untap witness; the compiler separately checks the
        // retained source and its complete unqualified target selection.
        // Complete typed Aura-control witness; compiler tests independently
        // qualify exact frozen sources without a profile admission change.
        definitions.push(aura_control_tests::aura());
        definitions.push(aura_control_tests::typed_aura(CardKind::Artifact));
        definitions.push(aura_control_tests::typed_aura(CardKind::Land));
        definitions.push(aura_control_tests::general_aura());
        definitions.push(indestructible_tests::intrinsic());
        definitions.push(temporary_indestructible_tests::grant());
        definitions.push(temporary_flying_tests::grant());
        definitions.push(temporary_deathtouch_tests::grant());
        definitions.push(compound_indestructible_tests::compound());
        definitions.push(temporary_creature_keyword_tests::grant(
            &[KeywordAbility::Lifelink],
            false,
        ));
        definitions.push(temporary_permanent_keyword_tests::grant(true));
        definitions.push(sacrifice_set_tests::grant(&[
            KeywordAbility::Indestructible,
        ]));
        definitions.push(controller_library::witness());
        definitions.push(cascade_land_entry::witness());
        definitions.push(hand_cast_cascade::witness());
        definitions.push(linked_hideaway::witness());
        definitions.push(bestow::witness());
        definitions.push(controller_library::surveil_witness());
        definitions.push(counter_spell_controller_mill::witness());
        definitions.push(fixed_mill::witness());
        definitions.push(graveyard_battlefield_return::witness(None));
        definitions.push(announced_x_draw::witness());
        definitions.push(each_player_draw::witness());
        definitions.push(announced_x_draw::effects_witness());
        definitions.push(power_entry_draw::witness());
        definitions.push(greatest_power::witness());
        definitions.push(power_filtered_removal::witness());
        definitions.push(cast_payer_draw::witness());
        definitions.push(divided_plus_one_counters::witness());
        definitions.push(commander_modal_keyword::witness());
        definitions.push(creature_entry_return::witness());
        definitions.push(entry_graveyard_exile::witness(CardKind::Land));
        definitions.push(land_entry_gain_life::witness());
        definitions.push(entry_copy::registry_witness());
        definitions.push(land_sacrifice_draw::witness());
        definitions.push(saga::witness());
        definitions.push(hand_cast::witness());
        definitions.push(cast_mana_value_sweep::tests::witness());
        definitions.push(own_graveyard_card_return::witness(CardKind::Creature));
        definitions.push(targeted_counted_draw::witness());
        definitions.push(targeted_discard::witness());
        definitions.push(targeted_discard::random_witness());
        definitions.push(group_discard::witness());
        definitions.push(opponent_life_drain::witness());
        definitions.push(
            Catalog::read(include_str!(
                "../../../tests/fixtures/synthetic-controlled-phasing-ir.json"
            ))
            .unwrap()
            .definitions
            .remove(0),
        );
        definitions.push(
            Catalog::read(include_str!(
                "../../../tests/fixtures/synthetic-controller-life-freeze-ir.json"
            ))
            .unwrap()
            .definitions
            .remove(0),
        );
        definitions.push(
            Catalog::read(include_str!(
                "../../../tests/fixtures/synthetic-controller-protection-ir.json"
            ))
            .unwrap()
            .definitions
            .remove(0),
        );
        definitions.push(
            Catalog::read(include_str!(
                "../../../tests/fixtures/synthetic-self-exile-ir.json"
            ))
            .unwrap()
            .definitions
            .remove(0),
        );
        definitions.push(targeted_game_loss::witness());
        definitions.push(counter_unless_generic::witness());
        definitions.push(graveyard_shuffle::witness());
        definitions.push(modal_targets::witness());
        definitions.push(creature_planeswalker_return::witness());
        definitions.push(controlled_creature_spell_copy::witness());
        definitions.push(creative_technique::witness());
        definitions.push(library_selection::witness());
        definitions.push(library_order::witness());
        definitions.push(hand_return::witness());
        definitions.push(draw_discard::witness());
        definitions.push(controller_counted_draw::witness());
        let mut draw_land_allowance = controller_counted_draw::witness();
        draw_land_allowance
            .spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::GrantLandPlaysThisTurn {
                count: "999999999999999999999999".parse().unwrap(),
                source_block: 0,
            });
        draw_land_allowance.capabilities = draw_land_allowance.requirements();
        definitions.push(draw_land_allowance);
        definitions.push(untap_tests::untap());
        definitions.push(tap_tests::tap());
        definitions.push(activated_permanent_tap_tests::activated_tap());
        definitions.push(tap_source_destroy_tests::tap_source_destroy());
        definitions.push(tap_source_untap_tests::tap_source_untap());
        definitions.push(activated_spell_counter_tests::activated_counter());
        definitions.push(activated_permanent_return_tests::activated_return());
        definitions.push(activated_artifact_draw::witness());
        definitions.extend(paid_counted_draw::witnesses());
        definitions.push(permanent_owner_hand_tests::permanent_owner_hand());
        definitions.push(basic_land_split_tests::split_search());
        definitions.push(typed_land_search_tests::typed_search());
        definitions.push(conditional_lands::tests::land());
        definitions.push(other_land_entry::witness());
        definitions.push(conditional_lands::tests::optional_land());
        definitions.push(cycling::witness());
        definitions.push(opponent_mana::witness());
        definitions.push(pain_mana::witness());
        definitions.push(listed_mana_choice::witness());
        definitions.push(source_tap_damage::witness());
        definitions.push(ramos_m1_tests::ramos());
        definitions.push(counter_placement_tests::provider(
            CounterPlacementModification::Add { amount: 7.into() },
        ));
        // Synthetic contract-shape witness only. Complete D2c-e source lowering
        // is checked in compiler/core tests, without fabricated expected IR.
        let mut damage = definitions
            .iter()
            .find(|d| {
                d.triggered.iter().any(|a| {
                    matches!(
                        a.condition,
                        TriggerCondition::CreatureAttacked {
                            attacker: AttackSubject::Source {}
                        }
                    )
                })
            })
            .unwrap()
            .clone();
        damage.triggered[0].condition = TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        };
        damage.capabilities = damage.requirements();
        definitions.push(damage.clone());
        // Complete synthetic contract-shape witness, not source admission.
        // Compiler/core M3 tests separately consume both exact frozen records.
        let mut cascade = damage.clone();
        cascade.triggered = vec![TriggeredAbility {
            functioning_zone: FunctioningZone::Stack,
            condition: TriggerCondition::SelfSpellCast,
            intervening_if: None,
            death_filter: None,
            program: SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                modes: vec![],
                instructions: vec![Instruction::Cascade {
                    threshold: CascadeThreshold::SourceSpellManaValue,
                    source_block: 0,
                }],
            },
        }];
        cascade.capabilities = cascade.requirements();
        definitions.push(cascade);
        let mut optional = damage.clone();
        optional.triggered[0].program.targets.clear();
        optional.triggered[0].program.instructions =
            vec![Instruction::OptionalDrawCards { source_block: 0 }];
        optional.capabilities = optional.requirements();
        definitions.push(optional);
        let mut discard = damage.clone();
        discard.triggered[0].program.targets.clear();
        discard.triggered[0].program.instructions = vec![Instruction::DiscardCards {
            recipient: Some(ActionRecipient::EventPlayer),
            count: Quantity::from(1),
            source_block: 0,
        }];
        discard.capabilities = discard.requirements();
        definitions.push(discard);
        let mut sacrifice = damage.clone();
        sacrifice.triggered[0].program.targets.clear();
        sacrifice.triggered[0].program.instructions = vec![Instruction::SacrificeCreature {
            recipient: ActionRecipient::EventPlayer,
            source_block: 0,
        }];
        sacrifice.capabilities = sacrifice.requirements();
        definitions.push(sacrifice);
        let mut filtered = damage.clone();
        filtered.triggered[0].condition = TriggerCondition::DamageDealt {
            source: DamageTriggerSource::CreatureFilter {
                controller: DamageSourceRelation::Controller,
                types: TypePredicate::Type {
                    kind: CardKind::Creature,
                },
            },
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        };
        filtered.capabilities = filtered.requirements();
        definitions.push(filtered);
        let mut controller_discard = damage.clone();
        controller_discard.triggered[0].program.targets.clear();
        controller_discard.triggered[0].program.instructions = vec![Instruction::DiscardCards {
            recipient: None,
            count: Quantity::from(1),
            source_block: 0,
        }];
        controller_discard.capabilities = controller_discard.requirements();
        definitions.push(controller_discard);
        let mut quantity_draw = damage.clone();
        quantity_draw.triggered[0].program.targets.clear();
        quantity_draw.triggered[0].program.instructions =
            vec![Instruction::DrawDamageAmount { source_block: 0 }];
        quantity_draw.capabilities = quantity_draw.requirements();
        definitions.push(quantity_draw);
        let mut snow = damage.clone();
        snow.characteristics.snow = true;
        snow.capabilities = snow.requirements();
        definitions.push(snow);
        let mut attacking = damage.clone();
        attacking.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::AttackingController,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![KeywordAbility::Deathtouch],
                restrictions: vec![],
            },
        }];
        attacking.capabilities = attacking.requirements();
        definitions.push(attacking);
        let mut prohibit = damage.clone();
        prohibit.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::OpponentsCantCastDuringControllerTurn,
        }];
        prohibit.capabilities = prohibit.requirements();
        definitions.push(prohibit);
        definitions.push(unlimited_hand_size::witness());
        definitions.push(hand_size_adjustments::reduced_witness());
        definitions.push(hand_size_adjustments::increased_witness());
        let mut controlled_replacement = damage.clone();
        controlled_replacement.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer,
        }];
        controlled_replacement.capabilities = controlled_replacement.requirements();
        definitions.push(controlled_replacement);
        let mut grouped = damage.clone();
        grouped.triggered[0].condition = TriggerCondition::GroupedDamageDealt {
            source: DamageTriggerSource::CreatureFilter {
                controller: DamageSourceRelation::Controller,
                types: TypePredicate::Type {
                    kind: CardKind::Creature,
                },
            },
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
            source_power: DamageSourcePowerPredicate::GreaterThanBase,
        };
        grouped.triggered[0].program.targets.clear();
        grouped.triggered[0].program.instructions = vec![Instruction::DrawCards {
            recipient: None,
            count: Quantity::from(1),
            source_block: 0,
        }];
        grouped.capabilities = grouped.requirements();
        definitions.push(grouped);
        // Kind and recipient each derive the expanded damage capability.
        for (kind, recipient) in [
            (TriggerDamageKind::Any, DamageTriggerRecipient::Player),
            (
                TriggerDamageKind::Combat,
                DamageTriggerRecipient::OpponentPlayer,
            ),
            (
                TriggerDamageKind::Any,
                DamageTriggerRecipient::OpponentPlayer,
            ),
        ] {
            let mut expanded = damage.clone();
            expanded.triggered[0].condition = TriggerCondition::DamageDealt {
                source: DamageTriggerSource::Source {},
                kind,
                recipient,
            };
            expanded.capabilities = expanded.requirements();
            assert!(expanded.capabilities.contains(&SourceDamageTriggerV2));
            assert!(!expanded.capabilities.contains(&DamageTriggerV1));
            definitions.push(expanded);
        }
        definitions.push(typed_fetchland_tests::definition());
        let protected_wipe = definitions
            .iter()
            .find(|d| {
                d.static_abilities
                    .iter()
                    .any(|a| a.effect == StaticEffect::CantBeCountered)
            })
            .unwrap()
            .clone();
        definitions.push(self_spell_protection::witness(protected_wipe));
        let grant = definitions
            .iter()
            .find(|d| {
                d.characteristics.kind == CardKind::Enchantment
                    && d.characteristics.subtypes.is_empty()
                    && d.activated.is_empty()
                    && d.triggered.is_empty()
                    && d.keywords.is_empty()
            })
            .unwrap()
            .clone();
        for (effect, record, ast) in [
            (
                StaticEffect::GrantControllerLandsAllBasicTypes,
                include_str!("../../../tests/fixtures/synthetic-controller-land-types.json"),
                controller_land_type_ast(),
            ),
            (
                StaticEffect::ControllerNonlandsAllColors,
                include_str!("../../../tests/fixtures/synthetic-nonland-colors.json"),
                controller_nonland_colors_ast(),
            ),
        ] {
            let mut grant = grant.clone();
            grant.static_abilities = vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect,
            }];
            let record: serde_json::Value = strict_read(record).unwrap();
            let raw = &record["source"];
            grant.name = raw["name"].as_str().unwrap().into();
            grant.oracle_id = raw["oracle_id"].as_str().unwrap().into();
            grant.characteristics.mana_cost = ManaCost {
                generic: 2.into(),
                colored: vec![Color::W],
            };
            grant.characteristics.colors = vec![Color::W];
            grant.characteristics.color_identity = vec![Color::W];
            grant.source_labels = None;
            grant.provenance.printing_id = raw["id"].as_str().unwrap().into();
            grant.provenance.source_json = canonical(raw).unwrap();
            grant.provenance.source_sha256 = sha256(grant.provenance.source_json.as_bytes());
            grant.provenance.abilities_json = canonical(&ast).unwrap();
            grant.provenance.ast_sha256 =
                sha256(format!("[{}]", grant.provenance.abilities_json).as_bytes());
            grant.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
            grant.capabilities = grant.requirements();
            definitions.push(grant);
        }

        let artifact = definitions
            .iter()
            .find(|d| d.name == "Sol Ring")
            .unwrap()
            .clone();
        definitions.push(granted_land_mana::witness(artifact));
        let source = definitions
            .iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap()
            .clone();
        definitions.push(unspent_mana::witness(source));
        definitions.push(library_top_mana::frozen_definition().unwrap());
        let sweep = definitions
            .iter()
            .find(|d| d.name == "Planar Cleansing")
            .unwrap()
            .clone();
        definitions.push(keyword_removal_wipe::witness(sweep));
        let mut colored = definitions
            .iter()
            .find(|d| {
                d.characteristics.kind == CardKind::Sorcery
                    && d.activated.is_empty()
                    && !d.x_mana_cost
            })
            .unwrap()
            .clone();
        let record: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/synthetic-colored-library.json"
        ))
        .unwrap();
        let source = &record["source"];
        colored.name = source["name"].as_str().unwrap().into();
        colored.oracle_id = source["oracle_id"].as_str().unwrap().into();
        colored.provenance.printing_id = source["id"].as_str().unwrap().into();
        colored.provenance.source_json = canonical(source).unwrap();
        colored.provenance.source_sha256 = sha256(colored.provenance.source_json.as_bytes());
        colored.provenance.abilities_json =
            canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
        colored.provenance.ast_sha256 =
            sha256(format!("[{}]", colored.provenance.abilities_json).as_bytes());
        colored.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
        colored.characteristics.mana_cost = ManaCost {
            generic: Quantity::zero(),
            colored: vec![Color::G],
        };
        colored.characteristics.colors = vec![Color::G];
        colored.characteristics.color_identity = vec![Color::G];
        colored.spell = Some(colored_library_program(vec![Color::R, Color::G]));
        colored.source_labels = None;
        colored.characteristics.subtypes.clear();
        colored.keywords.clear();
        colored.static_abilities.clear();
        colored.triggered.clear();
        colored.capabilities = colored.requirements();
        definitions.push(colored);
        let mut pregame = definitions
            .iter()
            .find(|d| {
                d.characteristics.kind == CardKind::Enchantment
                    && d.characteristics.subtypes.is_empty()
                    && d.activated.is_empty()
                    && d.triggered.is_empty()
                    && d.keywords.is_empty()
            })
            .unwrap()
            .clone();
        let record: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/synthetic-opening-placement.json"
        ))
        .unwrap();
        let raw = &record["source"];
        pregame.name = raw["name"].as_str().unwrap().into();
        pregame.oracle_id = raw["oracle_id"].as_str().unwrap().into();
        pregame.provenance.printing_id = raw["id"].as_str().unwrap().into();
        pregame.provenance.source_json = canonical(raw).unwrap();
        pregame.provenance.source_sha256 = sha256(pregame.provenance.source_json.as_bytes());
        pregame.provenance.abilities_json =
            canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
        pregame.provenance.ast_sha256 =
            sha256(format!("[{}]", pregame.provenance.abilities_json).as_bytes());
        pregame.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
        pregame.characteristics.mana_cost = ManaCost {
            generic: 2.into(),
            colored: vec![Color::G],
        };
        pregame.characteristics.colors = vec![Color::G];
        pregame.characteristics.color_identity = vec![Color::G];
        pregame.source_labels = None;
        pregame.static_abilities = vec![opening_placement_ability()];
        pregame.capabilities = pregame.requirements();
        definitions.push(pregame);
        let record: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/synthetic-glaze-lattice.json"
        ))
        .unwrap();
        let raw = &record["source"];
        let mut hybrid = definitions
            .iter()
            .find(|d| {
                d.characteristics.kind == CardKind::Artifact
                    && d.characteristics.additional_types.is_empty()
                    && d.characteristics.subtypes.is_empty()
            })
            .unwrap()
            .clone();
        hybrid.name = raw["name"].as_str().unwrap().into();
        hybrid.oracle_id = raw["oracle_id"].as_str().unwrap().into();
        hybrid.provenance.printing_id = raw["id"].as_str().unwrap().into();
        hybrid.provenance.source_json = canonical(raw).unwrap();
        hybrid.provenance.source_sha256 = sha256(hybrid.provenance.source_json.as_bytes());
        hybrid.provenance.abilities_json = "[]".into();
        hybrid.provenance.ast_sha256 = sha256(b"[[]]");
        hybrid.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
        (
            hybrid.characteristics.mana_cost,
            hybrid.characteristics.hybrid_mana,
        ) = color_hybrid_cost(raw["mana_cost"].as_str().unwrap()).unwrap();
        hybrid.characteristics.colors = vec![Color::W, Color::U, Color::G];
        hybrid.characteristics.color_identity = hybrid.characteristics.colors.clone();
        hybrid.source_labels = None;
        hybrid.static_abilities.clear();
        hybrid.triggered.clear();
        hybrid.activated.clear();
        hybrid.mana_abilities.clear();
        hybrid.keywords.clear();
        hybrid.capabilities = hybrid.requirements();
        definitions.push(hybrid);

        let record: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/basic-landwalk-zendikar-farguide.json"
        ))
        .unwrap();
        let source = &record["source"];
        let mut landwalk = definitions
            .iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap()
            .clone();
        landwalk.name = source["name"].as_str().unwrap().into();
        landwalk.oracle_id = source["oracle_id"].as_str().unwrap().into();
        landwalk.provenance.printing_id = source["id"].as_str().unwrap().into();
        landwalk.provenance.source_json = canonical(source).unwrap();
        landwalk.provenance.source_sha256 = sha256(landwalk.provenance.source_json.as_bytes());
        landwalk.provenance.abilities_json =
            canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
        landwalk.provenance.ast_sha256 =
            sha256(format!("[{}]", landwalk.provenance.abilities_json).as_bytes());
        landwalk.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
        landwalk.characteristics.subtypes = vec!["Elemental".into()];
        landwalk.characteristics.power = Some(3.into());
        landwalk.characteristics.toughness = Some(3.into());
        landwalk.characteristics.mana_cost.generic = 4.into();
        landwalk.source_labels = None;
        landwalk.keywords = vec![KeywordAbility::Forestwalk];
        landwalk.capabilities = landwalk.requirements();
        definitions.push(landwalk);

        let record: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/source-combat-restriction-tidal-kraken.json"
        ))
        .unwrap();
        let source = &record["source"];
        let mut kraken = definitions
            .iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap()
            .clone();
        kraken.name = source["name"].as_str().unwrap().into();
        kraken.oracle_id = source["oracle_id"].as_str().unwrap().into();
        kraken.provenance.printing_id = source["id"].as_str().unwrap().into();
        kraken.provenance.source_json = canonical(source).unwrap();
        kraken.provenance.source_sha256 = sha256(kraken.provenance.source_json.as_bytes());
        kraken.provenance.abilities_json =
            canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
        kraken.provenance.ast_sha256 =
            sha256(format!("[{}]", kraken.provenance.abilities_json).as_bytes());
        kraken.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
        kraken.characteristics.subtypes = vec!["Kraken".into()];
        kraken.characteristics.power = Some(6.into());
        kraken.characteristics.toughness = Some(6.into());
        kraken.characteristics.mana_cost = ManaCost {
            generic: 5.into(),
            colored: vec![Color::U; 3],
        };
        kraken.characteristics.colors = vec![Color::U];
        kraken.characteristics.color_identity = vec![Color::U];
        kraken.source_labels = None;
        kraken.static_abilities = vec![source_combat_restrictions::static_ability(
            CreatureRestriction::CantBeBlocked,
        )];
        kraken.capabilities = kraken.requirements();
        definitions.push(kraken);

        let record: serde_json::Value = strict_read(include_str!(
            "../../../tests/fixtures/activated-source-keyword-battlefly-swarm.json"
        ))
        .unwrap();
        let source = &record["source"];
        let mut self_grant = definitions
            .iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap()
            .clone();
        self_grant.name = source["name"].as_str().unwrap().into();
        self_grant.oracle_id = source["oracle_id"].as_str().unwrap().into();
        self_grant.provenance.printing_id = source["id"].as_str().unwrap().into();
        self_grant.provenance.source_json = canonical(source).unwrap();
        self_grant.provenance.source_sha256 = sha256(self_grant.provenance.source_json.as_bytes());
        self_grant.provenance.abilities_json =
            canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
        self_grant.provenance.ast_sha256 =
            sha256(format!("[{}]", self_grant.provenance.abilities_json).as_bytes());
        self_grant.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
        self_grant.characteristics.subtypes = vec!["Phyrexian".into(), "Insect".into()];
        self_grant.characteristics.power = Some(1.into());
        self_grant.characteristics.toughness = Some(1.into());
        self_grant.characteristics.mana_cost = ManaCost {
            generic: 0.into(),
            colored: vec![Color::B],
        };
        self_grant.characteristics.colors = vec![Color::B];
        self_grant.characteristics.color_identity = vec![Color::B];
        self_grant.source_labels = None;
        self_grant.keywords = vec![KeywordAbility::Flying];
        self_grant.activated = vec![
            activated_source_keyword::printed_grant(
                record["faces"][0]["result"]["lines"][1]["text"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap()
            .activation(1),
        ];
        self_grant.capabilities = self_grant.requirements();
        definitions.push(self_grant);

        definitions.push(self_graveyard_shuffle::tests::definition());
        for cap in vocabulary {
            assert!(
                definitions.iter().any(|d| d.capabilities.contains(&cap)
                    && Catalog {
                        schema_version: CURRENT_IR_VERSION,
                        rules_edition: RULES_EDITION.into(),
                        rules_sha256: RULES_SHA256.into(),
                        definitions: vec![d.clone()],
                    }
                    .validate()
                    .is_ok()),
                "{cap:?}: no complete current witness"
            );
        }
    }
    #[test]
    fn optional_draw_requirements_include_nested_instructions() {
        let catalog: Catalog = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-choices-ir.json"
        ))
        .unwrap();
        let mut modal = catalog
            .definitions
            .iter()
            .find(|d| d.spell.as_ref().is_some_and(|p| !p.modes.is_empty()))
            .unwrap()
            .clone();
        let child = &mut modal.spell.as_mut().unwrap().modes[0].program;
        child.targets.clear();
        child.instructions = vec![Instruction::OptionalDrawCards { source_block: 0 }];
        modal.capabilities = modal.requirements();
        assert!(
            modal
                .capabilities
                .contains(&Capability::OptionalControllerDrawV1)
        );
        let mut standalone = modal.clone();
        let p = standalone.spell.as_mut().unwrap();
        p.modes.clear();
        p.targets.clear();
        p.instructions = vec![Instruction::OptionalDrawCards { source_block: 0 }];
        standalone.capabilities = standalone.requirements();
        for d in [standalone, modal] {
            let mut missing = d.clone();
            missing
                .capabilities
                .remove(&Capability::OptionalControllerDrawV1);
            assert!(missing.validate().is_err());
            Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d],
            }
            .validate()
            .unwrap();
        }
    }
    #[test]
    fn event_player_discard_has_a_strict_instruction_shape_and_a_separate_binding_domain() {
        let raw = r#"{"op":"discard_cards","count":"1","source_block":0}"#;
        let instruction: Instruction = strict_read(raw).unwrap();
        assert_eq!(
            instruction,
            Instruction::DiscardCards {
                recipient: None,
                count: Quantity::from(1),
                source_block: 0,
            }
        );
        assert_eq!(serde_json::to_string(&instruction).unwrap(), raw);
        for recipient in [
            serde_json::json!({"event_player":null}),
            serde_json::json!({"controller":null}),
            serde_json::json!("target_player"),
        ] {
            assert!(serde_json::from_value::<ActionRecipient>(recipient).is_err());
        }

        let original: Catalog = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-trigger-ir.json"
        ))
        .unwrap();
        let mut bound = original
            .definitions
            .iter()
            .find(|d| d.characteristics.kind == CardKind::Creature && !d.triggered.is_empty())
            .unwrap()
            .clone();
        bound.triggered.truncate(1);
        let ability = &mut bound.triggered[0];
        ability.condition = TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        };
        ability.intervening_if = None;
        ability.death_filter = None;
        ability.program.targets.clear();
        ability.program.modes.clear();
        ability.program.instructions = vec![Instruction::DiscardCards {
            recipient: Some(ActionRecipient::EventPlayer),
            count: Quantity::from(1),
            source_block: 0,
        }];
        bound.capabilities = bound.requirements();
        bound.validate().unwrap();
        assert!(
            bound
                .capabilities
                .contains(&Capability::EventPlayerDiscardV1)
        );
        assert!(
            !bound
                .capabilities
                .contains(&Capability::EventPlayerRecipientV1)
        );
        assert!(
            !bound
                .capabilities
                .contains(&Capability::ControllerDiscardV1)
        );
        for condition in [
            TriggerCondition::SelfEntersBattlefield,
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::Controller,
            },
            TriggerCondition::SpellCast {
                caster: CasterRelation::Controller,
                spell: TypePredicate::Any {},
            },
        ] {
            let mut unbound = bound.clone();
            unbound.triggered[0].condition = condition;
            unbound.capabilities = unbound.requirements();
            assert!(
                unbound
                    .validate()
                    .unwrap_err()
                    .contains("event-player discard")
            );
        }
        let mut spell = bound.clone();
        spell.spell.as_mut().unwrap().instructions =
            spell.triggered[0].program.instructions.clone();
        spell.capabilities = spell.requirements();
        assert!(
            spell
                .validate()
                .unwrap_err()
                .contains("event-player discard")
        );
        let actions: Catalog = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-action-ir.json"
        ))
        .unwrap();
        let mut activation = actions
            .definitions
            .into_iter()
            .find(|d| !d.activated.is_empty())
            .unwrap();
        activation.activated[0].program.instructions =
            bound.triggered[0].program.instructions.clone();
        activation.capabilities = activation.requirements();
        assert!(
            activation
                .validate()
                .unwrap_err()
                .contains("event-player discard")
        );
        let mut c = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![bound.clone()],
        };
        c.validate().unwrap();
        c.definitions[0]
            .capabilities
            .remove(&Capability::EventPlayerDiscardV1);
        assert!(c.validate().is_err());

        // Requirements include nested programs independently of instruction context.
        let mut nested = bound;
        let child = nested.triggered[0].program.clone();
        let program = nested.spell.as_mut().unwrap();
        program.modes = vec![SpellMode { program: child }];
        nested.triggered.clear();
        nested.capabilities = nested.requirements();
        nested
            .capabilities
            .remove(&Capability::EventPlayerDiscardV1);
        c.definitions = vec![nested];
        assert!(c.validate().is_err());
    }
    #[test]
    fn event_player_sacrifice_requires_bound_damage_and_checks_nested_requirements() {
        let original: Catalog = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-trigger-ir.json"
        ))
        .unwrap();
        let mut bound = original
            .definitions
            .iter()
            .find(|d| d.characteristics.kind == CardKind::Creature && !d.triggered.is_empty())
            .unwrap()
            .clone();
        bound.triggered.truncate(1);
        let a = &mut bound.triggered[0];
        a.condition = TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        };
        a.intervening_if = None;
        a.death_filter = None;
        a.program.targets.clear();
        a.program.modes.clear();
        a.program.instructions = vec![Instruction::SacrificeCreature {
            recipient: ActionRecipient::EventPlayer,
            source_block: 0,
        }];
        bound.capabilities = bound.requirements();
        bound.validate().unwrap();
        assert!(
            bound
                .capabilities
                .contains(&Capability::EventPlayerSacrificeV1)
        );
        assert!(
            !bound
                .capabilities
                .contains(&Capability::EventPlayerDiscardV1)
        );
        assert!(
            !bound
                .capabilities
                .contains(&Capability::EventPlayerRecipientV1)
        );
        for condition in [
            TriggerCondition::SelfEntersBattlefield,
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::Controller,
            },
            TriggerCondition::SpellCast {
                caster: CasterRelation::Controller,
                spell: TypePredicate::Any {},
            },
        ] {
            let mut unbound = bound.clone();
            unbound.triggered[0].condition = condition;
            unbound.capabilities = unbound.requirements();
            assert!(
                unbound
                    .validate()
                    .unwrap_err()
                    .contains("bound damage trigger")
            );
        }
        let mut spell = bound.clone();
        spell.spell.as_mut().unwrap().instructions =
            spell.triggered[0].program.instructions.clone();
        spell.capabilities = spell.requirements();
        assert!(
            spell
                .validate()
                .unwrap_err()
                .contains("bound damage trigger")
        );
        let actions: Catalog = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-action-ir.json"
        ))
        .unwrap();
        let mut activation = actions
            .definitions
            .into_iter()
            .find(|d| !d.activated.is_empty())
            .unwrap();
        activation.activated[0].program.instructions =
            bound.triggered[0].program.instructions.clone();
        activation.capabilities = activation.requirements();
        assert!(
            activation
                .validate()
                .unwrap_err()
                .contains("bound damage trigger")
        );
        let mut c = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![bound.clone()],
        };
        c.validate().unwrap();
        c.definitions[0]
            .capabilities
            .remove(&Capability::EventPlayerSacrificeV1);
        assert!(c.validate().is_err());
        let mut nested = bound;
        let child = nested.triggered[0].program.clone();
        nested.spell.as_mut().unwrap().modes = vec![SpellMode { program: child }];
        nested.triggered.clear();
        nested.capabilities = nested.requirements();
        nested
            .capabilities
            .remove(&Capability::EventPlayerSacrificeV1);
        c.definitions = vec![nested];
        assert!(c.validate().is_err());
        assert_eq!(
            serde_json::from_str::<ActionRecipient>("\"controller\"").unwrap(),
            ActionRecipient::Controller
        );
        assert_eq!(
            serde_json::from_str::<ActionRecipient>("\"event_player\"").unwrap(),
            ActionRecipient::EventPlayer
        );
    }
    #[test]
    fn filtered_damage_sources_validate_predicates_observers_and_capabilities() {
        let original: Catalog = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-mana-ir.json"
        ))
        .unwrap();
        let mut observer = original
            .definitions
            .iter()
            .find(|d| d.name == "Mind Stone")
            .unwrap()
            .clone();
        let mut program = observer.activated[0].program.clone();
        program.instructions = vec![Instruction::DrawCards {
            recipient: None,
            count: Quantity::from(1),
            source_block: 0,
        }];
        let predicate = TypePredicate::All {
            terms: vec![
                TypePredicate::Type {
                    kind: CardKind::Artifact,
                },
                TypePredicate::Type {
                    kind: CardKind::Creature,
                },
            ],
        };
        observer.triggered = vec![TriggeredAbility {
            functioning_zone: FunctioningZone::Battlefield,
            death_filter: None,
            intervening_if: None,
            condition: TriggerCondition::DamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: predicate,
                },
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
            },
            program,
        }];
        observer.capabilities = observer.requirements();
        // Synthetic contract witness: filtered observers need not be creatures.
        observer.validate().unwrap();
        assert!(
            observer
                .capabilities
                .contains(&Capability::FilteredDamageTriggerV1)
        );
        assert!(observer.capabilities.contains(&Capability::DamageTriggerV1));
        assert!(
            !observer
                .capabilities
                .contains(&Capability::SourceDamageTriggerV2)
        );
        let mut c = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![observer.clone()],
        };
        c.validate().unwrap();
        c.definitions[0]
            .capabilities
            .remove(&Capability::FilteredDamageTriggerV1);
        assert!(c.validate().is_err());
        let mut self_observer = observer.clone();
        if let TriggerCondition::DamageDealt { source, .. } =
            &mut self_observer.triggered[0].condition
        {
            *source = DamageTriggerSource::Source {};
        }
        self_observer.capabilities = self_observer.requirements();
        assert!(
            self_observer
                .validate()
                .unwrap_err()
                .contains("creature source")
        );
        for (kind, recipient) in [
            (TriggerDamageKind::Any, DamageTriggerRecipient::Player),
            (
                TriggerDamageKind::Combat,
                DamageTriggerRecipient::OpponentPlayer,
            ),
        ] {
            let mut extended = observer.clone();
            if let TriggerCondition::DamageDealt {
                kind: k,
                recipient: r,
                ..
            } = &mut extended.triggered[0].condition
            {
                *k = kind;
                *r = recipient;
            }
            extended.capabilities = extended.requirements();
            extended.validate().unwrap();
            assert!(
                extended
                    .capabilities
                    .contains(&Capability::FilteredDamageTriggerV1)
            );
            assert!(
                extended
                    .capabilities
                    .contains(&Capability::SourceDamageTriggerV2)
            );
            assert!(!extended.capabilities.contains(&Capability::DamageTriggerV1));
        }
        for predicate in [
            TypePredicate::All { terms: vec![] },
            TypePredicate::AnyOf {
                terms: vec![TypePredicate::Any {}; 33],
            },
        ] {
            let mut bad = observer.clone();
            if let TriggerCondition::DamageDealt {
                source: DamageTriggerSource::CreatureFilter { types, .. },
                ..
            } = &mut bad.triggered[0].condition
            {
                *types = predicate;
            }
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err());
        }
        for relation in [
            serde_json::json!({"controller":null}),
            serde_json::json!("owner"),
            serde_json::json!("any_player"),
            serde_json::json!("opponent"),
            serde_json::json!(true),
        ] {
            assert!(serde_json::from_value::<DamageSourceRelation>(relation).is_err());
        }
    }
    #[test]
    fn damage_controller_discard_is_contextual_and_recursive() {
        let original = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-action-ir.json"
        ))
        .unwrap();
        let activation = original
            .definitions
            .iter()
            .find(|d| d.name == "Hapless Researcher")
            .unwrap();
        assert!(
            activation
                .capabilities
                .contains(&Capability::ControllerDiscardV1)
        );
        assert!(
            !activation
                .capabilities
                .contains(&Capability::DamageControllerDiscardV1)
        );
        let mut bound = activation.clone();
        let program = bound.activated[0].program.clone();
        bound.activated.clear();
        bound.triggered = vec![TriggeredAbility {
            death_filter: None,
            intervening_if: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::DamageDealt {
                source: DamageTriggerSource::Source {},
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
            },
            program,
        }];
        for recipient in [None, Some(ActionRecipient::Controller)] {
            let mut d = bound.clone();
            for i in &mut d.triggered[0].program.instructions {
                if let Instruction::DiscardCards { recipient: r, .. } = i {
                    *r = recipient;
                }
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            assert!(d.capabilities.contains(&Capability::ControllerDiscardV1));
            assert!(
                d.capabilities
                    .contains(&Capability::DamageControllerDiscardV1)
            );
            let mut c = Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d],
            };
            c.validate().unwrap();
            c.definitions[0]
                .capabilities
                .remove(&Capability::DamageControllerDiscardV1);
            assert!(c.validate().is_err());
        }
        for condition in [
            TriggerCondition::SelfEntersBattlefield,
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::Controller,
            },
            TriggerCondition::SpellCast {
                caster: CasterRelation::Controller,
                spell: TypePredicate::Any {},
            },
        ] {
            let mut unbound = bound.clone();
            unbound.triggered[0].condition = condition;
            unbound.capabilities = unbound.requirements();
            assert!(
                !unbound
                    .capabilities
                    .contains(&Capability::DamageControllerDiscardV1)
            );
            assert!(unbound.validate().unwrap_err().contains("damage trigger"));
        }
        let mut event = bound.clone();
        for i in &mut event.triggered[0].program.instructions {
            if let Instruction::DiscardCards { recipient, .. } = i {
                *recipient = Some(ActionRecipient::EventPlayer);
            }
        }
        event.capabilities = event.requirements();
        assert!(
            event
                .capabilities
                .contains(&Capability::EventPlayerDiscardV1)
        );
        assert!(
            !event
                .capabilities
                .contains(&Capability::DamageControllerDiscardV1)
        );
        Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![event],
        }
        .validate()
        .unwrap();
        // Trigger modes remain unsupported; requirements still include nested discards.
        let child = bound.triggered[0].program.clone();
        let p = &mut bound.triggered[0].program;
        p.instructions.clear();
        p.modes = vec![
            SpellMode {
                program: child.clone(),
            },
            SpellMode { program: child },
        ];
        bound.capabilities = bound.requirements();
        assert!(
            bound
                .capabilities
                .contains(&Capability::DamageControllerDiscardV1)
        );
        bound
            .capabilities
            .remove(&Capability::DamageControllerDiscardV1);
        let c = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![bound],
        };
        assert!(c.validate().is_err());
    }
    #[test]
    fn damage_quantity_draw_is_strict_bound_and_recursive() {
        let raw = r#"{"op":"draw_cards","count":"1","source_block":0}"#;
        let fixed: Instruction = strict_read(raw).unwrap();
        assert_eq!(serde_json::to_string(&fixed).unwrap(), raw);
        let opcode = serde_json::json!({"op":"draw_damage_amount","source_block":0});
        let dynamic: Instruction = serde_json::from_value(opcode.clone()).unwrap();
        assert_eq!(dynamic, Instruction::DrawDamageAmount { source_block: 0 });
        for (field, value) in [
            ("count", serde_json::json!("2")),
            ("recipient", serde_json::json!("controller")),
            ("optional", serde_json::json!(false)),
        ] {
            let mut invalid = opcode.clone();
            invalid[field] = value;
            assert!(serde_json::from_value::<Instruction>(invalid).is_err());
        }
        let original = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-action-ir.json"
        ))
        .unwrap();
        let mut d = original
            .definitions
            .iter()
            .find(|d| d.name == "Hapless Researcher")
            .unwrap()
            .clone();
        let mut program = d.activated[0].program.clone();
        program.targets.clear();
        program.instructions = vec![dynamic];
        d.activated.clear();
        d.triggered = vec![TriggeredAbility {
            death_filter: None,
            intervening_if: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::DamageDealt {
                source: DamageTriggerSource::Source {},
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
            },
            program: program.clone(),
        }];
        d.capabilities = d.requirements();
        d.validate().unwrap();
        assert!(d.capabilities.contains(&Capability::DamageQuantityDrawV1));
        assert!(!d.capabilities.contains(&Capability::ControllerDrawV1));
        let mut catalog = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![d.clone()],
        };
        catalog.validate().unwrap();
        catalog.definitions[0]
            .capabilities
            .remove(&Capability::DamageQuantityDrawV1);
        assert!(catalog.validate().is_err());
        let mut missing = d.clone();
        missing
            .capabilities
            .remove(&Capability::DamageQuantityDrawV1);
        assert!(missing.validate().is_err());
        for condition in [
            TriggerCondition::SelfEntersBattlefield,
            TriggerCondition::SelfDies,
            TriggerCondition::CreatureAttacked {
                attacker: AttackSubject::Source {},
            },
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::Controller,
            },
            TriggerCondition::SpellCast {
                caster: CasterRelation::Controller,
                spell: TypePredicate::Any {},
            },
        ] {
            let mut unbound = d.clone();
            unbound.triggered[0].condition = condition;
            unbound.capabilities = unbound.requirements();
            assert!(
                unbound
                    .validate()
                    .unwrap_err()
                    .contains("bound damage trigger")
            );
        }
        // Requirements inspect all program-bearing locations and recurse into modes
        // even when the context itself is unsupported.
        for nested in [false, true] {
            let mut p = program.clone();
            if nested {
                p.instructions.clear();
                p.modes = vec![
                    SpellMode {
                        program: program.clone(),
                    },
                    SpellMode {
                        program: program.clone(),
                    },
                ];
            }
            for location in 0..3 {
                let mut unbound = d.clone();
                unbound.triggered.clear();
                match location {
                    0 => unbound.spell = Some(p.clone()),
                    1 => {
                        unbound.activated = vec![ActivatedAbility {
                            cost: ActivationCost::TapSource,
                            functioning_zone: FunctioningZone::Battlefield,
                            program: p.clone(),
                        }]
                    }
                    _ => {
                        let mut ability = d.triggered[0].clone();
                        ability.program = p.clone();
                        unbound.triggered = vec![ability];
                    }
                }
                unbound.capabilities = unbound.requirements();
                assert!(
                    unbound
                        .capabilities
                        .contains(&Capability::DamageQuantityDrawV1)
                );
                if location != 2 {
                    assert!(
                        unbound
                            .validate()
                            .unwrap_err()
                            .contains("bound damage trigger")
                    );
                }
                unbound
                    .capabilities
                    .remove(&Capability::DamageQuantityDrawV1);
                let missing = Catalog {
                    schema_version: CURRENT_IR_VERSION,
                    rules_edition: RULES_EDITION.into(),
                    rules_sha256: RULES_SHA256.into(),
                    definitions: vec![unbound],
                };
                assert!(missing.validate().is_err());
            }
        }
    }
    #[test]
    fn snow_and_attacking_keyword_dimensions_are_strict_and_independent() {
        let old = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-wipes-ir.json"
        ))
        .unwrap();
        let base = old
            .definitions
            .iter()
            .find(|d| d.name == "Storm Crow")
            .unwrap()
            .clone();
        let old_bytes = serde_json::to_string(&base.characteristics).unwrap();
        assert!(!old_bytes.contains("snow"));
        let decoded: Characteristics = strict_read(&old_bytes).unwrap();
        assert!(!decoded.snow);
        assert_eq!(serde_json::to_string(&decoded).unwrap(), old_bytes);
        for value in [
            serde_json::Value::Null,
            serde_json::json!("true"),
            serde_json::json!(1),
        ] {
            let mut c = serde_json::to_value(&decoded).unwrap();
            c["snow"] = value;
            assert!(serde_json::from_value::<Characteristics>(c).is_err());
        }
        let grant = StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::AttackingController,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![KeywordAbility::Deathtouch],
                restrictions: vec![],
            },
        };
        for (snow, attacking) in [(true, false), (false, true), (true, true)] {
            let mut d = base.clone();
            d.characteristics.snow = snow;
            if attacking {
                d.static_abilities.push(grant.clone());
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            assert_eq!(d.capabilities.contains(&Capability::SnowSupertypeV1), snow);
            assert_eq!(
                d.capabilities
                    .contains(&Capability::AttackingCreatureKeywordsV1),
                attacking
            );
            assert!(!d.capabilities.contains(&Capability::AttachmentsV1));
            let mut c = Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d.clone()],
            };
            c.validate().unwrap();
            c.definitions[0]
                .capabilities
                .remove(&Capability::SnowSupertypeV1);
            c.definitions[0]
                .capabilities
                .remove(&Capability::AttackingCreatureKeywordsV1);
            assert!(c.validate().is_err());
            for cap in [
                Capability::SnowSupertypeV1,
                Capability::AttackingCreatureKeywordsV1,
            ] {
                if d.capabilities.contains(&cap) {
                    let mut missing = d.clone();
                    missing.capabilities.remove(&cap);
                    assert!(missing.validate().is_err());
                }
            }
        }
        // Extra capability labels without the corresponding semantic shape reject.
        for cap in [
            Capability::SnowSupertypeV1,
            Capability::AttackingCreatureKeywordsV1,
        ] {
            let mut d = base.clone();
            d.capabilities.insert(cap);
            let c = Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d],
            };
            assert!(c.validate().is_err());
        }
        let mut snow_mana = base.clone();
        snow_mana.characteristics.snow = true;
        snow_mana.mana_abilities = old
            .definitions
            .iter()
            .find(|d| d.name == "Sol Ring")
            .unwrap()
            .mana_abilities
            .clone();
        snow_mana.capabilities = snow_mana.requirements();
        assert!(
            snow_mana
                .validate()
                .unwrap_err()
                .contains("non-mana creature")
        );
        for kind in [
            CardKind::Land,
            CardKind::Artifact,
            CardKind::Enchantment,
            CardKind::Instant,
            CardKind::Sorcery,
        ] {
            let mut d = base.clone();
            d.characteristics.snow = true;
            d.characteristics.kind = kind;
            d.capabilities = d.requirements();
            assert!(
                d.validate()
                    .unwrap_err()
                    .contains(if kind == CardKind::Land {
                        "unsupported complete pure typed nonbasic land"
                    } else {
                        "non-mana creature"
                    })
            );
        }
        for (power, toughness, keywords, restrictions) in [
            (
                Quantity::from(1),
                Quantity::zero(),
                vec![KeywordAbility::Deathtouch],
                vec![],
            ),
            (
                Quantity::zero(),
                Quantity::from(1),
                vec![KeywordAbility::Deathtouch],
                vec![],
            ),
            (Quantity::zero(), Quantity::zero(), vec![], vec![]),
            (
                Quantity::zero(),
                Quantity::zero(),
                vec![KeywordAbility::Flying],
                vec![],
            ),
            (
                Quantity::zero(),
                Quantity::zero(),
                vec![KeywordAbility::Deathtouch, KeywordAbility::Deathtouch],
                vec![],
            ),
            (
                Quantity::zero(),
                Quantity::zero(),
                vec![KeywordAbility::Deathtouch],
                vec![CreatureRestriction::CantBlock],
            ),
        ] {
            let mut d = base.clone();
            d.static_abilities = vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::ModifyCreatures {
                    scope: CreatureScope::AttackingController,
                    power,
                    toughness,
                    keywords,
                    restrictions,
                },
            }];
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
        }
        let mut unbound = base.clone();
        unbound.static_abilities = vec![grant.clone()];
        unbound.static_abilities[0].functioning_zone = FunctioningZone::Stack;
        unbound.capabilities = unbound.requirements();
        assert!(unbound.validate().is_err());
        // Controller scope uses the same live modifier family on creatures.
        unbound.static_abilities = vec![grant];
        if let StaticEffect::ModifyCreatures { scope, .. } = &mut unbound.static_abilities[0].effect
        {
            *scope = CreatureScope::Controller;
        }
        unbound.capabilities = unbound.requirements();
        unbound.validate().unwrap();
    }
    #[test]
    fn grouped_damage_and_cast_prohibition_have_independent_capabilities_and_strict_shapes() {
        let old = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-wipes-ir.json"
        ))
        .unwrap();
        let base = old
            .definitions
            .iter()
            .find(|d| d.name == "Storm Crow")
            .unwrap()
            .clone();
        let program = SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            }],
            modes: vec![],
        };
        let grouped = TriggeredAbility {
            death_filter: None,
            intervening_if: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::GroupedDamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: TypePredicate::Type {
                        kind: CardKind::Creature,
                    },
                },
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
                source_power: DamageSourcePowerPredicate::GreaterThanBase,
            },
            program,
        };
        for (prohibit, group) in [(true, false), (false, true), (true, true)] {
            let mut d = base.clone();
            if prohibit {
                d.static_abilities = vec![StaticAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    effect: StaticEffect::OpponentsCantCastDuringControllerTurn,
                }];
            }
            if group {
                d.triggered = vec![grouped.clone()];
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            assert_eq!(
                d.capabilities
                    .contains(&Capability::OpponentCastProhibitionV1),
                prohibit
            );
            assert_eq!(
                d.capabilities
                    .contains(&Capability::GroupedCreatureDamageTriggerV1),
                group
            );
            assert!(
                !d.capabilities
                    .contains(&Capability::FilteredDamageTriggerV1)
            );
            assert!(!d.capabilities.contains(&Capability::DamageTriggerV1));
            let c = Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d.clone()],
            };
            c.validate().unwrap();
            for cap in [
                Capability::OpponentCastProhibitionV1,
                Capability::GroupedCreatureDamageTriggerV1,
            ] {
                if d.capabilities.contains(&cap) {
                    let mut missing = d.clone();
                    missing.capabilities.remove(&cap);
                    assert!(missing.validate().is_err());
                }
            }
        }
        // A public enum vocabulary cannot introduce unknown actor/power syntax.
        for value in [
            serde_json::json!("base_power"),
            serde_json::json!("greater_than_printed_source"),
            serde_json::json!({"greater_than_base":{}}),
            serde_json::json!({"greater_than_base":null}),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<DamageSourcePowerPredicate>(value).is_err());
        }
        assert_eq!(
            serde_json::to_value(StaticEffect::OpponentsCantCastDuringControllerTurn).unwrap(),
            serde_json::json!("opponents_cant_cast_during_controller_turn")
        );
        assert!(serde_json::from_value::<StaticEffect>(serde_json::json!({"opponents_cant_cast_during_controller_turn":{"who":"opponents"}})).is_err());
        let mut d = base.clone();
        d.triggered = vec![grouped.clone()];
        // Explicit Controller has the same established meaning as omission.
        if let Instruction::DrawCards { recipient, .. } =
            &mut d.triggered[0].program.instructions[0]
        {
            *recipient = Some(LifeRecipient::Controller);
        }
        d.capabilities = d.requirements();
        d.validate().unwrap();
        for condition in [
            TriggerCondition::GroupedDamageDealt {
                source: DamageTriggerSource::Source {},
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
                source_power: DamageSourcePowerPredicate::GreaterThanBase,
            },
            TriggerCondition::GroupedDamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: TypePredicate::Any {},
                },
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
                source_power: DamageSourcePowerPredicate::GreaterThanBase,
            },
            TriggerCondition::GroupedDamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: TypePredicate::Type {
                        kind: CardKind::Creature,
                    },
                },
                kind: TriggerDamageKind::Any,
                recipient: DamageTriggerRecipient::Player,
                source_power: DamageSourcePowerPredicate::GreaterThanBase,
            },
            TriggerCondition::GroupedDamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: TypePredicate::Type {
                        kind: CardKind::Creature,
                    },
                },
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::OpponentPlayer,
                source_power: DamageSourcePowerPredicate::GreaterThanBase,
            },
        ] {
            let mut invalid = d.clone();
            invalid.triggered[0].condition = condition;
            invalid.capabilities = invalid.requirements();
            assert!(invalid.validate().is_err());
        }
        for instruction in [
            Instruction::DrawCards {
                recipient: Some(LifeRecipient::EventPlayer),
                count: 1.into(),
                source_block: 0,
            },
            Instruction::DrawCards {
                recipient: None,
                count: 2.into(),
                source_block: 0,
            },
            Instruction::DrawDamageAmount { source_block: 0 },
            Instruction::OptionalDrawCards { source_block: 0 },
            Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: 1.into(),
                source_block: 0,
            },
            Instruction::GainLife {
                recipient: LifeRecipient::EventPlayer,
                amount: 1.into(),
                source_block: 0,
            },
            Instruction::LoseLife {
                recipient: LifeRecipient::EventPlayer,
                amount: 1.into(),
                source_block: 0,
            },
            Instruction::DiscardCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            },
            Instruction::DiscardCards {
                recipient: Some(ActionRecipient::EventPlayer),
                count: 1.into(),
                source_block: 0,
            },
            Instruction::SacrificeCreature {
                recipient: ActionRecipient::EventPlayer,
                source_block: 0,
            },
        ] {
            let mut invalid = d.clone();
            invalid.triggered[0].program.instructions = vec![instruction];
            invalid.capabilities = invalid.requirements();
            assert!(invalid.validate().is_err());
        }
        let mut invalid = d.clone();
        let extra = invalid.triggered[0].program.instructions[0].clone();
        invalid.triggered[0].program.instructions.push(extra);
        invalid.capabilities = invalid.requirements();
        assert!(invalid.validate().is_err());
        let mut invalid = d.clone();
        let child = invalid.triggered[0].program.clone();
        invalid.triggered[0].program.instructions.clear();
        invalid.triggered[0].program.modes = vec![
            SpellMode {
                program: child.clone(),
            },
            SpellMode { program: child },
        ];
        invalid.capabilities = invalid.requirements();
        assert!(invalid.validate().is_err());
        let mut invalid = d.clone();
        invalid.triggered[0].functioning_zone = FunctioningZone::Stack;
        invalid.capabilities = invalid.requirements();
        assert!(invalid.validate().is_err());
        let mut invalid = d;
        invalid.triggered[0].intervening_if = Some(InterveningCondition::SourceStatus {
            status: TapStatus::Tapped,
        });
        invalid.capabilities = invalid.requirements();
        assert!(invalid.validate().is_err());
        // New casting scope does not widen source/functioning-zone admission.
        for source in ["Lightning Bolt", "Forest"] {
            let mut d = old
                .definitions
                .iter()
                .find(|d| d.name == source)
                .unwrap()
                .clone();
            d.static_abilities = vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::OpponentsCantCastDuringControllerTurn,
            }];
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
        }
        let mut d = base;
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Stack,
            effect: StaticEffect::OpponentsCantCastDuringControllerTurn,
        }];
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    #[test]
    fn controlled_creature_replacement_requires_its_own_capability_and_battlefield_source() {
        let c = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-wipes-ir.json"
        ))
        .unwrap();
        let effect = StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer;
        assert_eq!(
            serde_json::to_value(&effect).unwrap(),
            serde_json::json!("double_controller_creature_damage_to_permanent_or_player")
        );
        for invalid in [
            serde_json::json!({"double_controller_creature_damage_to_permanent_or_player":{"controller":"opponent"}}),
            serde_json::json!({"double_controller_creature_damage_to_permanent_or_player":{"factor":3}}),
        ] {
            assert!(serde_json::from_value::<StaticEffect>(invalid).is_err());
        }
        for name in ["Storm Crow", "Sol Ring", "Furnace of Rath"] {
            let mut d = c
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .clone();
            d.static_abilities = vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: effect.clone(),
            }];
            d.capabilities = d.requirements();
            d.validate().unwrap();
            assert!(
                d.capabilities
                    .contains(&Capability::ControlledCreatureDamageReplacementV1)
            );
            assert!(
                !d.capabilities
                    .contains(&Capability::StaticDamageReplacementV1)
            );
            Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d.clone()],
            }
            .validate()
            .unwrap();
            let mut missing = d.clone();
            missing
                .capabilities
                .remove(&Capability::ControlledCreatureDamageReplacementV1);
            assert!(missing.validate().is_err());
            let mut wrong = d;
            wrong.static_abilities[0].functioning_zone = FunctioningZone::Stack;
            wrong.capabilities = wrong.requirements();
            assert!(wrong.validate().is_err());
        }
        let mut both = c
            .definitions
            .iter()
            .find(|d| d.name == "Furnace of Rath")
            .unwrap()
            .clone();
        both.static_abilities.push(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: effect.clone(),
        });
        both.capabilities = both.requirements();
        both.validate().unwrap();
        assert!(
            both.capabilities
                .contains(&Capability::StaticDamageReplacementV1)
        );
        assert!(
            both.capabilities
                .contains(&Capability::ControlledCreatureDamageReplacementV1)
        );
        for name in ["Forest", "Lightning Bolt"] {
            let mut invalid = c
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .clone();
            invalid.static_abilities = vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: effect.clone(),
            }];
            invalid.capabilities = invalid.requirements();
            assert!(invalid.validate().is_err());
        }
    }
    #[test]
    fn blocking_creature_targets_require_current_root_spell_pumps() {
        let catalog = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-effects-ir.json"
        ))
        .unwrap();
        let mut pump = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Giant Growth")
            .unwrap()
            .clone();
        pump.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BlockingBattlefieldCreature;
        pump.capabilities = pump.requirements();
        pump.validate().unwrap();
        assert!(
            pump.capabilities
                .contains(&Capability::BlockingCreatureTargetV1)
        );
        assert!(!pump.capabilities.contains(&Capability::CreatureTargetV1));
        let mut missing = pump.clone();
        missing
            .capabilities
            .remove(&Capability::BlockingCreatureTargetV1);
        assert!(missing.validate().is_err());
        for change in [
            (|p: &mut SpellProgram| {
                p.targets[0].filter = Some(PermanentFilter {
                    power: None,
                    types: vec![CardKind::Creature],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                })
            }) as fn(&mut SpellProgram),
            |p| {
                p.instructions[0] = Instruction::ReturnToOwnerHand {
                    target_slot: 0,
                    source_block: 0,
                }
            },
            |p| {
                p.instructions.push(Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 0,
                })
            },
            |p| {
                p.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Creature],
                    subtypes: vec![],
                })
            },
            |p| p.targets[0].slot = 1,
            |p| p.targets[0].selection_id.clear(),
            |p| p.targets[0].domain = TargetDomain::AnyDamageTarget,
            |p| p.functioning_zone = FunctioningZone::Battlefield,
            |p| p.resolution = SpellResolution::Permanent,
        ] {
            let mut invalid = pump.clone();
            change(invalid.spell.as_mut().unwrap());
            invalid.capabilities = invalid.requirements();
            assert!(invalid.validate().is_err());
        }
        let program = pump.spell.clone().unwrap();
        let mut modal = pump.clone();
        let root = modal.spell.as_mut().unwrap();
        root.targets.clear();
        root.instructions.clear();
        root.modes = vec![
            SpellMode {
                program: program.clone()
            };
            2
        ];
        modal.capabilities = modal.requirements();
        assert!(
            modal
                .validate()
                .unwrap_err()
                .contains("ordinary nonmodal spell")
        );
        for activated in [false, true] {
            let mut permanent = catalog
                .definitions
                .iter()
                .find(|d| d.name == "Yoked Ox")
                .unwrap()
                .clone();
            if activated {
                permanent.activated = vec![ActivatedAbility {
                    cost: ActivationCost::TapSource,
                    functioning_zone: FunctioningZone::Battlefield,
                    program: program.clone(),
                }];
            } else {
                permanent.triggered = vec![TriggeredAbility {
                    condition: TriggerCondition::SelfEntersBattlefield,
                    functioning_zone: FunctioningZone::Battlefield,
                    death_filter: None,
                    intervening_if: None,
                    program: program.clone(),
                }];
            }
            permanent.capabilities = permanent.requirements();
            assert!(
                permanent
                    .validate()
                    .unwrap_err()
                    .contains("ordinary nonmodal spell")
            );
        }
        // A new target restriction cannot be carried by a permanent's cast
        // program or be confused with the existing unqualified creature domain.
        let mut permanent = pump.clone();
        permanent.characteristics.kind = CardKind::Creature;
        permanent.capabilities = permanent.requirements();
        assert!(permanent.validate().is_err());
        let ordinary = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Giant Growth")
            .unwrap();
        assert_eq!(
            ordinary.spell.as_ref().unwrap().targets[0].domain,
            TargetDomain::BattlefieldCreature
        );
        ordinary.validate().unwrap();
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

#[cfg(test)]
mod indestructible_tests;

#[cfg(test)]
mod equipped_indestructible_tests;

#[cfg(test)]
mod temporary_indestructible_tests;

#[cfg(test)]
mod temporary_flying_tests;

#[cfg(test)]
mod compound_indestructible_family_tests;
#[cfg(test)]
mod compound_indestructible_tests;

#[cfg(test)]
mod temporary_deathtouch_tests;

#[cfg(test)]
mod ramos_m1_tests;

#[cfg(test)]
mod counter_placement_tests;

#[cfg(test)]
mod temporary_creature_keyword_tests;

#[cfg(test)]
mod temporary_combat_keyword_tests;

#[cfg(test)]
mod aura_control_tests;

#[cfg(test)]
mod creature_set_keyword_tests;

#[cfg(test)]
mod static_controller_tests;

#[cfg(test)]
mod permanent_set_keyword_tests;

#[cfg(test)]
mod spell_cascade_grant_tests;

#[cfg(test)]
mod temporary_permanent_keyword_tests;

#[cfg(test)]
mod temporary_reach_tests;
#[cfg(test)]
mod typed_land_search_tests;

mod sacrifice_set;
pub use sacrifice_set::sacrifice_source_creature_set_keyword_ability;

#[cfg(test)]
mod sacrifice_set_tests;
#[cfg(test)]
mod typed_fetchland_tests;

#[cfg(test)]
mod temporary_vigilance_tests;

#[cfg(test)]
mod temporary_double_strike_tests;

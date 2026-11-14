//! Executable Card IR contracts. No rules execution or parser dependency.
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
use serde::{Deserialize, Serialize, de};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt, str::FromStr};

mod archives;
pub use archives::*;
mod strict_io;
pub use strict_io::strict_read_seekable;

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
    pub fn has_type(&self, kind: CardKind) -> bool {
        self.kind == kind || self.additional_types.contains(&kind)
    }
}

/// Rule-bearing subtype boundaries from CR 205.3g–k, 301.5–7, 303.4/7,
/// 709.5, 714–720. Other subtype labels do not themselves supply abilities.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub fn unsupported_rules_subtype(subtype: &str) -> bool {
    matches!(
        subtype,
        "Saga"
            | "Class"
            | "Case"
            | "Vehicle"
            | "Curse"
            | "Spacecraft"
            | "Attraction"
            | "Room"
            | "Contraption"
            | "Fortification"
            | "Role"
            | "Adventure"
            | "Omen"
    )
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    CastTriggerV1,
    AttackTriggerV1,
    DamageTriggerV1,
    SourceDamageTriggerV2,
    OptionalControllerDrawV1,
    EventPlayerDiscardV1,
    EventPlayerSacrificeV1,
    FilteredDamageTriggerV1,
    DamageControllerDiscardV1,
    DamageQuantityDrawV1,
    SnowSupertypeV1,
    AttackingCreatureKeywordsV1,
    OpponentCastProhibitionV1,
    GroupedCreatureDamageTriggerV1,
    EventPlayerRecipientV1,
    SourceStatusConditionV1,
    StepBeginningTriggerV1,
    EntersTappedV1,
    FlashV1,
    CastArtifactV1,
    PermanentManaV1,
    CompositeActivationCostV1,
    CastEnchantmentV1,
    TemporaryPowerToughnessV1,
    CombatDamagePreventionV1,
    DamageBudgetPreventionV1,
    StaticDamageReplacementV1,
    ControlledCreatureDamageReplacementV1,
    NormalLayoutV1,
    BasicLandManaV1,
    LandPlayV1,
    BaseCreatureV1,
    CommanderIdentityV1,
    CastInstantV1,
    CastCreatureV1,
    CreatureTargetV1,
    BlockingCreatureTargetV1,
    TappedPermanentTargetV1,
    UntapPermanentV1,
    TapPermanentV1,
    ActivatedTapPermanentV1,
    TapSourceDestroyV1,
    ActivatedSpellCounterV1,
    ActivatedPermanentOwnerHandV1,
    OwnerHandMoveV1,
    PermanentOwnerHandMoveV1,
    CastSorceryV1,
    OwnerLibraryTopMoveV1,
    ActivatedAbilityV1,
    SacrificeSourceCostV1,
    ControllerDrawV1,
    ControllerDiscardV1,
    OpponentTargetV1,
    TargetPlayerSacrificeCreatureV1,
    StackSpellTargetV1,
    StackAbilityTargetV1,
    CounterStackObjectV1,
    TapSourceCostV1,
    ManaActivationCostV1,
    AnyDamageTargetV1,
    AnyPlayerTargetV1,
    NoncombatDamageV1,
    LifeGainV1,
    LifeLossV1,
    TriggeredAbilityV1,
    SelfEntersBattlefieldV1,
    OtherCreatureEntersBattlefieldV1,
    CreatureDiesV1,
    SelfDiesV1,
    InterveningIfV1,
    ModalChoiceV1,
    DefenderV1,
    FlyingV1,
    ReachV1,
    VigilanceV1,
    HasteV1,
    LifelinkV1,
    DeathtouchV1,
    TrampleV1,
    FirstStrikeV1,
    DoubleStrikeV1,
    MenaceV1,
    CreatureTokensV1,
    PlusOneCountersV1,
    DestroyCreatureV1,
    GraveyardRecursionV1,
    BasicLandSearchV1,
    CreatureAnthemV1,
    TargetPlayerDrawV1,
    AttachmentsV1,
    PermanentRemovalV1,
    SacrificeV1,
    BoardWipesV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunctioningZone {
    Battlefield,
    Stack,
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
    BattlefieldPermanent,
    ControlledBattlefieldCreature,
    BattlefieldCreature,
    /// CR 509.1g: a current blocking creature; ordinary spell pumps only.
    BlockingBattlefieldCreature,
    OwnGraveyardCreatureCard,
    OpponentPlayer,
    StackSpell,
    StackActivatedOrTriggeredAbility,
    /// CR 115.4: creatures, players, planeswalkers and battles. The closed
    /// scenario admits only creature/player inhabitants, not a narrower domain.
    AnyDamageTarget,
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
}
/// Closed non-targeted sets, evaluated as an instruction is performed (CR 608.2h).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectSet {
    Creatures,
    NonlandPermanents,
    Enchantments,
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
    Fixed { value: Quantity },
    AnnouncedX,
    CreatureCount,
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
    pub colors: Vec<Color>,
    pub subtypes: Vec<String>,
    pub power: Quantity,
    pub toughness: Quantity,
    pub keywords: Vec<KeywordAbility>,
}
impl CreatureToken {
    pub fn validate(&self) -> Result<(), String> {
        if self.keywords.contains(&KeywordAbility::Flash)
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
// Tagged operations are deliberate; unknown variants and fields reject.
// https://serde.rs/enum-representations.html
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Instruction {
    MoveSet {
        set: ObjectSet,
        action: SetMove,
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
    ReturnOwnGraveyardCreatureToHand {
        target_slot: u32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        optional: bool,
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
    PutPlusOneCounters {
        on: CounterRecipient,
        amount: Quantity,
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
    ReturnToOwnerHand {
        target_slot: u32,
        source_block: u32,
    },
    PutOnTopOwnerLibrary {
        target_slot: u32,
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
    DrawCards {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipient: Option<LifeRecipient>,
        count: Quantity,
        source_block: u32,
    },
    /// CR 603.5/608.2d: the controller chooses at resolution whether to draw
    /// one card, including when their library is empty.
    OptionalDrawCards {
        source_block: u32,
    },
    /// Draw once per unit of the triggering actual damage amount (CR 121.2).
    DrawDamageAmount {
        source_block: u32,
    },
    DiscardCards {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipient: Option<ActionRecipient>,
        count: Quantity,
        source_block: u32,
    },
    CounterStackObject {
        target_slot: u32,
        source_block: u32,
    },
    /// Noncombat damage from this invocation's physical source (CR 120.2b).
    DealDamage {
        target_slot: u32,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
    GainLife {
        recipient: LifeRecipient,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
    LoseLife {
        recipient: LifeRecipient,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StaticEffect {
    /// CR 101.2/109.5/601.3: opponents cannot cast during this controller's turn.
    OpponentsCantCastDuringControllerTurn,
    EntersTapped,
    ReduceGenericByCreatureCount,
    CantBeCountered,
    ModifyCreatures {
        scope: CreatureScope,
        power: Quantity,
        toughness: Quantity,
        keywords: Vec<KeywordAbility>,
        restrictions: Vec<CreatureRestriction>,
    },
    ControllerCreatureAnthem {
        power: Quantity,
        toughness: Quantity,
    },
    DoubleDamageToPermanentOrPlayer,
    /// Double damage from creatures controlled by this battlefield source's controller.
    DoubleControllerCreatureDamageToPermanentOrPlayer,
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
    Instructions,
    Permanent,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpellProgram {
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ActivationCost {
    SacrificePermanent {
        filter: SacrificeFilter,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mana: Option<ManaCost>,
    },
    SacrificeSource,
    TapSource,
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
    pub tap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<ManaCost>,
    pub sacrifice: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mana", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaProduction {
    Fixed { symbols: Vec<Color> },
    AnyColor,
    CommanderColor,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManaAbility {
    pub costs: PermanentActivationCosts,
    pub production: ManaProduction,
    pub source_block: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivatedAbility {
    pub functioning_zone: FunctioningZone,
    pub cost: ActivationCost,
    pub program: SpellProgram,
}
/// Mandatory ordinary event patterns. ETB patterns inspect the event's
/// resulting battlefield, including the entering source itself (CR 603.6a).
/// Dies inspects the battlefield immediately before the actual graveyard move
/// (CR 603.10/700.4), including a source that dies simultaneously.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TriggerCondition {
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
        spell: TypePredicate,
    },
    StepBegan {
        step: BeginningStep,
        turn_relation: TurnRelation,
    },
    OpponentCreatureDies,
    SelfEntersBattlefield,
    /// Excludes the exact source incarnation, never every copy of its card.
    OtherCreatureEntersBattlefield,
    /// The disjoint union of this creature and all other creatures dying.
    AnyCreatureDies,
    SelfDies,
    SelfAuraToGraveyard,
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
/// Source ObjectFilter fields are conjunctive; explicit anyOf retains OR.
/// Only full card-type membership is supported, never execution-kind equality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "predicate", rename_all = "snake_case", deny_unknown_fields)]
pub enum TypePredicate {
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
impl TypePredicate {
    pub fn matches(&self, c: &Characteristics) -> bool {
        match self {
            Self::Any {} => true,
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
            Self::SpellCast { .. } => "spell_cast",
            Self::OpponentCreatureDies => "opponent_creature_dies",
            Self::SelfEntersBattlefield => "self_enters_battlefield",
            Self::OtherCreatureEntersBattlefield => "other_creature_enters_battlefield",
            Self::AnyCreatureDies => "any_creature_dies",
            Self::SelfDies => "self_dies",
            Self::SelfAuraToGraveyard => "self_aura_to_graveyard",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum InterveningCondition {
    ControllerHandEmpty,
    SourceStatus { status: TapStatus },
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
    Trample,
    #[serde(rename = "first strike")]
    FirstStrike,
    #[serde(rename = "double strike")]
    DoubleStrike,
    Menace,
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
            "trample" => Ok(Self::Trample),
            "first strike" => Ok(Self::FirstStrike),
            "double strike" => Ok(Self::DoubleStrike),
            "menace" => Ok(Self::Menace),
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
                    "trample",
                    "first strike",
                    "double strike",
                    "menace",
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
impl Definition {
    pub fn requirements(&self) -> BTreeSet<Capability> {
        let mut set = BTreeSet::from([Capability::NormalLayoutV1]);
        if self.characteristics.snow {
            set.insert(Capability::SnowSupertypeV1);
        }
        if self.x_mana_cost {
            set.insert(Capability::BoardWipesV1);
        }
        match self.characteristics.kind {
            CardKind::Land => {
                set.insert(Capability::LandPlayV1);
                if self.characteristics.basic {
                    set.insert(Capability::BasicLandManaV1);
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
        for ability in &self.activated {
            set.insert(Capability::ActivatedAbilityV1);
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
            set.insert(match ability.cost {
                ActivationCost::SacrificePermanent { .. } => Capability::SacrificeV1,
                ActivationCost::SacrificeSource => Capability::SacrificeSourceCostV1,
                ActivationCost::TapSource => Capability::TapSourceCostV1,
                ActivationCost::Mana { .. } => Capability::ManaActivationCostV1,
                ActivationCost::Composite { .. } => Capability::CompositeActivationCostV1,
            });
            if ability.cost == ActivationCost::SacrificeSource
                && ability
                    .program
                    .instructions
                    .iter()
                    .any(|i| matches!(i, Instruction::SearchBasicLands { .. }))
            {
                set.insert(Capability::SacrificeV1);
            }
            add_program_requirements(&mut set, &ability.program);
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
            set.insert(match ability.condition {
                TriggerCondition::GroupedDamageDealt { .. } => {
                    Capability::GroupedCreatureDamageTriggerV1
                }
                TriggerCondition::StepBegan { .. } => Capability::StepBeginningTriggerV1,
                TriggerCondition::SpellCast { .. } => Capability::CastTriggerV1,
                TriggerCondition::CreatureAttacked { .. } => Capability::AttackTriggerV1,
                TriggerCondition::DamageDealt {
                    kind: TriggerDamageKind::Combat,
                    recipient: DamageTriggerRecipient::Player,
                    ..
                } => Capability::DamageTriggerV1,
                TriggerCondition::DamageDealt { .. } => Capability::SourceDamageTriggerV2,
                TriggerCondition::OpponentCreatureDies => Capability::BoardWipesV1,
                TriggerCondition::SelfEntersBattlefield => Capability::SelfEntersBattlefieldV1,
                TriggerCondition::OtherCreatureEntersBattlefield => {
                    Capability::OtherCreatureEntersBattlefieldV1
                }
                TriggerCondition::AnyCreatureDies => Capability::CreatureDiesV1,
                TriggerCondition::SelfDies => Capability::SelfDiesV1,
                TriggerCondition::SelfAuraToGraveyard => Capability::AttachmentsV1,
            });
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
            set.insert(match keyword {
                KeywordAbility::Flash => Capability::FlashV1,
                KeywordAbility::Defender => Capability::DefenderV1,
                KeywordAbility::Flying => Capability::FlyingV1,
                KeywordAbility::Reach => Capability::ReachV1,
                KeywordAbility::Vigilance => Capability::VigilanceV1,
                KeywordAbility::Haste => Capability::HasteV1,
                KeywordAbility::Lifelink => Capability::LifelinkV1,
                KeywordAbility::Deathtouch => Capability::DeathtouchV1,
                KeywordAbility::Trample => Capability::TrampleV1,
                KeywordAbility::FirstStrike => Capability::FirstStrikeV1,
                KeywordAbility::DoubleStrike => Capability::DoubleStrikeV1,
                KeywordAbility::Menace => Capability::MenaceV1,
                KeywordAbility::Hexproof | KeywordAbility::Shroud => Capability::AttachmentsV1,
            });
        }
        for a in &self.static_abilities {
            set.insert(match a.effect {
                StaticEffect::OpponentsCantCastDuringControllerTurn => {
                    Capability::OpponentCastProhibitionV1
                }
                StaticEffect::EntersTapped => Capability::EntersTappedV1,
                StaticEffect::ReduceGenericByCreatureCount | StaticEffect::CantBeCountered => {
                    Capability::BoardWipesV1
                }
                StaticEffect::DoubleDamageToPermanentOrPlayer => {
                    Capability::StaticDamageReplacementV1
                }
                StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer => {
                    Capability::ControlledCreatureDamageReplacementV1
                }
                StaticEffect::ControllerCreatureAnthem { .. } => Capability::CreatureAnthemV1,
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::AttackingController,
                    ..
                } => Capability::AttackingCreatureKeywordsV1,
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::Source,
                    ..
                } => Capability::SacrificeV1,
                StaticEffect::ModifyCreatures { .. } => Capability::AttachmentsV1,
            });
        }
        set
    }
    pub fn validate(&self) -> Result<(), String> {
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
        if self.activated.iter().any(|a| has_untap(&a.program))
            || self.triggered.iter().any(|a| has_untap(&a.program))
            || self.spell.as_ref().is_some_and(|p| {
                has_untap(p)
                    && (!matches!(
                        self.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    ) || !p.modes.is_empty())
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
        // Sacrifice programs admit enchantment programs and creature statics.
        // Other programs retain their narrower kind constraints.
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
                p.instructions.iter().any(|i| {
                    matches!(
                        i,
                        Instruction::DamageSet {
                            amount: WipeAmount::AnnouncedX,
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
            if unsupported_rules_subtype(subtype) {
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
        if self.keywords.iter().any(|k| *k != KeywordAbility::Flash) && c.kind != CardKind::Creature
        {
            return Err("keywords require creature characteristics".into());
        }
        if self.keywords.contains(&KeywordAbility::Flash) && c.kind == CardKind::Land {
            return Err("flash on lands is outside supported casting semantics".into());
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
        if !accepted_provenance(
            &self.provenance.archive_manifest_sha256,
            &self.provenance.parser_sha256,
        ) || sha256(self.provenance.source_json.as_bytes()) != self.provenance.source_sha256
            || sha256(format!("[{}]", self.provenance.abilities_json).as_bytes())
                != self.provenance.ast_sha256
        {
            return Err("invalid source/AST provenance".into());
        }
        if self.mana_abilities.len() > 1 {
            return Err("finite profile supports one mana ability per source".into());
        }
        for a in &self.mana_abilities {
            if !matches!(
                c.kind,
                CardKind::Creature | CardKind::Artifact | CardKind::Land
            ) || !a.costs.tap
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
        }
        if !self.activated.is_empty()
            && !matches!(c.kind, CardKind::Creature | CardKind::Artifact)
            && !(sacrifice && c.kind == CardKind::Enchantment)
        {
            return Err("activated programs require supported creature characteristics".into());
        }
        for ability in &self.activated {
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
            if let ActivationCost::SacrificePermanent { filter, mana } = &ability.cost {
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
            && !matches!(
                c.kind,
                CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
            )
        {
            return Err("trigger programs require a supported permanent source".into());
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
                spell.validate()?;
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
                || p.additional_sacrifice.is_some()
                || p.functioning_zone != FunctioningZone::Stack
                || p.timing != SpellTiming::Instant
                || p.resolution != SpellResolution::Instructions
                || p.instructions.is_empty()
                || !p.modes.is_empty()
                || (ability.intervening_if.is_some()
                    && ability.condition != TriggerCondition::SelfDies
                    && !matches!(ability.condition, TriggerCondition::StepBegan { .. }))
                || (matches!(
                    ability.intervening_if,
                    Some(InterveningCondition::SourceStatus { .. })
                ) && !matches!(ability.condition, TriggerCondition::StepBegan { .. }))
                || p.targets.iter().any(|target| {
                    !matches!(
                        target.domain,
                        TargetDomain::AnyPlayer
                            | TargetDomain::AnyDamageTarget
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                    )
                })
                || p.instructions.iter().any(|instruction| {
                    !matches!(
                        instruction,
                        Instruction::PumpSet { .. }
                            | Instruction::LoseLifeDeathController { .. }
                            | Instruction::SacrificeDamage { .. }
                            | Instruction::EachOpponentSacrifice { .. }
                            | Instruction::ReturnOwnGraveyardCreatureToHand { .. }
                            | Instruction::DestroyPermanent { .. }
                            | Instruction::CreateCreatureTokens { .. }
                            | Instruction::PutPlusOneCounters { .. }
                            | Instruction::DrawCards { .. }
                            | Instruction::OptionalDrawCards { .. }
                            | Instruction::DrawDamageAmount { .. }
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
            a.effect != StaticEffect::EntersTapped
                && a.effect != StaticEffect::OpponentsCantCastDuringControllerTurn
                && a.effect != StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
                && !matches!(
                    a.effect,
                    StaticEffect::ModifyCreatures {
                        scope: CreatureScope::AttackingController,
                        ..
                    }
                )
        }) && c.kind != CardKind::Enchantment
            && !equipment
            && !(sacrifice && c.kind == CardKind::Creature)
            && !(wipes && matches!(c.kind, CardKind::Instant | CardKind::Sorcery))
        {
            return Err("static damage replacement requires a supported enchantment".into());
        }
        if self.static_abilities.iter().any(|a| {
            if matches!(
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
            if a.effect == StaticEffect::EntersTapped {
                if matches!(c.kind, CardKind::Instant | CardKind::Sorcery) {
                    return Err("entry replacement requires a permanent".into());
                }
                continue;
            }
            if let StaticEffect::ModifyCreatures {
                scope,
                power,
                toughness,
                keywords,
                restrictions,
            } = &a.effect
            {
                if power.is_negative()
                    || toughness.is_negative()
                    || (power == &Quantity::zero()
                        && toughness == &Quantity::zero()
                        && keywords.is_empty()
                        && restrictions.is_empty())
                    || (*scope == CreatureScope::Source
                        && (c.kind != CardKind::Creature
                            || *power != Quantity::zero()
                            || *toughness != Quantity::zero()
                            || !keywords.is_empty()
                            || restrictions != &[CreatureRestriction::CantBlock]))
                    || (*scope == CreatureScope::AttackingController
                        && (!matches!(
                            c.kind,
                            CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
                        ) || *power != Quantity::zero()
                            || *toughness != Quantity::zero()
                            || keywords != &[KeywordAbility::Deathtouch]
                            || !restrictions.is_empty()))
                    || (*scope == CreatureScope::Equipped && !equipment)
                    || (*scope == CreatureScope::Enchanted && !aura)
                    || (*scope == CreatureScope::Controller
                        && (c.kind != CardKind::Enchantment || aura))
                    || keywords.contains(&KeywordAbility::Flash)
                    || keywords
                        .iter()
                        .enumerate()
                        .any(|(n, k)| keywords[..n].contains(k))
                    || restrictions
                        .iter()
                        .enumerate()
                        .any(|(n, k)| restrictions[..n].contains(k))
                {
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
                    || if aura {
                        !matches!(p.targets.as_slice(), [t] if t.domain == TargetDomain::BattlefieldCreature && t.slot == 0 && !t.selection_id.is_empty() && t.chooser == Chooser::Controller && t.stage == EvaluationStage::Announcement)
                    } else {
                        !p.targets.is_empty()
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
                    !c.subtypes.is_empty() || self.mana_abilities.is_empty()
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
                    || c.toughness.as_ref().is_none_or(|n| n <= &Quantity::zero())
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
                    || p.resolution != SpellResolution::Instructions
                    || (p.instructions.is_empty() && p.modes.is_empty())
                {
                    return Err("invalid spell functioning zone or empty program".into());
                }
                validate_instruction_program(p)?;
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
    p.additional_sacrifice.is_none()
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
    p.additional_sacrifice.is_none()
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
fn tap_source_destroy_ability(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.cost == ActivationCost::TapSource
        && a.functioning_zone == FunctioningZone::Battlefield
        && p.timing == SpellTiming::Instant
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && p.additional_sacrifice.is_none()
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
        && p.additional_sacrifice.is_none()
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
        && p.additional_sacrifice.is_none()
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
                && !f.tapped
        })
}
fn validate_instruction_program(p: &SpellProgram) -> Result<(), String> {
    if has_permanent_owner_hand(p)
        && (p.additional_sacrifice.is_some()
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
    if has_untap(p)
        && (p.additional_sacrifice.is_some()
            || !p.modes.is_empty()
            || p.functioning_zone != FunctioningZone::Stack
            || p.resolution != SpellResolution::Instructions
            || p.targets.len() != 1
            || p.targets[0].domain != TargetDomain::BattlefieldPermanent
            || p.targets[0].filter.as_ref().is_none_or(|f| {
                !f.types.is_empty()
                    || !f.non_types.is_empty()
                    || !f.non_colors.is_empty()
                    || f.opponent
                    || f.tapped
            })
            || !matches!(
                p.instructions.as_slice(),
                [Instruction::UntapPermanent { target_slot: 0, .. }]
            ))
    {
        return Err("untap requires one unqualified permanent target and one instruction".into());
    }
    if p.targets
        .iter()
        .any(|t| t.domain == TargetDomain::BlockingBattlefieldCreature)
        && (p.additional_sacrifice.is_some()
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
            || p.resolution != SpellResolution::Instructions
        {
            return Err("modal root must choose exactly one of multiple complete programs".into());
        }
        for mode in &p.modes {
            let child = &mode.program;
            if !child.modes.is_empty()
                || child.timing != p.timing
                || child.functioning_zone != p.functioning_zone
                || child.resolution != p.resolution
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
        return Err("mixed target domains are unsupported".into());
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
                TargetDomain::BattlefieldPermanent | TargetDomain::StackSpell
            ) || (t.domain == TargetDomain::StackSpell
                && (f.types.is_empty()
                    || !f.non_types.is_empty()
                    || !f.non_colors.is_empty()
                    || f.opponent
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
        } else if t.domain == TargetDomain::BattlefieldPermanent {
            return Err("permanent target requires explicit filter".into());
        }
    }
    let mut used = BTreeSet::new();
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
            Instruction::MoveSet { .. }
            | Instruction::PumpSet { .. }
            | Instruction::EachPlayerSacrifice { .. }
            | Instruction::OptionalDrawCards { .. }
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
            Instruction::ExilePermanent { target_slot, .. }
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
            Instruction::ReturnOwnGraveyardCreatureToHand { target_slot, .. } => {
                Some((*target_slot, TargetDomain::OwnGraveyardCreatureCard))
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
            Instruction::ModifyPowerToughness { target_slot, .. } => {
                let domain = p
                    .targets
                    .get(*target_slot as usize)
                    .map(|target| target.domain)
                    .ok_or("unbound creature pump target")?;
                if !matches!(
                    domain,
                    TargetDomain::BattlefieldCreature | TargetDomain::BlockingBattlefieldCreature
                ) {
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
                    TargetDomain::BattlefieldCreature | TargetDomain::BattlefieldPermanent
                ) {
                    return Err("unsupported owner-hand target domain".into());
                }
                Some((*target_slot, domain))
            }
            Instruction::PutOnTopOwnerLibrary { target_slot, .. } => {
                Some((*target_slot, TargetDomain::BattlefieldCreature))
            }
            Instruction::SacrificeCreature { recipient, .. } => {
                if *recipient != ActionRecipient::EventPlayer {
                    return Err("controller sacrifice is outside supported action semantics".into());
                }
                None
            }
            Instruction::TargetPlayerSacrificeCreature { target_slot, .. } => {
                Some((*target_slot, TargetDomain::OpponentPlayer))
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
            Instruction::DealDamage {
                target_slot,
                amount,
                ..
            } => {
                if *amount <= Quantity::zero() {
                    return Err("damage quantity must be positive".into());
                }
                Some((*target_slot, TargetDomain::AnyDamageTarget))
            }
            Instruction::GainLife {
                recipient, amount, ..
            }
            | Instruction::LoseLife {
                recipient, amount, ..
            } => {
                if *amount <= Quantity::zero() {
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
            Instruction::DrawCards {
                count, recipient, ..
            } => {
                if *count != Quantity::from(1) {
                    return Err("unsupported draw cardinality".into());
                }
                match recipient {
                    Some(LifeRecipient::EachOpponent) => {
                        return Err("unsupported each-opponent draw".into());
                    }
                    Some(LifeRecipient::Target { target_slot }) => {
                        Some((*target_slot, TargetDomain::AnyPlayer))
                    }
                    _ => None,
                }
            }
            Instruction::DiscardCards { count, .. } => {
                if *count != Quantity::from(1) {
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
fn add_program_requirements(set: &mut BTreeSet<Capability>, program: &SpellProgram) {
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
        for mode in &program.modes {
            add_program_requirements(set, &mode.program);
        }
    }
    for target in &program.targets {
        if target.filter.is_some() {
            set.insert(Capability::PermanentRemovalV1);
        }
        if target.filter.as_ref().is_some_and(|filter| filter.tapped) {
            set.insert(Capability::TappedPermanentTargetV1);
        }
        set.insert(match target.domain {
            TargetDomain::BattlefieldPermanent => Capability::PermanentRemovalV1,
            TargetDomain::BattlefieldCreature => Capability::CreatureTargetV1,
            TargetDomain::BlockingBattlefieldCreature => Capability::BlockingCreatureTargetV1,
            TargetDomain::ControlledBattlefieldCreature => Capability::AttachmentsV1,
            TargetDomain::OwnGraveyardCreatureCard => Capability::GraveyardRecursionV1,
            TargetDomain::OpponentPlayer => Capability::OpponentTargetV1,
            TargetDomain::StackSpell => Capability::StackSpellTargetV1,
            TargetDomain::StackActivatedOrTriggeredAbility => Capability::StackAbilityTargetV1,
            TargetDomain::AnyDamageTarget => Capability::AnyDamageTargetV1,
            TargetDomain::AnyPlayer => Capability::AnyPlayerTargetV1,
        });
    }
    for instruction in &program.instructions {
        if instruction.uses_event_player() {
            set.insert(Capability::EventPlayerRecipientV1);
        }
        if let Instruction::CreateCreatureTokens { token, .. } = instruction
            && token
                .keywords
                .iter()
                .any(|k| matches!(k, KeywordAbility::Hexproof | KeywordAbility::Shroud))
        {
            set.insert(Capability::AttachmentsV1);
        }
        set.insert(match instruction {
            Instruction::MoveSet { .. }
            | Instruction::PumpSet { .. }
            | Instruction::DamageSet { .. }
            | Instruction::GainLifeDestroyed { .. }
            | Instruction::ExileDamagedThisTurn { .. }
            | Instruction::EachPlayerSacrifice { .. }
            | Instruction::LoseLifeDeathController { .. } => Capability::BoardWipesV1,
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
            Instruction::ReturnOwnGraveyardCreatureToHand { .. } => {
                Capability::GraveyardRecursionV1
            }
            Instruction::SearchBasicLands { .. } => Capability::BasicLandSearchV1,
            Instruction::DrawCards {
                recipient: Some(_), ..
            } => Capability::TargetPlayerDrawV1,
            Instruction::CreateCreatureTokens { .. } => Capability::CreatureTokensV1,
            Instruction::PutPlusOneCounters { .. } => Capability::PlusOneCountersV1,
            Instruction::ModifyPowerToughness { .. } => Capability::TemporaryPowerToughnessV1,
            Instruction::PreventCombatDamage { .. } => Capability::CombatDamagePreventionV1,
            Instruction::PreventNextDamage { .. } => Capability::DamageBudgetPreventionV1,
            Instruction::ReturnToOwnerHand { .. } => Capability::OwnerHandMoveV1,
            Instruction::PutOnTopOwnerLibrary { .. } => Capability::OwnerLibraryTopMoveV1,
            Instruction::TargetPlayerSacrificeCreature { .. } => {
                Capability::TargetPlayerSacrificeCreatureV1
            }
            Instruction::SacrificeCreature { .. } => Capability::EventPlayerSacrificeV1,
            Instruction::DrawCards { .. } => Capability::ControllerDrawV1,
            Instruction::OptionalDrawCards { .. } => Capability::OptionalControllerDrawV1,
            Instruction::DrawDamageAmount { .. } => Capability::DamageQuantityDrawV1,
            Instruction::UntapPermanent { .. } => Capability::UntapPermanentV1,
            Instruction::TapPermanent { .. } => Capability::TapPermanentV1,
            Instruction::DiscardCards {
                recipient: Some(ActionRecipient::EventPlayer),
                ..
            } => Capability::EventPlayerDiscardV1,
            Instruction::DiscardCards { .. } => Capability::ControllerDiscardV1,
            Instruction::CounterStackObject { .. } => Capability::CounterStackObjectV1,
            Instruction::DealDamage { .. } => Capability::NoncombatDamageV1,
            Instruction::GainLife { .. } => Capability::LifeGainV1,
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
        let mut ids = BTreeSet::new();
        for d in &self.definitions {
            d.validate()?;
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
pub fn catalog_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Catalog)).expect("schema is JSON");
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
            "resolution":{"const":"instructions"},
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
    // The untap opcode is a complete ordinary spell, not a capability that
    // implicitly admits activations, triggers or modal child programs.
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
                "filter":{"type":"object","properties":{"types":{"maxItems":0},"non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}}}
            }}}
        }}
    }));
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "properties":{"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/UntapFreeProgram"}}}}}
    }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","required":["instructions"],"properties":untap}}},
        "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"capabilities":{"contains":{"const":Capability::UntapPermanentV1}}}}
    }));
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"properties":{
            "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/UntapFreeProgram"}}}},
            "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/UntapFreeProgram"}}}}
        }}));
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
mod activated_spell_counter_tests;

#[cfg(test)]
mod activated_permanent_return_tests;

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
            NormalLayoutV1,
            BasicLandManaV1,
            LandPlayV1,
            BaseCreatureV1,
            CommanderIdentityV1,
            CastInstantV1,
            CastCreatureV1,
            CreatureTargetV1,
            BlockingCreatureTargetV1,
            TappedPermanentTargetV1,
            UntapPermanentV1,
            TapPermanentV1,
            ActivatedTapPermanentV1,
            TapSourceDestroyV1,
            ActivatedSpellCounterV1,
            ActivatedPermanentOwnerHandV1,
            OwnerHandMoveV1,
            PermanentOwnerHandMoveV1,
            CastSorceryV1,
            OwnerLibraryTopMoveV1,
            ActivatedAbilityV1,
            SacrificeSourceCostV1,
            ControllerDrawV1,
            ControllerDiscardV1,
            OpponentTargetV1,
            TargetPlayerSacrificeCreatureV1,
            StackSpellTargetV1,
            StackAbilityTargetV1,
            CounterStackObjectV1,
            TapSourceCostV1,
            ManaActivationCostV1,
            AnyDamageTargetV1,
            AnyPlayerTargetV1,
            NoncombatDamageV1,
            LifeGainV1,
            LifeLossV1,
            TriggeredAbilityV1,
            SelfEntersBattlefieldV1,
            OtherCreatureEntersBattlefieldV1,
            CreatureDiesV1,
            CastEnchantmentV1,
            TemporaryPowerToughnessV1,
            CombatDamagePreventionV1,
            DamageBudgetPreventionV1,
            StaticDamageReplacementV1,
            SelfDiesV1,
            InterveningIfV1,
            ModalChoiceV1,
            DefenderV1,
            FlyingV1,
            ReachV1,
            VigilanceV1,
            HasteV1,
            LifelinkV1,
            DeathtouchV1,
            TrampleV1,
            FirstStrikeV1,
            DoubleStrikeV1,
            MenaceV1,
            CreatureTokensV1,
            PlusOneCountersV1,
            DestroyCreatureV1,
            GraveyardRecursionV1,
            BasicLandSearchV1,
            CreatureAnthemV1,
            TargetPlayerDrawV1,
            CastArtifactV1,
            PermanentManaV1,
            CompositeActivationCostV1,
            AttachmentsV1,
            PermanentRemovalV1,
            SacrificeV1,
            BoardWipesV1,
            FlashV1,
            EntersTappedV1,
            StepBeginningTriggerV1,
            EventPlayerRecipientV1,
            SourceStatusConditionV1,
            CastTriggerV1,
            AttackTriggerV1,
            DamageTriggerV1,
            SourceDamageTriggerV2,
            OptionalControllerDrawV1,
            EventPlayerDiscardV1,
            EventPlayerSacrificeV1,
            FilteredDamageTriggerV1,
            DamageControllerDiscardV1,
            DamageQuantityDrawV1,
            SnowSupertypeV1,
            AttackingCreatureKeywordsV1,
            OpponentCastProhibitionV1,
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
        definitions.push(untap_tests::untap());
        definitions.push(tap_tests::tap());
        definitions.push(activated_permanent_tap_tests::activated_tap());
        definitions.push(tap_source_destroy_tests::tap_source_destroy());
        definitions.push(activated_spell_counter_tests::activated_counter());
        definitions.push(activated_permanent_return_tests::activated_return());
        definitions.push(permanent_owner_hand_tests::permanent_owner_hand());
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
            assert!(d.validate().unwrap_err().contains("non-mana creature"));
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
        // Unqualified Controller scope requires an enchantment context; the
        // attacking scope does not widen it on creatures.
        unbound.static_abilities = vec![grant];
        if let StaticEffect::ModifyCreatures { scope, .. } = &mut unbound.static_abilities[0].effect
        {
            *scope = CreatureScope::Controller;
        }
        unbound.capabilities = unbound.requirements();
        assert!(unbound.validate().is_err());
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

//! Versioned executable contracts. No rules execution or parser dependency.
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
    pub mana_cost: ManaCost,
    pub colors: Vec<Color>,
    pub color_identity: Vec<Color>,
    pub power: Option<Quantity>,
    pub toughness: Option<Quantity>,
}
impl Characteristics {
    pub fn has_type(&self, kind: CardKind) -> bool {
        self.kind == kind || self.additional_types.contains(&kind)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    CastArtifactV1,
    PermanentManaV1,
    CompositeActivationCostV1,
    CastEnchantmentV1,
    TemporaryPowerToughnessV1,
    CombatDamagePreventionV1,
    DamageBudgetPreventionV1,
    StaticDamageReplacementV1,
    NormalLayoutV1,
    BasicLandManaV1,
    LandPlayV1,
    BaseCreatureV1,
    CommanderIdentityV1,
    CastInstantV1,
    CastCreatureV1,
    CreatureTargetV1,
    OwnerHandMoveV1,
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
/// alternatives; exclusions and the controller restriction are intersections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermanentFilter {
    pub types: Vec<CardKind>,
    pub non_types: Vec<CardKind>,
    pub non_colors: Vec<Color>,
    pub opponent: bool,
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
    EachOpponent,
    Controller,
    Target { target_slot: u32 },
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
        if self.colors.contains(&Color::C)
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
    DrawCards {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipient: Option<LifeRecipient>,
        count: Quantity,
        source_block: u32,
    },
    DiscardCards {
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TriggerCondition {
    OpponentCreatureDies,
    SelfEntersBattlefield,
    /// Excludes the exact source incarnation, never every copy of its card.
    OtherCreatureEntersBattlefield,
    /// The disjoint union of this creature and all other creatures dying.
    AnyCreatureDies,
    SelfDies,
    SelfAuraToGraveyard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterveningCondition {
    ControllerHandEmpty,
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
                CardKind::Artifact => {
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
            set.insert(match ability.condition {
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
            add_program_requirements(&mut set, &ability.program);
        }
        for keyword in &self.keywords {
            set.insert(match keyword {
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
                StaticEffect::ReduceGenericByCreatureCount | StaticEffect::CantBeCountered => {
                    Capability::BoardWipesV1
                }
                StaticEffect::DoubleDamageToPermanentOrPlayer => {
                    Capability::StaticDamageReplacementV1
                }
                StaticEffect::ControllerCreatureAnthem { .. } => Capability::CreatureAnthemV1,
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
                && a.death_filter.is_none()
        }) {
            return Err("tokens/counters require spells or self-enters triggers".into());
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
        // v19 alone admits enchantment programs and creature statics. Older
        // catalogs reject this capability, retaining their original kind checks.
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
        let equipment = c.kind == CardKind::Artifact && c.subtypes == ["Equipment"];
        let aura = c.kind == CardKind::Enchantment && c.subtypes == ["Aura"];
        if !self.keywords.is_empty() && c.kind != CardKind::Creature {
            return Err("keywords require creature characteristics".into());
        }
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
                    p.instructions.iter().any(|instruction| !matches!(instruction,
                        Instruction::DrawCards {count,..}|Instruction::DiscardCards {count,..} if *count==Quantity::from(1)))
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
            && c.kind != CardKind::Creature
            && !aura
            && !(sacrifice && c.kind == CardKind::Enchantment)
        {
            return Err("trigger programs require supported creature characteristics".into());
        }
        for ability in &self.triggered {
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
            {
                return Err("targeted removal triggers require self-enters".into());
            }
            if ability.condition == TriggerCondition::SelfAuraToGraveyard {
                return Err("Aura trigger requires Aura".into());
            }

            if ability.functioning_zone != FunctioningZone::Battlefield
                || p.functioning_zone != FunctioningZone::Stack
                || p.timing != SpellTiming::Instant
                || p.resolution != SpellResolution::Instructions
                || p.instructions.is_empty()
                || !p.modes.is_empty()
                || (ability.intervening_if.is_some()
                    && ability.condition != TriggerCondition::SelfDies)
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
        if !self.static_abilities.is_empty()
            && c.kind != CardKind::Enchantment
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
                    || (*scope == CreatureScope::Equipped && !equipment)
                    || (*scope == CreatureScope::Enchanted && !aura)
                    || (*scope == CreatureScope::Controller
                        && (c.kind != CardKind::Enchantment || aura))
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
                    || p.timing != SpellTiming::MainPhase
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
                    || p.timing != SpellTiming::MainPhase
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
                    || p.timing
                        != if c.kind == CardKind::Instant {
                            SpellTiming::Instant
                        } else {
                            SpellTiming::MainPhase
                        }
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
fn validate_instruction_program(p: &SpellProgram) -> Result<(), String> {
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
    for t in &p.targets {
        if let Some(f) = &t.filter {
            if !matches!(
                t.domain,
                TargetDomain::BattlefieldPermanent | TargetDomain::StackSpell
            ) || (t.domain == TargetDomain::StackSpell
                && (f.types.is_empty()
                    || !f.non_types.is_empty()
                    || !f.non_colors.is_empty()
                    || f.opponent))
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
            | Instruction::EachPlayerSacrifice { .. } => None,
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
                    || recipient
                        .as_ref()
                        .is_some_and(|r| matches!(r, LifeRecipient::Target { .. }))
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
            Instruction::ExilePermanent { target_slot, .. } => {
                Some((*target_slot, TargetDomain::BattlefieldPermanent))
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
                Some((*target_slot, TargetDomain::BattlefieldCreature))
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
            Instruction::ReturnToOwnerHand { target_slot, .. }
            | Instruction::PutOnTopOwnerLibrary { target_slot, .. } => {
                Some((*target_slot, TargetDomain::BattlefieldCreature))
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
                    LifeRecipient::Controller | LifeRecipient::EachOpponent => None,
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
fn add_program_requirements(set: &mut BTreeSet<Capability>, program: &SpellProgram) {
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
        set.insert(match target.domain {
            TargetDomain::BattlefieldPermanent => Capability::PermanentRemovalV1,
            TargetDomain::BattlefieldCreature => Capability::CreatureTargetV1,
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
            Instruction::DrawCards { .. } => Capability::ControllerDrawV1,
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
    #[schemars(range(min = 3, max = 20))]
    pub schema_version: u32,
    pub rules_edition: String,
    pub rules_sha256: String,
    pub definitions: Vec<Definition>,
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.schema_version, 3..=20)
            || self.rules_edition != RULES_EDITION
            || self.rules_sha256 != RULES_SHA256
        {
            return Err("unsupported IR version or rules snapshot".into());
        }
        let mut ids = BTreeSet::new();
        for d in &self.definitions {
            d.validate()?;
            if self.schema_version < 20 && d.capabilities.contains(&Capability::BoardWipesV1) {
                return Err("IR v20 is required for board wipes and X".into());
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
            if self.schema_version < 19 && d.capabilities.contains(&Capability::SacrificeV1) {
                return Err("IR v19 is required for sacrifice and death payoffs".into());
            }
            if self.schema_version < 18 && d.capabilities.contains(&Capability::PermanentRemovalV1)
            {
                return Err("IR v18 is required for permanent removal and LKI".into());
            }
            if self.schema_version < 17 && d.capabilities.contains(&Capability::AttachmentsV1) {
                return Err("IR v17 is required for attachments and targeting keywords".into());
            }
            if self.schema_version < 16
                && d.capabilities.iter().any(|c| {
                    matches!(
                        c,
                        Capability::CastArtifactV1
                            | Capability::PermanentManaV1
                            | Capability::CompositeActivationCostV1
                    )
                })
            {
                return Err("IR v16 is required for permanent mana and artifacts".into());
            }
            if self.schema_version < 15
                && d.capabilities.iter().any(|c| {
                    matches!(
                        c,
                        Capability::DestroyCreatureV1
                            | Capability::GraveyardRecursionV1
                            | Capability::BasicLandSearchV1
                            | Capability::CreatureAnthemV1
                            | Capability::TargetPlayerDrawV1
                    )
                })
            {
                return Err("IR v15 is required for Commander staples".into());
            }
            if self.schema_version < 14
                && d.capabilities.iter().any(|c| {
                    matches!(
                        c,
                        Capability::CreatureTokensV1 | Capability::PlusOneCountersV1
                    )
                })
            {
                return Err("IR v14 is required for creature tokens and counters".into());
            }
            if self.schema_version < 13
                && d.keywords.iter().any(|k| {
                    matches!(
                        k,
                        KeywordAbility::Trample
                            | KeywordAbility::FirstStrike
                            | KeywordAbility::DoubleStrike
                            | KeywordAbility::Menace
                    )
                })
            {
                return Err("IR v13 is required for combat damage keywords and menace".into());
            }
            if self.schema_version < 12 && d.keywords.iter().any(|k| *k != KeywordAbility::Defender)
            {
                return Err("IR v12 is required for creature keywords beyond defender".into());
            }
            if self.schema_version < 11
                && (!d.keywords.is_empty() || d.capabilities.contains(&Capability::DefenderV1))
            {
                return Err("IR v11 is required for defender".into());
            }
            if self.schema_version < 10
                && d.capabilities.iter().any(|c| {
                    matches!(
                        c,
                        Capability::ModalChoiceV1
                            | Capability::SelfDiesV1
                            | Capability::InterveningIfV1
                    )
                })
                || self.schema_version < 10
                    && d.triggered.iter().any(|a| {
                        a.program
                            .targets
                            .iter()
                            .any(|t| t.domain == TargetDomain::BattlefieldCreature)
                    })
            {
                return Err(
                    "IR v10 is required for modal choices and intervening-if dies triggers".into(),
                );
            }
            if self.schema_version < 9
                && d.capabilities.iter().any(|c| {
                    matches!(
                        c,
                        Capability::CastEnchantmentV1
                            | Capability::TemporaryPowerToughnessV1
                            | Capability::CombatDamagePreventionV1
                            | Capability::DamageBudgetPreventionV1
                            | Capability::StaticDamageReplacementV1
                    )
                })
            {
                return Err(
                    "IR v9 is required for temporary effects, prevention and replacements".into(),
                );
            }
            if self.schema_version < 8 && !d.triggered.is_empty() {
                return Err("IR v8 is required for ordinary triggered abilities".into());
            }
            if self.schema_version < 7
                && (d.capabilities.iter().any(|cap| {
                    matches!(
                        cap,
                        Capability::AnyDamageTargetV1
                            | Capability::AnyPlayerTargetV1
                            | Capability::NoncombatDamageV1
                            | Capability::LifeGainV1
                            | Capability::LifeLossV1
                    )
                }) || d
                    .activated
                    .iter()
                    .any(|ability| !ability.program.targets.is_empty())
                    || d.spell.as_ref().is_some_and(|program| {
                        program.resolution == SpellResolution::Instructions
                            && (program.targets.is_empty()
                                || program.instructions.iter().any(|instruction| {
                                    matches!(
                                        instruction,
                                        Instruction::DrawCards { .. }
                                            | Instruction::DiscardCards { .. }
                                    )
                                }))
                    }))
            {
                return Err("IR v7 is required for damage/life, targeted activations or untargeted spell instructions".into());
            }
            if self.schema_version < 6
                && (d
                    .activated
                    .iter()
                    .any(|ability| ability.cost != ActivationCost::SacrificeSource)
                    || d.spell.as_ref().is_some_and(|program| {
                        program.targets.iter().any(|target| {
                            matches!(
                                target.domain,
                                TargetDomain::StackSpell
                                    | TargetDomain::StackActivatedOrTriggeredAbility
                            )
                        }) || program.instructions.iter().any(|instruction| {
                            matches!(instruction, Instruction::CounterStackObject { .. })
                        })
                    }))
            {
                return Err(
                    "IR v6 is required for stack targets or tap/mana activation costs".into(),
                );
            }
            if self.schema_version < 5
                && (!d.activated.is_empty()
                    || d.spell.as_ref().is_some_and(|program| {
                        program
                            .targets
                            .iter()
                            .any(|target| target.domain == TargetDomain::OpponentPlayer)
                            || program.instructions.iter().any(|instruction| {
                                matches!(
                                    instruction,
                                    Instruction::TargetPlayerSacrificeCreature { .. }
                                        | Instruction::DrawCards { .. }
                                        | Instruction::DiscardCards { .. }
                                )
                            })
                    }))
            {
                return Err("IR v5 is required for activated or opponent-choice programs".into());
            }
            if self.schema_version == 3
                && (d.characteristics.kind == CardKind::Sorcery
                    || d.spell.as_ref().is_some_and(|program| {
                        program.instructions.iter().any(|instruction| {
                            matches!(instruction, Instruction::PutOnTopOwnerLibrary { .. })
                        })
                    }))
            {
                return Err("IR v3 does not support sorcery/library-top semantics".into());
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
/// IR v3/v4/v5/v6/v7/v8/v9 canonicalization (same algorithm as v1/v2): UTF-8 compact serde_json Value (sorted object keys),
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
    // Catalog versions share unchanged definition shapes. Restrict the v3
    // vocabularies explicitly so schema validation cannot downgrade programs.
    // https://json-schema.org/understanding-json-schema/reference/conditionals
    schema["allOf"] = serde_json::json!([{
        "if": {"required": ["schema_version"], "properties": {"schema_version": {"const": 3}}},
        "then": {"properties": {"definitions": {"items": {"properties": {
            "characteristics": {"properties": {"kind": {"enum": [CardKind::Land, CardKind::Creature, CardKind::Instant]}}},
            "capabilities": {"items": {"enum": [Capability::NormalLayoutV1, Capability::BasicLandManaV1,
                Capability::LandPlayV1, Capability::BaseCreatureV1, Capability::CommanderIdentityV1,
                Capability::CastInstantV1, Capability::CastCreatureV1, Capability::CreatureTargetV1,
                Capability::OwnerHandMoveV1]}},
            "spell": {"anyOf": [{"type": "null"}, {"type": "object", "properties": {
                "instructions": {"items": {"properties": {"op": {"enum": ["return_to_owner_hand"]}}}}
            }}]}
        }}}}}
    }, {
        "if": {"required": ["schema_version"], "properties": {"schema_version": {"enum": [3,4]}}},
        "then": {"properties": {"definitions": {"items": {"properties": {
            "activated": {"maxItems": 0},
            "capabilities": {"items": {"enum": [Capability::NormalLayoutV1, Capability::BasicLandManaV1,
                Capability::LandPlayV1, Capability::BaseCreatureV1, Capability::CommanderIdentityV1,
                Capability::CastInstantV1, Capability::CastCreatureV1, Capability::CreatureTargetV1,
                Capability::OwnerHandMoveV1, Capability::CastSorceryV1, Capability::OwnerLibraryTopMoveV1]}},
            "spell": {"anyOf": [{"type":"null"}, {"type":"object", "properties": {
                "targets": {"items": {"properties": {"domain": {"const":"battlefield_creature"}}}},
                "instructions": {"items": {"properties": {"op": {"enum":["return_to_owner_hand","put_on_top_owner_library"]}}}}
            }}]}
        }}}}}
    }, {
        "if": {"required": ["schema_version"], "properties": {"schema_version": {"enum": [3,4,5]}}},
        "then": {"properties": {"definitions": {"items": {"properties": {
            "activated": {"items": {"properties": {"cost": {"const": "sacrifice_source"}}}},
            "capabilities": {"items": {"not": {"enum": [Capability::StackSpellTargetV1,
                Capability::StackAbilityTargetV1, Capability::CounterStackObjectV1,
                Capability::TapSourceCostV1, Capability::ManaActivationCostV1]}}},
            "spell": {"anyOf": [{"type":"null"}, {"type":"object", "properties": {
                "targets": {"items": {"properties": {"domain": {"enum": ["battlefield_creature", "opponent_player"]}}}},
                "instructions": {"items": {"properties": {"op": {"not": {"const": "counter_stack_object"}}}}}
            }}]}
        }}}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"enum":[3,4,5,6]}}},
        "then":{"properties":{"definitions":{"items":{
            "properties":{
                "capabilities":{"items":{"not":{"enum":[Capability::AnyDamageTargetV1,Capability::AnyPlayerTargetV1,Capability::NoncombatDamageV1,Capability::LifeGainV1,Capability::LifeLossV1]}}},
                "activated":{"items":{"properties":{"program":{"properties":{"targets":{"maxItems":0},"instructions":{"items":{"properties":{"op":{"enum":["draw_cards","discard_cards"]}}}}}}}}},
                "spell":{"anyOf":[{"type":"null"},{"type":"object","properties":{
                    "targets":{"items":{"properties":{"domain":{"enum":["battlefield_creature","opponent_player","stack_spell","stack_activated_or_triggered_ability"]}}}},
                    "instructions":{"items":{"properties":{"op":{"enum":["return_to_owner_hand","put_on_top_owner_library","target_player_sacrifice_creature","counter_stack_object"]}}}}
                }}]}
            },
            "allOf":[{"if":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}}}},
                "then":{"properties":{"spell":{"properties":{"targets":{"minItems":1}}}}}}]
        }}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"enum":[3,4,5,6,7]}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "triggered":{"maxItems":0},
            "capabilities":{"items":{"not":{"enum":[Capability::TriggeredAbilityV1,Capability::SelfEntersBattlefieldV1,Capability::OtherCreatureEntersBattlefieldV1,Capability::CreatureDiesV1]}}}
        }}}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"enum":[3,4,5,6,7,8]}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "characteristics":{"properties":{"kind":{"not":{"const":"enchantment"}}}},
            "static_abilities":{"maxItems":0},
            "capabilities":{"items":{"not":{"enum":[Capability::CastEnchantmentV1,Capability::TemporaryPowerToughnessV1,Capability::CombatDamagePreventionV1,Capability::DamageBudgetPreventionV1,Capability::StaticDamageReplacementV1]}}},
            "spell":{"anyOf":[{"type":"null"},{"type":"object","properties":{"instructions":{"items":{"properties":{"op":{"not":{"enum":["modify_power_toughness","prevent_combat_damage","prevent_next_damage"]}}}}}}}]},
            "activated":{"items":{"properties":{"program":{"properties":{"instructions":{"items":{"properties":{"op":{"not":{"enum":["modify_power_toughness","prevent_combat_damage","prevent_next_damage"]}}}}}}}}}},
            "triggered":{"items":{"properties":{"program":{"properties":{"instructions":{"items":{"properties":{"op":{"not":{"enum":["modify_power_toughness","prevent_combat_damage","prevent_next_damage"]}}}}}}}}}}
        }}}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"enum":[3,4,5,6,7,8,9]}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "capabilities":{"items":{"not":{"enum":[Capability::ModalChoiceV1,Capability::SelfDiesV1,Capability::InterveningIfV1]}}},
            "spell":{"anyOf":[{"type":"null"},{"type":"object","properties":{"modes":{"maxItems":0}}}]},
            "activated":{"items":{"properties":{"program":{"properties":{"modes":{"maxItems":0}}}}}},
            "triggered":{"items":{"properties":{
                "condition":{"not":{"const":"self_dies"}},"intervening_if":{"type":"null"},
                "program":{"properties":{"modes":{"maxItems":0},"targets":{"items":{"properties":{"domain":{"const":"any_player"}}}}}}
            }}}
        }}}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"enum":[3,4,5,6,7,8,9,10]}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "keywords":{"maxItems":0},"capabilities":{"items":{"not":{"const":Capability::DefenderV1}}}
        }}}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":11}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "keywords":{"items":{"const":"defender"}},
            "capabilities":{"items":{"not":{"enum":[Capability::FlyingV1,Capability::ReachV1,Capability::VigilanceV1,Capability::HasteV1,Capability::LifelinkV1,Capability::DeathtouchV1]}}}
        }}}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":12}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "keywords":{"items":{"not":{"enum":["trample","first strike","double strike","menace"]}}},
            "capabilities":{"items":{"not":{"enum":[Capability::TrampleV1,Capability::FirstStrikeV1,Capability::DoubleStrikeV1,Capability::MenaceV1]}}}
        }}}}}
    }]);
    schema["$defs"]["PreWipesProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"properties":{"op":{"not":{"enum":["move_set","pump_set","damage_set","gain_life_destroyed","exile_damaged_this_turn","each_player_sacrifice","lose_life_death_controller"]}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PreWipesProgram"}}}}
    }});
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"properties":{"schema_version":{"maximum":19}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "x_mana_cost":{"const":false},
            "capabilities":{"items":{"not":{"const":Capability::BoardWipesV1}}},
            "static_abilities":{"items":{"properties":{"effect":{"not":{"enum":["reduce_generic_by_creature_count","cant_be_countered"]}}}}},
            "spell":{"$ref":"#/$defs/PreWipesProgram"},
            "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/PreWipesProgram"}}}},
            "triggered":{"items":{"properties":{"condition":{"not":{"const":"opponent_creature_dies"}},"program":{"$ref":"#/$defs/PreWipesProgram"}}}}
        }}}}}
    }));
    schema["$defs"]["PreTokensProgram"] = serde_json::json!({"properties":{
        "instructions":{"items":{"properties":{"op":{"not":{"enum":["create_creature_tokens","put_plus_one_counters"]}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PreTokensProgram"}}}}
    }});
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":13}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "capabilities":{"items":{"not":{"enum":[Capability::CreatureTokensV1,Capability::PlusOneCountersV1]}}},
            "spell":{"$ref":"#/$defs/PreTokensProgram"},
            "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/PreTokensProgram"}}}},
            "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/PreTokensProgram"}}}}
        }}}}}
    }));
    schema["$defs"]["PreStaplesProgram"] = serde_json::json!({"properties":{
        "targets":{"items":{"properties":{"domain":{"not":{"const":"own_graveyard_creature_card"}}}}},
        "instructions":{"items":{"properties":{
            "op":{"not":{"enum":["destroy_target_creature","destroy_all_creatures","return_own_graveyard_creature_to_hand","search_basic_lands"]}}
        },"allOf":[{"if":{"properties":{"op":{"const":"draw_cards"}}},"then":{"properties":{"recipient":{"type":"null"}}}}]}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PreStaplesProgram"}}}}
    }});
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":14}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "capabilities":{"items":{"not":{"enum":[Capability::DestroyCreatureV1,Capability::GraveyardRecursionV1,Capability::BasicLandSearchV1,Capability::CreatureAnthemV1,Capability::TargetPlayerDrawV1]}}},
            "static_abilities":{"items":{"properties":{"effect":{"const":"double_damage_to_permanent_or_player"}}}},
            "spell":{"$ref":"#/$defs/PreStaplesProgram"},
            "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/PreStaplesProgram"}}}},
            "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/PreStaplesProgram"}}}}
        }}}}}
    }));
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":15}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "characteristics":{"properties":{"kind":{"not":{"const":"artifact"}}}},
            "mana_abilities":{"maxItems":0},
            "activated":{"items":{"properties":{"cost":{"not":{"type":"object","required":["composite"]}}}}},
            "capabilities":{"items":{"not":{"enum":[Capability::CastArtifactV1,Capability::PermanentManaV1,Capability::CompositeActivationCostV1]}}}
        }}}}}
    }));
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":16}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "capabilities":{"items":{"not":{"const":Capability::AttachmentsV1}}},
            "keywords":{"items":{"not":{"enum":["hexproof","shroud"]}}},
            "characteristics":{"properties":{"subtypes":{"items":{"not":{"enum":["Equipment","Aura"]}}}}},
            "static_abilities":{"items":{"properties":{"effect":{"not":{"type":"object","required":["modify_creatures"]}}}}}
        }}}}}
    }));
    schema["$defs"]["PreRemovalProgram"] = serde_json::json!({"properties":{
        "targets":{"items":{"properties":{"filter":{"not":{}},"domain":{"not":{"const":"battlefield_permanent"}}}}},
        "instructions":{"items":{"properties":{"op":{"not":{"enum":["destroy_permanent","exile_permanent","change_life_from_target","create_tokens_for","damage_target_controller"]}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PreRemovalProgram"}}}}
    }});
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":17}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "capabilities":{"items":{"not":{"const":Capability::PermanentRemovalV1}}},
            "spell":{"$ref":"#/$defs/PreRemovalProgram"},
            "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/PreRemovalProgram"}}}},
            "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/PreRemovalProgram"}}}}
        }}}}}
    }));
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":18}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "capabilities":{"items":{"not":{"const":Capability::SacrificeV1}}},
            "spell":{"$ref":"#/$defs/PreSacrificeProgram"},
            "triggered":{"items":{"allOf":[{"if":{"properties":{"condition":{"const":"self_dies"}}},"then":{"properties":{"program":{"properties":{"instructions":{"items":{"properties":{"op":{"not":{"const":"create_creature_tokens"}}}}}}}}}}],"properties":{"death_filter":{"type":"null"},"program":{"$ref":"#/$defs/PreSacrificeProgram"}}}},
            "activated":{"items":{"allOf":[{"if":{"properties":{"cost":{"const":"sacrifice_source"}}},"then":{"properties":{"program":{"properties":{"instructions":{"items":{"properties":{"op":{"not":{"const":"search_basic_lands"}}}}}}}}}}],"properties":{"cost":{"not":{"type":"object","required":["sacrifice_permanent"]}},"program":{"$ref":"#/$defs/PreSacrificeProgram"}}}}
        }}}}}
    }));
    schema["$defs"]["PreSacrificeProgram"] = serde_json::json!({"properties":{
        "additional_sacrifice":{"type":"null"},
        "instructions":{"items":{"properties":{"op":{"not":{"enum":["sacrifice_damage","each_opponent_sacrifice","modify_source_power_toughness"]}},"recipient":{"not":{"const":"each_opponent"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PreSacrificeProgram"}}}}
    }});
    schema["$defs"]["PreAttachmentsProgram"] = serde_json::json!({"properties":{
        "targets":{"items":{"properties":{"domain":{"not":{"const":"controlled_battlefield_creature"}}}}},
        "instructions":{"items":{"properties":{"op":{"not":{"enum":["attach_equipment","return_aura_from_graveyard"]}},"token":{"properties":{"keywords":{"items":{"not":{"enum":["hexproof","shroud"]}}}}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/PreAttachmentsProgram"}}}}
    }});
    schema["allOf"].as_array_mut().unwrap().push(serde_json::json!({
        "if":{"required":["schema_version"],"properties":{"schema_version":{"maximum":16}}},
        "then":{"properties":{"definitions":{"items":{"properties":{
            "spell":{"$ref":"#/$defs/PreAttachmentsProgram"},
            "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/PreAttachmentsProgram"}}}},
            "triggered":{"items":{"properties":{"condition":{"not":{"const":"self_aura_to_graveyard"}},"program":{"$ref":"#/$defs/PreAttachmentsProgram"}}}}
        }}}}}
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
mod tests {
    use super::*;
    #[test]
    fn every_capability_has_a_minimum_ir_version_witness() {
        use Capability::*;
        let groups: &[(u32, &[Capability])] = &[
            (
                3,
                &[
                    NormalLayoutV1,
                    BasicLandManaV1,
                    LandPlayV1,
                    BaseCreatureV1,
                    CommanderIdentityV1,
                    CastInstantV1,
                    CastCreatureV1,
                    CreatureTargetV1,
                    OwnerHandMoveV1,
                ],
            ),
            (4, &[CastSorceryV1, OwnerLibraryTopMoveV1]),
            (
                5,
                &[
                    ActivatedAbilityV1,
                    SacrificeSourceCostV1,
                    ControllerDrawV1,
                    ControllerDiscardV1,
                    OpponentTargetV1,
                    TargetPlayerSacrificeCreatureV1,
                ],
            ),
            (
                6,
                &[
                    StackSpellTargetV1,
                    StackAbilityTargetV1,
                    CounterStackObjectV1,
                    TapSourceCostV1,
                    ManaActivationCostV1,
                ],
            ),
            (
                7,
                &[
                    AnyDamageTargetV1,
                    AnyPlayerTargetV1,
                    NoncombatDamageV1,
                    LifeGainV1,
                    LifeLossV1,
                ],
            ),
            (
                8,
                &[
                    TriggeredAbilityV1,
                    SelfEntersBattlefieldV1,
                    OtherCreatureEntersBattlefieldV1,
                    CreatureDiesV1,
                ],
            ),
            (
                9,
                &[
                    CastEnchantmentV1,
                    TemporaryPowerToughnessV1,
                    CombatDamagePreventionV1,
                    DamageBudgetPreventionV1,
                    StaticDamageReplacementV1,
                ],
            ),
            (10, &[SelfDiesV1, InterveningIfV1, ModalChoiceV1]),
            (11, &[DefenderV1]),
            (
                12,
                &[
                    FlyingV1,
                    ReachV1,
                    VigilanceV1,
                    HasteV1,
                    LifelinkV1,
                    DeathtouchV1,
                ],
            ),
            (13, &[TrampleV1, FirstStrikeV1, DoubleStrikeV1, MenaceV1]),
            (14, &[CreatureTokensV1, PlusOneCountersV1]),
            (
                15,
                &[
                    DestroyCreatureV1,
                    GraveyardRecursionV1,
                    BasicLandSearchV1,
                    CreatureAnthemV1,
                    TargetPlayerDrawV1,
                ],
            ),
            (
                16,
                &[CastArtifactV1, PermanentManaV1, CompositeActivationCostV1],
            ),
            (17, &[AttachmentsV1]),
            (18, &[PermanentRemovalV1]),
            (19, &[SacrificeV1]),
            (20, &[BoardWipesV1]),
        ];
        // Enumerate the public vocabulary, so a new capability cannot silently
        // escape this contract. Check real, fully valid definitions rather than
        // inventing capability-only records that fail independent shape checks.
        let schema = serde_json::to_value(schemars::schema_for!(Capability)).unwrap();
        let vocabulary: BTreeSet<Capability> =
            serde_json::from_value::<Vec<Capability>>(schema["enum"].clone())
                .unwrap()
                .into_iter()
                .collect();
        let covered: BTreeSet<Capability> = groups
            .iter()
            .flat_map(|(_, caps)| caps.iter().copied())
            .collect();
        assert_eq!(covered, vocabulary);
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalogs");
        let definitions = std::fs::read_dir(&root)
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
        for &(version, caps) in groups {
            for cap in caps {
                let witnesses = definitions
                    .iter()
                    .filter(|d| d.capabilities.contains(cap))
                    .collect::<Vec<_>>();
                let catalog = |d: &Definition, schema_version| Catalog {
                    schema_version,
                    rules_edition: RULES_EDITION.into(),
                    rules_sha256: RULES_SHA256.into(),
                    definitions: vec![d.clone()],
                };
                assert!(
                    witnesses
                        .iter()
                        .any(|d| catalog(d, version).validate().is_ok()),
                    "{cap:?}: no valid IR v{version} witness"
                );
                for d in witnesses {
                    assert!(
                        catalog(d, version - 1).validate().is_err(),
                        "{cap:?}: accepted below IR v{version}"
                    );
                }
            }
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

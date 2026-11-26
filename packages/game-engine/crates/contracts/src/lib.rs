//! Executable Card IR contracts. No rules execution or parser dependency.
mod basic_landwalk;
mod builders;

mod color_hybrid_cost;
pub use color_hybrid_cost::{ColorHybridSymbol, HybridPayment, color_hybrid_cost};
mod cycling;

pub use cycling::hand_discard_activation;

pub mod mana_spend;

pub use mana_spend::{ManaSpellSubtype, ManaSpellTraits, ManaSpendRestriction, RestrictedMana};

pub mod cast_subtype;

pub mod fixed_color_protection;

mod typed_land_search;
pub use typed_land_search::BasicLandType;
mod build_input;
pub use build_input::engine_build_input;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
mod activated_artifact_draw;

pub mod activated_source_keyword;
pub use activated_artifact_draw::activated_artifact_draw_ability;

mod controller_library;

pub mod equipped_trigger;
pub mod impulse_play;

pub use impulse_play::PlayWindow;
pub mod resolution_mana;
pub use resolution_mana::ManaResolutionQuantity;
pub mod predefined_tokens;
pub use predefined_tokens::PredefinedToken;

pub use controller_library::matches_fixed_amount;

use kir::{Context, PlayerExpr, SetExpr, Verb};

use serde::{Deserialize, Serialize, de};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

mod archives;
pub use archives::*;

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
impl ManaCost {}

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
impl CardKind {
    /// CR 205.2a: every card type the engine represents, in declaration
    /// order. Kindred, battle and planeswalker cards are rejected at
    /// admission, so a counted set of card types (Tarmogoyf's) is complete
    /// over these. Representing another type adds a variant, which
    /// [`CardKind::card_type_index`] then fails to compile without.
    pub const CARD_TYPES: [CardKind; 6] = [
        Self::Land,
        Self::Creature,
        Self::Instant,
        Self::Sorcery,
        Self::Enchantment,
        Self::Artifact,
    ];

    /// This card type's position in [`CardKind::CARD_TYPES`]. Exhaustive,
    /// with no wildcard, so a newly represented card type must be counted.
    pub const fn card_type_index(self) -> usize {
        match self {
            Self::Land => 0,
            Self::Creature => 1,
            Self::Instant => 2,
            Self::Sorcery => 3,
            Self::Enchantment => 4,
            Self::Artifact => 5,
        }
    }
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
    /// CR 205.3m, 702.73a: this object is every creature type. Printed
    /// changeling and "is every creature type" characteristic-defining
    /// abilities set it in every zone (CR 604.3); layer 4 effects set or
    /// clear it (CR 613.1d). Omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub all_creature_types: bool,
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
    /// CR 202.3b, 712.8e: the back face of a nonmodal double-faced card has
    /// no mana cost, but its mana value is calculated from its front face's
    /// mana cost. Present only on such a back face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_face_mana_value: Option<Quantity>,
}
fn is_false(value: &bool) -> bool {
    !*value
}
impl Characteristics {
    pub fn printed_mana_value(&self) -> Quantity {
        if let Some(value) = &self.front_face_mana_value {
            return value.clone();
        }
        let mut value = self.mana_cost.generic.clone();
        value.add(&Quantity::from(
            (self.mana_cost.colored.len() + self.hybrid_mana.len()) as i64,
        ));
        value
    }

    fn is_attachment(&self) -> bool {
        self.subtypes
            .iter()
            .any(|s| matches!(s.as_str(), "Aura" | "Equipment"))
    }

    pub fn has_type(&self, kind: CardKind) -> bool {
        self.kind == kind || self.additional_types.contains(&kind)
    }

    /// Whether `subtype` is one of this object's subtypes. An object that
    /// is every creature type has each CR 205.3m creature type while it is a
    /// creature (CR 205.3d: kindred is not an admitted card type).
    pub fn has_subtype(&self, subtype: &str) -> bool {
        self.subtypes.iter().any(|s| s == subtype)
            || (self.all_creature_types
                && self.has_type(CardKind::Creature)
                && kir::is_creature_subtype(subtype))
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
    PredefinedArtifactTokensV1,

    PlayerGraveyardShuffleV1,

    ControllerLifeFreezeV1,

    ProtectionFromEverythingV1,

    BasicLandwalkV1,
    EvasionKeywordsV1,
    FixedColorProtectionV1,
    SourceCombatRestrictionsV1,

    CastHistoryV1,
    CastManaValueSweepV1,

    CastTriggerV1,

    CastColorPredicateV1,
    CastSubtypePredicateV1,

    SourceTapTriggerV1,

    AttackTriggerV1,
    DamageTriggerV1,
    SourceDamageTriggerV2,

    EventPlayerDiscardV1,
    EventPlayerSacrificeV1,
    /// CR 701.21a: a resolving sacrifice of the source permanent itself.
    SacrificeSourceV1,
    FilteredDamageTriggerV1,
    DamageControllerDiscardV1,

    EventPlayerRecipientV1,

    UntapControllerLandsV1,
    StepBeginningTriggerV1,
    EntersTappedV1,
    EntryLifePaymentV1,
    EntryRevealV1,
    ConditionalEntryTappedV1,
    EntryPlusOneCountersV1,

    TypedNonbasicIntrinsicManaV1,
    FlashV1,
    /// CR 702.37, 708: cast face down as a 2/2 for {3} and turn face up for
    /// the morph or megamorph cost as a special action.
    FaceDownCastV1,
    EnterAsCopyOfPermanentEnteredThisTurnV1,
    CastArtifactV1,
    PermanentManaV1,

    CompositeActivationCostV1,
    CastEnchantmentV1,

    TemporaryPowerToughnessV1,
    TemporaryCombatProhibitionV1,
    ResolvedControlV1,
    ResolvedCharacteristicsV1,

    /// CR 615.1, 615.10: static and resolved damage prevention.
    DamagePreventionV1,
    /// CR 615.7: resolved "prevent the next N damage" shields.
    DamageBudgetPreventionV1,
    /// CR 614.1a: static damage-amount replacement.
    StaticDamageReplacementV1,
    /// CR 615.12: damage that can't be prevented.
    UnpreventableDamageV1,
    /// CR 604.3, 613.4a: a characteristic-defining ability that defines the
    /// source's power and/or toughness in every zone.
    CharacteristicPowerToughnessV1,

    NormalLayoutV1,
    ModalDoubleFacedV1,
    /// CR 712.2, 701.27: a nonmodal double-faced card that transforms.
    TransformingDoubleFacedV1,
    /// CR 715: an adventurer card and its Adventure spell.
    AdventurerV1,
    /// CR 701.27a,f: an ability of a permanent transforms that permanent.
    TransformSourceV1,

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
    OptionalObjectTargetV1,
    TapPermanentV1,
    ActivatedTapPermanentV1,
    ActivatedCreatureTapV1,
    TapSourceDestroyV1,
    TapSourceUntapCreatureV1,

    ActivatedPermanentOwnerHandV1,
    ActivatedArtifactDrawV1,
    OwnerHandMoveV1,
    PermanentOwnerHandMoveV1,
    CastSorceryV1,
    OwnerLibraryTopMoveV1,
    ActivatedAbilityV1,
    SacrificeSourceCostV1,
    ControllerDrawV1,

    FixedMillProgramV1,

    ControllerCountedDrawV1,

    ControllerDiscardV1,
    OpponentTargetV1,
    TargetPlayerSacrificeCreatureV1,
    StackSpellTargetV1,
    StackAbilityTargetV1,
    CounterStackObjectV1,
    RegenerationShieldV1,

    ControlledCreatureSpellCopyV1,
    TapSourceCostV1,
    PayLifeCostV1,
    ManaActivationCostV1,

    DiscardSourceCostV1,
    ExileSourceCostV1,
    ChosenObjectCostV1,
    AnyDamageTargetV1,

    PlayerOrPlaneswalkerTargetV1,
    AnyPlayerTargetV1,
    NoncombatDamageV1,
    LifeGainV1,
    LifeLossV1,
    OpponentLifeDrainV1,
    KernelControlV1,
    ResolutionObjectChoiceV1,
    TargetCharacteristicsV1,
    OptionalTargetGroupV1,
    /// CR 115.3, 601.2c: one occurrence announces exactly N distinct objects.
    CountedTargetGroupV1,
    /// CR 115.3, 608.2b: a counted battlefield group removed simultaneously.
    TargetGroupRemovalV1,
    TriggeredAbilityV1,
    SelfEntersBattlefieldV1,
    OtherCreatureEntersBattlefieldV1,

    EntryGraveyardExileV1,

    ControlledLandEntryV1,

    CreatureDiesV1,
    SelfDiesV1,
    InterveningIfV1,
    /// CR 603.2: an admitted event pattern outside the reviewed event rows.
    EventTriggerV1,
    /// CR 603.2: a turn ordinal or "triggers only once each turn", read from
    /// the public per-turn ledger.
    TriggerTurnHistoryV1,
    ResolutionTriggerV1,
    CreatureOrPlaneswalkerTargetV1,

    ModalChoiceV1,

    DefenderV1,
    FlyingV1,
    ReachV1,
    VigilanceV1,
    HasteV1,
    LifelinkV1,
    DeathtouchV1,
    IndestructibleV1,

    ActivatedSourceKeywordGrantV1,
    TrampleV1,
    FirstStrikeV1,
    DoubleStrikeV1,
    MenaceV1,
    FightV1,
    PopulateV1,
    PlayerCountersV1,
    ProliferateV1,
    CreatureTokensV1,
    MultiTypeCreatureTokensV1,
    PlusOneCountersV1,

    DestroyCreatureV1,
    GraveyardRecursionV1,

    OwnGraveyardCardReturnV1,
    GraveyardCardTargetV1,
    PrivateLibraryReorderV1,
    SearchLibraryTopV1,
    SearchSplitBasicLandsV1,
    OwnerLibraryBottomMoveV1,
    OwnerLibraryShuffleMoveV1,
    PrivateLibrarySelectionV1,
    BasicLandSearchV1,

    CreatureAnthemV1,
    ContinuousStaticsV1,
    RuleStaticsV1,
    /// CR 113.10, 613.1f: a battlefield static grants quoted mana abilities.
    GrantedManaAbilitiesV1,

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

    TargetPlayerDiscardV1,
    RandomPlayerDiscardV1,
    GroupHandDiscardV1,

    // Admitted by `composition`, not by a whole-card family. A plain comment:
    // a documented variant would change the published enum schema shape.
    ComposedAbilityV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunctioningZone {
    Hand,
    Graveyard,
    Battlefield,
    Stack,
    /// Reserved for the source card's own graveyard-move replacement and
    /// characteristic-defining abilities (CR 604.3).
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
    /// CR 404.1: a card in any graveyard, or only an opponent's.
    GraveyardCard {
        opponents_only: bool,
    },
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
    /// Additional fixed characteristic tests at announcement and resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<kir::Pred>,
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
    /// CR 702.73a: the token has changeling.
    #[serde(default, skip_serializing_if = "is_false")]
    pub all_creature_types: bool,
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
            all_creature_types: self.all_creature_types,
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
            front_face_mana_value: None,
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
    pub selective_players: Option<&'a kir::PlayerExpr>,
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
                tapped: false,
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
        let Self::Kernel { node, source_block } = self;
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
            selective_players: None,
            amount,
            source_block: *source_block,
        };
        for part in parts {
            match part {
                kir::DamageTo::Objects { set } => damage.sets.push(set),
                kir::DamageTo::Players {
                    players: kir::PlayerExpr::EachPlayer,
                } => damage.each_player = true,
                kir::DamageTo::Players { players } => damage.selective_players = Some(players),
                _ => return None,
            }
        }
        Some(damage)
    }
    /// The verb of a single-verb kernel instruction.
    pub fn verb(&self) -> Option<&kir::Verb> {
        match self {
            Self::Kernel { node, .. } => node.verb(),
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

pub use kir::StaticEffect;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaticAbility {
    pub functioning_zone: FunctioningZone,
    pub effect: StaticEffect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CreatureRestriction {
    CantAttack,
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SpellResolution {
    Instructions,
    Permanent,
}
/// CR 702.27a: the optional mana cost that changes successful spell completion.
pub const BUYBACK_COST: &str = "buyback";
/// CR 702.74a: the alternative cost whose payment makes the entering
/// permanent's controller sacrifice it.
pub const EVOKE_COST: &str = "evoke";

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
    /// CR 115.1d, 601.2c: the sole announced battlefield occurrence may be omitted.
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional_target: bool,
    pub targets: Vec<TargetSpec>,
    pub instructions: Vec<Instruction>,
}

/// CR 601.2b, 602.2b, 603.3c and 700.2a–d: one announcement or placement
/// chooses distinct modes. Each admitted selection is an ordinary program in
/// printed order with only its own target occurrences. Bit zero denotes the
/// first printed mode. Spells, activated and triggered abilities share it.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModalSpell {
    pub source_block: u32,
    pub min: u32,
    pub max: u32,
    pub labels: Vec<String>,
    pub selections: BTreeMap<u8, SpellProgram>,
}
impl ModalSpell {
    pub const MAX_MODES: usize = 5;

    pub fn masks(len: usize, min: u32, max: u32) -> impl Iterator<Item = u8> {
        let limit = if len <= Self::MAX_MODES {
            1u8 << len
        } else {
            0
        };
        (0..limit).filter(move |mask| (min..=max).contains(&mask.count_ones()))
    }
}
fn single_sacrifice() -> u32 {
    1
}
fn single_sacrifice_count(n: &u32) -> bool {
    *n == 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChosenCostAction {
    Discard,
    Tap,
    ExileFromGraveyard,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ChosenCostFilter {
    Any,
    Type(CardKind),
    Subtype(String),
    ArtifactOrCreature,
}

impl ChosenCostFilter {
    pub fn matches(&self, characteristics: &Characteristics) -> bool {
        match self {
            Self::Any => true,
            Self::Type(kind) => characteristics.has_type(*kind),
            Self::Subtype(name) => characteristics.has_subtype(name),
            Self::ArtifactOrCreature => {
                characteristics.has_type(CardKind::Artifact)
                    || characteristics.has_type(CardKind::Creature)
            }
        }
    }
}

/// CR 602.2b, 601.2g-h: one fixed chosen-object term paid after any mana.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChosenObjectCost {
    pub action: ChosenCostAction,
    pub filter: ChosenCostFilter,
    #[schemars(range(min = 1))]
    pub count: u32,
    pub another: bool,
    pub tap_source: bool,
    pub mana: Option<ManaCost>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ActivationCost {
    /// Fixed mana and this exact hand source, committed as one complete cost.
    DiscardSource {
        mana: ManaCost,
    },
    /// The exact source card is exiled from its functioning zone after mana.
    ExileSource {
        mana: ManaCost,
    },
    ChooseObjects(ChosenObjectCost),
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
        /// CR 602.2b, 701.21a: "{T}" is paid with the sacrifice, after mana.
        #[serde(default, skip_serializing_if = "is_false")]
        tap: bool,
        /// CR 109.1, 701.21a: "another" excludes the ability's own source.
        #[serde(default, skip_serializing_if = "is_false")]
        another: bool,
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
    /// CR 118.3, 119.4: "Pay N life", paid only by a player whose life total
    /// is at least N; paying life is loss of life, never damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub pay_life: Option<Quantity>,
}
/// CR 106.1, 106.4: the mana one resolving mana ability adds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mana", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaProduction {
    Fixed {
        symbols: Vec<Color>,
    },
    Bundles {
        options: Vec<Vec<Color>>,
    },
    AnyColor,
    CommanderColor,
    /// `amount` mana of one color its controller chooses from `colors` as
    /// the ability resolves (CR 106.1a, 605.3b): "Add {W} or {U}" and "Add
    /// three mana of any one color".
    Choose {
        colors: Vec<Color>,
        #[schemars(range(min = 1))]
        amount: Quantity,
    },
    /// CR 106.7: one mana of a type that a land controlled by `controller`
    /// could produce, chosen as the ability resolves. `colors_only` reads
    /// "any color" and excludes colorless; otherwise "any type".
    LandsCouldProduce {
        controller: ManaLandController,
        colors_only: bool,
    },
}
/// Whose lands a "could produce" ability inspects, relative to the mana
/// ability's controller (CR 109.5, 102.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ManaLandController {
    You,
    Opponents,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManaAbility {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restriction: Option<ManaActivationRestriction>,
    /// CR 602.5, 605.3a: "Activate only if"; one
    /// [`kir::ActivationCondition`] read as the activation begins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<kir::Cond>,
    pub costs: PermanentActivationCosts,
    pub production: ManaProduction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_restriction: Option<ManaSpendRestriction>,
    /// CR 605.1a, 605.3b: the ability's other printed instructions, Kernel
    /// nodes performed in order after the mana is added, while the ability
    /// resolves immediately and without the stack. A mana ability has no
    /// targets; [`kir::mana_rider`] admits each node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub riders: Vec<kir::Node>,
    pub source_block: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ManaActivationRestriction {
    OnceEachTurn,
}
/// CR 602.5b-d (2026-09-25): restrictions on an individual printed stack
/// activation. Sorcery timing uses the program's existing timing field.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivationRestriction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<ActivationWindow>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub once_each_turn: bool,
    /// CR 602.1b, 602.5: "Activate only if"; one
    /// [`kir::ActivationCondition`] read as the activation begins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<kir::Cond>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivationWindow {
    YourTurn,
    YourUpkeep,
    YourTurnBeforeAttackers,
    /// CR 506.1: any turn's combat phase, beginning of combat through end
    /// of combat.
    Combat,
    /// CR 511: any turn's end of combat step.
    EndOfCombat,
    /// CR 102.1: a turn whose active player is another player.
    NotYourTurn,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivatedAbility {
    pub functioning_zone: FunctioningZone,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restriction: Option<ActivationRestriction>,
    pub cost: ActivationCost,
    pub program: SpellProgram,
    /// CR 602.2b, 700.2: a modal ability chooses its modes as it is
    /// activated; `program` then carries timing only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal: Option<ModalSpell>,
}
impl ActivatedAbility {
    /// Executable versions of this ability's program: its selections when
    /// modal, otherwise the program itself.
    pub fn programs(&self) -> impl Iterator<Item = &SpellProgram> {
        modal_programs(&self.program, self.modal.as_ref())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CascadeThreshold {
    SourceSpellManaValue,
}
/// CR 603.2: an ordinary triggered ability's event, and the intervening
/// "if" it checks when the event occurs and again as it resolves (CR 603.4).
/// [`kir::check_event`] admits the pattern and [`kir::Intervening::of`] the
/// condition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TriggerCondition {
    Event {
        pattern: kir::EventPattern,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intervening_if: Option<kir::Cond>,
        /// "This ability triggers only once each turn": each incarnation of
        /// the source triggers this ability at most once per turn (CR 400.7).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        once_each_turn: bool,
    },
}
impl TriggerCondition {
    /// An event with no intervening condition.
    pub fn on(pattern: kir::EventPattern) -> Self {
        Self::Event {
            pattern,
            intervening_if: None,
            once_each_turn: false,
        }
    }
    pub fn once_each_turn(&self) -> bool {
        match self {
            Self::Event { once_each_turn, .. } => *once_each_turn,
        }
    }
    pub fn pattern(&self) -> &kir::EventPattern {
        match self {
            Self::Event { pattern, .. } => pattern,
        }
    }
    pub fn intervening_if(&self) -> Option<&kir::Cond> {
        match self {
            Self::Event { intervening_if, .. } => intervening_if.as_ref(),
        }
    }
    /// Stable reporting vocabulary, independent of the serde representation.
    pub fn reporting_key(&self) -> &'static str {
        self.pattern().reporting_key()
    }
}
/// Exact step predicates; delayed and conditional boundaries are excluded.
/// CR 503-507, 511-513: the beginning of the upkeep, draw, beginning of
/// combat, end of combat and end steps, and of the precombat and postcombat
/// main phases. A turn has one combat phase, so its postcombat main phase is
/// its second main phase (CR 505.1a).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BeginningStep {
    PrecombatMain,
    Upkeep,
    Draw,
    End,
    BeginningOfCombat,
    EndOfCombat,
    PostcombatMain,
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
    pub functioning_zone: FunctioningZone,
    pub condition: TriggerCondition,
    /// This mandatory stack program is placed by the rules, never cast or
    /// activated. Its existing Instant shape carries no timing permission.
    pub program: SpellProgram,
    /// CR 603.3c, 700.2: a modal trigger's modes are chosen as it is put on
    /// the stack; `program` then carries timing only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal: Option<ModalSpell>,
}
impl TriggeredAbility {
    /// Executable versions of this ability's program: its selections when
    /// modal, otherwise the program itself.
    pub fn programs(&self) -> impl Iterator<Item = &SpellProgram> {
        modal_programs(&self.program, self.modal.as_ref())
    }
}
fn modal_programs<'a>(
    base: &'a SpellProgram,
    modal: Option<&'a ModalSpell>,
) -> impl Iterator<Item = &'a SpellProgram> {
    std::iter::once(base)
        .filter(move |_| modal.is_none())
        .chain(modal.into_iter().flat_map(|m| m.selections.values()))
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
    // CR 702.13, 702.28, 702.31, 702.36, 702.118: evasion abilities that
    // restrict which creatures can block (and, for shadow, be blocked by).
    Intimidate,
    Shadow,
    Horsemanship,
    Fear,
    Skulk,
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
            "intimidate" => Ok(Self::Intimidate),
            "shadow" => Ok(Self::Shadow),
            "horsemanship" => Ok(Self::Horsemanship),
            "fear" => Ok(Self::Fear),
            "skulk" => Ok(Self::Skulk),

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
                    "intimidate",
                    "shadow",
                    "horsemanship",
                    "fear",
                    "skulk",
                ],
            )),
        }
    }
}
/// CR 712.8a,d-f and 715.4: a two-part card has its front (or normal)
/// characteristics until its back face (or Adventure) is chosen for the
/// stack or battlefield, or the permanent transforms.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CardFace {
    #[default]
    Front,
    Back,
}
impl CardFace {
    pub fn is_front(&self) -> bool {
        *self == Self::Front
    }
    /// CR 701.27a: the other face of a two-faced object.
    pub fn other(self) -> Self {
        match self {
            Self::Front => Self::Back,
            Self::Back => Self::Front,
        }
    }
}

/// How the two printed parts of one physical card relate (CR 712, 715).
/// <https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FaceLayout {
    /// One printed face.
    #[default]
    Single,
    /// CR 712.3, 712.11b, 712.12: either face may be cast or played.
    ModalDoubleFaced,
    /// CR 712.2, 712.11, 712.14: cast and enters front face up; the
    /// permanent turns over only when it transforms (CR 701.27).
    Transforming,
    /// CR 715.3: cast normally or as its Adventure (the back part).
    Adventurer,
}
impl FaceLayout {
    pub fn is_single(&self) -> bool {
        *self == Self::Single
    }
}

/// The printed keyword that lets a card be cast face down (CR 702.37a-b).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FaceDownKind {
    /// CR 702.37a: "Morph [cost]".
    Morph,
    /// CR 702.37b: "Megamorph [cost]"; a +1/+1 counter as it is turned face
    /// up for that cost.
    Megamorph,
}

/// CR 702.37a-b, e: "[Morph] [cost]": the card may be cast as a 2/2
/// face-down creature for {3} (CR 708.2a), and its controller may turn the
/// face-down permanent face up any time they have priority by paying `cost`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FaceDownCast {
    pub kind: FaceDownKind,
    pub cost: ManaCost,
}
impl FaceDownCast {
    /// CR 702.37a: the alternative cost of a face-down cast.
    pub fn cast_cost() -> ManaCost {
        ManaCost {
            generic: Quantity::from(3),
            colored: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    /// How `back_face` relates to this front face.
    #[serde(default, skip_serializing_if = "FaceLayout::is_single")]
    pub layout: FaceLayout,
    /// The other printed part of a two-part card: a double-faced card's back
    /// face or an adventurer's Adventure. Only a physical card's root
    /// definition owns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back_face: Option<Box<Definition>>,
    /// Offline compiler-owned labels; absent on historical IR and synthetic definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_labels: Option<SourceLabels>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub x_mana_cost: bool,
    /// CR 702.34a: "Flashback [cost]": this instant or sorcery may be cast
    /// from its owner's graveyard for this mana instead of its mana cost,
    /// and is exiled whenever it would leave the stack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flashback: Option<ManaCost>,
    /// CR 702.37a-b: the morph or megamorph ability of this card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_down: Option<FaceDownCast>,
    /// CR 118.9, 601.2b: a printed alternative cost (evoke, CR 702.74a) the
    /// caster may announce instead of the mana cost when casting from hand
    /// or the command zone. Its identity names the keyword.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternative_cost: Option<OptionalAdditionalManaCost>,
    pub oracle_id: String,
    pub name: String,
    pub characteristics: Characteristics,
    pub intrinsic_zone: Option<FunctioningZone>,
    pub spell: Option<SpellProgram>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal_spell: Option<ModalSpell>,
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
impl Definition {
    pub fn face(&self, face: CardFace) -> Option<&Self> {
        match face {
            CardFace::Front => Some(self),
            CardFace::Back => self.back_face.as_deref(),
        }
    }

    /// The parts a player may choose to cast or play from a zone other than
    /// the battlefield (CR 712.11b, 712.12, 715.3). A transforming card's
    /// back face is never chosen this way (CR 712.11).
    pub fn faces(&self) -> impl Iterator<Item = CardFace> {
        let back = matches!(
            self.layout,
            FaceLayout::ModalDoubleFaced | FaceLayout::Adventurer
        ) && self.back_face.is_some();
        std::iter::once(CardFace::Front).chain(back.then_some(CardFace::Back))
    }

    /// CR 712.9: only a nonmodal double-faced card's permanent transforms.
    pub fn transforms(&self) -> bool {
        self.layout == FaceLayout::Transforming && self.back_face.is_some()
    }

    /// Executable versions of the spell program. For a modal spell the base
    /// program carries timing and resolution; effects belong to selections.
    pub fn spell_programs(&self) -> impl Iterator<Item = &SpellProgram> {
        self.spell
            .iter()
            .filter(|_| self.modal_spell.is_none())
            .chain(
                self.modal_spell
                    .iter()
                    .flat_map(|modal| modal.selections.values()),
            )
    }

    pub fn requirements(&self) -> BTreeSet<Capability> {
        let mut set = BTreeSet::from([Capability::NormalLayoutV1, Capability::ComposedAbilityV1]);
        if self.characteristics.is_attachment() {
            set.insert(Capability::AttachmentsV1);
        }
        match self.characteristics.kind {
            CardKind::Land => {
                set.insert(Capability::LandPlayV1);
                if self.characteristics.basic {
                    set.insert(Capability::BasicLandManaV1);
                } else if !self.characteristics.subtypes.is_empty() {
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
        if self.modal_spell.is_some()
            || self.activated.iter().any(|a| a.modal.is_some())
            || self.triggered.iter().any(|a| a.modal.is_some())
        {
            set.insert(Capability::ModalChoiceV1);
        }
        for program in self.spell_programs() {
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
            if ability.programs().any(has_untap) {
                set.insert(Capability::TapSourceUntapCreatureV1);
            }
            if ability.programs().any(has_tap) {
                set.insert(if self.characteristics.kind == CardKind::Creature {
                    Capability::ActivatedCreatureTapV1
                } else {
                    Capability::ActivatedTapPermanentV1
                });
            }
            if ability.programs().any(has_permanent_owner_hand) {
                set.insert(Capability::ActivatedPermanentOwnerHandV1);
            }
            if ability.cost == ActivationCost::TapSource
                && ability.programs().any(has_destroy_permanent)
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
                ActivationCost::ExileSource { .. } => Capability::ExileSourceCostV1,
                ActivationCost::ChooseObjects(_) => Capability::ChosenObjectCostV1,
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
            for program in ability.programs() {
                add_program_requirements(&mut set, program);
            }
        }
        for ability in &self.triggered {
            set.insert(Capability::TriggeredAbilityV1);
            set.extend(trigger_capabilities(ability));
            for program in ability.programs() {
                add_program_requirements(&mut set, program);
            }
        }
        for keyword in &self.keywords {
            set.extend(intrinsic_keyword_capability(*keyword));
        }
        for ability in &self.static_abilities {
            set.insert(ability.effect.capability(self.characteristics.kind));
        }
        if self.face_down.is_some() {
            set.insert(Capability::FaceDownCastV1);
        }
        if let Some(back) = &self.back_face {
            set.insert(match self.layout {
                FaceLayout::Transforming => Capability::TransformingDoubleFacedV1,
                FaceLayout::Adventurer => Capability::AdventurerV1,
                FaceLayout::Single | FaceLayout::ModalDoubleFaced => Capability::ModalDoubleFacedV1,
            });
            set.extend(back.requirements());
        }
        set
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.composed {
            return Err("definition requires complete Archive R4 composition".into());
        }
        kir::executes(self)
    }
}
/// The reviewed event rows a triggered ability uses. Each capability names
/// one row of patterns whose detection a review covered; any other admitted
/// pattern uses the `Executes` row [`Capability::EventTriggerV1`].
fn trigger_capabilities(ability: &TriggeredAbility) -> Vec<Capability> {
    use kir::{Atom, EventPattern as E, Pred};
    let you = || PlayerExpr::you(Context::Triggered);
    let creature = Atom::Type {
        kind: CardKind::Creature,
    };
    let pattern = ability.condition.pattern();
    let mut caps = vec![];
    if ability.condition.intervening_if().is_some() {
        caps.push(Capability::InterveningIfV1);
    }
    if ability.condition.once_each_turn() || pattern.nth().is_some() {
        caps.push(Capability::TriggerTurnHistoryV1);
    }
    // CR 700.4: a creature you control (optionally another, nontoken or
    // non-Spirit) dying, any creature dying, or the host of this Aura or
    // Equipment dying.
    let creature_dies = |p: &Pred| {
        *p == Pred::attached_creature() || *p == Pred(vec![creature.clone()]) || {
            let mut rest = p.0.as_slice();
            let mut take = |atom: &Atom, required: bool| match rest.split_first() {
                Some((first, tail)) if first == atom => {
                    rest = tail;
                    true
                }
                _ => !required,
            };
            take(&creature, true)
                && take(
                    &Atom::NotSubtype {
                        subtype: "Spirit".into(),
                    },
                    false,
                )
                && take(&Atom::ControlledBy { player: you() }, true)
                && take(&Atom::NotToken, false)
                && take(
                    &Atom::IsNot {
                        object: kir::Ref::Source,
                    },
                    false,
                )
                && rest.is_empty()
        }
    };
    let event = match pattern {
        E::Enters { .. } if *pattern == E::self_enters() => Capability::SelfEntersBattlefieldV1,
        E::Enters { .. } if *pattern == E::other_creature_enters() => {
            Capability::OtherCreatureEntersBattlefieldV1
        }
        E::Enters { .. } if *pattern == E::controlled_land_enters() => {
            Capability::ControlledLandEntryV1
        }
        E::Dies { .. } if *pattern == E::self_dies() => {
            if ability.programs().flat_map(|p| &p.instructions).any(|i| {
                matches!(
                    i,
                    kernel!(CreateTokens {
                        token: kir::TokenSpec::Creature { .. },
                        ..
                    })
                )
            }) {
                caps.push(Capability::SacrificeV1);
            }
            Capability::SelfDiesV1
        }
        E::Dies { .. } if *pattern == E::opponent_creature_dies() => Capability::BoardWipesV1,
        E::Dies { object } if creature_dies(object) => {
            if object.names_you() {
                caps.push(Capability::SacrificeV1);
            }
            Capability::CreatureDiesV1
        }
        E::BecomesTapped { .. } if *pattern == E::self_becomes_tapped() => {
            Capability::SourceTapTriggerV1
        }
        E::Attacks { .. } => Capability::AttackTriggerV1,
        E::Cast { spell, .. } => {
            if matches!(spell.0.as_slice(), [Atom::Color { .. }]) {
                caps.push(Capability::CastColorPredicateV1);
            } else if cast_subtype::restricted(spell) {
                caps.push(Capability::CastSubtypePredicateV1);
            }
            Capability::CastTriggerV1
        }
        E::StepBegins { .. } => Capability::StepBeginningTriggerV1,
        E::DealsDamage {
            source,
            combat,
            recipient,
        } => {
            if ability.programs().any(has_controller_discard) {
                caps.push(Capability::DamageControllerDiscardV1);
            }
            if !source.0.iter().any(|a| matches!(a, Atom::Is { .. })) {
                caps.push(Capability::FilteredDamageTriggerV1);
            }
            if *combat && *recipient == PlayerExpr::EachPlayer {
                Capability::DamageTriggerV1
            } else {
                Capability::SourceDamageTriggerV2
            }
        }
        _ => Capability::EventTriggerV1,
    };
    caps.push(event);
    caps
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
                    | KeywordAbility::Menace
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
}
fn has_tap(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Tap {
                what: target_object!(_)
            })
        )
    })
}
fn has_untap(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Untap {
                what: target_object!(_)
            })
        )
    })
}
fn has_controller_discard(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| i.controller_discard().is_some())
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
        KeywordAbility::Intimidate
        | KeywordAbility::Shadow
        | KeywordAbility::Horsemanship
        | KeywordAbility::Fear
        | KeywordAbility::Skulk => Capability::EvasionKeywordsV1,
        KeywordAbility::Hexproof | KeywordAbility::Shroud => Capability::AttachmentsV1,
    })
}
// One primitive mapping for targeted and set grants, including schema closure.

fn add_program_requirements(set: &mut BTreeSet<Capability>, program: &SpellProgram) {
    if program.optional_target {
        set.insert(Capability::OptionalObjectTargetV1);
    }
    if program.target_group().is_some() {
        set.insert(if program.optional_target {
            Capability::OptionalTargetGroupV1
        } else {
            Capability::CountedTargetGroupV1
        });
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
        if target
            .filter
            .as_ref()
            .is_some_and(|filter| filter.predicate.is_some())
        {
            set.insert(Capability::TargetCharacteristicsV1);
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
            TargetDomain::GraveyardCard { .. } => Capability::GraveyardCardTargetV1,
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
        if let kir::Node::Delayed { program, .. } | kir::Node::Reflexive { program, .. } = node {
            add_program_requirements(set, program);
        }
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
    /// Each physical source contributes its native observer/self slots once
    /// per cast, never once per recipient permanent. Tokens/copies with
    /// abilities are outside this physical-source proof.
    pub fn cast_trigger_fanout_bound(&self, physical_sources: usize) -> Option<usize> {
        self.definitions
            .iter()
            .map(|d| d.triggered.len())
            .max()
            .unwrap_or(0)
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
mod schema_tests;

#[cfg(test)]
mod tests {
    use super::*;
    /// CR 205.2a: the counted card types are exactly the represented ones,
    /// each at its own index.
    #[test]
    fn card_types_are_exhaustive_and_indexed() {
        for (index, kind) in CardKind::CARD_TYPES.iter().enumerate() {
            assert_eq!(kind.card_type_index(), index);
        }
        let all = schemars::schema_for!(CardKind);
        let listed = serde_json::to_value(&all).unwrap()["enum"]
            .as_array()
            .unwrap()
            .len();
        assert_eq!(listed, CardKind::CARD_TYPES.len());
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

use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleCommanderReturnChoice {
    pub designation: String,
    pub owner: PlayerId,
    pub to_command: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ObjectHandle(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct StackHandle(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleAbility")]
#[serde(bound(deserialize = "H: Deserialize<'de>, O: Deserialize<'de>"))]
pub struct VisibleAbility<H = StackHandle, O = ObjectHandle> {
    pub handle: H,
    pub source: VisibleTarget<O>,
    pub oracle_id: String,
    pub ability: u32,
}
impl<H, O> VisibleAbility<H, O> {
    pub fn map_handles<T, P: for<'a> From<&'a O>>(
        self,
        mut convert: impl FnMut(H) -> T,
    ) -> VisibleAbility<T, P> {
        VisibleAbility {
            handle: convert(self.handle),
            source: self.source.convert(),
            oracle_id: self.oracle_id,
            ability: self.ability,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct TriggerHandle(pub String);
/// Public origin uses only already captured public/historical handles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "VisibleTriggerAbility")]
pub enum VisibleTriggerAbility {
    Intrinsic { index: u32 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleTriggered")]
#[serde(bound(deserialize = "H: Deserialize<'de>, O: Deserialize<'de>"))]
pub struct VisibleTriggered<H = StackHandle, O = ObjectHandle> {
    pub handle: H,
    pub source: VisibleTarget<O>,
    pub oracle_id: String,
    pub ability: VisibleTriggerAbility,
}
impl<H, O> VisibleTriggered<H, O> {
    pub fn map_handles<T, P: for<'a> From<&'a O>>(
        self,
        mut convert: impl FnMut(H) -> T,
    ) -> VisibleTriggered<T, P> {
        VisibleTriggered {
            handle: convert(self.handle),
            source: self.source.convert(),
            oracle_id: self.oracle_id,
            ability: self.ability,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisiblePendingTrigger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub handle: TriggerHandle,
    pub source: VisibleTarget,
    pub oracle_id: String,
    pub ability: VisibleTriggerAbility,
}
fn is_zero(value: &u64) -> bool {
    *value == 0
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct DecisionToken(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisiblePlayer")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub struct VisiblePlayer<O = ObjectHandle> {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub life_frozen_until_next_turn: bool,
    /// Public viewer handles, grouped by physical owner (CR406.1–3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exile: Vec<O>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub library_shuffles: u64,
    pub player: PlayerId,
    pub status: PlayerStatus,
    pub life: Quantity,
    pub mana: BTreeMap<Color, Quantity>,
    pub hand_count: u32,
    pub library_count: u32,
}
impl<O> VisiblePlayer<O> {
    pub fn convert<T: for<'a> From<&'a O>>(self) -> VisiblePlayer<T> {
        VisiblePlayer {
            life_frozen_until_next_turn: self.life_frozen_until_next_turn,
            exile: self.exile.iter().map(T::from).collect(),
            library_shuffles: self.library_shuffles,
            player: self.player,
            status: self.status,
            life: self.life,
            mana: self.mana,
            hand_count: self.hand_count,
            library_count: self.library_count,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleObject")]
#[serde(bound(deserialize = "H: Deserialize<'de>"))]
pub struct VisibleObject<H = ObjectHandle> {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub face_down: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exiled_by: Option<H>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub phased_out: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub entered_this_turn: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana_ability_used: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_to: Option<H>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<H>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<KeywordAbility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restrictions: Vec<CreatureRestriction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<VisibleToken>,
    #[serde(default, skip_serializing_if = "quantity_is_zero")]
    pub plus_one_counters: Quantity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub characteristics: Option<Characteristics>,
    pub marked_damage: Quantity,
    pub attack_ready: bool,
    pub handle: H,
    pub oracle_id: String,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub tapped: bool,
}

impl<H> VisibleObject<H> {
    pub fn map_handles<T>(self, mut convert: impl FnMut(H) -> T) -> VisibleObject<T> {
        VisibleObject {
            face_down: self.face_down,
            exiled_by: self.exiled_by.map(&mut convert),
            phased_out: self.phased_out,
            entered_this_turn: self.entered_this_turn,
            mana_ability_used: self.mana_ability_used,
            attached_to: self.attached_to.map(&mut convert),
            attachments: self.attachments.into_iter().map(&mut convert).collect(),
            keywords: self.keywords,
            restrictions: self.restrictions,
            token: self.token,
            plus_one_counters: self.plus_one_counters,
            characteristics: self.characteristics,
            marked_damage: self.marked_damage,
            attack_ready: self.attack_ready,
            handle: convert(self.handle),
            oracle_id: self.oracle_id,
            owner: self.owner,
            controller: self.controller,
            zone: self.zone,
            tapped: self.tapped,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleCommander {
    pub designation: String,
    pub oracle_id: String,
    pub history: CommanderHistory,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleTarget")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub struct VisibleTarget<O = ObjectHandle> {
    pub announced: O,
    pub current: Option<O>,
}
impl<O> VisibleTarget<O> {
    pub fn convert<T: for<'a> From<&'a O>>(self) -> VisibleTarget<T> {
        VisibleTarget {
            announced: T::from(&self.announced),
            current: self.current.as_ref().map(T::from),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleStackTarget")]
#[serde(bound(deserialize = "H: Deserialize<'de>"))]
pub struct VisibleStackTarget<H = StackHandle> {
    pub announced: H,
    pub current: Option<H>,
}
impl<H> VisibleStackTarget<H> {
    pub fn map_handles<T>(self, mut convert: impl FnMut(H) -> T) -> VisibleStackTarget<T> {
        VisibleStackTarget {
            announced: convert(self.announced),
            current: self.current.map(&mut convert),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "entity", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerTargetEntity {
    Player { player: PlayerId },
    Object { object: ObjectHandle },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "entity", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "VisibleEntityTarget")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub enum VisibleEntityTarget<O = ObjectHandle> {
    Player { player: PlayerId },
    Object { object: VisibleTarget<O> },
}
impl<O> VisibleEntityTarget<O> {
    pub fn convert<T: for<'a> From<&'a O>>(self) -> VisibleEntityTarget<T> {
        match self {
            Self::Player { player } => VisibleEntityTarget::Player { player },
            Self::Object { object } => VisibleEntityTarget::Object {
                object: object.convert(),
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleStackEntry")]
#[serde(bound(deserialize = "H: Deserialize<'de>, O: Deserialize<'de>"))]
pub struct VisibleStackEntry<H = StackHandle, O = ObjectHandle> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_additional_costs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hybrid_colors: Vec<HybridPayment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana_value: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggered: Option<VisibleTriggered<H, O>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_targets: Vec<VisibleEntityTarget<O>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<H>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack_targets: Vec<VisibleStackTarget<H>>,
    pub spell: Option<O>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability: Option<VisibleAbility<H, O>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_targets: Vec<PlayerId>,
    pub controller: PlayerId,
    pub cast_complete: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub spell_copy: bool,
    /// CR 702.34a: this spell was cast from its owner's graveyard for its
    /// flashback cost and is exiled as it leaves the stack.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flashback: bool,
    pub targets: Vec<VisibleTarget<O>>,
}
impl<H, O> VisibleStackEntry<H, O> {
    pub fn map_handles<T, P: for<'a> From<&'a O>>(
        self,
        mut convert: impl FnMut(H) -> T,
    ) -> VisibleStackEntry<T, P> {
        VisibleStackEntry {
            selected_additional_costs: self.selected_additional_costs,
            x: self.x,
            hybrid_colors: self.hybrid_colors,
            mana_value: self.mana_value,
            triggered: self.triggered.map(|row| row.map_handles(&mut convert)),
            entity_targets: self
                .entity_targets
                .into_iter()
                .map(|t| t.convert())
                .collect(),
            handle: self.handle.map(&mut convert),
            stack_targets: self
                .stack_targets
                .into_iter()
                .map(|row| row.map_handles(&mut convert))
                .collect(),
            spell: self.spell.as_ref().map(P::from),
            ability: self.ability.map(|row| row.map_handles(&mut convert)),
            player_targets: self.player_targets,
            controller: self.controller,
            cast_complete: self.cast_complete,
            spell_copy: self.spell_copy,
            flashback: self.flashback,
            targets: self.targets.into_iter().map(|t| t.convert()).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleAttacker")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub struct VisibleAttacker<O = ObjectHandle> {
    pub attacker: O,
    pub defender: PlayerId,
    pub blocked: bool,
    pub blockers: Vec<O>,
}
impl<O> VisibleAttacker<O> {
    pub fn convert<T: for<'a> From<&'a O>>(self) -> VisibleAttacker<T> {
        VisibleAttacker {
            attacker: T::from(&self.attacker),
            defender: self.defender,
            blocked: self.blocked,
            blockers: self.blockers.iter().map(T::from).collect(),
        }
    }
}
/// A remembered public tuck. Position is one-based; memory gives no object
/// handle or authority to inspect or target a card in a hidden library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KnownLibraryPosition {
    pub library_owner: PlayerId,
    pub position: u32,
    pub oracle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PermittedView")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub struct PermittedView<O = ObjectHandle> {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_searched_hand: Vec<(PlayerId, String)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_library_positions: Vec<KnownLibraryPosition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sacrifice_choices: Vec<(PlayerId, O)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<VisibleSetup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<VisibleEffect>,
    pub combat: Vec<VisibleAttacker<O>>,
    pub viewer: PlayerId,
    pub active: Option<PlayerId>,
    pub turn_owner: PlayerId,
    pub turn: TurnState,
    pub live_turn_order: Vec<PlayerId>,
    pub commanders: Vec<VisibleCommander>,
    pub priority: Option<PlayerId>,
    pub players: Vec<VisiblePlayer>,
    pub objects: Vec<VisibleObject>,
    pub stack: Vec<VisibleStackEntry>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_context: Option<DecisionContext>,
    pub observation_version: u32,
    pub view: PermittedView,
    pub decision: Option<Decision<Viewer>>,
}

fn quantity_is_zero(value: &Quantity) -> bool {
    *value == Quantity::zero()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnStep {
    Untap,
    Upkeep,
    Draw,
    PrecombatMain,
    BeginningCombat,
    DeclareAttackers,
    DeclareBlockers,
    FirstStrikeDamage,
    CombatDamage,
    EndCombat,
    PostcombatMain,
    End,
    Cleanup,
}
impl TurnStep {
    pub fn phase(self) -> Phase {
        match self {
            Self::Untap | Self::Upkeep | Self::Draw => Phase::Beginning,
            Self::PrecombatMain => Phase::PrecombatMain,
            Self::BeginningCombat
            | Self::DeclareAttackers
            | Self::DeclareBlockers
            | Self::FirstStrikeDamage
            | Self::CombatDamage
            | Self::EndCombat => Phase::Combat,
            Self::PostcombatMain => Phase::PostcombatMain,
            Self::End | Self::Cleanup => Phase::Ending,
        }
    }
    pub fn next_step(self) -> Self {
        match self {
            Self::Untap => Self::Upkeep,
            Self::Upkeep => Self::Draw,
            Self::Draw => Self::PrecombatMain,
            Self::PrecombatMain => Self::BeginningCombat,
            Self::BeginningCombat => Self::DeclareAttackers,
            Self::DeclareAttackers => Self::DeclareBlockers,
            Self::DeclareBlockers => Self::CombatDamage,
            Self::FirstStrikeDamage => Self::CombatDamage,
            Self::CombatDamage => Self::EndCombat,
            Self::EndCombat => Self::PostcombatMain,
            Self::PostcombatMain => Self::End,
            Self::End => Self::Cleanup,
            Self::Cleanup => Self::Untap,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TurnState {
    pub number: u64,
    pub step: TurnStep,
    pub lands_played: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisibleSetupPhase {
    Declare,
    Bottom,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleSetupPlayer {
    pub player: PlayerId,
    pub kept: bool,
    /// Accepted declaration rule-steps, including the current round before redraw.
    pub mulligans_taken: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleSetup {
    pub round: u8,
    pub phase: VisibleSetupPhase,
    pub players: Vec<VisibleSetupPlayer>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionSourceKind {
    Spell,
    Activated,
    Triggered,
    ManaAbility,
    Effect,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionSource {
    pub kind: DecisionSourceKind,
    pub oracle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ObjectHandle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability: Option<u32>,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionInstruction {
    pub source_block: u32,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CandidateLabel {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<DecisionSource>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<DecisionSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<DecisionInstruction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<CandidateLabel>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleToken {
    pub name: String,
    pub keywords: Vec<KeywordAbility>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "zone", rename_all = "snake_case", deny_unknown_fields)]
pub enum Zone {
    Battlefield,
    Hand { owner: PlayerId },
    Graveyard { owner: PlayerId },
    Library { owner: PlayerId },
    Command { owner: PlayerId },
    Exile,
    Stack,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommanderHistory {
    pub owner: PlayerId,
    pub color_identity: Vec<Color>,
    #[serde(deserialize_with = "player_quantity_map")]
    pub command_casts: BTreeMap<PlayerId, Quantity>,
    #[serde(deserialize_with = "player_quantity_map")]
    pub combat_damage: BTreeMap<PlayerId, Quantity>,
}
// JSON object keys are strings. Internally tagged publication/IPC enums
// deserialize buffered Content, which cannot decode a string as the numeric
// scalar inside PlayerId. Adapt these map KEYS only; PlayerId values remain
// numeric. Serialization and JsonSchema still use the original field types.
pub(super) fn player_quantity_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<PlayerId, Quantity>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct PlayerQuantities;
    impl<'de> serde::de::Visitor<'de> for PlayerQuantities {
        type Value = BTreeMap<PlayerId, Quantity>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map with canonical decimal u8 player keys")
        }
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some((key, quantity)) = map.next_entry::<String, Quantity>()? {
                let player = key
                    .parse::<u8>()
                    .map_err(|_| serde::de::Error::custom("invalid player history map key"))?;
                if key != player.to_string() {
                    return Err(serde::de::Error::custom(
                        "noncanonical player history map key",
                    ));
                }
                if result.insert(PlayerId(player), quantity).is_some() {
                    return Err(serde::de::Error::custom("duplicate player history map key"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(PlayerQuantities)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayerStatus {
    Live,
    Left,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GraveyardOrderOption {
    Regenerate,
}
/// The intrinsic ability for a basic land type or an index into this source's printed mana abilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "ability", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaAbilityIdentity {
    Intrinsic { color: Color },
    Printed { index: u32 },
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct PlayerId(pub u8);
impl From<&ObjectHandle> for ObjectHandle {
    fn from(handle: &Self) -> Self {
        handle.clone()
    }
}
impl From<&StackHandle> for StackHandle {
    fn from(handle: &Self) -> Self {
        handle.clone()
    }
}
impl From<&EffectHandle> for EffectHandle {
    fn from(handle: &Self) -> Self {
        handle.clone()
    }
}

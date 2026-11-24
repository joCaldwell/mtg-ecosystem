#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleCommanderReturnChoice {
    pub designation: String,
    pub owner: PlayerId,
    pub to_command: bool,
}
use super::*;
use super::{HandleId, stored_history::StoredPermittedView};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ObjectHandle(pub String);
/// Public declaration hints, scoped to current attacker/blocker handles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegalBlockers {
    pub blocker: ObjectHandle,
    pub attackers: Vec<ObjectHandle>,
}
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
    pub(super) fn map_handles<T, P: for<'a> From<&'a O>>(
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
#[serde(bound(deserialize = "O: Deserialize<'de>, E: Deserialize<'de>"))]
pub enum VisibleTriggerAbility<O = ObjectHandle, E = EffectHandle> {
    DelayedChapter {
        chapter: u32,
    },
    Intrinsic {
        index: u32,
    },
    Granted {
        source: VisibleTarget<O>,
        oracle_id: String,
        static_ability_index: u32,
    },
    TemporaryGrant {
        source: VisibleTarget<O>,
        oracle_id: String,
        triggered_ability_index: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effect: Option<E>,
    },
}
impl<O, E> VisibleTriggerAbility<O, E> {
    fn convert<T: for<'a> From<&'a O>, F: for<'a> From<&'a E>>(
        self,
    ) -> VisibleTriggerAbility<T, F> {
        match self {
            Self::DelayedChapter { chapter } => VisibleTriggerAbility::DelayedChapter { chapter },
            Self::Intrinsic { index } => VisibleTriggerAbility::Intrinsic { index },
            Self::Granted {
                source,
                oracle_id,
                static_ability_index,
            } => VisibleTriggerAbility::Granted {
                source: source.convert(),
                oracle_id,
                static_ability_index,
            },
            Self::TemporaryGrant {
                source,
                oracle_id,
                triggered_ability_index,
                effect,
            } => VisibleTriggerAbility::TemporaryGrant {
                source: source.convert(),
                oracle_id,
                triggered_ability_index,
                effect: effect.as_ref().map(F::from),
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleTriggered")]
#[serde(bound(deserialize = "H: Deserialize<'de>, O: Deserialize<'de>, E: Deserialize<'de>"))]
pub struct VisibleTriggered<H = StackHandle, O = ObjectHandle, E = EffectHandle> {
    pub handle: H,
    pub source: VisibleTarget<O>,
    pub oracle_id: String,
    pub ability: VisibleTriggerAbility<O, E>,
}
impl<H, O, E> VisibleTriggered<H, O, E> {
    pub(super) fn map_handles<T, P: for<'a> From<&'a O>, F: for<'a> From<&'a E>>(
        self,
        mut convert: impl FnMut(H) -> T,
    ) -> VisibleTriggered<T, P, F> {
        VisibleTriggered {
            handle: convert(self.handle),
            source: self.source.convert(),
            oracle_id: self.oracle_id,
            ability: self.ability.convert(),
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restricted_mana: Vec<RestrictedMana>,
    pub hand_count: u32,
    pub library_count: u32,
}
impl<O> VisiblePlayer<O> {
    pub(in crate::engine) fn convert<T: for<'a> From<&'a O>>(self) -> VisiblePlayer<T> {
        VisiblePlayer {
            life_frozen_until_next_turn: self.life_frozen_until_next_turn,
            exile: self.exile.iter().map(T::from).collect(),
            library_shuffles: self.library_shuffles,
            player: self.player,
            status: self.status,
            life: self.life,
            mana: self.mana,
            restricted_mana: self.restricted_mana,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_values: Option<VisiblePermanentCopy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_face: Option<String>,
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
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub plus_one_counters: Quantity,
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub lore_counters: Quantity,
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
    pub(super) fn map_handles<T>(self, mut convert: impl FnMut(H) -> T) -> VisibleObject<T> {
        VisibleObject {
            copied_values: self.copied_values,
            display_face: self.display_face,
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
            lore_counters: self.lore_counters,
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
pub struct VisiblePermanentCopy {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_id: Option<String>,
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
    pub(in crate::engine) fn convert<T: for<'a> From<&'a O>>(self) -> VisibleTarget<T> {
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
    pub(super) fn map_handles<T>(self, mut convert: impl FnMut(H) -> T) -> VisibleStackTarget<T> {
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
    pub(in crate::engine) fn convert<T: for<'a> From<&'a O>>(self) -> VisibleEntityTarget<T> {
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
#[serde(bound(deserialize = "H: Deserialize<'de>, O: Deserialize<'de>, E: Deserialize<'de>"))]
pub struct VisibleStackEntry<H = StackHandle, O = ObjectHandle, E = EffectHandle> {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bestowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_additional_costs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hybrid_colors: Vec<HybridPayment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana_value: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_modes: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggered: Option<VisibleTriggered<H, O, E>>,
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
    pub targets: Vec<VisibleTarget<O>>,
}
impl<H, O, E> VisibleStackEntry<H, O, E> {
    pub(super) fn map_handles<T, P: for<'a> From<&'a O>, F: for<'a> From<&'a E>>(
        self,
        mut convert: impl FnMut(H) -> T,
    ) -> VisibleStackEntry<T, P, F> {
        VisibleStackEntry {
            bestowed: self.bestowed,
            selected_additional_costs: self.selected_additional_costs,
            x: self.x,
            hybrid_colors: self.hybrid_colors,
            mana_value: self.mana_value,
            mode: self.mode,
            selected_modes: self.selected_modes,
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
    pub(in crate::engine) fn convert<T: for<'a> From<&'a O>>(self) -> VisibleAttacker<T> {
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
pub struct AdditionalCostChoice {
    pub id: String,
    pub mana: ManaCost,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerRequest {
    OpeningPlacement {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<ObjectHandle>,
    },
    GraveyardReplacementOrder {
        /// OptionalEffect(true) selects this alternative to exile. Omission selects the printed shuffle.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offered: Option<GraveyardOrderOption>,
        token: DecisionToken,
        player: PlayerId,
        object: ObjectHandle,
    },

    AdditionalCosts {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<AdditionalCostChoice>,
    },

    EntryCopy {
        token: DecisionToken,
        player: PlayerId,
        source: ObjectHandle,
        candidates: Vec<ObjectHandle>,
    },
    EntryAttachment {
        token: DecisionToken,
        player: PlayerId,
        source: ObjectHandle,
        candidates: Vec<ObjectHandle>,
    },

    LegendRule {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<ObjectHandle>,
        previous_choices: Vec<ObjectHandle>,
    },
    HybridCost {
        token: DecisionToken,
        player: PlayerId,
        symbols: Vec<ColorHybridSymbol>,
    },
    AnnounceX {
        token: DecisionToken,
        player: PlayerId,
    },
    SacrificeCost {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<ObjectHandle>,
    },
    ForcedSacrifice {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<ObjectHandle>,
        previous_choices: Vec<(PlayerId, ObjectHandle)>,
    },
    ChooseManaAbility {
        token: DecisionToken,
        player: PlayerId,
        choices: Vec<ManaAbilityIdentity>,
        mana_ability_source: ObjectHandle,
    },
    ManaCombination {
        token: DecisionToken,
        player: PlayerId,
        total: Quantity,
        colors: Vec<Color>,
    },
    ManaColor {
        token: DecisionToken,
        player: PlayerId,
        colors: Vec<Color>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mana_ability_source: Option<ObjectHandle>,
    },

    EntryReveal {
        token: DecisionToken,
        player: PlayerId,
        source: ObjectHandle,
        candidates: Vec<ObjectHandle>,
    },

    OptionalEffect {
        token: DecisionToken,
        player: PlayerId,
    },

    Mulligan {
        token: DecisionToken,
        player: PlayerId,
        mulligans_taken: u8,
        bottom_if_mulligan: u8,
        can_mulligan: bool,
    },
    BottomOpening {
        token: DecisionToken,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectHandle>,
    },

    Modes {
        token: DecisionToken,
        player: PlayerId,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        one_or_more: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        allow_repeat: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exactly: Option<u32>,
        candidates: Vec<u32>,
    },
    CounterPlacementEffect {
        token: DecisionToken,
        player: PlayerId,
        recipient: ObjectHandle,
        candidates: Vec<VisibleCounterPlacementCandidate>,
    },
    DamageEffect {
        token: DecisionToken,
        player: PlayerId,
        recipient: PlayerTargetEntity,
        candidates: Vec<VisibleDamageEffectCandidate>,
    },
    OrderTriggers {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<VisiblePendingTrigger>,
    },
    EntityTargets {
        token: DecisionToken,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<PlayerTargetEntity>,
    },
    StackTargets {
        token: DecisionToken,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<StackHandle>,
    },
    PlayerTargets {
        token: DecisionToken,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<PlayerId>,
    },
    Sacrifice {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<ObjectHandle>,
    },
    ResolveDiscard {
        token: DecisionToken,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectHandle>,
    },
    SpendMana {
        token: DecisionToken,
        player: PlayerId,
        cost: ManaCost,
        available: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        restricted_available: Vec<RestrictedMana>,
        /// The permanent whose mana ability this nested payment or color choice
        /// belongs to (CR 605.3a). Omitted outside nested mana-ability requests.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mana_ability_source: Option<ObjectHandle>,
    },
    CommanderReturn {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        previous_choices: Vec<VisibleCommanderReturnChoice>,
        token: DecisionToken,
        player: PlayerId,
        object: Option<ObjectHandle>,
        designation: String,
        destination: Zone,
    },
    Attackers {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<ObjectHandle>,
        defenders: Vec<PlayerId>,
    },
    Blockers {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        menace_attackers: Vec<ObjectHandle>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        legal_attackers: Vec<LegalBlockers>,
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<ObjectHandle>,
        attackers: Vec<ObjectHandle>,
    },
    Damage {
        token: DecisionToken,
        player: PlayerId,
        source: ObjectHandle,
        power: Quantity,
        recipients: Vec<ObjectHandle>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        player_recipient: Option<PlayerId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lethal_requirements: Vec<(ObjectHandle, Quantity)>,
    },
    Discard {
        token: DecisionToken,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectHandle>,
    },
    Priority {
        token: DecisionToken,
        player: PlayerId,
    },
    Targets {
        token: DecisionToken,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter_division: Option<CounterDivision>,
        candidates: Vec<ObjectHandle>,
    },
    Mana {
        token: DecisionToken,
        player: PlayerId,
        cost: ManaCost,
        /// The permanent whose mana ability this nested payment or color choice
        /// belongs to (CR 605.3a). Omitted outside nested mana-ability requests.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mana_ability_source: Option<ObjectHandle>,
    },
}
impl PlayerRequest {
    pub fn token(&self) -> &DecisionToken {
        match self {
            Self::EntryCopy { token, .. } | Self::EntryAttachment { token, .. } => token,
            Self::LegendRule { token, .. }
            | Self::ChooseManaAbility { token, .. }
            | Self::ManaCombination { token, .. }
            | Self::ManaColor { token, .. }
            | Self::EntryReveal { token, .. }
            | Self::OptionalEffect { token, .. }
            | Self::Mulligan { token, .. }
            | Self::OpeningPlacement { token, .. }
            | Self::BottomOpening { token, .. }
            | Self::AdditionalCosts { token, .. }
            | Self::HybridCost { token, .. }
            | Self::AnnounceX { token, .. }
            | Self::Modes { token, .. }
            | Self::CounterPlacementEffect { token, .. }
            | Self::DamageEffect { token, .. }
            | Self::OrderTriggers { token, .. }
            | Self::EntityTargets { token, .. }
            | Self::StackTargets { token, .. }
            | Self::PlayerTargets { token, .. }
            | Self::Sacrifice { token, .. }
            | Self::SacrificeCost { token, .. }
            | Self::ForcedSacrifice { token, .. }
            | Self::ResolveDiscard { token, .. }
            | Self::SpendMana { token, .. }
            | Self::GraveyardReplacementOrder { token, .. }
            | Self::CommanderReturn { token, .. }
            | Self::Attackers { token, .. }
            | Self::Blockers { token, .. }
            | Self::Damage { token, .. }
            | Self::Discard { token, .. }
            | Self::Priority { token, .. }
            | Self::Targets { token, .. }
            | Self::Mana { token, .. } => token,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_context: Option<DecisionContext>,
    pub observation_version: u32,
    pub view: PermittedView,
    pub decision: Option<PlayerRequest>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerResponse {
    OpeningPlacement {
        object: Option<ObjectHandle>,
    },
    AdditionalCosts {
        selected: Vec<String>,
    },

    EntryCopy {
        object: Option<ObjectHandle>,
    },
    EntryAttachment {
        object: ObjectHandle,
    },

    LegendRule {
        keep: ObjectHandle,
    },
    HybridCost {
        colors: Vec<HybridPayment>,
    },
    AnnounceX {
        value: Quantity,
    },
    SacrificeCost {
        object: ObjectHandle,
    },
    ForcedSacrifice {
        object: ObjectHandle,
    },
    ChooseManaAbility {
        ability: ManaAbilityIdentity,
    },
    ManaCombination {
        amounts: [Quantity; 5],
    },
    ManaColor {
        color: Color,
    },

    EntryReveal {
        card: Option<ObjectHandle>,
    },

    OptionalEffect {
        perform: bool,
    },

    Mulligan {
        take: bool,
    },
    BottomOpening {
        objects: Vec<ObjectHandle>,
    },

    ModeSubset {
        modes: Vec<u32>,
    },
    Modes {
        mode: u32,
    },
    CounterPlacementEffect {
        effect: CounterPlacementEffectHandle,
    },
    DamageEffect {
        effect: EffectHandle,
        damage: DamageHandle,
    },
    OrderTriggers {
        order: Vec<TriggerHandle>,
    },
    EntityTargets {
        entities: Vec<PlayerTargetEntity>,
    },
    StackTargets {
        entries: Vec<StackHandle>,
    },
    ActivateAbility {
        object: ObjectHandle,
        ability: u32,
    },
    PlayerTargets {
        players: Vec<PlayerId>,
    },
    Sacrifice {
        object: ObjectHandle,
    },
    ResolveDiscard {
        objects: Vec<ObjectHandle>,
    },
    SpendMana {
        spending: BTreeMap<Color, Quantity>,
    },
    SpendManaAllocated {
        spending: BTreeMap<Color, Quantity>,
        restricted: Vec<RestrictedMana>,
    },
    CommanderReturn {
        to_command: bool,
    },
    Attackers {
        attacks: Vec<(ObjectHandle, PlayerId)>,
    },
    Blockers {
        blocks: Vec<(ObjectHandle, ObjectHandle)>,
    },
    Damage {
        assignments: Vec<(ObjectHandle, Quantity)>,
    },
    TrampleDamage {
        assignments: Vec<(ObjectHandle, Quantity)>,
        player: PlayerId,
        amount: Quantity,
    },
    Discard {
        objects: Vec<ObjectHandle>,
    },
    PlayLand {
        object: ObjectHandle,
    },
    PlayBackLand {
        object: ObjectHandle,
    },
    ActivateMana {
        object: ObjectHandle,
    },
    Bestow {
        card: ObjectHandle,
    },
    Cast {
        card: ObjectHandle,
    },
    Pass,
    Targets {
        objects: Vec<ObjectHandle>,
    },
    FinishMana {
        undo_mana: bool,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenDecklist {
    pub player: PlayerId,
    pub definitions: BTreeMap<String, u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchContext {
    pub context_version: u32,
    pub observation: Observation,
    pub permitted_history: PermittedHistory,
    pub open_decklists: Vec<OpenDecklist>,
    pub catalog_digest: String,
    pub profile_id: String,
    pub sampling_seed: String,
}

// Only the forward map is authoritative. The reverse index preserves the old
// lexical-first lookup even for duplicate IDs in an untrusted snapshot; replay
// still proves whether that snapshot is reachable. Every mutation invalidates
// the derived index, and clones may share it while their maps remain unchanged.
// Staged forks and per-viewer publication copies share the forward map until
// one of them mints or retires a handle; the wire form is the map itself.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectHandles {
    #[serde(with = "shared_entries")]
    pub entries: Arc<BTreeMap<HandleId<b'o'>, ObjectId>>,
    #[serde(skip)]
    pub reverse: Arc<std::sync::OnceLock<BTreeMap<ObjectId, HandleId<b'o'>>>>,
}
mod shared_entries {
    use super::*;
    pub(in crate::engine) fn serialize<S: serde::Serializer>(
        value: &Arc<BTreeMap<HandleId<b'o'>, ObjectId>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_ref().serialize(serializer)
    }
    pub(in crate::engine) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<BTreeMap<HandleId<b'o'>, ObjectId>>, D::Error> {
        BTreeMap::deserialize(deserializer).map(Arc::new)
    }
}
impl PartialEq for ObjectHandles {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}
impl Eq for ObjectHandles {}
impl ObjectHandles {
    pub fn handle_for(&self, object: ObjectId) -> Option<&HandleId<b'o'>> {
        self.reverse
            .get_or_init(|| {
                let mut reverse = BTreeMap::new();
                for (handle, object) in self.entries.iter() {
                    reverse.entry(*object).or_insert_with(|| handle.clone());
                }
                reverse
            })
            .get(&object)
    }
    fn get(&self, handle: &str) -> Option<&ObjectId> {
        self.entries.get(&HandleId::parse(handle))
    }
    pub fn insert(&mut self, handle: HandleId<b'o'>, object: ObjectId) {
        // An unchanged row leaves the shared map and its index alone.
        if self.entries.get(&handle) == Some(&object) {
            return;
        }
        Arc::make_mut(&mut self.entries).insert(handle, object);
        self.reverse = Arc::default();
    }
    pub fn retain_objects(&mut self, current: &BTreeSet<ObjectId>) {
        // Unshare only when a row actually leaves.
        if self.entries.values().all(|object| current.contains(object)) {
            return;
        }
        Arc::make_mut(&mut self.entries).retain(|_, object| current.contains(object));
        self.reverse = Arc::default();
    }
    fn objects(&self) -> BTreeSet<ObjectId> {
        self.entries.values().copied().collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Knowledge {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub revealed_hand: BTreeSet<ObjectId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub public_tucked_cards: BTreeSet<ObjectId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub own_knowledge: Vec<OwnKnowledgeAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(in crate::engine) effect_handles: Vec<(EffectKey, HandleId<b'e'>)>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(in crate::engine) next_effect_handle: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(in crate::engine) damage_handles: Vec<((DamageTransactionId, DamageId), DamageHandle)>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(in crate::engine) next_damage_handle: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    trigger_handles: BTreeMap<TriggerId, TriggerHandle>,
    #[serde(default, skip_serializing_if = "is_zero")]
    next_trigger_handle: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    ability_handles: BTreeMap<StackId, HandleId<b's'>>,
    #[serde(default, skip_serializing_if = "is_zero")]
    next_ability_handle: u64,
    pub handles: ObjectHandles,
    next_handle: u64,
    decision_key: Option<(u64, u8)>,
    decision_token: Option<DecisionToken>,
    next_decision: u64,
    pub history: super::stored_history::StoredHistory,
}
fn scoped(namespace: &Option<String>, handle: String) -> String {
    namespace.as_ref().map_or_else(
        || handle.clone(),
        |namespace| format!("{namespace}:{handle}"),
    )
}
impl Knowledge {
    /// The key of this viewer's current decision token, if they owe one.
    pub(in crate::engine) fn decision_key(&self) -> Option<(u64, u8)> {
        self.decision_key
    }
    /// Mints a fresh token when the viewer's decision changes, and retires it
    /// when they owe none. An unchanged key keeps its token.
    pub(in crate::engine) fn set_decision_key(
        &mut self,
        viewer: usize,
        key: Option<(u64, u8)>,
    ) -> Result<(), String> {
        if key != self.decision_key {
            self.decision_token = if key.is_some() {
                let token =
                    DecisionToken(self.scoped(format!("v{viewer}-d{}", self.next_decision)));
                self.next_decision = self
                    .next_decision
                    .checked_add(1)
                    .ok_or("decision token resource limit")?;
                Some(token)
            } else {
                None
            };
            self.decision_key = key;
        }
        Ok(())
    }
    pub(in crate::engine) fn scoped(&self, handle: String) -> String {
        self.namespace.as_ref().map_or_else(
            || handle.clone(),
            |namespace| format!("{namespace}:{handle}"),
        )
    }
}
// Scratch for one publication or one query's Work. Rules are stable while the
// four observers' handles and histories are updated, and for as long as a
// Work's state token is current. Lists preserve object-table order.
#[derive(Default, Clone)]
pub struct ProjectionIndex {
    outside_libraries: Vec<CardId>,
    hand_counts: [usize; 4],
    library_counts: [usize; 4],
    attachments: BTreeMap<ObjectId, Vec<ObjectId>>,
    static_sources: Vec<CardId>,
}
// Derived publication data. The index and characteristics are shared by forks
// and by all four viewers. Each is carried forward only across its dependencies.
#[derive(Clone, Default)]
pub(super) struct ProjectionCache {
    layer_epoch: u64,
    index: Arc<ProjectionIndex>,
    cores: Arc<BTreeMap<ObjectId, Arc<ObjectCore>>>,
}

type ObjectCores = std::cell::RefCell<Arc<BTreeMap<ObjectId, Arc<ObjectCore>>>>;
#[derive(Clone)]
struct ObjectCore {
    keywords: Vec<KeywordAbility>,
    restrictions: Vec<CreatureRestriction>,
    token: Option<VisibleToken>,
    mana_ability_used: Option<bool>,
    characteristics: Characteristics,
}

impl RulesTxn {
    // Keep unresolved sources so the rules layer still decides when to skip
    // or propagate a lookup error.
    fn indexed_static_source(&self, object: &Object) -> bool {
        object.present_on_battlefield()
            && !self
                .copiable_definition(object.id)
                .is_ok_and(|definition| definition.static_abilities.is_empty())
    }
    /// Object-table-ordered sources for `*_with_sources`, valid only while
    /// the rules state it was read from is unchanged.
    pub(in crate::engine) fn static_source_index(&self) -> Vec<CardId> {
        self.state
            .rules
            .objects
            .iter()
            .filter(|(_, object)| self.indexed_static_source(object))
            .map(|(card, _)| card)
            .collect()
    }
    pub(in crate::engine) fn projection_index(&self) -> ProjectionIndex {
        let mut index = ProjectionIndex::default();
        for (card, object) in &self.state.rules.objects {
            if !matches!(object.zone, Zone::Library { .. }) {
                index.outside_libraries.push(card);
            }
            match object.zone {
                Zone::Hand { owner } => {
                    if let Some(count) = index.hand_counts.get_mut(owner.0 as usize) {
                        *count += 1;
                    }
                }
                Zone::Library { owner } => {
                    if let Some(count) = index.library_counts.get_mut(owner.0 as usize) {
                        *count += 1;
                    }
                }
                Zone::Battlefield => {
                    // Phased-out attachments are still shown in the view.
                    if let Some(host) = object.attached_to {
                        index.attachments.entry(host).or_default().push(object.id);
                    }
                    if self.indexed_static_source(object) {
                        index.static_sources.push(card);
                    }
                }
                _ => {}
            }
        }
        index
    }

    /// Update the projection index in table order. Tapping or damaging an
    /// existing permanent typically changes no index entry at all.
    fn update_projection_index(
        &self,
        old: &Published,
        dirty: &BTreeSet<CardId>,
    ) -> Arc<ProjectionIndex> {
        fn member<T: Ord>(rows: &mut Vec<T>, value: T, present: bool) {
            match (rows.binary_search(&value), present) {
                (Err(i), true) => rows.insert(i, value),
                (Ok(i), false) => {
                    rows.remove(i);
                }
                _ => {}
            }
        }
        fn count(index: &mut ProjectionIndex, zone: Zone, entering: bool) {
            let slot = match zone {
                Zone::Hand { owner } => index.hand_counts.get_mut(owner.0 as usize),
                Zone::Library { owner } => index.library_counts.get_mut(owner.0 as usize),
                _ => None,
            };
            if let Some(n) = slot {
                if entering {
                    *n += 1;
                } else {
                    *n -= 1;
                }
            }
        }
        let mut index = old.cache.index.clone();
        for card in dirty {
            let was = old.rules.objects.get(card);
            let now = self.state.rules.objects.get(card);
            let old_zone = was.map(|o| o.zone);
            let new_zone = now.map(|o| o.zone);
            if old_zone != new_zone {
                let index = Arc::make_mut(&mut index);
                if let Some(zone) = old_zone {
                    count(index, zone, false);
                }
                if let Some(zone) = new_zone {
                    count(index, zone, true);
                }
                member(
                    &mut index.outside_libraries,
                    *card,
                    new_zone.is_some_and(|zone| !matches!(zone, Zone::Library { .. })),
                );
            }
            let link = |o: &Object| {
                if o.zone == Zone::Battlefield {
                    o.attached_to.map(|host| (host, o.id))
                } else {
                    None
                }
            };
            let before = was.and_then(link);
            let after = now.and_then(link);
            if before != after {
                let attachments = &mut Arc::make_mut(&mut index).attachments;
                if let Some((host, id)) = before
                    && let Some(rows) = attachments.get_mut(&host)
                {
                    member(rows, id, false);
                    if rows.is_empty() {
                        attachments.remove(&host);
                    }
                }
                if let Some((host, id)) = after {
                    member(attachments.entry(host).or_default(), id, true);
                }
            }
            let is_static = now.is_some_and(|object| self.indexed_static_source(object));
            if index.static_sources.binary_search(card).is_ok() != is_static {
                member(
                    &mut Arc::make_mut(&mut index).static_sources,
                    *card,
                    is_static,
                );
            }
        }
        index
    }

    pub(crate) fn capture_trigger_source_handles(
        &mut self,
        object: ObjectId,
    ) -> Result<Vec<publish::StoredObjectHandle>, String> {
        let mut handles = vec![];
        for viewer in 0..4 {
            let k = &mut self.state.knowledge.visibility[viewer];
            let handle = if let Some(handle) = k.handles.handle_for(object) {
                handle.clone()
            } else {
                let handle = HandleId::new(viewer as u8, k.next_handle, k.namespace.as_deref());
                k.next_handle = k
                    .next_handle
                    .checked_add(1)
                    .ok_or("trigger source handle limit")?;
                k.handles.insert(handle.clone(), object);
                handle
            };
            handles.push(handle);
        }
        Ok(handles)
    }
    pub(crate) fn resolve_trigger_handle(
        &self,
        viewer: PlayerId,
        handle: &TriggerHandle,
    ) -> Result<TriggerId, String> {
        self.state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| {
                k.trigger_handles
                    .iter()
                    .find(|(_, h)| *h == handle)
                    .map(|(id, _)| *id)
            })
            .ok_or_else(|| "invalid trigger handle".into())
    }
    pub(crate) fn visible_pending_trigger(
        &self,
        viewer: PlayerId,
        id: TriggerId,
    ) -> Result<VisiblePendingTrigger, String> {
        let trigger = self
            .state
            .rules
            .pending_triggers
            .iter()
            .find(|t| t.id == id)
            .ok_or("missing pending trigger")?;
        Ok(VisiblePendingTrigger {
            label: self
                .trigger_source_label(trigger.source, trigger.ability)
                .map(str::to_owned),
            handle: self
                .state
                .knowledge
                .visibility
                .get(viewer.0 as usize)
                .and_then(|k| k.trigger_handles.get(&id))
                .ok_or("missing trigger handle")?
                .clone(),
            source: VisibleTarget {
                announced: trigger
                    .source_handles
                    .get(viewer.0 as usize)
                    .ok_or("missing trigger source reference")?
                    .into(),
                current: self.handle_for(viewer, trigger.source).ok(),
            },
            oracle_id: self.visible_ability_oracle_id(trigger.source)?.into(),
            ability: Self::visible_trigger_ability(trigger.ability),
        })
    }

    fn visible_trigger_ability<O, E>(
        ability: EffectiveTriggerAbility,
    ) -> VisibleTriggerAbility<O, E> {
        match ability {
            EffectiveTriggerAbility::Intrinsic { index } => {
                VisibleTriggerAbility::Intrinsic { index }
            }
        }
    }
    pub(crate) fn player_entity(
        &self,
        viewer: PlayerId,
        target: TargetEntity,
    ) -> Result<PlayerTargetEntity, String> {
        match target {
            TargetEntity::Player { player } => Ok(PlayerTargetEntity::Player { player }),
            TargetEntity::Object { object } => Ok(PlayerTargetEntity::Object {
                object: self.handle_for(viewer, object)?,
            }),
        }
    }
    pub(crate) fn resolve_entity(
        &self,
        viewer: PlayerId,
        target: &PlayerTargetEntity,
    ) -> Result<TargetEntity, String> {
        match target {
            PlayerTargetEntity::Player { player } => Ok(TargetEntity::Player { player: *player }),
            PlayerTargetEntity::Object { object } => Ok(TargetEntity::Object {
                object: self.resolve_handle(viewer, object)?,
            }),
        }
    }

    // CR 400.2, 401.2, 402.3: own hand is visible; all libraries and other
    // hands remain hidden. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn visible_to(&self, viewer: PlayerId, object: &Object) -> bool {
        if match object.zone {
            Zone::Hand { owner } => owner == viewer,
            Zone::Library { .. } => false,
            _ => true,
        } {
            return true;
        }
        // CR602.2a/701.20a: the validated hand-source announcement reveals
        // this incarnation until its ability leaves the stack, including the
        // unpaid mana window. A successor or another hand card never inherits
        // this authority. Cancellation ends current visibility; the existing
        // public observation journal preserves legitimately announced identity.
        if self.state.rules.stack.iter().any(|entry| {
            entry.source == object.id
                && entry.cast_from == object.zone
                && matches!(entry.cast_from, Zone::Hand { owner } if owner == entry.controller)
                && matches!(&entry.kind, StackKind::Activated { ability, .. }
                    if self.definition(object.id.card).is_ok_and(|d| d.activated.get(*ability as usize).is_some_and(hand_discard_draw_activation)))
        }) {
            return true;
        }
        // CR 701.20a-b: only this selected set is temporarily public; it
        // remains in the library, without publishing positions or other candidates.

        if self.replacement_reveals(object.id) {
            return true;
        }
        false
    }
    pub(crate) fn stack_handle_for(
        &self,
        viewer: PlayerId,
        id: StackId,
    ) -> Result<StackHandle, String> {
        self.stack_handle_id(viewer, id).map(StackHandle::from)
    }

    pub(in crate::engine) fn stack_handle_id(
        &self,
        viewer: PlayerId,
        id: StackId,
    ) -> Result<&HandleId<b's'>, String> {
        if !self.state.rules.stack.iter().any(|entry| entry.id == id) {
            return Err("stale stack identity".into());
        }
        self.state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|knowledge| knowledge.ability_handles.get(&id))
            .ok_or_else(|| "stack object is not visible".into())
    }
    fn resolve_stack_handle(
        &self,
        viewer: PlayerId,
        handle: &StackHandle,
    ) -> Result<StackId, String> {
        let handle = HandleId::parse(&handle.0);
        self.state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|knowledge| {
                knowledge
                    .ability_handles
                    .iter()
                    .find(|(_, value)| **value == handle)
            })
            .map(|(id, _)| *id)
            .filter(|id| self.state.rules.stack.iter().any(|entry| entry.id == *id))
            .ok_or_else(|| "stale or foreign stack handle".into())
    }
    // Unindexed reference projection, kept for the frozen query oracle.
    #[cfg(feature = "testkit")]
    pub(crate) fn visible_player(&self, player: PlayerId) -> Result<VisiblePlayer, String> {
        self.project_player(player, None)
    }
    /// `visible_player` for seats 0..4, with an index of this unchanged state.
    pub(in crate::engine) fn visible_player_indexed(
        &self,
        player: PlayerId,
        index: &ProjectionIndex,
    ) -> Result<VisiblePlayer, String> {
        self.project_player(player, Some(index))
    }

    fn project_player<O>(
        &self,
        player: PlayerId,
        index: Option<&ProjectionIndex>,
    ) -> Result<VisiblePlayer<O>, String> {
        let p = self
            .state
            .rules
            .players
            .get(player.0 as usize)
            .ok_or("invalid viewer")?;
        let count = |zone| {
            self.state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == zone)
                .count()
        };
        let (hand_count, library_count) = match index {
            Some(index) => (
                index.hand_counts[player.0 as usize],
                index.library_counts[player.0 as usize],
            ),
            None => (
                count(Zone::Hand { owner: player }),
                count(Zone::Library { owner: player }),
            ),
        };
        let checked_count =
            |count| u32::try_from(count).map_err(|_| "zone count resource limit".to_string());
        Ok(VisiblePlayer {
            life_frozen_until_next_turn: p.life_frozen_until_next_turn,
            exile: vec![],
            library_shuffles: self
                .state
                .rules
                .match_state
                .as_ref()
                .and_then(|m| m.library_shuffles.get(player.0 as usize))
                .copied()
                .unwrap_or_default(),
            player,
            status: p.status,
            life: p.life.clone(),
            mana: p.mana.clone(),
            restricted_mana: p.restricted_mana.clone(),
            hand_count: checked_count(hand_count)?,
            library_count: checked_count(library_count)?,
        })
    }
    pub(crate) fn object_handle_id(
        &self,
        viewer: PlayerId,
        id: ObjectId,
    ) -> Result<&HandleId<b'o'>, String> {
        self.state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| k.handles.handle_for(id))
            .ok_or_else(|| "object is not visible to this viewer".into())
    }

    pub(crate) fn handle_for(
        &self,
        viewer: PlayerId,
        id: ObjectId,
    ) -> Result<ObjectHandle, String> {
        self.object_handle_id(viewer, id).map(ObjectHandle::from)
    }
    pub(in crate::engine) fn visible_ability_oracle_id(
        &self,
        object: ObjectId,
    ) -> Result<&str, String> {
        let effective = self.copiable_definition(object)?;
        if !effective.oracle_id.starts_with("token:") {
            return Ok(&effective.oracle_id);
        }
        let original = self.definition(object.card)?;
        Ok(if original.oracle_id.starts_with("token:") {
            ""
        } else {
            &original.oracle_id
        })
    }
    // Unindexed reference projection, kept for the frozen query oracle.
    #[cfg(feature = "testkit")]
    pub(crate) fn visible_object(
        &self,
        viewer: PlayerId,
        object: &Object,
    ) -> Result<Option<VisibleObject>, String> {
        if !self.visible_to(viewer, object) {
            return Ok(None);
        }
        self.project_visible_object(viewer, object, None, None)
            .map(Some)
    }
    /// `visible_object`, with an index of this unchanged state.
    pub(in crate::engine) fn visible_object_indexed(
        &self,
        viewer: PlayerId,
        object: &Object,
        index: &ProjectionIndex,
    ) -> Result<Option<VisibleObject>, String> {
        if !self.visible_to(viewer, object) {
            return Ok(None);
        }
        self.project_visible_object(viewer, object, Some(index), None)
            .map(Some)
    }

    fn project_visible_object<H>(
        &self,
        viewer: PlayerId,
        object: &Object,
        index: Option<&ProjectionIndex>,
        cores: Option<&ObjectCores>,
    ) -> Result<VisibleObject<H>, String>
    where
        H: for<'a> From<&'a HandleId<b'o'>> + for<'a> From<&'a ObjectHandle>,
    {
        let handle = |id| self.object_handle_id(viewer, id).map(H::from);
        let physical = self
            .state
            .rules
            .cards
            .get(&object.id.card)
            .ok_or("missing object definition")?;
        if let Some(link) = &object.hideaway {
            return Ok(VisibleObject {
                copied_values: None,
                display_face: None,
                entered_this_turn: false,
                face_down: true,
                lore_counters: Quantity::zero(),
                exiled_by: Some(H::from(link.source_handle(viewer))),
                oracle_id: if self.is_live(viewer) && link.may_look(viewer) {
                    physical.oracle_id.clone()
                } else {
                    String::new()
                },
                handle: handle(object.id)?,
                owner: physical.owner,
                controller: object.controller,
                zone: object.zone,
                characteristics: None,
                mana_ability_used: None,
                token: None,
                keywords: vec![],
                restrictions: vec![],
                attachments: vec![],
                attached_to: None,
                marked_damage: Quantity::zero(),
                plus_one_counters: Quantity::zero(),
                attack_ready: false,
                tapped: false,
                phased_out: false,
            });
        }
        let copied_values = None;
        let attached_to = object
            .attached_to
            .filter(|t| self.current(*t).is_ok())
            .map(handle)
            .transpose()?;
        let attachments = match index {
            Some(index) => index
                .attachments
                .get(&object.id)
                .into_iter()
                .flatten()
                .map(|id| handle(*id))
                .collect::<Result<_, _>>()?,
            None => self
                .state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == Zone::Battlefield && o.attached_to == Some(object.id))
                .map(|o| handle(o.id))
                .collect::<Result<_, _>>()?,
        };
        // Same position as the unshared projection: after this viewer's
        // attachment handles and before its own handle.
        let ObjectCore {
            keywords,
            restrictions,
            token,
            mana_ability_used,
            characteristics,
        } = match cores {
            Some(cores) => {
                let mut cores = cores.borrow_mut();
                if let Some(core) = cores.get(&object.id) {
                    core.as_ref().clone()
                } else {
                    let core = self.object_core(object, index)?;
                    Arc::make_mut(&mut cores).insert(object.id, Arc::new(core.clone()));
                    core
                }
            }
            None => self.object_core(object, index)?,
        };
        Ok(VisibleObject {
            display_face: None,
            copied_values,
            face_down: false,
            exiled_by: None,
            phased_out: object.phased_out.is_some(),
            entered_this_turn: object.battlefield_entry_turn == Some(self.state.rules.turn.number),
            attached_to,
            attachments,
            keywords,
            restrictions,
            token,
            mana_ability_used,
            plus_one_counters: object.plus_one_counters.clone(),
            lore_counters: Quantity::zero(),
            characteristics: Some(characteristics),
            marked_damage: object.marked_damage.clone(),
            attack_ready: object.attack_ready,
            handle: handle(object.id)?,
            // A copy can show the original printed Oracle ID. An instruction-defined
            // token has only its public handle and visible token template.
            oracle_id: if self
                .state
                .rules
                .tokens
                .get(&object.id.card)
                .is_some_and(|t| !t.is_copy())
            {
                String::new()
            } else {
                physical.oracle_id.clone()
            },
            owner: physical.owner,
            controller: object.controller,
            zone: object.zone,
            tapped: object.tapped,
        })
    }
    // Rows of a visible, face-up object that no viewer's handles enter, in the
    // projection's evaluation order. Rules-only: publication changes player
    // knowledge between viewers, never an input of these derivations.
    fn object_core(
        &self,
        object: &Object,
        index: Option<&ProjectionIndex>,
    ) -> Result<ObjectCore, String> {
        let static_sources = index.map(|index| index.static_sources.as_slice());
        Ok(ObjectCore {
            keywords: if object.zone == Zone::Battlefield {
                let keywords = self.current_keywords_with_sources(object.id, static_sources);
                if self.target_is_creature(object.id)
                    || keywords.contains(&KeywordAbility::Indestructible)
                {
                    keywords
                } else {
                    vec![]
                }
            } else {
                vec![]
            },
            restrictions: {
                let current: Vec<_> = self
                    .creature_restrictions_with_sources(object.id, static_sources)
                    .collect();
                [
                    CreatureRestriction::CantBeBlocked,
                    CreatureRestriction::CantAttackOrBlock,
                    CreatureRestriction::CantBlock,
                    CreatureRestriction::CanBlockOnlyFlying,
                ]
                .into_iter()
                .filter(|r| current.contains(r))
                .collect()
            },
            token: self
                .state
                .rules
                .tokens
                .get(&object.id.card)
                .filter(|t| t.is_token(object))
                .map(|t| VisibleToken {
                    name: t.definition.name.clone(),
                    keywords: t.definition.keywords.clone(),
                }),
            mana_ability_used: self
                .copiable_definition(object.id)?
                .mana_abilities
                .first()
                .and_then(|a| a.restriction.map(|_| self.mana_ability_used(object.id))),
            characteristics: self.characteristics_with_sources(object.id, static_sources)?,
        })
    }
    pub(crate) fn visible_stack(
        &self,
        viewer: PlayerId,
        index: usize,
    ) -> Result<VisibleStackEntry, String> {
        self.project_stack_entry(viewer, index)
    }

    fn project_stack_entry<
        H: for<'a> From<&'a HandleId<b's'>>,
        O: for<'a> From<&'a HandleId<b'o'>>,
        E: for<'a> From<&'a HandleId<b'e'>>,
    >(
        &self,
        viewer: PlayerId,
        index: usize,
    ) -> Result<VisibleStackEntry<H, O, E>, String> {
        let instance = self
            .state
            .rules
            .stack
            .get(index)
            .ok_or("missing stack projection row")?;
        let announced = instance
            .announced_targets
            .get(viewer.0 as usize)
            .ok_or("missing announcement viewer")?;
        let targets = instance
            .targets
            .iter()
            .enumerate()
            .map(|(slot, target)| {
                Ok(VisibleTarget {
                    announced: announced
                        .get(slot)
                        .ok_or("missing announced target handle")?
                        .into(),
                    current: self.object_handle_id(viewer, *target).map(O::from).ok(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (spell, ability, triggered) = match &instance.kind {
            StackKind::Spell => (
                Some(
                    self.object_handle_id(viewer, instance.source)
                        .map(O::from)?,
                ),
                None,
                None,
            ),
            StackKind::Triggered {
                ability,
                source_handles,
                ..
            } => (
                None,
                None,
                Some(VisibleTriggered {
                    handle: H::from(self.stack_handle_id(viewer, instance.id)?),
                    source: VisibleTarget {
                        announced: source_handles
                            .get(viewer.0 as usize)
                            .ok_or("missing triggered source reference")?
                            .into(),
                        current: self
                            .object_handle_id(viewer, instance.source)
                            .map(O::from)
                            .ok(),
                    },
                    oracle_id: self.visible_ability_oracle_id(instance.source)?.into(),
                    ability: Self::visible_trigger_ability(*ability),
                }),
            ),
            StackKind::Activated {
                ability,
                source_handles,
            } => (
                None,
                Some(VisibleAbility {
                    handle: H::from(self.stack_handle_id(viewer, instance.id)?),
                    source: VisibleTarget {
                        announced: source_handles
                            .get(viewer.0 as usize)
                            .ok_or("missing announced ability source")?
                            .into(),
                        current: self
                            .object_handle_id(viewer, instance.source)
                            .map(O::from)
                            .ok(),
                    },
                    oracle_id: self.visible_ability_oracle_id(instance.source)?.into(),
                    ability: *ability,
                }),
                None,
            ),
        };
        let stack_targets = instance
            .stack_targets
            .iter()
            .enumerate()
            .map(|(slot, target)| {
                Ok(VisibleStackTarget {
                    announced: H::from(
                        instance
                            .announced_stack_targets
                            .get(viewer.0 as usize)
                            .and_then(|entries| entries.get(slot))
                            .ok_or("missing announced stack target")?,
                    ),
                    current: self.stack_handle_id(viewer, *target).ok().map(H::from),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let entity_targets = instance
            .entity_targets
            .iter()
            .enumerate()
            .map(|(slot, target)| match target {
                TargetEntity::Player { player } => {
                    Ok(VisibleEntityTarget::Player { player: *player })
                }
                TargetEntity::Object { object } => Ok(VisibleEntityTarget::Object {
                    object: VisibleTarget {
                        announced: instance
                            .announced_entity_targets
                            .get(viewer.0 as usize)
                            .and_then(|row| row.get(slot))
                            .and_then(Option::as_ref)
                            .ok_or("missing entity announcement")?
                            .into(),
                        current: self.object_handle_id(viewer, *object).map(O::from).ok(),
                    },
                }),
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(VisibleStackEntry {
            bestowed: false,
            selected_additional_costs: instance.selected_additional_costs.clone(),
            x: instance.x.clone(),
            hybrid_colors: instance.hybrid_colors.clone(),
            mana_value: instance
                .x
                .as_ref()
                .map(|_| self.stack_mana_value(instance))
                .transpose()?,
            mode: None,
            selected_modes: vec![],
            triggered,
            entity_targets,
            handle: { Some(H::from(self.stack_handle_id(viewer, instance.id)?)) },
            stack_targets,
            spell,
            ability,
            player_targets: instance.player_targets.clone(),
            controller: instance.controller,
            cast_complete: instance.paid.is_some(),
            spell_copy: self.is_spell_copy(instance),
            targets,
        })
    }
    pub(crate) fn blank_view(&self, viewer: PlayerId) -> Result<PermittedView, String> {
        self.view_header(viewer)
    }
    fn view_header<O: for<'a> From<&'a HandleId<b'o'>>>(
        &self,
        viewer: PlayerId,
    ) -> Result<PermittedView<O>, String> {
        if viewer.0 >= 4 {
            return Err("invalid viewer".into());
        }
        Ok(PermittedView {
            known_searched_hand: self.publicly_searched_hand(viewer)?,
            known_library_positions: self.known_library_positions(viewer)?,
            sacrifice_choices: if let Frame::ForcedSacrifice { choices, .. } =
                &self.state.rules.frame
            {
                choices
                    .iter()
                    .filter(|(p, o)| self.is_live(*p) && self.current(*o).is_ok())
                    .map(|(p, o)| Ok((*p, self.object_handle_id(viewer, *o).map(O::from)?)))
                    .collect::<Result<_, String>>()?
            } else {
                vec![]
            },
            setup: self.visible_setup(),
            effects: vec![],
            combat: self
                .state
                .rules
                .combat
                .iter()
                .map(|a| {
                    Ok(VisibleAttacker {
                        attacker: self.object_handle_id(viewer, a.attacker).map(O::from)?,
                        defender: a.defender,
                        blocked: a.blocked,
                        blockers: a
                            .blockers
                            .iter()
                            .map(|b| self.object_handle_id(viewer, *b).map(O::from))
                            .collect::<Result<_, _>>()?,
                    })
                })
                .collect::<Result<_, String>>()?,
            viewer,
            active: self.state.rules.active,
            turn_owner: self.state.rules.turn_owner,
            turn: self.state.rules.turn.clone(),
            live_turn_order: self.live_players(),
            commanders: self
                .state
                .rules
                .commanders
                .records
                .iter()
                .enumerate()
                .map(|(index, (card, history))| VisibleCommander {
                    designation: format!("v{}-c{index}", viewer.0),
                    oracle_id: self.state.rules.cards[card].oracle_id.clone(),
                    history: history.clone(),
                })
                .collect(),
            priority: matches!(self.state.rules.frame, Frame::Priority)
                .then_some(self.state.rules.priority),
            players: vec![],
            objects: vec![],
            stack: vec![],
        })
    }
    pub(crate) fn request_header(&self, viewer: PlayerId) -> Result<Option<PlayerRequest>, String> {
        if viewer.0 >= 4 {
            return Err("invalid viewer".into());
        }
        if self.state.rules.pending.is_some() || !self.state.rules.pending_concessions.is_empty() {
            return Ok(None);
        }
        let token = self.state.knowledge.visibility[viewer.0 as usize]
            .decision_token
            .clone();
        let Some(token) = token else {
            return Ok(None);
        };
        if let Frame::Kernel { task } = &self.state.rules.frame {
            return Ok(self
                .kernel_request(task)?
                .filter(|r| r.player() == viewer)
                .map(|_| PlayerRequest::OptionalEffect {
                    token,
                    player: viewer,
                }));
        }
        if let Some(Request::GraveyardReplacementOrder {
            player,
            object,
            offered,
            ..
        }) = self.group_replacement_request()
        {
            return if player == viewer {
                Ok(Some(PlayerRequest::GraveyardReplacementOrder {
                    offered,
                    token,
                    player,
                    object: self.handle_for(viewer, object)?,
                }))
            } else {
                Ok(None)
            };
        }
        if let Some(Request::CommanderReturn {
            object,
            player,
            destination,
            ..
        }) = self.replacement_request()
        {
            return if player == viewer {
                Ok(Some(PlayerRequest::CommanderReturn {
                    previous_choices: self.visible_commander_choices(viewer)?,
                    token,
                    player,
                    object: Some(self.handle_for(viewer, object)?),
                    designation: self.commander_designation(viewer, object.card)?,
                    destination,
                }))
            } else {
                Ok(None)
            };
        }
        if let Some(Request::LegendRule {
            player,
            candidates,
            previous_choices,
            ..
        }) = self.legend_request()?
        {
            return (player == viewer)
                .then(|| -> Result<_, String> {
                    Ok(PlayerRequest::LegendRule {
                        token,
                        player,
                        candidates: candidates
                            .into_iter()
                            .map(|o| self.handle_for(viewer, o))
                            .collect::<Result<_, _>>()?,
                        previous_choices: previous_choices
                            .into_iter()
                            .map(|o| self.handle_for(viewer, o))
                            .collect::<Result<_, _>>()?,
                    })
                })
                .transpose();
        }
        if matches!(
            self.state.rules.frame,
            Frame::SacrificeCost { .. } | Frame::ForcedSacrifice { .. }
        ) {
            return Ok(match self.sacrifice_request()? {
                Some(Request::SacrificeCost {
                    player, candidates, ..
                }) if player == viewer => Some(PlayerRequest::SacrificeCost {
                    token,
                    player,
                    candidates: candidates
                        .into_iter()
                        .map(|o| self.handle_for(viewer, o))
                        .collect::<Result<_, _>>()?,
                }),
                Some(Request::ForcedSacrifice {
                    player,
                    candidates,
                    previous_choices,
                    ..
                }) if player == viewer => Some(PlayerRequest::ForcedSacrifice {
                    token,
                    player,
                    candidates: candidates
                        .into_iter()
                        .map(|o| self.handle_for(viewer, o))
                        .collect::<Result<_, _>>()?,
                    previous_choices: previous_choices
                        .into_iter()
                        // CR 800.4a/104.3a: a departed chooser's objects no longer exist.
                        .filter(|(p, o)| self.is_live(*p) && self.current(*o).is_ok())
                        .map(|(p, o)| Ok((p, self.handle_for(viewer, o)?)))
                        .collect::<Result<_, String>>()?,
                }),
                Some(Request::OptionalEffect { player, .. }) if player == viewer => {
                    Some(PlayerRequest::OptionalEffect { token, player })
                }
                _ => None,
            });
        }
        if matches!(self.state.rules.frame, Frame::Setup) {
            return Ok(match self.setup_request()? {
                Some(Request::Mulligan {
                    player,
                    mulligans_taken,
                    bottom_if_mulligan,
                    can_mulligan,
                    ..
                }) if player == viewer => Some(PlayerRequest::Mulligan {
                    token,
                    player,
                    mulligans_taken,
                    bottom_if_mulligan,
                    can_mulligan,
                }),
                Some(Request::BottomOpening { player, count, .. }) if player == viewer => {
                    Some(PlayerRequest::BottomOpening {
                        token,
                        player,
                        count,
                        candidates: vec![],
                    })
                }
                _ => None,
            });
        }

        if let Frame::AdditionalCosts { .. } = self.state.rules.frame {
            return if self.top()?.controller == viewer {
                Ok(Some(PlayerRequest::AdditionalCosts {
                    token,
                    player: viewer,
                    candidates: self.public_additional_cost_choices(viewer)?,
                }))
            } else {
                Ok(None)
            };
        }
        if let Frame::HybridCost { .. } = self.state.rules.frame {
            let top = self.top()?;
            if top.controller != viewer {
                return Ok(None);
            }
            return Ok(Some(PlayerRequest::HybridCost {
                token,
                player: viewer,
                symbols: self
                    .definition(top.source.card)?
                    .characteristics
                    .hybrid_mana
                    .clone(),
            }));
        }
        if let Frame::AnnounceX { .. } = self.state.rules.frame {
            return Ok(
                (self.top()?.controller == viewer).then_some(PlayerRequest::AnnounceX {
                    token,
                    player: self.top()?.controller,
                }),
            );
        }
        if let Some(Request::DamageEffect {
            player, recipient, ..
        }) = self.damage_effect_request()?
            && player == viewer
        {
            return Ok(Some(PlayerRequest::DamageEffect {
                token,
                player,
                recipient: self.player_entity(viewer, recipient)?,
                candidates: vec![],
            }));
        }

        if let Some(request) = self.actions_request()?
            && request.player() == viewer
        {
            let handles = |objects: Vec<ObjectId>| {
                objects
                    .into_iter()
                    .map(|object| self.handle_for(viewer, object))
                    .collect::<Result<Vec<_>, _>>()
            };
            return Ok(Some(match request {
                Request::ManaCombination { total, colors, .. } => PlayerRequest::ManaCombination {
                    token,
                    player: viewer,
                    total,
                    colors,
                },
                Request::OptionalEffect { .. } => PlayerRequest::OptionalEffect {
                    token,
                    player: viewer,
                },

                Request::Sacrifice { candidates, .. } => PlayerRequest::Sacrifice {
                    token,
                    player: viewer,
                    candidates: handles(candidates)?,
                },
                Request::ResolveDiscard {
                    count, candidates, ..
                } => PlayerRequest::ResolveDiscard {
                    token,
                    player: viewer,
                    count,
                    candidates: handles(candidates)?,
                },
                _ => return Err("resolution request invariant".into()),
            }));
        }
        if self.combat_decision_player() == Some(viewer) {
            let handles = |v: Vec<ObjectId>| {
                v.into_iter()
                    .map(|o| self.handle_for(viewer, o))
                    .collect::<Result<Vec<_>, String>>()
            };
            return Ok(Some(
                match self.combat_request()?.ok_or("missing combat request")? {
                    Request::Attackers {
                        candidates,
                        defenders,
                        ..
                    } => PlayerRequest::Attackers {
                        token,
                        player: viewer,
                        candidates: handles(candidates)?,
                        defenders,
                    },
                    Request::Blockers {
                        candidates,
                        attackers,
                        ..
                    } => PlayerRequest::Blockers {
                        menace_attackers: handles(
                            attackers
                                .iter()
                                .copied()
                                .filter(|a| self.has_keyword(*a, KeywordAbility::Menace))
                                .collect(),
                        )?,
                        legal_attackers: {
                            candidates
                                .iter()
                                .map(|b| {
                                    Ok(LegalBlockers {
                                        blocker: self.handle_for(viewer, *b)?,
                                        attackers: handles(
                                            attackers
                                                .iter()
                                                .copied()
                                                .filter(|a| self.can_block(*b, *a))
                                                .collect(),
                                        )?,
                                    })
                                })
                                .collect::<Result<Vec<_>, String>>()?
                        },
                        token,
                        player: viewer,
                        candidates: handles(candidates)?,
                        attackers: handles(attackers)?,
                    },
                    Request::Damage {
                        source,
                        power,
                        recipients,
                        player_recipient,
                        lethal_requirements,
                        ..
                    } => PlayerRequest::Damage {
                        player_recipient,
                        lethal_requirements: lethal_requirements
                            .into_iter()
                            .map(|(o, n)| Ok((self.handle_for(viewer, o)?, n)))
                            .collect::<Result<_, String>>()?,
                        token,
                        player: viewer,
                        source: self.handle_for(viewer, source)?,
                        power,
                        recipients: handles(recipients)?,
                    },
                    Request::Discard {
                        count, candidates, ..
                    } => PlayerRequest::Discard {
                        token,
                        player: viewer,
                        count,
                        candidates: handles(candidates)?,
                    },
                    _ => return Err("combat request invariant".into()),
                },
            ));
        }
        if let Some(Request::OrderTriggers { player, .. }) = self.trigger_request()? {
            return Ok((player == viewer).then_some(PlayerRequest::OrderTriggers {
                token,
                player,
                candidates: vec![],
            }));
        }
        match &self.state.rules.frame {
            Frame::EntityTargets { .. } | Frame::TriggerTargets { .. }
                if self.top()?.controller == viewer
                    && self
                        .program()?
                        .targets
                        .first()
                        .is_some_and(|t| t.domain == TargetDomain::AnyDamageTarget) =>
            {
                Ok(Some(PlayerRequest::EntityTargets {
                    token,
                    player: viewer,
                    occurrences: self.program()?.targets.clone(),
                    candidates: vec![],
                }))
            }
            Frame::StackTargets { .. } if self.top()?.controller == viewer => {
                Ok(Some(PlayerRequest::StackTargets {
                    token,
                    player: viewer,
                    occurrences: self.program()?.targets.clone(),
                    candidates: self
                        .eligible_stack_targets_for(
                            &self.program()?.targets[0],
                            viewer,
                            Some(self.top()?.id),
                        )
                        .into_iter()
                        .map(|id| self.stack_handle_for(viewer, id))
                        .collect::<Result<Vec<_>, _>>()?,
                }))
            }
            Frame::TriggerTargets { .. }
                if self.top()?.controller == viewer
                    && self.program()?.targets.first().is_some_and(|t| {
                        matches!(
                            t.domain,
                            TargetDomain::BattlefieldPermanent
                                | TargetDomain::BattlefieldCreature
                                | TargetDomain::OwnGraveyardCreatureCard
                                | TargetDomain::OwnGraveyardCard
                        )
                    }) =>
            {
                if !self.top_has_target_domain()? {
                    return Ok(None);
                }
                Ok(Some(PlayerRequest::Targets {
                    token,
                    player: viewer,
                    occurrences: self.program()?.targets.clone(),
                    counter_division: None,
                    candidates: vec![],
                }))
            }
            Frame::PlayerTargets { .. } | Frame::TriggerTargets { .. }
                if self.top()?.controller == viewer =>
            {
                Ok(Some(PlayerRequest::PlayerTargets {
                    token,
                    player: viewer,
                    occurrences: self.program()?.targets.clone(),
                    candidates: self.eligible_players(self.program()?.targets[0].domain, viewer),
                }))
            }
            Frame::ManaSourceChoice {
                player,
                source,
                scope: ManaSourceScope::Mixed,
                ..
            } if *player == viewer => Ok(Some(PlayerRequest::ChooseManaAbility {
                token,
                player: viewer,
                choices: self.mixed_mana_choices(viewer, *source)?,
                mana_ability_source: self.handle_for(viewer, *source)?,
            })),
            Frame::ManaSourceChoice {
                player,
                source,
                scope,
                ..
            } if *player == viewer => Ok(Some(PlayerRequest::ManaColor {
                token,
                player: viewer,
                colors: self.mana_source_colors(*player, *source, *scope)?,
                mana_ability_source: Some(self.handle_for(viewer, *source)?),
            })),
            Frame::ResolutionPayment { payment, .. } if payment.payer == viewer => self
                .visible_payment_request(
                    viewer,
                    token,
                    self.resolution_payment_request(payment),
                    None,
                )
                .map(Some),
            Frame::ManaAbility { activation } if activation.player == viewer => {
                Ok(Some(match self.permanent_mana_request(activation)? {
                    request @ (Request::Mana { .. } | Request::SpendMana { .. }) => self
                        .visible_payment_request(
                            viewer,
                            token,
                            request,
                            Some(self.handle_for(viewer, activation.source)?),
                        )?,
                    Request::ManaCombination { total, colors, .. } => {
                        PlayerRequest::ManaCombination {
                            token,
                            player: viewer,
                            total,
                            colors,
                        }
                    }
                    Request::ManaColor { colors, .. } => PlayerRequest::ManaColor {
                        token,
                        player: viewer,
                        colors,
                        // CR 113.7a: the paid production choice survives its
                        // source, but a departed incarnation has no live handle.
                        mana_ability_source: self
                            .current(activation.source)
                            .ok()
                            .map(|_| self.handle_for(viewer, activation.source))
                            .transpose()?,
                    },
                    _ => return Err("nested mana request invariant".into()),
                }))
            }
            Frame::SpendMana { .. } if self.top()?.controller == viewer => self
                .visible_payment_request(viewer, token, self.spend_mana_request()?, None)
                .map(Some),
            Frame::CommanderReturn {
                object,
                destination,
                ..
            } if self.state.rules.cards[&object.card].owner == viewer => {
                Ok(Some(PlayerRequest::CommanderReturn {
                    previous_choices: self.visible_commander_choices(viewer)?,
                    token,
                    player: viewer,
                    object: self.handle_for(viewer, *object).ok(),
                    designation: self.commander_designation(viewer, object.card)?,
                    destination: *destination,
                }))
            }
            Frame::Priority if self.state.rules.priority == viewer => {
                Ok(Some(PlayerRequest::Priority {
                    token,
                    player: viewer,
                }))
            }
            Frame::Targets { .. } if self.top()?.controller == viewer => {
                Ok(Some(PlayerRequest::Targets {
                    token,
                    player: viewer,
                    occurrences: self.program()?.targets.clone(),
                    counter_division: None,
                    candidates: vec![],
                }))
            }
            Frame::Mana { .. } if self.top()?.controller == viewer => {
                Ok(Some(PlayerRequest::Mana {
                    token,
                    player: viewer,
                    cost: self.top()?.total_cost.clone(),
                    mana_ability_source: None,
                }))
            }
            _ => Ok(None),
        }
    }
    fn visible_payment_request(
        &self,
        viewer: PlayerId,
        token: DecisionToken,
        request: Request,
        mana_ability_source: Option<ObjectHandle>,
    ) -> Result<PlayerRequest, String> {
        Ok(match request {
            Request::OptionalEffect { .. } => PlayerRequest::OptionalEffect {
                token,
                player: viewer,
            },
            Request::Mana { cost, .. } => PlayerRequest::Mana {
                token,
                player: viewer,
                cost,
                mana_ability_source,
            },
            Request::SpendMana {
                cost,
                available,
                restricted_available,
                ..
            } => PlayerRequest::SpendMana {
                token,
                player: viewer,
                cost,
                available,
                restricted_available,
                mana_ability_source,
            },
            _ => return Err("payment request invariant".into()),
        })
    }
    pub(in crate::engine) fn order_view_objects(view: &mut PermittedView) {
        // A face-down card's position among older exile handles must not
        // disclose the ordering of its physical card ID.
        for player in &mut view.players {
            if view
                .objects
                .iter()
                .any(|o| o.face_down && o.owner == player.player)
            {
                player.exile.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        view.objects.sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
    }

    /// The reference projection of one viewer's view: every row projected in
    /// table order, then ordered. Debug builds publish it for the complete
    /// projection that every publication is compared with.
    #[cfg(debug_assertions)]
    fn permitted_view(
        &self,
        viewer: PlayerId,
        index: &ProjectionIndex,
        effects: &[EffectKey],
        visible: &[&Object],
    ) -> Result<PermittedView, String> {
        let mut view = self.blank_view(viewer)?;
        view.effects = self.visible_effects_from(viewer, effects)?;
        for player in 0..4 {
            view.players
                .push(self.project_player(PlayerId(player), Some(index))?);
        }
        for object in visible {
            let object: VisibleObject =
                self.project_visible_object(viewer, object, Some(index), None)?;
            if object.zone == Zone::Exile {
                view.players[object.owner.0 as usize]
                    .exile
                    .push(object.handle.clone());
            }
            view.objects.push(object);
        }
        for index in 0..self.state.rules.stack.len() {
            view.stack.push(self.visible_stack(viewer, index)?);
        }
        Self::order_view_objects(&mut view);
        Ok(view)
    }
    /// One viewer's view, built as it is stored. Rows are projected in handle
    /// order, which is the order a view lists them in, and each row that
    /// equals the previous view's row shares its allocation. The result
    /// equals `permitted_view` converted and shared with its predecessor.
    fn stored_view(
        &self,
        viewer: PlayerId,
        (index, cores): (&ProjectionIndex, &ObjectCores),
        effects: &[EffectKey],
        visible: &[&Object],
        clean_except: Option<&BTreeSet<CardId>>,
    ) -> Result<StoredPermittedView, String> {
        let header = self.view_header(viewer)?;
        let effects = self.visible_effects_from(viewer, effects)?;
        let mut players = Vec::with_capacity(4);
        for player in 0..4 {
            players.push(self.project_player(PlayerId(player), Some(index))?);
        }
        let knowledge = self
            .state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .ok_or("invalid viewer")?;
        // The handle map is ordered by handle and names every visible
        // object, so walking it yields the rows already in view order.
        let mut objects = Vec::with_capacity(visible.len());
        let mut face_down = [false; 4];
        let mut previous = knowledge
            .history
            .last()
            .into_iter()
            .flat_map(|view| view.objects.iter())
            .peekable();
        for (handle, id) in knowledge.handles.entries.iter() {
            let Ok(position) = visible.binary_search_by_key(&id.card, |object| object.id.card)
            else {
                continue;
            };
            let object = visible[position];
            if object.id != *id {
                continue;
            }
            while previous.peek().is_some_and(|row| row.handle < *handle) {
                previous.next();
            }
            let old = previous.next_if(|row| row.handle == *handle);
            let row = match old {
                Some(row)
                    if clean_except.is_some_and(|dirty| !dirty.contains(&id.card))
                        && object.attached_to.is_none()
                        && row.attachments.is_empty()
                        && !index.attachments.contains_key(id)
                        && object.hideaway.is_none() =>
                {
                    row.clone()
                }
                _ => {
                    let projected: VisibleObject<HandleId<b'o'>> =
                        self.project_visible_object(viewer, object, Some(index), Some(cores))?;
                    match old {
                        Some(row) if **row == projected => row.clone(),
                        _ => stored_history::StoredObject::new(projected),
                    }
                }
            };
            if row.face_down
                && let Some(owned) = face_down.get_mut(row.owner.0 as usize)
            {
                *owned = true;
            }
            objects.push(row);
        }
        if objects.len() != visible.len() {
            return Err("object is not visible to this viewer".into());
        }
        // Exile lists keep table order unless a face-down card of that owner
        // would let its position disclose the order of physical card IDs.
        for object in visible.iter().filter(|object| object.zone == Zone::Exile) {
            let owner = self
                .state
                .rules
                .cards
                .get(&object.id.card)
                .ok_or("missing object definition")?
                .owner;
            players
                .get_mut(owner.0 as usize)
                .ok_or("invalid exile owner")?
                .exile
                .push(self.object_handle_id(viewer, object.id)?.clone());
        }
        for player in &mut players {
            if face_down
                .get(player.player.0 as usize)
                .is_some_and(|owned| *owned)
            {
                player.exile.sort();
            }
        }
        let mut stack = Vec::with_capacity(self.state.rules.stack.len());
        for index in 0..self.state.rules.stack.len() {
            stack.push(self.project_stack_entry(viewer, index)?);
        }
        Ok(StoredPermittedView::from_rows(
            header,
            effects,
            players,
            objects,
            stack,
            knowledge.history.last(),
        ))
    }
    /// The complete projection: every handle, decision token and view row is
    /// derived again from the current rules. This is the definition that the
    /// incremental path in `publish.rs` must equal. `reference` selects the
    /// plain row-by-row view construction that debug builds compare with.
    pub(super) fn publish_full(
        &mut self,
        decider: Option<Decider>,
        reference: bool,
    ) -> Result<ProjectionCache, String> {
        let previous = self.published.clone();
        let rules = &self.state.rules;
        let dirty = previous
            .as_ref()
            .map(|old| rules.objects.changed_cards(&old.rules.objects));
        let same_definitions = previous.as_ref().is_some_and(|old| {
            Arc::ptr_eq(&rules.cards, &old.rules.cards)
                && Arc::ptr_eq(&rules.tokens, &old.rules.tokens)
        });
        let projection = match previous.as_ref().zip(dirty.as_ref()) {
            Some((old, dirty)) if same_definitions => self.update_projection_index(old, dirty),
            _ => Arc::new(self.projection_index()),
        };
        // Dependencies of project_visible_object, its derived characteristics and
        // ability restrictions: the physical card/token definitions, entry copies
        // and back faces above; the object incarnation and its dirty chunk; active
        // effects, stack, turn number, mana uses, and combat below.
        // Static sources can read other permanents, so a dirty board with a static
        // source in either revision or an active effect invalidates the whole layer.
        // Reusing a viewer row also requires unchanged viewer knowledge, the same
        // issued handle, and no attachment or hideaway links (see stored_view).
        // New inputs to either projection must participate in these checks.
        let clean_except = previous
            .as_ref()
            .filter(|old| {
                same_definitions
                    && Shared::same(&rules.effects, &old.rules.effects)
                    && Shared::same(&rules.stack, &old.rules.stack)
                    && rules.turn.number == old.rules.turn.number
                    && rules.mana_ability_uses == old.rules.mana_ability_uses
                    && rules.combat == old.rules.combat
                    && (dirty.as_ref().is_some_and(BTreeSet::is_empty)
                        || (old.cache.index.static_sources.is_empty()
                            && projection.static_sources.is_empty()
                            && rules.effects.is_empty()))
            })
            .and(dirty.as_ref());
        let layer_epoch = previous.as_ref().map_or(0, |old| {
            if clean_except.is_some() {
                old.cache.layer_epoch
            } else {
                old.cache.layer_epoch.wrapping_add(1)
            }
        });
        let mut cached = previous
            .as_ref()
            .filter(|old| old.cache.layer_epoch == layer_epoch)
            .map(|old| old.cache.cores.clone())
            .unwrap_or_default();
        if let Some(dirty) = clean_except
            && !dirty.is_empty()
        {
            Arc::make_mut(&mut cached).retain(|id, _| !dirty.contains(&id.card));
        }
        let cores = ObjectCores::new(cached);
        let reuse_viewer: [bool; 4] = std::array::from_fn(|v| {
            previous
                .as_ref()
                .is_some_and(|old| old.same_viewer(v, &self.state))
        });
        let effects = self.refresh_effect_handles()?;
        let abilities: Vec<_> = self
            .state
            .rules
            .stack
            .iter()
            .map(|entry| entry.id)
            .collect();
        let triggers: Vec<_> = self
            .state
            .rules
            .pending_triggers
            .iter()
            .map(|t| t.id)
            .collect();
        let trigger_set: BTreeSet<_> = triggers.iter().copied().collect();
        let ability_set: BTreeSet<_> = abilities.iter().copied().collect();
        let revision = self.state.rules.revision;
        for v in 0..4 {
            let viewer = PlayerId(v);
            // A library card can be visible only through a resolving look, reveal
            // or entry choice. Keep the reference scan independent of the index.
            let current: Vec<_> = if reference
                || matches!(
                    self.state.rules.frame,
                    Frame::ResolutionChoice { .. } | Frame::GraveyardGroup { .. }
                ) {
                self.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| self.visible_to(viewer, o))
                    .collect()
            } else {
                projection
                    .outside_libraries
                    .iter()
                    .filter_map(|card| self.state.rules.objects.get(card))
                    .filter(|o| self.visible_to(viewer, o))
                    .collect()
            };
            let ids = current.iter().map(|object| object.id);
            let current_set = ids.clone().collect::<BTreeSet<_>>();
            {
                // Written in place: the table is copied only if a fork
                // still shares it.
                let k = &mut *self.state.knowledge.visibility[v as usize];
                k.handles.retain_objects(&current_set);
                k.trigger_handles.retain(|id, _| trigger_set.contains(id));
                for &id in &triggers {
                    if !k.trigger_handles.contains_key(&id) {
                        let handle = TriggerHandle(scoped(
                            &k.namespace,
                            format!("v{v}-t{}", k.next_trigger_handle),
                        ));
                        k.next_trigger_handle = k
                            .next_trigger_handle
                            .checked_add(1)
                            .ok_or("trigger handle limit")?;
                        k.trigger_handles.insert(id, handle);
                    }
                }
                k.ability_handles.retain(|id, _| ability_set.contains(id));
                for &id in &abilities {
                    if !k.ability_handles.contains_key(&id) {
                        let handle =
                            HandleId::new(v, k.next_ability_handle, k.namespace.as_deref());
                        k.next_ability_handle = k
                            .next_ability_handle
                            .checked_add(1)
                            .ok_or("ability handle resource limit")?;
                        k.ability_handles.insert(id, handle);
                    }
                }
                // Allocate from visible rows only; hidden IDs never enter a handle.
                // Initial/global card order is internal, but hidden rows do not
                // consume viewer counters. Equal permitted histories retain equal
                // counters even when hidden cards/IDs differ.
                let mut known = k.handles.objects();
                // Newly looked-at cards get handles in the permitted top-to-bottom
                // order before any other row, irrespective of physical CardId.
                for id in ids {
                    if known.insert(id) {
                        let handle = HandleId::new(v, k.next_handle, k.namespace.as_deref());
                        k.next_handle = k
                            .next_handle
                            .checked_add(1)
                            .ok_or("visibility handle resource limit")?;
                        k.handles.insert(handle, id);
                    }
                }
                let key = decider
                    .filter(|decider| decider.player == viewer)
                    .map(|decider| (revision, decider.kind));
                k.set_decision_key(v as usize, key)?;
            }
            #[cfg(debug_assertions)]
            if reference {
                let view = self.permitted_view(viewer, &projection, &effects, &current)?;
                let history = &mut self.state.knowledge.visibility[v as usize].history;
                if !history
                    .last()
                    .is_some_and(|previous| previous.matches_public(&view))
                {
                    history.push_public(view);
                }
                continue;
            }
            #[cfg(not(debug_assertions))]
            let _ = reference;
            let view = self.stored_view(
                viewer,
                (&projection, &cores),
                &effects,
                &current,
                clean_except.filter(|_| reuse_viewer[v as usize]),
            )?;
            let history = &mut self.state.knowledge.visibility[v as usize].history;
            if history.last().is_none_or(|previous| *previous != view) {
                history.push(view);
            }
        }
        Ok(ProjectionCache {
            layer_epoch,
            index: projection,
            cores: cores.into_inner(),
        })
    }
    /// Existing permitted handle order only; no raw-object scan or handle minting.
    /// Publication prunes this map when an incarnation ceases to be visible.
    pub(in crate::engine) fn next_action_handle(
        &self,
        viewer: PlayerId,
        after: Option<&ObjectHandle>,
    ) -> Option<ObjectHandle> {
        use std::ops::Bound::{Excluded, Unbounded};
        let entries = &self
            .state
            .knowledge
            .visibility
            .get(viewer.0 as usize)?
            .handles
            .entries;
        let row = match after {
            None => entries.first_key_value(),
            Some(handle) => entries
                .range((Excluded(HandleId::parse(&handle.0)), Unbounded))
                .next(),
        };
        row.map(|(handle, _)| ObjectHandle(handle.to_string()))
    }

    pub(crate) fn resolve_handle(
        &self,
        viewer: PlayerId,
        handle: &ObjectHandle,
    ) -> Result<ObjectId, String> {
        let id = *self
            .state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| k.handles.get(&handle.0))
            .ok_or("invalid player response")?;
        let object = self.current(id).map_err(|_| "invalid player response")?;
        if !self.visible_to(viewer, object) {
            return Err("invalid player response".into());
        }
        Ok(id)
    }
    pub(crate) fn convert_response(
        &self,
        viewer: PlayerId,
        response: &PlayerResponse,
    ) -> Result<Response, String> {
        let size = match response {
            PlayerResponse::AdditionalCosts { selected } => selected.len(),

            PlayerResponse::Attackers { attacks } => attacks.len(),
            PlayerResponse::Blockers { blocks } => blocks.len(),
            PlayerResponse::Damage { assignments }
            | PlayerResponse::TrampleDamage { assignments, .. } => assignments.len(),
            PlayerResponse::BottomOpening { objects }
            | PlayerResponse::Discard { objects }
            | PlayerResponse::ResolveDiscard { objects } => objects.len(),

            PlayerResponse::OrderTriggers { order } => order.len(),
            PlayerResponse::PlayerTargets { players } => players.len(),
            PlayerResponse::EntityTargets { entities } => entities.len(),
            PlayerResponse::StackTargets { entries } => entries.len(),
            _ => 0,
        };
        if size > std::cmp::max(self.state.rules.objects.len(), self.stack_limit()) {
            return Err("oversized player declaration".into());
        }
        match response {
            PlayerResponse::AdditionalCosts { selected } => {
                let choices = self.public_additional_cost_choices(viewer)?;
                let selected = selected
                    .iter()
                    .map(|id| {
                        let index = choices
                            .iter()
                            .position(|c| c.id == *id)
                            .ok_or("foreign or stale optional cost ID")?;
                        Ok(self.program()?.optional_additional_costs[index]
                            .identity
                            .clone())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(Response::AdditionalCosts { selected })
            }
            PlayerResponse::HybridCost { colors } => Ok(Response::HybridCost {
                colors: colors.clone(),
            }),
            PlayerResponse::AnnounceX { value } => Ok(Response::AnnounceX {
                value: value.clone(),
            }),
            PlayerResponse::ModeSubset { modes } => Ok(Response::ModeSubset {
                modes: modes.clone(),
            }),
            PlayerResponse::Modes { mode } => Ok(Response::Modes { mode: *mode }),
            PlayerResponse::CounterPlacementEffect { .. } => {
                Err("no counter placement replacement is admitted".into())
            }
            PlayerResponse::DamageEffect { effect, damage } => {
                let (effect, damage) = self.resolve_damage_effect_pair(viewer, effect, damage)?;
                Ok(Response::DamageEffect { effect, damage })
            }
            PlayerResponse::OrderTriggers { order } => Ok(Response::OrderTriggers {
                order: order
                    .iter()
                    .map(|h| self.resolve_trigger_handle(viewer, h))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            PlayerResponse::EntityTargets { entities } => Ok(Response::EntityTargets {
                entities: entities
                    .iter()
                    .map(|target| self.resolve_entity(viewer, target))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            PlayerResponse::StackTargets { entries } => Ok(Response::StackTargets {
                entries: entries
                    .iter()
                    .map(|handle| self.resolve_stack_handle(viewer, handle))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            PlayerResponse::ActivateAbility { object, ability } => Ok(Response::ActivateAbility {
                object: self.resolve_handle(viewer, object)?,
                ability: *ability,
            }),
            PlayerResponse::PlayerTargets { players } => Ok(Response::PlayerTargets {
                players: players.clone(),
            }),
            PlayerResponse::EntryCopy { object } => Ok(Response::EntryCopy {
                object: object
                    .as_ref()
                    .map(|h| self.resolve_handle(viewer, h))
                    .transpose()?,
            }),
            PlayerResponse::EntryAttachment { object } => Ok(Response::EntryAttachment {
                object: self.resolve_handle(viewer, object)?,
            }),

            PlayerResponse::EntryReveal { card } => Ok(Response::EntryReveal {
                card: card
                    .as_ref()
                    .map(|handle| self.resolve_handle(viewer, handle))
                    .transpose()?,
            }),

            PlayerResponse::OptionalEffect { perform } => {
                Ok(Response::OptionalEffect { perform: *perform })
            }

            PlayerResponse::SacrificeCost { object } => Ok(Response::SacrificeCost {
                object: self.resolve_handle(viewer, object)?,
            }),
            PlayerResponse::LegendRule { keep } => Ok(Response::LegendRule {
                keep: self.resolve_handle(viewer, keep)?,
            }),
            PlayerResponse::ForcedSacrifice { object } => Ok(Response::ForcedSacrifice {
                object: self.resolve_handle(viewer, object)?,
            }),
            PlayerResponse::Sacrifice { object } => Ok(Response::Sacrifice {
                object: self.resolve_handle(viewer, object)?,
            }),
            PlayerResponse::ResolveDiscard { objects } => Ok(Response::ResolveDiscard {
                objects: objects
                    .iter()
                    .map(|object| self.resolve_handle(viewer, object))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            PlayerResponse::ChooseManaAbility { ability } => {
                Ok(Response::ChooseManaAbility { ability: *ability })
            }
            PlayerResponse::ManaCombination { amounts } => Ok(Response::ManaCombination {
                amounts: amounts.clone(),
            }),
            PlayerResponse::ManaColor { color } => Ok(Response::ManaColor { color: *color }),
            PlayerResponse::SpendManaAllocated {
                spending,
                restricted,
            } => Ok(Response::SpendManaAllocated {
                spending: spending.clone(),
                restricted: restricted.clone(),
            }),
            PlayerResponse::SpendMana { spending } => Ok(Response::SpendMana {
                spending: spending.clone(),
            }),
            PlayerResponse::CommanderReturn { to_command } => Ok(Response::CommanderReturn {
                to_command: *to_command,
            }),
            PlayerResponse::Attackers { attacks } => Ok(Response::Attackers {
                attacks: attacks
                    .iter()
                    .map(|(o, p)| Ok((self.resolve_handle(viewer, o)?, *p)))
                    .collect::<Result<_, String>>()?,
            }),
            PlayerResponse::Blockers { blocks } => Ok(Response::Blockers {
                blocks: blocks
                    .iter()
                    .map(|(b, a)| {
                        Ok((
                            self.resolve_handle(viewer, b)?,
                            self.resolve_handle(viewer, a)?,
                        ))
                    })
                    .collect::<Result<_, String>>()?,
            }),
            PlayerResponse::TrampleDamage {
                assignments,
                player,
                amount,
            } => Ok(Response::TrampleDamage {
                assignments: assignments
                    .iter()
                    .map(|(o, n)| Ok((self.resolve_handle(viewer, o)?, n.clone())))
                    .collect::<Result<_, String>>()?,
                player: *player,
                amount: amount.clone(),
            }),
            PlayerResponse::Damage { assignments } => Ok(Response::Damage {
                assignments: assignments
                    .iter()
                    .map(|(o, n)| Ok((self.resolve_handle(viewer, o)?, n.clone())))
                    .collect::<Result<_, String>>()?,
            }),
            PlayerResponse::Mulligan { take } => Ok(Response::Mulligan { take: *take }),
            PlayerResponse::OpeningPlacement { object } => Ok(Response::OpeningPlacement {
                object: object
                    .as_ref()
                    .map(|h| self.resolve_handle(viewer, h))
                    .transpose()?,
            }),
            PlayerResponse::BottomOpening { objects } => Ok(Response::BottomOpening {
                objects: objects
                    .iter()
                    .map(|h| self.resolve_handle(viewer, h))
                    .collect::<Result<_, _>>()?,
            }),
            PlayerResponse::Discard { objects } => Ok(Response::Discard {
                objects: objects
                    .iter()
                    .map(|o| self.resolve_handle(viewer, o))
                    .collect::<Result<_, _>>()?,
            }),
            PlayerResponse::PlayBackLand { object } => Ok(Response::PlayBackLand {
                card: self.resolve_handle(viewer, object)?.card,
            }),
            PlayerResponse::PlayLand { object } => Ok(Response::PlayLand {
                card: self.resolve_handle(viewer, object)?.card,
            }),
            PlayerResponse::ActivateMana { object } => Ok(Response::ActivateMana {
                object: self.resolve_handle(viewer, object)?,
            }),
            PlayerResponse::Bestow { card } => Ok(Response::Bestow {
                card: self.resolve_handle(viewer, card)?.card,
            }),
            PlayerResponse::Cast { card } => Ok(Response::Cast {
                card: self.resolve_handle(viewer, card)?.card,
            }),
            PlayerResponse::Pass => Ok(Response::Pass),
            PlayerResponse::FinishMana { undo_mana } => Ok(Response::FinishMana {
                undo_mana: *undo_mana,
            }),
            PlayerResponse::Targets { objects } => Ok(Response::Targets {
                objects: objects
                    .iter()
                    .map(|h| self.resolve_handle(viewer, h))
                    .collect::<Result<_, _>>()?,
            }),
        }
    }
    pub(crate) fn validate_player_token(
        &self,
        viewer: PlayerId,
        token: &DecisionToken,
    ) -> Result<(), String> {
        let request = self
            .request_header(viewer)?
            .ok_or("invalid player response")?;
        if request.token() != token {
            return Err("invalid player response".into());
        }
        Ok(())
    }
}

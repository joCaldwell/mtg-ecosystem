#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleCommanderReturnChoice {
    pub designation: String,
    pub owner: PlayerId,
    pub to_command: bool,
}
use super::*;

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
pub struct VisibleAbility {
    pub handle: StackHandle,
    pub source: VisibleTarget,
    pub oracle_id: String,
    pub ability: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct TriggerHandle(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleTriggered {
    pub handle: StackHandle,
    pub source: VisibleTarget,
    pub oracle_id: String,
    pub ability: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisiblePendingTrigger {
    pub handle: TriggerHandle,
    pub source: VisibleTarget,
    pub oracle_id: String,
    pub ability: u32,
}
fn is_zero(value: &u64) -> bool {
    *value == 0
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct DecisionToken(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisiblePlayer {
    pub player: PlayerId,
    pub status: PlayerStatus,
    pub life: Quantity,
    pub mana: BTreeMap<Color, Quantity>,
    pub hand_count: u32,
    pub library_count: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleObject {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub characteristics: Option<Characteristics>,
    pub marked_damage: Quantity,
    pub attack_ready: bool,
    pub handle: ObjectHandle,
    pub oracle_id: String,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub tapped: bool,
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
pub struct VisibleTarget {
    pub announced: ObjectHandle,
    pub current: Option<ObjectHandle>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleStackTarget {
    pub announced: StackHandle,
    pub current: Option<StackHandle>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "entity", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerTargetEntity {
    Player { player: PlayerId },
    Object { object: ObjectHandle },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "entity", rename_all = "snake_case", deny_unknown_fields)]
pub enum VisibleEntityTarget {
    Player { player: PlayerId },
    Object { object: VisibleTarget },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleStackEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggered: Option<VisibleTriggered>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_targets: Vec<VisibleEntityTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<StackHandle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack_targets: Vec<VisibleStackTarget>,
    pub spell: Option<ObjectHandle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability: Option<VisibleAbility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_targets: Vec<PlayerId>,
    pub controller: PlayerId,
    pub cast_complete: bool,
    pub targets: Vec<VisibleTarget>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleAttacker {
    pub attacker: ObjectHandle,
    pub defender: PlayerId,
    pub blocked: bool,
    pub blockers: Vec<ObjectHandle>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermittedView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<VisibleSetup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<VisibleEffect>,
    pub combat: Vec<VisibleAttacker>,
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
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerRequest {
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
    DamageAllocation {
        token: DecisionToken,
        player: PlayerId,
        recipient: PlayerTargetEntity,
        effect: EffectHandle,
        remaining: Quantity,
        required: Quantity,
        candidates: Vec<VisibleDamageAllocationCandidate>,
    },
    Modes {
        token: DecisionToken,
        player: PlayerId,
        candidates: Vec<u32>,
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
        candidates: Vec<ObjectHandle>,
    },
    Mana {
        token: DecisionToken,
        player: PlayerId,
        cost: ManaCost,
    },
}
impl PlayerRequest {
    pub fn token(&self) -> &DecisionToken {
        match self {
            Self::Mulligan { token, .. }
            | Self::BottomOpening { token, .. }
            | Self::DamageAllocation { token, .. }
            | Self::Modes { token, .. }
            | Self::DamageEffect { token, .. }
            | Self::OrderTriggers { token, .. }
            | Self::EntityTargets { token, .. }
            | Self::StackTargets { token, .. }
            | Self::PlayerTargets { token, .. }
            | Self::Sacrifice { token, .. }
            | Self::ResolveDiscard { token, .. }
            | Self::SpendMana { token, .. }
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
    pub observation_version: u32,
    pub view: PermittedView,
    pub decision: Option<PlayerRequest>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerResponse {
    Mulligan {
        take: bool,
    },
    BottomOpening {
        objects: Vec<ObjectHandle>,
    },
    DamageAllocation {
        allocations: Vec<(DamageHandle, Quantity)>,
    },
    Modes {
        mode: u32,
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
    Discard {
        objects: Vec<ObjectHandle>,
    },
    PlayLand {
        object: ObjectHandle,
    },
    ActivateMana {
        object: ObjectHandle,
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

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Knowledge {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) own_knowledge: Vec<OwnKnowledgeAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) effect_handles: Vec<(EffectKey, EffectHandle)>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(super) next_effect_handle: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) damage_handles: Vec<((DamageTransactionId, DamageId), DamageHandle)>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(super) next_damage_handle: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    trigger_handles: BTreeMap<TriggerId, TriggerHandle>,
    #[serde(default, skip_serializing_if = "is_zero")]
    next_trigger_handle: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    ability_handles: BTreeMap<StackId, StackHandle>,
    #[serde(default, skip_serializing_if = "is_zero")]
    next_ability_handle: u64,
    handles: BTreeMap<String, ObjectId>,
    next_handle: u64,
    decision_key: Option<(u64, u8)>,
    decision_token: Option<DecisionToken>,
    next_decision: u64,
    pub(super) history: super::stored_history::StoredHistory,
}
fn scoped(namespace: &Option<String>, handle: String) -> String {
    namespace.as_ref().map_or_else(
        || handle.clone(),
        |namespace| format!("{namespace}:{handle}"),
    )
}
impl Knowledge {
    pub(super) fn scoped(&self, handle: String) -> String {
        self.namespace.as_ref().map_or_else(
            || handle.clone(),
            |namespace| format!("{namespace}:{handle}"),
        )
    }
}
impl Game {
    pub(super) fn capture_trigger_source_handles(
        &mut self,
        object: ObjectId,
    ) -> Result<Vec<ObjectHandle>, String> {
        let mut handles = vec![];
        for viewer in 0..4 {
            let k = &mut self.state.visibility[viewer];
            let handle = if let Some((handle, _)) = k.handles.iter().find(|(_, id)| **id == object)
            {
                ObjectHandle(handle.clone())
            } else {
                let handle = ObjectHandle(k.scoped(format!("v{viewer}-o{}", k.next_handle)));
                k.next_handle = k
                    .next_handle
                    .checked_add(1)
                    .ok_or("trigger source handle limit")?;
                k.handles.insert(handle.0.clone(), object);
                handle
            };
            handles.push(handle);
        }
        Ok(handles)
    }
    pub(super) fn resolve_trigger_handle(
        &self,
        viewer: PlayerId,
        handle: &TriggerHandle,
    ) -> Result<TriggerId, String> {
        self.state
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
    pub(super) fn visible_pending_trigger(
        &self,
        viewer: PlayerId,
        id: TriggerId,
    ) -> Result<VisiblePendingTrigger, String> {
        let trigger = self
            .state
            .pending_triggers
            .iter()
            .find(|t| t.id == id)
            .ok_or("missing pending trigger")?;
        Ok(VisiblePendingTrigger {
            handle: self
                .state
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
                    .clone(),
                current: self.handle_for(viewer, trigger.source).ok(),
            },
            oracle_id: self
                .state
                .cards
                .get(&trigger.source.card)
                .ok_or("missing trigger physical source")?
                .oracle_id
                .clone(),
            ability: trigger.ability,
        })
    }

    pub(super) fn player_entity(
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
    pub(super) fn resolve_entity(
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
        match object.zone {
            Zone::Hand { owner } => owner == viewer,
            Zone::Library { .. } => false,
            _ => true,
        }
    }
    pub(super) fn stack_handle_for(
        &self,
        viewer: PlayerId,
        id: StackId,
    ) -> Result<StackHandle, String> {
        if !self.state.stack.iter().any(|entry| entry.id == id) {
            return Err("stale stack identity".into());
        }
        self.state
            .visibility
            .get(viewer.0 as usize)
            .and_then(|knowledge| knowledge.ability_handles.get(&id))
            .cloned()
            .ok_or_else(|| "stack object is not visible".into())
    }
    fn resolve_stack_handle(
        &self,
        viewer: PlayerId,
        handle: &StackHandle,
    ) -> Result<StackId, String> {
        self.state
            .visibility
            .get(viewer.0 as usize)
            .and_then(|knowledge| {
                knowledge
                    .ability_handles
                    .iter()
                    .find(|(_, value)| *value == handle)
            })
            .map(|(id, _)| *id)
            .filter(|id| self.state.stack.iter().any(|entry| entry.id == *id))
            .ok_or_else(|| "stale or foreign stack handle".into())
    }
    pub(super) fn visible_player(&self, player: PlayerId) -> Result<VisiblePlayer, String> {
        let p = self
            .state
            .players
            .get(player.0 as usize)
            .ok_or("invalid viewer")?;
        let count = |zone| {
            u32::try_from(
                self.state
                    .objects
                    .values()
                    .filter(|o| o.zone == zone)
                    .count(),
            )
            .map_err(|_| "zone count resource limit".to_string())
        };
        Ok(VisiblePlayer {
            player,
            status: p.status,
            life: p.life.clone(),
            mana: p.mana.clone(),
            hand_count: count(Zone::Hand { owner: player })?,
            library_count: count(Zone::Library { owner: player })?,
        })
    }
    pub(super) fn handle_for(
        &self,
        viewer: PlayerId,
        id: ObjectId,
    ) -> Result<ObjectHandle, String> {
        self.state
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| k.handles.iter().find(|(_, v)| **v == id))
            .map(|(h, _)| ObjectHandle(h.clone()))
            .ok_or_else(|| "object is not visible to this viewer".into())
    }
    pub(super) fn visible_object(
        &self,
        viewer: PlayerId,
        object: &Object,
    ) -> Result<Option<VisibleObject>, String> {
        if !self.visible_to(viewer, object) {
            return Ok(None);
        }
        let physical = self
            .state
            .cards
            .get(&object.id.card)
            .ok_or("missing object definition")?;
        Ok(Some(VisibleObject {
            characteristics: if self.prepared.config.effects_profile() {
                Some(self.characteristics(object.id)?)
            } else {
                None
            },
            marked_damage: object.marked_damage.clone(),
            attack_ready: object.attack_ready,
            handle: self.handle_for(viewer, object.id)?,
            oracle_id: physical.oracle_id.clone(),
            owner: physical.owner,
            controller: object.controller,
            zone: object.zone,
            tapped: object.tapped,
        }))
    }
    pub(super) fn visible_stack(
        &self,
        viewer: PlayerId,
        index: usize,
    ) -> Result<VisibleStackEntry, String> {
        let instance = self
            .state
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
                        .clone(),
                    current: self.handle_for(viewer, *target).ok(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (spell, ability, triggered) = match &instance.kind {
            StackKind::Spell => (Some(self.handle_for(viewer, instance.source)?), None, None),
            StackKind::Triggered {
                ability,
                source_handles,
                ..
            } => (
                None,
                None,
                Some(VisibleTriggered {
                    handle: self.stack_handle_for(viewer, instance.id)?,
                    source: VisibleTarget {
                        announced: source_handles
                            .get(viewer.0 as usize)
                            .ok_or("missing triggered source reference")?
                            .clone(),
                        current: self.handle_for(viewer, instance.source).ok(),
                    },
                    oracle_id: self
                        .state
                        .cards
                        .get(&instance.source.card)
                        .ok_or("missing triggered physical source")?
                        .oracle_id
                        .clone(),
                    ability: *ability,
                }),
            ),
            StackKind::Activated {
                ability,
                source_handles,
            } => (
                None,
                Some(VisibleAbility {
                    handle: self.state.visibility[viewer.0 as usize]
                        .ability_handles
                        .get(&instance.id)
                        .ok_or("missing ability stack handle")?
                        .clone(),
                    source: VisibleTarget {
                        announced: source_handles
                            .get(viewer.0 as usize)
                            .ok_or("missing announced ability source")?
                            .clone(),
                        current: self.handle_for(viewer, instance.source).ok(),
                    },
                    oracle_id: self
                        .state
                        .cards
                        .get(&instance.source.card)
                        .ok_or("missing ability physical source")?
                        .oracle_id
                        .clone(),
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
                    announced: instance
                        .announced_stack_targets
                        .get(viewer.0 as usize)
                        .and_then(|entries| entries.get(slot))
                        .ok_or("missing announced stack target")?
                        .clone(),
                    current: self.stack_handle_for(viewer, *target).ok(),
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
                            .and_then(Clone::clone)
                            .ok_or("missing entity announcement")?,
                        current: self.handle_for(viewer, *object).ok(),
                    },
                }),
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(VisibleStackEntry {
            mode: instance.mode,
            triggered,
            entity_targets,
            handle: if self.prepared.config.stack_costs_profile() {
                Some(self.stack_handle_for(viewer, instance.id)?)
            } else {
                None
            },
            stack_targets,
            spell,
            ability,
            player_targets: instance.player_targets.clone(),
            controller: instance.controller,
            cast_complete: instance.paid.is_some(),
            targets,
        })
    }
    pub(super) fn blank_view(&self, viewer: PlayerId) -> Result<PermittedView, String> {
        if viewer.0 >= 4 {
            return Err("invalid viewer".into());
        }
        Ok(PermittedView {
            setup: self.visible_setup(),
            effects: vec![],
            combat: self
                .state
                .combat
                .iter()
                .map(|a| {
                    Ok(VisibleAttacker {
                        attacker: self.handle_for(viewer, a.attacker)?,
                        defender: a.defender,
                        blocked: a.blocked,
                        blockers: a
                            .blockers
                            .iter()
                            .map(|b| self.handle_for(viewer, *b))
                            .collect::<Result<_, _>>()?,
                    })
                })
                .collect::<Result<_, String>>()?,
            viewer,
            active: self.state.active,
            turn_owner: self.state.turn_owner,
            turn: self.state.turn.clone(),
            live_turn_order: self.live_players(),
            commanders: self
                .state
                .commanders
                .records
                .iter()
                .enumerate()
                .map(|(index, (card, history))| VisibleCommander {
                    designation: format!("v{}-c{index}", viewer.0),
                    oracle_id: self.state.cards[card].oracle_id.clone(),
                    history: history.clone(),
                })
                .collect(),
            priority: matches!(self.state.frame, Frame::Priority).then_some(self.state.priority),
            players: vec![],
            objects: vec![],
            stack: vec![],
        })
    }
    pub(super) fn request_header(&self, viewer: PlayerId) -> Result<Option<PlayerRequest>, String> {
        if viewer.0 >= 4 {
            return Err("invalid viewer".into());
        }
        if self.state.pending.is_some() || !self.state.pending_concessions.is_empty() {
            return Ok(None);
        }
        let token = self.state.visibility[viewer.0 as usize]
            .decision_token
            .clone();
        let Some(token) = token else {
            return Ok(None);
        };
        if matches!(self.state.frame, Frame::Setup) {
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
        if let Some(Request::DamageAllocation {
            player,
            recipient,
            effect,
            remaining,
            required,
            ..
        }) = self.damage_allocation_request()?
        {
            if player != viewer {
                return Ok(None);
            }
            return Ok(Some(PlayerRequest::DamageAllocation {
                token: token.clone(),
                player,
                recipient: self.player_entity(viewer, recipient)?,
                effect: self.effect_handle(viewer, &effect)?,
                remaining,
                required,
                candidates: vec![],
            }));
        }
        if let Some(Request::Modes { player, .. }) = self.modes_request()? {
            return Ok((player == viewer).then_some(PlayerRequest::Modes {
                token,
                player,
                candidates: vec![],
            }));
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
                        legal_attackers: if self.prepared.config.profile == KEYWORDS_PROFILE {
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
                        } else {
                            vec![]
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
                        ..
                    } => PlayerRequest::Damage {
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
        match &self.state.frame {
            Frame::EntityTargets { .. } if self.top()?.controller == viewer => {
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
                        .eligible_stack_targets(
                            self.program()?.targets[0].domain,
                            Some(self.top()?.id),
                        )
                        .into_iter()
                        .map(|id| self.stack_handle_for(viewer, id))
                        .collect::<Result<Vec<_>, _>>()?,
                }))
            }
            Frame::TriggerTargets { .. }
                if self.top()?.controller == viewer
                    && self
                        .program()?
                        .targets
                        .first()
                        .is_some_and(|t| t.domain == TargetDomain::BattlefieldCreature) =>
            {
                if !self.has_target_domain(self.program()?, viewer) {
                    return Ok(None);
                }
                Ok(Some(PlayerRequest::Targets {
                    token,
                    player: viewer,
                    occurrences: self.program()?.targets.clone(),
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
            Frame::SpendMana { .. } if self.top()?.controller == viewer => {
                let Request::SpendMana {
                    cost, available, ..
                } = self.spend_mana_request()?
                else {
                    return Err("spending request invariant".into());
                };
                Ok(Some(PlayerRequest::SpendMana {
                    token,
                    player: viewer,
                    cost,
                    available,
                }))
            }
            Frame::CommanderReturn {
                object,
                destination,
                ..
            } if self.state.cards[&object.card].owner == viewer => {
                Ok(Some(PlayerRequest::CommanderReturn {
                    previous_choices: self.visible_commander_choices(viewer)?,
                    token,
                    player: viewer,
                    object: self.handle_for(viewer, *object).ok(),
                    designation: self.commander_designation(viewer, object.card)?,
                    destination: *destination,
                }))
            }
            Frame::Priority if self.state.priority == viewer => Ok(Some(PlayerRequest::Priority {
                token,
                player: viewer,
            })),
            Frame::Targets { .. } if self.top()?.controller == viewer => {
                Ok(Some(PlayerRequest::Targets {
                    token,
                    player: viewer,
                    occurrences: self.program()?.targets.clone(),
                    candidates: vec![],
                }))
            }
            Frame::Mana { .. } if self.top()?.controller == viewer => {
                Ok(Some(PlayerRequest::Mana {
                    token,
                    player: viewer,
                    cost: self.top()?.total_cost.clone(),
                }))
            }
            _ => Ok(None),
        }
    }
    fn permitted_view(&self, viewer: PlayerId) -> Result<PermittedView, String> {
        let mut view = self.blank_view(viewer)?;
        view.effects = self.visible_effects(viewer)?;
        for player in 0..4 {
            view.players.push(self.visible_player(PlayerId(player))?);
        }
        for object in self.state.objects.values() {
            if let Some(o) = self.visible_object(viewer, object)? {
                view.objects.push(o);
            }
        }
        for index in 0..self.state.stack.len() {
            view.stack.push(self.visible_stack(viewer, index)?);
        }
        view.objects.sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
        Ok(view)
    }
    pub(super) fn refresh_visibility(&mut self) -> Result<(), String> {
        if self.setup_automatic() {
            return Ok(());
        }
        self.refresh_effect_handles()?;
        for v in 0..4 {
            let viewer = PlayerId(v);
            let current = self
                .state
                .objects
                .values()
                .filter(|o| self.visible_to(viewer, o))
                .map(|o| o.id)
                .collect::<Vec<_>>();
            let mut k = self.state.visibility[v as usize].clone();
            k.handles.retain(|_, id| current.contains(id));
            let abilities = self
                .state
                .stack
                .iter()
                .filter(|entry| {
                    !entry.kind.is_spell() || self.prepared.config.stack_costs_profile()
                })
                .map(|entry| entry.id)
                .collect::<Vec<_>>();
            let triggers = self
                .state
                .pending_triggers
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>();
            k.trigger_handles.retain(|id, _| triggers.contains(id));
            for id in triggers {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    k.trigger_handles.entry(id)
                {
                    let handle = TriggerHandle(scoped(
                        &k.namespace,
                        format!("v{v}-t{}", k.next_trigger_handle),
                    ));
                    k.next_trigger_handle = k
                        .next_trigger_handle
                        .checked_add(1)
                        .ok_or("trigger handle limit")?;
                    entry.insert(handle);
                }
            }
            k.ability_handles.retain(|id, _| abilities.contains(id));
            for id in abilities {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    k.ability_handles.entry(id)
                {
                    let handle = StackHandle(scoped(
                        &k.namespace,
                        format!(
                            "v{v}-{}{}",
                            if self.prepared.config.stack_costs_profile() {
                                "s"
                            } else {
                                "a"
                            },
                            k.next_ability_handle
                        ),
                    ));
                    k.next_ability_handle = k
                        .next_ability_handle
                        .checked_add(1)
                        .ok_or("ability handle resource limit")?;
                    entry.insert(handle);
                }
            }
            // Allocate from visible rows only; hidden IDs never enter a handle.
            // Initial/global card order is internal, but hidden rows do not
            // consume viewer counters. Equal permitted histories retain equal
            // counters even when hidden cards/IDs differ.
            for id in current {
                if !k.handles.values().any(|existing| *existing == id) {
                    let handle = k.scoped(format!("v{v}-o{}", k.next_handle));
                    k.next_handle = k
                        .next_handle
                        .checked_add(1)
                        .ok_or("visibility handle resource limit")?;
                    k.handles.insert(handle, id);
                }
            }
            let key = if self.state.pending.is_some() || !self.state.pending_concessions.is_empty()
            {
                None
            } else if self
                .damage_allocation_request()?
                .is_some_and(|r| r.player() == viewer)
            {
                Some((self.state.revision, 14))
            } else if self.modes_request()?.is_some_and(|r| r.player() == viewer) {
                Some((self.state.revision, 13))
            } else if self
                .damage_effect_request()?
                .is_some_and(|r| r.player() == viewer)
            {
                Some((self.state.revision, 12))
            } else if self
                .trigger_request()?
                .is_some_and(|r| r.player() == viewer)
            {
                Some((self.state.revision, 11))
            } else if self
                .actions_request()?
                .is_some_and(|request| request.player() == viewer)
            {
                Some((self.state.revision, 6))
            } else if self.combat_decision_player() == Some(viewer) {
                Some((self.state.revision, 3))
            } else {
                match self.state.frame {
                    Frame::Setup if self.setup_request()?.is_some_and(|r| r.player() == viewer) => {
                        Some((self.state.revision, 15))
                    }
                    Frame::EntityTargets { .. } if self.top()?.controller == viewer => {
                        Some((self.state.revision, 10))
                    }
                    Frame::StackTargets { .. } if self.top()?.controller == viewer => {
                        Some((self.state.revision, 9))
                    }
                    Frame::TriggerTargets { .. }
                        if self.top()?.controller == viewer
                            && !self.has_target_domain(self.program()?, viewer) =>
                    {
                        None
                    }
                    Frame::PlayerTargets { .. } | Frame::TriggerTargets { .. }
                        if self.top()?.controller == viewer =>
                    {
                        Some((self.state.revision, 7))
                    }
                    Frame::SpendMana { .. } if self.top()?.controller == viewer => {
                        Some((self.state.revision, 8))
                    }
                    Frame::CommanderReturn { object, .. }
                        if self.state.cards[&object.card].owner == viewer =>
                    {
                        Some((self.state.revision, 4))
                    }
                    Frame::Priority if self.state.priority == viewer => {
                        Some((self.state.revision, 0))
                    }
                    Frame::Targets { .. } if self.top()?.controller == viewer => {
                        Some((self.state.revision, 1))
                    }
                    Frame::Mana { .. } if self.top()?.controller == viewer => {
                        Some((self.state.revision, 2))
                    }
                    _ => None,
                }
            };
            if key != k.decision_key {
                k.decision_token = if key.is_some() {
                    let token = DecisionToken(k.scoped(format!("v{v}-d{}", k.next_decision)));
                    k.next_decision = k
                        .next_decision
                        .checked_add(1)
                        .ok_or("decision token resource limit")?;
                    Some(token)
                } else {
                    None
                };
                k.decision_key = key;
            }
            self.state.visibility[v as usize] = k;
            let view = self.permitted_view(viewer)?;
            let history = &mut self.state.visibility[v as usize].history;
            if !history
                .last()
                .is_some_and(|previous| previous.matches_public(&view))
            {
                history.push_public(view);
            }
        }
        Ok(())
    }
    pub(super) fn resolve_handle(
        &self,
        viewer: PlayerId,
        handle: &ObjectHandle,
    ) -> Result<ObjectId, String> {
        let id = *self
            .state
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
    pub(super) fn convert_response(
        &self,
        viewer: PlayerId,
        response: &PlayerResponse,
    ) -> Result<Response, String> {
        let size = match response {
            PlayerResponse::Attackers { attacks } => attacks.len(),
            PlayerResponse::Blockers { blocks } => blocks.len(),
            PlayerResponse::Damage { assignments } => assignments.len(),
            PlayerResponse::BottomOpening { objects }
            | PlayerResponse::Discard { objects }
            | PlayerResponse::ResolveDiscard { objects } => objects.len(),
            PlayerResponse::DamageAllocation { allocations } => allocations.len(),
            PlayerResponse::OrderTriggers { order } => order.len(),
            PlayerResponse::PlayerTargets { players } => players.len(),
            PlayerResponse::EntityTargets { entities } => entities.len(),
            PlayerResponse::StackTargets { entries } => entries.len(),
            _ => 0,
        };
        if size
            > std::cmp::max(
                self.state.objects.len(),
                if self.prepared.config.triggers_profile() {
                    self.stack_limit()
                } else {
                    0
                },
            )
        {
            return Err("oversized player declaration".into());
        }
        match response {
            PlayerResponse::DamageAllocation { allocations } => Ok(Response::DamageAllocation {
                allocations: self.resolve_allocation_response(viewer, allocations)?,
            }),
            PlayerResponse::Modes { mode } => Ok(Response::Modes { mode: *mode }),
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
            PlayerResponse::Sacrifice { object } => Ok(Response::Sacrifice {
                object: self.resolve_handle(viewer, object)?,
            }),
            PlayerResponse::ResolveDiscard { objects } => Ok(Response::ResolveDiscard {
                objects: objects
                    .iter()
                    .map(|object| self.resolve_handle(viewer, object))
                    .collect::<Result<Vec<_>, _>>()?,
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
            PlayerResponse::Damage { assignments } => Ok(Response::Damage {
                assignments: assignments
                    .iter()
                    .map(|(o, n)| Ok((self.resolve_handle(viewer, o)?, n.clone())))
                    .collect::<Result<_, String>>()?,
            }),
            PlayerResponse::Mulligan { take } => Ok(Response::Mulligan { take: *take }),
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
            PlayerResponse::PlayLand { object } => Ok(Response::PlayLand {
                card: self.resolve_handle(viewer, object)?.card,
            }),
            PlayerResponse::ActivateMana { object } => Ok(Response::ActivateMana {
                object: self.resolve_handle(viewer, object)?,
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
    pub(super) fn validate_player_token(
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

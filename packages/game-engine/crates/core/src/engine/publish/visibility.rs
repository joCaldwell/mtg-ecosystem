use super::*;
use super::{HandleId, stored_history::StoredPermittedView};
fn is_zero(value: &u64) -> bool {
    *value == 0
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
    /// CR 701.23e: the minimum number of publicly revealed copies of each
    /// Oracle identity known to be in each owner's hand.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub revealed_hand: BTreeMap<PlayerId, BTreeMap<String, u32>>,
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
    pub(in crate::engine) decision_token: Option<DecisionToken>,
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

    fn visible_trigger_ability(ability: EffectiveTriggerAbility) -> VisibleTriggerAbility {
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
    // CR 400.2, 401.2, 402.3: own hand is visible; all libraries and other
    // hands remain hidden. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(in crate::engine) fn visible_to(&self, viewer: PlayerId, object: &Object) -> bool {
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
        // CR 701.22a, 701.23a, 701.25a: only the player searching, scrying
        // or surveiling sees the library cards of their own choice, while it
        // waits; no position or other card is published.
        if matches!(object.zone, Zone::Library { owner } if owner == viewer)
            && self.library_choice_visible(viewer, object.id)
        {
            return true;
        }
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
                entered_this_turn: false,
                face_down: true,
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
    >(
        &self,
        viewer: PlayerId,
        index: usize,
    ) -> Result<VisibleStackEntry<H, O>, String> {
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
            selected_additional_costs: instance.selected_additional_costs.clone(),
            x: instance.x.clone(),
            hybrid_colors: instance.hybrid_colors.clone(),
            mana_value: instance
                .x
                .as_ref()
                .map(|_| self.stack_mana_value(instance))
                .transpose()?,
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
                    && rules.active == old.rules.active
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
                    .as_ref()
                    .filter(|decider| decider.player == viewer)
                    .map(|decider| (revision, decider.reason.code()));
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
    pub(crate) fn validate_player_token(
        &self,
        viewer: PlayerId,
        token: &DecisionToken,
    ) -> Result<(), String> {
        if self.state.rules.pending.is_some()
            || !self.state.rules.pending_concessions.is_empty()
            || self
                .state
                .knowledge
                .visibility
                .get(viewer.0 as usize)
                .and_then(|k| k.decision_token.as_ref())
                != Some(token)
        {
            return Err("stale or foreign decision token".into());
        }
        Ok(())
    }
}

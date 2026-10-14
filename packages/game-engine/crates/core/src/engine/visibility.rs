use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectHandle(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DecisionToken(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisiblePlayer {
    pub player: PlayerId,
    pub status: PlayerStatus,
    pub life: Quantity,
    pub mana: BTreeMap<Color, Quantity>,
    pub hand_count: u32,
    pub library_count: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleObject {
    pub handle: ObjectHandle,
    pub oracle_id: String,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub tapped: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleCommander {
    pub designation: String,
    pub oracle_id: String,
    pub history: CommanderHistory,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleTarget {
    pub announced: ObjectHandle,
    pub current: Option<ObjectHandle>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleStackEntry {
    pub spell: ObjectHandle,
    pub controller: PlayerId,
    pub cast_complete: bool,
    pub targets: Vec<VisibleTarget>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermittedView {
    pub viewer: PlayerId,
    pub active: Option<PlayerId>,
    pub turn_owner: PlayerId,
    pub live_turn_order: Vec<PlayerId>,
    pub commanders: Vec<VisibleCommander>,
    pub priority: Option<PlayerId>,
    pub players: Vec<VisiblePlayer>,
    pub objects: Vec<VisibleObject>,
    pub stack: Vec<VisibleStackEntry>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerRequest {
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
}
impl PlayerRequest {
    pub fn token(&self) -> &DecisionToken {
        match self {
            Self::Priority { token, .. } | Self::Targets { token, .. } => token,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub observation_version: u32,
    pub view: PermittedView,
    pub decision: Option<PlayerRequest>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerResponse {
    ActivateMana { object: ObjectHandle },
    Cast { card: ObjectHandle },
    Pass,
    Targets { objects: Vec<ObjectHandle> },
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
    pub permitted_history: Vec<PermittedView>,
    pub open_decklists: Vec<OpenDecklist>,
    pub catalog_digest: String,
    pub profile_id: String,
    pub sampling_seed: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Knowledge {
    handles: BTreeMap<String, ObjectId>,
    next_handle: u64,
    decision_key: Option<(u64, u8)>,
    decision_token: Option<DecisionToken>,
    next_decision: u64,
    pub(super) history: Vec<PermittedView>,
}
impl Game {
    // CR 400.2, 401.2, 402.3: own hand is visible; all libraries and other
    // hands remain hidden. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn visible_to(&self, viewer: PlayerId, object: &Object) -> bool {
        match object.zone {
            Zone::Hand { owner } => owner == viewer,
            Zone::Library { .. } => false,
            _ => true,
        }
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
        Ok(VisibleStackEntry {
            spell: self.handle_for(viewer, instance.source)?,
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
            viewer,
            active: self.state.active,
            turn_owner: self.state.turn_owner,
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
        match &self.state.frame {
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
            _ => Ok(None),
        }
    }
    fn permitted_view(&self, viewer: PlayerId) -> Result<PermittedView, String> {
        let mut view = self.blank_view(viewer)?;
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
            // Allocate from visible rows only; hidden IDs never enter a handle.
            // Initial/global card order is internal, but hidden rows do not
            // consume viewer counters. Equal permitted histories retain equal
            // counters even when hidden cards/IDs differ.
            for id in current {
                if !k.handles.values().any(|existing| *existing == id) {
                    let handle = format!("v{v}-o{}", k.next_handle);
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
            } else {
                match self.state.frame {
                    Frame::Priority if self.state.priority == viewer => {
                        Some((self.state.revision, 0))
                    }
                    Frame::Targets { .. } if self.top()?.controller == viewer => {
                        Some((self.state.revision, 1))
                    }
                    _ => None,
                }
            };
            if key != k.decision_key {
                k.decision_token = if key.is_some() {
                    let token = DecisionToken(format!("v{v}-d{}", k.next_decision));
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
            if history.last() != Some(&view) {
                history.push(view);
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
        match response {
            PlayerResponse::ActivateMana { object } => Ok(Response::ActivateMana {
                object: self.resolve_handle(viewer, object)?,
            }),
            PlayerResponse::Cast { card } => Ok(Response::Cast {
                card: self.resolve_handle(viewer, card)?.card,
            }),
            PlayerResponse::Pass => Ok(Response::Pass),
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

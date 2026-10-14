use crate::BUILD_ID;
use mtg_contracts::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const SPELL: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
// Reviewed canonical definition digests, independent of compiler success.
const ALLOWED: [(&str, &str); 5] = [
    (
        ISAMARU,
        "9d6619bfb5ceee58efd54c12ceb6f2b814f7a3e2d3b5e8261057894c6783b30f",
    ),
    (
        ISLAND,
        "b4ee04295bb8d7b19ceed34a30d31ab98feb92cca7df245cd1a51b3d783c8077",
    ),
    (
        FOREST,
        "e97708a1d40c178b8355f78c9c1f52e5a3cb7f04be1b9bc406d38598581bb63c",
    ),
    (
        SPELL,
        "b9238623f9c0a7ad0b202a16c3036cf21c59057e58964b0c16b42b319f5a4b89",
    ),
    (
        BEARS,
        "83c5d4c810e25b84f607f9c8b99b5179abf39e060d093cd7dd38c0be90a8a083",
    ),
];
mod casting;
mod events;
mod trace;
pub use events::*;
pub use trace::*;
mod multiplayer;
pub use multiplayer::*;
mod visibility;
mod work;
pub use visibility::*;
pub use work::*;
#[cfg(test)]
mod departure_tests;
#[cfg(test)]
mod event_tests;
#[cfg(test)]
mod p2_tests;
#[cfg(test)]
mod payment_event_tests;
#[cfg(test)]
mod payment_query_tests;
#[cfg(test)]
mod payment_tests;
#[cfg(test)]
mod stack_tests;

macro_rules! id {
    ($name:ident,$inner:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $inner);
    };
}
id!(PlayerId, u8);
id!(CardId, u32);
id!(StackId, u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectId {
    pub card: CardId,
    pub incarnation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardSetup {
    pub id: CardId,
    pub oracle_id: String,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Scenario,
    CommanderMatch,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioConfig {
    pub mode: Mode,
    pub players: u8,
    pub profile: String,
    pub commanders: Vec<CardId>,
    pub cards: Vec<CardSetup>,
}
impl ScenarioConfig {
    pub fn casting_reversal() -> Self {
        let mut config = Self::land_return();
        config.profile = "casting-mana-reversal-v1".into();
        config.cards[0].oracle_id = FOREST.into();
        config.cards.push(CardSetup {
            id: CardId(3),
            oracle_id: FOREST.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        });
        config
    }
    pub fn stack_response() -> Self {
        let mut config = Self::land_return();
        config.profile = "four-seat-instant-response-v1".into();
        for player in 1..4 {
            let land = CardId(2 * u32::from(player) + 1);
            let spell = CardId(land.0 + 1);
            config.cards.push(CardSetup {
                id: land,
                oracle_id: ISLAND.into(),
                owner: PlayerId(player),
                controller: PlayerId(player),
                zone: Zone::Battlefield,
            });
            config.cards.push(CardSetup {
                id: spell,
                oracle_id: SPELL.into(),
                owner: PlayerId(player),
                controller: PlayerId(player),
                zone: Zone::Hand {
                    owner: PlayerId(player),
                },
            });
        }
        config
    }

    pub fn commander_foundation() -> Self {
        let mut config = Self::land_return();
        config.profile = "commander-foundation-scenario-v1".into();
        config.commanders.push(CardId(3));
        config.cards.push(CardSetup {
            id: CardId(3),
            oracle_id: ISAMARU.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Command { owner: PlayerId(0) },
        });
        config
    }
    pub fn land_return() -> Self {
        Self {
            mode: Mode::Scenario,
            players: 4,
            profile: SCENARIO_PROFILE.into(),
            commanders: vec![],
            cards: vec![
                CardSetup {
                    id: CardId(0),
                    oracle_id: ISLAND.into(),
                    owner: PlayerId(0),
                    controller: PlayerId(0),
                    zone: Zone::Battlefield,
                },
                CardSetup {
                    id: CardId(1),
                    oracle_id: SPELL.into(),
                    owner: PlayerId(0),
                    controller: PlayerId(0),
                    zone: Zone::Hand { owner: PlayerId(0) },
                },
                CardSetup {
                    id: CardId(2),
                    oracle_id: BEARS.into(),
                    owner: PlayerId(1),
                    controller: PlayerId(2),
                    zone: Zone::Battlefield,
                },
            ],
        }
    }
}
#[derive(Clone)]
pub struct Prepared {
    catalog: Arc<Catalog>,
    catalog_digest: String,
    config: ScenarioConfig,
    profile_digest: String,
}
impl Prepared {
    pub fn admit(catalog: Catalog, config: ScenarioConfig) -> Result<Self, String> {
        catalog.validate()?;
        if config != ScenarioConfig::land_return()
            && config != ScenarioConfig::commander_foundation()
            && config != ScenarioConfig::stack_response()
            && config != ScenarioConfig::casting_reversal()
        {
            return Err(
                "configuration is outside the reviewed four-seat scenario; no match admission"
                    .into(),
            );
        }
        if catalog.definitions.len() != ALLOWED.len() {
            return Err("definition pool is not the closed reviewed scenario pool".into());
        }
        for d in &catalog.definitions {
            let expected = ALLOWED
                .iter()
                .find(|(id, _)| *id == d.oracle_id)
                .ok_or("definition is not admitted")?
                .1;
            if digest(d)? != expected {
                return Err("definition digest is not admitted by this build/profile".into());
            }
        }
        let catalog_digest = digest(&catalog)?;
        let profile_digest = digest(&(SCENARIO_PROFILE, ALLOWED, &config))?;
        Ok(Self {
            catalog: Arc::new(catalog),
            catalog_digest,
            config,
            profile_digest,
        })
    }
    pub fn catalog_digest(&self) -> &str {
        &self.catalog_digest
    }
    pub fn profile_digest(&self) -> &str {
        &self.profile_digest
    }
    pub fn config(&self) -> &ScenarioConfig {
        &self.config
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Player {
    life: Quantity,
    mana: BTreeMap<Color, Quantity>,
    status: PlayerStatus,
    last_known: Option<PlayerLastKnown>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Card {
    owner: PlayerId,
    oracle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Object {
    id: ObjectId,
    controller: PlayerId,
    default_controller: PlayerId,
    zone: Zone,
    tapped: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StackInstance {
    id: StackId,
    source: ObjectId,
    controller: PlayerId,
    targets: Vec<ObjectId>,
    announced_targets: Vec<Vec<ObjectHandle>>,
    paid: Option<ManaCost>,
    cast_from: Zone,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Casting {
    stack_id: StackId,
    original_object: ObjectId,
    prior_passes: Vec<PlayerId>,
    mana_activations: Vec<ManaActivationUndo>,
    undo_mana: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManaActivationUndo {
    object: ObjectId,
    player: PlayerId,
    color: Color,
    amount: Quantity,
    mana_was_absent: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resolving {
    stack_id: StackId,
    spell: StackInstance,
    cursor: u32,
    legal_targets: Vec<bool>,
    outcomes: Vec<InstructionOutcome>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum Frame {
    Priority,
    Targets { cast: Casting },
    Mana { cast: Casting },
    Payment { cast: Casting },
    Resolve { frame: Resolving },
    Fault { reason: String },
    Finished { winner: PlayerId },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    ActivateMana { object: ObjectId },
    Cast { card: CardId },
    Pass,
    FinishMana { undo_mana: bool },
    Targets { objects: Vec<ObjectId> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedInput {
    pub decision_id: u64,
    pub player: PlayerId,
    pub response: Response,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Mana {
        decision_id: u64,
        player: PlayerId,
        cost: ManaCost,
    },
    Priority {
        decision_id: u64,
        player: PlayerId,
    },
    Targets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<ObjectId>,
    },
}
impl Request {
    pub fn decision_id(&self) -> u64 {
        match self {
            Self::Priority { decision_id, .. }
            | Self::Targets { decision_id, .. }
            | Self::Mana { decision_id, .. } => *decision_id,
        }
    }
    pub fn player(&self) -> PlayerId {
        match self {
            Self::Priority { player, .. }
            | Self::Targets { player, .. }
            | Self::Mana { player, .. } => *player,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "proposal", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposedEvent {
    ReverseMana {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: Option<ObjectId>,
        remove_zero: bool,
    },
    Tap {
        object: ObjectId,
    },
    AddMana {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: ObjectId,
    },
    PayMana {
        player: PlayerId,
        cost: ManaCost,
    },
    Move {
        object: ObjectId,
        destination: Zone,
    },
    LeaveGame {
        object: ObjectId,
        departing_owner: PlayerId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveOutcome {
    pub previous: ObjectId,
    pub successor: ObjectId,
    pub from: Zone,
    pub destination: Zone,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventOutcome {
    ManaReversed {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: Option<ObjectId>,
        removed_entry: bool,
    },
    LeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    Tapped {
        object: ObjectId,
    },
    ManaAdded {
        player: PlayerId,
        color: Color,
        amount: Quantity,
    },
    ManaPaid {
        player: PlayerId,
        cost: ManaCost,
    },
    Moved {
        movement: MoveOutcome,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
enum InstructionOutcome {
    Moved { movement: MoveOutcome },
    IllegalTarget { target: ObjectId },
    ObjectNoLongerPresent { target: ObjectId },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    PlayerDeparted {
        player: PlayerId,
    },
    ObjectLeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    ControlEffectEnded {
        effect_id: u32,
    },
    ControlChanged {
        object: ObjectId,
        previous: PlayerId,
        controller: PlayerId,
    },
    StackRemovedForDeparture {
        stack_id: StackId,
        player: PlayerId,
    },
    GameWon {
        winner: PlayerId,
    },
    Committed {
        proposed: ProposedEvent,
        actual: EventOutcome,
    },
    SpellProposed {
        card: CardId,
        stack_id: StackId,
    },
    SpellCast {
        stack_id: StackId,
    },
    PriorityPassed {
        player: PlayerId,
    },
    SpellCompleted {
        stack_id: StackId,
        resolved: bool,
    },
    CastReversed {
        stack_id: StackId,
        reason: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    revision: u64,
    rule_steps: u64,
    visibility: Vec<visibility::Knowledge>,
    players: Vec<Player>,
    cards: BTreeMap<CardId, Card>,
    objects: BTreeMap<CardId, Object>,
    active: Option<PlayerId>,
    turn_owner: PlayerId,
    original_seating: Vec<PlayerId>,
    commanders: CommanderLedger,
    control_effects: Vec<ControlEffect>,
    external_inputs: Vec<ExternalInput>,
    pending_concessions: Vec<PlayerId>,
    priority: PlayerId,
    passes: Vec<PlayerId>,
    next_stack_id: u32,
    stack: Vec<StackInstance>,
    frame: Frame,
    pending: Option<AcceptedInput>,
    journal: Vec<AcceptedInput>,
    next_batch_id: u64,
    next_event_sequence: u64,
}
pub struct Game {
    prepared: Prepared,
    state: State,
    trace: TraceEvidence,
    executor: Arc<()>,
    state_token: Arc<()>,
}
pub struct ValidatedResponse {
    executor: Arc<()>,
    revision: u64,
    state_token: Arc<()>,
    input: AcceptedInput,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    snapshot_version: u32,
    build_id: String,
    catalog_digest: String,
    profile_digest: String,
    state_sha256: String,
    state: State,
    trace: TraceEvidence,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Advance {
    Finished { winner: PlayerId },
    AwaitingDecision(Request),
    Fault(String),
    Suspended { required_minimum: u64 },
}

impl Game {
    pub fn new(prepared: Prepared) -> Self {
        let cards = prepared
            .config
            .cards
            .iter()
            .map(|c| {
                (
                    c.id,
                    Card {
                        owner: c.owner,
                        oracle_id: c.oracle_id.clone(),
                    },
                )
            })
            .collect();
        let objects = prepared
            .config
            .cards
            .iter()
            .map(|c| {
                (
                    c.id,
                    Object {
                        id: ObjectId {
                            card: c.id,
                            incarnation: 0,
                        },
                        controller: c.controller,
                        default_controller: c.controller,
                        zone: c.zone,
                        tapped: false,
                    },
                )
            })
            .collect();
        let state = State {
            revision: 0,
            rule_steps: 0,
            visibility: (0..4).map(|_| visibility::Knowledge::default()).collect(),
            players: (0..4)
                .map(|_| Player {
                    life: Quantity::from(40),
                    mana: BTreeMap::new(),
                    status: PlayerStatus::Live,
                    last_known: None,
                })
                .collect(),
            cards,
            objects,
            active: Some(PlayerId(0)),
            turn_owner: PlayerId(0),
            original_seating: (0..4).map(PlayerId).collect(),
            commanders: CommanderLedger::new(),
            control_effects: vec![],
            external_inputs: vec![],
            pending_concessions: vec![],
            priority: PlayerId(0),
            passes: vec![],
            next_stack_id: 0,
            stack: vec![],
            frame: Frame::Priority,
            pending: None,
            journal: vec![],
            next_batch_id: 0,
            next_event_sequence: 0,
        };
        let mut game = Self {
            prepared,
            state,
            trace: Self::initial_trace(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
        };
        game.initialize_commanders()
            .expect("admitted commander configuration");
        game.refresh_visibility()
            .expect("admitted initial visibility");
        game
    }
    pub fn fork(&self) -> Self {
        Self {
            prepared: self.prepared.clone(),
            state: self.state.clone(),
            trace: self.trace.clone(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
        }
    }
    pub fn state_digest(&self) -> Result<String, String> {
        digest(&self.state)
    }
    pub fn events(&self) -> &[Event] {
        &self.trace.events
    }
    pub fn config(&self) -> &ScenarioConfig {
        self.prepared.config()
    }
    pub fn journal(&self) -> &[AcceptedInput] {
        &self.state.journal
    }
    pub fn object_id(&self, card: CardId) -> Option<ObjectId> {
        self.state.objects.get(&card).map(|o| o.id)
    }
    pub fn zone(&self, object: ObjectId) -> Option<Zone> {
        self.current(object).ok().map(|o| o.zone)
    }
    pub fn mana(&self, player: PlayerId, color: Color) -> Option<Quantity> {
        self.state
            .players
            .get(player.0 as usize)
            .map(|p| p.mana.get(&color).cloned().unwrap_or_default())
    }
    pub fn characteristics(&self, object: ObjectId) -> Result<Characteristics, String> {
        self.current(object)?;
        Ok(self.definition(object.card)?.characteristics.clone())
    }
    // Characteristics supply intrinsic behavior; reminder text never supplies
    // runtime code. CR 305.6 / 113.6 / 605.3b, pinned official rules:
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub fn mana_abilities(&self, object: ObjectId) -> Result<Vec<Color>, String> {
        let o = self.current(object)?;
        let d = self.definition(object.card)?;
        if o.zone != Zone::Battlefield || d.intrinsic_zone != Some(FunctioningZone::Battlefield) {
            return Ok(vec![]);
        }
        let c = self.characteristics(object)?;
        Ok(if c.kind == CardKind::Land {
            c.subtypes
                .iter()
                .filter_map(|s| basic_land_color(s))
                .collect()
        } else {
            vec![]
        })
    }
    fn definition(&self, card: CardId) -> Result<&Definition, String> {
        let physical = self.state.cards.get(&card).ok_or("unknown physical card")?;
        self.prepared
            .catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == physical.oracle_id)
            .ok_or_else(|| "missing admitted definition".into())
    }
    fn current(&self, id: ObjectId) -> Result<&Object, String> {
        self.state
            .objects
            .get(&id.card)
            .filter(|o| o.id == id)
            .ok_or_else(|| "stale or absent object incarnation".into())
    }
    fn target_legal(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| o.zone == Zone::Battlefield)
            && self
                .characteristics(id)
                .is_ok_and(|c| c.kind == CardKind::Creature)
    }
    fn top(&self) -> Result<&StackInstance, String> {
        self.state
            .stack
            .last()
            .ok_or_else(|| "missing stack frame".into())
    }
    fn program(&self) -> Result<&SpellProgram, String> {
        self.program_for(self.top()?)
    }
    fn program_for(&self, stack: &StackInstance) -> Result<&SpellProgram, String> {
        self.definition(stack.source.card)?
            .spell
            .as_ref()
            .ok_or_else(|| "missing spell program".into())
    }
    pub fn decision_for(&self, viewer: PlayerId) -> Result<Option<Request>, String> {
        if self.state.pending.is_some() || !self.state.pending_concessions.is_empty() {
            return Ok(None);
        }
        let req = match &self.state.frame {
            Frame::Priority => Request::Priority {
                decision_id: self.state.revision,
                player: self.state.priority,
            },
            Frame::Mana { .. } => Request::Mana {
                decision_id: self.state.revision,
                player: self.top()?.controller,
                cost: self
                    .definition(self.top()?.source.card)?
                    .characteristics
                    .mana_cost
                    .clone(),
            },
            Frame::Targets { .. } => Request::Targets {
                decision_id: self.state.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self
                    .state
                    .objects
                    .values()
                    .filter(|o| self.target_legal(o.id))
                    .map(|o| o.id)
                    .collect(),
            },
            _ => return Ok(None),
        };
        Ok((req.player() == viewer).then_some(req))
    }
    fn validate_response(&self, player: PlayerId, response: &Response) -> Result<(), String> {
        match (&self.state.frame, response) {
            (Frame::Priority, Response::Pass) if self.state.priority == player => Ok(()),
            (Frame::Priority, Response::ActivateMana { object })
                if self.state.priority == player =>
            {
                self.validate_mana_activation(player, *object)
            }
            (Frame::Mana { .. }, Response::ActivateMana { object })
                if self.top()?.controller == player =>
            {
                self.validate_mana_activation(player, *object)
            }
            (Frame::Mana { .. }, Response::FinishMana { .. })
                if self.top()?.controller == player =>
            {
                Ok(())
            }
            (Frame::Priority, Response::Cast { card }) if self.state.priority == player => {
                let o = self.state.objects.get(card).ok_or("invalid cast")?;
                if o.zone != (Zone::Hand { owner: player })
                    || self.state.cards[card].owner != player
                    || self.definition(*card)?.spell.is_none()
                    || self.state.stack.len() >= 4
                {
                    return Err("invalid or unsupported cast".into());
                }
                // Restrict proposals to spells with at least one eligible target;
                // the target-stage request never waits for an impossible choice.
                if !self.state.objects.values().any(|o| self.target_legal(o.id)) {
                    return Err("no legal target for this proposal".into());
                }
                Ok(())
            }
            (Frame::Targets { .. }, Response::Targets { objects })
                if self.top()?.controller == player =>
            {
                if objects.len() != self.program()?.targets.len()
                    || objects.iter().any(|o| !self.target_legal(*o))
                {
                    return Err("invalid targets".into());
                }
                Ok(())
            }
            _ => Err("invalid or unauthorized decision response".into()),
        }
    }
    pub fn check_response(
        &self,
        player: PlayerId,
        decision_id: u64,
        response: Response,
    ) -> Result<ValidatedResponse, String> {
        if self.state.pending.is_some()
            || decision_id != self.state.revision
            || self.decision_for(player)?.is_none()
        {
            return Err("stale or unauthorized decision".into());
        }
        self.validate_response(player, &response)?;
        Ok(ValidatedResponse {
            executor: self.executor.clone(),
            revision: self.state.revision,
            state_token: self.state_token.clone(),
            input: AcceptedInput {
                decision_id,
                player,
                response,
            },
        })
    }
    /// Submit is a bounded state transition; automatic rules run in advance.
    pub fn submit(&mut self, receipt: ValidatedResponse) -> Result<(), String> {
        if !Arc::ptr_eq(&receipt.executor, &self.executor)
            || !Arc::ptr_eq(&receipt.state_token, &self.state_token)
            || receipt.revision != self.state.revision
            || self.state.pending.is_some()
        {
            return Err("stale or foreign validation receipt".into());
        }
        let revision = self
            .state
            .revision
            .checked_add(1)
            .ok_or("structural revision resource limit")?;
        let mut next = self.fork();
        next.state.journal.push(receipt.input.clone());
        next.state.pending = Some(receipt.input);
        next.state.revision = revision;
        next.refresh_visibility()?;
        self.state = next.state;
        self.trace = next.trace;
        self.state_token = Arc::new(());
        Ok(())
    }
    pub fn respond(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        response: Response,
    ) -> Result<(), String> {
        let r = self.check_response(player, decision_id, response)?;
        self.submit(r)
    }
    pub fn advance(&mut self, mut budget: Budget) -> Advance {
        loop {
            if let Frame::Finished { winner } = self.state.frame {
                return Advance::Finished { winner };
            }
            if let Frame::Fault { reason } = &self.state.frame {
                return Advance::Fault(reason.clone());
            }
            if self.state.pending.is_none()
                && self.state.pending_concessions.is_empty()
                && matches!(
                    self.state.frame,
                    Frame::Priority | Frame::Targets { .. } | Frame::Mana { .. }
                )
            {
                let viewer = if matches!(self.state.frame, Frame::Priority) {
                    self.state.priority
                } else {
                    match self.top() {
                        Ok(s) => s.controller,
                        Err(e) => {
                            self.state.frame = Frame::Fault { reason: e.clone() };
                            return Advance::Fault(e);
                        }
                    }
                };
                return match self.decision_for(viewer) {
                    Ok(Some(r)) => Advance::AwaitingDecision(r),
                    _ => Advance::Fault("missing current decision".into()),
                };
            }
            if !budget.consume() {
                return Advance::Suspended {
                    required_minimum: 1,
                };
            }
            // A rules step commits atomically. Resource/invariant failures retain
            // its pre-step state and become faults, never wins/losses/draws.
            let mut next = self.fork();
            if let Err(reason) = next.step() {
                self.state.frame = Frame::Fault {
                    reason: reason.clone(),
                };
                self.state_token = Arc::new(());
                return Advance::Fault(reason);
            }
            let Some(steps) = next.state.rule_steps.checked_add(1) else {
                return Advance::Fault("execution boundary resource limit".into());
            };
            next.state.rule_steps = steps;
            if let Err(reason) = next.refresh_visibility() {
                return Advance::Fault(reason);
            }
            self.state = next.state;
            self.trace = next.trace;
            self.state_token = Arc::new(());
        }
    }
    fn step(&mut self) -> Result<(), String> {
        if let Some(player) = self.state.pending_concessions.first().copied() {
            self.state.pending_concessions.remove(0);
            return self.apply_departure(player);
        }
        match self.state.frame.clone() {
            Frame::Priority => {
                let input = self
                    .state
                    .pending
                    .take()
                    .ok_or("missing accepted priority input")?;
                match input.response {
                    Response::ActivateMana { object } => {
                        self.activate_intrinsic_mana(&input, object)?;
                        self.state.passes.clear();
                        self.stabilize()?;
                    }
                    Response::Cast { card } => {
                        let original_object = self
                            .state
                            .objects
                            .get(&card)
                            .ok_or("missing cast source")?
                            .id;
                        let id = StackId(self.state.next_stack_id);
                        self.state.next_stack_id = self
                            .state
                            .next_stack_id
                            .checked_add(1)
                            .ok_or("stack ID resource limit")?;
                        self.record_event(Event::SpellProposed { card, stack_id: id })?;
                        let source = self
                            .move_object_for(
                                original_object,
                                Zone::Stack,
                                BatchCause::Casting {
                                    stack_id: id,
                                    player: input.player,
                                    source: original_object,
                                },
                            )?
                            .successor;
                        self.state.stack.push(StackInstance {
                            id,
                            source,
                            controller: input.player,
                            targets: vec![],
                            announced_targets: vec![vec![]; 4],
                            paid: None,
                            cast_from: Zone::Hand {
                                owner: input.player,
                            },
                        });
                        self.state.frame = Frame::Targets {
                            cast: Casting {
                                stack_id: id,
                                original_object,
                                prior_passes: self.state.passes.clone(),
                                mana_activations: vec![],
                                undo_mana: false,
                            },
                        };
                        self.state.passes.clear();
                    }
                    Response::Pass => {
                        self.record_event(Event::PriorityPassed {
                            player: input.player,
                        })?;
                        self.state.passes.push(input.player);
                        // CR 117.4 applies to the current live player set.
                        if self.all_live_passed() {
                            self.begin_resolution()?;
                        } else {
                            self.state.priority = self
                                .next_live(input.player)
                                .ok_or("no live priority recipient")?;
                            self.stabilize()?;
                        }
                    }
                    _ => return Err("response/frame invariant".into()),
                }
            }
            Frame::Targets { cast } => {
                let input = self
                    .state
                    .pending
                    .take()
                    .ok_or("missing accepted target input")?;
                let Response::Targets { objects } = input.response else {
                    return Err("target response/frame invariant".into());
                };
                let announced = (0..4)
                    .map(|v| {
                        objects
                            .iter()
                            .map(|object| self.handle_for(PlayerId(v), *object))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let top = self.state.stack.last_mut().ok_or("missing stack")?;
                top.targets = objects;
                top.announced_targets = announced;
                self.state.frame = Frame::Mana { cast };
            }
            Frame::Mana { mut cast } => {
                let input = self
                    .state
                    .pending
                    .take()
                    .ok_or("missing accepted mana input")?;
                match input.response {
                    Response::ActivateMana { object } => {
                        let receipt = self.activate_intrinsic_mana(&input, object)?;
                        cast.mana_activations.push(receipt);
                        self.state.frame = Frame::Mana { cast };
                    }
                    Response::FinishMana { undo_mana } => {
                        cast.undo_mana = undo_mana;
                        self.state.frame = Frame::Payment { cast };
                    }
                    _ => return Err("mana response/frame invariant".into()),
                }
            }
            Frame::Payment { cast } => {
                let stack = self.top()?.clone();
                let cost = self
                    .definition(stack.source.card)?
                    .characteristics
                    .mana_cost
                    .clone();
                if self.payment_pool(stack.controller, &cost).is_err() {
                    let undo = cast.undo_mana;
                    self.reverse_cast(cast, "insufficient mana", undo)?;
                    self.stabilize()?;
                } else {
                    self.commit(
                        ProposedEvent::PayMana {
                            player: stack.controller,
                            cost: cost.clone(),
                        },
                        BatchCause::Payment {
                            stack_id: cast.stack_id,
                            player: stack.controller,
                        },
                    )?;
                    self.state.stack.last_mut().ok_or("missing stack")?.paid = Some(cost);
                    self.record_event(Event::SpellCast {
                        stack_id: cast.stack_id,
                    })?;
                    self.state.commanders.record_cast(
                        stack.source.card,
                        stack.controller,
                        stack.cast_from,
                    )?;
                    self.state.priority = stack.controller;
                    self.state.frame = Frame::Priority;
                    self.stabilize()?;
                }
            }
            Frame::Resolve { mut frame } => {
                let stack = frame.spell.clone();
                let program = self.program_for(&stack)?.clone();
                // One spell-wide legality check before any block resolves.
                // CR 608.2b: if all targets are illegal, the entire spell does
                // not resolve, including its otherwise untargeted blocks.
                if !frame.legal_targets.iter().any(|l| *l) {
                    self.finish_spell(&stack, false)?;
                } else if let Some(instruction) = program.instructions.get(frame.cursor as usize) {
                    match instruction {
                        Instruction::ReturnToOwnerHand {
                            target_slot,
                            source_block,
                        } => {
                            let target = stack.targets[*target_slot as usize];
                            let outcome = if !frame.legal_targets[*target_slot as usize] {
                                InstructionOutcome::IllegalTarget { target }
                            } else if self.current(target).is_err() {
                                InstructionOutcome::ObjectNoLongerPresent { target }
                            } else {
                                let owner = self.state.cards[&target.card].owner;
                                InstructionOutcome::Moved {
                                    movement: self.move_object_for(
                                        target,
                                        Zone::Hand { owner },
                                        BatchCause::Instruction {
                                            stack_id: stack.id,
                                            player: stack.controller,
                                            source: stack.source,
                                            cursor: frame.cursor,
                                            source_block: *source_block,
                                        },
                                    )?,
                                }
                            };
                            frame.outcomes.push(outcome);
                        }
                    }
                    frame.cursor = frame
                        .cursor
                        .checked_add(1)
                        .ok_or("instruction cursor resource limit")?;
                    self.state.frame = Frame::Resolve { frame };
                } else {
                    self.finish_spell(&stack, true)?;
                }
            }
            Frame::Fault { reason } => return Err(reason),
            Frame::Finished { .. } => {}
        }
        Ok(())
    }
    fn finish_spell(&mut self, stack: &StackInstance, resolved: bool) -> Result<(), String> {
        let owner = self.state.cards[&stack.source.card].owner;
        if self.current(stack.source).is_ok() {
            if self.top()?.id != stack.id {
                return Err("completion is not the top stack entry".into());
            }
            self.move_object_for(
                stack.source,
                Zone::Graveyard { owner },
                BatchCause::SpellCompletion {
                    stack_id: stack.id,
                    player: stack.controller,
                    source: stack.source,
                },
            )?;
            self.state.stack.pop();
        } else if self.is_live(owner) || self.state.stack.iter().any(|s| s.id == stack.id) {
            return Err("unsupported resolving-source movement".into());
        }
        self.record_event(Event::SpellCompleted {
            stack_id: stack.id,
            resolved,
        })?;
        self.state.frame = Frame::Priority;
        self.state.priority = self
            .state
            .active
            .or_else(|| self.next_live(self.state.turn_owner))
            .ok_or("no live player after resolution")?;
        self.stabilize()
    }
    fn payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
    ) -> Result<BTreeMap<Color, Quantity>, String> {
        let mut pool = self
            .state
            .players
            .get(player.0 as usize)
            .ok_or("missing paying player")?
            .mana
            .clone();
        for color in &cost.colored {
            let n = pool.entry(*color).or_default();
            if *n < Quantity::from(1) {
                return Err("insufficient mana".into());
            }
            n.subtract(&Quantity::from(1));
        }
        let mut generic = cost.generic.clone();
        for n in pool.values_mut() {
            let spent = std::cmp::min(n.clone(), generic.clone());
            n.subtract(&spent);
            generic.subtract(&spent);
        }
        if generic > Quantity::zero() {
            return Err("insufficient mana".into());
        }
        Ok(pool)
    }
    fn stabilize(&self) -> Result<(), String> {
        // CR 704.3: this entry point runs before granting priority. The closed
        // scenario has no life changes, marked damage, counters, or triggers.
        // Unsupported reachable conditions fault instead of being skipped.
        if self
            .state
            .players
            .iter()
            .any(|p| p.life != Quantity::from(40))
        {
            return Err("unsupported reachable life/SBA state".into());
        }
        for o in self
            .state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
        {
            let c = self.characteristics(o.id)?;
            if c.kind == CardKind::Creature
                && c.toughness.as_ref().is_none_or(|n| n <= &Quantity::zero())
            {
                return Err("unsupported reachable creature SBA state".into());
            }
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Result<String, String> {
        canonical(&Snapshot {
            snapshot_version: 6,
            build_id: BUILD_ID.into(),
            catalog_digest: self.prepared.catalog_digest.clone(),
            profile_digest: self.prepared.profile_digest.clone(),
            state_sha256: self.state_digest()?,
            state: self.state.clone(),
            trace: self.trace.clone(),
        })
    }
    pub fn restore(prepared: Prepared, json: &str) -> Result<Self, String> {
        let s: Snapshot = strict_read(json)?;
        if s.snapshot_version != 6
            || s.build_id != BUILD_ID
            || s.catalog_digest != prepared.catalog_digest
            || s.profile_digest != prepared.profile_digest
            || digest(&s.state)? != s.state_sha256
        {
            return Err("snapshot version/build/catalog/profile/hash mismatch".into());
        }
        let game = Self {
            prepared,
            state: s.state,
            trace: s.trace,
            executor: Arc::new(()),
            state_token: Arc::new(()),
        };
        game.validate_state()?;
        // The first profile is tiny: verify reachability by replay rather than
        // accepting a rehashed arbitrary state through the recovery API.
        let mut replayed = Self::replay_to(
            game.prepared.clone(),
            &game.replay_journal(),
            Some(game.state.rule_steps),
            game.trace.mode,
        )?;
        if matches!(game.state.frame, Frame::Fault { .. })
            && !matches!(replayed.state.frame, Frame::Fault { .. })
        {
            let _ = replayed.advance(Budget::new(1));
        }
        if replayed.state != game.state || replayed.trace != game.trace {
            return Err("snapshot is not reachable from this scenario's accepted inputs".into());
        }
        Ok(game)
    }
    fn validate_state(&self) -> Result<(), String> {
        let s = &self.state;
        if s.players.len() != 4
            || s.visibility.len() != 4
            || s.turn_owner != PlayerId(0)
            || s.active.is_some_and(|p| !self.is_live(p))
            || s.original_seating != (0..4).map(PlayerId).collect::<Vec<_>>()
            || s.priority.0 >= 4
            || !self.is_live(s.priority)
            || s.passes.len() > self.live_players().len()
            || s.passes.iter().any(|p| !self.is_live(*p))
            || s.passes.iter().collect::<BTreeSet<_>>().len() != s.passes.len()
            || s.cards.len() != self.prepared.config.cards.len()
            || s.objects.len() > s.cards.len()
            || s.stack.len() > 4
        {
            return Err("snapshot seating/table invariant".into());
        }
        for setup in &self.prepared.config.cards {
            let card = s.cards.get(&setup.id).ok_or("snapshot missing card")?;
            if card.owner != setup.owner || card.oracle_id != setup.oracle_id {
                return Err("snapshot physical identity invariant".into());
            }
            let Some(o) = s.objects.get(&setup.id) else {
                if self.is_live(setup.owner) {
                    return Err("snapshot missing live owner's object".into());
                }
                continue;
            };
            if !self.is_live(card.owner) {
                return Err("departed owner still has an object".into());
            }
            if o.id.card != setup.id
                || o.controller.0 >= 4
                || o.default_controller.0 >= 4
                || !self.is_live(o.controller)
                || (o.tapped && self.definition(setup.id)?.characteristics.kind != CardKind::Land)
            {
                return Err("snapshot object invariant".into());
            }
            if let Zone::Hand { owner } | Zone::Graveyard { owner } = o.zone
                && owner != card.owner
            {
                return Err("snapshot zone-owner invariant".into());
            }
        }
        for p in &s.players {
            if (p.status == PlayerStatus::Left) != p.last_known.is_some()
                || p.last_known
                    .as_ref()
                    .is_some_and(|last| last.life != p.life || last.mana != p.mana)
            {
                return Err("snapshot departed-player history invariant".into());
            }
            if p.mana.values().any(|n| n.is_negative()) {
                return Err("snapshot negative mana".into());
            }
        }
        let ids: BTreeSet<_> = s.stack.iter().map(|entry| entry.id).collect();
        let sources: BTreeSet<_> = s.stack.iter().map(|entry| entry.source.card).collect();
        if ids.len() != s.stack.len()
            || sources.len() != s.stack.len()
            || !s.stack.windows(2).all(|w| w[0].id < w[1].id)
        {
            return Err("snapshot duplicate or unordered stack identity".into());
        }
        for (index, stack) in s.stack.iter().enumerate() {
            if stack.id.0 >= s.next_stack_id
                || !self.is_live(stack.controller)
                || self.current(stack.source)?.zone != Zone::Stack
            {
                return Err("snapshot stack identity invariant".into());
            }
            let n = self.program_for(stack)?.targets.len();
            if stack.announced_targets.len() != 4
                || stack
                    .announced_targets
                    .iter()
                    .any(|a| a.len() != stack.targets.len())
            {
                return Err("snapshot target announcement invariant".into());
            }
            let incomplete_top = index + 1 == s.stack.len()
                && (matches!(
                    s.frame,
                    Frame::Targets { .. } | Frame::Mana { .. } | Frame::Payment { .. }
                ) || (matches!(s.frame, Frame::Finished { .. } | Frame::Fault { .. })
                    && stack.paid.is_none()));
            if !incomplete_top
                && (stack.targets.len() != n
                    || stack.paid.as_ref()
                        != Some(
                            &self
                                .definition(stack.source.card)?
                                .characteristics
                                .mana_cost,
                        ))
            {
                return Err("snapshot completed lower/top cast invariant".into());
            }
        }
        let stack_cards: BTreeSet<_> = s.stack.iter().map(|v| v.source.card).collect();
        if s.objects
            .values()
            .any(|o| (o.zone == Zone::Stack) != stack_cards.contains(&o.id.card))
        {
            return Err("snapshot stack/zone invariant".into());
        }
        match &s.frame {
            Frame::Targets { cast } | Frame::Mana { cast } | Frame::Payment { cast } => {
                let top = self.top()?;
                if cast.stack_id != top.id
                    || cast.original_object.card != top.source.card
                    || cast.original_object.incarnation.checked_add(1)
                        != Some(top.source.incarnation)
                    || top.paid.is_some()
                {
                    return Err("snapshot casting frame invariant".into());
                }
                let sources: BTreeSet<_> = cast.mana_activations.iter().map(|a| a.object).collect();
                if sources.len() != cast.mana_activations.len()
                    || cast.mana_activations.len() > self.prepared.config.cards.len()
                    || cast
                        .mana_activations
                        .iter()
                        .any(|a| a.player != top.controller || a.amount != Quantity::from(1))
                    || (matches!(s.frame, Frame::Targets { .. })
                        && (!cast.mana_activations.is_empty() || cast.undo_mana))
                    || (matches!(s.frame, Frame::Mana { .. }) && cast.undo_mana)
                {
                    return Err("snapshot casting mana receipt invariant".into());
                }
                if matches!(s.frame, Frame::Targets { .. }) && !top.targets.is_empty() {
                    return Err("snapshot announcement target invariant".into());
                }
                if matches!(s.frame, Frame::Mana { .. } | Frame::Payment { .. })
                    && top.targets.len() != self.program()?.targets.len()
                {
                    return Err("snapshot payment target invariant".into());
                }
            }
            Frame::Resolve { frame } => {
                let spell = &frame.spell;
                let program = self.program_for(spell)?;
                if spell.id != frame.stack_id
                    || spell.id.0 >= s.next_stack_id
                    || spell.targets.len() != program.targets.len()
                    || spell.paid.as_ref()
                        != Some(
                            &self
                                .definition(spell.source.card)?
                                .characteristics
                                .mana_cost,
                        )
                    || frame.cursor as usize > program.instructions.len()
                    || frame.outcomes.len() != frame.cursor as usize
                    || frame.legal_targets.len() != program.targets.len()
                {
                    return Err("snapshot resolving invocation invariant".into());
                }
                if self.current(spell.source).is_ok() {
                    if self.top()? != spell {
                        return Err("snapshot resolving top binding".into());
                    }
                } else if self.is_live(s.cards[&spell.source.card].owner)
                    || s.stack.iter().any(|entry| entry.id == spell.id)
                {
                    return Err("snapshot detached invocation owner/source invariant".into());
                }
            }
            _ => {}
        }
        if let Some(pending) = &s.pending {
            if pending.decision_id >= s.revision || s.journal.last() != Some(pending) {
                return Err("snapshot pending-input invariant".into());
            }
            if !matches!(s.frame, Frame::Fault { .. }) {
                self.validate_response(pending.player, &pending.response)?;
            }
        }
        self.stabilize()
    }
    pub fn replay(prepared: Prepared, journal: &ReplayJournal) -> Result<Self, String> {
        Self::replay_to(prepared, journal, None, TraceMode::Detailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepared() -> Prepared {
        Prepared::admit(
            Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap()
    }
    fn respond(game: &mut Game, response: Response) {
        let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
            panic!("expected decision")
        };
        game.respond(r.player(), r.decision_id(), response).unwrap();
    }
    fn cast(game: &mut Game) {
        respond(
            game,
            Response::ActivateMana {
                object: game.object_id(CardId(0)).unwrap(),
            },
        );
        respond(game, Response::Cast { card: CardId(1) });
    }
    fn complete(game: &mut Game) {
        respond(
            game,
            Response::Targets {
                objects: vec![game.object_id(CardId(2)).unwrap()],
            },
        );
        respond(game, Response::FinishMana { undo_mana: false });
        for _ in 0..4 {
            respond(game, Response::Pass);
        }
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(_)
        ));
    }
    #[test]
    fn complete_source_scenario_preserves_owner_and_incarnations() {
        let mut game = Game::new(prepared());
        cast(&mut game);
        complete(&mut game);
        assert_eq!(
            game.state.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(1) }
        );
        assert_eq!(game.state.objects[&CardId(2)].id.incarnation, 1);
        assert_eq!(
            game.state.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        assert_eq!(game.state.objects[&CardId(1)].id.incarnation, 2);
        assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
        assert_eq!(game.state.priority, PlayerId(0));
        assert_eq!(game.events().len(), 13);
    }
    #[test]
    fn snapshot_fork_and_replay_agree_and_receipts_do_not_cross_executors() {
        let p = prepared();
        let mut game = Game::new(p.clone());
        cast(&mut game);
        let _ = game.advance(Budget::unlimited());
        let mut restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
        let mut fork = game.fork();
        let response = Response::Targets {
            objects: vec![game.object_id(CardId(2)).unwrap()],
        };
        let req = game.decision_for(PlayerId(0)).unwrap().unwrap();
        let receipt = game
            .check_response(PlayerId(0), req.decision_id(), response)
            .unwrap();
        assert!(fork.submit(receipt).is_err());
        complete(&mut game);
        complete(&mut restored);
        complete(&mut fork);
        assert_eq!(game.state, restored.state);
        assert_eq!(game.state, fork.state);
        let replay = Game::replay(p, &game.replay_journal()).unwrap();
        assert_eq!(game.state, replay.state);
    }
    #[test]
    fn queries_and_invalid_responses_leave_state_unchanged() {
        let mut game = Game::new(prepared());
        let before = game.state_digest().unwrap();
        for _ in 0..3 {
            for viewer in 0..4 {
                let _ = game.decision_for(PlayerId(viewer)).unwrap();
            }
            let _ = game
                .characteristics(game.object_id(CardId(2)).unwrap())
                .unwrap();
            let _ = game
                .mana_abilities(game.object_id(CardId(0)).unwrap())
                .unwrap();
        }
        assert!(game.respond(PlayerId(1), 0, Response::Pass).is_err());
        assert!(game.respond(PlayerId(0), 99, Response::Pass).is_err());
        assert!(
            game.respond(
                PlayerId(0),
                0,
                Response::ActivateMana {
                    object: ObjectId {
                        card: CardId(0),
                        incarnation: 9
                    }
                }
            )
            .is_err()
        );
        assert_eq!(game.state_digest().unwrap(), before);
        cast(&mut game);
        let _ = game.advance(Budget::unlimited());
        let before = game.state_digest().unwrap();
        assert!(
            game.respond(
                PlayerId(0),
                game.state.revision,
                Response::Targets {
                    objects: vec![ObjectId {
                        card: CardId(2),
                        incarnation: 9
                    }]
                }
            )
            .is_err()
        );
        assert_eq!(game.state_digest().unwrap(), before);
    }
    #[test]
    fn payment_reversal_retains_inputs_and_original_incarnation() {
        let mut game = Game::new(prepared());
        let original = game.object_id(CardId(1)).unwrap();
        respond(&mut game, Response::Cast { card: CardId(1) });
        respond(
            &mut game,
            Response::Targets {
                objects: vec![ObjectId {
                    card: CardId(2),
                    incarnation: 0,
                }],
            },
        );
        respond(&mut game, Response::FinishMana { undo_mana: true });
        let _ = game.advance(Budget::unlimited());
        assert_eq!(game.zone(original), Some(Zone::Hand { owner: PlayerId(0) }));
        assert!(game.state.stack.is_empty());
        assert_eq!(game.journal().len(), 3);
        assert!(matches!(
            game.events().last(),
            Some(Event::CastReversed { .. })
        ));
        let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
        assert_eq!(replay.state, game.state);
    }
    #[test]
    fn pending_accepted_input_survives_restore_and_rehashed_impossible_state_rejects() {
        let p = prepared();
        let mut game = Game::new(p.clone());
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected decision")
        };
        game.respond(
            PlayerId(0),
            request.decision_id(),
            Response::ActivateMana {
                object: ObjectId {
                    card: CardId(0),
                    incarnation: 0,
                },
            },
        )
        .unwrap();
        let mut restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
        assert_eq!(game.state, restored.state);
        let _ = game.advance(Budget::unlimited());
        let _ = restored.advance(Budget::unlimited());
        assert_eq!(game.state, restored.state);
        let mut snapshot: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
        snapshot.state.players[0]
            .mana
            .insert(Color::U, Quantity::from(99));
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
    }
    #[test]
    fn forest_intrinsic_is_derived_and_partial_multiblock_legality_keeps_other_block() {
        let mut game = Game::new(prepared());
        game.state.cards.get_mut(&CardId(0)).unwrap().oracle_id = FOREST.into();
        assert_eq!(
            game.mana_abilities(game.object_id(CardId(0)).unwrap())
                .unwrap(),
            [Color::G]
        );
        let mut p = prepared();
        let mut c = (*p.catalog).clone();
        let program = c
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == SPELL)
            .unwrap()
            .spell
            .as_mut()
            .unwrap();
        let mut second = program.targets[0].clone();
        second.slot = 1;
        second.selection_id = "synthetic:second".into();
        program.targets.push(second);
        program.instructions.push(Instruction::ReturnToOwnerHand {
            target_slot: 1,
            source_block: 1,
        });
        p.catalog = Arc::new(c);
        let mut game = Game::new(p);
        cast(&mut game);
        respond(
            &mut game,
            Response::Targets {
                objects: vec![
                    ObjectId {
                        card: CardId(2),
                        incarnation: 0
                    };
                    2
                ],
            },
        );
        respond(&mut game, Response::FinishMana { undo_mana: false });
        let _ = game.advance(Budget::unlimited());
        // Test-only inaccessible mutation makes one occurrence stale. Both
        // blocks still share one resolution legality check and stack object.
        game.state.stack.last_mut().unwrap().targets[0].incarnation = 99;
        for _ in 0..4 {
            respond(&mut game, Response::Pass);
        }
        let _ = game.advance(Budget::unlimited());
        assert_eq!(
            game.state.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(1) }
        );
        assert!(matches!(
            game.events().last(),
            Some(Event::SpellCompleted { resolved: true, .. })
        ));
    }
    #[test]
    fn unsupported_configuration_and_modified_ir_reject_admission() {
        let p = prepared();
        let mut config = ScenarioConfig::land_return();
        config.mode = Mode::CommanderMatch;
        assert!(Prepared::admit((*p.catalog).clone(), config).is_err());
        let mut c = (*p.catalog).clone();
        c.definitions[0].name = "modified definition".into();
        assert!(Prepared::admit(c, ScenarioConfig::land_return()).is_err());
    }
    #[test]
    fn stale_resolution_target_does_not_follow_physical_card() {
        let mut game = Game::new(prepared());
        cast(&mut game);
        respond(
            &mut game,
            Response::Targets {
                objects: vec![ObjectId {
                    card: CardId(2),
                    incarnation: 0,
                }],
            },
        );
        respond(&mut game, Response::FinishMana { undo_mana: false });
        let _ = game.advance(Budget::unlimited());
        let target = game.object_id(CardId(2)).unwrap();
        let moved = game
            .move_object(target, Zone::Hand { owner: PlayerId(1) })
            .unwrap();
        game.move_object(moved.successor, Zone::Battlefield)
            .unwrap();
        for _ in 0..4 {
            respond(&mut game, Response::Pass);
        }
        let _ = game.advance(Budget::unlimited());
        assert_eq!(game.state.objects[&CardId(2)].zone, Zone::Battlefield);
        assert!(matches!(
            game.events().last(),
            Some(Event::SpellCompleted {
                resolved: false,
                ..
            })
        ));
    }
    #[test]
    fn synthetic_multiple_blocks_have_one_target_check_and_stack_frame() {
        let mut p = prepared();
        let mut c = (*p.catalog).clone();
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == SPELL)
            .unwrap();
        let program = d.spell.as_mut().unwrap();
        program.instructions.push(Instruction::ReturnToOwnerHand {
            target_slot: 0,
            source_block: 1,
        });
        // Test-only interpreter fixture. No public admission bypass is provided.
        p.catalog = Arc::new(c);
        let mut game = Game::new(p);
        cast(&mut game);
        complete(&mut game);
        let target_moves=game.events().iter().filter(|e|matches!(e,Event::Committed{proposed:ProposedEvent::Move{object,..},..} if object.card==CardId(2))).count();
        assert_eq!(target_moves, 1);
        assert_eq!(
            game.state.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(1) }
        );
    }
    #[test]
    fn zone_intrinsics_and_corrupt_snapshot_fail_closed() {
        let p = prepared();
        let mut game = Game::new(p.clone());
        let id = game.object_id(CardId(0)).unwrap();
        assert_eq!(game.mana_abilities(id).unwrap(), vec![Color::U]);
        let movement = game
            .move_object(id, Zone::Hand { owner: PlayerId(0) })
            .unwrap();
        assert!(game.mana_abilities(movement.successor).unwrap().is_empty());
        assert!(
            Game::restore(
                p.clone(),
                &game.snapshot().unwrap().replace(BUILD_ID, "wrong-build")
            )
            .is_err()
        );
        let mut snapshot: Snapshot =
            strict_read(&Game::new(p.clone()).snapshot().unwrap()).unwrap();
        snapshot.state.objects.get_mut(&CardId(2)).unwrap().id.card = CardId(99);
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
    }
}

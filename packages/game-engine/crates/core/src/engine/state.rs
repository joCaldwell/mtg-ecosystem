//! Runtime ownership is distinct from the stable, flat snapshot contract.
//! Rules, player knowledge and immutable audit entries remain equally bound by
//! recovery hashes. No history is discarded or treated as nonsemantic here.
use super::*;

mod access;
mod error;
mod model;
mod objects;
mod shared;
mod txn;
mod validate;
pub use error::RuleError;
pub use model::*;
pub use objects::ObjectTable;
pub use shared::Shared;
pub use txn::{Game, PreEvent};

// Declare each field once. The flat decoder keeps deny_unknown_fields without
// serde's flatten/unknown-field ambiguity, while the encoder borrows groups.
macro_rules! state_layout {
    (rules { $( $(#[$ra:meta])* $r:ident: $rt:ty, )* }
     knowledge { $( $(#[$ka:meta])* $k:ident: $kt:ty, )* }
     audit { $( $(#[$aa:meta])* $a:ident: $at:ty, )* }) => {
        /// The rules tables. Every routine reads them through `RulesState`;
        /// only `RulesTxn::rules_mut` hands out exclusive access.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub struct RulesTables { $( $(#[$ra])* pub $r: $rt, )* }
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub struct KnowledgeState { $( $(#[$ka])* pub $k: $kt, )* }
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub struct AuditState { $( $(#[$aa])* pub $a: $at, )* }
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub struct State {
            #[serde(flatten)] pub rules: RulesState,
            #[serde(flatten)] pub knowledge: KnowledgeState,
            #[serde(flatten)] pub audit: AuditState,
        }
        impl<'de> Deserialize<'de> for State {
            fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Flat {
                    $( $(#[$ra])* $r: $rt, )*
                    $( $(#[$ka])* $k: $kt, )*
                    $( $(#[$aa])* $a: $at, )*
                }
                let Flat { $( $r, )* $( $k, )* $( $a, )* } = Flat::deserialize(decoder)?;
                Ok(Self {
                    rules: RulesState::new(RulesTables { $( $r, )* }),
                    knowledge: KnowledgeState { $( $k, )* },
                    audit: AuditState { $( $a, )* },
                })
            }
        }
    };
}
/// The rules state of a game. Its tables are private to this module: the rest
/// of the engine reads them through `Deref` and writes them only through
/// `RulesTxn::rules_mut`, so every write to the rules is explicit and stays
/// inside the staged copy a transaction commits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct RulesState {
    tables: RulesTables,
}
impl RulesState {
    pub fn new(tables: RulesTables) -> Self {
        Self { tables }
    }
}
impl std::ops::Deref for RulesState {
    type Target = RulesTables;
    fn deref(&self) -> &RulesTables {
        &self.tables
    }
}
// White-box tests build and forge states directly; production code cannot.
#[cfg(any(test, feature = "testkit"))]
impl std::ops::DerefMut for RulesState {
    fn deref_mut(&mut self) -> &mut RulesTables {
        &mut self.tables
    }
}
impl RulesTxn {
    /// Exclusive access to the rules tables: the single write path.
    pub(in crate::engine) fn rules_mut(&mut self) -> &mut RulesTables {
        self.priority_cache.take();
        &mut self.state.rules.tables
    }
}

// Private ownership only: staged forks share tables that change only when a
// token is created. Serialized shape and typed equality remain the map's own.
mod shared_map {
    use super::*;
    pub(super) fn serialize<K: Serialize, V: Serialize, S: serde::Serializer>(
        value: &Arc<BTreeMap<K, V>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_ref().serialize(serializer)
    }
    pub(super) fn deserialize<'de, K, V, D>(
        deserializer: D,
    ) -> Result<Arc<BTreeMap<K, V>>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: serde::Deserializer<'de>,
    {
        BTreeMap::deserialize(deserializer).map(Arc::new)
    }
    pub(super) fn is_empty<K, V>(value: &Arc<BTreeMap<K, V>>) -> bool {
        value.is_empty()
    }
}
fn initial_attachment_timestamp() -> u64 {
    1
}
fn attachment_timestamp_is_initial(timestamp: &u64) -> bool {
    *timestamp == initial_attachment_timestamp()
}
state_layout! {
    rules {
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        mana_ability_uses: BTreeSet<mana_permanents::ManaAbilityUse>,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        activation_uses: BTreeSet<stack_costs::ActivationUse>,
        #[serde(default, skip_serializing_if = "shared_map::is_empty", with = "shared_map")]
        tokens: Arc<BTreeMap<CardId, tokens::TokenIdentity>>,
        // Deterministic conservative charge for inputs and potential detailed
        // evidence, even in Off/Compact modes. Replay proves the counter.
        #[serde(default, skip_serializing_if = "stored_history::is_zero_charge")]
        retained_auxiliary_charge: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        typed_fault: Option<RuleFault>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_state: Option<MatchState>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Shared<Vec<ResolvedEffect>>,
        #[serde(default, skip_serializing_if = "triggers::is_zero")]
        next_effect_id: u64,
        #[serde(default, skip_serializing_if = "triggers::is_zero")]
        next_damage_transaction_id: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pending_triggers: Vec<PendingTrigger>,
        resolution_triggers: Shared<BTreeMap<StackId, resolution_triggers::ResolutionTrigger>>,
        resolution_trigger_clock: resolution_triggers::ResolutionTriggerClock,
        #[serde(default, skip_serializing_if = "triggers::is_zero")]
        next_trigger_id: u64,
        libraries: Shared<BTreeMap<PlayerId, Vec<CardId>>>,
        combat: Vec<Attacker>,
        combat_declared: bool,
        damage_choices: BTreeMap<ObjectId, Vec<(ObjectId, Quantity)>>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        player_damage_choices: BTreeMap<ObjectId, (PlayerId, Quantity)>,
        /// CR510.4/702.7b: incarnation-bound first-step eligibility, retained even
        /// after combat departure. Empty in every historical profile.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        first_strike_participants: BTreeSet<ObjectId>,
        /// CR509.1g/506.4/511.3: blocking status is independent of the remaining
        /// attacker links and lasts until this creature leaves combat.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        blocking_creatures: BTreeSet<ObjectId>,
        cleanup_repeat: bool,
        simultaneous_losses: bool,
        losing_players: Vec<PlayerId>,
        revision: u64,
        rule_steps: u64,
        players: Vec<Player>,
        #[serde(with = "shared_map")]
        cards: Arc<BTreeMap<CardId, Card>>,
        objects: ObjectTable,
        active: Option<PlayerId>,
        turn_owner: PlayerId,
        turn: TurnState,
        original_seating: Vec<PlayerId>,
        cast_history: [triggers::TurnCastHistory; 4],
        commanders: CommanderLedger,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        commander_returns: BTreeSet<ObjectId>,
        control_effects: Vec<ControlEffect>,
        // CR 613.7: semantic attachment order, independent of IDs and trace.
        // Like other unused allocation counters, omit the initial value. This
        // leaves unrelated scenario states unchanged; consumed history is kept.
        #[serde(default = "initial_attachment_timestamp", skip_serializing_if = "attachment_timestamp_is_initial")]
        next_attachment_timestamp: u64,
        pending_concessions: Vec<PlayerId>,
        priority: PlayerId,
        passes: Vec<PlayerId>,
        next_stack_id: u32,
        stack: Shared<Vec<StackInstance>>,
        /// Invocations a frame is still resolving after their entry left the
        /// stack (CR 608.2m). Keyed by identity; empty between resolutions.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        detached: BTreeMap<StackId, StackInstance>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        frames: Vec<Frame>,
        frame: Frame,
        pending: Option<PendingInput>,
        next_batch_id: u64,
        next_event_sequence: u64,
    }
    knowledge {
        visibility: Vec<Shared<visibility::Knowledge>>,
    }
    audit {
        external_inputs: History<ExternalInput>,
        journal: History<AcceptedInput>,
    }
}

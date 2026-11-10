//! Runtime ownership is distinct from the stable, flat snapshot contract.
//! Rules, player knowledge and immutable audit entries remain equally bound by
//! recovery hashes. No history is discarded or treated as nonsemantic here.
use super::*;

// Declare each field once. The flat decoder keeps deny_unknown_fields without
// serde's flatten/unknown-field ambiguity, while the encoder borrows groups.
macro_rules! state_layout {
    (rules { $( $(#[$ra:meta])* $r:ident: $rt:ty, )* }
     knowledge { $( $(#[$ka:meta])* $k:ident: $kt:ty, )* }
     audit { $( $(#[$aa:meta])* $a:ident: $at:ty, )* }) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub(super) struct RulesState { $( $(#[$ra])* pub(super) $r: $rt, )* }
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub(super) struct KnowledgeState { $( $(#[$ka])* pub(super) $k: $kt, )* }
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub(super) struct AuditState { $( $(#[$aa])* pub(super) $a: $at, )* }
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub(super) struct State {
            #[serde(flatten)] pub(super) rules: RulesState,
            #[serde(flatten)] pub(super) knowledge: KnowledgeState,
            #[serde(flatten)] pub(super) audit: AuditState,
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
                    rules: RulesState { $( $r, )* },
                    knowledge: KnowledgeState { $( $k, )* },
                    audit: AuditState { $( $a, )* },
                })
            }
        }
    };
}
state_layout! {
    rules {
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        tokens: BTreeMap<CardId, tokens::TokenIdentity>,
        // Deterministic conservative charge for inputs and potential detailed
        // evidence, even in Off/Compact modes. Replay proves the counter.
        #[serde(default, skip_serializing_if = "stored_history::is_zero_charge")]
        retained_auxiliary_charge: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        typed_fault: Option<RuleFault>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_state: Option<MatchState>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<ResolvedEffect>,
        #[serde(default, skip_serializing_if = "triggers::is_zero")]
        next_effect_id: u64,
        #[serde(default, skip_serializing_if = "triggers::is_zero")]
        next_damage_transaction_id: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pending_triggers: Vec<PendingTrigger>,
        #[serde(default, skip_serializing_if = "triggers::is_zero")]
        next_trigger_id: u64,
        libraries: BTreeMap<PlayerId, Vec<CardId>>,
        combat: Vec<Attacker>,
        combat_declared: bool,
        damage_choices: BTreeMap<ObjectId, Vec<(ObjectId, Quantity)>>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        player_damage_choices: BTreeMap<ObjectId, (PlayerId, Quantity)>,
        /// CR510.4/702.7b: incarnation-bound first-step eligibility, retained even
        /// after combat departure. Empty in every historical profile.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        first_strike_participants: BTreeSet<ObjectId>,
        /// CR506.4/509.1h: first/double strikers still blocking after their
        /// attacker left combat. They assign no damage (510.1d) but still start a
        /// first-strike damage step (510.4). Empty in every historical profile.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        orphan_first_strike_blockers: BTreeSet<ObjectId>,
        cleanup_repeat: bool,
        simultaneous_losses: bool,
        losing_players: Vec<PlayerId>,
        revision: u64,
        rule_steps: u64,
        players: Vec<Player>,
        cards: BTreeMap<CardId, Card>,
        objects: BTreeMap<CardId, Object>,
        active: Option<PlayerId>,
        turn_owner: PlayerId,
        turn: TurnState,
        original_seating: Vec<PlayerId>,
        commanders: CommanderLedger,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        commander_returns: BTreeSet<ObjectId>,
        control_effects: Vec<ControlEffect>,
        pending_concessions: Vec<PlayerId>,
        priority: PlayerId,
        passes: Vec<PlayerId>,
        next_stack_id: u32,
        stack: Vec<StackInstance>,
        frame: Frame,
        pending: Option<AcceptedInput>,
        next_batch_id: u64,
        next_event_sequence: u64,
    }
    knowledge {
        visibility: Vec<visibility::Knowledge>,
    }
    audit {
        external_inputs: History<ExternalInput>,
        journal: History<AcceptedInput>,
    }
}

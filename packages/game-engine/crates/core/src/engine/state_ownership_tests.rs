use super::*;

// Frozen flat-State oracle from 364cbdc, before ownership groups and persistent
// audit arrays. Its independent field/default policy detects wire changes in
// the production layout macro; this is not a second runtime representation.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlatStateReference {
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
    /// CR509.1g/506.4/511.3: blocking status is independent of the remaining
    /// attacker links and lasts until this creature leaves combat.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    blocking_creatures: BTreeSet<ObjectId>,
    cleanup_repeat: bool,
    simultaneous_losses: bool,
    losing_players: Vec<PlayerId>,
    revision: u64,
    rule_steps: u64,
    visibility: Vec<visibility::Knowledge>,
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

fn game() -> Game {
    Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap(),
    )
}

fn states() -> Vec<State> {
    let mut game = game();
    let mut states = vec![game.state.clone()];
    game.respond(
        PlayerId(0),
        0,
        Response::ActivateMana {
            object: game.object_id(CardId(0)).unwrap(),
        },
    )
    .unwrap();
    states.push(game.state.clone());
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
    game.concede(PlayerId(3)).unwrap();
    states.push(game.state.clone());
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
    states.push(game.state.clone());
    states
}

#[test]
fn ownership_groups_preserve_the_frozen_flat_wire_contract_and_roundtrip() {
    for state in states() {
        let encoded = canonical(&state).unwrap();
        let reference: FlatStateReference = strict_read(&encoded).unwrap();
        assert_eq!(canonical(&reference).unwrap(), encoded);
        let restored: State = strict_read(&encoded).unwrap();
        assert_eq!(restored, state);
        assert_eq!(canonical(&restored).unwrap(), encoded);
        assert_eq!(
            state_digest::state_digest(&state).unwrap(),
            digest(&reference).unwrap()
        );
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let fields = value.as_object().unwrap();
        for internal_group in ["rules", "knowledge", "audit"] {
            assert!(!fields.contains_key(internal_group));
        }
        assert!(fields["visibility"].is_array());
        assert!(fields["journal"].is_array());
        assert!(fields["external_inputs"].is_array());
    }
}

#[test]
fn flat_state_decoder_rejects_unknown_duplicate_and_missing_group_fields() {
    let state = game().state;
    let encoded = canonical(&state).unwrap();
    for added in [
        "\"rules\":{}",
        "\"knowledge\":{}",
        "\"audit\":{}",
        "\"unexpected\":true",
        "\"revision\":0",
        "\"visibility\":[]",
        "\"journal\":[]",
    ] {
        let malformed = format!("{{{added},{}", &encoded[1..]);
        assert!(
            serde_json::from_str::<State>(&malformed).is_err(),
            "accepted {added}"
        );
        assert!(
            strict_read::<State>(&malformed).is_err(),
            "accepted {added}"
        );
    }
    for required in ["players", "visibility", "journal", "external_inputs"] {
        let mut value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        value.as_object_mut().unwrap().remove(required);
        assert!(
            strict_read::<State>(&canonical(&value).unwrap()).is_err(),
            "missing {required}"
        );
    }
}

#[test]
fn rules_knowledge_and_audit_mutations_are_isolated_between_forks() {
    let mut original = game();
    original.respond(PlayerId(0), 0, Response::Pass).unwrap();
    let baseline = original.state.clone();
    let snapshot = original.snapshot().unwrap();
    let sibling = original.fork();
    let mut branch = original.fork();

    branch.state.rules.players[0]
        .life
        .subtract(&Quantity::from(1));
    assert_ne!(branch.state.rules, baseline.rules);
    assert_eq!(branch.state.knowledge, baseline.knowledge);
    assert_eq!(branch.state.audit, baseline.audit);

    branch.state.knowledge.visibility[0].namespace = Some("isolated-test-view".into());
    assert_ne!(branch.state.knowledge, baseline.knowledge);
    assert_eq!(branch.state.audit, baseline.audit);

    branch.state.audit.journal[0].response = Response::FinishMana { undo_mana: true };
    branch.state.audit.external_inputs.push(ExternalInput {
        after_decisions: 1,
        boundary: 0,
        player: PlayerId(3),
    });
    assert_ne!(branch.state.audit, baseline.audit);
    assert_eq!(original.state, baseline);
    assert_eq!(sibling.state, baseline);
    assert_eq!(original.snapshot().unwrap(), snapshot);
    assert_eq!(sibling.snapshot().unwrap(), snapshot);
}

#[test]
fn audit_forks_share_chunk_and_tail_entries_until_mutated() {
    let mut original = game();
    // Private storage probe crosses two 32-entry chunks and the live tail.
    // These fabricated journal rows are not claimed to be a reachable game.
    for index in 0..65 {
        original.state.audit.journal.push(AcceptedInput {
            decision_id: index,
            player: PlayerId(0),
            response: Response::Pass,
        });
        original.state.audit.external_inputs.push(ExternalInput {
            after_decisions: index,
            boundary: index,
            player: PlayerId(3),
        });
    }
    let baseline = canonical(&original.state.audit).unwrap();
    let mut branch = original.fork();
    for index in [0, 31, 32, 64] {
        assert!(std::ptr::eq(
            &original.state.audit.journal[index],
            &branch.state.audit.journal[index]
        ));
        assert!(std::ptr::eq(
            &original.state.audit.external_inputs[index],
            &branch.state.audit.external_inputs[index]
        ));
    }
    branch.state.audit.journal[31].player = PlayerId(1);
    branch.state.audit.external_inputs[64].player = PlayerId(2);
    assert!(!std::ptr::eq(
        &original.state.audit.journal[31],
        &branch.state.audit.journal[31]
    ));
    assert!(!std::ptr::eq(
        &original.state.audit.external_inputs[64],
        &branch.state.audit.external_inputs[64]
    ));
    for index in [0, 32] {
        assert!(std::ptr::eq(
            &original.state.audit.journal[index],
            &branch.state.audit.journal[index]
        ));
        assert!(std::ptr::eq(
            &original.state.audit.external_inputs[index],
            &branch.state.audit.external_inputs[index]
        ));
    }
    assert_eq!(canonical(&original.state.audit).unwrap(), baseline);
    assert_eq!(
        canonical(&original.state.audit.journal).unwrap(),
        canonical(&original.state.audit.journal.as_slice()).unwrap()
    );
    assert_eq!(
        canonical(&original.state.audit.external_inputs).unwrap(),
        canonical(&original.state.audit.external_inputs.as_slice()).unwrap()
    );
}

#[test]
fn public_journal_reads_borrow_shared_entries_without_flattening() {
    let mut game = game();
    game.respond(PlayerId(0), 0, Response::Pass).unwrap();
    let branch = game.fork();
    assert_eq!(branch.journal().len(), 1);
    assert!(std::ptr::eq(
        branch.journal().last().unwrap(),
        &game.state.audit.journal[0]
    ));
    assert!(std::ptr::eq(
        branch.journal().iter().next().unwrap(),
        &game.state.audit.journal[0]
    ));
}

//! Synthetic interaction diagnostic; every rule operation is an existing core
//! path. This is NOT admission of the enlarged table or new card semantics.
//! CR 603.3b-d/603.10 (APNAP/lookback), 615.7/616.1 (allocation/order),
//! 601.2c/608.2b (target occurrences), 508-510/704.5 (combat/SBAs):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::{hint::black_box, time::Instant};

const INTERACTION_CONTRACT: &str = r#"{
 "schema":"synthetic-dense-interactions-v2",
 "supersedes":"synthetic-dense-diagnostic-v1 interaction coverage gap only; prior kernel proof preserved",
 "public_admission":"all enlarged configurations rejected",
 "battlefield_rows":256,"other_zone_cards":400,"commanders":0,"row_count_scope":"initial baseline; ETB adds4BF/removes4hand, physical656 unchanged",
 "warmup_repetitions":1,"measured_repetitions":5,
 "trigger_rows":{"artists":16,"simultaneous_deaths":[4,8],"expected_pending":[64,128],"overflow_deaths":9,"overflow_expected":144,"actual_stack_limit":128,"placement_resolution_deaths":4,"order":"APNAP 0,1,2,3; reverse source-order trigger IDs inside each controller; self player targets; LIFO resolution","etb_wardens":16,"simultaneous_visionaries":4,"expected_etb_triggers":68},
 "damage_rows":{"sources":12,"recipients":[1,2,3],"occurrences":12,"original_per_portion":2,"furnaces":2,"shields":3,"shield_budget_each":3,"orders":["double-double-shield","shield-double-double","fog-first","shield-first-fog"],"expected_damage_per_player":[29,20,0,0],"actual_damage_limit":24,"overflow_occurrences":25,"prefix_provider_departure":"4portions P1 doubled before provider leaves; second provider and shield still leave29actual damage; exactly8doubling applications"},
 "target_rows":{"programs":["Seeds of Strength","Cruel Edict"],"seeds_slots":3,"controllers":[1,2,3],"variants":["distinct","repeated","middle_target_illegal"],"edict_separate_target_players":[1,2,3],"oracle":"archived full programs, per-slot +1/+1 and chosen owner graveyard; exact chooser/candidate sets"},
 "combat_ownership":"successful legacy-scenario row concentrates all Forest filler/offzone ownership in P0 (650/2/2/2); balanced656-card departure is paired unsupported16-leaf rollback; neither is legal Commander admission",
 "combat_row":{"attackers":9,"defenders":[1,2,3],"blockers_each":2,"unblocked_each":1,"actual_damage_portions":15,"lethal_creatures":12,"actual_event_group_limit":16,"overflow_group":17,"simultaneous_player_losses":[1,2,3],"winner":0,"starting_life":[6,2,2,2]},
 "timing":"only production advance/respond/commit/query segments; all expected arithmetic, canonical SHA, rollback, restore and replay outside stopwatches",
 "replay":"ordinary accepted-input rows use normal restore/replay; injected typed fixture groups use explicit same-operation replay and generic full State bytes, never claim journal reachability",
 "oracle_memory":"generic skeleton retains every dynamic State/Knowledge field; each complete history View canonicalized generically one atatime to private generic file; production bytes compared/hash-counted through bounded reader with exact EOF, no actual.json materialization; retained original/replay generic files compared byteforbyte; small whole-generic exactbyte guard; phaseRSS/HWM not core-only peaks",
 "outer_guard":"120s-2gib-v1 required and independently enforced; env declaration is not proof",
 "new_mechanics_or_capacity_changes":false,"acceptance_latency_claim":false
}"#;

#[derive(Default, Serialize)]
struct Work {
    advance_calls: u64,
    accepted_inputs: u64,
    committed_rule_steps: u64,
    explicit_batches_attempted: u64,
    explicit_occurrences_attempted: u64,
    explicit_batches_committed: u64,
    explicit_occurrences_committed: u64,
    initial_queries: u64,
    query_resumes: u64,
    samples: Vec<OpSample>,
}
#[derive(Serialize)]
struct OpSample {
    operation: &'static str,
    nanos: u128,
}
impl Work {
    fn op<T>(&mut self, timed: bool, operation: &'static str, f: impl FnOnce() -> T) -> T {
        if !timed {
            return f();
        }
        let start = Instant::now();
        let value = black_box(f());
        self.samples.push(OpSample {
            operation,
            nanos: start.elapsed().as_nanos(),
        });
        value
    }
}
#[derive(Clone)]
struct Table {
    cards: Vec<CardSetup>,
}
impl Table {
    fn new() -> Self {
        Self { cards: vec![] }
    }
    fn add(&mut self, oracle: &str, owner: u8, zone: Zone) -> CardId {
        let id = CardId(u32::try_from(self.cards.len()).unwrap());
        self.cards.push(CardSetup {
            id,
            oracle_id: oracle.into(),
            owner: PlayerId(owner),
            controller: PlayerId(owner),
            zone,
        });
        id
    }
    fn bf(&mut self, oracle: &str, owner: u8) -> CardId {
        self.add(oracle, owner, Zone::Battlefield)
    }
    fn fill(mut self, creatures: bool) -> Vec<CardSetup> {
        while self
            .cards
            .iter()
            .filter(|c| c.zone == Zone::Battlefield)
            .count()
            < 256
        {
            let owner = 1 + (self.cards.len() % 3) as u8;
            self.bf(if creatures { BEARS } else { FOREST }, owner);
        }
        // Exactly400 nonbattlefield physical cards, including explicit spell hands.
        let explicit = self
            .cards
            .iter()
            .filter(|c| c.zone != Zone::Battlefield)
            .count();
        assert!(explicit <= 28);
        for index in explicit..28 {
            let owner = (index % 4) as u8;
            self.add(
                FOREST,
                owner,
                Zone::Hand {
                    owner: PlayerId(owner),
                },
            );
        }
        for index in 0..372 {
            let owner = (index % 4) as u8;
            self.add(
                FOREST,
                owner,
                Zone::Library {
                    owner: PlayerId(owner),
                },
            );
        }
        assert_eq!(self.cards.len(), 656);
        self.cards
    }
}
fn game(table: Table, creatures: bool, combat: bool) -> Game {
    game_layout(table, creatures, combat, false)
}
fn game_layout(table: Table, creatures: bool, combat: bool, concentrate_fillers: bool) -> Game {
    let catalog = Catalog::read(if combat {
        include_str!("../../../../tests/fixtures/expected-ir.json")
    } else {
        include_str!("../../../../tests/fixtures/expected-choices-ir.json")
    })
    .unwrap();
    let config = if combat {
        ScenarioConfig::combat_interactions()
    } else {
        ScenarioConfig::choices()
    };
    let mut prepared = Prepared::admit(catalog.clone(), config).unwrap();
    prepared.config.cards = table.fill(creatures);
    if concentrate_fillers {
        assert!(combat);
        for card in &mut prepared.config.cards {
            if card.oracle_id == FOREST {
                card.owner = PlayerId(0);
                card.controller = PlayerId(0);
                card.zone = match card.zone {
                    Zone::Hand { .. } => Zone::Hand { owner: PlayerId(0) },
                    Zone::Library { .. } => Zone::Library { owner: PlayerId(0) },
                    other => other,
                };
            }
        }
    }
    prepared.config.commanders.clear();
    assert!(Prepared::admit(catalog, prepared.config.clone()).is_err());
    prepared.profile_digest = digest(&(
        "synthetic-dense-interactions-v2",
        sha256(INTERACTION_CONTRACT.as_bytes()),
        &prepared.config,
    ))
    .unwrap();
    Game::with_trace(prepared, TraceMode::Detailed)
}
fn obj(g: &Game, card: CardId) -> ObjectId {
    g.object_id(card).unwrap()
}
// This generic test oracle bounds temporary memory to one complete View.
// It does not use production entry memos or the typed object-row accelerator.
// Every other State/Knowledge field passes through dynamic generic serde Value.
// A separate test compares its complete output with whole-State canonical().
fn generic_state_to<W: std::io::Write>(state: &State, out: &mut W) {
    use std::io::Write;
    fn json<W: Write, T: Serialize>(out: &mut W, value: &T) {
        serde_json::to_writer(out, value).unwrap();
    }
    let mut skeleton = state.clone();
    for k in &mut skeleton.visibility {
        k.history.clear();
    }
    let value = serde_json::to_value(&skeleton).unwrap();
    drop(skeleton);
    out.write_all(b"{").unwrap();
    let fields = value.as_object().unwrap();
    let mut visibility_seen = false;
    for (index, (key, value)) in fields.iter().enumerate() {
        if index != 0 {
            out.write_all(b",").unwrap();
        }
        json(out, key);
        out.write_all(b":").unwrap();
        if key != "visibility" {
            json(out, value);
            continue;
        }
        visibility_seen = true;
        let rows = value.as_array().unwrap();
        assert_eq!(rows.len(), state.visibility.len());
        out.write_all(b"[").unwrap();
        for (index, (row, knowledge)) in rows.iter().zip(&state.visibility).enumerate() {
            if index != 0 {
                out.write_all(b",").unwrap();
            }
            out.write_all(b"{").unwrap();
            let mut history_seen = false;
            for (index, (key, value)) in row.as_object().unwrap().iter().enumerate() {
                if index != 0 {
                    out.write_all(b",").unwrap();
                }
                json(out, key);
                out.write_all(b":").unwrap();
                if key != "history" {
                    json(out, value);
                    continue;
                }
                history_seen = true;
                assert_eq!(value, &serde_json::json!([]));
                out.write_all(b"[").unwrap();
                for (index, view) in knowledge.history.iter().enumerate() {
                    if index != 0 {
                        out.write_all(b",").unwrap();
                    }
                    let generic = serde_json::to_value(view).unwrap();
                    json(out, &generic);
                }
                out.write_all(b"]").unwrap();
            }
            assert!(history_seen);
            out.write_all(b"}").unwrap();
        }
        out.write_all(b"]").unwrap();
    }
    assert!(visibility_seen);
    out.write_all(b"}").unwrap();
}
fn proof_memory(phase: &str, g: &Game) {
    let raw = std::fs::read_to_string("/proc/self/status").unwrap();
    let fields = raw
        .lines()
        .filter(|l| l.starts_with("VmRSS:") || l.starts_with("VmHWM:"))
        .collect::<Vec<_>>();
    println!("DENSE_INTERACTION_MEMORY={}",canonical(&serde_json::json!({"phase":phase,"kernel_counters":fields,"history_lengths":g.state.visibility.iter().map(|k|k.history.len()).collect::<Vec<_>>(),"note":"phase snapshot/cumulative HWM, not a core-only peak or allocation count"})).unwrap());
}
fn state_oracle_raw(state: &State) -> String {
    state_oracle_raw_keep(state, None)
}
/// Test-oracle-only sink. Every production-emitted byte must match the generic
/// reader, with bounded scratch storage, exact length/EOF, and a complete SHA.
/// An I/O/mismatch error permanently poisons the sink; no digest is returned.
struct CanonicalByteComparator<R> {
    expected: R,
    expected_len: u64,
    count: u64,
    hash: Sha256Sink,
    failed: bool,
    scratch: [u8; 65536],
}
impl<R: std::io::Read> CanonicalByteComparator<R> {
    fn new(expected: R, expected_len: u64) -> Self {
        Self {
            expected,
            expected_len,
            count: 0,
            hash: Sha256Sink::new(),
            failed: false,
            scratch: [0; 65536],
        }
    }
    fn fail(&mut self, kind: std::io::ErrorKind, message: &'static str) -> std::io::Error {
        self.failed = true;
        std::io::Error::new(kind, message)
    }
    fn finish(mut self) -> std::io::Result<(String, u64)> {
        std::io::Write::flush(&mut self)?;
        if self.count != self.expected_len {
            return Err(self.fail(
                std::io::ErrorKind::UnexpectedEof,
                "production stream truncated",
            ));
        }
        let mut extra = [0u8; 1];
        match self.expected.read(&mut extra) {
            Ok(0) => Ok((self.hash.finalize_hex(), self.count)),
            Ok(_) => Err(self.fail(std::io::ErrorKind::InvalidData, "generic stream appended")),
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
}
impl<R: std::io::Read> std::io::Write for CanonicalByteComparator<R> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.failed {
            return Err(self.fail(
                std::io::ErrorKind::InvalidData,
                "canonical comparator poisoned",
            ));
        }
        let Ok(length) = u64::try_from(bytes.len()) else {
            return Err(self.fail(
                std::io::ErrorKind::InvalidData,
                "canonical byte count overflow",
            ));
        };
        let Some(next) = self.count.checked_add(length) else {
            return Err(self.fail(
                std::io::ErrorKind::InvalidData,
                "canonical byte count overflow",
            ));
        };
        if next > self.expected_len {
            return Err(self.fail(
                std::io::ErrorKind::InvalidData,
                "production stream appended",
            ));
        }
        for chunk in bytes.chunks(self.scratch.len()) {
            if let Err(error) = self.expected.read_exact(&mut self.scratch[..chunk.len()]) {
                self.failed = true;
                return Err(error);
            }
            if &self.scratch[..chunk.len()] != chunk {
                return Err(self.fail(std::io::ErrorKind::InvalidData, "canonical byte mismatch"));
            }
            self.hash.update(chunk);
        }
        self.count = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        // This sink owns no output buffer/file: each write synchronously compares
        // and hashes its bytes. The generic file was flushed before it was opened.
        if self.failed {
            Err(self.fail(
                std::io::ErrorKind::InvalidData,
                "canonical comparator poisoned",
            ))
        } else {
            Ok(())
        }
    }
}

fn state_oracle_raw_keep(state: &State, retain: Option<&std::path::Path>) -> String {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../generated/performance-verification/oracle-work");
    std::fs::create_dir_all(&base).unwrap();
    let directory = base.join(format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let generic = directory.join("generic.json");
    let open = |p: &std::path::Path| {
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(p)
            .unwrap()
    };
    {
        let mut out = std::io::BufWriter::new(open(&generic));
        generic_state_to(state, &mut out);
        out.flush().unwrap();
    }
    let generic_len = std::fs::metadata(&generic).unwrap().len();
    let reader = std::io::BufReader::new(std::fs::File::open(&generic).unwrap());
    let mut compared = CanonicalByteComparator::new(reader, generic_len);
    {
        // Coalesce tiny serializer fragments before comparing/hashing. This is
        // bounded I/O buffering only: the complete byte stream still must match.
        let mut out = std::io::BufWriter::with_capacity(65536, &mut compared);
        state_digest::write_state(state, &mut out).unwrap();
        out.flush().unwrap();
    }
    compared.flush().unwrap();
    let (expected, actual_len) = compared.finish().unwrap();
    assert_eq!(actual_len, generic_len);
    assert_eq!(state_digest::state_digest(state).unwrap(), expected);
    if let Some(path) = retain {
        std::fs::hard_link(&generic, path).unwrap();
    }
    expected
}
struct ProofDirectory(std::path::PathBuf);
impl ProofDirectory {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../generated/performance-verification/oracle-work");
        std::fs::create_dir_all(&base).unwrap();
        let p = base.join(format!(
            "retained-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        Self(p)
    }
}
impl Drop for ProofDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn equal_files(a: &std::path::Path, b: &std::path::Path) {
    use std::io::Read;
    let mut a = std::io::BufReader::new(std::fs::File::open(a).unwrap());
    let mut b = std::io::BufReader::new(std::fs::File::open(b).unwrap());
    let mut aa = [0u8; 65536];
    let mut bb = [0u8; 65536];
    loop {
        let n = a.read(&mut aa).unwrap();
        let mut got = 0;
        while got < n {
            let m = b.read(&mut bb[got..n]).unwrap();
            assert_ne!(m, 0);
            got += m;
        }
        assert_eq!(&aa[..n], &bb[..n]);
        if n == 0 {
            break;
        }
    }
    assert_eq!(b.read(&mut bb).unwrap(), 0);
}
fn state_oracle(g: &Game) -> String {
    state_oracle_raw(&g.state)
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    let replayed = Game::replay(g.prepared.clone(), &g.replay_journal()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(state_oracle(&restored), state_oracle(g));
    assert_eq!(replayed.state, g.state);
    assert_eq!(state_oracle(&replayed), state_oracle(g));
    assert_eq!(restored.trace(), g.trace());
    assert_eq!(replayed.trace(), g.trace());
}
fn advance(g: &mut Game, w: &mut Work, timed: bool, quantum: u64) -> Advance {
    let before = g.rule_boundary();
    w.advance_calls += 1;
    let result = w.op(timed, "advance", || g.advance(Budget::new(quantum)));
    w.committed_rule_steps += g.rule_boundary() - before;
    result
}
fn ready(g: &mut Game, w: &mut Work, timed: bool, quantum: u64) -> Request {
    for _ in 0..100_000 {
        match advance(g, w, timed, quantum) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("interaction unexpected advance {other:?}"),
        }
    }
    panic!("interaction ready cap")
}
fn answer(g: &mut Game, w: &mut Work, timed: bool, r: &Request, response: Response) {
    w.op(timed, "validate_and_accept_input", || {
        g.respond(r.player(), r.decision_id(), response)
    })
    .unwrap();
    w.accepted_inputs += 1;
}
fn query(g: &Game, w: &mut Work, timed: bool, viewer: PlayerId) -> Observation {
    w.initial_queries += 1;
    let mut result = w.op(timed, "observe_initial", || {
        g.observe(viewer, Budget::new(64))
    });
    for _ in 0..100_000 {
        result = match result {
            QueryResult::Complete(QueryValue::Observation(o)) => {
                assert_eq!(o, direct_interaction_projection(g, viewer));
                assert_eq!(o, super::observe(g, viewer, u64::MAX));
                return o;
            }
            QueryResult::Incomplete { work, .. } => {
                w.query_resumes += 1;
                w.op(timed, "observe_resume", || {
                    g.resume_work(work, Budget::new(64))
                })
            }
            _ => panic!("interaction query rejected or wrong result variant"),
        };
    }
    panic!("interaction query cap")
}
fn direct_interaction_projection(g: &Game, viewer: PlayerId) -> Observation {
    let mut view = g.blank_view(viewer).unwrap();
    view.players = (0..4)
        .map(|p| g.visible_player(PlayerId(p)).unwrap())
        .collect();
    view.objects = g
        .state
        .objects
        .values()
        .filter_map(|o| g.visible_object(viewer, o).unwrap())
        .collect();
    view.objects.sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
    view.stack = (0..g.state.stack.len())
        .map(|i| g.visible_stack(viewer, i).unwrap())
        .collect();
    view.effects = g.visible_effects(viewer).unwrap();
    let mut decision = g.request_header(viewer).unwrap();
    match &mut decision {
        Some(PlayerRequest::Targets { candidates, .. }) => {
            *candidates = g
                .state
                .objects
                .values()
                .filter(|o| g.target_legal(o.id))
                .map(|o| g.handle_for(viewer, o.id).unwrap())
                .collect()
        }
        Some(PlayerRequest::EntityTargets { candidates, .. }) => {
            candidates.extend(
                g.live_players()
                    .into_iter()
                    .map(|player| PlayerTargetEntity::Player { player }),
            );
            candidates.extend(
                g.state
                    .objects
                    .values()
                    .filter(|o| g.target_legal(o.id))
                    .map(|o| PlayerTargetEntity::Object {
                        object: g.handle_for(viewer, o.id).unwrap(),
                    }),
            );
        }
        Some(PlayerRequest::DamageAllocation { candidates, .. }) => {
            let Some(Request::DamageAllocation {
                candidates: raw, ..
            }) = g.damage_allocation_request().unwrap()
            else {
                panic!("allocation oracle");
            };
            *candidates = raw
                .iter()
                .map(|c| g.visible_allocation_candidate(viewer, c).unwrap())
                .collect();
        }
        Some(PlayerRequest::DamageEffect { candidates, .. }) => {
            let (_, _, raw) = g.damage_effect_group().unwrap().unwrap();
            *candidates = raw
                .iter()
                .map(|c| g.visible_damage_effect_candidate(viewer, c).unwrap())
                .collect();
        }
        Some(PlayerRequest::OrderTriggers { candidates, .. }) => {
            *candidates = g
                .trigger_order_ids()
                .iter()
                .map(|id| g.visible_pending_trigger(viewer, *id).unwrap())
                .collect()
        }
        Some(PlayerRequest::Modes { candidates, .. }) => {
            let Some(Request::Modes {
                candidates: raw, ..
            }) = g.modes_request().unwrap()
            else {
                panic!("mode oracle");
            };
            *candidates = raw;
        }
        _ => (),
    }
    Observation {
        observation_version: 16,
        view,
        decision,
    }
}
fn batch(
    g: &mut Game,
    w: &mut Work,
    timed: bool,
    proposed: EventGroup,
    count: usize,
) -> Result<CommittedBatch, String> {
    w.explicit_batches_attempted += 1;
    w.explicit_occurrences_attempted += u64::try_from(count).unwrap();
    let result = w.op(timed, "atomic_event_group", || {
        g.commit_batch(BatchCause::Fixture, proposed)
    });
    if let Ok(receipt) = &result {
        w.explicit_batches_committed += 1;
        w.explicit_occurrences_committed += u64::try_from(receipt.occurrences.len()).unwrap();
    }
    result
}
fn leaf(proposed: ProposedEvent) -> EventGroup {
    EventGroup::Occurrence { proposed }
}
fn unchanged(g: &Game, state: &State, trace: &TraceEvidence) {
    assert_eq!(&g.state, state);
    assert_eq!(state_oracle(g), state_oracle_raw(state));
    assert_eq!(g.trace(), trace);
}
fn outcome_counts(g: &Game) -> serde_json::Value {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in g.events() {
        let key = match e {
            Event::TriggerCollected { .. } => "trigger_collected",
            Event::TriggerPlaced { .. } => "trigger_placed",
            Event::TriggeredAbilityCompleted { .. } => "trigger_completed",
            Event::SpellCompleted { .. } => "spell_completed",
            Event::Committed { actual, .. } => match actual {
                EventOutcome::LifeGained { .. } => "life_gained",
                EventOutcome::LifeLost { .. } => "life_lost",
                EventOutcome::EffectCreated { .. } => "effect_created",
                EventOutcome::DamageEffectApplied { .. } => "damage_effect_applied",
                EventOutcome::DamageEffectAllocated { .. } => "damage_effect_allocated",
                EventOutcome::DamageCommitted { .. } => "damage_committed",
                EventOutcome::AttackersDeclared { .. } => "attackers_declared",
                EventOutcome::BlockersDeclared { .. } => "blockers_declared",
                EventOutcome::DamageDealt { .. } => "combat_damage",
                EventOutcome::StateBasedMoved { .. } => "sba_creature",
                EventOutcome::Sacrificed { .. } => "sacrificed",
                _ => "other_committed",
            },
            _ => "other_event",
        };
        *counts.entry(key.into()).or_default() += 1;
    }
    serde_json::to_value(counts).unwrap()
}

fn death_group(g: &Game, ids: &[CardId]) -> EventGroup {
    EventGroup::Simultaneous {
        occurrences: ids
            .iter()
            .map(|card| ProposedEvent::Move {
                object: obj(g, *card),
                destination: Zone::Graveyard {
                    owner: g.state.cards[card].owner,
                },
            })
            .collect(),
    }
}
fn trigger_row(timed: bool, quantum: u64) -> serde_json::Value {
    let mut t = Table::new();
    let artists = (0..16)
        .map(|i| t.bf(BLOOD_ARTIST, (i % 4) as u8))
        .collect::<Vec<_>>();
    let initial = game(t, false, false);
    let mut w = Work::default();
    let mut collection = vec![];
    for deaths in [4usize, 8, 9] {
        let mut g = initial.fork();
        let before = g.state.clone();
        let trace = g.trace().clone();
        let group = death_group(&g, &artists[..deaths]);
        let receipt = batch(&mut g, &mut w, timed, group, deaths);
        if deaths == 9 {
            assert_eq!(receipt.unwrap_err(), "trigger/stack resource limit");
            unchanged(&g, &before, &trace);
        } else {
            let receipt = receipt.unwrap();
            assert_eq!(receipt.occurrences.len(), deaths);
            assert_eq!(g.state.pending_triggers.len(), 16 * deaths);
            let pairs = g
                .events()
                .iter()
                .filter_map(|e| {
                    if let Event::TriggerCollected {
                        source,
                        subject,
                        source_facts,
                        ..
                    } = e
                    {
                        assert_eq!(source_facts.object, *source);
                        assert_eq!(source_facts.zone, Zone::Battlefield);
                        Some((source.card, subject.card))
                    } else {
                        None
                    }
                })
                .collect::<BTreeSet<_>>();
            let expected = artists
                .iter()
                .flat_map(|a| artists[..deaths].iter().map(move |s| (*a, *s)))
                .collect::<BTreeSet<_>>();
            assert_eq!(pairs, expected);
            assert_eq!(pairs.len(), 16 * deaths);
            g.validate_trigger_state().unwrap();
            state_oracle(&g);
        }
        collection.push(
            serde_json::json!({"deaths":deaths,"expected":16*deaths,"overflow_rejected":deaths==9}),
        );
    }
    let mut g = initial.fork();
    let group = death_group(&g, &artists[..4]);
    batch(&mut g, &mut w, timed, group, 4).unwrap();
    let expected_order = (0..4)
        .flat_map(|p| {
            g.state
                .pending_triggers
                .iter()
                .filter(move |t| t.controller == PlayerId(p))
                .map(|t| t.id)
                .rev()
        })
        .collect::<Vec<_>>();
    assert!(
        g.begin_trigger_placement(Frame::Priority, false, false)
            .unwrap()
    );
    loop {
        let r = ready(&mut g, &mut w, timed, quantum);
        let response = match &r {
            Request::OrderTriggers { candidates, .. } => {
                let mut order = candidates.clone();
                order.reverse();
                Response::OrderTriggers { order }
            }
            Request::PlayerTargets {
                player, candidates, ..
            } => {
                assert!(candidates.contains(player));
                query(&g, &mut w, timed, *player);
                Response::PlayerTargets {
                    players: vec![*player],
                }
            }
            Request::Priority { .. } => break,
            other => panic!("trigger placement {other:?}"),
        };
        answer(&mut g, &mut w, timed, &r, response);
    }
    proof_memory("after_all64_placements_before_oracles", &g);
    let placed = g
        .events()
        .iter()
        .filter_map(|e| {
            if let Event::TriggerPlaced { trigger_id, .. } = e {
                Some(*trigger_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(placed, expected_order);
    assert_eq!(g.state.stack.len(), 64);
    let expected_lifo = g.state.stack.iter().rev().map(|s| s.id).collect::<Vec<_>>();
    for _ in 0..1000 {
        if g.state.stack.is_empty() {
            break;
        }
        let r = ready(&mut g, &mut w, timed, quantum);
        if g.state.stack.is_empty() {
            break;
        }
        assert!(matches!(r, Request::Priority { .. }));
        answer(&mut g, &mut w, timed, &r, Response::Pass);
    }
    let _ = ready(&mut g, &mut w, timed, quantum);
    assert!(g.state.stack.is_empty());
    let completed = g
        .events()
        .iter()
        .filter_map(|e| {
            if let Event::TriggeredAbilityCompleted { stack_id, .. } = e {
                Some(*stack_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(completed, expected_lifo);
    assert_eq!(w.accepted_inputs, 4 + 64 + 4 * 64);
    let counts = outcome_counts(&g);
    assert_eq!(counts["life_gained"], 64);
    assert_eq!(counts["life_lost"], 64);
    assert!(g.state.players.iter().all(|p| p.life == 40.into()));
    // Same injected typed occurrence and accepted journal replay, not a claim
    // that the fixture death itself was a legal player command.
    proof_memory("after_all64_resolutions_before_oracle", &g);
    let proof_directory = ProofDirectory::new();
    let original_file = proof_directory.0.join("original.json");
    let replay_file = proof_directory.0.join("replay.json");
    let final_bytes = state_oracle_raw_keep(&g.state, Some(&original_file));
    proof_memory("after_all64_resolution_oracle", &g);
    let journal = g.replay_journal();
    let expected_trace = g.trace().clone();
    drop(g);
    let mut replay = initial.fork();
    let group = death_group(&replay, &artists[..4]);
    replay.commit_batch(BatchCause::Fixture, group).unwrap();
    replay
        .begin_trigger_placement(Frame::Priority, false, false)
        .unwrap();
    let mut rw = Work::default();
    for d in &journal.decisions {
        let r = ready(&mut replay, &mut rw, false, 1);
        assert_eq!((r.player(), r.decision_id()), (d.player, d.decision_id));
        replay
            .respond(d.player, d.decision_id, d.response.clone())
            .unwrap();
    }
    let _ = ready(&mut replay, &mut rw, false, 1);
    assert_eq!(
        state_oracle_raw_keep(&replay.state, Some(&replay_file)),
        final_bytes
    );
    equal_files(&original_file, &replay_file);
    assert_eq!(replay.trace(), &expected_trace);
    proof_memory("after_exact_replay_and_disk_byte_comparison", &replay);
    drop(replay);
    drop(expected_trace);
    drop(proof_directory);
    // ETB uses resulting view, including four arriving self-draw creatures;
    // every existing Warden sees each entering creature, never itself.
    let mut t = Table::new();
    let wardens = (0..16)
        .map(|i| t.bf(WARDEN, (i % 4) as u8))
        .collect::<Vec<_>>();
    let arrivals = (0..4)
        .map(|p| t.add(VISIONARY, p, Zone::Hand { owner: PlayerId(p) }))
        .collect::<Vec<_>>();
    let mut etb = game(t, false, false);
    let enters = EventGroup::Simultaneous {
        occurrences: arrivals
            .iter()
            .map(|c| ProposedEvent::Move {
                object: obj(&etb, *c),
                destination: Zone::Battlefield,
            })
            .collect(),
    };
    batch(&mut etb, &mut w, timed, enters, 4).unwrap();
    assert_eq!(etb.state.pending_triggers.len(), 68);
    let etb_pairs = etb
        .events()
        .iter()
        .filter_map(|e| {
            if let Event::TriggerCollected {
                source,
                subject,
                source_facts,
                ..
            } = e
            {
                assert_eq!(subject.incarnation, 1);
                assert_eq!(source_facts.zone, Zone::Battlefield);
                Some((source.card, subject.card))
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    let expected_etb = wardens
        .iter()
        .flat_map(|w| arrivals.iter().map(move |a| (*w, *a)))
        .chain(arrivals.iter().map(|a| (*a, *a)))
        .collect::<BTreeSet<_>>();
    assert_eq!(etb_pairs, expected_etb);
    assert_eq!(etb_pairs.len(), 68);
    etb.validate_trigger_state().unwrap();
    state_oracle(&etb);
    serde_json::json!({"row":"triggers","collection":collection,"etb_pending":68,"placed":64,"completed":64,"counts":counts,"state_sha256":final_bytes,"work":w,"ordinary_journal_restore_claim":false,"typed_fixture_operation_replay_verified":true})
}

struct CastPlan<'a> {
    card: CardId,
    mode: Option<u32>,
    targets: Response,
    lands: &'a [CardId],
    spending: BTreeMap<Color, Quantity>,
}
fn cast(g: &mut Game, w: &mut Work, timed: bool, quantum: u64, plan: CastPlan<'_>) {
    let CastPlan {
        card,
        mode,
        targets,
        lands,
        spending,
    } = plan;
    let r = ready(g, w, timed, quantum);
    assert!(matches!(
        r,
        Request::Priority {
            player: PlayerId(0),
            ..
        }
    ));
    answer(g, w, timed, &r, Response::Cast { card });
    if let Some(mode) = mode {
        let r = ready(g, w, timed, quantum);
        assert!(matches!(r, Request::Modes { .. }));
        answer(g, w, timed, &r, Response::Modes { mode });
    }
    let r = ready(g, w, timed, quantum);
    assert!(matches!(
        r,
        Request::Targets { .. } | Request::EntityTargets { .. } | Request::PlayerTargets { .. }
    ));
    if g.definition(card).unwrap().oracle_id == EDICT {
        let Request::PlayerTargets { candidates, .. } = &r else {
            panic!("Edict opponent domain");
        };
        assert_eq!(candidates, &vec![PlayerId(1), PlayerId(2), PlayerId(3)]);
    }
    query(g, w, timed, r.player());
    answer(g, w, timed, &r, targets);
    for card in lands {
        let r = ready(g, w, timed, quantum);
        assert!(matches!(r, Request::Mana { .. }));
        let object = obj(g, *card);
        answer(g, w, timed, &r, Response::ActivateMana { object });
    }
    let r = ready(g, w, timed, quantum);
    assert!(matches!(r, Request::Mana { .. }));
    answer(g, w, timed, &r, Response::FinishMana { undo_mana: false });
    let r = ready(g, w, timed, quantum);
    assert!(matches!(r, Request::SpendMana { .. }));
    answer(g, w, timed, &r, Response::SpendMana { spending });
    let r = ready(g, w, timed, quantum);
    assert!(matches!(r, Request::Priority { .. }));
    assert!(g.state.stack.last().unwrap().paid.is_some());
}
fn resolve(
    g: &mut Game,
    w: &mut Work,
    timed: bool,
    quantum: u64,
    mut sacrifice: impl FnMut(PlayerId, &[ObjectId]) -> ObjectId,
) {
    for _ in 0..1000 {
        let r = ready(g, w, timed, quantum);
        if g.state.stack.is_empty() {
            assert!(matches!(
                r,
                Request::Priority {
                    player: PlayerId(0),
                    ..
                }
            ));
            return;
        }
        let response = match &r {
            Request::Priority { .. } => Response::Pass,
            Request::Sacrifice {
                player, candidates, ..
            } => {
                query(g, w, timed, *player);
                let before = g.state.clone();
                let trace = g.trace().clone();
                assert!(
                    g.respond(
                        PlayerId((player.0 + 1) % 4),
                        r.decision_id(),
                        Response::Sacrifice {
                            object: candidates[0]
                        }
                    )
                    .is_err()
                );
                unchanged(g, &before, &trace);
                Response::Sacrifice {
                    object: sacrifice(*player, candidates),
                }
            }
            other => panic!("resolution choice {other:?}"),
        };
        answer(g, w, timed, &r, response);
    }
    panic!("resolution cap")
}
fn move_out(g: &mut Game, w: &mut Work, timed: bool, card: CardId) -> ObjectId {
    let object = obj(g, card);
    let b = batch(
        g,
        w,
        timed,
        leaf(ProposedEvent::Move {
            object,
            destination: Zone::Exile,
        }),
        1,
    )
    .unwrap();
    assert_eq!(b.occurrences.len(), 1);
    assert_eq!(g.zone(obj(g, card)), Some(Zone::Exile));
    object
}
fn allocate_shield(
    g: &mut Game,
    w: &mut Work,
    timed: bool,
    quantum: u64,
    tx: DamageTransactionId,
    effect: EffectKey,
    allocations: Vec<(DamageId, Quantity)>,
) {
    let mut r = ready(g, w, timed, quantum);
    if let Request::DamageEffect { candidates, .. } = &r {
        let damage = candidates
            .iter()
            .find(|p| p.effect == effect)
            .unwrap()
            .damage;
        answer(
            g,
            w,
            timed,
            &r,
            Response::DamageEffect {
                effect: effect.clone(),
                damage,
            },
        );
        r = ready(g, w, timed, quantum);
    }
    let Request::DamageAllocation {
        player,
        candidates,
        required,
        ..
    } = &r
    else {
        panic!("shield allocation request {r:?}");
    };
    assert_eq!(candidates.len(), 4);
    assert_eq!(*required, 3.into());
    query(g, w, timed, *player);
    let before = g.state.clone();
    let trace = g.trace().clone();
    assert!(
        g.respond(
            *player,
            r.decision_id(),
            Response::DamageAllocation {
                allocations: vec![(allocations[0].0, 3.into())]
            }
        )
        .is_err()
    );
    unchanged(g, &before, &trace);
    answer(g, w, timed, &r, Response::DamageAllocation { allocations });
    let result = advance(g, w, timed, 1);
    assert!(!matches!(
        result,
        Advance::Fault(_) | Advance::Finished { .. } | Advance::Draw
    ));
    assert_eq!(g.damage_transaction().unwrap().id, tx);
    assert!(matches!(g.state.frame, Frame::DamageEffects { .. }));
}
fn damage_row(timed: bool, quantum: u64) -> serde_json::Value {
    let mut t = Table::new();
    let sources = (0..12)
        .map(|i| t.bf(BEARS, (i % 4) as u8))
        .collect::<Vec<_>>();
    let furnaces = [t.bf(FURNACE, 2), t.bf(FURNACE, 3)];
    let plains = (0..3).map(|_| t.bf(PLAINS, 0)).collect::<Vec<_>>();
    let fog_land = t.bf(FOREST, 0);
    let salves = (0..3)
        .map(|_| t.add(choices::SALVE, 0, Zone::Hand { owner: PlayerId(0) }))
        .collect::<Vec<_>>();
    let fog = t.add(FOG, 0, Zone::Hand { owner: PlayerId(0) });
    let mut g = game(t, false, false);
    let mut w = Work::default();
    for (index, card) in salves.iter().enumerate() {
        cast(
            &mut g,
            &mut w,
            timed,
            quantum,
            CastPlan {
                card: *card,
                mode: Some(1),
                targets: Response::EntityTargets {
                    entities: vec![TargetEntity::Player {
                        player: PlayerId(index as u8 + 1),
                    }],
                },
                lands: &[plains[index]],
                spending: [(Color::W, 1.into())].into(),
            },
        );
        resolve(&mut g, &mut w, timed, quantum, |_, _| {
            panic!("no sacrifice")
        });
    }
    assert_eq!(g.state.effects.len(), 3);
    recover(&g);
    let baseline = g.fork();
    let portions = |state: &Game| {
        sources
            .iter()
            .enumerate()
            .map(|(i, s)| DamageOccurrence {
                source: obj(state, *s),
                controller: state.current(obj(state, *s)).unwrap().controller,
                recipient: DamageRecipient::Player {
                    player: PlayerId(1 + (i % 3) as u8),
                },
                amount: 2.into(),
            })
            .collect::<Vec<_>>()
    };
    let mut rows = vec![];
    for shield_first in [false, true] {
        let mut branch = baseline.fork();
        let ds = portions(&branch);
        w.op(timed, "prepare_simultaneous_damage", || {
            branch.prepare_combat_damage(ds)
        })
        .unwrap();
        let tx = branch.damage_transaction().unwrap().id;
        let mut chooser_counts = BTreeMap::<u8, usize>::new();
        for player in 1..4u8 {
            let ids = branch
                .damage_transaction()
                .unwrap()
                .entries
                .iter()
                .filter(|e| {
                    e.recipient
                        == DamageRecipient::Player {
                            player: PlayerId(player),
                        }
                })
                .map(|e| e.id)
                .collect::<Vec<_>>();
            let shield=baseline.state.effects.iter().find(|e|matches!(e.kind,ResolvedEffectKind::PreventNextDamage{target:TargetEntity::Player{player:p},..} if p==PlayerId(player))).unwrap().id;
            let allocation = vec![
                (ids[0], 1.into()),
                (ids[1], 2.into()),
                (ids[2], 0.into()),
                (ids[3], 0.into()),
            ];
            let apply_shield = |branch: &mut Game, w: &mut Work| {
                allocate_shield(
                    branch,
                    w,
                    timed,
                    quantum,
                    tx,
                    EffectKey::Resolved { id: shield },
                    allocation.clone(),
                );
            };
            if shield_first {
                apply_shield(&mut branch, &mut w);
            }
            for card in furnaces {
                for id in &ids {
                    let (chooser, _, _) = branch.damage_effect_group().unwrap().unwrap();
                    assert_eq!(chooser, PlayerId(player));
                    *chooser_counts.entry(player).or_default() += 1;
                    let key = EffectKey::Static {
                        source: obj(&branch, card),
                        ability: 0,
                    };
                    // A shield-first zero portion has no remaining damage to double.
                    if branch
                        .damage_transaction()
                        .unwrap()
                        .entries
                        .iter()
                        .find(|e| e.id == *id)
                        .unwrap()
                        .amount
                        > 0.into()
                    {
                        batch(
                            &mut branch,
                            &mut w,
                            timed,
                            leaf(ProposedEvent::ApplyDamageEffect {
                                transaction: tx,
                                effect: key,
                                damage: *id,
                            }),
                            1,
                        )
                        .unwrap();
                    }
                }
            }
            if !shield_first {
                apply_shield(&mut branch, &mut w);
            }
        }
        assert!(branch.damage_effect_group().unwrap().is_none());
        let receipt = batch(
            &mut branch,
            &mut w,
            timed,
            leaf(ProposedEvent::CommitDamage { transaction: tx }),
            1,
        )
        .unwrap();
        let expected = if shield_first { 20 } else { 29 };
        assert!(
            branch.state.players[1..]
                .iter()
                .all(|p| p.life == Quantity::from(40 - expected))
        );
        let EventOutcome::DamageCommitted {
            actual,
            results,
            commander_damage,
            ..
        } = &receipt.occurrences[0].actual
        else {
            panic!("damage outcome")
        };
        assert_eq!(results.len(), 3);
        assert!(
            results
                .iter()
                .all(|r| r.previous == 40.into() && r.current == Quantity::from(40 - expected))
        );
        assert!(receipt.before.players.iter().all(|p| p.life == 40.into()));
        assert!(commander_damage.is_empty());
        let expected_portions = sources
            .iter()
            .enumerate()
            .filter_map(|(i, card)| {
                let prevented = match i / 3 {
                    0 => 1,
                    1 => 2,
                    _ => 0,
                };
                let amount = if shield_first {
                    (2 - prevented) * 4
                } else {
                    8 - prevented
                };
                (amount > 0).then_some((
                    DamageId(i as u32),
                    *card,
                    PlayerId((i % 4) as u8),
                    DamageRecipient::Player {
                        player: PlayerId(1 + (i % 3) as u8),
                    },
                    Quantity::from(amount),
                ))
            })
            .collect::<Vec<_>>();
        let actual_portions = actual
            .iter()
            .map(|d| {
                (
                    d.id,
                    d.source.object.card,
                    d.controller,
                    d.recipient.clone(),
                    d.amount.clone(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_portions, expected_portions);
        let typed = outcome_counts(&branch);
        assert_eq!(
            typed["damage_effect_applied"],
            if shield_first { 18 } else { 24 }
        );
        assert_eq!(typed["damage_effect_allocated"], 3);
        assert_eq!(typed["damage_committed"], 1);
        assert_eq!(typed["effect_created"], 3);
        let total = actual.iter().fold(Quantity::zero(), |mut sum, d| {
            sum.add(&d.amount);
            sum
        });
        assert_eq!(total, Quantity::from(expected * 3));
        assert!(branch.state.effects.iter().all(|e|matches!(&e.kind,ResolvedEffectKind::PreventNextDamage{remaining,..} if *remaining==0.into())));
        rows.push(serde_json::json!({"shield_first":shield_first,"damage_per_player":expected,"chooser_pair_scans":chooser_counts,"final_portions":actual_portions,"counts":outcome_counts(&branch),"state_sha256":state_oracle(&branch)}));
    }
    // Applied doubling survives the provider leaving; un-applied portions
    // never acquire that departed provider retroactively (CR 616.1f).
    let mut prefix = baseline.fork();
    let ds = portions(&prefix)
        .into_iter()
        .filter(|d| {
            d.recipient
                == DamageRecipient::Player {
                    player: PlayerId(1),
                }
        })
        .collect();
    prefix.prepare_combat_damage(ds).unwrap();
    let tx = prefix.damage_transaction().unwrap().id;
    let first_key = EffectKey::Static {
        source: obj(&prefix, furnaces[0]),
        ability: 0,
    };
    for damage in 0..4 {
        batch(
            &mut prefix,
            &mut w,
            timed,
            leaf(ProposedEvent::ApplyDamageEffect {
                transaction: tx,
                effect: first_key.clone(),
                damage: DamageId(damage),
            }),
            1,
        )
        .unwrap();
    }
    move_out(&mut prefix, &mut w, timed, furnaces[0]);
    let second_key = EffectKey::Static {
        source: obj(&prefix, furnaces[1]),
        ability: 0,
    };
    for damage in 0..4 {
        batch(
            &mut prefix,
            &mut w,
            timed,
            leaf(ProposedEvent::ApplyDamageEffect {
                transaction: tx,
                effect: second_key.clone(),
                damage: DamageId(damage),
            }),
            1,
        )
        .unwrap();
    }
    let shield = prefix
        .state
        .effects
        .iter()
        .find(|e| {
            matches!(
                e.kind,
                ResolvedEffectKind::PreventNextDamage {
                    target: TargetEntity::Player {
                        player: PlayerId(1)
                    },
                    ..
                }
            )
        })
        .unwrap()
        .id;
    allocate_shield(
        &mut prefix,
        &mut w,
        timed,
        quantum,
        tx,
        EffectKey::Resolved { id: shield },
        vec![
            (DamageId(0), 1.into()),
            (DamageId(1), 2.into()),
            (DamageId(2), 0.into()),
            (DamageId(3), 0.into()),
        ],
    );
    batch(
        &mut prefix,
        &mut w,
        timed,
        leaf(ProposedEvent::CommitDamage { transaction: tx }),
        1,
    )
    .unwrap();
    assert_eq!(prefix.state.players[1].life, 11.into());
    assert_eq!(outcome_counts(&prefix)["damage_effect_applied"], 8);
    rows.push(serde_json::json!({"prefix_provider_leaves":true,"damage_player1":29,"counts":outcome_counts(&prefix),"state_sha256":state_oracle(&prefix)}));
    // Global combat prevention preserves future finite budgets only if used
    // before them. Exact complete Fog instruction is retained from the archive.
    let fog_program = g.definition(fog).unwrap().spell.as_ref().unwrap();
    assert!(matches!(
        fog_program.instructions[0],
        Instruction::PreventCombatDamage { .. }
    ));
    let r = ready(&mut g, &mut w, timed, quantum);
    answer(&mut g, &mut w, timed, &r, Response::Cast { card: fog });
    let r = ready(&mut g, &mut w, timed, quantum);
    assert!(matches!(r, Request::Mana { .. }));
    let object = obj(&g, fog_land);
    answer(&mut g, &mut w, timed, &r, Response::ActivateMana { object });
    let r = ready(&mut g, &mut w, timed, quantum);
    answer(
        &mut g,
        &mut w,
        timed,
        &r,
        Response::FinishMana { undo_mana: false },
    );
    let r = ready(&mut g, &mut w, timed, quantum);
    answer(
        &mut g,
        &mut w,
        timed,
        &r,
        Response::SpendMana {
            spending: [(Color::G, 1.into())].into(),
        },
    );
    resolve(&mut g, &mut w, timed, quantum, |_, _| {
        panic!("no sacrifice")
    });
    recover(&g);
    let id = g
        .state
        .effects
        .iter()
        .find(|e| matches!(e.kind, ResolvedEffectKind::PreventCombatDamage))
        .unwrap()
        .id;
    for shield_first in [false, true] {
        let mut b = g.fork();
        let ds = portions(&b);
        b.prepare_combat_damage(ds).unwrap();
        let tx = b.damage_transaction().unwrap().id;
        for p in 1..4u8 {
            let ids = b
                .damage_transaction()
                .unwrap()
                .entries
                .iter()
                .filter(|e| {
                    e.recipient
                        == DamageRecipient::Player {
                            player: PlayerId(p),
                        }
                })
                .map(|e| e.id)
                .collect::<Vec<_>>();
            if shield_first {
                let shield=b.state.effects.iter().find(|e|matches!(e.kind,ResolvedEffectKind::PreventNextDamage{target:TargetEntity::Player{player},..} if player==PlayerId(p))).unwrap().id;
                allocate_shield(
                    &mut b,
                    &mut w,
                    timed,
                    quantum,
                    tx,
                    EffectKey::Resolved { id: shield },
                    ids.iter()
                        .enumerate()
                        .map(|(i, id)| {
                            (
                                *id,
                                if i == 0 {
                                    1.into()
                                } else if i == 1 {
                                    2.into()
                                } else {
                                    0.into()
                                },
                            )
                        })
                        .collect(),
                );
            }
            for damage in ids {
                if b.damage_transaction().unwrap().entries[damage.0 as usize].amount > 0.into() {
                    batch(
                        &mut b,
                        &mut w,
                        timed,
                        leaf(ProposedEvent::ApplyDamageEffect {
                            transaction: tx,
                            effect: EffectKey::Resolved { id },
                            damage,
                        }),
                        1,
                    )
                    .unwrap();
                }
            }
        }
        batch(
            &mut b,
            &mut w,
            timed,
            leaf(ProposedEvent::CommitDamage { transaction: tx }),
            1,
        )
        .unwrap();
        assert!(b.state.players.iter().all(|p| p.life == 40.into()));
        let remaining = b
            .state
            .effects
            .iter()
            .filter_map(|e| {
                if let ResolvedEffectKind::PreventNextDamage { remaining, .. } = &e.kind {
                    Some(remaining.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            remaining,
            vec![if shield_first { 0.into() } else { 3.into() }; 3]
        );
        let counts = outcome_counts(&b);
        assert_eq!(
            counts["damage_effect_applied"],
            if shield_first { 9 } else { 12 }
        );
        assert_eq!(
            counts["damage_effect_allocated"].as_u64().unwrap_or(0),
            if shield_first { 3 } else { 0 }
        );
        assert_eq!(counts["damage_committed"], 1);
        assert_eq!(counts["effect_created"], 4);
        rows.push(serde_json::json!({"fog":true,"shield_first":shield_first,"remaining":remaining,"counts":outcome_counts(&b),"state_sha256":state_oracle(&b)}));
    }
    let mut overflow = baseline.fork();
    let before = overflow.state.clone();
    let trace = overflow.trace().clone();
    let ds = vec![portions(&overflow)[0].clone(); 25];
    assert_eq!(
        overflow.prepare_combat_damage(ds).unwrap_err(),
        "invalid prepared damage group"
    );
    // prepare_combat_damage may allocate already-known handles but no semantic
    // state changes here because all sources were exposed by real spell work.
    unchanged(&overflow, &before, &trace);
    serde_json::json!({"row":"damage","variants":rows,"actual_damage_limit":baseline.damage_limit(),"overflow25_rejected":true,"work":w,"effect_creation":"all three Salves and Fog are actually cast/paid/resolved via archived complete programs; damage vectors alone are typed fixture work"})
}

fn target_row(timed: bool, quantum: u64) -> serde_json::Value {
    let mut t = Table::new();
    let green = t.bf(FOREST, 0);
    let white = t.bf(PLAINS, 0);
    let black = t.bf(SWAMP, 0);
    let generic = t.bf(FOREST, 0);
    let seeds = t.add(choices::SEEDS, 0, Zone::Hand { owner: PlayerId(0) });
    let edict = t.add(EDICT, 0, Zone::Hand { owner: PlayerId(0) });
    let initial = game(t, true, false);
    let targets = (1..4)
        .map(|p| {
            initial
                .state
                .objects
                .values()
                .find(|o| o.controller == PlayerId(p) && initial.target_legal(o.id))
                .unwrap()
                .id
        })
        .collect::<Vec<_>>();
    let seed_program = initial.definition(seeds).unwrap().spell.as_ref().unwrap();
    assert_eq!(seed_program.targets.len(), 3);
    assert_eq!(seed_program.instructions.len(), 3);
    for (slot, op) in seed_program.instructions.iter().enumerate() {
        assert!(
            matches!(op,Instruction::ModifyPowerToughness{target_slot,power,toughness,..}if *target_slot as usize==slot&&*power==1.into()&&*toughness==1.into())
        );
    }
    let mut w = Work::default();
    let mut rows = vec![];
    for variant in ["distinct", "repeated", "middle_target_illegal"] {
        let mut g = initial.fork();
        let chosen = if variant == "repeated" {
            vec![targets[0]; 3]
        } else {
            targets.clone()
        };
        let r = ready(&mut g, &mut w, timed, quantum);
        answer(&mut g, &mut w, timed, &r, Response::Cast { card: seeds });
        let r = ready(&mut g, &mut w, timed, quantum);
        let Request::Targets {
            candidates,
            occurrences,
            player,
            ..
        } = &r
        else {
            panic!("Seeds target request")
        };
        assert_eq!(*player, PlayerId(0));
        assert_eq!(occurrences.len(), 3);
        assert_eq!(candidates.len(), 252);
        assert_eq!(
            candidates.iter().copied().collect::<BTreeSet<_>>(),
            initial
                .state
                .objects
                .values()
                .filter(|o| initial.target_legal(o.id))
                .map(|o| o.id)
                .collect()
        );
        query(&g, &mut w, timed, PlayerId(0));
        let before = g.state.clone();
        let trace = g.trace().clone();
        assert!(
            g.respond(
                PlayerId(0),
                r.decision_id(),
                Response::Targets {
                    objects: targets[..2].to_vec()
                }
            )
            .is_err()
        );
        unchanged(&g, &before, &trace);
        let stale = ObjectId {
            card: targets[0].card,
            incarnation: targets[0].incarnation + 1,
        };
        assert!(
            g.respond(
                PlayerId(0),
                r.decision_id(),
                Response::Targets {
                    objects: vec![stale, targets[1], targets[2]]
                }
            )
            .is_err()
        );
        unchanged(&g, &before, &trace);
        answer(
            &mut g,
            &mut w,
            timed,
            &r,
            Response::Targets {
                objects: chosen.clone(),
            },
        );
        for card in [green, white] {
            let r = ready(&mut g, &mut w, timed, quantum);
            let object = obj(&g, card);
            answer(&mut g, &mut w, timed, &r, Response::ActivateMana { object });
        }
        let r = ready(&mut g, &mut w, timed, quantum);
        answer(
            &mut g,
            &mut w,
            timed,
            &r,
            Response::FinishMana { undo_mana: false },
        );
        let r = ready(&mut g, &mut w, timed, quantum);
        answer(
            &mut g,
            &mut w,
            timed,
            &r,
            Response::SpendMana {
                spending: [(Color::G, 1.into()), (Color::W, 1.into())].into(),
            },
        );
        let _ = ready(&mut g, &mut w, timed, quantum);
        if variant == "middle_target_illegal" {
            move_out(&mut g, &mut w, timed, targets[1].card);
        }
        resolve(&mut g, &mut w, timed, quantum, |_, _| {
            panic!("no sacrifice")
        });
        let expected_effects = if variant == "middle_target_illegal" {
            2
        } else {
            3
        };
        assert_eq!(g.state.effects.len(), expected_effects);
        for (index, target) in targets.iter().enumerate() {
            if variant == "middle_target_illegal" && index == 1 {
                assert!(!g.target_legal(*target));
                continue;
            }
            let expected = if variant == "repeated" {
                if index == 0 { 5 } else { 2 }
            } else {
                3
            };
            assert_eq!(
                g.characteristics(*target).unwrap().power,
                Some(expected.into())
            );
            assert_eq!(
                g.characteristics(*target).unwrap().toughness,
                Some(expected.into())
            );
        }
        let cursors = g
            .state
            .effects
            .iter()
            .map(|e| e.origin.cursor)
            .collect::<Vec<_>>();
        assert_eq!(
            cursors,
            if variant == "middle_target_illegal" {
                vec![0, 2]
            } else {
                vec![0, 1, 2]
            }
        );
        if variant != "middle_target_illegal" {
            recover(&g);
        }
        rows.push(serde_json::json!({"variant":variant,"candidate_count":252,"chosen_controllers":chosen.iter().map(|o|initial.current(*o).unwrap().controller).collect::<Vec<_>>(),"effect_cursors":cursors,"counts":outcome_counts(&g),"state_sha256":state_oracle(&g),"normal_restore_replay_verified":variant!="middle_target_illegal"}));
    }
    for player in 1..4u8 {
        let mut g = initial.fork();
        cast(
            &mut g,
            &mut w,
            timed,
            quantum,
            CastPlan {
                card: edict,
                mode: None,
                targets: Response::PlayerTargets {
                    players: vec![PlayerId(player)],
                },
                lands: &[black, generic],
                spending: [(Color::B, 1.into()), (Color::G, 1.into())].into(),
            },
        );
        let mut chosen = None;
        let mut candidate_count = 0;
        resolve(&mut g, &mut w, timed, quantum, |chooser, candidates| {
            assert_eq!(chooser, PlayerId(player));
            let expected = initial
                .state
                .objects
                .values()
                .filter(|o| o.controller == chooser && initial.target_legal(o.id))
                .map(|o| o.id)
                .collect::<Vec<_>>();
            assert_eq!(candidates, expected);
            candidate_count = candidates.len();
            chosen = Some(*candidates.last().unwrap());
            *candidates.last().unwrap()
        });
        let chosen = chosen.unwrap();
        assert_eq!(
            g.zone(obj(&g, chosen.card)),
            Some(Zone::Graveyard {
                owner: PlayerId(player)
            })
        );
        assert_eq!(outcome_counts(&g)["sacrificed"], 1);
        recover(&g);
        rows.push(serde_json::json!({"variant":"edict","opponent":player,"sacrifice_candidates":candidate_count,"chosen":chosen,"counts":outcome_counts(&g),"state_sha256":state_oracle(&g),"normal_restore_replay_verified":true}));
    }
    serde_json::json!({"row":"targets","variants":rows,"work":w,"announcing_actor":0,"capacity_boundary":"exact3 Seeds occurrences; wrong cardinality and stale incarnation rejected without changes"})
}

fn combat_row(timed: bool, quantum: u64) -> serde_json::Value {
    let mut t = Table::new();
    let attackers = (0..9).map(|_| t.bf(BEARS, 0)).collect::<Vec<_>>();
    let blockers = (1..4)
        .map(|p| (0..2).map(|_| t.bf(BEARS, p)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut balanced = game(t.clone(), false, true);
    let balanced_owned = (0..4)
        .map(|p| {
            balanced
                .state
                .cards
                .values()
                .filter(|c| c.owner == PlayerId(p))
                .count()
        })
        .collect::<Vec<_>>();
    assert!(balanced_owned[1] > 16);
    balanced.concede(PlayerId(1)).unwrap();
    let mut expected = balanced.state.clone();
    let trace = balanced.trace().clone();
    let reason = "empty or oversized simultaneous group".to_string();
    assert_eq!(
        balanced.advance(Budget::new(1)),
        Advance::Fault(reason.clone())
    );
    expected.frame = Frame::Fault { reason };
    unchanged(&balanced, &expected, &trace);
    assert_eq!(
        balanced.live_players(),
        vec![PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)]
    );
    drop(balanced);
    let mut g = game_layout(t, false, true, true);
    let owned_counts = (0..4)
        .map(|p| {
            g.state
                .cards
                .values()
                .filter(|c| c.owner == PlayerId(p))
                .count()
        })
        .collect::<Vec<_>>();
    assert_eq!(owned_counts, vec![650, 2, 2, 2]);
    let mut w = Work::default();
    let attacks = attackers
        .iter()
        .enumerate()
        .map(|(i, c)| (obj(&g, *c), PlayerId(1 + (i / 3) as u8)))
        .collect::<Vec<_>>();
    let expected_dead = attackers
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 3 != 2)
        .map(|(_, c)| *c)
        .chain(blockers.iter().flatten().copied())
        .collect::<BTreeSet<_>>();
    assert_eq!(expected_dead.len(), 12);
    let attack_request = loop {
        let r = ready(&mut g, &mut w, timed, quantum);
        if matches!(r, Request::Attackers { .. }) {
            break r;
        }
        assert!(matches!(r, Request::Priority { .. }));
        answer(&mut g, &mut w, timed, &r, Response::Pass);
    };
    let Request::Attackers {
        candidates,
        defenders,
        ..
    } = &attack_request
    else {
        unreachable!()
    };
    assert_eq!(candidates.len(), 9);
    assert_eq!(defenders, &vec![PlayerId(1), PlayerId(2), PlayerId(3)]);
    query(&g, &mut w, timed, PlayerId(0));
    let before = g.state.clone();
    let trace = g.trace().clone();
    let mut bad = attacks.clone();
    bad.push(attacks[0]);
    assert!(
        g.respond(
            PlayerId(0),
            attack_request.decision_id(),
            Response::Attackers { attacks: bad }
        )
        .is_err()
    );
    unchanged(&g, &before, &trace);
    answer(
        &mut g,
        &mut w,
        timed,
        &attack_request,
        Response::Attackers {
            attacks: attacks.clone(),
        },
    );
    let _ = ready(&mut g, &mut w, timed, quantum);
    assert!(
        attackers
            .iter()
            .all(|c| g.current(obj(&g, *c)).unwrap().tapped)
    );
    let mut block_choosers = vec![];
    let terminal = loop {
        match advance(&mut g, &mut w, timed, quantum) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                let response = match &r {
                    Request::Priority { .. } => Response::Pass,
                    Request::Blockers {
                        player,
                        candidates,
                        attackers: opponents,
                        ..
                    } => {
                        block_choosers.push(*player);
                        assert_eq!(candidates.len(), 2);
                        assert_eq!(opponents.len(), 3);
                        query(&g, &mut w, timed, *player);
                        let blocks = blockers[(player.0 - 1) as usize]
                            .iter()
                            .zip(&opponents[..2])
                            .map(|(b, a)| (obj(&g, *b), *a))
                            .collect::<Vec<_>>();
                        let before = g.state.clone();
                        let trace = g.trace().clone();
                        let mut invalid = blocks.clone();
                        invalid[0].1 = attacks.iter().find(|(_, p)| p != player).unwrap().0;
                        assert!(
                            g.respond(
                                *player,
                                r.decision_id(),
                                Response::Blockers { blocks: invalid }
                            )
                            .is_err()
                        );
                        unchanged(&g, &before, &trace);
                        Response::Blockers { blocks }
                    }
                    other => panic!("combat request {other:?}"),
                };
                answer(&mut g, &mut w, timed, &r, response);
            }
            result @ Advance::Finished { .. } => break result,
            other => panic!("combat advance {other:?}"),
        }
    };
    assert_eq!(
        terminal,
        Advance::Finished {
            winner: PlayerId(0)
        }
    );
    assert_eq!(block_choosers, vec![PlayerId(1), PlayerId(2), PlayerId(3)]);
    let mut actual_edges = vec![];
    let mut actual_dead = BTreeSet::new();
    let mut losses = vec![];
    for event in g.events() {
        match event {
            Event::Committed {
                actual:
                    EventOutcome::DamageDealt {
                        damage,
                        commander_damage,
                        ..
                    },
                ..
            } => {
                assert!(commander_damage.is_empty());
                actual_edges.extend(damage.iter().cloned());
            }
            Event::Committed {
                actual: EventOutcome::StateBasedMoved { movement, reason },
                ..
            } => {
                assert_eq!(*reason, CreatureLossReason::LethalDamage);
                actual_dead.insert(movement.previous.card);
            }
            Event::PlayersLost { losses: wave } => {
                assert!(losses.is_empty());
                losses = wave.clone();
            }
            _ => (),
        }
    }
    let mut expected_edges = vec![];
    for (i, (a, p)) in attacks.iter().enumerate() {
        if i % 3 == 2 {
            expected_edges.push(DamageOccurrence {
                source: *a,
                controller: PlayerId(0),
                recipient: DamageRecipient::Player { player: *p },
                amount: 2.into(),
            });
        } else {
            let b = ObjectId {
                card: blockers[(p.0 - 1) as usize][i % 3],
                incarnation: 0,
            };
            expected_edges.push(DamageOccurrence {
                source: *a,
                controller: PlayerId(0),
                recipient: DamageRecipient::Creature { object: b },
                amount: 2.into(),
            });
            expected_edges.push(DamageOccurrence {
                source: b,
                controller: *p,
                recipient: DamageRecipient::Creature { object: *a },
                amount: 2.into(),
            });
        }
    }
    assert_eq!(
        canonical(&actual_edges).unwrap(),
        canonical(&expected_edges).unwrap()
    );
    assert_eq!(actual_edges.len(), 15);
    assert_eq!(actual_dead, expected_dead);
    assert_eq!(
        losses,
        vec![
            (PlayerId(1), PlayerLossReason::Life),
            (PlayerId(2), PlayerLossReason::Life),
            (PlayerId(3), PlayerLossReason::Life)
        ]
    );
    assert_eq!(g.live_players(), vec![PlayerId(0)]);
    assert_eq!(g.state.players[0].life, 6.into());
    recover(&g);
    // The scenario group's actual16-leaf bound stays intact. Independent
    // individually-valid moves exceed it at17 and leave all State/trace bytes.
    let mut too_many = Game::with_trace(g.prepared.clone(), TraceMode::Detailed);
    let before = too_many.state.clone();
    let trace = too_many.trace().clone();
    let group = EventGroup::Simultaneous {
        occurrences: (0..17)
            .map(|i| ProposedEvent::Move {
                object: obj(&too_many, CardId(i)),
                destination: Zone::Exile,
            })
            .collect(),
    };
    assert!(too_many.commit_batch(BatchCause::Fixture, group).is_err());
    unchanged(&too_many, &before, &trace);
    serde_json::json!({"row":"combat","attackers":9,"blockers":6,"defenders":block_choosers,"damage_portions":15,"sba_creatures":12,"same_wave_player_losses":losses,"winner":0,"counts":outcome_counts(&g),"state_sha256":state_oracle(&g),"normal_restore_replay_verified":true,"event_group17_rejected_atomically":true,"synthetic_owned_counts":owned_counts,"balanced_ownership_unsupported_owned_counts":balanced_owned,"balanced_owned_departure_fault_rollback_verified":true,"balanced_commander_admission_claim":false,"work":w})
}

fn run(name: &str, timed: bool, quantum: u64) -> serde_json::Value {
    match name {
        "triggers" => trigger_row(timed, quantum),
        "damage" => damage_row(timed, quantum),
        "targets" => target_row(timed, quantum),
        "combat" => combat_row(timed, quantum),
        _ => panic!("declared interaction case"),
    }
}
#[test]
fn dense_interaction_contract_is_explicit() {
    let c: serde_json::Value = serde_json::from_str(INTERACTION_CONTRACT).unwrap();
    assert_eq!(c["warmup_repetitions"], 1);
    assert_eq!(c["measured_repetitions"], 5);
    assert_eq!(c["battlefield_rows"], 256);
    assert_eq!(c["other_zone_cards"], 400);
}
fn prove_row(name: &str) {
    let a = run(name, false, 64);
    let b = run(name, false, 1);
    let mut sa = a.clone();
    let mut sb = b.clone();
    sa.as_object_mut().unwrap().remove("work");
    sb.as_object_mut().unwrap().remove("work");
    assert_eq!(sa, sb, "{name} semantic partition equality");
    assert!(a["work"]["samples"].as_array().unwrap().is_empty());
    println!("DENSE_INTERACTION_PROOF={}", canonical(&a).unwrap());
}
#[test]
fn dense_interaction_trigger_paths() {
    prove_row("triggers");
}
#[test]
fn dense_interaction_damage_paths() {
    prove_row("damage");
}
#[test]
fn dense_interaction_target_paths() {
    prove_row("targets");
}
#[test]
fn dense_interaction_combat_paths() {
    prove_row("combat");
}
#[test]
fn dense_interaction_stream_oracle_matches_whole_generic() {
    let mut t = Table::new();
    t.bf(BEARS, 0);
    let mut g = game(t, false, false);
    let mut w = Work::default();
    for _ in 0..5 {
        let r = ready(&mut g, &mut w, false, 1);
        answer(&mut g, &mut w, false, &r, Response::Pass);
    }
    let _ = ready(&mut g, &mut w, false, 1);
    let whole = canonical(&g.state).unwrap();
    let mut actual = vec![];
    generic_state_to(&g.state, &mut actual);
    assert_eq!(actual, whole.as_bytes());
    assert_eq!(state_oracle(&g), sha256(whole.as_bytes()));
}
#[test]
#[ignore = "explicit interaction host diagnostic; requires actual outer watchdog and RSS cap"]
fn dense_interaction_host_diagnostic() {
    assert_eq!(
        std::env::var("DENSE_KERNEL_OUTER_GUARD").unwrap(),
        "120s-2gib-v1"
    );
    let name = std::env::var("DENSE_INTERACTION_CASE").unwrap();
    for repetition in 0..6 {
        println!(
            "DENSE_INTERACTION_START={}",
            canonical(
                &serde_json::json!({"case":name,"repetition":repetition,"warmup":repetition==0})
            )
            .unwrap()
        );
        let row = run(&name, true, 64);
        println!("DENSE_INTERACTION_RESULT={}",canonical(&serde_json::json!({"schema":"synthetic-dense-interaction-result-v2","build_id":BUILD_ID,"contract_sha256":sha256(INTERACTION_CONTRACT.as_bytes()),"contract":serde_json::from_str::<serde_json::Value>(INTERACTION_CONTRACT).unwrap(),"repetition":repetition,"warmup":repetition==0,"result":row,"outer_guard_only_declared_here":true,"acceptance_latency_claim":false})).unwrap());
    }
}

#[test]
fn dense_interaction_comparator_checks_every_byte_and_fragment() {
    use std::io::Write;
    let bytes = (0..200_003)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    for width in [1, 7, 65_535, 65_536, 65_537, bytes.len()] {
        let mut sink = CanonicalByteComparator::new(bytes.as_slice(), bytes.len() as u64);
        assert_eq!(sink.write(&[]).unwrap(), 0);
        for chunk in bytes.chunks(width) {
            sink.write_all(chunk).unwrap();
            sink.flush().unwrap();
        }
        let (hash, count) = sink.finish().unwrap();
        assert_eq!(hash, sha256(&bytes));
        assert_eq!(count, bytes.len() as u64);
    }
    let empty = CanonicalByteComparator::new(&b""[..], 0).finish().unwrap();
    assert_eq!(empty, (sha256(b""), 0));
    let bytes = "{\"value\":\"雪🦀\"}".as_bytes();
    let mut sink = CanonicalByteComparator::new(bytes, bytes.len() as u64);
    for byte in bytes {
        sink.write_all(std::slice::from_ref(byte)).unwrap();
    }
    assert_eq!(sink.finish().unwrap().0, sha256(bytes));
}

#[test]
fn dense_interaction_comparator_handles_short_reads_and_rejects_overflow() {
    use std::io::{Read, Write};
    struct ShortReader<'a>(&'a [u8]);
    impl Read for ShortReader<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            let n = bytes.len().min(3);
            self.0.read(&mut bytes[..n])
        }
    }
    let bytes = b"every-byte-remains-in-the-proof";
    let mut sink = CanonicalByteComparator::new(ShortReader(bytes), bytes.len() as u64);
    sink.write_all(bytes).unwrap();
    assert_eq!(sink.finish().unwrap(), (sha256(bytes), bytes.len() as u64));
    let mut overflow = CanonicalByteComparator::new(&b""[..], u64::MAX);
    overflow.count = u64::MAX;
    assert!(overflow.write_all(b"x").is_err());
    assert!(overflow.flush().is_err());
    assert!(overflow.finish().is_err());
}

#[test]
fn dense_interaction_comparator_rejects_mismatch_truncation_and_append() {
    use std::io::{ErrorKind, Write};
    let mut mismatch = CanonicalByteComparator::new(&b"abc"[..], 3);
    assert!(mismatch.write_all(b"axc").is_err());
    assert!(mismatch.flush().is_err());
    // The default helper skips write for an empty buffer. Probe the poisoned
    // method directly; flush/finish must also keep rejecting the failed proof.
    // https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    mismatch.write_all(b"").unwrap();
    assert!(mismatch.write(b"").is_err());
    assert!(mismatch.finish().is_err());
    let mut truncated = CanonicalByteComparator::new(&b"abc"[..], 3);
    truncated.write_all(b"ab").unwrap();
    assert_eq!(
        truncated.finish().unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    let mut extra_production = CanonicalByteComparator::new(&b"abc"[..], 3);
    assert!(extra_production.write_all(b"abcd").is_err());
    assert!(extra_production.finish().is_err());
    let mut short_generic = CanonicalByteComparator::new(&b"ab"[..], 3);
    assert_eq!(
        short_generic.write_all(b"abc").unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert!(short_generic.finish().is_err());
    let mut extra_generic = CanonicalByteComparator::new(&b"abcd"[..], 3);
    extra_generic.write_all(b"abc").unwrap();
    assert!(extra_generic.finish().is_err());
}

#[test]
fn dense_interaction_comparator_propagates_read_and_production_write_errors() {
    use std::io::{ErrorKind, Read, Write};
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected generic read failure"))
        }
    }
    let mut read_error = CanonicalByteComparator::new(BrokenReader, 1);
    assert_eq!(
        read_error.write_all(b"x").unwrap_err().kind(),
        ErrorKind::Other
    );
    assert!(read_error.flush().is_err());
    assert!(read_error.finish().is_err());
    let finish_error = CanonicalByteComparator::new(BrokenReader, 0);
    assert_eq!(finish_error.finish().unwrap_err().kind(), ErrorKind::Other);
    let spec = FixtureSpec {
        name: "comparator-small-production-error".into(),
        battlefield_objects: 8,
        minimum_history_entries: 8,
    };
    let game = fixture(&spec, TraceMode::Off);
    let mut rejected = CanonicalByteComparator::new(&b""[..], 0);
    assert!(state_digest::write_state(&game.state, &mut rejected).is_err());
    assert!(rejected.flush().is_err());
    assert!(rejected.finish().is_err());
}

//! Trusted host diagnostics, not a strategy or a product search acceptance gate.
//! Ordinary games use actual pod admission and the current IR-driven Strategy.
//! Preparation, policy, queries, transitions, replay and adapters stay separate.
use super::*;
use serde_json::{Value, json};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const PREFIX_INPUTS: usize = 1024;
const REPEATS: usize = 5;
const QUANTUM: u64 = 1024;
const CATALOG_JSON: &str = include_str!("../../../../tests/fixtures/expected-match-ir.json");

fn load_catalog() -> Result<(Catalog, Value), String> {
    let clock = Instant::now();
    let catalog = Catalog::read(CATALOG_JSON)?;
    let decode_ns = clock.elapsed().as_nanos();
    let clock = Instant::now();
    let compiled = mtg_compiler::catalog::embedded_catalog("match").map_err(|e| e.to_string())?;
    let source_compile_verify_ns = clock.elapsed().as_nanos();
    let encoded = canonical(&catalog)?;
    if encoded != canonical(&compiled)? {
        return Err("embedded IR differs from actual source compiler".into());
    }
    let metrics = json!({"scope":"already mapped embedded IR strict decoding once; filesystem/startup excluded; source compiler validation separately outside decoder clock",
        "input_bytes":CATALOG_JSON.len(),"input_sha256":sha256(CATALOG_JSON.as_bytes()),"canonical_bytes":encoded.len(),
        "catalog_sha256":mtg_contracts::digest(&catalog)?,"decode_ns":decode_ns,"source_compile_verify_ns":source_compile_verify_ns});
    Ok((catalog, metrics))
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Semantics {
    inputs: usize,
    rule_steps: u64,
    advances: usize,
    query_calls: usize,
    response_kinds: BTreeMap<String, usize>,
    event_kinds: BTreeMap<String, usize>,
    actual_outcome_kinds: BTreeMap<String, usize>,
    commander_casts: usize,
    noncreature_casts: usize,
    paid_command_casts: usize,
    paid_noncreature_casts: usize,
    survivor_counts: Vec<usize>,
    outcome: String,
    final_state_sha256: String,
    journal_sha256: String,
    trace_sha256: String,
    journal: ReplayJournal,
}

#[derive(Serialize)]
struct Timings {
    admission_ns: u128,
    setup_advance_ns: u128,
    observation_ns: Vec<u128>,
    policy_ns: Vec<u128>,
    check_submit_advance_ns: Vec<u128>,
    response_order: Vec<String>,
    replay_verify_ns: u128,
    dispose_ns: u128,
}

struct Run {
    semantics: Semantics,
    timings: Timings,
}

fn next(
    game: &mut Game,
    quantum: u64,
    work: &mut usize,
    calls: &mut usize,
) -> Result<Advance, String> {
    loop {
        if *work >= WORK_LIMIT {
            return Err("ordinary diagnostic automatic work limit".into());
        }
        *work += 1;
        *calls += 1;
        match game.advance(Budget::new(quantum)) {
            Advance::Suspended { .. } => {}
            other => return Ok(other),
        }
    }
}

fn count_tag(
    value: &impl Serialize,
    tag: &str,
    counts: &mut BTreeMap<String, usize>,
) -> Result<(), String> {
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    let key = value
        .get(tag)
        .and_then(Value::as_str)
        .ok_or("diagnostic enum tag missing")?;
    *counts.entry(key.into()).or_default() += 1;
    Ok(())
}

/// A complete generic State proof, kept on disk only until its byte oracle has
/// finished. Each proof owns one private directory; errors also remove it.
/// These trusted-host tests perform filesystem I/O outside every timed core
/// transition and outside the separately attributed raw-core profile region.
struct GenericStateProof {
    directory: std::path::PathBuf,
    path: std::path::PathBuf,
    sha256: String,
    bytes: u64,
}

impl Drop for GenericStateProof {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

impl GenericStateProof {
    fn remove(self) -> Result<(), String> {
        // A successful oracle must release its proof files, not merely hope
        // that a Drop cleanup succeeds. Drop remains the error-path fallback.
        fs::remove_file(&self.path).map_err(|e| e.to_string())?;
        fs::remove_dir(&self.directory).map_err(|e| e.to_string())
    }

    fn create() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir();
        if !root.is_absolute()
            || root
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("generic proof needs an absolute temporary parent".into());
        }
        for ancestor in root.ancestors() {
            let metadata = fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err("generic proof temporary ancestry must be plain directories".into());
            }
        }
        // No random or ambient hidden game data is used in proof file names.
        // Atomic create prevents collisions; this is trusted local host I/O,
        // not an adversarial same-UID filesystem isolation mechanism.
        for _ in 0..32 {
            let ordinal = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = root.join(format!(
                "mtg-ordinary-generic-proof-{}-{ordinal}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    return Ok(Self {
                        path: directory.join("state.json"),
                        directory,
                        sha256: String::new(),
                        bytes: 0,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("generic proof temporary name collision limit".into())
    }
}

struct HashingProofWriter<'a, W: Write> {
    writer: &'a mut W,
    hash: mtg_contracts::Sha256Sink,
    bytes: u64,
}

impl<W: Write> Write for HashingProofWriter<'_, W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let written = self.writer.write(data)?;
        self.bytes = self
            .bytes
            .checked_add(written as u64)
            .ok_or_else(|| std::io::Error::other("generic proof byte count overflow"))?;
        self.hash.update(&data[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

fn write_generic_state_value(
    state: &Value,
    writer: &mut impl Write,
) -> Result<(String, u64), String> {
    if !state.is_object() {
        return Err("generic snapshot State must be an object".into());
    }
    let mut hashed = HashingProofWriter {
        writer,
        hash: mtg_contracts::Sha256Sink::new(),
        bytes: 0,
    };
    {
        let mut buffered = std::io::BufWriter::with_capacity(64 * 1024, &mut hashed);
        // The pinned serde_json default Map is sorted recursively. Serializing
        // the already parsed generic Value directly therefore emits precisely
        // contracts::canonical(Value)'s bytes, without its extra to_value clone.
        // The small canonical oracle below protects this format assumption.
        serde_json::to_writer(&mut buffered, state).map_err(|e| e.to_string())?;
        buffered.flush().map_err(|e| e.to_string())?;
    }
    Ok((hashed.hash.finalize_hex(), hashed.bytes))
}

mod generic_state_spool;

fn generic_state_proof(game: &Game) -> Result<GenericStateProof, String> {
    generic_state_spool::prove(game)
}

fn read_proof_chunk(reader: &mut impl std::io::Read, bytes: &mut [u8]) -> Result<usize, String> {
    let mut used = 0;
    while used < bytes.len() {
        match reader.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(count) => used += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(used)
}

fn equal_proof_streams(
    a: &mut impl std::io::Read,
    b: &mut impl std::io::Read,
) -> Result<(), String> {
    let mut aa = [0u8; 64 * 1024];
    let mut bb = [0u8; 64 * 1024];
    loop {
        let an = read_proof_chunk(a, &mut aa)?;
        let bn = read_proof_chunk(b, &mut bb)?;
        if an != bn || aa[..an] != bb[..bn] {
            return Err("ordinary diagnostic complete generic State bytes differ".into());
        }
        if an == 0 {
            return Ok(());
        }
    }
}

fn equal_state_proofs(a: &GenericStateProof, b: &GenericStateProof) -> Result<(), String> {
    // Reject metadata disagreement, but even matching SHA/counts never replace
    // full ordered byte comparison with an explicit EOF check on both streams.
    if a.bytes != b.bytes || a.sha256 != b.sha256 {
        return Err("ordinary diagnostic generic State identities differ".into());
    }
    let mut aa = fs::File::open(&a.path).map_err(|e| e.to_string())?;
    let mut bb = fs::File::open(&b.path).map_err(|e| e.to_string())?;
    if aa.metadata().map_err(|e| e.to_string())?.len() != a.bytes
        || bb.metadata().map_err(|e| e.to_string())?.len() != b.bytes
    {
        return Err("ordinary diagnostic generic State file length differs".into());
    }
    equal_proof_streams(&mut aa, &mut bb)
}

#[test]
fn ordinary_generic_state_stream_matches_complete_canonical_oracle() {
    // Construct in nonlexicographic textual order, including nested numeric map
    // keys, negative quantities, control/Unicode strings, null and arrays.
    let state: Value = serde_json::from_str(
        r#"{"z":null,"2":{"10":"-7","2":"雪\u0000\n\""},"a":[true,false,0,-4,{"z":1,"a":2}]}"#,
    )
    .unwrap();
    let expected = canonical(&state).unwrap().into_bytes();
    let mut actual = Vec::new();
    let (hash, bytes) = write_generic_state_value(&state, &mut actual).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(bytes, expected.len() as u64);
    assert_eq!(hash, sha256(&expected));
    assert!(write_generic_state_value(&Value::Null, &mut actual).is_err());
    assert!(write_generic_state_value(&json!([]), &mut actual).is_err());
}

#[test]
fn ordinary_generic_state_stream_checks_all_bytes_eof_and_io_errors() {
    use std::io::{self, Read};
    struct ShortRead<'a> {
        remaining: &'a [u8],
        interrupted: bool,
    }
    impl Read for ShortRead<'_> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let n = self.remaining.len().min(out.len()).min(7);
            out[..n].copy_from_slice(&self.remaining[..n]);
            self.remaining = &self.remaining[n..];
            Ok(n)
        }
    }
    let data: Vec<u8> = (0..(128 * 1024 + 13)).map(|n| (n % 251) as u8).collect();
    fn short(bytes: &[u8]) -> ShortRead<'_> {
        ShortRead {
            remaining: bytes,
            interrupted: false,
        }
    }
    equal_proof_streams(&mut short(&data), &mut data.as_slice()).unwrap();
    for index in [0, 64 * 1024 - 1, 64 * 1024, data.len() - 1] {
        let mut changed = data.clone();
        changed[index] ^= 1;
        assert!(equal_proof_streams(&mut data.as_slice(), &mut short(&changed)).is_err());
    }
    assert!(
        equal_proof_streams(&mut data.as_slice(), &mut short(&data[..data.len() - 1])).is_err()
    );
    let mut appended = data.clone();
    appended.push(0);
    assert!(equal_proof_streams(&mut short(&data), &mut appended.as_slice()).is_err());
    struct BrokenRead;
    impl Read for BrokenRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("read failure"))
        }
    }
    assert!(equal_proof_streams(&mut data.as_slice(), &mut BrokenRead).is_err());

    struct BrokenWrite {
        written: usize,
        fail_flush: bool,
    }
    impl Write for BrokenWrite {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            if !self.fail_flush && self.written >= 9 {
                return Err(io::Error::other("write failure"));
            }
            let n = data.len().min(3);
            self.written += n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::other("flush failure"))
            } else {
                Ok(())
            }
        }
    }
    let state = json!({"z": "control\n雪", "a": [1, 2, 3]});
    for fail_flush in [false, true] {
        assert!(
            write_generic_state_value(
                &state,
                &mut BrokenWrite {
                    written: 0,
                    fail_flush
                }
            )
            .is_err()
        );
    }
    struct PartialWrite(Vec<u8>);
    impl Write for PartialWrite {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let n = bytes.len().min(3);
            self.0.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut out = PartialWrite(Vec::new());
    let (hash, bytes) = write_generic_state_value(&state, &mut out).unwrap();
    assert_eq!(out.0, canonical(&state).unwrap().into_bytes());
    assert_eq!(bytes, out.0.len() as u64);
    assert_eq!(hash, sha256(&out.0));
}

#[test]
fn ordinary_generic_state_files_are_private_and_removed_after_complete_comparison() {
    let mut one = GenericStateProof::create().unwrap();
    let path = one.directory.clone();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&one.path)
        .unwrap();
    file.write_all(b"all-state-bytes").unwrap();
    file.flush().unwrap();
    assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    drop(file);
    let mut two = GenericStateProof::create().unwrap();
    let mut other = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&two.path)
        .unwrap();
    other.write_all(b"all-state-bytes").unwrap();
    other.flush().unwrap();
    drop(other);
    one.bytes = b"all-state-bytes".len() as u64;
    two.bytes = one.bytes;
    one.sha256 = sha256(b"all-state-bytes");
    two.sha256 = one.sha256.clone();
    // Metadata intentionally agrees: byte mutation must still be detected.
    equal_state_proofs(&one, &two).unwrap();
    fs::write(&two.path, b"all-state-byteS").unwrap();
    assert!(equal_state_proofs(&one, &two).is_err());
    one.remove().unwrap();
    assert!(!path.exists());
    let second = two.directory.clone();
    drop(two);
    assert!(!second.exists());
}

/// Exactly one response + all automatic rules to the next exposed boundary.
/// Observation and policy are timed independently before this core segment.
fn ordinary_prefix(catalog: &Catalog, count: usize, quantum: u64) -> Result<Run, String> {
    if count == 0 || count > PREFIX_INPUTS || quantum == 0 {
        return Err("ordinary diagnostic count/quantum out of range".into());
    }
    let config =
        MatchConfig::for_profile("commander-foundations-v1", seed(1), PlayerId(1)).unwrap();
    let clock = Instant::now();
    let prepared = Prepared::admit_pod(catalog.clone(), config).map_err(|e| format!("{e:?}"))?;
    let admission_ns = clock.elapsed().as_nanos();
    if prepared.config().cards.len() != 400 || prepared.config().commanders.len() != 4 {
        return Err("ordinary diagnostic requires four legal hundred-card decks".into());
    }
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    let mut strategy = Strategy {
        catalog,
        policy: Policy::Develop,
        mulligans: [0; 4],
        mana_plan: Default::default(),
        mana_color: None,
    };
    let (mut work, mut advances, mut queries) = (0, 0, 0);
    let clock = Instant::now();
    let mut boundary = next(&mut game, quantum, &mut work, &mut advances)?;
    let setup_advance_ns = clock.elapsed().as_nanos();
    let mut timing = Timings {
        admission_ns,
        setup_advance_ns,
        observation_ns: vec![],
        policy_ns: vec![],
        check_submit_advance_ns: vec![],
        response_order: vec![],
        replay_verify_ns: 0,
        dispose_ns: 0,
    };
    let mut responses = BTreeMap::new();
    let mut commander_casts = 0;
    let mut noncreature_casts = 0;
    let mut command_proposal_inputs = std::collections::BTreeSet::new();
    let mut survivor_counts = vec![4];
    let mut outcome = "fixed_prefix_horizon".to_string();
    for _ in 0..count {
        let request = match &boundary {
            Advance::AwaitingDecision(request) => request,
            Advance::Finished { .. } => {
                outcome = "finished".into();
                break;
            }
            Advance::Draw => {
                outcome = "draw".into();
                break;
            }
            other => return Err(format!("ordinary diagnostic invalid boundary: {other:?}")),
        };
        let clock = Instant::now();
        let observation = observe(&game, request.player(), quantum, &mut work, &mut queries)?;
        timing.observation_ns.push(clock.elapsed().as_nanos());
        let clock = Instant::now();
        let response = strategy.choose(&observation)?;
        timing.policy_ns.push(clock.elapsed().as_nanos());
        count_tag(&response, "response", &mut responses)?;
        timing.response_order.push(
            serde_json::to_value(&response).map_err(|e| e.to_string())?["response"]
                .as_str()
                .ok_or("response tag missing")?
                .to_string(),
        );
        if let PlayerResponse::Cast { card } = &response
            && observation
                .view
                .objects
                .iter()
                .any(|object| object.handle == *card && matches!(object.zone, Zone::Command { .. }))
        {
            commander_casts += 1;
            command_proposal_inputs.insert(game.accepted_input_counts()?.decisions + 1);
        }
        if let PlayerResponse::Cast { card } = &response
            && let Some(object) = observation
                .view
                .objects
                .iter()
                .find(|object| object.handle == *card)
            && definition(catalog, &object.oracle_id)?.characteristics.kind != CardKind::Creature
        {
            noncreature_casts += 1;
        }
        let token = observation
            .decision
            .as_ref()
            .ok_or("ordinary decision missing")?
            .token()
            .clone();
        let clock = Instant::now();
        match complete(
            &game,
            || game.check_player_response(request.player(), token, response, Budget::new(quantum)),
            quantum,
            &mut work,
            &mut queries,
        )? {
            QueryValue::Validated(receipt) => game.submit(receipt)?,
            _ => return Err("ordinary diagnostic wrong receipt".into()),
        }
        boundary = next(&mut game, quantum, &mut work, &mut advances)?;
        timing
            .check_submit_advance_ns
            .push(clock.elapsed().as_nanos());
        let living = game.live_players().len();
        if survivor_counts.last() != Some(&living) {
            survivor_counts.push(living);
        }
    }
    match boundary {
        Advance::Finished { .. } => outcome = "finished".into(),
        Advance::Draw => outcome = "draw".into(),
        Advance::AwaitingDecision(_) => {}
        other => return Err(format!("ordinary final boundary invalid: {other:?}")),
    }
    let journal = game.replay_journal();
    if !journal.concessions.is_empty()
        || journal.decisions.len() != timing.check_submit_advance_ns.len()
    {
        return Err("ordinary prefix input conservation failed".into());
    }
    let final_rule_boundary = game.rule_boundary();
    let digest = game.state_digest()?;
    let clock = Instant::now();
    // The strict generic proof streams each complete history view to a private
    // file; no complete snapshot String or history Value is retained here.
    // Preserve every State byte until the exact replay comparison completes.
    let original = generic_state_proof(&game)?;
    if original.sha256 != digest {
        return Err("ordinary original generic State digest mismatch".into());
    }
    // The full original State survives in its exact generic proof and journal.
    // Release the original Game before constructing the Detailed replay, rather
    // than retaining both complete Games throughout the replay's proof work.
    let dispose_clock = Instant::now();
    drop(game);
    let original_dispose_ns = dispose_clock.elapsed().as_nanos();
    let replay = Game::replay_with_trace(prepared, &journal, TraceMode::Detailed)?;
    let replayed = generic_state_proof(&replay)?;
    equal_state_proofs(&original, &replayed)?;
    if replayed.sha256 != digest || replay.state_digest()? != digest {
        return Err("ordinary diagnostic independent replay mismatch".into());
    }
    // Release both complete proof files immediately, before trace/event proof
    // processing; successful repetitions never accumulate cached proof files.
    original.remove()?;
    replayed.remove()?;
    let mut events = BTreeMap::new();
    let mut actual_outcomes = BTreeMap::new();
    for event in replay.events() {
        count_tag(event, "event", &mut events)?;
        if let mtg_core::Event::Committed { actual, .. } = event {
            count_tag(actual, "outcome", &mut actual_outcomes)?;
        }
    }
    // Join actual proposal identities to actual completed casting events.
    // A chosen/validated Cast alone does not establish a paid command cast.
    let mut proposed = BTreeMap::new();
    let mut paid = std::collections::BTreeSet::new();
    let (mut paid_command_casts, mut paid_noncreature_casts) = (0, 0);
    for record in replay.trace().records.iter() {
        let mtg_core::EvidenceRecord::Event { event } = &record.record else {
            continue;
        };
        match event.as_ref() {
            mtg_core::Event::SpellProposed { card, stack_id } => {
                let source = replay
                    .config()
                    .cards
                    .iter()
                    .find(|row| row.id == *card)
                    .ok_or("actual cast source missing")?;
                let from_command = replay.config().commanders.contains(card)
                    && command_proposal_inputs.contains(&record.decisions_seen);
                let noncreature = definition(catalog, &source.oracle_id)?.characteristics.kind
                    != CardKind::Creature;
                if proposed
                    .insert(*stack_id, (from_command, noncreature))
                    .is_some()
                {
                    return Err("duplicate actual proposal identity".into());
                }
            }
            mtg_core::Event::SpellCast { stack_id } => {
                let (from_command, noncreature) = proposed
                    .get(stack_id)
                    .ok_or("paid cast lacks actual proposal")?;
                if !paid.insert(*stack_id) {
                    return Err("duplicate actual paid cast".into());
                }
                paid_command_casts += usize::from(*from_command);
                paid_noncreature_casts += usize::from(*noncreature);
            }
            _ => {}
        }
    }
    let trace_sha256 = mtg_contracts::digest(replay.trace())?;
    // The early original-Game disposal is reported in dispose_ns; remove it
    // here so summing component fields does not count that interval twice.
    timing.replay_verify_ns = clock
        .elapsed()
        .as_nanos()
        .checked_sub(original_dispose_ns)
        .ok_or("ordinary replay proof duration underflow")?;
    let semantics = Semantics {
        inputs: journal.decisions.len(),
        rule_steps: final_rule_boundary,
        advances,
        query_calls: queries,
        response_kinds: responses,
        event_kinds: events,
        actual_outcome_kinds: actual_outcomes,
        commander_casts,
        noncreature_casts,
        paid_command_casts,
        paid_noncreature_casts,
        survivor_counts,
        outcome,
        final_state_sha256: digest,
        journal_sha256: mtg_contracts::digest(&journal)?,
        trace_sha256,
        journal: journal.clone(),
    };
    let clock = Instant::now();
    drop(replay);
    drop(journal);
    timing.dispose_ns = original_dispose_ns
        .checked_add(clock.elapsed().as_nanos())
        .ok_or("ordinary disposal duration overflow")?;
    Ok(Run {
        semantics,
        timings: timing,
    })
}

fn private_output(variable: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(
        std::env::var(variable).map_err(|_| "explicit diagnostic output required")?,
    );
    if !path.is_absolute()
        || path.exists()
        || path
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        return Err("diagnostic output must be new and absolute".into());
    }
    let parent = path.parent().ok_or("diagnostic parent missing")?;
    for ancestor in parent.ancestors() {
        if fs::symlink_metadata(ancestor)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("symlink diagnostic ancestry".into());
        }
    }
    if fs::metadata(parent)
        .map_err(|e| e.to_string())?
        .permissions()
        .mode()
        & 0o777
        != 0o700
    {
        return Err("diagnostic parent must be private0700".into());
    }
    Ok(path)
}

fn publish(file: &mut fs::File, report: &Value) -> Result<(), String> {
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let data = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    file.write_all(&data).map_err(|e| e.to_string())?;
    file.set_len(data.len() as u64).map_err(|e| e.to_string())?;
    file.sync_data().map_err(|e| e.to_string())
}

#[test]
fn ordinary_prefix_replays_and_preserves_budget_partition_semantics() {
    let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
    let full = ordinary_prefix(&catalog, 64, QUANTUM).unwrap();
    let tiny = ordinary_prefix(&catalog, 64, 1).unwrap();
    assert_eq!(full.semantics.inputs, 64);
    assert_eq!(
        full.semantics.final_state_sha256,
        tiny.semantics.final_state_sha256
    );
    assert_eq!(full.semantics.journal_sha256, tiny.semantics.journal_sha256);
    assert_eq!(full.semantics.trace_sha256, tiny.semantics.trace_sha256);
    assert!(tiny.semantics.query_calls > full.semantics.query_calls);
    assert!(ordinary_prefix(&catalog, 0, QUANTUM).is_err());
    assert!(ordinary_prefix(&catalog, 1, 0).is_err());
    assert!(ordinary_prefix(&catalog, PREFIX_INPUTS + 1, QUANTUM).is_err());
}

#[test]
fn ordinary_full_fixture_contains_commander_casts_and_simple_effects() {
    let (catalog, load) = load_catalog().unwrap();
    assert_eq!(load["input_bytes"], CATALOG_JSON.len());
    assert!(load["canonical_bytes"].as_u64().unwrap() > 0);
    let full = ordinary_prefix(&catalog, PREFIX_INPUTS, QUANTUM).unwrap();
    assert!(full.semantics.commander_casts > 0, "{:#?}", full.semantics);
    assert!(
        full.semantics.paid_command_casts > 0,
        "{:#?}",
        full.semantics
    );
    assert!(
        full.semantics.paid_noncreature_casts > 0,
        "{:#?}",
        full.semantics
    );
    assert!(
        full.semantics.noncreature_casts > 0,
        "{:#?}",
        full.semantics
    );
    assert!(
        full.semantics
            .actual_outcome_kinds
            .get("life_gained")
            .copied()
            .unwrap_or(0)
            + full
                .semantics
                .actual_outcome_kinds
                .get("life_lost")
                .copied()
                .unwrap_or(0)
            > 0,
        "{:#?}",
        full.semantics
    );
    let repeated = ordinary_prefix(&catalog, PREFIX_INPUTS, QUANTUM).unwrap();
    assert_eq!(full.semantics, repeated.semantics);
}

#[test]
#[ignore = "explicit guarded host measurement; not a product latency acceptance test"]
fn ordinary_legal_pod_diagnostic() {
    let output = private_output("MTG_ORDINARY_OUTPUT").unwrap();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    let mut report = json!({"schema":"ordinary-legal-pod-diagnostic-v1","status":"running","build_id":BUILD_ID,
        "classification":"legal four100card pod; fixed1024input prefix, not necessarily a completed game",
        "policy":"IR-driven Develop Strategy; chooser permitted Observation only",
        "seed_index":1,"starting_player":1,"mulligans":[0,0,0,0],"trace_mode":"off","quantum":QUANTUM,
        "warmups":1,"measured_repeats":REPEATS,"samples":[],
        "scope":"Core segment check+submit+automatic advancement; observation and policy clocks separate. Admission, preparation, replay oracle, serialization and I/O outside segment. Whole host process must be externally bounded and measured."});
    report["setup_decisions"] = json!(
        "Actual initial mulligan/bottom decisions are included and identified in response_order; setup advance before the first input is separately timed."
    );
    report["cast_count_scope"] = json!(
        "commander_casts/noncreature_casts count validated Cast proposals, not completed payment or resolved spells; typed actual_outcome_kinds and event_kinds prove committed effects."
    );
    publish(&mut file, &report).unwrap();
    let result = (|| -> Result<(), String> {
        let (catalog, load) = load_catalog()?;
        report["catalog_load"] = load;
        let warmup = ordinary_prefix(&catalog, PREFIX_INPUTS, QUANTUM)?;
        if warmup.semantics.commander_casts == 0
            || warmup.semantics.noncreature_casts == 0
            || warmup.semantics.paid_command_casts == 0
            || warmup.semantics.paid_noncreature_casts == 0
            || warmup
                .semantics
                .response_kinds
                .get("cast")
                .copied()
                .unwrap_or(0)
                == 0
        {
            return Err("predeclared ordinary fixture did not exercise commander casting".into());
        }
        report["reference_semantics"] =
            serde_json::to_value(&warmup.semantics).map_err(|e| e.to_string())?;
        report["warmup_timings"] = json!(warmup.timings);
        publish(&mut file, &report)?;
        for repetition in 0..REPEATS {
            let sample = ordinary_prefix(&catalog, PREFIX_INPUTS, QUANTUM)?;
            if sample.semantics != warmup.semantics {
                return Err("ordinary repeated complete semantic output differs".into());
            }
            report["samples"].as_array_mut().ok_or("sample array missing")?.push(json!({"repetition":repetition,"semantics":sample.semantics,"timings":sample.timings}));
            publish(&mut file, &report)?;
        }
        Ok(())
    })();
    report["status"] = json!(if result.is_ok() { "passed" } else { "failed" });
    report["error"] = json!(result.as_ref().err());
    publish(&mut file, &report).unwrap();
    result.unwrap();
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct BatchSemantics {
    size: usize,
    query_calls: usize,
    bytes: usize,
    serialized_sha256: String,
}

#[derive(Serialize)]
struct BatchTimes {
    query_ns: u128,
    copy_ns: u128,
    serialize_ns: u128,
    dispose_ns: u128,
}

/// A native observation->owned copy->JSON batch adapter, no network or FFI.
/// The same real root is queried repeatedly; it is not N independent games.
fn observation_batch(
    game: &Game,
    viewer: PlayerId,
    size: usize,
) -> Result<(BatchSemantics, BatchTimes), String> {
    if ![1, 2, 64, 1024].contains(&size) {
        return Err("unsupported diagnostic batch size".into());
    }
    let before = game.snapshot()?;
    let mut work = 0;
    let mut calls = 0;
    let expected = observe(game, viewer, QUANTUM, &mut work, &mut calls)?;
    let expected_json = canonical(&vec![expected.clone(); size])?;
    let initial_calls = calls;
    let mut rows = Vec::with_capacity(size);
    let clock = Instant::now();
    for _ in 0..size {
        rows.push(observe(game, viewer, QUANTUM, &mut work, &mut calls)?);
    }
    let query_ns = clock.elapsed().as_nanos();
    let clock = Instant::now();
    let copies = rows.clone();
    let copy_ns = clock.elapsed().as_nanos();
    let clock = Instant::now();
    let encoded = canonical(&copies)?;
    let serialize_ns = clock.elapsed().as_nanos();
    if rows.iter().any(|row| row != &expected)
        || rows != copies
        || encoded != expected_json
        || game.snapshot()? != before
    {
        return Err("observation/copy/serialization purity or exact output mismatch".into());
    }
    let semantics = BatchSemantics {
        size,
        query_calls: calls - initial_calls,
        bytes: encoded.len(),
        serialized_sha256: sha256(encoded.as_bytes()),
    };
    let clock = Instant::now();
    drop(rows);
    drop(copies);
    drop(encoded);
    let times = BatchTimes {
        query_ns,
        copy_ns,
        serialize_ns,
        dispose_ns: clock.elapsed().as_nanos(),
    };
    Ok((semantics, times))
}

fn snapshot_roundtrip(game: &Game, prepared: Prepared) -> Result<Value, String> {
    let digest = game.state_digest()?;
    let clock = Instant::now();
    let snapshot = game.snapshot()?;
    let snapshot_ns = clock.elapsed().as_nanos();
    let clock = Instant::now();
    let restored = Game::restore(prepared, &snapshot)?;
    let restore_ns = clock.elapsed().as_nanos();
    if restored.snapshot()? != snapshot
        || restored.state_digest()? != digest
        || restored.replay_journal() != game.replay_journal()
    {
        return Err("artifact restore changed real history/state".into());
    }
    let bytes = snapshot.len();
    let snapshot_sha256 = sha256(snapshot.as_bytes());
    let clock = Instant::now();
    drop(restored);
    drop(snapshot);
    Ok(
        json!({"snapshot_bytes":bytes,"snapshot_sha256":snapshot_sha256,"state_sha256":digest,"snapshot_ns":snapshot_ns,"restore_ns":restore_ns,"dispose_ns":clock.elapsed().as_nanos()}),
    )
}

#[test]
fn ordinary_adapter_preserves_survivor_history_and_exact_batches() {
    let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
    for (root, living) in [(0, 4), (4, 3), (8, 2)] {
        let fixture = crate::search_roots::prepare(&catalog, root).unwrap();
        assert_eq!(fixture.game.live_players().len(), living);
        assert_eq!(fixture.game.replay_journal().concessions.len(), 4 - living);
        let before = fixture.game.snapshot().unwrap();
        let (one, _) = observation_batch(&fixture.game, fixture.case.viewer, 1).unwrap();
        let (two, _) = observation_batch(&fixture.game, fixture.case.viewer, 2).unwrap();
        assert!(one.bytes > 0 && two.bytes > one.bytes);
        assert_eq!(two.query_calls, 2 * one.query_calls);
        assert!(observation_batch(&fixture.game, fixture.case.viewer, 3).is_err());
        let prepared = Prepared::admit_pod(catalog.clone(), fixture.case.config).unwrap();
        let result = snapshot_roundtrip(&fixture.game, prepared).unwrap();
        assert_eq!(result["state_sha256"], fixture.game.state_digest().unwrap());
        assert_eq!(fixture.game.snapshot().unwrap(), before);
    }
}

#[test]
#[ignore = "explicit guarded host artifact/batch diagnostic"]
fn ordinary_artifact_batch_diagnostic() {
    let output = private_output("MTG_ORDINARY_OUTPUT").unwrap();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    let mut report = json!({"schema":"ordinary-artifact-batch-v1","status":"running","build_id":BUILD_ID,
        "scope":"Twelve unchanged legal-pod root fixtures, including declared pre-setup departures. Query/copy/JSON batches1/64/1024 repeat the same root; not independent pods or a network/FFI consumer. Snapshot serialize and normal replay-validating restore measured separately.",
        "trace_mode":"off","warmups":1,"measured_repeats":REPEATS,"roots":[]});
    publish(&mut file, &report).unwrap();
    let result = (|| -> Result<(), String> {
        let (catalog, load) = load_catalog()?;
        report["catalog_load"] = load;
        for index in 0..crate::search_roots::ROOT_COUNT {
            let clock = Instant::now();
            let fixture = crate::search_roots::prepare(&catalog, index)?;
            let prepare_ns = clock.elapsed().as_nanos();
            let prepared = Prepared::admit_pod(catalog.clone(), fixture.case.config.clone())
                .map_err(|e| format!("{e:?}"))?;
            let state = fixture.game.state_digest()?;
            let snapshot_ref = snapshot_roundtrip(&fixture.game, prepared.clone())?;
            let mut root = json!({"index":index,"case":fixture.case,"live_players":fixture.game.live_players(),"state_sha256":state,
                "journal":fixture.game.replay_journal(),"prepare_ns":prepare_ns,"snapshot_warmup":snapshot_ref,"snapshots":[],"batches":[]});
            for _ in 0..REPEATS {
                let sample = snapshot_roundtrip(&fixture.game, prepared.clone())?;
                if sample["snapshot_sha256"] != snapshot_ref["snapshot_sha256"]
                    || sample["snapshot_bytes"] != snapshot_ref["snapshot_bytes"]
                {
                    return Err("snapshot repeated bytes differ".into());
                }
                root["snapshots"]
                    .as_array_mut()
                    .ok_or("snapshot array")?
                    .push(sample);
            }
            for size in [1, 64, 1024] {
                let (reference, warmup) =
                    observation_batch(&fixture.game, fixture.case.viewer, size)?;
                let mut row = json!({"semantics":reference,"warmup":warmup,"samples":[]});
                for repetition in 0..REPEATS {
                    let (semantics, timing) =
                        observation_batch(&fixture.game, fixture.case.viewer, size)?;
                    if semantics != reference {
                        return Err("adapter batch repeated complete semantics differ".into());
                    }
                    row["samples"]
                        .as_array_mut()
                        .ok_or("batch sample array")?
                        .push(
                            json!({"repetition":repetition,"semantics":semantics,"timings":timing}),
                        );
                }
                root["batches"]
                    .as_array_mut()
                    .ok_or("batch array")?
                    .push(row);
            }
            if fixture.game.state_digest()? != state {
                return Err("artifact root changed".into());
            }
            report["roots"]
                .as_array_mut()
                .ok_or("root array")?
                .push(root);
            publish(&mut file, &report)?;
        }
        Ok(())
    })();
    report["status"] = json!(if result.is_ok() { "passed" } else { "failed" });
    report["error"] = json!(result.as_ref().err());
    publish(&mut file, &report).unwrap();
    result.unwrap();
}

/// Separate non-inlined attribution root for the external heap/CPU collector.
/// Owned inputs are cloned BEFORE this call; no chooser or serialization runs
/// here. Game::respond performs the ordinary trusted-native input validation.
#[inline(never)]
fn raw_core_prefix(
    game: &mut Game,
    inputs: Vec<mtg_core::AcceptedInput>,
    mut boundary: Advance,
) -> Result<(usize, usize), String> {
    let (mut work, mut advances, mut accepted) = (0, 0, 0);
    for input in inputs {
        let Advance::AwaitingDecision(request) = &boundary else {
            return Err("raw core prefix stopped before recorded input".into());
        };
        if request.player() != input.player || request.decision_id() != input.decision_id {
            return Err("raw core prefix decision identity differs".into());
        }
        game.respond(input.player, input.decision_id, input.response)?;
        accepted += 1;
        boundary = next(game, QUANTUM, &mut work, &mut advances)?;
    }
    if !matches!(
        boundary,
        Advance::AwaitingDecision(_) | Advance::Finished { .. } | Advance::Draw
    ) {
        return Err("raw core prefix invalid final boundary".into());
    }
    Ok((accepted, advances))
}

fn raw_fixture(catalog: &Catalog) -> Result<(Game, Advance), String> {
    let prepared = Prepared::admit_pod(
        catalog.clone(),
        MatchConfig::for_profile("commander-foundations-v1", seed(1), PlayerId(1)).unwrap(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    let (mut work, mut calls) = (0, 0);
    let boundary = next(&mut game, QUANTUM, &mut work, &mut calls)?;
    Ok((game, boundary))
}

#[test]
fn ordinary_raw_core_prefix_matches_player_checked_execution() {
    let (catalog, _) = load_catalog().unwrap();
    let reference = ordinary_prefix(&catalog, 64, QUANTUM).unwrap();
    let (mut game, boundary) = raw_fixture(&catalog).unwrap();
    let (accepted, _) = raw_core_prefix(
        &mut game,
        reference.semantics.journal.decisions.clone(),
        boundary,
    )
    .unwrap();
    assert_eq!(accepted, reference.semantics.inputs);
    assert_eq!(
        game.state_digest().unwrap(),
        reference.semantics.final_state_sha256
    );
    assert_eq!(game.replay_journal(), reference.semantics.journal);
    let (mut fresh, boundary) = raw_fixture(&catalog).unwrap();
    let mut wrong = reference.semantics.journal.decisions.clone();
    wrong[0].decision_id += 1;
    assert!(raw_core_prefix(&mut fresh, wrong, boundary).is_err());
    assert!(fresh.replay_journal().decisions.is_empty());
}

#[test]
#[ignore = "explicit external paused gprofng collector; never run directly"]
fn ordinary_core_profile_diagnostic() {
    // Documented external control, not unsafe FFI or a custom allocator:
    // https://sourceware.org/binutils/docs/gprofng.html (collect app -y/-H).
    assert_eq!(std::env::var("MTG_GPROFNG_PAUSED").as_deref(), Ok("USR2"));
    let output = private_output("MTG_ORDINARY_OUTPUT").unwrap();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    let mut report = json!({"schema":"ordinary-core-profile-v1","status":"running","build_id":BUILD_ID,
        "scope":"External collector starts paused. Reference fixture/policy/oracle and initial advance precede resume. The raw_core_prefix attribution root consumes precloned owned native inputs, validates and runs automatic rules, without policy/Observation/serialization. Total phase allocations also include signal-bookend overhead; no guessed subtraction or pure-core total claim.",
        "attribution_function":"match_games::ordinary_diagnostic::raw_core_prefix",
        "control":"external gprofng -y USR2, starts paused; two documented signals; host must pin collector and kill tool identities and retain actual profiler warnings"});
    publish(&mut file, &report).unwrap();
    let result = (|| -> Result<(), String> {
        let (catalog, load) = load_catalog()?;
        report["catalog_load"] = load;
        let reference = ordinary_prefix(&catalog, PREFIX_INPUTS, QUANTUM)?;
        let inputs = reference.semantics.journal.decisions.clone();
        let (mut game, boundary) = raw_fixture(&catalog)?;
        let pid = std::process::id().to_string();
        let mut resume = std::process::Command::new("/usr/bin/kill");
        resume.args(["-USR2", &pid]);
        let mut pause = std::process::Command::new("/usr/bin/kill");
        pause.args(["-USR2", &pid]);
        if !resume.status().map_err(|e| e.to_string())?.success() {
            return Err("profiler resume signal failed".into());
        }
        let clock = Instant::now();
        let execution = raw_core_prefix(&mut game, inputs, boundary);
        let core_ns = clock.elapsed().as_nanos();
        if !pause.status().map_err(|e| e.to_string())?.success() {
            return Err("profiler pause signal failed".into());
        }
        let (accepted, advances) = execution?;
        if accepted != reference.semantics.inputs
            || game.replay_journal() != reference.semantics.journal
            || game.state_digest()? != reference.semantics.final_state_sha256
        {
            return Err("profiled raw core differs from checked legal fixture".into());
        }
        // Independent generic full-State serialization after collection pauses.
        let generic = generic_state_proof(&game)?;
        if generic.sha256 != reference.semantics.final_state_sha256 {
            return Err("profile generic State digest mismatch".into());
        }
        generic.remove()?;
        report["accepted_inputs"] = json!(accepted);
        report["advance_calls"] = json!(advances);
        report["core_ns"] = json!(core_ns);
        report["reference_semantics"] = json!(reference.semantics);
        Ok(())
    })();
    report["status"] = json!(if result.is_ok() { "passed" } else { "failed" });
    report["error"] = json!(result.as_ref().err());
    publish(&mut file, &report).unwrap();
    result.unwrap();
}

#[cfg(feature = "diagnostic-allocations")]
#[test]
#[ignore = "explicit fresh counter-build diagnostic process only"]
fn ordinary_core_allocation_diagnostic() {
    use crate::diagnostic_allocations as allocation;
    assert_eq!(
        std::env::var("MTG_DIAGNOSTIC_ALLOCATIONS").as_deref(),
        Ok(allocation::ID)
    );
    let output = private_output("MTG_ORDINARY_OUTPUT").unwrap();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    let mut report = json!({
        "schema":"ordinary-core-allocation-v1", "status":"running", "build_id":BUILD_ID,
        "instrumentation":{"id":allocation::ID,"crate":"stats_alloc","version":allocation::CRATE_VERSION,
            "required_SIM_features":["diagnostic-allocations"],"allocator":"StatsAlloc<System>",
            "cfg":"test AND feature diagnostic-allocations","scope":"process-global Rust global-allocator requests; alloc+alloc_zeroed combined, realloc and dealloc separate; not successful allocations or all C/System calls"},
        "phase":"Region snapshot immediately before existing raw_core_prefix; final raw counters immediately after return; preclones/reference/initial advance/proof/report/disposal outside; consumption of precloned owned inputs is inside",
        "separate_instrumentation_authority_required":true,
        "latency_acceptance_or_production_allocator_claim":false
    });
    publish(&mut file, &report).unwrap();
    let result = (|| -> Result<(), String> {
        let (catalog, load) = load_catalog()?;
        report["catalog_load"] = load;
        let reference = ordinary_prefix(&catalog, PREFIX_INPUTS, QUANTUM)?;
        let inputs = reference.semantics.journal.decisions.clone();
        let (mut game, boundary) = raw_fixture(&catalog)?;
        let threads = allocation::ThreadGuard::capture()?;
        let control_before = allocation::empty_control()?;
        let clock = Instant::now();
        let region = allocation::begin();
        let execution = raw_core_prefix(&mut game, inputs, boundary);
        let counters_after = allocation::snapshot();
        let core_ns = clock.elapsed().as_nanos();
        // No JSON, output, allocation-based diagnostics or cleanup precedes the
        // final snapshot. Region::change is avoided for checked arithmetic.
        let counters_before = allocation::initial(&region);
        let counts = allocation::difference(counters_before, counters_after)?;
        threads.unchanged()?;
        let control_after = allocation::empty_control()?;
        let (accepted, advances) = execution?;
        if accepted != PREFIX_INPUTS
            || accepted != reference.semantics.inputs
            || game.replay_journal() != reference.semantics.journal
            || game.state_digest()? != reference.semantics.final_state_sha256
        {
            return Err("allocation raw core differs from checked legal fixture".into());
        }
        let generic = generic_state_proof(&game)?;
        if generic.sha256 != reference.semantics.final_state_sha256 {
            return Err("allocation generic State digest mismatch".into());
        }
        generic.remove()?;
        report["thread_guard"] = json!(threads);
        report["empty_controls"] = json!([control_before, control_after]);
        report["raw_before"] = json!(counters_before);
        report["raw_after"] = json!(counters_after);
        report["checked_difference"] = json!(counts);
        report["accepted_inputs"] = json!(accepted);
        report["advance_calls"] = json!(advances);
        report["instrumented_core_ns"] = json!(core_ns);
        report["request_rate_denominator"] = json!({"accepted_inputs":accepted,"no_rule_boundary_claim":true,"no_control_subtraction":true});
        report["reference_semantics"] = json!(reference.semantics);
        Ok(())
    })();
    report["status"] = json!(if result.is_ok() { "passed" } else { "failed" });
    report["error"] = json!(result.as_ref().err());
    publish(&mut file, &report).unwrap();
    result.unwrap();
}

#[cfg(test)]
mod shrinking_choice_costs;

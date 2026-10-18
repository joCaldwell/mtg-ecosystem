//! Test-only finite definition-domain diagnostics; never public card admission.
//! CR 611.2/613.4c/514.2; the synthetic constants below are not Oracle text.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::{hint::black_box, time::Instant};

#[path = "definition_diversity_tests/heterogeneous.rs"]
mod heterogeneous;

const CONTRACT: &str = r#"{
 "schema":"definition-diversity-diagnostic-v1",
 "synthetic_counts":[0,8,64,256],"selector_partitions":4,
 "payload_uniqueness":"canonical typed instruction plus stable partition, excluding effect IDs/source IDs; each tuple must be unique",
 "generator":"index=partition+4*k; power=(-1)^k*(k+1), toughness=2*(k+1)+partition; exact signed integer Quantity constants",
 "oracle":"per-partition m terms: power sum m odd?(m+1)/2:-m/2; toughness m*(m+1)+partition*m, added to archived base characteristics",
 "synthetic_scope":"pure existing modifier evaluator; generated constants are not authentic card text or cast invocations; normal restore rejects nonempty domains; above-capacity domains never advance",
 "cold_warm":"fresh generic State decode resets immutable history-byte memo, exact full generic bytes/SHA checked before and after repeat; characteristics currently has no memo, first/repeat calls are labeled accordingly",
 "invalidation":"fork a warmed domain, change one modifier power/toughness without changing identities, verify changed arithmetic and full State bytes/SHA and unchanged sibling; cleanup uses existing production typed event",
 "heterogeneous_sources":["Blood Artist","Essence Warden","Elvish Visionary","Slaughterhouse Bouncer","Furnace of Rath","Giant Growth","Disfigure","Fog"],
 "heterogeneous_scope":"private rejected configuration; three complete archived spells actually paid/resolved; typed fixture movements collect actual trigger matches/nonmatches; exact condition results; cleanup expires three resolved effects while static provider remains",
 "warmups":1,"measured_repetitions":5,"horizon_or_pool_changes":false,
 "outer_guard":"120s-2gib-v1 must be proved by independent host, env alone is not proof",
 "capacity_claim":"no admission or supported invocation creation above effect capacity; authentic256different Oracle abilities unavailable in finite catalog",
 "semantic_denominators":"unique authentic source programs, unique canonical synthetic payloads, instances and observed work reported separately",
 "timing":"production evaluator/hash/respond/advance/commit only; setup, generic byte/SHA/arithmetic oracles, manifests and report serialization outside segment clocks but inside whole process envelope",
 "product_latency_or_strategic_quality_claim":false
}"#;

#[derive(Clone, Debug, Serialize)]
struct Payload {
    partition: usize,
    instruction: Instruction,
}
#[derive(Serialize)]
struct Sample {
    operation: &'static str,
    repetition: usize,
    nanos: u128,
    evaluator_calls: usize,
    state_hash_calls: usize,
}
fn timed<T>(
    samples: &mut Vec<Sample>,
    enabled: bool,
    op: &'static str,
    repetition: usize,
    calls: usize,
    hashes: usize,
    f: impl FnOnce() -> T,
) -> T {
    let start = Instant::now();
    let result = black_box(f());
    if enabled {
        samples.push(Sample {
            operation: op,
            repetition,
            nanos: start.elapsed().as_nanos(),
            evaluator_calls: calls,
            state_hash_calls: hashes,
        });
    }
    result
}
fn canonical_state(state: &State) -> Vec<u8> {
    let generic = canonical(state).unwrap().into_bytes();
    let mut actual = vec![];
    state_digest::write_state(state, &mut actual).unwrap();
    assert_eq!(
        actual, generic,
        "every canonical State byte, including complete history Views"
    );
    assert_eq!(state_digest::state_digest(state).unwrap(), sha256(&generic));
    generic
}
fn payloads(count: usize) -> Vec<Payload> {
    assert!([0, 8, 64, 256].contains(&count));
    let template = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    let source = template
        .definitions
        .iter()
        .find(|d| d.oracle_id == GROWTH)
        .unwrap();
    let program = source.spell.as_ref().unwrap();
    assert_eq!(program.instructions.len(), 1);
    assert!(matches!(
        program.instructions[0],
        Instruction::ModifyPowerToughness { .. }
    ));
    (0..count)
        .map(|index| {
            let partition = index % 4;
            let k = i64::try_from(index / 4).unwrap();
            let power = if k % 2 == 0 { k + 1 } else { -(k + 1) };
            let toughness = 2 * (k + 1) + i64::try_from(partition).unwrap();
            let mut instruction = program.instructions[0].clone();
            let Instruction::ModifyPowerToughness {
                target_slot,
                power: p,
                toughness: t,
                ..
            } = &mut instruction
            else {
                unreachable!()
            };
            *target_slot = u32::try_from(partition).unwrap();
            *p = power.into();
            *t = toughness.into();
            Payload {
                partition,
                instruction,
            }
        })
        .collect()
}
fn unique_payloads(rows: &[Payload]) -> Vec<String> {
    let bytes = rows
        .iter()
        .map(|r| canonical(r).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        bytes.iter().collect::<BTreeSet<_>>().len(),
        rows.len(),
        "identity-free complete typed payload uniqueness, not copied IDs"
    );
    bytes
}
fn fixture() -> (Game, Vec<ObjectId>, ObjectId) {
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    let prepared = Prepared::admit(catalog, ScenarioConfig::effects()).unwrap();
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    for _ in 0..1000 {
        match game.advance(Budget::new(64)) {
            Advance::AwaitingDecision(Request::Priority {
                player: PlayerId(0),
                ..
            }) => break,
            Advance::Suspended { .. } => (),
            other => panic!("unchanged effects fixture initial priority: {other:?}"),
        }
    }
    assert!(game.state.effects.is_empty());
    let targets = game
        .state
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && !game.prepared.config.commanders.contains(&o.id.card)
                && game.definition(o.id.card).unwrap().characteristics.kind == CardKind::Creature
        })
        .map(|o| o.id)
        .take(4)
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 4);
    let source = game
        .state
        .objects
        .values()
        .find(|o| game.definition(o.id.card).unwrap().oracle_id == GROWTH)
        .unwrap()
        .id;
    assert!(game.state.visibility.iter().all(|v| !v.history.is_empty()));
    (game, targets, source)
}
fn seed_domain(base: &Game, targets: &[ObjectId], source: ObjectId, rows: &[Payload]) -> Game {
    let mut game = base.fork();
    for (index, payload) in rows.iter().enumerate() {
        let Instruction::ModifyPowerToughness {
            power,
            toughness,
            source_block,
            ..
        } = &payload.instruction
        else {
            unreachable!()
        };
        let target = targets[payload.partition];
        let source_handles = game.capture_trigger_source_handles(source).unwrap();
        let target_handles = game.capture_trigger_source_handles(target).unwrap();
        game.state.effects.push(ResolvedEffect {
            target_handles,
            id: EffectId(u64::try_from(index).unwrap()),
            origin: EffectOrigin {
                mode: None,
                stack_id: StackId(u32::try_from(index).unwrap()),
                source,
                controller: base.current(source).unwrap().controller,
                source_handles,
                cursor: 0,
                source_block: *source_block,
            },
            expires_in_cleanup_of_turn: game.state.turn.number,
            kind: ResolvedEffectKind::ModifyPowerToughness {
                target,
                power: power.clone(),
                toughness: toughness.clone(),
            },
        });
    }
    game.state.next_effect_id = u64::try_from(rows.len()).unwrap();
    let restore = Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap());
    assert_eq!(
        restore.is_err(),
        !rows.is_empty(),
        "synthetic payloads have no legal invocation history"
    );
    if rows.len() > game.effect_limit() {
        assert_eq!(game.validate_effect_state().unwrap_err(), "effect capacity");
    }
    let mut rejected = game.prepared.config.clone();
    rejected.profile = format!("synthetic-definition-domain-{}", rows.len());
    assert!(Prepared::admit((*game.prepared.catalog).clone(), rejected).is_err());
    game
}
fn arithmetic(base: &Game, targets: &[ObjectId], count: usize) -> Vec<Characteristics> {
    targets
        .iter()
        .enumerate()
        .map(|(partition, target)| {
            // Independent closed form; does not read generated effects/payloads.
            let m = i64::try_from(count / 4 + usize::from(partition < count % 4)).unwrap();
            let power = if m % 2 == 0 { -(m / 2) } else { (m + 1) / 2 };
            let toughness = m * (m + 1) + i64::try_from(partition).unwrap() * m;
            let mut c = base
                .definition(target.card)
                .unwrap()
                .characteristics
                .clone();
            c.power.as_mut().unwrap().add(&power.into());
            c.toughness.as_mut().unwrap().add(&toughness.into());
            c
        })
        .collect()
}
fn evaluate(game: &Game, targets: &[ObjectId]) -> Vec<Characteristics> {
    targets
        .iter()
        .map(|t| game.characteristics(*t).unwrap())
        .collect()
}
fn modifier_row(count: usize, repetition: usize, collect: bool) -> serde_json::Value {
    let (base, targets, source) = fixture();
    let rows = payloads(count);
    let manifest = unique_payloads(&rows);
    let game = seed_domain(&base, &targets, source, &rows);
    let expected = arithmetic(&base, &targets, count);
    let before = canonical_state(&game.state);
    let mut samples = vec![];
    let first = timed(
        &mut samples,
        collect,
        "characteristics_first_no_property_cache",
        repetition,
        4,
        0,
        || evaluate(&game, &targets),
    );
    let again = timed(
        &mut samples,
        collect,
        "characteristics_repeat_no_property_cache",
        repetition,
        4,
        0,
        || evaluate(&game, &targets),
    );
    assert_eq!(first, expected);
    assert_eq!(again, expected);
    let cold: State = serde_json::from_slice(&before).unwrap();
    let cold_sha = timed(
        &mut samples,
        collect,
        "full_State_hash_cold_history_memos",
        repetition,
        0,
        1,
        || state_digest::state_digest(&cold).unwrap(),
    );
    let warm_sha = timed(
        &mut samples,
        collect,
        "full_State_hash_warm_history_memos",
        repetition,
        0,
        1,
        || state_digest::state_digest(&cold).unwrap(),
    );
    assert_eq!(cold_sha, warm_sha);
    assert_eq!(cold_sha, sha256(&before));
    assert_eq!(canonical_state(&cold), before);
    let mut changed = game.fork();
    let mut changed_expected = expected.clone();
    if count > 0 {
        let ResolvedEffectKind::ModifyPowerToughness {
            power, toughness, ..
        } = &mut changed.state.effects[0].kind
        else {
            unreachable!()
        };
        power.add(&1.into());
        toughness.add(&2.into());
        changed_expected[0].power.as_mut().unwrap().add(&1.into());
        changed_expected[0]
            .toughness
            .as_mut()
            .unwrap()
            .add(&2.into());
        assert_ne!(changed_expected, expected);
    }
    let actual = timed(
        &mut samples,
        collect,
        "same_identity_payload_change_characteristics",
        repetition,
        4,
        0,
        || evaluate(&changed, &targets),
    );
    assert_eq!(actual, changed_expected);
    assert_eq!(evaluate(&game, &targets), expected);
    let changed_bytes = canonical_state(&changed.state);
    assert_eq!(changed_bytes == before, count == 0);
    assert_eq!(canonical_state(&game.state), before);
    let mut moved = game.fork();
    timed(
        &mut samples,
        collect,
        "production_target_incarnation_invalidation",
        repetition,
        0,
        0,
        || {
            moved.commit(
                ProposedEvent::Move {
                    object: targets[0],
                    destination: Zone::Exile,
                },
                BatchCause::Fixture,
            )
        },
    )
    .unwrap();
    let successor = moved.object_id(targets[0].card).unwrap();
    assert_ne!(successor, targets[0]);
    assert!(moved.characteristics(targets[0]).is_err());
    assert_eq!(
        moved.characteristics(successor).unwrap(),
        base.definition(targets[0].card).unwrap().characteristics
    );
    for (index, target) in targets.iter().enumerate().skip(1) {
        assert_eq!(moved.characteristics(*target).unwrap(), expected[index]);
    }
    canonical_state(&moved.state);
    assert_eq!(
        canonical_state(&game.state),
        before,
        "independent changed/moved siblings"
    );
    // Actual cleanup operation on this pure evaluator domain; never advance an
    // above-capacity synthetic invocation or claim a normal journal replay.
    let outcome = timed(
        &mut samples,
        collect,
        "production_cleanup_effects_event",
        repetition,
        0,
        0,
        || {
            changed
                .commit(ProposedEvent::CleanupEffects, BatchCause::Fixture)
                .unwrap()
        },
    );
    let EventOutcome::EffectsCleaned { expired, .. } = outcome else {
        panic!("cleanup actual")
    };
    assert_eq!(
        expired,
        (0..count)
            .map(|i| EffectId(u64::try_from(i).unwrap()))
            .collect::<Vec<_>>()
    );
    assert!(changed.state.effects.is_empty());
    assert_eq!(evaluate(&changed, &targets), arithmetic(&base, &targets, 0));
    serde_json::json!({"row":"synthetic_unique_modifier_payloads","repetition":repetition,"warmup":repetition==0,
        "unique_authentic_active_source_programs":0,"archived_operation_templates":1,"synthetic_unique_canonical_payloads":manifest.len(),
        "effect_instances":count,"selector_partitions":4,"complete_payload_manifest":rows,"canonical_payload_sha256s":manifest.iter().map(|s|sha256(s.as_bytes())).collect::<Vec<_>>(),
        "manifest_sha256":digest(&manifest).unwrap(),"expected_characteristics":expected,"changed_characteristics":changed_expected,
        "full_State_sha256":cold_sha,"changed_full_State_sha256":sha256(&changed_bytes),"cleanup_expired_instances":expired.len(),
        "actual_effect_capacity":game.effect_limit(),"above_capacity":count>game.effect_limit(),"normal_restore_rejected":count>0,
        "synthetic_public_admission_rejected":true,"supported_cast_invocation_claim":false,"history_byte_oracles_verified":true,
        "evaluated_objects_per_pass":4,"actual_target_incarnation_invalidation_verified":true,"samples":samples})
}

#[test]
fn definition_diversity_unique_payloads_arithmetic_and_invalidation() {
    for count in [0, 8, 64, 256] {
        modifier_row(count, 0, false);
    }
}
#[test]
#[should_panic(expected = "identity-free complete typed payload uniqueness")]
fn definition_diversity_duplicate_payload_with_different_identity_is_not_diversity() {
    let mut rows = payloads(8);
    rows[7] = rows[0].clone();
    unique_payloads(&rows);
}
#[test]
fn definition_diversity_heterogeneous_source_programs_and_actual_events() {
    for enabled in [false, true] {
        for empty_hand in [false, true] {
            heterogeneous::row(enabled, empty_hand, 0, false);
        }
    }
}
fn semantic_bytes(row: &serde_json::Value) -> String {
    let mut value = row.clone();
    let object = value.as_object_mut().unwrap();
    for key in ["repetition", "warmup", "samples"] {
        object.remove(key);
    }
    if let Some(work) = object.get_mut("work") {
        work.as_object_mut().unwrap().remove("samples");
    }
    canonical(&value).unwrap()
}
fn append_row(
    rows: &mut Vec<serde_json::Value>,
    first: &mut BTreeMap<String, String>,
    key: String,
    mut row: serde_json::Value,
) {
    let bytes = semantic_bytes(&row);
    if let Some(reference) = first.get(&key) {
        assert_eq!(
            &bytes, reference,
            "complete same-work semantics across all six repetitions"
        );
    } else {
        first.insert(key, bytes.clone());
    }
    row["same_work_semantics_sha256"] = sha256(bytes.as_bytes()).into();
    println!(
        "DEFINITION_DIVERSITY_ROW_RESULT={}",
        canonical(&row).unwrap()
    );
    rows.push(row);
}
#[test]
fn definition_diversity_repetition_parity_keeps_every_count_and_state_field() {
    let a = serde_json::json!({"repetition":0,"warmup":true,"samples":[1],"work":{"samples":[2],"accepted_inputs":3},"full_State_sha256":"a"});
    let mut b = a.clone();
    b["repetition"] = 5.into();
    b["warmup"] = false.into();
    b["samples"] = serde_json::json!([99]);
    b["work"]["samples"] = serde_json::json!([98]);
    assert_eq!(semantic_bytes(&a), semantic_bytes(&b));
    b["work"]["accepted_inputs"] = 4.into();
    assert_ne!(semantic_bytes(&a), semantic_bytes(&b));
    b["work"]["accepted_inputs"] = 3.into();
    b["full_State_sha256"] = "changed".into();
    assert_ne!(semantic_bytes(&a), semantic_bytes(&b));
}
#[test]
#[ignore = "parent-authorized external 120s/2GiB guard required; diagnostic not product acceptance"]
fn definition_diversity_host_diagnostic() {
    assert_eq!(
        std::env::var("DEFINITION_DIVERSITY_OUTER_GUARD").as_deref(),
        Ok("120s-2gib-v1")
    );
    let output = std::path::PathBuf::from(
        std::env::var("MTG_DEFINITION_DIVERSITY_OUTPUT").expect("private output"),
    );
    assert!(output.is_absolute() && !output.exists());
    let contract: serde_json::Value = serde_json::from_str(CONTRACT).unwrap();
    let mut rows = vec![];
    let mut first = BTreeMap::new();
    for repetition in 0..6 {
        for count in [0, 8, 64, 256] {
            println!("DEFINITION_DIVERSITY_ROW_START={}", canonical(&serde_json::json!({"row":"synthetic_unique_modifier_payloads","count":count,"repetition":repetition})).unwrap());
            let row = modifier_row(count, repetition, true);
            append_row(&mut rows, &mut first, format!("modifier-{count}"), row);
        }
        for enabled in [false, true] {
            for empty in [false, true] {
                println!("DEFINITION_DIVERSITY_ROW_START={}", canonical(&serde_json::json!({"row":"source_backed_heterogeneous","enabled":enabled,"bouncer_controller_hand_empty":empty,"repetition":repetition})).unwrap());
                let row = heterogeneous::row(enabled, empty, repetition, true);
                append_row(
                    &mut rows,
                    &mut first,
                    format!("heterogeneous-{enabled}-{empty}"),
                    row,
                );
            }
        }
    }
    let report = serde_json::json!({"schema":"definition-diversity-diagnostic-v1","status":"all_declared_rows_verified","expected_rows":48,"build_id":BUILD_ID,
        "contract":contract,"contract_sha256":sha256(CONTRACT.as_bytes()),"warmups":1,"measured_repetitions":5,
        "rows":rows,"outer_guard_declaration_is_not_OS_proof":true,"authentic256_Oracle_ability_claim":false,
        "legal_pool_expansion":false,"product_latency_or_quality_claim":false});
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    f.write_all(canonical(&report).unwrap().as_bytes()).unwrap();
    f.write_all(b"\n").unwrap();
    f.sync_all().unwrap();
}

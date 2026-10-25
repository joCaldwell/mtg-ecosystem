//! Full old generic SnapshotRef is the byte oracle, not a projection/hash-only
//! comparison. Exercise the complete encoder against the original generic oracle.
use super::*;
use std::io;

fn reference<'a>(game: &'a Game, digest: &'a str) -> SnapshotRef<'a> {
    SnapshotRef {
        snapshot_version: 18,
        build_id: BUILD_ID,
        catalog_digest: &game.prepared.catalog_digest,
        profile_digest: &game.prepared.profile_digest,
        state_sha256: digest,
        state: &game.state,
        trace: &game.trace,
    }
}
fn exact(game: &Game) -> String {
    let generic_state = canonical(&game.state).unwrap();
    let digest = sha256(generic_state.as_bytes());
    assert_eq!(digest, game.state_digest().unwrap());
    let snapshot = reference(game, &digest);
    let old = canonical(&snapshot).unwrap();
    let new = encode(&snapshot).unwrap();
    assert_eq!(new.as_bytes(), old.as_bytes());
    let mut written = Vec::new();
    let info = game.write_snapshot(&mut written).unwrap();
    assert_eq!(written, old.as_bytes());
    assert_eq!(info.bytes, old.len() as u64);
    assert_eq!(info.snapshot_sha256, sha256(old.as_bytes()));
    assert_eq!(info.state_sha256, digest);
    let (read, read_digest) = Game::read_snapshot_from_reader(
        game.prepared.as_ref().clone(),
        &mut std::io::Cursor::new(&written),
    )
    .unwrap();
    assert_eq!(read.state, game.state);
    assert_eq!(read.trace, game.trace);
    assert_eq!(read_digest, digest);

    assert_eq!(game.snapshot_with_digest().unwrap(), (old.clone(), digest));
    let decoded: Snapshot = strict_read(&new).unwrap();
    assert_eq!(decoded.state, game.state);
    assert_eq!(decoded.trace, game.trace);
    assert_eq!(canonical(&decoded).unwrap(), old);
    new
}
fn historical() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}
fn ready(game: &mut Game) {
    for _ in 0..100_000 {
        match game.advance(Budget::new(1)) {
            Advance::AwaitingDecision(_) => return,
            Advance::Suspended { .. } => (),
            other => panic!("unexpected boundary: {other:?}"),
        }
    }
    panic!("bounded advance did not reach a request")
}

#[test]
fn snapshot_codec_payment_pending_and_each_rule_boundary_restore_and_replay_all_trace_modes() {
    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/d1b2-payment-inputs.json"
    ))
    .unwrap();
    let inputs: Vec<AcceptedInput> =
        serde_json::from_value(raw["accepted_inputs"].clone()).unwrap();
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut game = Game::with_trace(historical(), mode);
        exact(&game);
        for input in &inputs {
            ready(&mut game);
            game.respond(input.player, input.decision_id, input.response.clone())
                .unwrap();
            let json = exact(&game);
            let restored = Game::restore(historical(), &json).unwrap();
            let streamed =
                Game::restore_from_reader(historical(), &mut std::io::Cursor::new(json.as_bytes()))
                    .unwrap();
            assert_eq!(streamed.state, restored.state);
            assert_eq!(streamed.trace, restored.trace);

            assert_eq!(restored.state, game.state);
            assert_eq!(restored.trace, game.trace);
            let mut awaiting = false;
            for _ in 0..100_000 {
                let result = game.advance(Budget::new(1));
                exact(&game); // Includes accepted, suspended and coherent states.
                match result {
                    Advance::Suspended { .. } => (),
                    Advance::AwaitingDecision(_) => {
                        awaiting = true;
                        break;
                    }
                    other => panic!("unexpected payment boundary: {other:?}"),
                }
            }
            assert!(awaiting, "bounded payment continuation did not finish");
        }
        ready(&mut game);
        let replay = Game::replay_with_trace(historical(), &game.replay_journal(), mode).unwrap();
        assert_eq!(replay.state, game.state);
        assert_eq!(replay.trace, game.trace);
        exact(&replay);
    }
}

#[test]
fn snapshot_codec_arbitrary_metadata_escaping_null_trace_and_every_original_history_byte() {
    let mut game = Game::with_trace(historical(), TraceMode::Off);
    let mut view = game.state.visibility[0].history.last().unwrap().to_public();
    // Deliberately untrusted serialization domain, not an admitted/reachable
    // position. Preserve Unicode, controls, signed decimal strings and map keys.
    view.objects[0].oracle_id = "quote\" slash\\ newline\n snowman☃ astral🧪".into();
    view.objects[0].marked_damage = Quantity::from(-17);
    view.commanders.push(visibility::VisibleCommander {
        designation: "numeric-map-domain".into(),
        oracle_id: "serialization-only".into(),
        history: CommanderHistory {
            owner: PlayerId(0),
            color_identity: vec![],
            command_casts: BTreeMap::from([
                (PlayerId(2), Quantity::from(4)),
                (PlayerId(10), Quantity::from(5)),
            ]),
            combat_damage: BTreeMap::from([(PlayerId(3), Quantity::from(21))]),
        },
    });
    for _ in 0..70 {
        game.state.visibility[0].history.push_public(view.clone());
    }
    let digest = game.state_digest().unwrap();
    let mut snapshot = reference(&game, &digest);
    snapshot.build_id = "\"\\\n☃";
    snapshot.catalog_digest = "catalog\u{0000}";
    snapshot.profile_digest = "🧪";
    assert_eq!(encode(&snapshot).unwrap(), canonical(&snapshot).unwrap());
    assert_eq!(encode(&snapshot).unwrap(), canonical(&snapshot).unwrap());
    let value: serde_json::Value = serde_json::from_str(&encode(&snapshot).unwrap()).unwrap();
    assert!(value["trace"]["verification"].is_null());
    assert_eq!(
        value["state"]["visibility"][0]["history"]
            .as_array()
            .unwrap()
            .len(),
        game.state.visibility[0].history.len()
    );
}

struct BrokenSink {
    remaining: usize,
    flush_error: bool,
    zero: bool,
    interrupted: bool,
    bytes: Vec<u8>,
}
impl Write for BrokenSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interrupted {
            self.interrupted = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.remaining == 0 {
            return if self.zero {
                Ok(0)
            } else {
                Err(io::ErrorKind::Other.into())
            };
        }
        let count = bytes.len().min(self.remaining).min(7);
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_error {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}
fn sink(remaining: usize) -> BrokenSink {
    BrokenSink {
        remaining,
        flush_error: false,
        zero: false,
        interrupted: false,
        bytes: vec![],
    }
}
#[test]
fn snapshot_codec_short_writes_interrupted_write_zero_write_and_late_flush_fail_closed() {
    let game = Game::new(historical());
    let digest = game.state_digest().unwrap();
    let snapshot = reference(&game, &digest);
    let old = canonical(&snapshot).unwrap();
    let mut good = sink(usize::MAX);
    good.interrupted = true;
    write(&snapshot, &mut good).unwrap();
    assert_eq!(good.bytes, old.as_bytes());
    for length in [0, 1, 63, old.len() / 2, old.len() - 1] {
        for zero in [false, true] {
            let mut bad = sink(length);
            bad.zero = zero;
            assert!(write(&snapshot, &mut bad).is_err());
            assert!(old.as_bytes().starts_with(&bad.bytes));
            assert!(bad.bytes.len() <= length);
        }
    }
    let mut late = sink(usize::MAX);
    late.flush_error = true;
    assert!(write(&snapshot, &mut late).is_err());
    assert_eq!(late.bytes, old.as_bytes()); // Complete output is not success.
}

#[test]
fn snapshot_codec_unchanged_reader_rejects_full_rehashed_shape_and_state_forgeries() {
    let game = Game::new(historical());
    let value: serde_json::Value = serde_json::from_str(&exact(&game)).unwrap();
    for key in [
        "snapshot_version",
        "build_id",
        "catalog_digest",
        "profile_digest",
        "state_sha256",
    ] {
        let mut changed = value.clone();
        changed[key] = if key == "snapshot_version" {
            serde_json::json!(17)
        } else {
            serde_json::json!("wrong")
        };
        let raw = canonical(&changed).unwrap();
        assert!(Game::restore(historical(), &raw).is_err());
        assert!(
            Game::restore_from_reader(historical(), &mut std::io::Cursor::new(raw.as_bytes()))
                .is_err()
        );
    }
    for nested_trace in [false, true] {
        let mut unknown = value.clone();
        let object = if nested_trace {
            &mut unknown["trace"]
        } else {
            &mut unknown
        };
        object["unknown_field"] = serde_json::json!({"ignored_by_no_reader": false});
        let raw = canonical(&unknown).unwrap();
        assert!(Game::restore(historical(), &raw).is_err());
        assert!(
            Game::restore_from_reader(historical(), &mut std::io::Cursor::new(raw.as_bytes()))
                .is_err()
        );
    }
    let mut forged: Snapshot = strict_read(&canonical(&value).unwrap()).unwrap();
    forged.state.cards.get_mut(&CardId(0)).unwrap().oracle_id = "unknown".into();
    forged.state_sha256 = digest(&forged.state).unwrap();
    let raw = canonical(&forged).unwrap();
    assert!(Game::restore(historical(), &raw).is_err());
    assert!(
        Game::restore_from_reader(historical(), &mut std::io::Cursor::new(raw.as_bytes())).is_err()
    );
    let duplicate = exact(&game).replacen("{", "{\"snapshot_version\":18,", 1);
    assert!(Game::restore(historical(), &duplicate).is_err());
    assert!(
        Game::restore_from_reader(
            historical(),
            &mut std::io::Cursor::new(duplicate.as_bytes())
        )
        .is_err()
    );
    let trailing = exact(&game) + " trailing";
    assert!(Game::restore(historical(), &trailing).is_err());
    assert!(
        Game::restore_from_reader(historical(), &mut std::io::Cursor::new(trailing.as_bytes()))
            .is_err()
    );
}

#[test]
fn snapshot_codec_queued_departures_terminal_and_reachable_fault_keep_all_trace_bytes() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut game = Game::with_trace(historical(), mode);
        let mut verifier = SnapshotVerifier::new(historical(), mode);
        for player in [PlayerId(1), PlayerId(2)] {
            game.concede(player).unwrap();
            let json = exact(&game);
            assert_eq!(verifier.verify(&json).unwrap().state, game.state);
        }
        for _ in 0..2 {
            let _ = game.advance(Budget::new(1));
            assert_eq!(verifier.verify(&exact(&game)).unwrap().state, game.state);
        }
        game.concede(PlayerId(3)).unwrap();
        assert_eq!(verifier.verify(&exact(&game)).unwrap().state, game.state);
        assert_eq!(
            game.advance(Budget::new(1)),
            Advance::Finished {
                winner: PlayerId(0)
            }
        );
        let json = exact(&game);
        assert_eq!(verifier.verify(&json).unwrap().state, game.state);
        assert_eq!(
            Game::restore(historical(), &json).unwrap().trace,
            game.trace
        );

        let mut game = Game::with_trace(historical(), mode);
        let mut verifier = SnapshotVerifier::new(historical(), mode);
        for _ in 0..4 {
            let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
                panic!("expected priority before reachable unsupported transition")
            };
            game.respond(request.player(), request.decision_id(), Response::Pass)
                .unwrap();
            verifier.verify(&exact(&game)).unwrap();
        }
        let boundary = game.rule_boundary();
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(_)
        ));
        assert!(game.rule_boundary() > boundary);
        let json = exact(&game);
        let restored = verifier.verify(&json).unwrap();
        assert_eq!(restored.state, game.state);
        assert_eq!(restored.trace, game.trace);
    }
}

#[test]
fn snapshot_codec_all_historical_families_and_real_match_setup_preserve_complete_snapshot_bytes() {
    let groups = [
        (
            include_str!("../../../../tests/fixtures/expected-ir.json"),
            vec![
                ScenarioConfig::land_return(),
                ScenarioConfig::commander_foundation(),
                ScenarioConfig::stack_response(),
                ScenarioConfig::casting_reversal(),
                ScenarioConfig::turns_permanents(),
                ScenarioConfig::combat_game(),
                ScenarioConfig::combat_interactions(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-commander-ir.json"),
            vec![
                ScenarioConfig::command_casting(),
                ScenarioConfig::commander_lifecycle(),
                ScenarioConfig::commander_exile(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-damage-ir.json"),
            vec![
                ScenarioConfig::commander_damage(),
                ScenarioConfig::commander_damage_borrowed(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-library-ir.json"),
            vec![ScenarioConfig::commander_library()],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-action-ir.json"),
            vec![
                ScenarioConfig::actions(),
                ScenarioConfig::actions_borrowed(),
                ScenarioConfig::actions_empty_library(),
                ScenarioConfig::mixed_mana(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-stack-ir.json"),
            vec![
                ScenarioConfig::stack_costs(),
                ScenarioConfig::stack_costs_borrowed(),
                ScenarioConfig::stack_costs_empty_library(),
                ScenarioConfig::stack_costs_library(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-damage-life-ir.json"),
            vec![
                ScenarioConfig::damage_life(),
                ScenarioConfig::damage_life_low(),
                ScenarioConfig::damage_life_borrowed(),
                ScenarioConfig::damage_life_empty_library(),
                ScenarioConfig::damage_life_library(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-trigger-ir.json"),
            vec![
                ScenarioConfig::triggers(),
                ScenarioConfig::triggers_low(),
                ScenarioConfig::triggers_borrowed(),
                ScenarioConfig::triggers_empty_library(),
                ScenarioConfig::triggers_nonactive(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-effects-ir.json"),
            vec![
                ScenarioConfig::effects(),
                ScenarioConfig::effects_furnace(),
                ScenarioConfig::effects_two_furnaces(),
                ScenarioConfig::effects_borrowed(),
                ScenarioConfig::effects_low(),
            ],
        ),
        (
            include_str!("../../../../tests/fixtures/expected-choices-ir.json"),
            vec![
                ScenarioConfig::choices(),
                ScenarioConfig::choices_furnace(),
                ScenarioConfig::conditions(),
                ScenarioConfig::conditions_nonempty(),
                ScenarioConfig::conditions_borrowed(),
                ScenarioConfig::conditions_no_targets(),
            ],
        ),
    ];
    for (catalog, configs) in groups {
        for config in configs {
            for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
                let prepared =
                    Prepared::admit(Catalog::read(catalog).unwrap(), config.clone()).unwrap();
                let game = Game::with_trace(prepared.clone(), mode);
                let json = exact(&game);
                let (decoded, _) = Game::read_snapshot(prepared, &json).unwrap();
                assert_eq!(decoded.state, game.state);
                assert_eq!(decoded.trace, game.trace);
            }
        }
    }
    let prepared = Prepared::admit_pod(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::for_profile("commander-foundations-v1", [21; 32], PlayerId(0)).unwrap(),
    )
    .unwrap();
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut game = Game::with_trace(prepared.clone(), mode);
        exact(&game);
        let _ = game.advance(Budget::new(1));
        exact(&game);
        let _ = game.advance(Budget::unlimited());
        let json = exact(&game);
        let restored = Game::restore(prepared.clone(), &json).unwrap();
        assert_eq!(restored.state, game.state);
        assert_eq!(restored.trace, game.trace);
    }
}

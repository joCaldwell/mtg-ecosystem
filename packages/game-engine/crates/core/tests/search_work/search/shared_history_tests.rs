//! Full old-owned public capture oracle and exact budget/authority boundaries.
use super::*;

fn root() -> Game {
    let catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    let mut g = Game::with_trace(
        Prepared::admit_pod(
            catalog,
            MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(0)).unwrap(),
        )
        .unwrap(),
        TraceMode::Off,
    );
    for _ in 0..100 {
        let Advance::AwaitingDecision(r) = g.advance(Budget::unlimited()) else {
            panic!("opening")
        };
        if matches!(
            r,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && g.turn().step == TurnStep::PrecombatMain
            && g.state.rules.passes.is_empty()
        {
            return g;
        }
        let response = match &r {
            Request::Mulligan {
                player,
                mulligans_taken,
                ..
            } => Response::Mulligan {
                take: player.0 == 0 && *mulligans_taken < 2,
            },
            Request::BottomOpening {
                count, candidates, ..
            } => Response::BottomOpening {
                objects: candidates.iter().rev().take(*count).copied().collect(),
            },
            Request::Priority { .. } => Response::Pass,
            _ => panic!("unexpected opening {r:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("root cap")
}
fn complete(g: &Game) -> SearchCapture {
    match g.capture_search(PlayerId(0), Budget::unlimited()) {
        CaptureOutcome::Complete(c) => c,
        _ => panic!("supported capture"),
    }
}
fn old_history(g: &Game) -> Vec<PermittedView> {
    g.state.knowledge.visibility[0]
        .history
        .iter()
        .map(|view| serde_json::from_value(serde_json::to_value(view).unwrap()).unwrap())
        .collect()
}
#[derive(Serialize)]
struct OwnedCapturedContext {
    observation: Observation,
    permitted_history: Vec<PermittedView>,
}
#[derive(Serialize)]
struct OwnedCapture {
    capture_version: u32,
    profile: PublicSearchProfile,
    context: OwnedCapturedContext,
    own_knowledge: Vec<OwnKnowledgeAction>,
    known_bottom: Vec<RememberedCard>,
    known_hands: Vec<KnownHandMemory>,
}
fn assert_old_capture(g: &Game, c: &SearchCapture) {
    let old = old_history(g);
    let SearchCaptureData {
        capture_version,
        profile,
        context,
        own_knowledge,
        known_bottom,
        known_hands,
    } = c.data().clone();
    let CapturedSearchContext {
        observation,
        permitted_history,
    } = context;
    assert_eq!(permitted_history.iter().collect::<Vec<_>>(), old);
    let expected = OwnedCapture {
        capture_version,
        profile,
        context: OwnedCapturedContext {
            observation,
            permitted_history: old.clone(),
        },
        own_knowledge,
        known_bottom,
        known_hands,
    };
    assert_eq!(canonical(c.data()).unwrap(), canonical(&expected).unwrap());
    let old_digest = digest(&old).unwrap();
    assert_eq!(
        c.context_id(),
        capture::capture_identity(c.data(), &old_digest, true).unwrap()
    );
    assert_eq!(
        c.sampler_identity(),
        capture::capture_identity(c.data(), &old_digest, false).unwrap()
    );
    // Arbitrary DTO decode preserves data only: it is never accepted as a
    // SearchCapture (no Deserialize/public constructor/real origin was added).
    let decoded: SearchCaptureData = strict_read(&canonical(&expected).unwrap()).unwrap();
    assert_eq!(&decoded, c.data());
}
#[test]
fn shared_capture_full_old_owned_wire_hash_and_one_object_unit_partition_are_unchanged() {
    let g = root();
    let before = g.state_digest().unwrap();
    let expected = complete(&g);
    assert_old_capture(&g, &expected);
    assert_eq!(expected.data().known_bottom.len(), 1);
    // Count original public Query units independently through its actual API.
    let mut query = g.search_context(PlayerId(0), "capture-v1".into(), Budget::new(0));
    let mut query_units = 0_u64;
    loop {
        match query {
            QueryResult::Incomplete {
                work,
                required_minimum,
            } => {
                assert_eq!(required_minimum, 1);
                query_units += 1;
                query = g.resume_work(work, Budget::new(1));
            }
            QueryResult::Complete(QueryValue::SearchContext(_)) => break,
            _ => panic!("query"),
        }
    }
    let old = old_history(&g);
    let scan_units = old.iter().map(|v| v.objects.len() as u64 + 1).sum::<u64>();
    let total =
        query_units + g.state.knowledge.visibility[0].own_knowledge.len() as u64 + scan_units + 1;
    for quantum in [1, 64, u64::MAX] {
        let mut outcome = g.capture_search(PlayerId(0), Budget::new(0));
        let mut supplied = 0_u64;
        loop {
            match outcome {
                CaptureOutcome::Incomplete {
                    work,
                    required_minimum,
                } => {
                    assert_eq!(required_minimum, 1);
                    supplied += quantum.min(total - supplied);
                    outcome = g.resume_search_capture(work, Budget::new(quantum));
                }
                CaptureOutcome::Complete(c) => {
                    assert_eq!(supplied, total);
                    assert_eq!(c.data(), expected.data());
                    assert_eq!(c.context_id(), expected.context_id());
                    assert_eq!(c.sampler_identity(), expected.sampler_identity());
                    break;
                }
                CaptureOutcome::Rejected(e) => panic!("{e:?}"),
            }
        }
    }
    assert_eq!(g.state_digest().unwrap(), before);
}
#[test]
fn shared_capture_contains_no_hidden_state_and_survives_original_history_cow() {
    let g = root();
    let c = complete(&g);
    let bytes = canonical(c.data()).unwrap();
    let mut hidden = g.fork();
    // Private information-flow domain: NOT asserted to be a reachable new
    // setup seed or restorable real origin. Existing capture inputs stay equal.
    hidden
        .state
        .rules
        .match_state
        .as_mut()
        .unwrap()
        .random
        .word_position = RngPosition(12345);
    hidden
        .state
        .rules
        .libraries
        .get_mut(&PlayerId(1))
        .unwrap()
        .reverse();
    let paired = complete(&hidden);
    assert_ne!(g.state_digest().unwrap(), hidden.state_digest().unwrap());
    assert_eq!(paired.data(), c.data());
    assert_eq!(paired.context_id(), c.context_id());
    assert_eq!(paired.sampler_identity(), c.sampler_identity());
    assert_old_capture(&hidden, &paired);
    let mut edited = g.fork();
    edited.state.knowledge.visibility[0].history[0].update_public(|v| v.turn.number += 123);
    assert_ne!(complete(&edited).context_id(), c.context_id());
    assert_eq!(canonical(c.data()).unwrap(), bytes);
    drop(edited);
    drop(hidden);
    drop(g);
    assert_eq!(canonical(c.data()).unwrap(), bytes);
}
#[test]
fn shared_capture_still_rejects_foreign_history_viewers_and_stale_or_foreign_work() {
    let mut g = root();
    let CaptureOutcome::Incomplete { work, .. } = g.capture_search(PlayerId(0), Budget::new(0))
    else {
        panic!("lazy")
    };
    let fork = g.fork();
    assert!(matches!(
        fork.resume_search_capture(work, Budget::unlimited()),
        CaptureOutcome::Rejected(SearchRejection::StaleWork)
    ));
    let CaptureOutcome::Incomplete { work, .. } = g.capture_search(PlayerId(0), Budget::new(0))
    else {
        panic!("lazy")
    };
    g.concede(PlayerId(3)).unwrap();
    assert!(matches!(
        g.resume_search_capture(work, Budget::unlimited()),
        CaptureOutcome::Rejected(SearchRejection::StaleWork)
    ));
    let mut bad = root();
    bad.state.knowledge.visibility[0].history[0].update_public(|v| v.viewer = PlayerId(3));
    assert!(matches!(
        bad.capture_search(PlayerId(0), Budget::unlimited()),
        CaptureOutcome::Rejected(SearchRejection::UnsupportedHistory(_))
    ));
}

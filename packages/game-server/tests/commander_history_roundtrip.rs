//! Natural paid Commander cast and combat publications cross every private codec.
use mtg_contracts::{Catalog, Color, canonical};
use mtg_core::{MatchConfig, Observation, PlayerId, PlayerRequest, PlayerResponse, Zone};
use mtg_server::{
    codec,
    ipc::{self, ReplyFrame, WorkerReply},
    protocol::{CommandBody, CommandEnvelope, Counter, Publication, StreamPage, SubmissionResult},
    worker::{Worker, WorkerPhase},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn settle(worker: &mut Worker) {
    for _ in 0..1000 {
        if !matches!(worker.host_state().phase, WorkerPhase::Running) {
            return;
        }
        worker.advance(Instant::now()).unwrap();
    }
    panic!("finite progress bound");
}
fn latest(worker: &Worker, seat: u8) -> Observation {
    let cursor = worker.host_state().viewer_cursors[usize::from(seat)];
    let page = worker.stream_page(seat, cursor - 1, 1).unwrap();
    let Publication::ViewReplaced { observation } = &page.entries[0].publication else {
        panic!("latest publication is a view");
    };
    observation.as_ref().clone()
}
#[test]
fn naturally_paid_cast_and_combat_histories_survive_store_ipc_and_recovery() {
    let directory = Temporary(std::env::temp_dir().join(format!(
        "mtg-commander-codec-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    let catalog = Catalog::read(include_str!(
        "../../game-engine/tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    let plains = catalog
        .definitions
        .iter()
        .find(|card| card.name == "Plains")
        .unwrap()
        .oracle_id
        .clone();
    let config = MatchConfig {
        decks: std::array::from_fn(|_| {
            mtg_core::match_profile("commander-foundations-v1")
                .unwrap()
                .decks[0]
                .clone()
        }),
        ..MatchConfig::for_profile("commander-foundations-v1", [3; 32], PlayerId(0)).unwrap()
    };
    let commander = config.decks[0].commander.clone();
    let mut worker =
        Worker::create(&directory.0, catalog.clone(), config, "a1", Instant::now()).unwrap();
    let mut reached = false;
    for ordinal in 1..=300 {
        settle(&mut worker);
        let views = (0..4).map(|seat| latest(&worker, seat)).collect::<Vec<_>>();
        if views[0].view.commanders.iter().any(|commander| {
            commander.history.owner == PlayerId(0)
                && !commander.history.command_casts.is_empty()
                && !commander.history.combat_damage.is_empty()
        }) {
            reached = true;
            break;
        }
        let observation = views.iter().find(|view| view.decision.is_some()).unwrap();
        let request = observation.decision.as_ref().unwrap();
        let seat = observation.view.viewer;
        let view = &observation.view;
        let untapped = || {
            view.objects.iter().find(|object| {
                object.zone == Zone::Battlefield
                    && object.controller == seat
                    && object.oracle_id == plains
                    && !object.tapped
            })
        };
        let response = match request {
            PlayerRequest::Mulligan { .. } => PlayerResponse::Mulligan { take: false },
            PlayerRequest::Priority { .. }
                if seat == PlayerId(0)
                    && view.active == Some(seat)
                    && view.stack.is_empty()
                    && matches!(view.turn.step, mtg_core::TurnStep::PrecombatMain) =>
            {
                if view.turn.lands_played == 0 {
                    let land = view.objects.iter().find(|object| {
                        object.zone == (Zone::Hand { owner: seat }) && object.oracle_id == plains
                    });
                    match land {
                        Some(land) => PlayerResponse::PlayLand {
                            object: land.handle.clone(),
                        },
                        None => PlayerResponse::Pass,
                    }
                } else if let Some(card) = view.objects.iter().find(|object| {
                    object.zone == (Zone::Command { owner: seat }) && object.oracle_id == commander
                }) {
                    assert!(untapped().is_some());
                    PlayerResponse::Cast {
                        card: card.handle.clone(),
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Mana { .. } => {
                let mana = &view
                    .players
                    .iter()
                    .find(|player| player.player == seat)
                    .unwrap()
                    .mana;
                if mana.get(&Color::W).is_some_and(|white| white >= &1.into()) {
                    PlayerResponse::FinishMana { undo_mana: true }
                } else {
                    PlayerResponse::ActivateMana {
                        object: untapped().unwrap().handle.clone(),
                    }
                }
            }
            PlayerRequest::SpendMana { .. } => PlayerResponse::SpendMana {
                spending: [(Color::W, 1.into())].into(),
            },
            PlayerRequest::Attackers {
                candidates,
                defenders,
                ..
            } => PlayerResponse::Attackers {
                attacks: if seat == PlayerId(0) {
                    candidates
                        .iter()
                        .map(|handle| {
                            assert!(defenders.contains(&PlayerId(1)));
                            (handle.clone(), PlayerId(1))
                        })
                        .collect()
                } else {
                    vec![]
                },
            },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
            unexpected => panic!("unexpected request {unexpected:?}"),
        };
        let input = CommandEnvelope {
            protocol_version: 1,
            command_id: format!("{ordinal:032x}"),
            expected_revision: Counter::new(
                worker.host_state().viewer_cursors[usize::from(seat.0)],
            ),
            command: CommandBody::Decision {
                token: request.token().clone(),
                response,
            },
        };
        assert!(matches!(
            worker.submit(seat.0, input, Instant::now()).unwrap(),
            SubmissionResult::Accepted { .. }
        ));
    }
    assert!(
        reached,
        "must reach an actual paid cast and combat hit, not a seeded ledger"
    );
    for seat in 0..4 {
        let cursor = worker.host_state().viewer_cursors[usize::from(seat)];
        let page = worker.stream_page(seat, cursor - 1, 1).unwrap();
        let bytes = canonical(&page).unwrap();
        let decoded: StreamPage = codec::exact(&bytes).unwrap();
        assert_eq!(decoded, page);
        assert_eq!(canonical(&decoded).unwrap(), bytes);
        let reply = ReplyFrame {
            ipc_version: ipc::IPC_VERSION,
            request_id: 1,
            reply: WorkerReply::Page {
                page: Box::new(page),
            },
        };
        let wire = ipc::encode(&reply).unwrap();
        assert_eq!(
            ipc::read_frame::<_, ReplyFrame>(&mut wire.as_slice())
                .unwrap()
                .unwrap(),
            reply
        );
    }
    let expected = worker.state_sha256().unwrap();
    let view = latest(&worker, 0);
    drop(worker);
    let recovered = Worker::recover(&directory.0, catalog, "a1", Instant::now()).unwrap();
    assert_eq!(recovered.state_sha256().unwrap(), expected);
    assert_eq!(latest(&recovered, 0), view);
}

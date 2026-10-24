//! Reachable private search, exact attachments identity and durable recovery.
use mtg_contracts::{CardKind, Catalog, Color, canonical};
use mtg_core::{
    ATTACHMENTS_PROFILE, MatchConfig, Observation, PlayerId, PlayerRequest, PlayerResponse, Zone,
};
use mtg_server::{
    protocol::{CommandBody, CommandEnvelope, Counter, Publication, SubmissionResult},
    worker::{Worker, WorkerPhase},
};
use std::{path::PathBuf, time::Instant};
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn latest(worker: &Worker, seat: u8) -> Observation {
    let cursor = worker.host_state().viewer_cursors[usize::from(seat)];
    let page = worker.stream_page(seat, cursor - 1, 1).unwrap();
    let Publication::ViewReplaced { observation } = &page.entries[0].publication else {
        panic!("expected latest view")
    };
    observation.as_ref().clone()
}
#[test]
fn attachments_worker_recovers_private_search_without_other_seat_stream_leaks() {
    let directory = Temporary(
        std::env::temp_dir().join(format!("mtg-attachments-host-{}", std::process::id())),
    );
    let catalog = Catalog::read(include_str!(
        "../../game-engine/tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap();
    // This admitted seed opens with Rampant Growth and two lands including green;
    // the witness reaches the private choice without relying on late library draws.
    let config = MatchConfig::for_profile(ATTACHMENTS_PROFILE, [39; 32], PlayerId(1)).unwrap();
    let mut worker =
        Worker::create(&directory.0, catalog.clone(), config, "a2", Instant::now()).unwrap();
    let mut searched = false;
    for ordinal in 1..=2000 {
        for _ in 0..1000 {
            if !matches!(worker.host_state().phase, WorkerPhase::Running) {
                break;
            }
            worker.advance(Instant::now()).unwrap();
        }
        let views: Vec<_> = (0..4).map(|seat| latest(&worker, seat)).collect();
        for view in &views {
            assert!(
                view.view
                    .objects
                    .iter()
                    .all(|o| !matches!(o.zone, Zone::Library { .. })
                        && !matches!(o.zone, Zone::Hand { owner } if owner != view.view.viewer))
            );
            assert!(
                view.decision
                    .as_ref()
                    .is_none_or(|r| serde_json::to_value(r).unwrap()["player"]
                        == serde_json::json!(view.view.viewer))
            );
        }
        let observation = views
            .iter()
            .find(|o| o.decision.is_some())
            .expect("reachable decision");
        let request = observation.decision.as_ref().unwrap();
        let seat = observation.view.viewer;
        let view = &observation.view;
        if let PlayerRequest::SearchLands { candidates, .. } = request {
            assert_eq!(seat, PlayerId(1));
            assert!(!candidates.is_empty());
            for other in [0, 2, 3] {
                let mut cursor = 0;
                loop {
                    let page = worker.stream_page(other, cursor, 32).unwrap();
                    let bytes = canonical(&page).unwrap();
                    assert!(!bytes.contains("\"request\":\"search_lands\""));
                    assert!(!bytes.contains(&request.token().0));
                    for candidate in candidates {
                        assert!(!bytes.contains(&candidate.handle.0));
                    }
                    if !page.has_more {
                        break;
                    }
                    cursor = page.next.get();
                }
            }
            let hash = worker.state_sha256().unwrap();
            let host = worker.host_state().clone();
            drop(worker);
            worker = Worker::recover(&directory.0, catalog.clone(), "a2", Instant::now()).unwrap();
            assert_eq!(worker.state_sha256().unwrap(), hash);
            assert_eq!(worker.host_state(), &host);
            for (seat, expected) in views.iter().enumerate() {
                assert_eq!(latest(&worker, seat as u8), *expected);
            }
            searched = true;
        }
        let request = observation.decision.as_ref().unwrap();
        let lands = || {
            view.objects
                .iter()
                .filter(|o| {
                    o.zone == Zone::Battlefield
                        && o.controller == seat
                        && !o.tapped
                        && catalog.definitions.iter().any(|d| {
                            d.oracle_id == o.oracle_id && d.characteristics.kind == CardKind::Land
                        })
                })
                .collect::<Vec<_>>()
        };
        let response = match request {
            PlayerRequest::Mulligan { .. } => PlayerResponse::Mulligan { take: false },
            PlayerRequest::Priority { .. }
                if seat == PlayerId(1)
                    && view.active == Some(seat)
                    && view.stack.is_empty()
                    && view.turn.step == mtg_core::TurnStep::PrecombatMain =>
            {
                if view.turn.lands_played == 0 {
                    view.objects
                        .iter()
                        .find(|o| {
                            o.zone == (Zone::Hand { owner: seat })
                                && catalog.definitions.iter().any(|d| {
                                    d.oracle_id == o.oracle_id
                                        && d.characteristics.kind == CardKind::Land
                                })
                        })
                        .map(|o| PlayerResponse::PlayLand {
                            object: o.handle.clone(),
                        })
                        .unwrap_or(PlayerResponse::Pass)
                } else {
                    view.objects
                        .iter()
                        .find(|o| {
                            o.zone == (Zone::Hand { owner: seat })
                                && catalog.definitions.iter().any(|d| {
                                    d.oracle_id == o.oracle_id
                                        && matches!(d.name.as_str(), "Rampant Growth" | "Cultivate")
                                        && lands().len()
                                            >= if d.name == "Cultivate" { 3 } else { 2 }
                                        && lands().iter().any(|o| {
                                            catalog.definitions.iter().any(|d| {
                                                d.oracle_id == o.oracle_id
                                                    && d.characteristics
                                                        .color_identity
                                                        .contains(&Color::G)
                                            })
                                        })
                                })
                        })
                        .map(|o| PlayerResponse::Cast {
                            card: o.handle.clone(),
                        })
                        .unwrap_or(PlayerResponse::Pass)
                }
            }
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Mana { cost, .. } => {
                let mana = &view.players.iter().find(|p| p.player == seat).unwrap().mana;
                let total: mtg_contracts::Quantity =
                    mana.values().fold(0.into(), |mut n, value| {
                        n.add(value);
                        n
                    });
                let mut needed = cost.generic.clone();
                needed.add(&(cost.colored.len() as i64).into());
                if mana.get(&Color::G).is_some_and(|n| *n >= 1.into()) && total >= needed {
                    PlayerResponse::FinishMana { undo_mana: true }
                } else {
                    let need_green = mana.get(&Color::G).is_none_or(|n| *n < 1.into());
                    let land = lands()
                        .into_iter()
                        .find(|o| {
                            !need_green
                                || catalog.definitions.iter().any(|d| {
                                    d.oracle_id == o.oracle_id
                                        && d.characteristics.color_identity.contains(&Color::G)
                                })
                        })
                        .unwrap();
                    PlayerResponse::ActivateMana {
                        object: land.handle.clone(),
                    }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => {
                let mut spending =
                    std::collections::BTreeMap::<Color, mtg_contracts::Quantity>::from([(
                        Color::G,
                        1.into(),
                    )]);
                let mut rest = cost.generic.clone();
                for c in [Color::B, Color::G] {
                    let mut free = available.get(&c).cloned().unwrap_or_default();
                    if c == Color::G {
                        free.subtract(&1.into());
                    }
                    let amount = free.min(rest.clone());
                    rest.subtract(&amount);
                    if amount > 0.into() {
                        spending.entry(c).or_default().add(&amount);
                    }
                }
                assert_eq!(rest, 0.into());
                PlayerResponse::SpendMana { spending }
            }
            PlayerRequest::SearchLands {
                maximum,
                candidates,
                ..
            } => PlayerResponse::SearchLands {
                objects: candidates
                    .iter()
                    .take(*maximum as usize)
                    .map(|c| c.handle.clone())
                    .collect(),
            },
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
            other => panic!("unexpected request {other:?}"),
        };
        let command = CommandEnvelope {
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
            worker.submit(seat.0, command, Instant::now()).unwrap(),
            SubmissionResult::Accepted { .. }
        ));
        if searched {
            // Resolve the private search and verify the post-choice round trip too.
            for _ in 0..1000 {
                if !matches!(worker.host_state().phase, WorkerPhase::Running) {
                    break;
                }
                worker.advance(Instant::now()).unwrap();
            }
            let hash = worker.state_sha256().unwrap();
            drop(worker);
            let recovered = Worker::recover(&directory.0, catalog, "a2", Instant::now()).unwrap();
            assert_eq!(recovered.state_sha256().unwrap(), hash);
            break;
        }
    }
    assert!(searched, "must reach a real search from admitted inputs");
}

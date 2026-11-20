//! Exact frozen Cultivate/Reach, paid setup and private decisions. No profile admission.
//! CR 121.5, 401.2, 701.20a-b, 701.23b, 701.24a/e:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

// Private source witness only: replace the source in an otherwise legal seeded
// 100-card setup BEFORE constructing Game. This does not expand a reviewed pool
// or call the new catalog admitted. Every subsequent change is an accepted input.
fn ramos_split_prepared(raw: &str) -> Prepared {
    let mut p = search_prepared("Rampant Growth");
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        raw,
    ) else {
        panic!("complete frozen split source must lower")
    };
    let old = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Rampant Growth")
        .unwrap()
        .oracle_id
        .clone();
    let new = definition.oracle_id.clone();
    let mut catalog = p.catalog.as_ref().clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    for card in &mut p.config.cards {
        if card.oracle_id == old {
            card.oracle_id = new.clone();
        }
    }
    for deck in &mut p.match_config.as_mut().unwrap().decks {
        for card in &mut deck.cards {
            if card.oracle_id == old {
                card.oracle_id = new.clone();
            }
        }
        deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    }
    p.config.profile = "ramos-m2-private-source-witness".into();
    p.catalog_digest = digest(&catalog).unwrap();
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
    p.catalog = Arc::new(catalog);
    p
}
const RAMOS_SPLIT_SOURCES: [&str; 2] = [
    include_str!("../../../../tests/fixtures/ramos/8b755881-a72d-4e21-a369-d2924eb4585a.json"),
    include_str!("../../../../tests/fixtures/ramos/1593ea18-2f2f-4ab4-83fb-6ccc0bec8a90.json"),
];
fn ramos_split_search(g: &mut Game, name: &str) -> Request {
    let spell = develop_search(g, name, 3);
    announce(g, spell.0, vec![]);
    let spell = g.state.rules.stack.last().unwrap();
    assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
    for _ in 0..20 {
        let r = request(g);
        if matches!(r, Request::SearchLands { .. }) {
            return r;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("split search not reached")
}
fn ramos_split_observation_recovery(g: &Game, viewer: PlayerId) {
    let before = g.state.clone();
    let QueryResult::Incomplete { work, .. } = g.observe(viewer, Budget::new(1)) else {
        panic!("bounded observation")
    };
    let saved = g.save_work(&work).unwrap();
    let mut wrong: serde_json::Value = serde_json::from_str(&saved).unwrap();
    wrong["build_id"] = "old-build".into();
    assert!(
        g.restore_work(&serde_json::to_string(&wrong).unwrap())
            .is_err()
    );
    let work = g.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Observation(restored)) =
        g.resume_work(work, Budget::unlimited())
    else {
        panic!("recovered observation")
    };
    assert_eq!(restored, observation(g, viewer.0));
    assert_eq!(g.state, before);
}
fn ramos_split_validate(g: &Game, response: PlayerResponse) -> ValidatedResponse {
    let own = observation(g, 0);
    let before = g.state.clone();
    let trace = g.trace.clone();
    let token = own.decision.unwrap().token().clone();
    let QueryResult::Incomplete { work, .. } =
        g.check_player_response(PlayerId(0), token, response, Budget::new(1))
    else {
        panic!("bounded validation should suspend")
    };
    let saved = g.save_work(&work).unwrap();
    let work = g.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Validated(validated)) =
        g.resume_work(work, Budget::unlimited())
    else {
        panic!("recovered split validation")
    };
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    validated
}
#[test]
fn ramos_split_paid_zero_one_two_identity_reveal_privacy_restore_replay() {
    for (raw, name) in RAMOS_SPLIT_SOURCES
        .into_iter()
        .zip(["Cultivate", "Kodama's Reach"])
    {
        let p = ramos_split_prepared(raw);
        for count in 0..=2 {
            // Both selection input orders select the SAME battlefield identity.
            let ordered = std::cell::RefCell::new(None);
            for &reverse in if count == 2 {
                &[false, true][..]
            } else {
                &[false][..]
            } {
                traces(p.clone(), |g| {
                    let r = ramos_split_search(g, name);
                    let Request::SearchLands {
                        candidates,
                        maximum,
                        ..
                    } = &r
                    else {
                        unreachable!()
                    };
                    assert_eq!(*maximum, 2);
                    check(g);
                    let own = observation(g, 0);
                    let Some(PlayerRequest::SearchLands {
                        candidates: visible,
                        ..
                    }) = own.decision
                    else {
                        panic!("search projection")
                    };
                    let mut alternate = g.fork();
                    alternate
                        .state
                        .rules
                        .libraries
                        .get_mut(&PlayerId(0))
                        .unwrap()
                        .reverse();
                    for seat in 1..4 {
                        assert_eq!(observation(g, seat), observation(&alternate, seat));
                        assert!(observation(g, seat).decision.is_none());
                        assert!(
                            observation(g, seat)
                                .view
                                .objects
                                .iter()
                                .all(|o| !matches!(o.zone, Zone::Library { .. }))
                        );
                        assert!(
                            g.resolve_search_handle(PlayerId(seat), &visible[0].handle)
                                .is_err()
                        );
                    }
                    assert_eq!(
                        g.search_lands(PlayerId(0)),
                        alternate.search_lands(PlayerId(0))
                    );
                    let before = g.state.clone();
                    for (actor, objects) in [
                        (PlayerId(1), vec![]),
                        (PlayerId(0), vec![candidates[0]; 2]),
                        (PlayerId(0), candidates[..3].to_vec()),
                        (PlayerId(0), vec![g.search_lands(PlayerId(1))[0]]),
                        (
                            PlayerId(0),
                            vec![g.state.rules.stack.last().unwrap().source],
                        ),
                        (
                            PlayerId(0),
                            vec![ObjectId {
                                card: candidates[0].card,
                                incarnation: candidates[0].incarnation + 1,
                            }],
                        ),
                    ] {
                        assert!(
                            g.respond(actor, r.decision_id(), Response::SearchLands { objects })
                                .is_err()
                        );
                        assert_eq!(g.state, before);
                    }
                    let token = observation(g, 0).decision.unwrap().token().clone();
                    assert!(matches!(
                        g.check_player_response(
                            PlayerId(0),
                            token,
                            PlayerResponse::SearchLands {
                                objects: vec![visible[0].handle.clone(); 2]
                            },
                            Budget::unlimited()
                        ),
                        QueryResult::Rejected(_)
                    ));
                    let mut nonbasic = g.fork();
                    nonbasic
                        .state
                        .rules
                        .cards
                        .get_mut(&candidates[0].card)
                        .unwrap()
                        .oracle_id = YARGLE.into();
                    for seat in 1..4 {
                        assert_eq!(observation(g, seat), observation(&nonbasic, seat));
                    }
                    assert!(
                        nonbasic
                            .check_response(
                                PlayerId(0),
                                r.decision_id(),
                                Response::SearchLands {
                                    objects: vec![candidates[0]]
                                }
                            )
                            .is_err()
                    );
                    let found = candidates[..count].to_vec();
                    let mut handles = visible[..count]
                        .iter()
                        .map(|c| c.handle.clone())
                        .collect::<Vec<_>>();
                    if reverse {
                        handles.reverse();
                    }
                    let receipt = ramos_split_validate(
                        g,
                        PlayerResponse::SearchLands {
                            objects: handles.clone(),
                        },
                    );
                    g.submit(receipt).unwrap();
                    let initial_library = g.state.rules.libraries[&PlayerId(0)].len();
                    // Exactly one semantic step reveals without moving anything.
                    assert!(matches!(
                        g.advance(Budget::new(1)),
                        Advance::Suspended { .. } | Advance::AwaitingDecision(_)
                    ));
                    assert!(matches!(
                        g.state.rules.frame,
                        Frame::ResolutionChoice {
                            choice: ResolutionChoice::SearchSplit { .. },
                            ..
                        }
                    ));
                    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), initial_library);
                    for seat in 0..4 {
                        let view = observation(g, seat);
                        assert_eq!(
                            view.view
                                .objects
                                .iter()
                                .filter(|o| matches!(o.zone, Zone::Library { .. }))
                                .count(),
                            count
                        );
                        if seat != 0 {
                            assert!(view.decision.is_none());
                        }
                    }
                    check(g);
                    for seat in 0..4 {
                        ramos_split_observation_recovery(g, PlayerId(seat));
                    }
                    if count == 2 {
                        let mut forged = g.fork();
                        let Frame::ResolutionChoice {
                            choice: ResolutionChoice::SearchSplit { objects, .. },
                            ..
                        } = &mut forged.state.rules.frame
                        else {
                            unreachable!()
                        };
                        objects[1] = objects[0];
                        assert!(
                            Game::restore(g.prepared.as_ref().clone(), &forged.snapshot().unwrap())
                                .is_err()
                        );
                    }
                    for h in &handles {
                        assert!(g.resolve_search_handle(PlayerId(0), h).is_err());
                    }
                    if count == 2 {
                        let r = request(g);
                        assert!(matches!(r, Request::SearchLandBattlefield { .. }));
                        let invalid = candidates[2];
                        for (actor, object) in [(PlayerId(0), invalid), (PlayerId(1), found[0])] {
                            assert!(
                                g.check_response(
                                    actor,
                                    r.decision_id(),
                                    Response::SearchLandBattlefield { object }
                                )
                                .is_err()
                            );
                        }
                        let Some(PlayerRequest::SearchLandBattlefield {
                            candidates: shown, ..
                        }) = observation(g, 0).decision
                        else {
                            panic!("partition projection")
                        };
                        assert_eq!(shown.len(), 2);
                        // Pick found[1], deliberately independent of canonical/input order.
                        let handle = g.handle_for(PlayerId(0), found[1]).unwrap();
                        let receipt = ramos_split_validate(
                            g,
                            PlayerResponse::SearchLandBattlefield { object: handle },
                        );
                        g.submit(receipt).unwrap();
                    } else {
                        assert!(g.actions_request().unwrap().is_none());
                    }
                    assert!(matches!(
                        g.advance(Budget::new(1)),
                        Advance::Suspended { .. }
                    ));
                    assert!(matches!(
                        g.state.rules.frame,
                        Frame::ResolutionChoice {
                            choice: ResolutionChoice::Shuffle { .. },
                            ..
                        }
                    ));
                    check(g);
                    for (i, id) in found.iter().enumerate() {
                        let object = g.current(g.object_id(id.card).unwrap()).unwrap();
                        let battlefield = count == 1 || i == 1;
                        assert_eq!(
                            object.zone,
                            if battlefield {
                                Zone::Battlefield
                            } else {
                                Zone::Hand { owner: PlayerId(0) }
                            }
                        );
                        assert_eq!(object.tapped, battlefield);
                    }
                    // One-word shuffle suspension and retained reveal history.
                    let _ = g.advance(Budget::new(1));
                    check(g);
                    resolve(g, vec![]);
                    assert_eq!(
                        g.state.rules.libraries[&PlayerId(0)].len(),
                        initial_library - count
                    );
                    for seat in 0..4 {
                        assert_eq!(observation(g, seat).view.players[0].library_shuffles, 1);
                        assert!(
                            observation(g, seat)
                                .view
                                .objects
                                .iter()
                                .all(|o| !matches!(o.zone, Zone::Library { .. }))
                        );
                        assert!(
                            g.state.knowledge.visibility[seat as usize]
                                .history
                                .iter()
                                .any(|v| {
                                    v.to_public()
                                        .objects
                                        .iter()
                                        .filter(|o| matches!(o.zone, Zone::Library { .. }))
                                        .count()
                                        == count
                                })
                        );
                    }
                    // The library return path emits no draw outcome for the hand card.
                    assert_eq!(g.state.rules.stack.len(), 0);
                    if let Some(previous) = ordered.borrow().as_ref() {
                        assert!(previous == &g.state.rules);
                    }
                    *ordered.borrow_mut() = Some(g.state.rules.clone());
                    eprintln!(
                        "ramos_m2 name={name:?} selected={count} reverse={reverse} trace={:?} build={} boundary={} journal={} state={}",
                        g.trace.mode,
                        BUILD_ID,
                        g.rule_boundary(),
                        digest(&g.replay_journal()).unwrap(),
                        g.state_digest().unwrap()
                    );
                });
            }
        }
    }
}
#[test]
fn ramos_split_capability_stays_outside_reviewed_host_search_and_pod() {
    use crate::profile_data::{Feature, ProfileError};
    let p = ramos_split_prepared(RAMOS_SPLIT_SOURCES[0]);
    assert!(
        Prepared::admit_pod(p.catalog.as_ref().clone(), p.match_config.clone().unwrap()).is_err()
    );
    for feature in [Feature::Hosting, Feature::Search] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted = feature == Feature::Hosting;
        profile.search = feature == Feature::Search;
        assert!(
            matches!(profile.validate_requested_features(&p.catalog), Err(ProfileError::UnreviewedCapabilities { capabilities, .. }) if capabilities.contains(&Capability::BasicLandSplitSearchV1))
        );
    }
}

#[test]
fn ramos_split_zero_singleton_remainder_shuffle_controls() {
    // Supplemental synthetic forks only: ordinary movement shrinks a real paid
    // search's library. The natural witnesses above own replay/recovery claims.
    let p = ramos_split_prepared(RAMOS_SPLIT_SOURCES[0]);
    let mut base = Game::new(p);
    let _ = ramos_split_search(&mut base, "Cultivate");
    for count in 0..=2 {
        for remainder in 0..=1 {
            let mut g = base.fork();
            let candidates = g.search_lands(PlayerId(0));
            let Frame::ResolutionChoice { frame, .. } = g.state.rules.frame.clone() else {
                unreachable!()
            };
            let cause = BatchCause::Instruction {
                stack_id: frame.spell.id,
                player: frame.spell.controller,
                source: frame.spell.source,
                cursor: frame.cursor,
                source_block: 0,
            };
            for object in &candidates[count + remainder..] {
                g.move_object_for(
                    *object,
                    Zone::Graveyard { owner: PlayerId(0) },
                    cause.clone(),
                )
                .unwrap();
            }
            g.refresh_visibility().unwrap();
            let random = g
                .state
                .rules
                .match_state
                .as_ref()
                .unwrap()
                .random
                .word_position;
            let r = request(&mut g);
            g.respond(
                r.player(),
                r.decision_id(),
                Response::SearchLands {
                    objects: candidates[..count].to_vec(),
                },
            )
            .unwrap();
            if count == 2 {
                let r = request(&mut g);
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::SearchLandBattlefield {
                        object: candidates[0],
                    },
                )
                .unwrap();
            }
            resolve(&mut g, vec![]);
            assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), remainder);
            assert_eq!(
                g.state
                    .rules
                    .match_state
                    .as_ref()
                    .unwrap()
                    .random
                    .word_position,
                random
            );
            assert_eq!(observation(&g, 0).view.players[0].library_shuffles, 1);
        }
    }
}

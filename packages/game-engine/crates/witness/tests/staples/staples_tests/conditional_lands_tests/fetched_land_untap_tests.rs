use super::*;

const SOURCE: &str =
    include_str!("../../../../../../tests/fixtures/0c85b8f7-0bd0-4680-9ec5-d4b110460a54.json");

#[test]
fn fetched_land_untap_uses_exact_successor_after_shuffle_and_counts_controller_lands() {
    let name = lower(SOURCE).name;
    let prepared = super::super::typed_land_search_tests::typed_prepared(SOURCE);
    for (count, find) in [(2, true), (3, true), (3, false)] {
        let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        let card = develop_search(&mut game, &name, count);
        play_fetch_in_next_main(&mut game, card);
        let source = game.object_id(card).unwrap();
        assert_eq!(game.zone(source), Some(Zone::Battlefield));
        let object = game.handle_for(PlayerId(0), source).unwrap();
        public(
            &mut game,
            PlayerResponse::ActivateAbility { object, ability: 0 },
        );
        let mut decision = request(&mut game);
        let activation = game.state.rules.stack.last().unwrap();
        assert_eq!(activation.paid, Some(activation.total_cost.clone()));
        assert!(activation.source_facts.as_ref().unwrap().tapped);
        assert_eq!(
            game.zone(game.object_id(source.card).unwrap()),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        for _ in 0..4 {
            assert!(matches!(decision, Request::Priority { .. }));
            game.respond(decision.player(), decision.decision_id(), Response::Pass)
                .unwrap();
            decision = request(&mut game);
        }
        assert!(matches!(decision, Request::SearchLands { maximum: 1, .. }));
        let visible = observation(&game, 0);
        let PlayerRequest::SearchLands { candidates, .. } = visible.decision.unwrap() else {
            panic!()
        };
        let handles = if find {
            vec![candidates[0].handle.clone()]
        } else {
            vec![]
        };
        let previous = handles
            .first()
            .map(|handle| game.resolve_search_handle(PlayerId(0), handle).unwrap());
        let prior_shuffles = visible.view.players[0].library_shuffles;
        public(&mut game, PlayerResponse::SearchLands { objects: handles });
        let _ = game.advance(Budget::new(1));
        let Frame::ResolutionChoice {
            choice: ResolutionChoice::Shuffle { fetched_land, .. },
            ..
        } = &game.state.rules.frame
        else {
            panic!("missing shuffle continuation")
        };
        assert_eq!(fetched_land.as_ref().map(|entry| entry.previous), previous);
        let successor = fetched_land.as_ref().map(|entry| entry.successor);
        if let Some(land) = successor {
            assert!(game.current(land).unwrap().tapped);
        }
        check(&game);
        if count == 3 && find {
            let mut forged: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
            let Frame::ResolutionChoice {
                choice: ResolutionChoice::Shuffle { fetched_land, .. },
                ..
            } = &mut forged.state.rules.frame
            else {
                panic!("missing saved shuffle")
            };
            *fetched_land = None;
            forged.state_sha256 = state_digest::state_digest(&forged.state).unwrap();
            assert!(Game::restore(prepared.clone(), &canonical(&forged).unwrap()).is_err());
        }
        let _ = request(&mut game);
        assert_eq!(
            observation(&game, 0).view.players[0].library_shuffles,
            prior_shuffles + 1
        );
        if let Some(land) = successor {
            assert_eq!(game.current(land).unwrap().tapped, count < 3);
        }
        let untaps = game
            .trace
            .records
            .iter()
            .filter_map(|record| {
                let EvidenceRecord::Batch { batch } = &record.record else {
                    return None;
                };
                match batch.result(&[]) {
                    Some(EventOutcome::Untapped { object }) => Some(*object),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            untaps,
            if count == 3 {
                successor.into_iter().collect::<Vec<_>>()
            } else {
                vec![]
            }
        );
        check(&game);
    }
}

fn play_fetch_in_next_main(game: &mut Game, card: CardId) {
    for _ in 0..1000 {
        let decision = request(game);
        if matches!(
            decision,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && game.active_player() == Some(PlayerId(0))
            && matches!(
                game.turn().step,
                TurnStep::PrecombatMain | TurnStep::PostcombatMain
            )
            && game.turn().lands_played == 0
        {
            let object = game
                .handle_for(PlayerId(0), game.object_id(card).unwrap())
                .unwrap();
            public(game, PlayerResponse::PlayLand { object });
            let _ = request(game);
            return;
        }
        let response = match &decision {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates
                    .iter()
                    .copied()
                    .filter(|object| object.card != card)
                    .take(*count)
                    .collect(),
            },
            _ => panic!("develop fetch setup: {decision:?}"),
        };
        game.respond(decision.player(), decision.decision_id(), response)
            .unwrap();
    }
    panic!("fetch land play setup bound");
}

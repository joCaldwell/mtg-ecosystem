//! CR 608.2b/d, 701.9a–b and 903.9a. The target chooses from their own hand.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn discard_setup() -> Prepared {
    let mut rows = vec![
        ("Sign in Blood", 0, 0, true),
        ("Forest", 1, 1, true),
        ("Swamp", 1, 1, true),
        ("Caelorna, Coral Tyrant", 1, 1, true),
        ("Island", 2, 2, true),
        ("Plains", 0, 0, false),
        ("Island", 0, 0, false),
        ("Swamp", 0, 0, false),
        ("Swamp", 0, 0, false),
    ];
    let mut prepared = if let Some(path) = std::env::var_os("MTG_TARGETED_DISCARD_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen archive probe");
        let charm = archive
            .lines()
            .find(|line| {
                mtg_contracts::sha256(line.as_bytes())
                    == "8aaa516e9f4cefbfe05ae874227692c942670e83c3a7501bdb2fb8821c860510"
            })
            .expect("pinned complete Esper Charm row");
        rows[0].0 = "Esper Charm";
        prepared_with_sources(&rows, &[charm])
    } else {
        let mut prepared = prepared_with_sources(&rows, &[]);
        let catalog = Arc::make_mut(&mut prepared.catalog);
        let spell = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Sign in Blood")
            .unwrap();
        spell.name = "Synthetic Opponent Discard".into();
        spell.oracle_id = "synthetic-opponent-discard".into();
        spell.source_labels = None;
        let program = spell.spell.as_mut().unwrap();
        program.targets[0].domain = TargetDomain::OpponentPlayer;
        program.instructions = vec![Instruction::DiscardTargetPlayer {
            target_slot: 0,
            count: 2.into(),
            source_block: 0,
        }];
        spell.capabilities = spell.requirements();
        prepared.config.cards[0].oracle_id = spell.oracle_id.clone();
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        prepared.catalog_digest = digest(catalog).unwrap();
        prepared
    };
    prepared.config.commanders = vec![CardId(3)];
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

pub(super) fn public_answer(g: &mut Game, seat: PlayerId, response: PlayerResponse) {
    let r = request(g, 1);
    assert_eq!(r.player(), seat);
    let QueryValue::Decision(Some(choice)) =
        finish_query(g, g.player_decision_for(seat, Budget::unlimited()))
    else {
        panic!("missing public choice for {seat:?}");
    };
    let QueryValue::Validated(receipt) = finish_query(
        g,
        g.check_player_response(seat, choice.token().clone(), response, Budget::unlimited()),
    ) else {
        panic!("invalid public response");
    };
    g.submit(receipt).unwrap();
}

#[test]
fn targeted_discard_paid_choice_clamp_privacy_departure_and_restore() {
    let caster = PlayerId(0);
    let prepared = discard_setup();
    for target in [PlayerId(1), PlayerId(2)] {
        let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        let hand = game.action_hand(target);
        let modal = !game
            .definition(CardId(0))
            .unwrap()
            .spell
            .as_ref()
            .unwrap()
            .modes
            .is_empty();
        let card = game.handle_for(caster, object(&game, 0)).unwrap();
        public_answer(&mut game, caster, PlayerResponse::Cast { card });
        if modal {
            public_answer(&mut game, caster, PlayerResponse::Modes { mode: 2 });
        }
        let Request::PlayerTargets { candidates, .. } = request(&mut game, 1) else {
            panic!("announce the discard target");
        };
        assert_eq!(candidates.contains(&caster), modal);
        public_answer(
            &mut game,
            caster,
            PlayerResponse::PlayerTargets {
                players: vec![target],
            },
        );
        let lands: &[(u32, Color)] = if modal {
            &[(5, Color::W), (6, Color::U), (7, Color::B)]
        } else {
            &[(7, Color::B), (8, Color::B)]
        };
        let mut spending = BTreeMap::new();
        for &(id, color) in lands {
            let object = game.handle_for(caster, object(&game, id)).unwrap();
            public_answer(&mut game, caster, PlayerResponse::ActivateMana { object });
            spending
                .entry(color)
                .or_insert_with(Quantity::zero)
                .add(&1.into());
        }
        public_answer(
            &mut game,
            caster,
            PlayerResponse::FinishMana { undo_mana: false },
        );
        public_answer(&mut game, caster, PlayerResponse::SpendMana { spending });
        assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
        let spell = game.top().unwrap().clone();
        assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
        assert_eq!(spell.player_targets, [target]);
        let source_block = game.program_for(&spell).unwrap().instructions[0].source_block();

        if target == PlayerId(2) {
            let mut departed = game.fork();
            departed.concede(target).unwrap();
            resolve(&mut departed, 1, |_| {});
            assert!(departed.events().iter().any(|e| matches!(e,
                Event::SpellCompleted { stack_id, resolved: false } if *stack_id == spell.id)));
        }

        for seat in game.live_players() {
            public_answer(&mut game, seat, PlayerResponse::Pass);
        }
        assert!(matches!(
            request(&mut game, 1),
            Request::ResolveDiscard { .. }
        ));
        let QueryValue::Decision(Some(PlayerRequest::ResolveDiscard {
            token,
            candidates,
            count,
            ..
        })) = finish_query(&game, game.player_decision_for(target, Budget::unlimited()))
        else {
            panic!("target chooses discards");
        };
        assert_eq!(count, 2.min(hand.len()));
        assert_eq!(candidates.len(), hand.len());
        for old in &hand {
            assert!(candidates.contains(&game.handle_for(target, *old).unwrap()));
        }
        for viewer in game.live_players().into_iter().filter(|s| *s != target) {
            let QueryValue::Observation(other) =
                finish_query(&game, game.observe(viewer, Budget::unlimited()))
            else {
                panic!("public observation");
            };
            assert!(other.decision.is_none());
            assert!(
                other
                    .view
                    .objects
                    .iter()
                    .all(|o| o.zone != (Zone::Hand { owner: target }))
            );
        }
        recover(&game);
        let selected = &hand[hand.len() - count..];
        let response = PlayerResponse::ResolveDiscard {
            objects: selected
                .iter()
                .map(|o| game.handle_for(target, *o).unwrap())
                .collect(),
        };
        assert!(matches!(
            game.check_player_response(caster, token, response.clone(), Budget::unlimited()),
            QueryResult::Rejected(_)
        ));
        public_answer(&mut game, target, response);
        recover(&game);
        if target == PlayerId(1) {
            let mut departed = game.fork();
            departed.concede(target).unwrap();
            resolve(&mut departed, 1, |_| {});
            assert!(
                batches(&departed)
                    .iter()
                    .all(|b| !matches!(b.cause, BatchCause::Instruction { .. }))
            );

            let r = request(&mut game, 1);
            assert!(matches!(r, Request::CommanderReturn { player, object, .. }
                if player == target && object.card == CardId(3)));
            assert!(game.state.rules.stack.is_empty());
            assert_eq!(
                game.zone(object(&game, 3)),
                Some(Zone::Graveyard { owner: target })
            );
            public_answer(
                &mut game,
                target,
                PlayerResponse::CommanderReturn { to_command: true },
            );
        }
        resolve(&mut game, 1, |_| {});
        assert_eq!(game.action_hand(target), hand[..hand.len() - count]);
        let effects: Vec<_> = batches(&game)
            .into_iter()
            .filter(|b| matches!(b.cause, BatchCause::Instruction { .. }))
            .collect();
        let [batch] = effects.as_slice() else {
            panic!("one simultaneous discard batch")
        };
        assert_eq!(
            batch.cause,
            BatchCause::Instruction {
                stack_id: spell.id,
                player: caster,
                source: spell.source,
                cursor: 0,
                source_block
            }
        );
        assert!(matches!(&batch.group, EventGroup::Simultaneous { .. }));
        assert_eq!(batch.occurrences.len(), count);
        for (&old, occurrence) in selected.iter().zip(&batch.occurrences) {
            let EventOutcome::CardDiscarded { player, movement } = &occurrence.actual else {
                panic!("discard event")
            };
            assert_eq!(*player, target);
            assert_eq!(movement.previous, old);
            assert_eq!(movement.destination, Zone::Graveyard { owner: target });
            let destination = if old.card == CardId(3) {
                Zone::Command { owner: target }
            } else {
                movement.destination
            };
            assert_eq!(
                game.zone(game.object_id(old.card).unwrap()),
                Some(destination)
            );
        }
        recover(&game);
    }
}

//! Frozen row 29420 (a7440993…) reparsed using the authorized parser derivative.
//! The paid witness stops before the reveal-and-cast instruction runs.
use super::controlled_creature_spell_copy_tests::respond_to_spell;
use super::targeted_discard_tests::public_answer;
use super::*;

const CASTER: PlayerId = PlayerId(0);

fn pod() -> Prepared {
    let archive = std::fs::read_to_string(
        std::env::var_os("MTG_CREATIVE_TECHNIQUE_ARCHIVE_PROBE")
            .expect("supply the exact Creative Technique archive"),
    )
    .unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "86c721c13151e4ed8f82be4154f0202e8fbd337a3454902f45c33979bc7f7a9e"
    );
    let mut lines = archive.lines();
    let manifest = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(lines.next().is_none());
    assert_eq!(
        sha256(row.as_bytes()),
        "e7f74eeafcd6a4ba0a23cdeb1f02cc3b6bbeb643b83bdef426f949904639f49b"
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(manifest, row)
    else {
        panic!("complete Creative Technique source must lower");
    };
    assert_eq!(definition.name, "Creative Technique");
    let mut rows = vec![("Counterspell", 0, 0, true), ("Counterspell", 1, 1, true)];
    lands(&mut rows, "Mountain", 0, 5);
    lands(&mut rows, "Island", 1, 2);
    let mut pod = prepared(&rows);
    pod.config.profile = "private-demonstrate".into();
    pod.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut pod.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();
    pod
}

fn pass_round(g: &mut Game) {
    for _ in g.live_players() {
        assert!(matches!(request(g, 1), Request::Priority { .. }));
        answer(g, 1, Response::Pass);
    }
}

fn counter(g: &mut Game, target: StackId) {
    until_player(g, 1, PlayerId(1));
    respond_to_spell(
        g,
        CardId(1),
        target,
        &[("Island", Color::U), ("Island", Color::U)],
    );
    pass_round(g);
    assert!(matches!(request(g, 1), Request::Priority { .. }));
    assert!(g.state.rules.stack.iter().all(|s| s.id != target));
}

#[test]
#[ignore = "requires the pinned one-record Creative Technique archive"]
fn demonstrate_paid_copy() {
    let pod = pod();
    let mut paid = Game::with_trace(pod.clone(), TraceMode::Detailed);
    announce(&mut paid, 1, 0, vec![]);
    let [original, trigger] = paid.state.rules.stack.as_slice() else {
        panic!("self-cast trigger above spell")
    };
    let original = original.clone();
    let trigger = trigger.clone();
    assert_eq!(original.paid.as_ref(), Some(&original.total_cost));
    assert!(matches!(trigger.kind, StackKind::Triggered { .. }));

    for source_countered in [false, true] {
        let mut game = Game::restore(pod.clone(), &paid.snapshot().unwrap()).unwrap();
        if source_countered {
            counter(&mut game, original.id);
        }
        let casts = game.state.rules.cast_history.clone();
        pass_round(&mut game);
        assert!(matches!(
            request(&mut game, 1),
            Request::OptionalEffect { player: CASTER, .. }
        ));
        let mut declined = Game::restore(pod.clone(), &game.snapshot().unwrap()).unwrap();
        answer(
            &mut declined,
            1,
            Response::OptionalEffect { perform: false },
        );
        assert!(matches!(
            request(&mut declined, 1),
            Request::Priority { .. }
        ));
        assert!(declined.state.rules.tokens.is_empty());
        assert_eq!(
            declined.state.rules.stack.len(),
            usize::from(!source_countered)
        );

        answer(&mut game, 1, Response::OptionalEffect { perform: true });
        let Request::ChooseOpponent {
            player,
            decision_id,
            candidates,
        } = request(&mut game, 1)
        else {
            panic!("opponent choice immediately after first copy")
        };
        assert_eq!(
            (player, candidates),
            (CASTER, vec![PlayerId(1), PlayerId(2), PlayerId(3)])
        );
        let first = game.top().unwrap().clone();
        assert!(game.is_spell_copy(&first));
        assert_eq!(game.state.rules.stack.iter().rev().nth(1), Some(&trigger));
        assert!(
            game.respond(
                player,
                decision_id,
                Response::ChooseOpponent { opponent: CASTER }
            )
            .is_err()
        );
        recover(&game);

        // A concession invalidates an already accepted opponent without redoing
        // the first copy or granting priority between the two placements.
        let mut departed = Game::restore(pod.clone(), &game.snapshot().unwrap()).unwrap();
        answer(
            &mut departed,
            1,
            Response::ChooseOpponent {
                opponent: PlayerId(1),
            },
        );
        departed.concede(PlayerId(1)).unwrap();
        assert!(
            matches!(request(&mut departed, 1), Request::ChooseOpponent { ref candidates, .. } if *candidates == [PlayerId(2), PlayerId(3)])
        );
        assert_eq!(departed.top().unwrap(), &first);
        recover(&departed);

        public_answer(
            &mut game,
            CASTER,
            PlayerResponse::ChooseOpponent {
                opponent: PlayerId(3),
            },
        );
        recover(&game);
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        assert!(
            matches!(&game.state.rules.frame, Frame::Resolve { frame } if frame.stack_id == trigger.id)
        );
        let second = game.top().unwrap().clone();
        recover(&game);
        assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
        let expected: Vec<_> = (!source_countered)
            .then_some(original.id)
            .into_iter()
            .chain([first.id, second.id])
            .collect();
        assert_eq!(
            game.state
                .rules
                .stack
                .iter()
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            expected
        );
        assert_ne!(first.source.card, second.source.card);
        for (copy, owner) in [(&first, CASTER), (&second, PlayerId(3))] {
            assert!(game.is_spell_copy(copy));
            assert_ne!(copy.source.card, original.source.card);
            assert_eq!(copy.controller, owner);
            assert_eq!(game.state.rules.cards[&copy.source.card].owner, owner);
            assert_eq!(
                game.definition(copy.source.card),
                game.definition(original.source.card)
            );
            assert!(copy.paid.is_none() && copy.cost_lock.is_none());
            assert_eq!(copy.alternative_cost, original.alternative_cost);
        }
        assert_eq!(game.state.rules.cast_history, casts);
        assert_eq!(
            game.events()
                .iter()
                .filter(|e| matches!(e, Event::SpellCast { .. }))
                .count(),
            1 + usize::from(source_countered)
        );
        assert_eq!(
            game.events()
                .iter()
                .filter(
                    |e| matches!(e, Event::SpellCopied { target, .. } if *target == original.id)
                )
                .count(),
            2
        );
        if !source_countered {
            counter(&mut game, second.id);
            assert_eq!(game.top().unwrap(), &first);
            assert!(game.object_id(second.source.card).is_none());
            assert!(batches(&game).iter().flat_map(|b| &b.occurrences).any(|leaf| matches!(&leaf.actual, EventOutcome::SpellCopyCeased {object} if object.card == second.source.card)));
        }
        game.validate_state().unwrap();
    }
}

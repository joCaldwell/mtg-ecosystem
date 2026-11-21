use super::*;
use mtg_witness::equipped::{answer, recover};

include!("../../../../../tests/fixtures/synthetic_artifact_loss.rs");

fn prepared_artifact() -> Prepared {
    let mut p = Prepared::admit(
        Catalog::read(include_str!(
            "../../../../../tests/fixtures/expected-ir.json"
        ))
        .unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap();
    let catalog = Arc::make_mut(&mut p.catalog);
    let d = catalog
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == SPELL)
        .unwrap();
    set_up_synthetic_artifact_loss(d);
    p.config.cards[1].oracle_id = d.oracle_id.clone();
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}

#[test]
fn paid_artifact_loss_uses_sacrificed_source_and_selected_player() {
    let mut p = prepared_artifact();
    let mut cast = Game::with_trace(p.clone(), TraceMode::Detailed);
    let land = cast.object_id(CardId(0)).unwrap();
    answer(&mut cast, 1, Response::ActivateMana { object: land });
    answer(&mut cast, 1, Response::Cast { card: CardId(1) });
    answer(&mut cast, 1, Response::FinishMana { undo_mana: false });
    cast.complete_test_mana();
    for _ in 0..4 {
        answer(&mut cast, 1, Response::Pass);
    }
    assert!(matches!(
        cast.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
    let entered = cast.object_id(CardId(1)).unwrap();
    assert_eq!(cast.current(entered).unwrap().zone, Zone::Battlefield);
    assert!(cast.current(entered).unwrap().tapped);
    assert!(cast.validate_activation(PlayerId(0), entered, 0).is_err());
    recover(&cast);
    p.config.cards[1].zone = Zone::Battlefield;
    p.config.cards[1].owner = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let object = g.object_id(CardId(1)).unwrap();
    let island = g.object_id(CardId(0)).unwrap();
    answer(&mut g, 1, Response::ActivateAbility { object, ability: 0 });
    recover(&g);
    let victim = PlayerId(2);
    answer(
        &mut g,
        1,
        Response::PlayerTargets {
            players: vec![victim],
        },
    );
    answer(&mut g, 1, Response::ActivateMana { object: island });
    answer(&mut g, 1, Response::FinishMana { undo_mana: false });
    g.complete_test_mana();
    assert!(g.current(object).is_err());
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    let invoked = g.top().unwrap();
    assert_eq!(invoked.source, object);
    assert_eq!(invoked.paid.as_ref(), Some(&invoked.total_cost));
    assert_eq!(invoked.player_targets, [victim]);
    recover(&g);
    for _ in 0..4 {
        answer(&mut g, 1, Response::Pass);
    }
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
    assert!(!g.is_live(victim));
    assert!(g.events().iter().any(|e|matches!(e,Event::PlayersLost{losses} if losses==&[(victim,PlayerLossReason::TargetedEffect)])));
    recover(&g);
}

#[test]
fn targeted_game_loss_binds_paid_activation_departure_and_terminal_result() {
    for (target, terminal) in [
        (PlayerId(1), false),
        (PlayerId(0), false),
        (PlayerId(1), true),
        (PlayerId(0), true),
    ] {
        let mut setup = prepared_artifact();
        setup.config.cards[1].zone = Zone::Battlefield;
        setup.profile_digest = digest(&setup.config).unwrap();
        let mut g = Game::with_trace(setup, TraceMode::Detailed);
        if terminal {
            g.concede(PlayerId(2)).unwrap();
            g.concede(PlayerId(3)).unwrap();
        }
        let source = g.object_id(CardId(0)).unwrap();
        answer(&mut g, 1, Response::ActivateMana { object: source });
        let object = g.object_id(CardId(1)).unwrap();
        answer(&mut g, 1, Response::ActivateAbility { object, ability: 0 });
        answer(
            &mut g,
            1,
            Response::PlayerTargets {
                players: vec![target],
            },
        );
        answer(&mut g, 1, Response::FinishMana { undo_mana: false });
        g.complete_test_mana();
        for _ in 0..g.live_players().len() {
            answer(&mut g, 1, Response::Pass);
        }
        for _ in 0..100 {
            if matches!(g.state.rules.frame, Frame::Resolve { .. }) {
                break;
            }
            assert!(matches!(
                g.advance(Budget::new(1)),
                Advance::Suspended { .. }
            ));
        }
        if !terminal && target == PlayerId(1) {
            let mut departed = g.fork();
            departed.concede(target).unwrap();
            assert!(matches!(
                departed.advance(Budget::unlimited()),
                Advance::AwaitingDecision(_)
            ));
            assert!(!departed.events().iter().any(|event| matches!(event,
                Event::PlayersLost { losses } if losses.iter().any(|(_,reason)| *reason == PlayerLossReason::TargetedEffect))));
            recover(&departed);
        }
        let (cause, victim) = g.targeted_loss_authority().unwrap();
        assert_eq!(victim, target);
        let wrong = PlayerId(if target == PlayerId(0) { 1 } else { 0 });
        let lose = |player| ProposedEvent::Lose {
            player,
            reason: PlayerLossReason::TargetedEffect,
        };
        let before = g.state.clone();
        assert!(g.commit(lose(wrong), cause.clone()).is_err());
        assert!(g.commit(lose(target), BatchCause::StateBased).is_err());
        assert_eq!(g.state, before);
        let end = g.advance(Budget::unlimited());
        if terminal {
            assert_eq!(end, Advance::Finished { winner: wrong });
        } else {
            assert!(matches!(end, Advance::AwaitingDecision(_)));
            assert!(g.state.rules.stack.is_empty());
            if target == PlayerId(0) {
                assert!(g.object_id(CardId(1)).is_none());
            } else {
                assert_eq!(
                    g.state.rules.objects[&CardId(1)].zone,
                    Zone::Graveyard { owner: PlayerId(0) }
                );
                // Seat one owned this permanent, although seat two controlled it.
                assert!(g.object_id(CardId(2)).is_none());
            }
        }
        assert_eq!(
            g.state.rules.players[target.0 as usize]
                .last_known
                .as_ref()
                .unwrap()
                .life,
            Quantity::from(40)
        );
        assert!(
            g.events()
                .iter()
                .any(|event| matches!(event,Event::PlayersLost{losses}
            if losses == &[(target,PlayerLossReason::TargetedEffect)]))
        );
        recover(&g);
    }
}

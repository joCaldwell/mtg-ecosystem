//! Complete Kutzil source: only casting is prohibited. Private kernel tests
//! label unadmitted control-transition setups; no fake card definition is used.
use super::grouped_damage_trigger_tests::{
    CROW, FOREST, GROWTH, KUTZIL, RESEARCH, announce, answer, cast, fund, hand, object, passive,
    prepared, priority, recover, request, resolve, view,
};
use super::*;

const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const MAGE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";

fn public_cast(g: &Game, player: u8, card: u32) -> QueryResult {
    let Some(PlayerRequest::Priority { token, .. }) = view(g, player).decision else {
        panic!("public priority");
    };
    let card = g.handle_for(PlayerId(player), object(g, card)).unwrap();
    g.check_player_response(
        PlayerId(player),
        token,
        PlayerResponse::Cast { card },
        Budget::unlimited(),
    )
}

#[test]
fn cast_prohibition_blocks_opponent_public_instant_flash_and_command_zone_without_mutation() {
    let mut g = Game::new(prepared(&[
        (KUTZIL, 2, 0, Zone::Battlefield),
        (UNSUMMON, 1, 1, Zone::Hand { owner: PlayerId(1) }),
        (RESEARCH, 1, 1, Zone::Hand { owner: PlayerId(1) }),
        (ISAMARU, 1, 1, Zone::Command { owner: PlayerId(1) }),
        (CROW, 0, 0, Zone::Battlefield),
    ]));
    priority(&mut g, 1);
    assert!(g.validate_cast_permission(PlayerId(1)).is_err());
    let before = g.state.clone();
    let trace = g.trace.clone();
    for card in [1, 2, 3] {
        assert!(matches!(public_cast(&g, 1, card), QueryResult::Rejected(_)));
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        let r = request(&mut g);
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Cast { card: CardId(card) }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
    }
    // The command-zone case also obeys ordinary nonactive sorcery timing;
    // the shared zone-independent permission check above proves the new ban.
    recover(&g);
}

#[test]
fn cast_prohibition_allows_own_public_spell_land_and_opponent_mana_and_paid_ability() {
    let mut g = Game::new(prepared(&[
        (KUTZIL, 2, 0, Zone::Battlefield),
        (GROWTH, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        (CROW, 0, 0, Zone::Battlefield),
        (FOREST, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        (HEALER, 1, 1, Zone::Battlefield),
        (MAGE, 1, 1, Zone::Battlefield),
    ]));
    let _ = request(&mut g);
    let land = object(&g, 3);
    answer(&mut g, Response::PlayLand { card: land.card });
    let _ = request(&mut g);
    assert_eq!(g.current(object(&g, 3)).unwrap().zone, Zone::Battlefield);
    let QueryResult::Complete(QueryValue::Validated(receipt)) = public_cast(&g, 0, 1) else {
        panic!("own public cast must remain legal");
    };
    g.submit(receipt).unwrap();
    let crow = object(&g, 2);
    answer(
        &mut g,
        Response::Targets {
            objects: vec![crow],
        },
    );
    fund(&mut g);
    resolve(&mut g);
    assert_eq!(g.characteristics(crow).unwrap().power, Some(4.into()));
    priority(&mut g, 1);
    let land = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            o.zone == Zone::Battlefield
                && o.controller == PlayerId(1)
                && !o.tapped
                && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
        })
        .unwrap()
        .id;
    let Some(PlayerRequest::Priority { token, .. }) = view(&g, 1).decision else {
        panic!("priority");
    };
    let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
        PlayerId(1),
        token,
        PlayerResponse::ActivateMana {
            object: g.handle_for(PlayerId(1), land).unwrap(),
        },
        Budget::unlimited(),
    ) else {
        panic!("opponent mana activation");
    };
    g.submit(receipt).unwrap();
    let _ = request(&mut g);
    assert!(g.current(land).unwrap().tapped);
    let healer = object(&g, 4);
    answer(
        &mut g,
        Response::ActivateAbility {
            object: healer,
            ability: 0,
        },
    );
    answer(
        &mut g,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player {
                player: PlayerId(1),
            }],
        },
    );
    resolve(&mut g);
    priority(&mut g, 1);
    let mage = object(&g, 5);
    answer(
        &mut g,
        Response::ActivateAbility {
            object: mage,
            ability: 0,
        },
    );
    fund(&mut g);
    resolve(&mut g);
    assert_eq!(hand(&g, 1), 1);
    recover(&g);
}

#[test]
fn cast_prohibition_other_turn_and_source_owner_departure_release_future_casts() {
    let rows = [
        (KUTZIL, 2, 0, Zone::Battlefield),
        (UNSUMMON, 1, 1, Zone::Hand { owner: PlayerId(1) }),
        (CROW, 0, 0, Zone::Battlefield),
    ];
    for leave_owner in [false, true] {
        let mut g = Game::new(prepared(&rows));
        if leave_owner {
            priority(&mut g, 1);
            assert!(g.validate_cast_permission(PlayerId(1)).is_err());
            g.concede(PlayerId(2)).unwrap();
            let _ = request(&mut g);
            assert!(g.object_id(CardId(0)).is_none());
            priority(&mut g, 1);
        } else {
            for _ in 0..500 {
                let r = request(&mut g);
                if g.turn().number == 2
                    && g.turn().step == TurnStep::PrecombatMain
                    && r.player() == PlayerId(1)
                {
                    break;
                }
                passive(&mut g, r);
            }
            assert_eq!(g.turn().number, 2);
            assert_eq!(g.turn().step, TurnStep::PrecombatMain);
        }
        g.validate_cast_permission(PlayerId(1)).unwrap();
        let crow = object(&g, 2);
        cast(&mut g, 1, &[crow]);
        assert_eq!(
            g.state.rules.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(0) }
        );
        assert!(g.state.rules.stack.is_empty());
        recover(&g);
    }
}

#[test]
fn cast_prohibition_private_kernel_new_controller_does_not_counter_preexisting_spell() {
    // The admitted pool has no control-changing spell or flash enabler for
    // Kutzil. Only the current-controller transition below is a private kernel
    // setup. The prior Unsummon cast is paid normally; no replay/admission claim
    // is made for the synthetic control transition or the following segment.
    let mut g = Game::new(prepared(&[
        (KUTZIL, 2, 1, Zone::Battlefield),
        (UNSUMMON, 1, 1, Zone::Hand { owner: PlayerId(1) }),
        (CROW, 0, 0, Zone::Battlefield),
        (RESEARCH, 1, 1, Zone::Hand { owner: PlayerId(1) }),
    ]));
    priority(&mut g, 1);
    let crow = object(&g, 2);
    announce(&mut g, 1, &[crow]);
    assert_eq!(g.state.rules.stack.len(), 1);
    recover(&g);
    g.state
        .rules
        .objects
        .get_mut(&CardId(0))
        .unwrap()
        .controller = PlayerId(0);
    assert!(g.validate_cast_permission(PlayerId(1)).is_err());
    assert!(matches!(public_cast(&g, 1, 3), QueryResult::Rejected(_)));
    resolve(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(2)].zone,
        Zone::Hand { owner: PlayerId(0) }
    );
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert!(g.state.rules.stack.is_empty());
}

#[test]
fn cast_prohibition_private_kernel_departure_checks_proposal_but_preserves_payment() {
    // No control-changing spell is admitted here. This private grant fixture
    // exercises the real departure/reversion path, not source-card admission
    // or accepted-input replay for the synthetic grant. CR 601.2e/601.6 checks
    // legality after proposal, not again while paying costs (601.2f-h).
    for after_legality in [false, true] {
        let mut g = Game::new(prepared(&[
            (KUTZIL, 3, 0, Zone::Battlefield),
            (UNSUMMON, 1, 1, Zone::Hand { owner: PlayerId(1) }),
            (CROW, 0, 0, Zone::Battlefield),
            (HEALER, 2, 2, Zone::Battlefield),
        ]));
        let kutzil = object(&g, 0);
        let source = object(&g, 3);
        g.state
            .rules
            .objects
            .get_mut(&CardId(0))
            .unwrap()
            .controller = PlayerId(1);
        g.state.rules.control_effects.push(ControlEffect {
            id: 1,
            object: kutzil,
            controller: PlayerId(1),
            source: ControlSource::Static { object: source },
        });
        priority(&mut g, 1);
        g.validate_cast_permission(PlayerId(1)).unwrap();
        let original = object(&g, 1);
        let crow = object(&g, 2);
        answer(&mut g, Response::Cast { card: CardId(1) });
        let r = request(&mut g);
        assert!(matches!(r, Request::Targets { .. }));
        if after_legality {
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Targets {
                    objects: vec![crow],
                },
            )
            .unwrap();
            assert!(matches!(request(&mut g), Request::Mana { .. }));
        }
        g.concede(PlayerId(2)).unwrap();
        let r = request(&mut g);
        assert_eq!(g.current(kutzil).unwrap().controller, PlayerId(0));
        assert!(g.validate_cast_permission(PlayerId(1)).is_err());
        assert!(g.current(source).is_err());
        assert!(g.state.rules.control_effects.is_empty());
        if after_legality {
            assert!(matches!(r, Request::Mana { .. }));
            assert_eq!(g.state.rules.stack.len(), 1);
            fund(&mut g);
            assert!(g.top().unwrap().paid.is_some());
            resolve(&mut g);
            assert_eq!(
                g.state.rules.objects[&CardId(2)].zone,
                Zone::Hand { owner: PlayerId(0) }
            );
            assert_eq!(
                g.state.rules.objects[&CardId(1)].zone,
                Zone::Graveyard { owner: PlayerId(1) }
            );
        } else {
            assert!(matches!(r, Request::Priority { .. }));
            assert_eq!(object(&g, 1), original);
            assert_eq!(
                g.current(original).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
            assert_eq!(g.current(crow).unwrap().zone, Zone::Battlefield);
        }
        assert!(g.state.rules.stack.is_empty());
        assert!(g.state.rules.pending.is_none());
    }
}

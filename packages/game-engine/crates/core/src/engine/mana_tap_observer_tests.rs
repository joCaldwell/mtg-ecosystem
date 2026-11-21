use super::aura_control_tests::prepared_with_sources;
use super::equipped_indestructible_tests::{
    answer, both, object, pay_to_priority, recover, request,
};
use super::*;

fn prepared() -> Prepared {
    let mut p = prepared_with_sources(
        &[
            ("Grizzly Bears", 1, 1, false),
            ("Grizzly Bears", 2, 2, false),
            ("Forest", 0, 0, false),
            ("Grizzly Bears", 0, 0, true),
            ("Sol Ring", 0, 0, false),
        ],
        &[],
    );
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-phase1-r3.json"),
        include_str!(
            "../../../../tests/fixtures/phase1-r3/0f1afedd-c60f-454f-b84a-c8117aec0128.json"
        ),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    for c in &mut p.config.cards[..2] {
        c.oracle_id = definition.oracle_id.clone();
    }
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}

#[test]
fn slice6c_tap_observers_wait_for_a_paid_activation_and_reverse_with_its_mana() {
    for (pay, undo) in [(false, true), (false, false), (true, false)] {
        both(prepared(), |g, q| {
            let forest = object(g, 2);
            let ring = object(g, 4);
            answer(g, q, Response::Cast { card: CardId(3) });
            if pay {
                answer(g, q, Response::ActivateMana { object: ring });
                assert!(matches!(request(g, q), Request::Mana { .. }));
                assert!(
                    g.state.rules.pending_triggers.is_empty(),
                    "Sol Ring is not a land"
                );
            }
            answer(g, q, Response::ActivateMana { object: forest });
            assert!(matches!(request(g, q), Request::Mana { .. }));
            assert_eq!(g.state.rules.pending_triggers.len(), 2);
            assert!(g.state.rules.pending_triggers.iter().all(|t| matches!(&t.witness,
                TriggerEventWitness::ManaTapped { land, player: PlayerId(0) } if land.object == forest)));
            assert_eq!(
                g.state.rules.stack.len(),
                1,
                "the paid mana window never places triggers"
            );
            recover(g);
            if pay {
                pay_to_priority(g, q, vec![]);
            } else {
                answer(g, q, Response::FinishMana { undo_mana: undo });
                assert!(matches!(request(g, q), Request::Priority { .. }));
            }
            assert_eq!(g.current(forest).unwrap().tapped, !undo);
            if !undo {
                let stack = &g.state.rules.stack;
                assert_eq!(stack[stack.len() - 2].controller, PlayerId(1));
                assert_eq!(stack[stack.len() - 1].controller, PlayerId(2));
            }
            for _ in 0..12 {
                if g.state.rules.stack.is_empty() {
                    break;
                }
                answer(g, q, Response::Pass);
                request(g, q);
            }
            assert!(g.state.rules.stack.is_empty());
            let p = &g.state.rules.players;
            assert_eq!(p[0].life, if undo { 40.into() } else { 38.into() });
            assert_eq!(p[1].life, 40.into());
            assert_eq!(p[2].life, 40.into());
        });
    }
}

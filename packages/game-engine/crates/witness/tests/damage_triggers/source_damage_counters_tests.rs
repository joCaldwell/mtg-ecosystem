use super::*;
use mtg_witness::aura::prepared_with_sources;
use mtg_witness::equipped::{answer, both, object, pay_to_priority, recover, request};

const MARAUDERS: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/f819b134-24cd-4863-a46a-0a068e34ca9e.json");

fn prepared() -> Prepared {
    let mut p = prepared_with_sources(
        &[
            ("Grizzly Bears", 0, 0, false),
            ("Unsummon", 0, 0, true),
            ("Island", 0, 0, false),
        ],
        &[],
    );
    let result = mtg_witness::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-phase1-r3.json"),
        MARAUDERS,
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
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
fn slice6d_combat_damage_counters_stay_bound_to_the_exact_source() {
    for bounce in [false, true] {
        both(prepared(), |g, q| {
            let attacker = object(g, 0);
            for _ in 0..100 {
                let r = request(g, q);
                let response = match r {
                    Request::Attackers { .. } => Response::Attackers {
                        attacks: vec![(attacker, PlayerId(1))],
                    },
                    Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                    Request::Priority { .. } if g.state.rules.stack.is_empty() => Response::Pass,
                    Request::Priority { .. } => break,
                    _ => panic!("unexpected request {r:?}"),
                };
                answer(g, q, response);
            }
            let trigger = g.top().unwrap();
            assert!(matches!(
                &trigger.kind,
                StackKind::Triggered {
                    witness: TriggerEventWitness::DamageDealt {
                        kind: DamageKind::Combat,
                        ..
                    },
                    ..
                }
            ));
            assert_eq!(trigger.source, attacker);
            recover(g);
            if bounce {
                answer(g, q, Response::Cast { card: CardId(1) });
                pay_to_priority(g, q, vec![TargetEntity::Object { object: attacker }]);
            }
            for _ in 0..12 {
                if g.state.rules.stack.is_empty() {
                    break;
                }
                answer(g, q, Response::Pass);
                request(g, q);
            }
            assert!(g.state.rules.stack.is_empty());
            let after = g.current(object(g, 0)).unwrap();
            if bounce {
                assert_ne!(after.id, attacker);
                assert_eq!(after.zone, Zone::Hand { owner: PlayerId(0) });
                assert_eq!(after.plus_one_counters, Quantity::zero());
            } else {
                assert_eq!(after.id, attacker);
                assert_eq!(after.plus_one_counters, Quantity::from(2));
            }
        });
    }
}

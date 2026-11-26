//! Paid casts and resolution helpers for equipment, keywords and permanents.
use mtg_core::testkit::*;
use std::collections::BTreeMap;

pub fn exercise(p: Prepared, run: impl FnOnce(&mut Game, u64)) {
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    Driver::request(&mut g, u64::MAX);
    run(&mut g, u64::MAX);
}
pub fn cast(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let obj = Driver::object(g, id);
    Driver::answer(g, q, g.next_test_decision().cast(obj.card));
    pay_and_resolve(g, q, targets);
}
pub fn pay_to_priority(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
    loop {
        let r = Driver::request(g, q);
        let response = match &r {
            Decision {
                reason: Reason::Targets { .. },
                ..
            } => r.entity_targets(targets.clone()),
            Decision {
                shape: Shape::Pay { cost, .. },
                reason: Reason::Mana { .. },
                ..
            } => {
                let enough = g.payment_pool(r.player, cost).is_ok();
                if enough {
                    Answer::Pay {
                        pay: PayAnswer::Finish {
                            undo_on_failure: false,
                        },
                    }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .unwrap();
                    r.activate_mana(land.id)
                }
            }
            Decision {
                shape:
                    Shape::Pay {
                        cost,
                        pool:
                            ManaPool {
                                unrestricted: available,
                                ..
                            },
                        ..
                    },
                reason: Reason::SpendMana { .. },
                ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Answer::Pay {
                    pay: PayAnswer::Spend {
                        unrestricted: spending,
                    },
                }
            }
            Decision {
                reason: Reason::Priority,
                ..
            } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player, r.id, response).unwrap();
    }
}
pub fn pay_and_resolve(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
    pay_to_priority(g, q, targets);
    while !g.state.rules.stack.is_empty() {
        Driver::answer(
            g,
            q,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
        Driver::request(g, q);
    }
}

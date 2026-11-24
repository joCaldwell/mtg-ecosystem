//! Temporary-indestructible drivers: targets, priority passes and turn steps.

use mtg_core::testkit::*;
use std::collections::BTreeMap;

pub fn pass(r: &Decision<Rules>) -> Answer {
    match r.clone() {
        Decision {
            reason: Reason::Priority,
            ..
        } => Answer::Pick {
            picks: vec![vec![0]],
        },
        Decision {
            reason: Reason::Attackers,
            ..
        } => r.attack(vec![]),
        Decision {
            reason: Reason::Blockers,
            ..
        } => r.block(vec![]),
        found @ Decision {
            reason: Reason::Discard,
            ..
        } => {
            let candidates = found.objects_offered();
            let count = &found.required_count();
            r.pick_objects(candidates.iter().take(*count).copied().collect())
        }
        x => panic!("turn {x:?}"),
    }
}
pub fn announce(g: &mut Game, q: u64, id: u32, t: Vec<TargetEntity>) {
    Driver::answer(g, q, g.next_test_decision().cast(CardId(id)));
    loop {
        let r = Driver::request(g, q);
        let response = match &r {
            Decision {
                reason: Reason::Targets { .. },
                ..
            } => r.targets(
                t.iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object"),
                    })
                    .collect(),
            ),
            Decision {
                reason: Reason::Targets { .. },
                ..
            } => r.entity_targets(t.clone()),
            Decision {
                shape: Shape::Pay { cost: cost, .. },
                reason: Reason::Mana { .. },
                ..
            } => {
                if g.payment_pool(r.player, cost).is_ok() {
                    Answer::Pay {
                        pay: PayAnswer::Finish {
                            undo_on_failure: false,
                        },
                    }
                } else {
                    let o = g
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
                    r.activate_mana(o.id)
                }
            }
            Decision {
                shape:
                    Shape::Pay {
                        cost: cost,
                        pool:
                            ManaPool {
                                unrestricted: available,
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
            found @ Decision {
                reason: Reason::OrderTriggers,
                ..
            } => {
                let candidates = found
                    .candidates
                    .iter()
                    .filter_map(|c| {
                        if let Candidate::Trigger { trigger } = c {
                            Some(trigger.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                Answer::Order {
                    order: (candidates.clone())
                        .into_iter()
                        .map(|trigger| r.test_index(Candidate::Trigger { trigger }))
                        .collect(),
                }
            }
            Decision {
                reason: Reason::Priority,
                ..
            } => return,
            x => panic!("pay {x:?}"),
        };
        g.respond(r.player, r.id, response).unwrap();
    }
}

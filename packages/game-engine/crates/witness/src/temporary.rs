//! Temporary-indestructible drivers: targets, priority passes and turn steps.

use mtg_core::testkit::*;
use std::collections::BTreeMap;

pub fn pass(r: &Request) -> Response {
    match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::Discard {
            candidates, count, ..
        } => Response::Discard {
            objects: candidates.iter().take(*count).copied().collect(),
        },
        x => panic!("turn {x:?}"),
    }
}
pub fn announce(g: &mut Game, q: u64, id: u32, t: Vec<TargetEntity>) {
    Driver::answer(g, q, Response::Cast { card: CardId(id) });
    loop {
        let r = Driver::request(g, q);
        let response = match &r {
            Request::Targets { .. } => Response::Targets {
                objects: t
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object"),
                    })
                    .collect(),
            },
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: t.clone(),
            },
            Request::Mana { cost, .. } => {
                if g.payment_pool(r.player(), cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let o = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .unwrap();
                    Response::ActivateMana { object: o.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
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
                Response::SpendMana { spending }
            }
            Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                order: candidates.clone(),
            },
            Request::Priority { .. } => return,
            x => panic!("pay {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}

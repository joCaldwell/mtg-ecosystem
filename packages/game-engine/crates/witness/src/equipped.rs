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
    Driver::answer(g, q, Response::Cast { card: obj.card });
    pay_and_resolve(g, q, targets);
}
pub fn pay_to_priority(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
    loop {
        let r = Driver::request(g, q);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: targets.clone(),
            },
            Request::Targets { .. } => Response::Targets {
                objects: targets
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object target"),
                    })
                    .collect(),
            },
            Request::Mana { cost, .. } => {
                let enough = g.payment_pool(r.player(), cost).is_ok();
                if enough {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
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
                    Response::ActivateMana { object: land.id }
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
            Request::Priority { .. } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}
pub fn pay_and_resolve(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
    pay_to_priority(g, q, targets);
    while !g.state.rules.stack.is_empty() {
        Driver::answer(g, q, Response::Pass);
        Driver::request(g, q);
    }
}

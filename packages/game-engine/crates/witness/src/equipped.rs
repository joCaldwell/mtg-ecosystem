//! Equipped-indestructible witness drivers shared by keyword, aura and
//! permanent binaries: source preparation, paid casts and recovery checks.
use mtg_core::testkit::*;
use std::collections::BTreeMap;

pub fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
pub fn request(g: &mut Game, q: u64) -> Request {
    for _ in 0..1000 {
        match g.advance(Budget::new(q)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            x => panic!("{x:?}"),
        }
    }
    panic!("bounded stabilization")
}
pub fn answer(g: &mut Game, q: u64, response: Response) {
    let r = request(g, q);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
pub fn recover(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(g.state, restored.state);
    assert_eq!(g.trace(), restored.trace());
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(g.state, replay.state);
    assert_eq!(g.trace(), replay.trace());
}
pub fn both(p: Prepared, run: impl Fn(&mut Game, u64)) {
    let mut prior = None;
    for (q, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        request(&mut g, q);
        run(&mut g, q);
        recover(&g);
        let hash = g.state_digest().unwrap();
        if let Some(old) = &prior {
            assert_eq!(&hash, old)
        }
        prior = Some(hash);
    }
}
pub fn cast(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let obj = object(g, id);
    answer(g, q, Response::Cast { card: obj.card });
    pay_and_resolve(g, q, targets);
}
pub fn pay_to_priority(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
    loop {
        let r = request(g, q);
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
        answer(g, q, Response::Pass);
        request(g, q);
    }
}

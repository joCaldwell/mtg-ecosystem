//! Private complete-source paid scenarios; no pool or consumer admission.
//! Frozen archive d705222b280a71cb; CR20260925 109.5, 302.6, 303.4,
//! 400.7, 506.4, 608.2b, 608.3c, 613.7e, 702.10/11, 800.4a-c:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_witness::equipped::{answer, object, recover, request};
use mtg_witness::temporary::pass;

fn resolve(g: &mut Game, q: u64, mut inspect: impl FnMut(&Game)) {
    for _ in 0..3000 {
        let a = g.advance(Budget::new(q));
        inspect(g);
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.rules.stack.is_empty()
                    && g.state.rules.pending_triggers.is_empty()
                {
                    return;
                }
                let response = match &r {
                    Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                        order: candidates.clone(),
                    },
                    Request::ResolveDiscard {
                        candidates, count, ..
                    } => Response::ResolveDiscard {
                        objects: candidates.iter().take(*count).copied().collect(),
                    },
                    _ => pass(&r),
                };
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            x => panic!("paid Aura control resolution {x:?}"),
        }
    }
    panic!("bounded paid resolution")
}

fn fund(g: &mut Game, q: u64, name: &str, count: usize) {
    for _ in 0..count {
        let r = request(g, q);
        let source = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.controller == r.player()
                    && o.zone == Zone::Battlefield
                    && !o.tapped
                    && g.definition(o.id.card).unwrap().name == name
            })
            .unwrap()
            .id;
        g.respond(
            r.player(),
            r.decision_id(),
            Response::ActivateMana { object: source },
        )
        .unwrap();
    }
}

mod activated_artifact_draw_tests;
mod random_discard_tests;

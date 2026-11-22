//! Temporary-indestructible drivers: targets, priority passes and turn steps.
use mtg_core::testkit::*;

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

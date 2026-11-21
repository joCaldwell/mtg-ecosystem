use super::*;
use mtg_witness::equipped::{object, prepared};

#[test]
fn graveyard_target_group_requires_caster_and_preserves_physical_incarnations() {
    let mut p = prepared(
        &[("Forest", false), ("Plains", false), ("Island", false)],
        false,
    );
    for (i, c) in p.config.cards.iter_mut().take(3).enumerate() {
        let owner = PlayerId(if i == 2 { 2 } else { 1 });
        c.owner = owner;
        c.controller = owner;
        c.zone = Zone::Graveyard { owner };
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let caster = PlayerId(0);
    let owner = PlayerId(1);
    let who = TargetEntity::Player { player: owner };
    let ids = [object(&g, 0), object(&g, 1), object(&g, 2)];
    let targets = ids.map(|object| TargetEntity::Object { object });
    assert_eq!(
        g.announced_graveyard_group(caster, caster, 2, &[who])
            .unwrap(),
        (owner, vec![])
    );
    let entities = [who, targets[0], targets[1]];
    let (_, cards) = g
        .announced_graveyard_group(caster, caster, 2, &entities)
        .unwrap();
    assert!(
        g.announced_graveyard_group(caster, owner, 2, &entities)
            .is_err()
    );
    for (limit, selection) in [
        (1, entities),
        (2, [who, targets[0], targets[0]]),
        (2, [who, targets[0], targets[2]]),
    ] {
        assert!(
            g.announced_graveyard_group(caster, caster, limit, &selection)
                .is_err()
        );
    }
    let before = g.state_digest().unwrap();
    let occurrences = cards
        .iter()
        .map(|object| ProposedEvent::Move {
            object: *object,
            destination: Zone::Library { owner },
        })
        .collect();
    assert_eq!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous { occurrences }
        )
        .unwrap_err(),
        "unsupported simultaneous read/write dependency"
    );
    assert_eq!(g.state_digest().unwrap(), before);
    let moved = g
        .move_object_for(ids[0], Zone::Hand { owner }, BatchCause::Fixture)
        .unwrap();
    g.move_object_for(
        moved.successor,
        Zone::Graveyard { owner },
        BatchCause::Fixture,
    )
    .unwrap();
    assert_eq!(g.remaining_graveyard_targets(owner, &cards), [ids[1]]);
}

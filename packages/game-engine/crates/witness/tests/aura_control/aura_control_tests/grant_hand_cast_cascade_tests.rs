//! Private exact-source witness. CR 109.5/514.2/601.2a/611.2f/702.85/800.4.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const CASTER: PlayerId = PlayerId(0);
const GRANTOR_OWNER: PlayerId = PlayerId(1);
const DEFENDER: PlayerId = PlayerId(2);
const STROBE: &str =
    include_str!("../../../../../tests/fixtures/9bf4ee0f-6e28-4793-b251-f81ee444a119.json");

fn scenario() -> (Prepared, CardId) {
    let archive = std::fs::read_to_string(
        std::env::var_os("MTG_GRANT_HAND_CAST_CASCADE_ARCHIVE_PROBE")
            .expect("supply the pinned Yidris archive"),
    )
    .unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "1797d0c1ae2b3988e92038d42efd75aebd952c9ad259711e4e2d06d84949ecb8"
    );
    let mut lines = archive.lines();
    let manifest = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(lines.next().is_none());
    assert_eq!(
        sha256(row.as_bytes()),
        "e14b03d2b0721bcdde1a106d0b2966c0713bdf963e2b3d03d10d71c3d2533904"
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(manifest, row)
    else {
        panic!("exact Yidris source must lower");
    };
    assert_eq!(definition.oracle_id, "9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f");
    let mut rows = vec![
        ("Grizzly Bears", 1, 0, false), // borrowed Yidris
        ("Assault Strobe", 0, 0, true),
        ("Giant Growth", 0, 0, true),
        ("Grizzly Bears", 0, 0, true),  // Apex Devastator
        ("Grizzly Bears", 0, 0, false), // Imoti
        ("Grizzly Bears", 0, 0, true),
    ];
    lands(&mut rows, "Mountain", 0, 1);
    lands(&mut rows, "Forest", 0, 12);
    let mut pod = prepared_with_sources(&rows, &[STROBE]);
    pod.config.profile = "private-grant-hand-cast-cascade".into();
    pod.config.cards[0].oracle_id = definition.oracle_id.clone();
    let bears = pod.config.cards[5].oracle_id.clone();
    let mut library = pod
        .config
        .cards
        .iter_mut()
        .filter(|card| card.zone == (Zone::Library { owner: CASTER }));
    let hit = library.next().unwrap();
    hit.oracle_id = bears;
    let hit = hit.id;
    // Leave a singleton remainder for the seedless midgame root and a land
    // to draw before the next-turn paid cast.
    for card in library.skip(1) {
        card.zone = Zone::Graveyard { owner: CASTER };
    }
    let catalog = Arc::make_mut(&mut pod.catalog);
    catalog.definitions.push(*definition);
    for (slot, row) in [
        (
            3,
            include_str!(
                "../../../../../tests/fixtures/ramos/b4d6747c-3516-4e71-be30-af007c6b1dc4.json"
            ),
        ),
        (
            4,
            include_str!(
                "../../../../../tests/fixtures/ramos/eca32dcd-6845-433e-a631-ed1f0ee78f25.json"
            ),
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
            include_str!("../../../../../tests/fixtures/ramos/archive-manifest-current.json"),
            row,
        ) else {
            panic!("complete cascade source must lower");
        };
        pod.config.cards[slot].oracle_id = definition.oracle_id.clone();
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();
    (pod, hit)
}

fn seek(g: &mut Game, mut matches: impl FnMut(&Game, &Request) -> bool) -> Request {
    for _ in 0..500 {
        let r = request(g, 1);
        if matches(g, &r) {
            return r;
        }
        answer(g, 1, pass(&r));
    }
    panic!("bounded combat or cascade boundary");
}

fn grant_ids(g: &Game) -> Vec<EffectId> {
    g.state
        .rules
        .effects
        .iter()
        .filter(|effect| matches!(effect.kind, ResolvedEffectKind::GrantHandCastCascade { .. }))
        .map(|effect| effect.id)
        .collect()
}

fn cast_matches(g: &Game, spell: StackId) -> impl Iterator<Item = EffectiveTriggerAbility> {
    g.events().iter().filter_map(move |event| match event {
        Event::TriggerCollected {
            ability,
            witness: TriggerEventWitness::SpellCast { cast },
            ..
        } if cast.stack_id == spell => Some(*ability),
        _ => None,
    })
}

#[test]
#[ignore = "requires the pinned one-card MTG_GRANT_HAND_CAST_CASCADE_ARCHIVE_PROBE"]
fn grant_hand_cast_cascade_paid_resolution_origin_and_cleanup() {
    let (pod, hit) = scenario();
    let mut g = Game::with_trace(pod, TraceMode::Detailed);
    let grantor = object(&g, 0);
    let turn = g.turn().number;
    let grantor_target = target(&g, 0);
    cast(&mut g, 1, 1, grantor_target);
    seek(&mut g, |_, r| matches!(r, Request::Attackers { .. }));
    answer(
        &mut g,
        1,
        Response::Attackers {
            attacks: vec![(grantor, DEFENDER)],
        },
    );
    seek(&mut g, |g, r| {
        g.turn().step == TurnStep::FirstStrikeDamage
            && matches!(r, Request::Priority { player: CASTER, .. })
    });
    let first = g.top().unwrap().id;
    assert_eq!(g.top().unwrap().source, grantor);
    assert_eq!(g.top().unwrap().controller, CASTER);
    assert!(grant_ids(&g).is_empty());

    // This paid spell responds to the unresolved trigger. Target Imoti so the
    // next combat hit still uses Yidris's exact damage source.
    let imoti = target(&g, 4);
    announce(&mut g, 1, 2, imoti);
    let early = g.top().unwrap();
    assert!(early.paid.is_some());
    assert_eq!(cast_matches(&g, early.id).count(), 0);
    resolve(&mut g, 1, |_| {});
    let first_effect = grant_ids(&g)[0];

    seek(&mut g, |g, r| {
        g.turn().step == TurnStep::CombatDamage
            && matches!(r, Request::Priority { player: CASTER, .. })
    });
    let second = g.top().unwrap().id;
    assert_ne!(first, second);
    assert_eq!(g.top().unwrap().source, grantor);
    assert_eq!(g.state.rules.players[DEFENDER.0 as usize].life, 30.into());
    assert_eq!(grant_ids(&g), [first_effect]);
    g.concede(GRANTOR_OWNER).unwrap();
    request(&mut g, 1);
    assert!(g.current(grantor).is_err());
    assert_eq!(g.top().unwrap().id, second);
    assert_eq!(grant_ids(&g), [first_effect]);
    resolve(&mut g, 1, |_| {});
    let effects = grant_ids(&g);
    assert_eq!(effects.len(), 2);
    assert_ne!(effects[0], effects[1]);

    seek(&mut g, |g, r| {
        g.turn().step == TurnStep::PostcombatMain
            && matches!(r, Request::Priority { player: CASTER, .. })
    });
    let hand = object(&g, 3);
    answer(&mut g, 1, Response::Cast { card: hand.card });
    assert!(matches!(request(&mut g, 1), Request::Mana { .. }));
    let proposal = g.top().unwrap().id;
    answer(&mut g, 1, Response::FinishMana { undo_mana: false });
    request(&mut g, 1);
    assert_eq!(object(&g, 3), hand);
    assert!(g.events().iter().any(|event| matches!(event,
        Event::CastReversed { stack_id, .. } if *stack_id == proposal
    )));
    assert_eq!(cast_matches(&g, proposal).count(), 0);
    assert_eq!(grant_ids(&g), effects);

    announce(&mut g, 1, 3, vec![]);
    let apex = g.state.rules.stack.first().unwrap();
    assert_eq!(apex.source.card, hand.card);
    assert!(apex.paid.is_some());
    let abilities = cast_matches(&g, apex.id).collect::<Vec<_>>();
    assert_eq!(abilities.len(), 7);
    assert_eq!(
        abilities
            .iter()
            .filter(|ability| matches!(ability, EffectiveTriggerAbility::Intrinsic { .. }))
            .count(),
        4
    );
    assert_eq!(
        abilities
            .iter()
            .filter(|ability| matches!(ability, EffectiveTriggerAbility::Granted { .. }))
            .count(),
        1
    );
    let temporary = abilities
        .iter()
        .filter_map(|ability| match ability {
            EffectiveTriggerAbility::TemporaryGrant {
                effect,
                grant_source,
                grant_stack_id,
                turn: applied_turn,
                ..
            } => {
                assert_eq!(*grant_source, grantor);
                assert_eq!(*applied_turn, turn);
                Some((*effect, *grant_stack_id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(temporary, [(effects[0], first), (effects[1], second)]);
    let permitted_effects = g.visible_effects(DEFENDER).unwrap();
    for (effect, _) in temporary {
        let handle = g
            .effect_handle(DEFENDER, &EffectKey::Resolved { id: effect })
            .unwrap();
        assert!(permitted_effects.iter().any(|row| matches!(
            row,
            VisibleEffect::GrantHandCastCascade { handle: seen, origin, .. }
                if *seen == handle && origin.controller == CASTER && origin.source.current.is_none()
        )));
    }
    recover(&g);

    seek(&mut g, |_, r| matches!(r, Request::OptionalEffect { .. }));
    answer(&mut g, 1, Response::OptionalEffect { perform: true });
    resolve(&mut g, 1, |_| {});
    assert_eq!(
        g.current(g.object_id(hit).unwrap()).unwrap().zone,
        Zone::Battlefield
    );
    assert_eq!(
        g.events()
            .iter()
            .filter(|e| matches!(e, Event::SpellCast { .. }))
            .count(),
        4 // Strobe, Growth, Apex and the cascade cast from exile.
    );
    assert_eq!(
        g.events()
            .iter()
            .filter(|e| matches!(
                e,
                Event::TriggerCollected {
                    ability: EffectiveTriggerAbility::TemporaryGrant { .. },
                    ..
                }
            ))
            .count(),
        2
    );
    assert_eq!(grant_ids(&g), effects);
    own_main(&mut g, 1, CASTER.0, turn);
    assert!(grant_ids(&g).is_empty());
    assert!(
        batches(&g)
            .iter()
            .flat_map(|batch| &batch.occurrences)
            .any(|occurrence| {
                matches!(&occurrence.actual, EventOutcome::EffectsCleaned { expired, .. }
            if effects.iter().all(|effect| expired.contains(effect)))
            })
    );
    announce(&mut g, 1, 5, vec![]);
    let later = g.top().unwrap();
    assert!(later.paid.is_some());
    assert_eq!(cast_matches(&g, later.id).count(), 0);
    recover(&g);
}

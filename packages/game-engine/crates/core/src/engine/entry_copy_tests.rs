//! One complete-source paid entry and restore. CR 303.4f, 614.12a, 707.5–6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[test]
#[ignore = "requires the frozen one-record Protege archive"]
fn protege_paid_entry_copies_aura_and_chooses_its_own_attachment() {
    let archive = std::fs::read_to_string(
        std::env::var_os("MTG_PROTEGE_ENTRY_COPY_ARCHIVE_PROBE")
            .expect("supply the frozen archive"),
    )
    .unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "e8b63300e5669872bf113bb6be2cbf6a4856b4ce7281c76bfdc8a98680a6d78b"
    );
    let mut lines = archive.lines();
    let mtg_compiler::CompileResult::Lowered { definition } =
        mtg_compiler::compile(lines.next().unwrap(), lines.next().unwrap())
    else {
        panic!("complete frozen Protege must lower")
    };
    assert!(lines.next().is_none());
    assert_eq!(definition.oracle_id, "cf127842-5fb1-44df-8d91-8cc0f48c559a");
    let mut rows = vec![
        ("Control Magic", 0, 0, true),
        ("Grizzly Bears", 1, 1, false),
        ("Grizzly Bears", 2, 2, false),
        ("Counterspell", 0, 0, true),
    ];
    lands(&mut rows, "Island", 0, 10);
    let mut pod = prepared(&rows);
    pod.config.cards[3].oracle_id = definition.oracle_id.clone();
    // An empty library for this scenario caster resolves cascade with no hit.
    pod.config
        .cards
        .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }));
    pod.config.profile = "private-paid-entry-copy".into();
    let catalog = Arc::make_mut(&mut pod.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();

    let mut game = Game::with_trace(pod.clone(), TraceMode::Detailed);
    let old_target = object(&game, 1);
    let new_target = object(&game, 2);
    cast(
        &mut game,
        1,
        0,
        vec![TargetEntity::Object { object: old_target }],
    );
    let donor = object(&game, 0);
    assert_eq!(game.current(donor).unwrap().attached_to, Some(old_target));
    announce(&mut game, 1, 3, vec![]);
    let spell = game
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell() && s.source.card == CardId(3))
        .unwrap()
        .clone();
    assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
    for _ in 0..8 {
        answer(&mut game, 1, Response::Pass);
    }
    let Request::EntryCopy {
        player,
        source,
        candidates,
        ..
    } = request(&mut game, 1)
    else {
        panic!("copy choice")
    };
    assert_eq!(player, PlayerId(0));
    assert_eq!(source, spell.source);
    assert_eq!(candidates, [donor]);
    assert!(game.decision_for(PlayerId(1)).unwrap().is_none());
    let mut forged = game.fork();
    let before = forged.state.clone();
    assert!(forged.move_object(source, Zone::Battlefield).is_err());
    assert_eq!(forged.state, before);
    let mut declined = game.fork();
    answer(&mut declined, 1, Response::EntryCopy { object: None });
    resolve(&mut declined, 1, |_| {});
    assert_eq!(
        declined
            .copiable_definition(object(&declined, 3))
            .unwrap()
            .name,
        "Sakashima's Protege"
    );

    answer(
        &mut game,
        1,
        Response::EntryCopy {
            object: Some(donor),
        },
    );
    assert!(
        matches!(request(&mut game, 1), Request::EntryAttachment { source: actual, candidates, .. }
        if actual == source && candidates.contains(&new_target))
    );
    let mut game = Game::restore(pod, &game.snapshot().unwrap()).unwrap();
    assert_eq!(game.zone(source), Some(Zone::Stack));
    answer(
        &mut game,
        1,
        Response::EntryAttachment { object: new_target },
    );
    resolve(&mut game, 1, |_| {});
    let entrant = object(&game, 3);
    assert_eq!(entrant.incarnation, source.incarnation + 1);
    assert_eq!(
        game.copiable_definition(entrant).unwrap().name,
        "Control Magic"
    );
    assert_eq!(game.current(entrant).unwrap().attached_to, Some(new_target));
    assert_eq!(game.current(donor).unwrap().attached_to, Some(old_target));
    assert_eq!(game.current(new_target).unwrap().controller, PlayerId(0));
    assert!(batches(&game).iter().any(|b| matches!(b.cause, BatchCause::SpellCompletion { stack_id, .. } if stack_id == spell.id)
        && b.occurrences.iter().any(|r| matches!(&r.actual, EventOutcome::EnteredAttached { movement, target, .. } if movement.successor == entrant && *target == new_target))));
    recover(&game);
}

//! Paid exact-source cast, response, resolution and counterfactual counter.
//! Run the ignored witness with MTG_CONTROLLED_CREATURE_SPELL_COPY_ARCHIVE_PROBE
//! pointing to the pinned two-line archive (manifest followed by one card).
use super::*;

const CASTER: PlayerId = PlayerId(0);

fn scenario() -> Prepared {
    let archive = std::fs::read_to_string(
        std::env::var_os("MTG_CONTROLLED_CREATURE_SPELL_COPY_ARCHIVE_PROBE")
            .expect("supply the exact Double Major archive"),
    )
    .unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "fb9290e269061249690c5e128652d51af00d3fe13af4cb363f342f8883fe5823"
    );
    let mut lines = archive.lines();
    let manifest = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(lines.next().is_none());
    assert_eq!(
        sha256(row.as_bytes()),
        "2fb5407e0742bb508fc968c9254d3edbcb4e67e1ff773c43ad03a4acc339ce84"
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(manifest, row)
    else {
        panic!("exact Double Major source must lower");
    };
    assert_eq!(definition.name, "Double Major");

    let mut rows = vec![
        ("Isamaru, Hound of Konda", 0, 0, true),
        ("Counterspell", 0, 0, true),
        ("Counterspell", 1, 1, true),
    ];
    lands(&mut rows, "Plains", 0, 1);
    lands(&mut rows, "Forest", 0, 1);
    lands(&mut rows, "Island", 0, 1);
    lands(&mut rows, "Island", 1, 2);
    let mut pod = prepared(&rows);
    pod.config.profile = "private-controlled-creature-spell-copy".into();
    pod.config.cards[0].zone = Zone::Command { owner: CASTER };
    pod.config.commanders = vec![CardId(0)];
    pod.config.cards[1].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut pod.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();
    pod
}

pub(super) fn respond_to_spell(
    g: &mut Game,
    card: CardId,
    target: StackId,
    lands: &[(&str, Color)],
) -> StackId {
    answer(g, 1, Response::Cast { card });
    assert!(
        matches!(request(g, 1), Request::StackTargets { candidates, .. } if candidates.contains(&target))
    );
    answer(
        g,
        1,
        Response::StackTargets {
            entries: vec![target],
        },
    );
    let mut spending = BTreeMap::<Color, Quantity>::new();
    for (name, color) in lands {
        fund(g, 1, name, 1);
        spending.entry(*color).or_default().add(&1.into());
    }
    answer(g, 1, Response::FinishMana { undo_mana: false });
    answer(g, 1, Response::SpendMana { spending });
    assert!(matches!(request(g, 1), Request::Priority { .. }));
    let spell = g.top().unwrap();
    assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
    spell.id
}

#[test]
#[ignore = "requires the pinned one-record Double Major archive"]
fn double_major_paid_copy_resolves_or_is_countered() {
    let pod = scenario();
    let mut game = Game::with_trace(pod.clone(), TraceMode::Detailed);
    announce(&mut game, 1, 0, vec![]);
    let original = game.top().unwrap().clone();
    assert_eq!(original.paid.as_ref(), Some(&original.total_cost));
    let major = respond_to_spell(
        &mut game,
        CardId(1),
        original.id,
        &[("Forest", Color::G), ("Island", Color::U)],
    );
    let cast_history = game.state.rules.cast_history.clone();
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    // Advance across the instruction and its enclosing spell's completion
    // independently. No priority or SBA may intervene between these boundaries.
    let mut child_boundary = false;
    for _ in 0..40 {
        match game.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {
                if matches!(&game.state.rules.frame, Frame::Resolve { frame } if frame.stack_id == major && frame.copied_child().is_some())
                {
                    game.validate_state().unwrap();
                    child_boundary = true;
                }
            }
            Advance::AwaitingDecision(Request::Priority { .. }) => break,
            other => panic!("unexpected copy resolution boundary {other:?}"),
        }
    }
    assert!(child_boundary);
    assert!(matches!(game.state.rules.frame, Frame::Priority));
    let copy = game.top().unwrap().clone();
    assert_eq!(
        game.state.rules.stack.as_slice(),
        &[original.clone(), copy.clone()]
    );
    assert_ne!(copy.id, original.id);
    assert_ne!(copy.source.card, original.source.card);
    assert_eq!(copy.controller, CASTER);
    assert_eq!(game.state.rules.cards[&copy.source.card].owner, CASTER);
    assert!(game.is_spell_copy(&copy) && copy.paid.is_none() && copy.cost_lock.is_none());
    assert_eq!(game.state.rules.cast_history, cast_history);
    assert_eq!(
        game.events()
            .iter()
            .filter(|e| matches!(e, Event::SpellCast { .. }))
            .count(),
        2
    );
    assert_eq!(
        game.commander_ledger()
            .tax(original.source.card, CASTER)
            .unwrap(),
        2.into()
    );
    assert!(game.commander_ledger().history(copy.source.card).is_none());
    assert!(
        game.visible_object(CASTER, game.current(copy.source).unwrap())
            .unwrap()
            .unwrap()
            .token
            .is_none()
    );

    // Restore at the actual response window and counter only the copy.
    let mut countered = Game::restore(pod, &game.snapshot().unwrap()).unwrap();
    assert_eq!(game.state, countered.state);
    until_player(&mut countered, 1, PlayerId(1));
    respond_to_spell(
        &mut countered,
        CardId(2),
        copy.id,
        &[("Island", Color::U), ("Island", Color::U)],
    );
    resolve(&mut countered, 1, |_| {});
    assert!(countered.object_id(copy.source.card).is_none());
    assert_eq!(
        countered.state.rules.objects[&original.source.card].zone,
        Zone::Battlefield
    );
    assert!(
        batches(&countered)
            .iter()
            .flat_map(|batch| &batch.occurrences)
            .any(|leaf| matches!(&leaf.actual,
        EventOutcome::SpellCopyCeased { object } if object.card == copy.source.card))
    );

    resolve(&mut game, 1, |_| {});
    let permanent = &game.state.rules.objects[&copy.source.card];
    assert_eq!(permanent.zone, Zone::Battlefield);
    assert_eq!(permanent.id.incarnation, 1);
    assert!(
        game.visible_object(CASTER, permanent)
            .unwrap()
            .unwrap()
            .token
            .is_some()
    );
    let original_permanent = &game.state.rules.objects[&original.source.card];
    assert_eq!(original_permanent.zone, Zone::Battlefield);
    let mut expected = game.characteristics(original_permanent.id).unwrap();
    assert!(expected.legendary);
    expected.legendary = false;
    assert_eq!(game.characteristics(permanent.id).unwrap(), expected);
    assert!(batches(&game).iter().all(|batch| {
        !batch
            .occurrences
            .iter()
            .any(|leaf| matches!(leaf.actual, EventOutcome::TokensCreated { .. }))
    }));
    assert_eq!(game.state.rules.cast_history, cast_history);
    game.validate_state().unwrap();
}

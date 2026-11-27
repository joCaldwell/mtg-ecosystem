//! Annihilator (CR 702.86) and mentor (CR 702.134), lowered to their
//! defining attack triggers, from exact frozen R5 source on private boards.
//! Mentor's target bound is rechecked on resolution, from the source's last
//! known information once it has left (CR 608.2b).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_core::QueryResult;

const ULAMOGS_CRUSHER: &str = record!("597bce51-af89-4d13-9a97-667b4f4f4694");
const PATHRAZER_OF_ULAMOG: &str = record!("4bf95747-4572-49b8-b892-87fe7f910252");
const HAMMER_DROPPER: &str = record!("088c9fa7-65ec-48c6-90cc-9087bc9df43e");
const BLADE_INSTRUCTOR: &str = record!("77cff683-013c-4568-8ae0-f86ef0590fe1");

fn zone(g: &Game, object: ObjectId) -> Zone {
    g.state.rules.objects[&object.card].zone
}

fn plus_one(g: &Game, object: ObjectId) -> Quantity {
    g.current(object).unwrap().plus_one_counters.clone()
}

fn power(g: &Game, object: ObjectId) -> Option<Quantity> {
    g.characteristics(object).unwrap().power
}

fn triggers_of(g: &Game, source: ObjectId) -> usize {
    g.state
        .rules
        .stack
        .iter()
        .filter(|s| s.source == source && matches!(s.kind, StackKind::Triggered { .. }))
        .count()
}

/// Target the first of `preferred` that a decision offers.
fn first_of(preferred: &[ObjectId]) -> impl Fn(&[ObjectId]) -> ObjectId + '_ {
    move |offered| {
        *preferred
            .iter()
            .find(|o| offered.contains(o))
            .expect("a preferred target is offered")
    }
}

/// No mentor targets are expected.
fn no_targets(offered: &[ObjectId]) -> ObjectId {
    panic!("unexpected target decision {offered:?}")
}

/// Declare `attacks` and stop at the first priority with the declaration's
/// triggers on the stack. Each mentor target decision targets `pick` of
/// the objects it offers; the offers are returned in order.
fn attack(
    g: &mut Game,
    attacks: &[(ObjectId, PlayerId)],
    pick: impl Fn(&[ObjectId]) -> ObjectId,
) -> Vec<Vec<ObjectId>> {
    let mut offered = vec![];
    let mut declared = false;
    for _ in 0..256 {
        let d = Driver::request(g, u64::MAX);
        if declared && d.reason == Reason::Priority {
            return offered;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => {
                declared = true;
                d.attack(attacks.to_vec())
            }
            Reason::Targets { .. } => {
                let objects = d.objects_offered();
                let answer = d.targets(vec![pick(&objects)]);
                offered.push(objects);
                answer
            }
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the attack was never declared");
}

/// Pass priority until a sacrifice decision, returning it, or until the
/// stack is empty.
fn until_sacrifice(g: &mut Game) -> Option<Decision<Rules>> {
    for _ in 0..256 {
        let d = Driver::request(g, u64::MAX);
        match &d.reason {
            Reason::Sacrifice => return Some(d),
            Reason::Priority if g.state.rules.stack.is_empty() => return None,
            Reason::Priority => {
                let answer = mtg_witness::temporary::pass(&d);
                g.respond(d.player, d.id, answer).unwrap();
            }
            other => panic!("unexpected decision {other:?}"),
        }
    }
    panic!("the stack never emptied");
}

/// Cast the hand instant `card` at `target`, paying with `land`.
fn cast_at(g: &mut Game, card: u32, target: ObjectId, land: u32) {
    Driver::cast(g, CardId(card));
    let d = Driver::request(g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }), "{:?}", d.reason);
    g.respond(d.player, d.id, d.targets(vec![target])).unwrap();
    Driver::pay(g, &[land]);
}

/// Four seats. Ulamog's Crusher (annihilator 2) attacks seat 1, who
/// controls three permanents: only seat 1 is asked, for exactly two of its
/// own permanents (CR 702.86a, 508.5), and sacrifices the two it chooses.
/// Seat 2's permanent is never offered.
#[test]
fn annihilator_defending_player_sacrifices_n_of_their_choice() {
    let mut g = Game::with_trace(
        prepare(
            board(&[ULAMOGS_CRUSHER], TurnStep::BeginningCombat)
                .seats(4)
                .permanent_or_hand("Ulamog's Crusher", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Grizzly Bears", 2, 2, false),
        ),
        TraceMode::Detailed,
    );
    let [crusher, bears, first, second, other] = [0, 1, 2, 3, 4].map(|i| Driver::object(&g, i));
    attack(&mut g, &[(crusher, PlayerId(1))], no_targets);
    assert_eq!(triggers_of(&g, crusher), 1);
    checked(&g);
    let d = until_sacrifice(&mut g).expect("the defending player sacrifices");
    assert_eq!(d.player, PlayerId(1));
    assert_eq!(d.required_count(), 2);
    assert_eq!(d.objects_offered(), vec![bears, first, second]);
    g.respond(d.player, d.id, d.pick_objects(vec![bears, second]))
        .unwrap();
    assert!(until_sacrifice(&mut g).is_none());
    assert_eq!(zone(&g, bears), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(zone(&g, second), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(zone(&g, first), Zone::Battlefield);
    assert_eq!(zone(&g, other), Zone::Battlefield);
    checked(&g);
    assert_recoverable((&g).into());
}

/// CR 508.5: the controller of the attacked planeswalker is the defending
/// player. The planeswalker itself can be one of the sacrificed permanents.
#[test]
fn annihilator_binds_the_attacked_planeswalkers_controller() {
    let mut g = Game::new(prepare(
        board(
            &[ULAMOGS_CRUSHER, crate::planeswalker::JACE],
            TurnStep::BeginningCombat,
        )
        .seats(3)
        .permanent_or_hand("Ulamog's Crusher", 0, 0, false)
        .permanent_or_hand("Jace Beleren", 1, 1, false)
        .permanent_or_hand("Island", 1, 1, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Island", 2, 2, false),
    ));
    let [jace, island, bears, other] = [1, 2, 3, 4].map(|i| Driver::object(&g, i));
    let d = crate::evasion::until(&mut g, &Reason::Attackers);
    let defender = d.index(&Candidate::Object { object: jace }).unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![defender]],
        },
    )
    .unwrap();

    let d = until_sacrifice(&mut g).expect("the defending planeswalker controller sacrifices");
    assert_eq!((d.player, d.required_count()), (PlayerId(1), 2));
    assert_eq!(d.objects_offered(), vec![jace, island, bears]);
    let answer = d.pick_objects(vec![jace, island]);
    g.respond(d.player, d.id, answer).unwrap();
    assert!(until_sacrifice(&mut g).is_none());
    assert_eq!(zone(&g, jace), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(zone(&g, other), Zone::Battlefield);
    checked(&g);
    assert_recoverable((&g).into());
}

/// Pathrazer of Ulamog (annihilator 3) attacks a player who controls two
/// permanents: both are sacrificed (CR 609.3). Against a player with none,
/// the trigger resolves with no choice.
#[test]
fn annihilator_takes_every_permanent_when_fewer_than_n() {
    let mut g = Game::with_trace(
        prepare(
            board(&[PATHRAZER_OF_ULAMOG], TurnStep::BeginningCombat)
                .seats(4)
                .permanent_or_hand("Pathrazer of Ulamog", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let [pathrazer, bears, island] = [0, 1, 2].map(|i| Driver::object(&g, i));
    attack(&mut g, &[(pathrazer, PlayerId(1))], no_targets);
    let d = until_sacrifice(&mut g).expect("the defending player sacrifices");
    assert_eq!((d.player, d.required_count()), (PlayerId(1), 2));
    assert_eq!(d.objects_offered(), vec![bears, island]);
    g.respond(d.player, d.id, d.pick_objects(vec![bears, island]))
        .unwrap();
    assert!(until_sacrifice(&mut g).is_none());
    assert_eq!(zone(&g, bears), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(zone(&g, island), Zone::Graveyard { owner: PlayerId(1) });
    assert_recoverable((&g).into());

    let mut g = Game::with_trace(
        prepare(
            board(&[PATHRAZER_OF_ULAMOG], TurnStep::BeginningCombat)
                .seats(4)
                .permanent_or_hand("Pathrazer of Ulamog", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let pathrazer = Driver::object(&g, 0);
    attack(&mut g, &[(pathrazer, PlayerId(2))], no_targets);
    assert_eq!(triggers_of(&g, pathrazer), 1);
    assert!(until_sacrifice(&mut g).is_none());
    checked(&g);
    assert_recoverable((&g).into());
}

/// A creature with two instances of Ulamog's Crusher's annihilator 2 (as a
/// granted second instance would give it) triggers twice (CR 702.86b): the
/// defending player sacrifices two permanents for each, four in all.
#[test]
fn annihilator_instances_trigger_separately() {
    let mut definitions = compiled(&[ULAMOGS_CRUSHER], true);
    let instance = definitions[0].triggered[0].clone();
    definitions[0].triggered.push(instance);
    let labels = definitions[0].source_labels.as_mut().unwrap();
    labels.triggered.push(labels.triggered[0]);
    let mut g = Game::with_trace(
        prepare(
            board_with(definitions, TurnStep::BeginningCombat)
                .permanent_or_hand("Ulamog's Crusher", 0, 0, false)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let crusher = Driver::object(&g, 0);
    let islands = [1, 2, 3, 4, 5].map(|i| Driver::object(&g, i));
    attack(&mut g, &[(crusher, PlayerId(1))], no_targets);
    assert_eq!(triggers_of(&g, crusher), 2);
    for pair in [[islands[0], islands[1]], [islands[2], islands[3]]] {
        let d = until_sacrifice(&mut g).expect("each instance sacrifices");
        assert_eq!((d.player, d.required_count()), (PlayerId(1), 2));
        g.respond(d.player, d.id, d.pick_objects(pair.to_vec()))
            .unwrap();
        checked(&g);
    }
    assert!(until_sacrifice(&mut g).is_none());
    let left: Vec<_> = islands
        .iter()
        .filter(|island| zone(&g, **island) == Zone::Battlefield)
        .collect();
    assert_eq!(left, [&islands[4]]);
    assert_recoverable((&g).into());
}

/// Hammer Dropper (5 power) attacks with Blade Instructor (3), Grizzly Bears
/// (2) and Ulamog's Crusher (8) while another Bears stays home. Dropper's
/// mentor offers only the attacking creatures of lesser power, Instructor
/// and the attacking Bears, never itself, the larger Crusher or the
/// creature at home (CR 702.134a). Instructor's mentor offers only the
/// Bears. Each puts its counter on its target.
#[test]
fn mentor_targets_an_attacking_creature_with_lesser_power() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[HAMMER_DROPPER, BLADE_INSTRUCTOR, ULAMOGS_CRUSHER],
                TurnStep::BeginningCombat,
            )
            .permanent_or_hand("Hammer Dropper", 0, 0, false)
            .permanent_or_hand("Blade Instructor", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Ulamog's Crusher", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let [dropper, instructor, bears, crusher, home] =
        [0, 1, 2, 3, 4].map(|i| Driver::object(&g, i));
    let defender = PlayerId(1);
    let attacks = [
        (dropper, defender),
        (instructor, defender),
        (bears, defender),
        (crusher, defender),
    ];
    let mut offered = attack(&mut g, &attacks, first_of(&[instructor, bears]));
    // The Crusher's annihilator has nothing to target.
    offered.iter_mut().for_each(|o| o.sort());
    offered.sort_by_key(|o| std::cmp::Reverse(o.len()));
    assert_eq!(offered, [vec![instructor, bears], vec![bears]]);
    assert!(!offered.concat().contains(&home));
    checked(&g);
    assert!(until_sacrifice(&mut g).is_none());
    assert_eq!(plus_one(&g, instructor), Quantity::from(1));
    assert_eq!(plus_one(&g, bears), Quantity::from(1));
    assert_eq!(plus_one(&g, dropper), Quantity::zero());
    assert_eq!(plus_one(&g, crusher), Quantity::zero());
    assert_recoverable((&g).into());
}

/// The mentor bound is serialized both in suspended observation work and in
/// a snapshot while the accepted target answer awaits application.
#[test]
fn mentor_target_bound_round_trips_through_pending_input_and_observation_work() {
    let prepared = prepare(
        board(&[BLADE_INSTRUCTOR], TurnStep::BeginningCombat)
            .permanent_or_hand("Blade Instructor", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false),
    );
    let mut g = Game::new(prepared.clone());
    let mentor = Driver::object(&g, 0);
    let bears = Driver::object(&g, 1);
    let d = crate::evasion::until(&mut g, &Reason::Attackers);
    let answer = d.attack(vec![(mentor, PlayerId(1)), (bears, PlayerId(1))]);
    g.respond(d.player, d.id, answer).unwrap();

    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }));
    let QueryResult::Incomplete { work, .. } = g.observe(d.player, Budget::new(1)) else {
        panic!("mentor target observation should suspend");
    };
    let raw = g.save_work(&work).unwrap();
    assert!(
        raw.contains("\"less_than_source\""),
        "mentor bound was not saved"
    );
    let restored = g.restore_work(&raw).unwrap();
    assert_eq!(g.save_work(&restored).unwrap(), raw);

    let answer = d.targets(vec![bears]);
    g.respond(d.player, d.id, answer).unwrap();
    let snapshot = g.snapshot().unwrap();
    assert!(
        snapshot.contains("\"less_than_source\""),
        "mentor bound was not snapshotted"
    );
    let restored = Game::restore(prepared, &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert_recoverable((&g).into());
}

/// Blade Instructor (3) attacks with Hammer Dropper (5) alone beside it: no
/// attacking creature has power less than 3, so its mentor has no legal
/// target and is removed (CR 603.3d), while Dropper's targets Instructor.
#[test]
fn mentor_without_a_lesser_attacker_is_removed() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[HAMMER_DROPPER, BLADE_INSTRUCTOR],
                TurnStep::BeginningCombat,
            )
            .permanent_or_hand("Hammer Dropper", 0, 0, false)
            .permanent_or_hand("Blade Instructor", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let [dropper, instructor] = [0, 1].map(|i| Driver::object(&g, i));
    let offered = attack(
        &mut g,
        &[(dropper, PlayerId(1)), (instructor, PlayerId(1))],
        first_of(&[instructor]),
    );
    assert_eq!(offered, [vec![instructor]]);
    assert_eq!(triggers_of(&g, instructor), 0);
    assert_eq!(triggers_of(&g, dropper), 1);
    assert!(until_sacrifice(&mut g).is_none());
    assert_eq!(plus_one(&g, instructor), Quantity::from(1));
    assert_recoverable((&g).into());
}

/// Hammer Dropper's mentor targets the attacking Grizzly Bears; in response
/// Giant Growth makes the Bears 5/5. On resolution its power is no longer
/// less than Dropper's, so the target is illegal and the ability does not
/// resolve (CR 608.2b): no counter.
#[test]
fn mentor_does_not_resolve_when_the_target_is_no_longer_lesser() {
    let mut g = Game::with_trace(
        prepare(
            board(&[HAMMER_DROPPER], TurnStep::BeginningCombat)
                .permanent_or_hand("Hammer Dropper", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Giant Growth", 0, 0, true),
        ),
        TraceMode::Detailed,
    );
    let [dropper, bears] = [0, 1].map(|i| Driver::object(&g, i));
    attack(
        &mut g,
        &[(dropper, PlayerId(1)), (bears, PlayerId(1))],
        first_of(&[bears]),
    );
    cast_at(&mut g, 3, bears, 2);
    assert!(until_sacrifice(&mut g).is_none());
    assert_eq!(power(&g, bears), Some(Quantity::from(5)));
    assert_eq!(plus_one(&g, bears), Quantity::zero());
    checked(&g);
    assert_recoverable((&g).into());
}

/// Hammer Dropper's mentor targets an attacker; in response Disfigure gives
/// Dropper -2/-2 and it dies as a 3/0. On resolution "this creature's
/// power" is its last known power, 3 (CR 608.2b, 113.7a): an attacking
/// Grizzly Bears (2) is still a legal target and gets the counter, but Blade
/// Instructor (3) is not and gets none. Printed power would allow both.
/// (Instructor's own mentor has no lesser attacker beside Dropper and is
/// removed.)
#[test]
fn mentor_rechecks_the_departed_source_by_last_known_power() {
    for (attacker, counters) in [("Grizzly Bears", 1), ("Blade Instructor", 0)] {
        let mut g = Game::with_trace(
            prepare(
                board(
                    &[HAMMER_DROPPER, BLADE_INSTRUCTOR],
                    TurnStep::BeginningCombat,
                )
                .permanent_or_hand("Hammer Dropper", 0, 0, false)
                .permanent_or_hand(attacker, 0, 0, false)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Disfigure", 0, 0, true),
            ),
            TraceMode::Detailed,
        );
        let [dropper, target] = [0, 1].map(|i| Driver::object(&g, i));
        attack(
            &mut g,
            &[(dropper, PlayerId(1)), (target, PlayerId(1))],
            first_of(&[target]),
        );
        assert_eq!(g.state.rules.stack.len(), 1);
        cast_at(&mut g, 3, dropper, 2);
        assert!(until_sacrifice(&mut g).is_none());
        assert_eq!(zone(&g, dropper), Zone::Graveyard { owner: PlayerId(0) });
        assert_eq!(plus_one(&g, target), Quantity::from(counters), "{attacker}");
        checked(&g);
        assert_recoverable((&g).into());
    }
}

#[test]
fn mentor_reads_current_source_power_at_target_selection_and_resolution() {
    const GOBLIN_BANNERET: &str = record!("a2d9554e-70c0-4213-ba5e-f0954ad32854");
    let mut g = Game::with_trace(
        prepare(
            board(&[GOBLIN_BANNERET], TurnStep::BeginningCombat)
                .permanent_or_hand("Goblin Banneret", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Disfigure", 0, 0, true),
        ),
        TraceMode::Detailed,
    );
    let [mentor, bears] = [0, 1].map(|id| Driver::object(&g, id));
    cast_at(&mut g, 3, mentor, 2);
    // A 1/1 Banneret becomes 4/4 before it attacks.
    assert!(until_sacrifice(&mut g).is_none());
    let offered = attack(
        &mut g,
        &[(mentor, PlayerId(1)), (bears, PlayerId(1))],
        first_of(&[bears]),
    );
    assert_eq!(offered, [vec![bears]]);
    // Disfigure leaves the source on the battlefield at equal power.
    cast_at(&mut g, 5, mentor, 4);
    assert!(until_sacrifice(&mut g).is_none());
    assert_eq!(power(&g, mentor), Some(Quantity::from(2)));
    assert_eq!(plus_one(&g, bears), Quantity::zero());
    assert_recoverable((&g).into());
}

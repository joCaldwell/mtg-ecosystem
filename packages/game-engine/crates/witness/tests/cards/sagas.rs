//! CR 505.4, 603.3, 704.5s, 714: frozen R5 Sagas and their chapter invocations.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_compiler::CompileResult;
use mtg_contracts::kir::{
    Atom, Change, CounterKind, EventPattern, Node, Pred, Ref, Scope, StaticEffect, Tiers,
    definition_tiers,
};
use mtg_core::testkit::*;
use mtg_sim::policy::{Policy, Strategy};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const HISTORY: &str = record!("c15bb7eb-aaaa-4468-9641-8f706d6137e8");
const WARCALLER: &str = record!("b44d3fe8-b1e2-4abb-ab82-7a516a1db1e5");
const BASTION: &str = record!("9fb8cd81-403a-4988-8f1c-b8eccf8abd9c");
const AVACYN: &str = record!("216cb26e-8da9-478b-bfbc-8030f7adee72");
const HUMILITY: &str = record!("ed7bdb3e-5c51-4547-9266-76a791e0b2b0");
const NATURALIZE: &str = record!("bdb3ca68-ec1f-4e16-81cc-d23f8f52c728");
const GAMES: &str = record!("58934f6d-1aa2-414c-85c6-955a1e26d675");
const CLASH: &str = record!("b1c38b70-575d-4d87-bc50-0b77b48ee823");

fn compile(raw: &str) -> CompileResult {
    mtg_witness::compile(mtg_witness::R5_MANIFEST, raw)
}

fn lower(raw: &str) -> Definition {
    let result = compile(raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("frozen Saga witness failed: {result:?}");
    };
    *definition
}

fn catalog(extra: impl IntoIterator<Item = Definition>) -> Catalog {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for definition in extra {
        if !catalog
            .definitions
            .iter()
            .any(|d| d.oracle_id == definition.oracle_id)
        {
            catalog.definitions.push(definition);
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    catalog
}

fn board(catalog: Catalog, name: &str) -> Board {
    Board::new(catalog, name)
        .seats(2)
        .step(TurnStep::PrecombatMain)
}

fn game(board: Board) -> Game {
    Game::with_trace(board.libraries("Plains", 4).prepare(), TraceMode::Detailed)
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert_recoverable(g.into());
}

fn priority(g: &mut Game) {
    let d = Driver::request(g, 1);
    assert_eq!(d.reason, Reason::Priority, "{d:?}");
}

fn pass_until(g: &mut Game, predicate: impl Fn(&Game) -> bool) {
    for _ in 0..160 {
        let d = Driver::request(g, 1);
        let answer = match d.reason {
            Reason::Priority if predicate(g) => return,
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(vec![]),
            Reason::Blockers => d.block(vec![]),
            _ => panic!("unexpected decision while passing: {d:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!(
        "did not reach expected priority at {:?} (active {:?}, {:?})",
        g.turn(),
        g.state.rules.active,
        g.state.rules.frame
    );
}

fn lore(g: &Game, card: u32) -> Quantity {
    g.current(Driver::object(g, card))
        .unwrap()
        .counter_count(&CounterKind::Lore)
}

fn chapter(g: &Game, source: ObjectId, index: u32, previous: i64, current: i64) {
    let entry = g.state.rules.stack.last().expect("chapter on the stack");
    assert_eq!(entry.source, source);
    assert!(matches!(&entry.kind, StackKind::Triggered {
        ability: EffectiveTriggerAbility::Intrinsic { index: i },
        witness: TriggerEventWitness::LorePlaced { object, previous: before, current: after },
        ..
    } if *i == index && *object == source && *before == previous.into() && *after == current.into()));
}

fn sacrifice_recorded(g: &Game, saga: ObjectId) -> bool {
    g.events().iter().any(|e| {
        matches!(e,
            Event::Committed {
                proposed: ProposedEvent::Sacrifice { object, player: PlayerId(0) },
                actual: EventOutcome::Sacrificed { movement, .. },
            } if *object == saga && movement.previous == saga
                && movement.destination == (Zone::Graveyard { owner: PlayerId(0) })
        )
    })
}

fn observe(g: &Game, player: PlayerId) -> mtg_core::Observation {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.observe(player, Budget::unlimited())
    else {
        panic!("missing observation");
    };
    observation
}

fn policy_answer(g: &mut Game, strategy: &mut Strategy<'_>, reason: Reason<Rules>) {
    let d = Driver::request(g, 1);
    assert_eq!(d.reason, reason);
    let observation = observe(g, d.player);
    let answer = strategy.choose_with_game(&observation, g).unwrap();
    let token = observation.decision.unwrap().id;
    let QueryResult::Complete(QueryValue::Validated(receipt)) =
        g.check_player_response(d.player, token, answer, Budget::unlimited())
    else {
        panic!("simulator produced an invalid chapter decision");
    };
    g.submit(receipt).unwrap();
}

#[test]
fn saga_entry_precombat_proliferate_and_sacrifice() {
    let history = lower(HISTORY);
    assert_eq!(definition_tiers(&history).unwrap(), Tiers::EXECUTES);
    let oracle_id = history.oracle_id.clone();
    let mut g = game(
        board(
            catalog([history, lower(BASTION), lower(AVACYN)]),
            "saga-turn",
        )
        .permanent_or_hand("History of Benalia", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Karn's Bastion", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Avacyn, Angel of Hope", 0, 0, false)
        .permanent_or_hand("History of Benalia", 1, 1, false),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    pass_until(&mut g, |g| {
        g.zone(Driver::object(g, 0)) == Some(Zone::Battlefield)
    });
    let saga = Driver::object(&g, 0);
    assert_eq!(lore(&g, 0), 1.into());
    chapter(&g, saga, 0, 0, 1);
    let view = observe(&g, PlayerId(1)).view;
    let visible = view
        .objects
        .iter()
        .find(|o| o.oracle_id == oracle_id && o.controller == PlayerId(0))
        .unwrap();
    assert!(
        visible
            .counters
            .iter()
            .any(|c| c.kind == CounterKind::Lore && c.count == 1.into())
    );
    checked(&g);

    pass_until(&mut g, |g| g.turn().step == TurnStep::PostcombatMain);
    assert_eq!(lore(&g, 0), 1.into());
    pass_until(&mut g, |g| {
        g.turn().step == TurnStep::PrecombatMain && g.state.rules.active == Some(PlayerId(1))
    });
    assert_eq!(lore(&g, 0), 1.into());
    let other = Driver::object(&g, 7);
    assert_eq!(lore(&g, 7), 1.into());
    chapter(&g, other, 0, 0, 1);
    pass_until(&mut g, |g| {
        g.turn().step == TurnStep::Upkeep && g.state.rules.active == Some(PlayerId(0))
    });
    assert_eq!(lore(&g, 0), 1.into());
    pass_until(&mut g, |g| g.turn().step == TurnStep::PrecombatMain);
    assert_eq!(lore(&g, 0), 2.into());
    chapter(&g, saga, 1, 1, 2);
    pass_until(&mut g, |g| g.state.rules.stack.is_empty());

    let bastion = Driver::object(&g, 4);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: bastion,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1, 2, 3, 5]);
    let d = loop {
        let d = Driver::request(&mut g, 1);
        if d.reason == Reason::Proliferate {
            break d;
        }
        assert_eq!(d.reason, Reason::Priority);
        Driver::pass(&mut g);
    };
    checked(&g);
    let picks = [saga, other].map(|object| d.index(&Candidate::Object { object }).unwrap());
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![picks.into()],
        },
    )
    .unwrap();
    priority(&mut g);
    assert_eq!(lore(&g, 0), 3.into());
    assert_eq!(lore(&g, 7), 2.into());
    assert_eq!(g.state.rules.stack.len(), 2);
    chapter(&g, other, 1, 1, 2);
    pass_until(&mut g, |g| g.state.rules.stack.len() == 1);
    chapter(&g, saga, 2, 2, 3);
    assert!(g.has_keyword(saga, KeywordAbility::Indestructible));
    assert!(!sacrifice_recorded(&g, saga));
    checked(&g);

    pass_until(&mut g, |g| g.state.rules.stack.is_empty());
    assert!(sacrifice_recorded(&g, saga));
    assert_eq!(lore(&g, 0), 0.into());
    checked(&g);
}

// Keldon Warcaller's frozen target and placement program, with an exact
// additional counter in the test, exercises multiple crossings in one event.
fn warcaller(amount: i64) -> Definition {
    let mut card = lower(WARCALLER);
    let [ability] = card.triggered.as_mut_slice() else {
        panic!("missing Warcaller attack trigger");
    };
    let [
        Instruction::Kernel {
            node:
                Node::Do(Verb::PutCounters {
                    amount: quantity,
                    counter: CounterKind::Lore,
                    ..
                }),
            ..
        },
    ] = ability.program.instructions.as_mut_slice()
    else {
        panic!("missing Warcaller lore placement");
    };
    *quantity = Amount::Fixed {
        value: amount.into(),
    };
    card.source_labels = None;
    card.capabilities = card.requirements();
    card
}

fn attack(g: &mut Game, attacker: u32, saga: ObjectId) {
    let attacker = Driver::object(g, attacker);
    for _ in 0..18 {
        let d = Driver::request(g, 1);
        match d.reason {
            Reason::Priority => Driver::pass(g),
            Reason::Attackers => {
                let answer = d.attack(vec![(attacker, PlayerId(1))]);
                g.respond(d.player, d.id, answer).unwrap();
            }
            Reason::Targets { .. } => {
                let answer = d.entity_targets(vec![TargetEntity::Object { object: saga }]);
                g.respond(d.player, d.id, answer).unwrap();
                return;
            }
            _ => panic!("unexpected attack decision: {d:?}"),
        }
    }
    panic!("Warcaller did not trigger");
}

#[test]
fn saga_crosses_multiple_chapters_and_simulator_orders_them() {
    let catalog = catalog([lower(HISTORY), warcaller(3)]);
    let mut g = game(
        board(catalog.clone(), "saga-multiple-crossings")
            .step(TurnStep::BeginningCombat)
            .permanent_or_hand("History of Benalia", 0, 0, false)
            .permanent_or_hand("Keldon Warcaller", 0, 0, false),
    );
    let saga = Driver::object(&g, 0);
    assert_eq!(lore(&g, 0), 0.into());
    attack(&mut g, 1, saga);
    let d = (0..8)
        .find_map(|_| {
            let d = Driver::request(&mut g, 1);
            if d.reason == Reason::OrderTriggers {
                return Some(d);
            }
            assert_eq!(d.reason, Reason::Priority, "{d:?}");
            Driver::pass(&mut g);
            None
        })
        .expect("chapter order after the Warcaller trigger resolves");
    assert_eq!(lore(&g, 0), 3.into());
    assert_eq!(d.candidates.len(), 3);
    checked(&g);
    policy_answer(
        &mut g,
        &mut Strategy::new(&catalog, Policy::Develop, [0; 4]),
        Reason::OrderTriggers,
    );
    priority(&mut g);
    assert_eq!(g.state.rules.stack.len(), 3);
    chapter(&g, saga, 2, 0, 3);
    pass_until(&mut g, |g| g.state.rules.stack.len() == 2);
    chapter(&g, saga, 1, 0, 3);
    assert_eq!(g.zone(saga), Some(Zone::Battlefield));
    checked(&g);
    pass_until(&mut g, |g| g.state.rules.stack.is_empty());
    assert!(sacrifice_recorded(&g, saga));
    assert_eq!(g.state.rules.tokens.len(), 2);
    checked(&g);
}

#[test]
fn saga_without_chapters_does_not_advance_or_sacrifice() {
    // Retarget Humility's existing layer-6 removal at other enchantments
    // while leaving their types alone. Printed Saga creatures are outside admission.
    let mut loss = lower(HUMILITY);
    loss.source_labels = None;
    let [ability] = loss.static_abilities.as_mut_slice() else {
        panic!("missing Humility effect");
    };
    ability.effect = StaticEffect::Continuous {
        affects: Scope::Matching {
            filter: Pred(vec![
                Atom::Type {
                    kind: CardKind::Enchantment,
                },
                Atom::IsNot {
                    object: Ref::Source,
                },
            ]),
        },
        changes: vec![Change::RemoveAllAbilities {
            except_own_grants: false,
        }],
        condition: None,
    };
    loss.capabilities = loss.requirements();
    let mut g = game(
        board(
            catalog([lower(HISTORY), lower(CLASH), loss, lower(NATURALIZE)]),
            "saga-ability-loss",
        )
        .permanent_or_hand("History of Benalia", 0, 0, true)
        .permanent_or_hand("Humility", 0, 0, false)
        .permanent_or_hand("Naturalize", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Clash of the Eikons", 0, 0, true)
        .permanent_or_hand("Clash of the Eikons", 0, 0, true)
        .permanent_or_hand("Clash of the Eikons", 0, 0, true),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[3, 4, 5]);
    pass_until(&mut g, |g| g.state.rules.stack.is_empty());
    let saga = Driver::object(&g, 0);
    assert_eq!(g.zone(saga), Some(Zone::Battlefield));
    assert_eq!(lore(&g, 0), 0.into());
    for (card, land) in [(9, 6), (10, 7), (11, 8)] {
        cast_clash(&mut g, card, land, 0b100, saga);
        pass_until(&mut g, |g| g.state.rules.stack.is_empty());
    }
    assert_eq!(lore(&g, 0), 3.into());
    let turn = g.turn().number;
    pass_until(&mut g, |g| {
        g.turn().number > turn
            && g.turn().step == TurnStep::PrecombatMain
            && g.state.rules.active == Some(PlayerId(0))
    });
    assert_eq!(lore(&g, 0), 3.into());
    assert_eq!(g.zone(saga), Some(Zone::Battlefield));
    assert!(g.state.rules.stack.is_empty());
    assert!(!sacrifice_recorded(&g, saga));
    checked(&g);

    Driver::cast(&mut g, CardId(2));
    let d = Driver::request(&mut g, 1);
    let answer = d.entity_targets(vec![TargetEntity::Object {
        object: Driver::object(&g, 1),
    }]);
    g.respond(d.player, d.id, answer).unwrap();
    Driver::pay(&mut g, &[6, 7]);
    pass_until(&mut g, |g| g.state.rules.stack.is_empty());
    assert!(sacrifice_recorded(&g, saga));
    checked(&g);
}

#[test]
fn saga_chapter_remains_on_the_stack_after_its_source_leaves() {
    let mut g = game(
        board(
            catalog([lower(HISTORY), lower(NATURALIZE)]),
            "saga-source-lki",
        )
        .permanent_or_hand("History of Benalia", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Naturalize", 1, 1, true)
        .permanent_or_hand("Forest", 1, 1, false)
        .permanent_or_hand("Forest", 1, 1, false),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    pass_until(&mut g, |g| {
        g.zone(Driver::object(g, 0)) == Some(Zone::Battlefield)
    });
    let saga = Driver::object(&g, 0);
    chapter(&g, saga, 0, 0, 1);
    Driver::pass(&mut g);
    Driver::cast(&mut g, CardId(4));
    let d = Driver::request(&mut g, 1);
    let answer = d.entity_targets(vec![TargetEntity::Object { object: saga }]);
    g.respond(d.player, d.id, answer).unwrap();
    Driver::pay(&mut g, &[5, 6]);

    pass_until(&mut g, |g| g.zone(saga).is_none());
    chapter(&g, saga, 0, 0, 1);
    checked(&g);
    pass_until(&mut g, |g| !g.state.rules.tokens.is_empty());
    assert_eq!(g.state.rules.tokens.len(), 1);
    checked(&g);
}

fn cast_clash(g: &mut Game, card: u32, land: u32, mask: u8, saga: ObjectId) {
    Driver::cast(g, CardId(card));
    let d = Driver::request(g, 1);
    assert_eq!(d.reason, Reason::ChooseModes);
    let mode = d
        .candidates
        .iter()
        .find(|c| matches!(c, Candidate::Modes { mask: offered, .. } if *offered == mask))
        .unwrap();
    g.respond(d.player, d.id, d.pick_one(mode).unwrap())
        .unwrap();
    let d = Driver::request(g, 1);
    let targets = vec![TargetEntity::Object { object: saga }; mask.count_ones() as usize];
    g.respond(d.player, d.id, d.entity_targets(targets))
        .unwrap();
    Driver::pay(g, &[land]);
}

#[test]
fn saga_lore_removal_allows_a_chapter_to_trigger_again() {
    let mut g = game(
        board(catalog([lower(HISTORY), lower(CLASH)]), "saga-remove-lore")
            .permanent_or_hand("History of Benalia", 0, 0, true)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Clash of the Eikons", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    pass_until(&mut g, |g| g.state.rules.stack.is_empty());
    let saga = Driver::object(&g, 0);
    assert_eq!(lore(&g, 0), 1.into());
    assert_eq!(g.state.rules.tokens.len(), 1);

    cast_clash(&mut g, 4, 5, 0b110, saga);

    pass_until(&mut g, |g| {
        g.events().iter().any(|e| {
            matches!(e, Event::Committed {
        actual: EventOutcome::LoreCounterRemoved { object, .. }, ..
    } if *object == saga)
        })
    });
    assert_eq!(lore(&g, 0), 1.into());
    assert_eq!(g.state.rules.stack.len(), 1);
    chapter(&g, saga, 0, 0, 1);
    checked(&g);

    pass_until(&mut g, |g| g.state.rules.stack.is_empty());
    assert_eq!(g.state.rules.tokens.len(), 2);
    checked(&g);
}

#[test]
fn saga_rejects_unsupported_printed_rules_and_keeps_every_chapter() {
    for (raw, blocker) in [
        (
            record!("6b92a5f4-a241-48c8-9aec-c8e9d5b01fa5"),
            "keyword:read_ahead",
        ),
        (
            record!("0990e57f-279e-4348-a049-e06187cc38d4"),
            "subtype:saga_creature",
        ),
        (
            record!("02c60b8b-f665-4279-a00f-ae03991df4a6"),
            "layout:multiface_saga",
        ),
        (
            record!("89a2d5bc-df06-47e8-8b4a-6d1f0a63c78d"),
            "trigger:final-chapter-ability",
        ),
        (
            record!("e376702a-f56e-4e47-aa8c-3f80384d12a9"),
            "ring-tempts",
        ),
    ] {
        let result = compile(raw);
        assert!(
            matches!(&result, CompileResult::Unsupported { path, reason } if reason == blocker || path.ends_with(blocker) || reason.starts_with(&format!("unknown variant `{blocker}`"))),
            "{result:?}"
        );
    }
    let history = lower(HISTORY);
    let chapters: Vec<_> = history
        .triggered
        .iter()
        .map(|a| a.condition.pattern())
        .collect();
    assert_eq!(
        chapters,
        (1..=3)
            .map(|number| EventPattern::SagaChapter { number })
            .collect::<Vec<_>>()
            .iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(history.triggered[0].program, history.triggered[1].program);
    assert_eq!(lower(GAMES).triggered.len(), 4);
}

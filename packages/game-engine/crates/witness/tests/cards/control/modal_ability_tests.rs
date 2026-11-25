//! Modal triggered and activated abilities from exact frozen R4 source on
//! private boards. A triggered ability's modes are chosen as it is put on
//! the stack and an activated ability's as it is activated, before targets;
//! only selections whose targets can be chosen are offered, and a trigger
//! with none is removed from the stack. The chosen modes run in printed
//! order. Every choice is checked for state validity and an exact snapshot
//! round trip; every finished resolution replays from its journal.
//! Pinned CR 602.2b, 603.3c, 608.2c, 700.2a-b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Tiers, definition_tiers};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

const TRUFFLESNOUT: &str = record!("8a15841a-dcb6-454a-b865-43a067f42b4e");
const COLISEUM_BEHEMOTH: &str = record!("076cf4f1-b3d4-4175-b976-938ad851cc0e");
const DECEIVER_EXARCH: &str = record!("3c939ea6-68b7-4965-b1d3-af1d3dc79778");
const FEROCIFICATION: &str = record!("814e06c0-9b61-4a64-8c67-73a8eb00a768");
const RETREAT_TO_KAZANDU: &str = record!("3f8e5ff1-af89-427e-924c-19a44f9a3788");
const KNIGHT_OF_AUTUMN: &str = record!("c4212ba4-8c43-40ca-aa36-b3b535659fa8");
const ELVEN_RAFT_STEERER: &str = record!("d0f62148-1015-49b8-b61f-a4332325a6b0");
const GLAMERMITE: &str = record!("a7b0d897-38c0-4f90-81e8-3c5b8d31a607");
const SAURIAN_SYMBIOTE: &str = record!("14b180d2-5701-452d-8deb-cf0845c95df5");
const BLACK_PANTHER: &str = record!("b0bf1964-c543-4c53-9f09-0dd7c608d5c5");
const ETHERWROUGHT_PAGE: &str = record!("f1c87913-9374-41d7-ae70-444d427290b4");

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen modal ability witness failed: {result:?}");
    };
    *definition
}

/// A two-seat board at `step` whose catalog adds `definitions`, each with a
/// modal ability and `Executes` only.
fn board_with(definitions: Vec<Definition>, step: TurnStep) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for definition in definitions {
        assert!(
            definition
                .activated
                .iter()
                .any(|a| a.modal.is_some())
                || definition.triggered.iter().any(|a| a.modal.is_some()),
            "{}",
            definition.name
        );
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        catalog.definitions.push(definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-modal-ability-witness")
        .step(step)
        .seats(2)
}

fn board(cards: &[&str], step: TurnStep) -> Board {
    board_with(cards.iter().map(|raw| lowered(raw)).collect(), step)
}

/// State validation and an exact snapshot round trip.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// The offered mode masks of a mode decision, in offer order.
fn offered(d: &Decision<Rules>) -> Vec<u8> {
    d.candidates
        .iter()
        .map(|c| match c {
            Candidate::Modes { mask, .. } => *mask,
            other => panic!("non-mode candidate {other:?}"),
        })
        .collect()
}

/// Pass priority until seat `seat` is asked to choose modes.
fn until_modes(g: &mut Game, seat: u8) -> Decision<Rules> {
    for _ in 0..64 {
        let d = Driver::request(g, u64::MAX);
        if d.reason == Reason::ChooseModes {
            assert_eq!(d.player, PlayerId(seat), "{d:?}");
            assert!(matches!(d.shape, Shape::Pick { .. }), "{d:?}");
            checked(g);
            return d;
        }
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        Driver::pass(g);
    }
    panic!("no mode decision");
}

fn choose(g: &mut Game, d: &Decision<Rules>, mask: u8) {
    let chosen = d
        .candidates
        .iter()
        .find(|c| matches!(c, Candidate::Modes { mask: m, .. } if *m == mask))
        .unwrap();
    g.respond(d.player, d.id, d.pick_one(chosen).unwrap())
        .unwrap();
}

/// Resolve to an empty stack, then prove replay from the journal.
fn settle(g: &mut Game) {
    for _ in 0..24 {
        let d = Driver::request(g, 1);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            checked(g);
            assert_recoverable((&*g).into());
            return;
        }
        Driver::pass(g);
    }
    panic!("no empty-stack priority");
}

/// Pass until the active player's next step begins, then check the game.
fn play_on(g: &mut Game) {
    let step = g.state.rules.turn.step;
    for _ in 0..64 {
        let d = Driver::request(g, u64::MAX);
        if g.state.rules.turn.step != step {
            checked(g);
            assert_recoverable((&*g).into());
            return;
        }
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        Driver::pass(g);
    }
    panic!("the game did not continue");
}

fn life(g: &Game, seat: usize) -> Quantity {
    g.state.rules.players[seat].life.clone()
}

fn plus(q: &Quantity, n: i64) -> Quantity {
    let mut q = q.clone();
    q.add(&Quantity::from(n));
    q
}

fn counters(g: &Game, card: u32) -> Quantity {
    g.current(Driver::object(g, card))
        .unwrap()
        .plus_one_counters
        .clone()
}

/// Trufflesnout: "When this creature enters, choose one — • Put a +1/+1
/// counter on this creature. • You gain 4 life." The trigger waits for its
/// mode in its placement frame with no mask; saved state with a mask that
/// names no selection, or a bound untargeted selection still being placed,
/// is rejected. Only the chosen mode is performed.
#[test]
fn trufflesnout_chooses_its_mode_as_the_trigger_is_placed() {
    for mask in [0b01, 0b10] {
        let mut g = Game::new(
            board(&[TRUFFLESNOUT], TurnStep::PrecombatMain)
                .permanent_or_hand("Trufflesnout", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .libraries("Island", 4)
                .prepare(),
        );
        let start = life(&g, 0);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3]);
        let d = until_modes(&mut g, 0);
        assert_eq!(offered(&d), [0b01, 0b10]);
        assert!(matches!(g.state.rules.frame, Frame::TriggerTargets { .. }));
        let top = g.state.rules.stack.last().unwrap();
        assert!(matches!(top.kind, StackKind::Triggered { .. }));
        assert_eq!((top.mode_mask, top.paid.is_none()), (None, true));
        for forged in [0b11, 0b100, 0b01] {
            let mut bad = g.fork();
            bad.state.rules.stack.last_mut().unwrap().mode_mask = Some(forged);
            assert!(bad.validate_state().is_err(), "mask {forged:#b}");
        }
        choose(&mut g, &d, mask);
        checked(&g);
        settle(&mut g);
        let (counter, gained) = if mask == 0b01 { (1, 0) } else { (0, 4) };
        assert_eq!(counters(&g, 0), Quantity::from(counter));
        assert_eq!(life(&g, 0), plus(&start, gained));
        play_on(&mut g);
    }
}

/// Coliseum Behemoth: "Destroy target artifact or enchantment." cannot be
/// chosen without one (CR 700.2b); its target is chosen after the mode, and
/// the opponent sees the announced mode.
#[test]
fn coliseum_behemoth_offers_only_modes_with_legal_targets() {
    let game = |ring: bool| {
        let mut b = board(&[COLISEUM_BEHEMOTH], TurnStep::PrecombatMain)
            .permanent_or_hand("Coliseum Behemoth", 0, 0, true);
        for _ in 0..7 {
            b = b.permanent_or_hand("Forest", 0, 0, false);
        }
        if ring {
            b = b.permanent_or_hand("Sol Ring", 1, 1, false);
        }
        let mut g = Game::new(b.libraries("Island", 4).prepare());
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3, 4, 5, 6, 7]);
        g
    };
    let mut g = game(false);
    let d = until_modes(&mut g, 0);
    assert_eq!(offered(&d), [0b10]);
    choose(&mut g, &d, 0b10);
    settle(&mut g);

    let mut g = game(true);
    let d = until_modes(&mut g, 0);
    assert_eq!(offered(&d), [0b01, 0b10]);
    choose(&mut g, &d, 0b01);
    let ring = Driver::object(&g, 8);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
    assert_eq!(d.candidates, [Candidate::Object { object: ring }]);
    checked(&g);
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(1), Budget::unlimited())
    else {
        panic!("missing observation");
    };
    assert_eq!(o.view.stack.last().unwrap().mode_mask, Some(0b01));
    Driver::pick_object(&mut g, ring);
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 8)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
    play_on(&mut g);
}

/// Ferocification: "At the beginning of combat on your turn, choose one —"
/// with both modes targeting a creature you control. Without one, no mode
/// can be chosen and the ability is removed from the stack (CR 603.3c);
/// with one, the chosen pump applies.
#[test]
fn ferocification_with_no_choosable_mode_is_removed_from_the_stack() {
    let mut g = Game::with_trace(
        board(&[FEROCIFICATION], TurnStep::PrecombatMain)
            .permanent_or_hand("Ferocification", 0, 0, false)
            .libraries("Island", 4)
            .prepare(),
        TraceMode::Detailed,
    );
    play_on(&mut g);
    assert_eq!(g.state.rules.turn.step, TurnStep::BeginningCombat);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert!(g.state.rules.stack.is_empty());
    assert!(g.events().iter().any(|e| matches!(
        e,
        Event::TriggeredAbilityNotPlaced {
            reason: TriggerPlacementFailure::NoLegalTargets,
            ..
        }
    )));
    checked(&g);
    assert_recoverable((&g).into());

    let mut g = Game::new(
        board(&[FEROCIFICATION], TurnStep::PrecombatMain)
            .permanent_or_hand("Ferocification", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .libraries("Island", 4)
            .prepare(),
    );
    let bears = Driver::object(&g, 1);
    let d = until_modes(&mut g, 0);
    assert_eq!(g.state.rules.turn.step, TurnStep::BeginningCombat);
    assert_eq!(offered(&d), [0b01, 0b10]);
    choose(&mut g, &d, 0b01);
    Driver::pick_object(&mut g, bears);
    settle(&mut g);
    assert_eq!(
        g.characteristics(bears).unwrap().power,
        Some(Quantity::from(4))
    );
    play_on(&mut g);
}

/// Deceiver Exarch: "Untap target permanent you control." is always
/// choosable (the Exarch itself); "Tap target permanent an opponent
/// controls." needs an opponent's permanent.
#[test]
fn deceiver_exarch_taps_an_opponents_permanent() {
    let game = |bears: bool| {
        let mut b = board(&[DECEIVER_EXARCH], TurnStep::PrecombatMain)
            .permanent_or_hand("Deceiver Exarch", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false);
        if bears {
            b = b.permanent_or_hand("Grizzly Bears", 1, 1, false);
        }
        let mut g = Game::new(b.libraries("Island", 4).prepare());
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3]);
        g
    };
    let mut g = game(false);
    assert_eq!(offered(&until_modes(&mut g, 0)), [0b01]);
    let mut g = game(true);
    let d = until_modes(&mut g, 0);
    assert_eq!(offered(&d), [0b01, 0b10]);
    choose(&mut g, &d, 0b10);
    let bears = Driver::object(&g, 4);
    Driver::pick_object(&mut g, bears);
    settle(&mut g);
    assert!(g.current(bears).unwrap().tapped);
    play_on(&mut g);
}

/// Retreat to Kazandu: a landfall trigger collected from a land play
/// chooses its mode when it is placed; the counter goes on the target.
#[test]
fn retreat_to_kazandu_chooses_a_mode_for_each_landfall() {
    let mut g = Game::new(
        board(&[RETREAT_TO_KAZANDU], TurnStep::PrecombatMain)
            .permanent_or_hand("Retreat to Kazandu", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, true)
            .libraries("Island", 4)
            .prepare(),
    );
    let (bears, forest) = (Driver::object(&g, 1), Driver::object(&g, 2));
    Driver::action(&mut g, Action::PlayLand { object: forest });
    let d = until_modes(&mut g, 0);
    assert_eq!(offered(&d), [0b01, 0b10]);
    choose(&mut g, &d, 0b01);
    Driver::pick_object(&mut g, bears);
    settle(&mut g);
    assert_eq!(counters(&g, 1), Quantity::from(1));
    play_on(&mut g);
}

/// Every offered selection of further frozen modal triggers resolves on a
/// shared board, each in a fresh game: targets and resolution choices take
/// the first candidate. Each choice is checked and each resolution replays.
fn resolve_every_selection(raw: &str) -> Vec<u8> {
    let source: serde_json::Value = serde_json::from_str(raw).unwrap();
    let (name, cost) = (
        source["source"]["name"].as_str().unwrap(),
        source["source"]["mana_cost"].as_str().unwrap(),
    );
    let lands: Vec<&str> = cost
        .split(['{', '}'])
        .filter(|s| !s.is_empty())
        .flat_map(|symbol| match symbol {
            "W" => vec!["Plains"],
            "U" => vec!["Island"],
            "B" => vec!["Swamp"],
            "R" => vec!["Mountain"],
            "G" => vec!["Forest"],
            n => vec!["Mountain"; n.parse().unwrap()],
        })
        .collect();
    let game = || {
        let mut b = board(&[raw], TurnStep::PrecombatMain).permanent_or_hand(name, 0, 0, true);
        for land in &lands {
            b = b.permanent_or_hand(land, 0, 0, false);
        }
        let mut g = Game::new(
            b.permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Sol Ring", 1, 1, false)
                .permanent_or_hand("Glorious Anthem", 1, 1, false)
                .libraries("Forest", 4)
                .seed([7; 32])
                .prepare(),
        );
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &(1..=lands.len() as u32).collect::<Vec<_>>());
        g
    };
    let masks = offered(&until_modes(&mut game(), 0));
    for &mask in &masks {
        let mut g = game();
        let d = until_modes(&mut g, 0);
        choose(&mut g, &d, mask);
        for _ in 0..64 {
            let d = Driver::request(&mut g, u64::MAX);
            checked(&g);
            match &d.reason {
                Reason::Priority if g.state.rules.stack.is_empty() => break,
                Reason::Priority => Driver::pass(&mut g),
                _ => g
                    .respond(d.player, d.id, d.pick_one(&d.candidates[0]).unwrap())
                    .unwrap(),
            }
        }
        assert!(g.state.rules.stack.is_empty(), "{name} {mask:#b}");
        assert_eq!(g.zone(Driver::object(&g, 0)), Some(Zone::Battlefield));
        assert_recoverable((&g).into());
    }
    masks
}

/// Further entry triggers: counters on the source, token creation,
/// targeted destruction, life gain and targeted tap or untap.
#[test]
fn every_offered_entry_mode_resolves() {
    for (raw, masks) in [
        (SAURIAN_SYMBIOTE, &[0b01, 0b10][..]),
        (KNIGHT_OF_AUTUMN, &[0b001, 0b010, 0b100]),
        (GLAMERMITE, &[0b01, 0b10]),
        (TRUFFLESNOUT, &[0b01, 0b10]),
        (DECEIVER_EXARCH, &[0b01, 0b10]),
    ] {
        assert_eq!(resolve_every_selection(raw), masks);
    }
}

/// The modal triggers of an upkeep artifact, an entering-Hero observer and
/// a landfall creature lower with one selection per mode, in printed order.
#[test]
fn further_modal_triggers_lower_one_selection_per_mode() {
    for (raw, modes) in [
        (ETHERWROUGHT_PAGE, 3),
        (BLACK_PANTHER, 2),
        (ELVEN_RAFT_STEERER, 2),
    ] {
        let d = lowered(raw);
        let modal = d.triggered[0].modal.as_ref().unwrap();
        assert_eq!(
            modal.selections.keys().copied().collect::<Vec<_>>(),
            (0..modes).map(|i| 1u8 << i).collect::<Vec<_>>()
        );
        assert!(d.triggered[0].program.instructions.is_empty());
        assert_eq!(definition_tiers(&d).unwrap(), Tiers::EXECUTES);
    }
}

/// A private artifact with the activated ability "{1}: Choose one — • Tap
/// target creature. • Untap target creature." built from Glamermite's
/// frozen modes, since no frozen modal activation lowers yet. Its modes are
/// chosen as it is activated, before its target and its mana (CR 602.2b,
/// 601.2b-c).
fn modal_activator() -> Definition {
    let mut d = lowered(GLAMERMITE);
    d.oracle_id = "ffffffff-ffff-4fff-8fff-00000000c002".into();
    d.name = "Private Modal Activator".into();
    d.characteristics.kind = CardKind::Artifact;
    d.characteristics.subtypes.clear();
    (d.characteristics.power, d.characteristics.toughness) = (None, None);
    d.keywords.clear();
    d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
    let trigger = d.triggered.remove(0);
    d.activated.push(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        restriction: None,
        cost: ActivationCost::Mana {
            cost: ManaCost {
                generic: Quantity::from(1),
                colored: vec![],
            },
        },
        program: trigger.program,
        modal: trigger.modal,
    });
    if let Some(labels) = &mut d.source_labels {
        labels.activated = std::mem::take(&mut labels.triggered);
    }
    d.capabilities = d.requirements();
    d.validate().unwrap();
    d
}

#[test]
fn a_modal_activation_chooses_modes_before_its_target_and_mana() {
    let game = |bears: bool| {
        let mut b = board_with(vec![modal_activator()], TurnStep::PrecombatMain)
            .permanent_or_hand("Private Modal Activator", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false);
        if bears {
            b = b.permanent_or_hand("Grizzly Bears", 1, 1, false);
        }
        Game::new(b.libraries("Island", 4).prepare())
    };
    let mut g = game(true);
    let (activator, bears) = (Driver::object(&g, 0), Driver::object(&g, 2));
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: activator,
            ability: 0,
        },
    );
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::ChooseModes, "{d:?}");
    assert!(matches!(g.state.rules.frame, Frame::ChooseModes { .. }));
    checked(&g);
    // Both selections target a creature; untapping a creature is legal
    // whether or not it is tapped.
    assert_eq!(offered(&d), [0b01, 0b10]);
    // A forged announcement whose modes cannot be offered is a fault, not
    // an empty decision.
    let mut empty = g.fork();
    for object in empty.state.rules.objects.values_mut() {
        if object.id == bears {
            object.zone = Zone::Exile;
        }
    }
    assert!(
        empty
            .decision_for(PlayerId(0))
            .is_err_and(|e| e.contains("no offerable mode"))
    );
    choose(&mut g, &d, 0b01);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
    assert_eq!(g.state.rules.stack.last().unwrap().mode_mask, Some(0b01));
    checked(&g);
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert!(g.current(bears).unwrap().tapped);
    play_on(&mut g);

    // With no creature at all, neither mode can be chosen, so the ability
    // cannot be activated (CR 700.2a).
    let mut g = game(false);
    let d = Driver::request(&mut g, u64::MAX);
    let activator = Driver::object(&g, 0);
    assert!(
        d.index(&Candidate::Action {
            action: Action::ActivateAbility {
                object: activator,
                ability: 0,
            },
        })
        .is_none()
    );
}

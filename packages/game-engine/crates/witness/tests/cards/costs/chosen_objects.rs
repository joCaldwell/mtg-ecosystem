//! Chosen costs from frozen R5. CR 302.6, 400.2, 601.2c/g-h, 602.2b,
//! 701.9, 701.13 and 701.26.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const HOUNDS: &str = record!("0184d74b-e9aa-4b5a-8db0-b15cecbc17fb");
const HAUNTED_DEAD: &str = record!("03259490-a789-4f96-8c5b-b410425bdd02");
const METEOR_STORM: &str = record!("03f96c23-de0b-4b85-81f8-febc850aa621");
const CLOCK: &str = record!("5d0f4665-aa10-4251-b590-129b59274fb3");
const GOBLIN: &str = record!("2055eb91-ee36-4752-a6dc-581eaef335c8");
const BRAMBLESNAP: &str = record!("312ce96b-6abe-4f72-a205-9b27c885a5fe");
const MINES: &str = record!("583cdebe-0195-45be-bd2e-5765f07cb902");
const EVANGEL: &str = record!("4a2fea39-99d6-4766-9f2c-0a63925349cb");
const INFESTATION: &str = record!("bdce1af0-3643-4e77-88f9-320206a191d4");

fn reason(action: ChosenCostAction) -> Reason<Rules> {
    Reason::ChosenObjectCost { action }
}

#[test]
fn typed_discard_is_private_until_paid_and_rejected_without_a_match() {
    for with_creature in [false, true] {
        let b = board(&[(HOUNDS, &[Cell::ChosenObjectCost])])
            .permanent_or_hand("Vampire Hounds", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 1, true);
        let prepared = if with_creature {
            b.permanent_or_hand("Colossal Dreadmaw", 0, 0, true)
                .prepare()
        } else {
            b.prepare()
        };
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let hounds = Driver::object(&g, 0);
        if !with_creature {
            assert!(!actions(&mut g).contains(&activate(hounds, 0)));
            continue;
        }

        let discard = Driver::object(&g, 3);
        let identity = g.state.rules.cards[&discard.card].oracle_id.clone();
        let shown = |g: &Game| {
            observation(g, 1)
                .view
                .objects
                .iter()
                .any(|o| o.oracle_id == identity)
        };
        Driver::action(&mut g, activate(hounds, 0));
        let d = expect(&mut g, 0, reason(ChosenCostAction::Discard));
        assert_eq!(offered_objects(&d), [discard]);
        assert_eq!(observation(&g, 1).decision, None);
        assert!(!shown(&g));
        let answer = d.pick_one(&Candidate::Object { object: discard }).unwrap();
        assert!(g.respond(PlayerId(1), d.id, answer.clone()).is_err());
        g.respond(d.player, d.id, answer).unwrap();
        assert!(!shown(&g));
        expect(&mut g, 0, Reason::Priority);
        assert_eq!(zone(&g, 3), Some(Zone::Graveyard { owner: PlayerId(0) }));
        assert_eq!(zone(&g, 1), Some(Zone::Hand { owner: PlayerId(0) }));
        assert!(shown(&g));
        assert_recoverable((&g).into());
        settle(&mut g);
        assert_eq!(
            observation(&g, 0)
                .view
                .objects
                .iter()
                .find(|o| o.oracle_id == g.state.rules.cards[&hounds.card].oracle_id)
                .unwrap()
                .characteristics
                .as_ref()
                .unwrap()
                .power,
            Some(4.into())
        );
    }
}

#[test]
fn discard_two_keeps_the_prefix_private_and_pays_only_on_completion() {
    for hand in [1, 3] {
        let mut b = board(&[(INFESTATION, &[Cell::ChosenObjectCost])]).permanent_or_hand(
            "Zombie Infestation",
            0,
            0,
            false,
        );
        for _ in 0..hand {
            b = b.permanent_or_hand("Forest", 0, 0, true);
        }
        let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let ability = activate(Driver::object(&g, 0), 0);
        if hand < 2 {
            assert!(!actions(&mut g).contains(&ability));
            continue;
        }
        Driver::action(&mut g, ability);
        let [first, second, third] = [1, 2, 3].map(|n| Driver::object(&g, n));
        let pick_discard = reason(ChosenCostAction::Discard);
        let d = expect(&mut g, 0, pick_discard.clone());
        assert_eq!(offered_objects(&d), [first, second]);
        Driver::pick_object(&mut g, second);
        let d = expect(&mut g, 0, pick_discard);
        assert_eq!(offered_objects(&d), [third]);
        assert_eq!(hand_size(&g, 0), 3);
        assert!(observation(&g, 1).decision.is_none());
        let mut forged = g.fork();
        let Frame::ChosenObjectCost { selected, .. } = &mut forged.state.rules.frame else {
            panic!("missing partial chosen-cost frame");
        };
        *selected = vec![third];
        assert!(
            forged
                .validate_state()
                .unwrap_err()
                .contains("chosen-object cost progress")
        );
        assert_recoverable((&g).into());
        Driver::pick_object(&mut g, third);
        expect(&mut g, 0, Reason::Priority);
        assert_eq!(hand_size(&g, 0), 1);
        assert_eq!(zone(&g, 2), Some(Zone::Graveyard { owner: PlayerId(0) }));
        assert_eq!(zone(&g, 3), Some(Zone::Graveyard { owner: PlayerId(0) }));

        let mut forged = g.fork();
        let mut entry = serde_json::to_value(&forged.state.rules.stack[0]).unwrap();
        entry["chosen_payment"].as_array_mut().unwrap().swap(0, 1);
        forged.state.rules.stack[0] = serde_json::from_value(entry).unwrap();
        assert!(
            forged
                .validate_state()
                .unwrap_err()
                .contains("count or order")
        );
        settle(&mut g);
        assert_eq!(g.state.rules.tokens.len(), 1);
    }
}

#[test]
fn tap_two_targets_first_and_another_activation_can_untap_a_paid_object() {
    let prepared = board(&[(CLOCK, &[Cell::ChosenObjectCost])])
        .permanent_or_hand("Clock of Omens", 0, 0, false)
        .permanent_or_hand("Wooden Sphere", 0, 0, false)
        .permanent_or_hand("Pristine Talisman", 0, 0, false)
        .permanent_or_hand("Sol Ring", 0, 0, false)
        .permanent_or_hand("Wooden Sphere", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let [clock, sphere, talisman, ring, opponent] = [0, 1, 2, 3, 4].map(|n| Driver::object(&g, n));
    Driver::action(&mut g, activate(clock, 0));
    assert!(matches!(
        Driver::request(&mut g, u64::MAX).reason,
        Reason::Targets { .. }
    ));
    Driver::pick_object(&mut g, opponent);
    let tap = reason(ChosenCostAction::Tap);
    let d = expect(&mut g, 0, tap.clone());
    assert_eq!(offered_objects(&d), [clock, sphere, talisman]);
    Driver::pick_object(&mut g, sphere);
    let d = expect(&mut g, 0, tap.clone());
    assert_eq!(offered_objects(&d), [talisman, ring]);
    assert!(!g.current(sphere).unwrap().tapped);
    Driver::pick_object(&mut g, talisman);
    expect(&mut g, 0, Reason::Priority);
    assert!(g.current(sphere).unwrap().tapped && g.current(talisman).unwrap().tapped);

    let mut forged = g.fork();
    forged
        .state
        .rules
        .objects
        .get_mut(&sphere.card)
        .unwrap()
        .tapped = false;
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("chosen-object cost outcome")
    );

    Driver::action(&mut g, activate(clock, 0));
    Driver::pick_object(&mut g, sphere);
    assert_eq!(offered_objects(&expect(&mut g, 0, tap.clone())), [clock]);
    Driver::pick_object(&mut g, clock);
    assert_eq!(offered_objects(&expect(&mut g, 0, tap)), [ring]);
    Driver::pick_object(&mut g, ring);
    assert!(!actions(&mut g).contains(&activate(clock, 0)));
    for _ in g.live_players() {
        Driver::pass(&mut g);
    }
    Driver::request(&mut g, u64::MAX);
    assert_eq!(g.state.rules.stack.len(), 1);
    assert!(!g.current(sphere).unwrap().tapped);
    checked(&g);
    settle(&mut g);
}

#[test]
fn written_tap_can_pay_with_the_summoning_sick_source_without_a_symbol() {
    let prepared = board(&[(BRAMBLESNAP, &[Cell::ChosenObjectCost])])
        .permanent_or_hand("Bramblesnap", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    settle(&mut g);
    let source = Driver::object(&g, 0);
    assert!(!g.current(source).unwrap().attack_ready);
    Driver::action(&mut g, activate(source, 0));
    let d = expect(&mut g, 0, reason(ChosenCostAction::Tap));
    assert_eq!(offered_objects(&d), [source]);
    Driver::pick_object(&mut g, source);
    expect(&mut g, 0, Reason::Priority);
    assert!(g.current(source).unwrap().tapped);
    settle(&mut g);
}

#[test]
fn evangel_offer_reserves_a_creature_after_funding_mana() {
    for second_creature in [false, true] {
        let board = board(&[(EVANGEL, &[Cell::ChosenObjectCost])])
            .permanent_or_hand("Selesnya Evangel", 0, 0, false)
            .permanent_or_hand("Llanowar Elves", 0, 0, false);
        let board = if second_creature {
            board.permanent_or_hand("Llanowar Elves", 0, 0, false)
        } else {
            board
        };
        let mut game = Game::with_trace(board.prepare(), TraceMode::Detailed);
        assert!(game.state.rules.players[0].mana.is_empty());
        let source = Driver::object(&game, 0);
        let elves = Driver::object(&game, 1);
        let activation = activate(source, 0);
        assert_eq!(actions(&mut game).contains(&activation), second_creature);
        if second_creature {
            let other_elves = Driver::object(&game, 2);
            Driver::action(&mut game, activation);
            Driver::pay(&mut game, &[1]);
            let request = expect(&mut game, 0, reason(ChosenCostAction::Tap));
            assert_eq!(offered_objects(&request), [other_elves]);
            Driver::pick_object(&mut game, other_elves);
            expect(&mut game, 0, Reason::Priority);
            assert!(
                [source, elves, other_elves]
                    .into_iter()
                    .all(|id| game.current(id).unwrap().tapped)
            );
            settle(&mut game);
            assert_eq!(game.state.rules.tokens.len(), 1);
        }
    }
}

#[test]
fn source_symbol_and_chosen_tap_need_different_objects_and_recheck_after_mana() {
    for mana_uses_creature in [false, true] {
        let prepared = board(&[(EVANGEL, &[Cell::ChosenObjectCost])])
            .permanent_or_hand("Selesnya Evangel", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Llanowar Elves", 0, 0, false)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let [source, forest, elves] = [0, 1, 2].map(|n| Driver::object(&g, n));
        Driver::action(&mut g, activate(source, 0));
        Driver::pay(&mut g, &[if mana_uses_creature { 2 } else { 1 }]);
        if mana_uses_creature {
            expect(&mut g, 0, Reason::Priority);
            assert!(g.state.rules.stack.is_empty());
            assert!(!g.current(elves).unwrap().tapped);
            assert!(!g.current(source).unwrap().tapped);
            assert!(g.state.rules.players[0].mana.is_empty());
            assert_recoverable((&g).into());
        } else {
            let d = expect(&mut g, 0, reason(ChosenCostAction::Tap));
            assert_eq!(offered_objects(&d), [elves]);
            assert!(!g.current(source).unwrap().tapped);
            assert!(g.current(forest).unwrap().tapped);
            Driver::pick_object(&mut g, elves);
            expect(&mut g, 0, Reason::Priority);
            assert!(g.current(elves).unwrap().tapped && g.current(source).unwrap().tapped);
            settle(&mut g);
            assert_eq!(g.state.rules.tokens.len(), 1);
        }
    }
}

#[test]
fn source_tap_and_discard_pay_together() {
    let prepared = board(&[(GOBLIN, &[Cell::ChosenObjectCost])])
        .permanent_or_hand("Rummaging Goblin", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, true)
        .libraries("Island", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let [source, discard] = [0, 1].map(|n| Driver::object(&g, n));
    Driver::action(&mut g, activate(source, 0));
    expect(&mut g, 0, reason(ChosenCostAction::Discard));
    assert!(!g.current(source).unwrap().tapped);
    Driver::pick_object(&mut g, discard);
    expect(&mut g, 0, Reason::Priority);
    assert!(g.current(source).unwrap().tapped);
    assert_eq!(hand_size(&g, 0), 0);
    settle(&mut g);
    assert_eq!(hand_size(&g, 0), 1);
}

#[test]
fn graveyard_exile_follows_mana_and_moves_only_own_cards() {
    let yard = |owner| Zone::Graveyard {
        owner: PlayerId(owner),
    };
    let mut b = board(&[(MINES, &[Cell::ChosenObjectCost])]).permanent_or_hand(
        "Mines of Moria",
        0,
        0,
        false,
    );
    for _ in 0..4 {
        b = b.permanent_or_hand("Mountain", 0, 0, false);
    }
    let prepared = b
        .card("Island", 0, 0, yard(0))
        .card("Forest", 0, 0, yard(0))
        .card("Plains", 0, 0, yard(0))
        .card("Swamp", 1, 1, yard(1))
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let [source, first, second, third] = [0, 5, 6, 7].map(|n| Driver::object(&g, n));
    Driver::action(&mut g, activate(source, 0));
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let exile = reason(ChosenCostAction::ExileFromGraveyard);
    for object in [first, second, third] {
        let d = expect(&mut g, 0, exile.clone());
        assert_eq!(offered_objects(&d), [object]);
        assert_eq!(zone(&g, 5), Some(yard(0)));
        if object == third {
            let mut forged = g.fork();
            let Frame::ChosenObjectCost { selected, .. } = &mut forged.state.rules.frame else {
                panic!("missing exile selection");
            };
            selected.swap(0, 1);
            assert!(
                forged
                    .validate_state()
                    .unwrap_err()
                    .contains("chosen-object cost progress")
            );
        }
        Driver::pick_object(&mut g, object);
    }
    expect(&mut g, 0, Reason::Priority);
    assert!(g.current(source).unwrap().tapped);
    for card in [5, 6, 7] {
        assert_eq!(zone(&g, card), Some(Zone::Exile));
    }
    assert_eq!(zone(&g, 8), Some(yard(1)));
    assert!(g.state.rules.tokens.is_empty());

    let mut forged = g.fork();
    let moved = Driver::object(&g, 5);
    forged
        .state
        .rules
        .objects
        .get_mut(&moved.card)
        .unwrap()
        .zone = yard(0);
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("chosen-object cost outcome")
    );
    settle(&mut g);
    assert_eq!(g.state.rules.tokens.len(), 2);
}

#[test]
fn rejects_random_discard_and_unaudited_graveyard_source_actions() {
    for raw in [METEOR_STORM, HAUNTED_DEAD] {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        assert!(
            matches!(result, mtg_compiler::CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
}

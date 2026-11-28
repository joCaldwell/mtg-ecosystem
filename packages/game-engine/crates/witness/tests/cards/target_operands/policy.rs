//! The baseline policy answers announced X and target counts within the
//! bounds the targets impose (CR 115.3, 601.2b-c) on frozen R5 sources.
use super::*;
use mtg_sim::policy::{Policy, Strategy};

const DREGS_OF_SORROW: &str = record!("8126e60b-cf3d-4bf0-bfb5-f1e914d3e20e");
const STIR_THE_GRAVE: &str = record!("296dcc57-5651-4cb1-b89c-fcdd3f5311bd");
const BY_FORCE: &str = record!("7ce950fb-e065-45d3-aeff-f8c475460754");

fn observation(g: &Game) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("missing observation for seat 0");
    };
    o
}

/// Seat 0's policy answers each decision until `reason` is pending.
fn drive(g: &mut Game, strategy: &mut Strategy<'_>, reason: Reason<Rules>) -> Decision<Rules> {
    for _ in 0..32 {
        let d = Driver::request(g, u64::MAX);
        if d.reason == reason {
            return d;
        }
        assert_eq!(d.player, PlayerId(0), "{d:?}");
        let answer = strategy.choose_with_game(&observation(g), g).unwrap();
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("no {reason:?} decision");
}

/// The policy's answer to the pending decision, which the rules accept.
fn policy_answer(g: &mut Game, strategy: &mut Strategy<'_>, d: &Decision<Rules>) -> Answer {
    let answer = strategy.choose_with_game(&observation(g), g).unwrap();
    g.respond(d.player, d.id, answer.clone()).unwrap();
    answer
}

/// Dregs of Sorrow: "Destroy X target nonblack creatures. Draw X cards."
/// Eight lands fund X = 3, but the rules cap X at the two legal creatures
/// and the policy announces only the one opposing creature.
#[test]
fn the_policy_caps_exact_x_at_the_targets_it_would_choose() {
    let mut b = board(&[(DREGS_OF_SORROW, &[Cell::RemoveTargetGroup], Tiers::EXECUTES)])
        .permanent_or_hand("Dregs of Sorrow", 0, 0, true)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false);
    for _ in 0..8 {
        b = b.permanent_or_hand("Swamp", 0, 0, false);
    }
    let mut g = Game::with_trace(b.libraries("Island", 4).prepare(), TraceMode::Detailed);
    let catalog = g.prepared.catalog.clone();
    let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
    let x = drive(&mut g, &mut strategy, Reason::AnnounceX);
    assert!(matches!(&x.shape, Shape::Number { max: Some(max), .. } if *max == 2.into()));
    assert_eq!(
        policy_answer(&mut g, &mut strategy, &x),
        Answer::Number { value: 1.into() }
    );
    let d = targets(&mut g);
    let enemy = Driver::object(&g, 2);
    assert_eq!(
        policy_answer(&mut g, &mut strategy, &d),
        Answer::Pick {
            picks: vec![vec![d.index(&Candidate::Object { object: enemy }).unwrap()]]
        }
    );
    drive(&mut g, &mut strategy, Reason::Priority);
    settle(&mut g);
    assert_eq!(zone(&g, 1), Zone::Battlefield);
    assert_eq!(zone(&g, 2), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(
        g.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
            .count(),
        1
    );
}

/// Stir the Grave: "Return target creature card with mana value X or
/// less from your graveyard to the battlefield." The policy announces the
/// smallest X that admits the best card it can fund, never below the
/// rules minimum.
#[test]
fn the_policy_announces_the_mana_value_its_graveyard_target_needs() {
    for (lands, x, offset) in [(3, 2, 1), (7, 6, 2)] {
        let mut b = board(&[(
            STIR_THE_GRAVE,
            &[Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Move)],
            Tiers::EXECUTES,
        )])
        .permanent_or_hand("Stir the Grave", 0, 0, true);
        for _ in 0..lands {
            b = b.permanent_or_hand("Swamp", 0, 0, false);
        }
        let prepared = b
            .card(
                "Grizzly Bears",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .card(
                "Colossal Dreadmaw",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .card(
                "Grizzly Bears",
                1,
                1,
                Zone::Graveyard { owner: PlayerId(1) },
            )
            .libraries("Island", 4)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let catalog = g.prepared.catalog.clone();
        let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
        let announce = drive(&mut g, &mut strategy, Reason::AnnounceX);
        assert!(matches!(&announce.shape, Shape::Number { min, .. } if *min == 2.into()));
        assert_eq!(
            policy_answer(&mut g, &mut strategy, &announce),
            Answer::Number { value: x.into() }
        );
        let d = targets(&mut g);
        let card = Driver::object(&g, lands + offset);
        assert_eq!(
            policy_answer(&mut g, &mut strategy, &d),
            Answer::Pick {
                picks: vec![vec![d.index(&Candidate::Object { object: card }).unwrap()]]
            }
        );
        drive(&mut g, &mut strategy, Reason::Priority);
        settle(&mut g);
        assert_eq!(zone(&g, card.card.0), Zone::Battlefield);
    }
}

/// By Force with X = 0 announces no targets (CR 601.2c), and a fresh
/// policy answering a rules minimum above its own plan raises X to it.
#[test]
fn the_policy_answers_zero_targets_and_the_rules_minimum_x() {
    let prepared = board(&[(BY_FORCE, &[Cell::RemoveTargetGroup], Tiers::EXECUTES)])
        .permanent_or_hand("By Force", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Sol Ring", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let catalog = g.prepared.catalog.clone();
    let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
    Driver::cast(&mut g, CardId(0));
    Driver::answer(&mut g, u64::MAX, Answer::Number { value: 0.into() });
    let d = targets(&mut g);
    assert!(
        matches!(&d.shape, Shape::Pick { slots, .. } if slots[0].min == 0 && slots[0].max == 0)
    );
    assert_eq!(
        policy_answer(&mut g, &mut strategy, &d),
        Answer::Pick {
            picks: vec![vec![]]
        }
    );
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Battlefield);

    // One Swamp funds no X above 1, yet the graveyard sets the minimum at 2.
    let prepared = board(&[(STIR_THE_GRAVE, &[], Tiers::EXECUTES)])
        .permanent_or_hand("Stir the Grave", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .card(
            "Grizzly Bears",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        )
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let catalog = g.prepared.catalog.clone();
    let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
    Driver::cast(&mut g, CardId(0));
    let x = Driver::request(&mut g, u64::MAX);
    assert_eq!(x.reason, Reason::AnnounceX);
    assert_eq!(
        policy_answer(&mut g, &mut strategy, &x),
        Answer::Number { value: 2.into() }
    );
}

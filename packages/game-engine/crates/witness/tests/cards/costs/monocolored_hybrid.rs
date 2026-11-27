//! SPECIAL-MANA: monocolored hybrid symbols `{2/C}` from exact frozen R5
//! source. Each is announced as one mana of its color or {2} before the cost
//! locks, and counts 2 toward mana value in every zone (CR 107.4e, 202.3f,
//! 601.2b, 601.2f-h, 733.1).
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

const SPECTRAL_PROCESSION: &str = record!("376271a6-80f0-47a6-990a-5ec5517aae42");
const REAPER_KING: &str = record!("70034860-5198-421f-871d-7c1676337b6e");

fn white_or_two() -> [Candidate<Rules>; 2] {
    [
        Candidate::HybridPayment {
            payment: HybridPayment::Mana(Color::W),
        },
        Candidate::HybridPayment {
            payment: HybridPayment::PayTwoGeneric,
        },
    ]
}

fn procession() -> Game {
    let prepared = board(&[(SPECTRAL_PROCESSION, &[Cell::HybridManaCost])])
        .permanent_or_hand("Spectral Procession", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .libraries("Island", 4)
        .prepare();
    Game::with_trace(prepared, TraceMode::Detailed)
}

fn battlefield(g: &Game) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield)
        .count()
}

/// Spectral Procession ({2/W}{2/W}{2/W}) and Reaper King
/// ({2/W}{2/U}{2/B}{2/R}{2/G}) have mana values 6 and 10 (CR 202.3f).
#[test]
fn monocolored_hybrid_counts_two_toward_mana_value() {
    for (raw, value) in [(SPECTRAL_PROCESSION, 6), (REAPER_KING, 10)] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_witness::compile(mtg_witness::R5_MANIFEST, raw)
        else {
            panic!("frozen monocolored hybrid card must lower");
        };
        assert_eq!(
            definition.characteristics.printed_mana_value(),
            value.into()
        );
        assert!(definition.characteristics.mana_cost.colored.is_empty());
        assert!(
            definition
                .characteristics
                .hybrid_mana
                .iter()
                .all(|s| s.colors[1] == HybridPayment::PayTwoGeneric)
        );
    }
}

/// White, then {2} twice, costs {W}{4}: a Plains and four Islands pay it,
/// the spell's mana value on the stack stays 6, and three Spirits enter.
#[test]
fn procession_announces_white_or_two_per_symbol() {
    let mut g = procession();
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, Reason::HybridCost);
    assert_eq!(d.candidates, white_or_two());
    let picks = [
        HybridPayment::Mana(Color::W),
        HybridPayment::PayTwoGeneric,
        HybridPayment::PayTwoGeneric,
    ]
    .iter()
    .map(|payment| {
        vec![
            d.index(&Candidate::HybridPayment { payment: *payment })
                .unwrap(),
        ]
    })
    .collect();
    g.respond(d.player, d.id, Answer::Pick { picks }).unwrap();
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{d:?}");
    checked(&g);
    let entry = g.state.rules.stack.last().unwrap();
    assert_eq!(
        entry.total_cost,
        ManaCost {
            generic: 4.into(),
            colored: vec![Color::W],
        }
    );
    assert_eq!(g.stack_mana_value(entry).unwrap(), 6.into());
    let before = battlefield(&g);
    Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
    settle(&mut g);
    assert_eq!(battlefield(&g), before + 3);
}

/// Announcing {2} for every symbol needs six mana; five lands cannot pay,
/// so the cast is reversed (CR 601.2h, 733.1).
#[test]
fn unpayable_monocolored_hybrid_announcement_reverses_the_cast() {
    let mut g = procession();
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, Reason::HybridCost);
    let two = d
        .index(&Candidate::HybridPayment {
            payment: HybridPayment::PayTwoGeneric,
        })
        .unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![two]; 3],
        },
    )
    .unwrap();
    Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(zone(&g, 0), Some(Zone::Hand { owner: PlayerId(0) }));
    checked(&g);
    assert_recoverable((&g).into());
}

/// Develop announces white where the Plains pays it and {2} otherwise, and
/// the engine accepts the plan.
#[test]
fn simulator_plans_monocolored_hybrid_payments() {
    use mtg_sim::policy::{Policy, Strategy};
    let mut g = procession();
    let prepared = g.prepared.clone();
    let mut strategy = Strategy::new(&prepared.catalog, Policy::Develop, [0; 4]);
    let mut announced = vec![];
    for _ in 0..24 {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::Priority && !g.state.rules.stack.is_empty() {
            break;
        }
        let answer = strategy.choose_with_game(&observation(&g, 0), &g).unwrap();
        if let (Reason::HybridCost, Answer::Pick { picks }) = (&d.reason, &answer) {
            announced.extend(picks.iter().map(|p| d.candidates[p[0] as usize].clone()));
        }
        g.respond(d.player, d.id, answer).unwrap();
        checked(&g);
    }
    assert_eq!(
        announced,
        [
            white_or_two()[0].clone(),
            white_or_two()[1].clone(),
            white_or_two()[1].clone()
        ]
    );
    assert_eq!(
        g.state.rules.stack.last().unwrap().paid,
        Some(ManaCost {
            generic: 4.into(),
            colored: vec![Color::W],
        })
    );
}

/// Pearl Medallion reduces white spells by {1} after the announced {2}
/// alternatives are added (CR 601.2f): {2}{2}{2} locks at {5}, which two
/// Sol Rings and a Mountain pay. Develop quotes each announcement through
/// the engine in that order.
#[test]
fn simulator_adds_announced_generic_before_cost_reductions() {
    use mtg_sim::policy::{Policy, Strategy};
    const PEARL_MEDALLION: &str = record!("19015380-1332-4960-8cc6-0732009525a2");
    let prepared = board(&[
        (SPECTRAL_PROCESSION, &[Cell::HybridManaCost]),
        (PEARL_MEDALLION, &[]),
    ])
    .permanent_or_hand("Spectral Procession", 0, 0, true)
    .permanent_or_hand("Pearl Medallion", 0, 0, false)
    .permanent_or_hand("Sol Ring", 0, 0, false)
    .permanent_or_hand("Sol Ring", 0, 0, false)
    .permanent_or_hand("Mountain", 0, 0, false)
    .libraries("Island", 4)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let prepared = g.prepared.clone();
    let mut strategy = Strategy::new(&prepared.catalog, Policy::Develop, [0; 4]);
    let mut announced = vec![];
    for _ in 0..24 {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::Priority && !g.state.rules.stack.is_empty() {
            break;
        }
        let answer = strategy.choose_with_game(&observation(&g, 0), &g).unwrap();
        if let (Reason::HybridCost, Answer::Pick { picks }) = (&d.reason, &answer) {
            announced.extend(picks.iter().map(|p| d.candidates[p[0] as usize].clone()));
        }
        g.respond(d.player, d.id, answer).unwrap();
        checked(&g);
    }
    assert_eq!(announced, vec![white_or_two()[1].clone(); 3]);
    assert_eq!(
        g.state.rules.stack.last().unwrap().paid,
        Some(ManaCost {
            generic: 5.into(),
            colored: vec![],
        })
    );
}

/// The public hybrid quote prices only a complete announcement: one choice
/// per printed symbol (CR 601.2b, 601.2f).
#[test]
fn hybrid_quote_requires_a_complete_announcement() {
    let mut g = procession();
    Driver::request(&mut g, u64::MAX);
    let o = observation(&g, 0);
    let card = o
        .decision
        .as_ref()
        .unwrap()
        .candidates
        .iter()
        .find_map(|c| match c {
            Candidate::Action {
                action: Action::Cast { card },
            } => Some(card.clone()),
            _ => None,
        })
        .unwrap();
    let two = HybridPayment::PayTwoGeneric;
    let white = HybridPayment::Mana(Color::W);
    for partial in [
        &[][..],
        &[two][..],
        &[two, two][..],
        &[two, two, two, two][..],
    ] {
        assert!(
            g.quote_hybrid_cast_cost(PlayerId(0), &card, None, partial)
                .is_err(),
            "{partial:?}"
        );
    }
    assert_eq!(
        g.quote_hybrid_cast_cost(PlayerId(0), &card, None, &[white, two, two])
            .unwrap(),
        ManaCost {
            generic: 4.into(),
            colored: vec![Color::W],
        }
    );
}

/// Merrow Skyswimmer ({3}{W/U}{W/U}, convoke) with Pearl Medallion locks at
/// {2}{W}{W} once white is announced for both symbols (CR 601.2f); Develop
/// quotes that total and convokes four white creatures for it (CR 702.51a).
#[test]
fn simulator_convokes_a_quoted_hybrid_total() {
    use mtg_sim::policy::{Policy, Strategy};
    const PEARL_MEDALLION: &str = record!("19015380-1332-4960-8cc6-0732009525a2");
    const MERROW_SKYSWIMMER: &str = record!("d709f396-021f-49eb-adb5-374aa790eaae");
    let mut board = board(&[
        (MERROW_SKYSWIMMER, &[Cell::HybridManaCost, Cell::Convoke]),
        (PEARL_MEDALLION, &[]),
    ])
    .permanent_or_hand("Merrow Skyswimmer", 0, 0, true)
    .permanent_or_hand("Pearl Medallion", 0, 0, false);
    for _ in 0..4 {
        board = board.permanent_or_hand("Soul's Attendant", 0, 0, false);
    }
    let mut g = Game::with_trace(board.libraries("Island", 4).prepare(), TraceMode::Detailed);
    let prepared = g.prepared.clone();
    let mut strategy = Strategy::new(&prepared.catalog, Policy::Develop, [0; 4]);
    let mut announced = vec![];
    for _ in 0..24 {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::Priority && !g.state.rules.stack.is_empty() {
            break;
        }
        let answer = strategy.choose_with_game(&observation(&g, 0), &g).unwrap();
        if let (Reason::HybridCost, Answer::Pick { picks }) = (&d.reason, &answer) {
            announced.extend(picks.iter().map(|p| d.candidates[p[0] as usize].clone()));
        }
        g.respond(d.player, d.id, answer).unwrap();
        checked(&g);
    }
    let white = Candidate::HybridPayment {
        payment: HybridPayment::Mana(Color::W),
    };
    assert_eq!(announced, [white.clone(), white]);
    let entry = g.state.rules.stack.last().unwrap();
    assert_eq!(zone(&g, 0), Some(Zone::Stack));
    assert_eq!(
        entry.total_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::W, Color::W],
        }
    );
    assert!((2..=5).all(|card| g.current(Driver::object(&g, card)).unwrap().tapped));
}

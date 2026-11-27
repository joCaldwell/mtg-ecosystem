//! SPECIAL-MANA: activated abilities whose costs announce two-color hybrid,
//! single-color Phyrexian and X symbols, from exact frozen R5 source. Each
//! symbol is announced as the ability is activated, before targets, and the
//! total cost adds what was announced (CR 107.3a, 107.4e-f, 119.4, 601.2b,
//! 601.2f-h, 602.2b, 733.1).
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

const AZORIUS_LOCKET: &str = record!("ca13dda2-fbb0-4b6b-9126-3c8df04dee2b");
const KESSIG_WOLF_RUN: &str = record!("c6911265-54ef-4c16-bcf2-1ffb24b7d426");
const INSATIABLE_SOULEATER: &str = record!("f81c0ddc-cd3c-42c0-a7d9-0057b69a5df1");
const MOLTENSTEEL_DRAGON: &str = record!("dc2d67bb-29b3-43d7-8ebf-2100c5287bbf");

fn hybrid(payment: HybridPayment) -> Candidate<Rules> {
    Candidate::HybridPayment { payment }
}

/// Answers a hybrid announcement with one payment per printed symbol.
fn announce(g: &mut Game, payments: &[HybridPayment]) {
    let d = expect(g, 0, Reason::HybridCost);
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("hybrid pick expected");
    };
    assert_eq!(slots.len(), payments.len());
    let picks = payments
        .iter()
        .map(|p| vec![d.index(&hybrid(*p)).unwrap()])
        .collect();
    g.respond(d.player, d.id, Answer::Pick { picks }).unwrap();
}

fn top_cost(g: &Game) -> ManaCost {
    g.state.rules.stack.last().unwrap().total_cost.clone()
}

fn locket(payments: &[HybridPayment]) -> Game {
    let prepared = board(&[(AZORIUS_LOCKET, &[Cell::HybridManaCost])])
        .permanent_or_hand("Azorius Locket", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let source = Driver::object(&g, 0);
    Driver::action(&mut g, activate(source, 0));
    let d = expect(&mut g, 0, Reason::HybridCost);
    let allowed = [
        hybrid(HybridPayment::Mana(Color::W)),
        hybrid(HybridPayment::Mana(Color::U)),
    ];
    assert_eq!(d.candidates, allowed);
    // CR 107.4e: each symbol is announced as one of its two colors.
    assert_eq!(
        top_cost(&g),
        ManaCost {
            generic: Quantity::zero(),
            colored: vec![],
        }
    );
    announce(&mut g, payments);
    g
}

/// Azorius Locket: "{W/U}{W/U}{W/U}{W/U}, {T}, Sacrifice this artifact: Draw
/// two cards." Two white and two blue announcements are paid by two Plains
/// and two Islands; a snapshot inside the payment restores exactly.
#[test]
fn locket_announces_each_hybrid_symbol_before_its_mana_window() {
    use HybridPayment::Mana;
    let mut g = locket(&[
        Mana(Color::W),
        Mana(Color::U),
        Mana(Color::W),
        Mana(Color::U),
    ]);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{d:?}");
    assert_eq!(
        top_cost(&g).colored,
        [Color::W, Color::U, Color::W, Color::U]
    );
    checked(&g);
    let hand = hand_size(&g, 0);
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    settle(&mut g);
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
    assert_eq!(hand_size(&g, 0), hand + 2);
}

/// Four blue announcements cannot be paid by two Plains and two Islands:
/// the activation is reversed and the Locket stays untapped (CR 601.2h,
/// 733.1).
#[test]
fn unpayable_hybrid_activation_is_reversed() {
    let mut g = locket(&[HybridPayment::Mana(Color::U); 4]);
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(zone(&g, 0), Some(Zone::Battlefield));
    assert!(!g.current(Driver::object(&g, 0)).unwrap().tapped);
    checked(&g);
}

fn wolf_run() -> Game {
    let prepared = board(&[(KESSIG_WOLF_RUN, &[Cell::XManaCost])])
        .permanent_or_hand("Kessig Wolf Run", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .libraries("Island", 4)
        .prepare();
    Game::with_trace(prepared, TraceMode::Detailed)
}

/// Kessig Wolf Run: "{X}{R}{G}, {T}: Target creature gets +X/+0 and gains
/// trample until end of turn." X is announced before the target, adds that
/// much generic mana and is the pump (CR 107.3a, 601.2b, 602.2b).
#[test]
fn wolf_run_announces_x_before_its_target_and_pumps_by_it() {
    let mut g = wolf_run();
    let source = Driver::object(&g, 0);
    Driver::action(&mut g, activate(source, 0));
    let d = expect(&mut g, 0, Reason::AnnounceX);
    assert!(matches!(&d.shape, Shape::Number { min, .. } if *min == Quantity::zero()));
    g.respond(d.player, d.id, Answer::Number { value: 2.into() })
        .unwrap();
    let bears = Driver::object(&g, 5);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
    checked(&g);
    let answer = d.pick_one(&Candidate::Object { object: bears }).unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    assert_eq!(
        top_cost(&g),
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::R, Color::G],
        }
    );
    assert_eq!(g.state.rules.stack.last().unwrap().x, Some(2.into()));
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    settle(&mut g);
    let c = g.characteristics(bears).unwrap();
    assert_eq!(c.power, Some(4.into()));
    assert!(g.has_keyword(bears, KeywordAbility::Trample));
}

/// An X of 3 needs five mana; four lands cannot pay it, so the activation
/// is reversed and the Wolf Run stays untapped (CR 601.2h, 733.1).
#[test]
fn unpayable_announced_x_reverses_the_activation() {
    let mut g = wolf_run();
    let source = Driver::object(&g, 0);
    Driver::action(&mut g, activate(source, 0));
    let d = expect(&mut g, 0, Reason::AnnounceX);
    g.respond(d.player, d.id, Answer::Number { value: 3.into() })
        .unwrap();
    let bears = Driver::object(&g, 5);
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert!(g.state.rules.stack.is_empty());
    assert!(!g.current(Driver::object(&g, 0)).unwrap().tapped);
    checked(&g);
}

/// Insatiable Souleater: "{G/P}: This creature gains trample until end of
/// turn." The symbol is paid with {G} or 2 life; a player with 1 life
/// cannot pay 2 life, so that activation is reversed (CR 107.4f, 119.4).
#[test]
fn phyrexian_activation_pays_green_or_two_life() {
    for (start, payment, paid) in [
        (20, HybridPayment::PayTwoLife, true),
        (1, HybridPayment::PayTwoLife, false),
        (1, HybridPayment::Mana(Color::G), true),
    ] {
        let prepared = board(&[(INSATIABLE_SOULEATER, &[Cell::HybridManaCost])])
            .permanent_or_hand("Insatiable Souleater", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .life(PlayerId(0), start.into())
            .libraries("Island", 4)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let souleater = Driver::object(&g, 0);
        Driver::action(&mut g, activate(souleater, 0));
        let d = expect(&mut g, 0, Reason::HybridCost);
        assert_eq!(
            d.candidates,
            [
                hybrid(HybridPayment::Mana(Color::G)),
                hybrid(HybridPayment::PayTwoLife)
            ]
        );
        announce(&mut g, &[payment]);
        // CR 601.2g: 2 life for the whole cost leaves no mana to pay, so no
        // mana window opens; the life is paid (or the activation reversed)
        // at once.
        if payment != HybridPayment::PayTwoLife {
            Driver::pay(&mut g, &[1]);
        }
        let d = Driver::request(&mut g, u64::MAX);
        assert_eq!(d.reason, Reason::Priority);
        checked(&g);
        if paid {
            let spent = if payment == HybridPayment::PayTwoLife {
                2
            } else {
                0
            };
            assert_eq!(life(&g, 0), plus(&start.into(), -spent));
            settle(&mut g);
            assert!(g.has_keyword(souleater, KeywordAbility::Trample));
        } else {
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(life(&g, 0), Quantity::from(1));
            assert!(!g.has_keyword(souleater, KeywordAbility::Trample));
            assert_recoverable((&g).into());
        }
    }
}

/// Moltensteel Dragon ({4}{R/P}{R/P}) has mana value 6 however its symbols
/// are paid (CR 202.3g), and its "{R/P}: This creature gets +1/+0" ability
/// announces its own Phyrexian symbol.
#[test]
fn phyrexian_spell_and_ability_symbols_keep_their_mana_value() {
    let prepared = board(&[(MOLTENSTEEL_DRAGON, &[Cell::HybridManaCost])])
        .permanent_or_hand("Moltensteel Dragon", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let dragon = Driver::object(&g, 0);
    let start = life(&g, 0);
    let definition = g.copiable_definition(dragon).unwrap().clone();
    assert_eq!(definition.characteristics.printed_mana_value(), 6.into());
    assert_eq!(definition.activated[0].hybrid_mana.len(), 1);
    Driver::cast(&mut g, CardId(0));
    announce(
        &mut g,
        &[HybridPayment::PayTwoLife, HybridPayment::PayTwoLife],
    );
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&start, -4));
    let dragon = Driver::object(&g, 0);
    assert_eq!(
        g.characteristics(dragon).unwrap().mana_cost,
        definition.characteristics.mana_cost
    );
    assert_eq!(
        g.copiable_definition(dragon)
            .unwrap()
            .characteristics
            .printed_mana_value(),
        6.into()
    );
    // Untapped lands are gone; 2 life pays the ability's {R/P}.
    Driver::action(&mut g, activate(dragon, 0));
    announce(&mut g, &[HybridPayment::PayTwoLife]);
    assert_eq!(Driver::request(&mut g, u64::MAX).reason, Reason::Priority);
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&start, -6));
    assert_eq!(g.characteristics(dragon).unwrap().power, Some(5.into()));
}

const SILKLASH_SPIDER: &str = record!("2d0c73da-ed8c-44fc-abd9-7c344b1f890d");

/// Drives seat 0 with the Develop policy until its first stack entry, and
/// returns the hybrid and X answers it gave. Every answer is one the engine
/// accepts, and every boundary validates and round-trips.
fn develop(g: &mut Game) -> (Vec<Candidate<Rules>>, Option<Quantity>) {
    use mtg_sim::policy::{Policy, Strategy};
    let prepared = g.prepared.clone();
    let mut strategy = Strategy::new(&prepared.catalog, Policy::Develop, [0; 4]);
    let (mut hybrid, mut x) = (vec![], None);
    for _ in 0..24 {
        let d = Driver::request(g, u64::MAX);
        assert_eq!(d.player, PlayerId(0), "{d:?}");
        if d.reason == Reason::Priority && !g.state.rules.stack.is_empty() {
            return (hybrid, x);
        }
        let answer = strategy.choose_with_game(&observation(g, 0), g).unwrap();
        match (&d.reason, &answer) {
            (Reason::HybridCost, Answer::Pick { picks }) => {
                hybrid.extend(picks.iter().map(|p| d.candidates[p[0] as usize].clone()));
            }
            (Reason::AnnounceX, Answer::Number { value }) => x = Some(value.clone()),
            (Reason::Priority, _) => assert_ne!(
                answer,
                d.pick_one(&Candidate::Action {
                    action: Action::Pass
                })
                .unwrap(),
                "Develop passed instead of activating"
            ),
            _ => {}
        }
        g.respond(d.player, d.id, answer).unwrap();
        checked(g);
    }
    panic!("Develop never put the activation on the stack");
}

/// Silklash Spider: "{X}{G}{G}: This creature deals X damage to each
/// creature with flying." Develop announces the largest X four Forests fund
/// and plans that total, which the engine accepts (CR 107.3a, 601.2f-h).
#[test]
fn simulator_announces_and_funds_an_activation_x() {
    let prepared = board(&[(SILKLASH_SPIDER, &[Cell::XManaCost])])
        .permanent_or_hand("Silklash Spider", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Storm Crow", 1, 1, false)
        .permanent_or_hand("Birds of Paradise", 1, 1, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let (hybrid, x) = develop(&mut g);
    assert!(hybrid.is_empty());
    assert_eq!(x, Some(2.into()));
    assert_eq!(
        g.state.rules.stack.last().unwrap().paid,
        Some(ManaCost {
            generic: 2.into(),
            colored: vec![Color::G, Color::G],
        })
    );
    settle(&mut g);
    assert_eq!(zone(&g, 5), Some(Zone::Graveyard { owner: PlayerId(1) }));
    assert_eq!(zone(&g, 6), Some(Zone::Graveyard { owner: PlayerId(1) }));
}

/// Moltensteel Dragon's "{R/P}: +1/+0": Develop pumps an attack-ready
/// Dragon, announcing red while a Mountain can pay it and 2 life once no
/// mana is left (CR 107.4f, 119.4).
#[test]
fn simulator_announces_a_phyrexian_activation_from_its_plan() {
    let prepared = board(&[(MOLTENSTEEL_DRAGON, &[Cell::HybridManaCost])])
        .permanent_or_hand("Moltensteel Dragon", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let start = life(&g, 0);
    let (hybrid, _) = develop(&mut g);
    assert_eq!(
        hybrid,
        [Candidate::HybridPayment {
            payment: HybridPayment::Mana(Color::R)
        }]
    );
    assert_eq!(
        g.state
            .rules
            .stack
            .last()
            .unwrap()
            .paid
            .as_ref()
            .unwrap()
            .colored,
        [Color::R]
    );
    settle(&mut g);
    let (hybrid, _) = develop(&mut g);
    assert_eq!(
        hybrid,
        [Candidate::HybridPayment {
            payment: HybridPayment::PayTwoLife
        }]
    );
    assert_eq!(life(&g, 0), plus(&start, -2));
}

/// CR 601.2g: the life-only route reaches priority with the ability paid and
/// never offers a mana decision; a later snapshot restores it exactly.
#[test]
fn life_only_phyrexian_activation_opens_no_mana_window() {
    let prepared = board(&[(INSATIABLE_SOULEATER, &[Cell::HybridManaCost])])
        .permanent_or_hand("Insatiable Souleater", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let start = life(&g, 0);
    let souleater = Driver::object(&g, 0);
    Driver::action(&mut g, activate(souleater, 0));
    announce(&mut g, &[HybridPayment::PayTwoLife]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority, "{d:?}");
    let entry = g.state.rules.stack.last().unwrap();
    assert_eq!(entry.paid.as_ref().unwrap().colored, []);
    assert_eq!(life(&g, 0), plus(&start, -2));
    assert!(!g.current(Driver::object(&g, 1)).unwrap().tapped);
    checked(&g);
    settle(&mut g);
}

/// A paid Icy Manipulator activation stays valid below a Kessig Wolf Run
/// activation that is still announcing X: each entry is checked against its
/// own announcement stage, through a snapshot round trip.
#[test]
fn announcing_activation_above_a_paid_one_keeps_both_valid() {
    let prepared = board(&[(KESSIG_WOLF_RUN, &[Cell::XManaCost])])
        .permanent_or_hand("Kessig Wolf Run", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Icy Manipulator", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let icy = Driver::object(&g, 6);
    Driver::action(&mut g, activate(icy, 0));
    let foe = Driver::object(&g, 7);
    Driver::pick_object(&mut g, foe);
    Driver::pay(&mut g, &[4]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!((d.reason, d.player), (Reason::Priority, PlayerId(0)));
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
    let run = Driver::object(&g, 0);
    Driver::action(&mut g, activate(run, 0));
    let d = expect(&mut g, 0, Reason::AnnounceX);
    assert_eq!(g.state.rules.stack.len(), 2);
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    restored.validate_state().unwrap();
    g.respond(d.player, d.id, Answer::Number { value: 1.into() })
        .unwrap();
    let bears = Driver::object(&g, 5);
    Driver::pick_object(&mut g, bears);
    checked(&g);
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    assert_eq!(g.characteristics(bears).unwrap().power, Some(3.into()));
    assert!(g.current(foe).unwrap().tapped);
}

/// Develop does not judge Silklash Spider at X = 1: against one 1/2 flier,
/// only X = 2 kills it, and four Forests fund that X (CR 107.3a, 602.2b).
#[test]
fn simulator_searches_past_a_useless_small_x() {
    let prepared = board(&[(SILKLASH_SPIDER, &[Cell::XManaCost])])
        .permanent_or_hand("Silklash Spider", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Storm Crow", 1, 1, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let (_, x) = develop(&mut g);
    assert_eq!(x, Some(2.into()));
    settle(&mut g);
    assert_eq!(zone(&g, 5), Some(Zone::Graveyard { owner: PlayerId(1) }));
}

//! SPECIAL-MANA: filter lands whose mana ability costs a two-color hybrid
//! symbol, from exact frozen R5 source. The hybrid symbol is announced as one
//! of its colors before the nested payment, which then pays that color.
//! Pinned CR 107.4e, 601.2b/g-h, 602.2b, 605.3a, 733.1:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
use mtg_contracts::{ColorHybridSymbol, HybridPayment, ManaProduction};
use mtg_core::ManaAbilityIdentity;

const MANIFEST: &str =
    include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json");
const FLOODED_GROVE: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/dc974eb4-72b9-4213-887b-8ee684b93420.json"
);
const FETID_HEATH: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/42bf259d-4bb9-49c3-b4ec-223dca62f4d6.json"
);

fn lowered(raw: &str) -> Definition {
    match mtg_witness::compile(MANIFEST, raw) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        result => panic!("frozen filter land must lower: {result:?}"),
    }
}

fn board(cards: &[(&str, u8)]) -> Game {
    board_with(cards, &[])
}

fn board_with(cards: &[(&str, u8)], hand: &[&str]) -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog
        .definitions
        .extend([lowered(FLOODED_GROVE), lowered(FETID_HEATH)]);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut board = Board::new(catalog, "special-mana-filter-land")
        .step(TurnStep::PrecombatMain)
        .seats(2);
    for (name, seat) in cards {
        board = board.permanent_or_hand(name, *seat, *seat, false);
    }
    for name in hand {
        board = board.permanent_or_hand(name, 0, 0, true);
    }
    Game::new(board.libraries("Forest", 4).prepare())
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

fn pool(g: &Game) -> BTreeMap<Color, Quantity> {
    g.state.rules.players[0]
        .mana
        .iter()
        .filter(|(_, n)| **n > Quantity::zero())
        .map(|(c, n)| (*c, n.clone()))
        .collect()
}

fn hybrid(payment: HybridPayment) -> Candidate<Rules> {
    Candidate::HybridPayment { payment }
}

fn answer(g: &mut Game, expected: impl Fn(&Reason<Rules>) -> bool, choice: Candidate<Rules>) {
    let d = Driver::request(g, u64::MAX);
    assert!(expected(&d.reason), "unexpected decision {:?}", d.reason);
    let answer = d.pick_one(&choice).unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    checked(g);
}

fn finish(g: &mut Game) {
    Driver::answer(
        g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
}

/// Activates the grove's filter ability and announces `payment`.
fn announce(g: &mut Game, payment: HybridPayment) {
    let grove = Driver::object(g, 0);
    let activate = g.next_test_decision().activate_mana(grove);
    Driver::answer(g, u64::MAX, activate);
    answer(
        g,
        |r| matches!(r, Reason::ManaAbility { .. }),
        Candidate::ManaAbility {
            ability: ManaAbilityIdentity::Printed { index: 1 },
        },
    );
    let d = Driver::request(g, u64::MAX);
    assert_eq!(d.reason, Reason::HybridCost);
    assert_eq!(
        d.candidates,
        [
            hybrid(HybridPayment::Mana(Color::G)),
            hybrid(HybridPayment::Mana(Color::U))
        ]
    );
    checked(g);
    answer(g, |r| *r == Reason::HybridCost, hybrid(payment));
}

/// "{G/U}, {T}: Add {G}{G}, {G}{U}, or {U}{U}." lowers to one tapped mana
/// ability whose only mana cost is the announced hybrid symbol.
#[test]
fn filter_lands_lower_their_hybrid_activation_cost() {
    for (raw, a, b) in [
        (FLOODED_GROVE, Color::G, Color::U),
        (FETID_HEATH, Color::W, Color::B),
    ] {
        let d = lowered(raw);
        let filter = &d.mana_abilities[1];
        assert_eq!(
            filter.hybrid_mana,
            [ColorHybridSymbol {
                colors: [HybridPayment::Mana(a), HybridPayment::Mana(b)]
            }]
        );
        assert!(filter.costs.tap && filter.costs.mana.is_none());
        assert!(
            matches!(&filter.production, ManaProduction::Bundles { options } if options.len() == 3)
        );
        let cells = definition_cells(&d).unwrap();
        assert!(
            cells.contains(&Cell::HybridManaCost) && cells.contains(&Cell::ManaAbilityManaCost)
        );
        assert_eq!(definition_tiers(&d).unwrap(), Tiers::EXECUTES);
    }
}

/// CR 602.2b, 605.3a, 601.2g-h: green is announced for {G/U}, a Forest pays
/// it through a nested activation, and the grove adds the chosen bundle.
#[test]
fn announced_hybrid_color_is_paid_through_a_nested_activation() {
    let mut g = board(&[("Flooded Grove", 0), ("Forest", 0)]);
    announce(&mut g, HybridPayment::Mana(Color::G));
    let forest = Driver::object(&g, 1);
    let activate = g.next_test_decision().activate_mana(forest);
    Driver::answer(&mut g, u64::MAX, activate);
    checked(&g);
    finish(&mut g);
    let d = Driver::request(&mut g, u64::MAX);
    let Shape::Pay { cost, .. } = &d.shape else {
        panic!("expected the grove's spending window, got {d:?}");
    };
    assert_eq!(cost.colored, [Color::G]);
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::G, 1.into())]),
            },
        },
    );
    answer(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        Candidate::ManaBundle {
            symbols: vec![Color::U, Color::U],
        },
    );
    assert_eq!(Driver::request(&mut g, u64::MAX).reason, Reason::Priority);
    assert_eq!(pool(&g), BTreeMap::from([(Color::U, 2.into())]));
    assert!(g.current(Driver::object(&g, 0)).unwrap().tapped);
    assert!(g.current(Driver::object(&g, 1)).unwrap().tapped);
    assert_recoverable((&g).into());
}

/// CR 601.2g-h, 733.1: blue cannot be paid with a Forest, so finishing
/// cancels the grove's activation without tapping it and undoes the Forest.
#[test]
fn unpayable_announced_hybrid_color_cancels_the_activation() {
    let mut g = board(&[("Flooded Grove", 0), ("Forest", 0)]);
    announce(&mut g, HybridPayment::Mana(Color::U));
    let forest = Driver::object(&g, 1);
    let activate = g.next_test_decision().activate_mana(forest);
    Driver::answer(&mut g, u64::MAX, activate);
    finish(&mut g);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert!(pool(&g).is_empty());
    assert!(!g.current(Driver::object(&g, 0)).unwrap().tapped);
    assert!(!g.current(Driver::object(&g, 1)).unwrap().tapped);
    checked(&g);
}

/// The simulator funds Kalastria Highborn ({B}{B}) with Fetid Heath: it
/// plans the Plains first, announces white for {W/B}, and picks {B}{B}
/// (CR 107.4e, 601.2g-h, 605.3a). Every answer is one the engine accepts.
#[test]
fn simulator_plans_a_filter_land_through_its_hybrid_announcement() {
    use mtg_sim::policy::{Policy, Strategy};
    let mut g = board_with(
        &[("Fetid Heath", 0), ("Plains", 0)],
        &["Kalastria Highborn"],
    );
    let prepared = g.prepared.clone();
    let mut strategy = Strategy::new(&prepared.catalog, Policy::Develop, [0; 4]);
    let mut announced = vec![];
    for _ in 0..16 {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::Priority && g.state.rules.stack.len() == 1 {
            break;
        }
        let QueryResult::Complete(QueryValue::Observation(o)) =
            g.observe(d.player, Budget::unlimited())
        else {
            panic!("missing observation");
        };
        let answer = strategy.choose_with_game(&o, &g).unwrap();
        if d.reason == Reason::HybridCost {
            let Answer::Pick { picks } = &answer else {
                panic!("hybrid pick expected");
            };
            announced.extend(picks.iter().map(|p| d.candidates[p[0] as usize].clone()));
        }
        g.respond(d.player, d.id, answer).unwrap();
        checked(&g);
    }
    assert_eq!(announced, [hybrid(HybridPayment::Mana(Color::W))]);
    let stack = &g.state.rules.stack;
    assert_eq!(stack.len(), 1);
    assert_eq!(
        stack[0].paid.as_ref().unwrap().colored,
        [Color::B, Color::B]
    );
    assert!(g.current(Driver::object(&g, 0)).unwrap().tapped);
    assert!(g.current(Driver::object(&g, 1)).unwrap().tapped);
}

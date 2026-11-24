//! Track 3A wave W3 mana base: complete archived dual, pain, "could produce",
//! any-one-color, Signet, life-cost and life-gain mana sources in private
//! configurations, driven only by accepted decisions.
//! Pinned CR 106.1a, 106.7, 118.3, 119.4, 120.3a, 305.6, 601.2g-h, 605.1a,
//! 605.3a-b, 608.2c, 733.1:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{ManaLandController, ManaProduction};
use mtg_core::ManaAbilityIdentity;

const MANIFEST: &str =
    include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r4.json");
const SOURCES: [&str; 9] = [
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/14d2979d-5728-42d7-a027-0eb1f754655d.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/40b36bc6-c185-4bda-99e7-0118953c2c97.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/27b047e3-0d41-45e2-98e9-9391d7923a1e.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/67f43ac6-2a58-4b53-b5d7-0330e2a252e2.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/9a02a9a7-39d9-4763-85d3-747a0540b60b.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/2fda4fe7-8b0c-489c-a000-6d358e614e34.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/d0ee5bdc-2b69-4b73-9a20-ffcc18783b29.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/1b3d7fce-e9fe-4176-9d9a-472415826cdd.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r4/7190debf-708b-4f41-9714-0d0a5bd5a74e.json"
    ),
];
const LIFE: i64 = 20;

fn catalog() -> Catalog {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in SOURCES {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_witness::compile(MANIFEST, raw)
        else {
            panic!("complete W3 mana source must lower");
        };
        // CR 605.1a: every printed mana ability lowers to a mana ability.
        assert!(!definition.mana_abilities.is_empty() && definition.activated.is_empty());
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    catalog
}

fn definition<'a>(catalog: &'a Catalog, name: &str) -> &'a Definition {
    catalog.definitions.iter().find(|d| d.name == name).unwrap()
}

/// Cards are numbered in order: `(name, controller, in hand)`.
fn prepared(cards: &[(&str, u8, bool)]) -> Prepared {
    let mut board = Board::new(catalog(), "w3-mana-base-private-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2);
    for seat in 0..2 {
        board = board.life(PlayerId(seat), LIFE.into());
    }
    for (name, seat, hand) in cards {
        board = board.permanent_or_hand(name, *seat, *seat, *hand);
    }
    board.libraries("Forest", 4).prepare()
}

fn life(g: &Game, seat: u8) -> Quantity {
    g.state.rules.players[usize::from(seat)].life.clone()
}

fn pool(g: &Game, seat: u8) -> BTreeMap<Color, Quantity> {
    g.state.rules.players[usize::from(seat)]
        .mana
        .iter()
        .filter(|(_, n)| **n > Quantity::zero())
        .map(|(c, n)| (*c, n.clone()))
        .collect()
}

/// State validation and an exact snapshot round trip at a boundary.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

fn activate(g: &mut Game, card: u32) {
    let object = Driver::object(g, card);
    Driver::answer(g, u64::MAX, g.next_test_decision().activate_mana(object));
}

/// The pending decision offers exactly `expected`, then picks `choice`.
fn pick(
    g: &mut Game,
    reason: impl Fn(&Reason<Rules>) -> bool,
    expected: &[Candidate<Rules>],
    choice: Candidate<Rules>,
) {
    let d = Driver::request(g, u64::MAX);
    assert!(reason(&d.reason), "unexpected decision {:?}", d.reason);
    assert_eq!(d.candidates, expected);
    let answer = d.pick_one(&choice).unwrap();
    g.respond(d.player, d.id, answer).unwrap();
}

fn printed(index: u32) -> Candidate<Rules> {
    Candidate::ManaAbility {
        ability: ManaAbilityIdentity::Printed { index },
    }
}

fn color(color: Color) -> Candidate<Rules> {
    Candidate::Color { color }
}

fn colors(list: &[Color]) -> Vec<Candidate<Rules>> {
    list.iter().map(|c| color(*c)).collect()
}

fn priority(g: &mut Game) {
    assert!(matches!(
        Driver::request(g, u64::MAX),
        Decision {
            reason: Reason::Priority,
            player: PlayerId(0),
            ..
        }
    ));
}

#[test]
fn w3_sources_lower_to_exact_productions_costs_and_riders() {
    let c = catalog();
    let talisman = definition(&c, "Talisman of Creativity");
    assert_eq!(talisman.mana_abilities.len(), 2);
    assert_eq!(
        talisman.mana_abilities[0].production,
        ManaProduction::Fixed {
            symbols: vec![Color::C]
        }
    );
    assert_eq!(
        talisman.mana_abilities[1].production,
        ManaProduction::Choose {
            colors: vec![Color::U, Color::R],
            amount: 1.into()
        }
    );
    assert!(matches!(
        talisman.mana_abilities[1].riders.as_slice(),
        [mtg_contracts::kir::Node::Do(
            mtg_contracts::kir::Verb::Damage {
                source: mtg_contracts::kir::Ref::Source,
                ..
            }
        )]
    ));
    assert_eq!(
        definition(&c, "Gilded Lotus").mana_abilities[0].production,
        ManaProduction::Choose {
            colors: vec![Color::W, Color::U, Color::B, Color::R, Color::G],
            amount: 3.into()
        }
    );
    assert_eq!(
        definition(&c, "Exotic Orchard").mana_abilities[0].production,
        ManaProduction::LandsCouldProduce {
            controller: ManaLandController::Opponents,
            colors_only: true
        }
    );
    assert_eq!(
        definition(&c, "Reflecting Pool").mana_abilities[0].production,
        ManaProduction::LandsCouldProduce {
            controller: ManaLandController::You,
            colors_only: false
        }
    );
    let signet = &definition(&c, "Izzet Signet").mana_abilities[0];
    assert!(signet.costs.tap && signet.costs.mana.is_some());
    assert_eq!(
        definition(&c, "Mana Confluence").mana_abilities[0]
            .costs
            .pay_life,
        Some(1.into())
    );
    assert_eq!(
        definition(&c, "Pristine Talisman").mana_abilities[0]
            .riders
            .len(),
        1
    );
    // CR 605.3a: every new shape executes, and none is yet reviewed above it.
    for name in [
        "Talisman of Creativity",
        "Exotic Orchard",
        "Izzet Signet",
        "Mana Confluence",
    ] {
        let tiers = mtg_contracts::kir::definition_tiers(definition(&c, name)).unwrap();
        assert!(!tiers.admits(mtg_contracts::kir::ExecutionTier::HeadlessQualified));
    }
}

/// CR 605.3a, 106.1a, 120.3a: the player chooses which printed ability; the
/// colored one adds the chosen color and then deals 1 damage to its
/// controller, while the colorless one only adds {C}.
fn pain_source(name: &str) {
    let mut g = Game::new(prepared(&[(name, 0, false)]));
    activate(&mut g, 0);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaAbility { .. }),
        &[printed(0), printed(1)],
        printed(1),
    );
    let (a, b) = match name {
        "Talisman of Creativity" => (Color::U, Color::R),
        _ => (Color::G, Color::U),
    };
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        &colors(&[a, b]),
        color(b),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(b, 1.into())]));
    assert_eq!(life(&g, 0), Quantity::from(LIFE - 1));
    assert!(g.current(Driver::object(&g, 0)).unwrap().tapped);
    checked(&g);

    let mut g = Game::new(prepared(&[(name, 0, false)]));
    activate(&mut g, 0);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaAbility { .. }),
        &[printed(0), printed(1)],
        printed(0),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::C, 1.into())]));
    assert_eq!(life(&g, 0), Quantity::from(LIFE));
    checked(&g);
}

#[test]
fn talisman_selects_its_colored_ability_and_damages_its_controller() {
    pain_source("Talisman of Creativity");
}

#[test]
fn pain_land_selects_its_colored_ability_and_damages_its_controller() {
    pain_source("Yavimaya Coast");
}

/// CR 106.1a: one of the three printed colors.
#[test]
fn tri_land_offers_exactly_its_three_colors() {
    let mut g = Game::new(prepared(&[("Crumbling Necropolis", 0, false)]));
    activate(&mut g, 0);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        &colors(&[Color::U, Color::B, Color::R]),
        color(Color::B),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::B, 1.into())]));
    checked(&g);
}

/// CR 106.1a: three mana of the one chosen color.
#[test]
fn gilded_lotus_adds_three_mana_of_one_chosen_color() {
    let mut g = Game::new(prepared(&[("Gilded Lotus", 0, false)]));
    activate(&mut g, 0);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        &colors(&[Color::W, Color::U, Color::B, Color::R, Color::G]),
        color(Color::G),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::G, 3.into())]));
    checked(&g);
}

/// CR 106.7: an opponent's basic lands define the colors, and so does the
/// opponent's own Exotic Orchard, which could produce what its opponent's
/// Forest could; the cycle between the two Orchards adds nothing more.
#[test]
fn exotic_orchard_offers_colors_opponents_lands_could_produce() {
    let mut g = Game::new(prepared(&[
        ("Exotic Orchard", 0, false),
        ("Forest", 0, false),
        ("Mountain", 1, false),
        ("Swamp", 1, false),
        ("Exotic Orchard", 1, false),
    ]));
    activate(&mut g, 0);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        &colors(&[Color::B, Color::R, Color::G]),
        color(Color::G),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::G, 1.into())]));
    checked(&g);
}

/// CR 106.7: with no land an opponent controls, no type of mana is defined
/// and the ability adds nothing, though its cost is paid.
#[test]
fn exotic_orchard_without_opponent_lands_adds_no_mana() {
    let mut g = Game::new(prepared(&[("Exotic Orchard", 0, false)]));
    activate(&mut g, 0);
    priority(&mut g);
    assert!(pool(&g, 0).is_empty());
    assert!(g.current(Driver::object(&g, 0)).unwrap().tapped);
    checked(&g);
}

/// CR 106.7: "any type" includes colorless; a pain land's {C}, {G} and {U}
/// abilities and an Island define the types, ignoring the pain land's cost.
#[test]
fn reflecting_pool_offers_every_type_its_controllers_lands_could_produce() {
    let mut g = Game::new(prepared(&[
        ("Reflecting Pool", 0, false),
        ("Yavimaya Coast", 0, false),
        ("Island", 0, false),
        ("Mountain", 1, false),
    ]));
    activate(&mut g, 0);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        &colors(&[Color::U, Color::G, Color::C]),
        color(Color::C),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::C, 1.into())]));
    checked(&g);
}

/// CR 605.3a, 601.2g-h: the Signet's {1} is paid with a nested mana
/// activation before it adds {U}{R}.
#[test]
fn signet_pays_its_mana_cost_through_a_nested_activation() {
    let mut g = Game::new(prepared(&[
        ("Izzet Signet", 0, false),
        ("Forest", 0, false),
    ]));
    activate(&mut g, 0);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{:?}", d.reason);
    activate(&mut g, 1);
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::G, 1.into())]),
            },
        },
    );
    priority(&mut g);
    assert_eq!(
        pool(&g, 0),
        BTreeMap::from([(Color::U, 1.into()), (Color::R, 1.into())])
    );
    assert!(g.current(Driver::object(&g, 0)).unwrap().tapped);
    assert!(g.current(Driver::object(&g, 1)).unwrap().tapped);
    checked(&g);
}

/// CR 601.2g-h: an unpaid Signet cost pays nothing; undo restores the land.
#[test]
fn unpaid_signet_activation_is_cancelled_without_paying_its_tap_cost() {
    let mut g = Game::new(prepared(&[
        ("Izzet Signet", 0, false),
        ("Forest", 0, false),
    ]));
    activate(&mut g, 0);
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    priority(&mut g);
    assert!(pool(&g, 0).is_empty());
    assert!(!g.current(Driver::object(&g, 0)).unwrap().tapped);
    checked(&g);
}

/// CR 118.3, 119.4: "Pay 1 life" is loss of life and is payable only with
/// at least 1 life.
#[test]
fn mana_confluence_pays_one_life_for_any_color() {
    let mut g = Game::new(prepared(&[("Mana Confluence", 0, false)]));
    activate(&mut g, 0);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        &colors(&[Color::W, Color::U, Color::B, Color::R, Color::G]),
        color(Color::W),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::W, 1.into())]));
    assert_eq!(life(&g, 0), Quantity::from(LIFE - 1));
    checked(&g);

    let mut board = Board::new(catalog(), "w3-mana-base-private-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .life(PlayerId(0), 0.into())
        .life(PlayerId(1), LIFE.into());
    board = board.permanent_or_hand("Mana Confluence", 0, 0, false);
    let mut g = Game::new(board.libraries("Forest", 4).prepare());
    let d = Driver::request(&mut g, u64::MAX);
    let confluence = Driver::object(&g, 0);
    assert!(
        d.index(&Candidate::Action {
            action: Action::ActivateMana { object: confluence }
        })
        .is_none()
    );
}

/// CR 605.1a, 119.3: "Add {C}. You gain 1 life."
#[test]
fn pristine_talisman_adds_colorless_and_gains_one_life() {
    let mut g = Game::new(prepared(&[("Pristine Talisman", 0, false)]));
    activate(&mut g, 0);
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::C, 1.into())]));
    assert_eq!(life(&g, 0), Quantity::from(LIFE + 1));
    checked(&g);
}

/// CR 733.1: reversing an unpayable cast may reverse its mana abilities.
/// Undo restores a plain land; an activation that dealt damage is kept, and
/// with it the whole set.
#[test]
fn reversal_keeps_a_damaging_activation_and_undoes_a_plain_one() {
    for kept in [false, true] {
        let mut cards = vec![("Colossal Dreadmaw", 0, true), ("Forest", 0, false)];
        if kept {
            cards.push(("Yavimaya Coast", 0, false));
        }
        let mut g = Game::new(prepared(&cards));
        Driver::cast(&mut g, CardId(0));
        activate(&mut g, 1);
        if kept {
            activate(&mut g, 2);
            pick(
                &mut g,
                |r| matches!(r, Reason::ManaAbility { .. }),
                &[printed(0), printed(1)],
                printed(1),
            );
            pick(
                &mut g,
                |r| matches!(r, Reason::ManaColor { .. }),
                &colors(&[Color::G, Color::U]),
                color(Color::G),
            );
        }
        Driver::answer(
            &mut g,
            u64::MAX,
            Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: true,
                },
            },
        );
        priority(&mut g);
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(
            g.current(Driver::object(&g, 0)).unwrap().zone,
            Zone::Hand { owner: PlayerId(0) }
        );
        let forest = g.current(Driver::object(&g, 1)).unwrap().tapped;
        if kept {
            assert!(forest);
            assert_eq!(pool(&g, 0), BTreeMap::from([(Color::G, 2.into())]));
            assert_eq!(life(&g, 0), Quantity::from(LIFE - 1));
        } else {
            assert!(!forest);
            assert!(pool(&g, 0).is_empty());
            assert_eq!(life(&g, 0), Quantity::from(LIFE));
        }
        checked(&g);
    }
}

/// PR #203 review: `Pay { Spend }` offers only unrestricted mana, so the
/// payment frame never counts restricted mana (CR 106.6, 601.2h): a Signet
/// whose {1} only restricted mana could pay is cancelled rather than sent to
/// a spend decision no answer satisfies.
#[test]
fn restricted_mana_never_reaches_a_spend_decision() {
    let mut g = Game::new(prepared(&[
        ("Izzet Signet", 0, false),
        ("Forest", 0, false),
    ]));
    let forest = Driver::object(&g, 1);
    g.apply_occurrence(&ProposedEvent::AddMana {
        player: PlayerId(0),
        color: Color::C,
        amount: 1.into(),
        source: forest,
        restriction: Some(mtg_contracts::ManaSpendRestriction::ActivatedAbility),
    })
    .unwrap();
    activate(&mut g, 0);
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        },
    );
    priority(&mut g);
    assert!(!g.current(Driver::object(&g, 0)).unwrap().tapped);
    assert!(pool(&g, 0).is_empty());
}

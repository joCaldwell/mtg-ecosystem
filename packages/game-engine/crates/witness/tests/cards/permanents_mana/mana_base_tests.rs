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
    include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json");
const SOURCES: [&str; 11] = [
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/14d2979d-5728-42d7-a027-0eb1f754655d.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/40b36bc6-c185-4bda-99e7-0118953c2c97.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/27b047e3-0d41-45e2-98e9-9391d7923a1e.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/67f43ac6-2a58-4b53-b5d7-0330e2a252e2.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/9a02a9a7-39d9-4763-85d3-747a0540b60b.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/2fda4fe7-8b0c-489c-a000-6d358e614e34.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/d0ee5bdc-2b69-4b73-9a20-ffcc18783b29.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/1b3d7fce-e9fe-4176-9d9a-472415826cdd.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/7190debf-708b-4f41-9714-0d0a5bd5a74e.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/2cc8c24f-cca8-462c-a5ad-1f8662e69a8a.json"
    ),
    include_str!(
        "../../../../../tests/fixtures/phase1-r5/0895c9b7-ae7d-4bb3-af17-3b75deb50a25.json"
    ),
];
const LIFE: i64 = 20;

/// The witness catalog carries every W3 source; each still lowers to
/// exactly its committed definition.
fn catalog() -> Catalog {
    let catalog = FixtureCatalog::Witness.get().clone();
    for raw in SOURCES {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_witness::compile(MANIFEST, raw)
        else {
            panic!("complete W3 mana source must lower");
        };
        // CR 605.1a: every printed mana ability lowers to a mana ability.
        assert!(!definition.mana_abilities.is_empty() && definition.activated.is_empty());
        assert_eq!(
            catalog.definition_by_oracle_id(&definition.oracle_id),
            Some(definition.as_ref())
        );
    }
    catalog
}

fn definition<'a>(catalog: &'a Catalog, name: &str) -> &'a Definition {
    catalog.definitions.iter().find(|d| d.name == name).unwrap()
}

/// Cards are numbered in order: `(name, controller, in hand)`.
fn prepared(cards: &[(&str, u8, bool)]) -> Prepared {
    prepared_with(2, LIFE, cards)
}

fn prepared_with(seats: u8, life: i64, cards: &[(&str, u8, bool)]) -> Prepared {
    let mut board = Board::new(catalog(), "w3-mana-base-private-witness")
        .step(TurnStep::PrecombatMain)
        .seats(seats);
    for seat in 0..seats {
        board = board.life(PlayerId(seat), if seat == 0 { life } else { LIFE }.into());
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

/// CR 106.6: the nested activation cannot spend mana reserved for a spell.
#[test]
fn spell_only_mana_cannot_pay_a_nested_activation() {
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
        restriction: Some(mtg_contracts::ManaSpendRestriction::ColorlessSpell),
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

/// Review P1: Standing Stones' {1} is paid by Mana Confluence, whose own
/// life payment leaves 0 life, so Stones' "Pay 1 life" is no longer payable.
/// Finishing cancels Stones without a spend decision (CR 601.2g-h); the
/// legal Confluence activation stays (CR 733.1) and state-based actions
/// handle the loss when priority resumes (CR 704.5a).
#[test]
fn nested_life_payment_that_empties_life_cancels_the_outer_activation() {
    let mut g = Game::new(prepared_with(
        3,
        1,
        &[("Standing Stones", 0, false), ("Mana Confluence", 0, false)],
    ));
    activate(&mut g, 0);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{:?}", d.reason);
    activate(&mut g, 1);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaColor { .. }),
        &colors(&[Color::W, Color::U, Color::B, Color::R, Color::G]),
        color(Color::R),
    );
    assert_eq!(life(&g, 0), Quantity::zero());
    checked(&g);
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Priority), "{:?}", d.reason);
    // Player 0 left the game with 0 life; no spend decision was offered.
    assert!(!g.live_players().contains(&PlayerId(0)));
    g.validate_state().unwrap();
}

/// Review P1: the only opponent whose land defined Exotic Orchard's color
/// concedes while the color is being chosen. No type is defined any more,
/// so the paid ability adds no mana (CR 106.7, 800.4a).
#[test]
fn orchard_color_choice_emptied_by_concession_adds_no_mana() {
    let mut g = Game::new(prepared_with(
        3,
        LIFE,
        &[("Exotic Orchard", 0, false), ("Mountain", 1, false)],
    ));
    activate(&mut g, 0);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::ManaColor { .. }));
    assert_eq!(d.candidates, colors(&[Color::R]));
    g.concede(PlayerId(1)).unwrap();
    priority(&mut g);
    assert!(pool(&g, 0).is_empty());
    assert!(g.current(Driver::object(&g, 0)).unwrap().tapped);
    checked(&g);
}

/// Review P1: CR 106.7 is a least fixed point, independent of object order:
/// the opponent's Orchard reads this player's lands, this player's Orchard
/// reads it back, and Reflecting Pool still sees Yavimaya Coast's {C}.
#[test]
fn could_produce_is_independent_of_object_order() {
    for order in [
        ["Reflecting Pool", "Exotic Orchard", "Yavimaya Coast"],
        ["Yavimaya Coast", "Exotic Orchard", "Reflecting Pool"],
        ["Exotic Orchard", "Yavimaya Coast", "Reflecting Pool"],
    ] {
        let mut cards: Vec<(&str, u8, bool)> = vec![("Exotic Orchard", 1, false)];
        cards.extend(order.iter().map(|name| (*name, 0, false)));
        let pool_card = 1 + order.iter().position(|n| *n == "Reflecting Pool").unwrap() as u32;
        let orchard = 1 + order.iter().position(|n| *n == "Exotic Orchard").unwrap() as u32;
        let mut g = Game::new(prepared(&cards));
        activate(&mut g, pool_card);
        pick(
            &mut g,
            |r| matches!(r, Reason::ManaColor { .. }),
            &colors(&[Color::U, Color::G, Color::C]),
            color(Color::C),
        );
        activate(&mut g, orchard);
        pick(
            &mut g,
            |r| matches!(r, Reason::ManaColor { .. }),
            &colors(&[Color::U, Color::G]),
            color(Color::G),
        );
        priority(&mut g);
        checked(&g);
    }
}

/// Review P2: while a mana rider's damage transaction is suspended inside a
/// cast's payment, the unpaid spell remains the incomplete top of the stack
/// and the snapshot validates and round-trips (CR 605.3b, 120.3a).
#[test]
fn suspended_mana_rider_damage_keeps_the_payment_valid_in_snapshots() {
    let mut g = Game::new(prepared(&[
        ("Colossal Dreadmaw", 0, true),
        ("Yavimaya Coast", 0, false),
    ]));
    Driver::cast(&mut g, CardId(0));
    activate(&mut g, 1);
    pick(
        &mut g,
        |r| matches!(r, Reason::ManaAbility { .. }),
        &[printed(0), printed(1)],
        printed(1),
    );
    let d = Driver::request(&mut g, u64::MAX);
    g.respond(d.player, d.id, d.pick_one(&color(Color::G)).unwrap())
        .unwrap();
    let mut suspended = false;
    for _ in 0..64 {
        if matches!(
            &g.state.rules.frame,
            Frame::DamageEffects { transaction }
                if matches!(transaction.continuation, DamageContinuation::Mana { .. })
        ) {
            suspended = true;
            assert_eq!(life(&g, 0), Quantity::from(LIFE));
            checked(&g);
        }
        if !matches!(g.advance(Budget::new(1)), Advance::Suspended { .. }) {
            break;
        }
    }
    assert!(suspended, "the rider damage is a suspended transaction");
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{:?}", d.reason);
    assert_eq!(life(&g, 0), Quantity::from(LIFE - 1));
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::G, 1.into())]));
    checked(&g);
}

/// Review P2: an unrelated player's concession revalidates the pending
/// choice of which printed mana ability to activate; Yavimaya Coast still
/// offers both, so the choice stands (CR 605.3a, 800.4a).
#[test]
fn unrelated_concession_keeps_the_mana_ability_choice() {
    let mut g = Game::new(prepared_with(
        3,
        LIFE,
        &[("Yavimaya Coast", 0, false), ("Mountain", 2, false)],
    ));
    activate(&mut g, 0);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::ManaAbility { .. }));
    g.concede(PlayerId(2)).unwrap();
    checked(&g);
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
        color(Color::U),
    );
    priority(&mut g);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::U, 1.into())]));
    assert_eq!(life(&g, 0), Quantity::from(LIFE - 1));
    checked(&g);
}

#[test]
fn combination_output_is_one_choice_of_a_complete_bundle() {
    let mut catalog = catalog();
    let raw = include_str!(
        "../../../../../tests/fixtures/phase1-r5/d98b4250-3492-4864-9c4c-42db09b3ccd4.json"
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(MANIFEST, raw)
    else {
        panic!("Cascading Cataracts must lower");
    };
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut board = Board::new(catalog, "combination-mana-witness")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("Cascading Cataracts", 0, 0, false);
    for _ in 0..5 {
        board = board.permanent_or_hand("Forest", 0, 0, false);
    }
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    for card in 1..=5 {
        activate(&mut game, card);
    }
    activate(&mut game, 0);
    pick(
        &mut game,
        |reason| matches!(reason, Reason::ManaAbility { .. }),
        &[printed(0), printed(1)],
        printed(1),
    );
    let pay = Driver::request(&mut game, u64::MAX);
    game.respond(
        pay.player,
        pay.id,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    )
    .unwrap();
    let pay = Driver::request(&mut game, u64::MAX);
    game.respond(
        pay.player,
        pay.id,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::G, 5.into())]),
            },
        },
    )
    .unwrap();
    let choice = Driver::request(&mut game, u64::MAX);
    assert_eq!(choice.candidates.len(), 126);
    checked(&game);
    let mut symbols = vec![Color::R, Color::G, Color::R, Color::U, Color::W];
    symbols.sort();
    let answer = choice.pick_one(&Candidate::ManaBundle { symbols }).unwrap();
    game.respond(choice.player, choice.id, answer).unwrap();
    priority(&mut game);
    assert_eq!(
        pool(&game, 0),
        BTreeMap::from([
            (Color::R, 2.into()),
            (Color::G, 1.into()),
            (Color::U, 1.into()),
            (Color::W, 1.into()),
        ])
    );
    checked(&game);
}

#[test]
fn bundle_with_hybrid_activation_input_is_still_rejected() {
    let raw = include_str!(
        "../../../../../tests/fixtures/phase1-r5/dc974eb4-72b9-4213-887b-8ee684b93420.json"
    );
    let result = mtg_witness::compile(MANIFEST, raw);
    assert!(
        !matches!(result, mtg_compiler::CompileResult::Lowered { .. }),
        "a bundle cannot bypass its activation cost"
    );
}

#[test]
fn activation_only_mana_pays_the_signet_and_leaves_the_other_unit() {
    let mut catalog = catalog();
    let raw = include_str!(
        "../../../../../tests/fixtures/phase1-r5/215fd0b4-5817-4335-a0ed-1694e44bde41.json"
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(MANIFEST, raw)
    else {
        panic!("Omen Hawker must lower")
    };
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut game = Game::new(
        Board::new(catalog, "nested restricted mana")
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand("Omen Hawker", 0, 0, false)
            .permanent_or_hand("Izzet Signet", 0, 0, false)
            .libraries("Forest", 4)
            .prepare(),
    );
    activate(&mut game, 0);
    activate(&mut game, 1);
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    let decision = Driver::request(&mut game, u64::MAX);
    let Shape::Pay {
        pool: offered,
        step: PayStep::Spend,
        ..
    } = &decision.shape
    else {
        panic!("expected the Signet's spending window")
    };
    let selected = offered
        .restricted
        .iter()
        .find(|unit| unit.color == Color::C)
        .unwrap()
        .clone();
    game.respond(
        decision.player,
        decision.id,
        Answer::Pay {
            pay: PayAnswer::SpendRestricted {
                unrestricted: BTreeMap::new(),
                restricted: vec![selected],
            },
        },
    )
    .unwrap();
    priority(&mut game);
    assert_eq!(
        pool(&game, 0),
        BTreeMap::from([(Color::U, 1.into()), (Color::R, 1.into())])
    );
    assert_eq!(game.state.rules.players[0].restricted_mana.len(), 1);
    assert_eq!(
        game.state.rules.players[0].restricted_mana[0].color,
        Color::U
    );
    checked(&game);
}

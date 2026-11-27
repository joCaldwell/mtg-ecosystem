//! CR 306.5b, 606.3–4, 508.1b, 120.3c and 704.5i–j.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::evasion::{checked, game, until};
use mtg_core::testkit::*;

pub(super) const JACE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/phase1-r5/cc2b6e58-2d6f-495e-95f3-9496105c4cba.json"
));

fn view(g: &Game, player: u8) -> PermittedView {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.observe(PlayerId(player), Budget::new(u64::MAX))
    else {
        panic!("expected permitted observation");
    };
    observation.view
}

fn resolve(g: &mut Game) -> Decision<Rules> {
    for _ in 0..12 {
        let decision = Driver::request(g, u64::MAX);
        if decision.reason != Reason::Priority || g.state.rules.stack.is_empty() {
            return decision;
        }
        Driver::pass(g);
    }
    panic!("stack did not resolve");
}

fn jace_loyalty(g: &Game, controller: u8) -> Quantity {
    view(g, controller)
        .objects
        .iter()
        .find(|o| {
            o.oracle_id == "cc2b6e58-2d6f-495e-95f3-9496105c4cba" && o.zone == Zone::Battlefield
        })
        .unwrap()
        .loyalty_counters
        .clone()
}

#[test]
fn planeswalker_loyalty_timing_cost_limit_and_restore() {
    let mut g = game(
        &[JACE],
        &[
            ("Jace Beleren", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    let d = resolve(&mut g);
    assert_eq!(jace_loyalty(&g, 0), 3.into());
    let object = Driver::object(&g, 0);
    let action = |ability| Candidate::Action {
        action: Action::ActivateAbility { object, ability },
    };
    assert!(d.index(&action(0)).is_some());
    assert!(d.index(&action(1)).is_some());
    assert!(d.index(&action(2)).is_none());
    Driver::action(&mut g, Action::ActivateAbility { object, ability: 1 });
    let d = Driver::request(&mut g, u64::MAX);
    let answer = d
        .pick_one(&Candidate::Player {
            player: PlayerId(0),
        })
        .unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    let d = resolve(&mut g);
    assert_eq!(jace_loyalty(&g, 0), 2.into());
    assert!(d.index(&action(0)).is_none());
    assert!(d.index(&action(1)).is_none());
    checked(&g);
}

#[test]
fn planeswalker_defender_loses_loyalty_and_dies() {
    let mut g = game(
        &[JACE],
        &[
            ("Grizzly Bears", 0, Zone::Battlefield),
            ("Grizzly Bears", 0, Zone::Battlefield),
            ("Jace Beleren", 1, Zone::Battlefield),
        ],
    );
    let d = until(&mut g, &Reason::Attackers);
    let target = d
        .index(&Candidate::Object {
            object: Driver::object(&g, 2),
        })
        .unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![target], vec![target]],
        },
    )
    .unwrap();
    let d = until(&mut g, &Reason::Blockers);
    g.respond(d.player, d.id, d.block(vec![])).unwrap();
    for _ in 0..16 {
        let d = Driver::request(&mut g, u64::MAX);
        let view = view(&g, 1);
        if view.objects.iter().any(|o| {
            o.oracle_id == "cc2b6e58-2d6f-495e-95f3-9496105c4cba"
                && o.zone == (Zone::Graveyard { owner: PlayerId(1) })
        }) {
            assert_eq!(view.players[1].life, 40.into());
            checked(&g);
            return;
        }
        g.respond(
            d.player,
            d.id,
            d.select(Candidate::Action {
                action: Action::Pass,
            }),
        )
        .unwrap();
    }
    panic!("combat did not put the planeswalker into its owner's graveyard");
}

#[test]
fn planeswalker_legend_choice_keeps_one_copy() {
    let mut g = game(
        &[JACE],
        &[
            ("Jace Beleren", 0, Zone::Battlefield),
            ("Jace Beleren", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2, 3, 4]);
    let d = resolve(&mut g);
    assert!(matches!(d.reason, Reason::LegendRule { .. }));
    let keep = Driver::object(&g, 1);
    Driver::pick_object(&mut g, keep);
    Driver::request(&mut g, u64::MAX);
    assert_eq!(jace_loyalty(&g, 0), 3.into());
    checked(&g);
}

#[test]
fn planeswalker_is_a_player_or_planeswalker_damage_target() {
    let spike = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/phase1-r5/2837888c-bfa8-4955-9334-0605ad409f7e.json"
    ));
    let mut g = game(
        &[JACE, spike],
        &[
            ("Jace Beleren", 1, Zone::Battlefield),
            ("Grizzly Bears", 1, Zone::Battlefield),
            ("Lava Spike", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Mountain", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(2));
    let d = Driver::request(&mut g, u64::MAX);
    let bears = Driver::object(&g, 1);
    assert!(d.index(&Candidate::Object { object: bears }).is_none());
    let jace = Driver::object(&g, 0);
    Driver::pick_object(&mut g, jace);
    Driver::pay(&mut g, &[3]);
    resolve(&mut g);
    assert_eq!(
        view(&g, 1)
            .objects
            .iter()
            .find(|o| o.oracle_id == "cc2b6e58-2d6f-495e-95f3-9496105c4cba")
            .unwrap()
            .zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    checked(&g);
}

#[test]
fn planeswalker_damage_preserves_the_printed_creature_and_color_filter() {
    let fry = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/phase1-r5/3a747893-a514-4689-8486-ecccf654fecb.json"
    ));
    let mut g = game(
        &[JACE, fry],
        &[
            ("Jace Beleren", 1, Zone::Battlefield),
            ("Grizzly Bears", 1, Zone::Battlefield),
            ("Storm Crow", 1, Zone::Battlefield),
            ("Fry", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Mountain", 0, Zone::Battlefield),
            ("Mountain", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(3));
    let d = Driver::request(&mut g, u64::MAX);
    let object = |card| Candidate::Object {
        object: Driver::object(&g, card),
    };
    assert!(d.index(&object(0)).is_some());
    assert!(d.index(&object(1)).is_none());
    assert!(d.index(&object(2)).is_some());
    assert!(
        !d.candidates
            .iter()
            .any(|c| matches!(c, Candidate::Player { .. }))
    );
    let target = Driver::object(&g, 0);
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[4, 5]);
    resolve(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(0)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    checked(&g);
}

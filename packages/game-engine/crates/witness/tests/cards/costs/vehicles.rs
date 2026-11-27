//! CR 301.7, 302.6, 702.122: Vehicles use the stack and retain their
//! printed power and toughness while animated by Crew.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const FERRY: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/09726c3d-f700-4bf4-a9e7-326fe41b0a4a.json"
);

#[test]
fn crew_checks_total_power_and_uses_the_stack() {
    let reason = Reason::ChosenObjectCost {
        action: ChosenCostAction::Tap,
    };
    for crew in [
        &["Llanowar Elves"][..],
        &["Grizzly Bears"][..],
        &["Llanowar Elves", "Llanowar Elves"][..],
    ] {
        for sick_vehicle in [false, true] {
            let mut b = board(&[(FERRY, &[Cell::ChosenObjectCost])]).permanent_or_hand(
                "Skybox Ferry",
                0,
                0,
                sick_vehicle,
            );
            for name in crew {
                b = b.permanent_or_hand(name, 0, 0, true);
            }
            let lands = crew.len() as u32 + 1;
            for _ in 0..7 {
                b = b.permanent_or_hand("Forest", 0, 0, false);
            }
            let mut g = Game::new(b.libraries("Island", 8).prepare());
            let mut next_land = lands;
            let members: Vec<_> = crew
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let card = index as u32 + 1;
                    let cost = if *name == "Grizzly Bears" { 2 } else { 1 };
                    let d = Driver::request(&mut g, u64::MAX);
                    assert!(
                        d.candidates.contains(&Candidate::Action {
                            action: Action::Cast {
                                card: Driver::object(&g, card)
                            }
                        }),
                        "{name} {card}: {d:?}"
                    );
                    Driver::cast(&mut g, CardId(card));
                    Driver::pay(&mut g, &(next_land..next_land + cost).collect::<Vec<_>>());
                    next_land += cost;
                    settle(&mut g);
                    let member = Driver::object(&g, card);
                    assert!(!g.current(member).unwrap().attack_ready);
                    member
                })
                .collect();
            if sick_vehicle {
                Driver::cast(&mut g, CardId(0));
                Driver::pay(&mut g, &(next_land..next_land + 5).collect::<Vec<_>>());
                settle(&mut g);
            }
            let vehicle = Driver::object(&g, 0);
            assert_eq!(g.characteristics(vehicle).unwrap().power, None);
            let activation = Action::ActivateAbility {
                object: vehicle,
                ability: 0,
            };
            let offered = actions(&mut g);
            if crew == ["Llanowar Elves"] {
                assert!(!offered.contains(&activation));
                continue;
            }
            assert!(offered.contains(&activation));
            Driver::action(&mut g, activation);
            for &object in &members {
                let decision = expect(&mut g, 0, reason.clone());
                assert_eq!(decision.candidates, [Candidate::Object { object }]);
                assert!(
                    g.respond(
                        decision.player,
                        decision.id,
                        Answer::Pick {
                            picks: vec![vec![]]
                        }
                    )
                    .is_err()
                );
                Driver::pick_object(&mut g, object);
            }
            pick(&mut g, 0, reason.clone(), &[]);
            expect(&mut g, 0, Reason::Priority);
            assert!(members.iter().all(|id| g.current(*id).unwrap().tapped));
            assert_eq!(g.characteristics(vehicle).unwrap().power, None);
            settle(&mut g);
            let c = g.characteristics(vehicle).unwrap();
            assert!(c.has_type(CardKind::Creature) && c.has_type(CardKind::Artifact));
            assert_eq!((c.power, c.toughness), (Some(4.into()), Some(4.into())));

            for _ in 0..64 {
                let d = Driver::request(&mut g, u64::MAX);
                if g.state.rules.turn.number > 1 {
                    break;
                }
                let answer = match d.reason {
                    Reason::Priority => mtg_witness::temporary::pass(&d),
                    Reason::Attackers => {
                        assert_eq!(d.subjects().contains(&vehicle), !sick_vehicle);
                        let attackers = if sick_vehicle {
                            vec![]
                        } else {
                            vec![(vehicle, PlayerId(1))]
                        };
                        d.attack(attackers)
                    }
                    Reason::Blockers => d.block(vec![]),
                    other => panic!("unexpected decision {other:?}"),
                };
                g.respond(d.player, d.id, answer).unwrap();
            }
            assert!(g.state.rules.turn.number > 1);
            assert_eq!(
                life(&g, 1),
                if sick_vehicle { 40.into() } else { 36.into() }
            );
            let c = g.characteristics(vehicle).unwrap();
            assert!(!c.has_type(CardKind::Creature));
            assert_eq!((c.power, c.toughness), (None, None));
            checked(&g);
        }
    }
}

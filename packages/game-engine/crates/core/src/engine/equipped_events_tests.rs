//! Paid source contracts under small and large rule budgets.
use super::aura_control_tests::prepared_with_sources;
use super::equipped_indestructible_tests::{
    answer, both, equip, object, pay_to_priority, recover, request,
};
use super::temporary_indestructible_tests::{pass, target};
use super::*;

const GLOVES: &str =
    include_str!("../../../../tests/fixtures/308d7868-49f9-47a3-a7b1-4b0332d610f1.json");
const CLAMP: &str =
    include_str!("../../../../tests/fixtures/65986c1b-8e51-4604-b685-d82fa7d1263a.json");
const BLADE: &str =
    include_str!("../../../../tests/fixtures/2ccdc60a-49a9-44b9-a7af-0ebf18b26785.json");
const TEMPLE: &str =
    include_str!("../../../../tests/fixtures/79f94050-d850-41ca-b1db-5ae0cf743f0a.json");
const OMEN: &str =
    include_str!("../../../../tests/fixtures/41960d32-ddb5-42be-94b2-3a2e77ca148d.json");

#[test]
fn slice4_equipped_combat_optional_draw_keeps_the_damage_attachment_witness() {
    let p = prepared_with_sources(
        &[
            ("Rogue's Gloves", 0, 0, false),
            ("Grizzly Bears", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
        ],
        &[GLOVES],
    );
    both(p, |g, q| {
        equip(g, q, 0, 1);
        let bearer = object(g, 1);
        let equipment = object(g, 0);
        let before = g.state.rules.libraries[&PlayerId(0)].len();
        let mut declared = false;
        for _ in 0..100 {
            let r = request(g, q);
            if matches!(
                r,
                Request::OptionalEffect {
                    player: PlayerId(0),
                    ..
                }
            ) {
                let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
                    panic!("missing choice frame")
                };
                let entry = &frame.spell;
                assert_eq!(entry.source, equipment);
                assert!(
                    matches!(&entry.kind,StackKind::Triggered{witness:TriggerEventWitness::Equipped{attachment,occurrence:event},..}
                    if attachment.target==Some(bearer) && matches!(event.as_ref(),TriggerEventWitness::DamageDealt{..}))
                );
                recover(g);
                answer(g, q, Response::OptionalEffect { perform: true });
                request(g, q);
                assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), before - 1);
                return;
            }
            if !declared
                && matches!(
                    r,
                    Request::Attackers {
                        player: PlayerId(0),
                        ..
                    }
                )
            {
                answer(
                    g,
                    q,
                    Response::Attackers {
                        attacks: vec![(bearer, PlayerId(1))],
                    },
                );
                declared = true;
            } else {
                answer(g, q, pass(&r));
            }
        }
        panic!("missing equipped damage trigger");
    });
}

#[test]
fn slice4_equipped_death_uses_pre_movement_attachment_and_draws_twice() {
    both(
        prepared_with_sources(
            &[
                ("Skullclamp", 0, 0, false),
                ("Zephyr Sprite", 0, 0, false),
                ("Forest", 0, 0, false),
            ],
            &[CLAMP],
        ),
        |g, q| {
            let equipment = object(g, 0);
            let bearer = object(g, 1);
            let before = g.state.rules.libraries[&PlayerId(0)].len();
            answer(
                g,
                q,
                Response::ActivateAbility {
                    object: equipment,
                    ability: 0,
                },
            );
            pay_to_priority(g, q, target(g, 1));
            for _ in 0..20 {
                let r = request(g, q);
                if matches!(g.top().map(|a|&a.kind),Ok(StackKind::Triggered{witness:TriggerEventWitness::Equipped{attachment,occurrence:event},..})
                if attachment.target==Some(bearer) && matches!(event.as_ref(),TriggerEventWitness::Moved{..}))
                {
                    assert_ne!(object(g, 1), bearer);
                    assert_eq!(g.current(equipment).unwrap().attached_to, None);
                    recover(g);
                }
                if g.state.rules.stack.is_empty() {
                    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), before - 2);
                    return;
                }
                answer(g, q, pass(&r));
            }
            panic!("missing equipped death resolution");
        },
    );
}

#[test]
fn slice4_attachment_protection_applies_only_to_the_current_bearer() {
    both(
        prepared_with_sources(
            &[
                ("Sword of Fire and Ice", 0, 0, false),
                ("Grizzly Bears", 0, 0, false),
                ("Grizzly Bears", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Lightning Bolt", 0, 0, true),
            ],
            &[BLADE],
        ),
        |g, q| {
            let red = &g.definition(CardId(7)).unwrap().characteristics.clone();
            equip(g, q, 0, 1);
            assert!(g.protected_from(object(g, 1), red));
            assert!(!g.protected_from(object(g, 2), red));
            recover(g);
            equip(g, q, 0, 2);
            assert!(!g.protected_from(object(g, 1), red));
            assert!(g.protected_from(object(g, 2), red));
        },
    );
}

#[test]
fn slice4_land_entry_and_sacrificed_activation_keep_scry_private_and_replayable() {
    both(
        prepared_with_sources(
            &[
                ("Temple of Epiphany", 0, 0, true),
                ("Omen of the Sea", 0, 0, false),
                ("Island", 0, 0, false),
                ("Island", 0, 0, false),
                ("Island", 0, 0, false),
            ],
            &[TEMPLE, OMEN],
        ),
        |g, q| {
            answer(g, q, Response::PlayLand { card: CardId(0) });
            for (amount, paid) in [(1, false), (2, true)] {
                if paid {
                    answer(
                        g,
                        q,
                        Response::ActivateAbility {
                            object: object(g, 1),
                            ability: 0,
                        },
                    );
                    pay_to_priority(g, q, vec![]);
                    assert_eq!(
                        g.current(object(g, 1)).unwrap().zone,
                        Zone::Graveyard { owner: PlayerId(0) }
                    );
                }
                let mut found = false;
                for _ in 0..12 {
                    let r = request(g, q);
                    if let Request::Scry {
                        player, candidates, ..
                    } = &r
                    {
                        assert_eq!(*player, PlayerId(0));
                        assert_eq!(candidates.len(), amount);
                        assert!(g.current(object(g, 0)).unwrap().tapped);
                        for candidate in candidates {
                            assert!(g.handle_for(PlayerId(1), *candidate).is_err());
                        }
                        recover(g);
                        let top = candidates.clone();
                        answer(
                            g,
                            q,
                            Response::Scry {
                                top,
                                bottom: vec![],
                            },
                        );
                        request(g, q);
                        found = true;
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                assert!(found, "missing scry");
            }
        },
    );
}

#[test]
fn slice4_created_treasure_pays_a_real_cost_and_retains_its_template_after_sacrifice() {
    use super::equipped_indestructible_tests::cast;
    const SAILOR: &str =
        include_str!("../../../../tests/fixtures/c4efae03-99de-4439-925b-504602623bfd.json");
    both(
        prepared_with_sources(
            &[
                ("Sailor of Means", 0, 0, true),
                ("Unsummon", 0, 0, true),
                ("Grizzly Bears", 1, 1, false),
                ("Island", 0, 0, false),
                ("Island", 0, 0, false),
                ("Island", 0, 0, false),
            ],
            &[SAILOR],
        ),
        |g, q| {
            cast(g, q, 0, vec![]);
            let (id, token) = g.state.rules.tokens.first_key_value().unwrap();
            let id = *id;
            assert_eq!(token.definition.characteristics.kind, CardKind::Artifact);
            assert_eq!(token.definition.characteristics.subtypes, ["Treasure"]);
            let treasure = object(g, id.0);
            recover(g);
            answer(g, q, Response::Cast { card: CardId(1) });
            let targets = target(g, 2);
            let r = request(g, q);
            let response = match r {
                Request::Targets { .. } => Response::Targets {
                    objects: vec![object(g, 2)],
                },
                Request::EntityTargets { .. } => Response::EntityTargets { entities: targets },
                _ => panic!("missing Unsummon target"),
            };
            answer(g, q, response);
            answer(g, q, Response::ActivateMana { object: treasure });
            let r = request(g, q);
            assert!(matches!(r, Request::ManaColor { .. }));
            recover(g);
            answer(g, q, Response::ManaColor { color: Color::U });
            pay_to_priority(g, q, vec![]);
            assert!(g.current(treasure).is_err());
            assert!(g.state.rules.tokens.contains_key(&id));
            recover(g);
        },
    );
}

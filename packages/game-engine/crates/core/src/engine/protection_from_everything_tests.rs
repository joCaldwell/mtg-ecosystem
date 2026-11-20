//! CR 702.16b–f,j, 615.6, 616.1. The private setup starts Progenitus on the
//! battlefield. Equip, declarations, prevention ordering, and damage use game inputs.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::targeted_discard_tests::public_answer;
use super::*;

#[test]
fn printed_protection_from_everything_orders_colorless_combat_prevention() {
    let prepared = prepared_with_sources(
        &[
            ("Silver Myr", 0, 0, false),
            ("Progenitus", 1, 1, false),
            ("Basilisk Collar", 0, 0, false),
            ("Furnace of Rath", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
        ],
        &[
            include_str!(
                "../../../../tests/fixtures/ramos/2d7e00b6-12f0-4b03-82a6-50e3d1b5395e.json"
            ),
            include_str!("../../../../tests/fixtures/66e8f7f8-3a6d-46ba-837c-b9713ddf7f40.json"),
            include_str!("../../../../tests/fixtures/f5f4dd28-f4ae-4d39-b9b8-6ebfd63c93fe.json"),
        ],
    );
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let (myr, protected, collar) = (object(&game, 0), object(&game, 1), object(&game, 2));
    let defender = PlayerId(1);
    equip(&mut game, 4096, 2, 0);
    assert!(game.has_keyword(myr, KeywordAbility::Lifelink));
    assert!(game.has_keyword(myr, KeywordAbility::Deathtouch));
    assert!(!Game::has_color(&game.characteristics(myr).unwrap()));
    assert!(!game.can_block(myr, protected));
    assert!(game.can_block(protected, myr));
    assert!(
        game.apply_occurrence(&ProposedEvent::Attach {
            object: collar,
            target: Some(protected),
        })
        .is_err()
    );
    assert_eq!(game.current(collar).unwrap().attached_to, Some(myr));

    let mut chose_prevention = false;
    for _ in 0..100 {
        let r = request(&mut game, 4096);
        let response = match &r {
            Request::Attackers { .. } => Response::Attackers {
                attacks: vec![(myr, defender)],
            },
            Request::Blockers { .. } => Response::Blockers {
                blocks: vec![(protected, myr)],
            },
            Request::DamageEffect { player, .. } => {
                assert_eq!(*player, defender);
                let QueryResult::Complete(QueryValue::Decision(Some(
                    PlayerRequest::DamageEffect { candidates, .. },
                ))) = game.player_decision_for(defender, Budget::unlimited())
                else {
                    panic!("affected controller's ordering");
                };
                assert_eq!(candidates.len(), 2);
                assert!(
                    candidates
                        .iter()
                        .any(|c| c.kind == DamageEffectKind::DoubleDamage)
                );
                let prevention = candidates
                    .iter()
                    .find(|c| c.kind == DamageEffectKind::ProtectionFromEverything)
                    .unwrap();
                assert_eq!(prevention.effect_controller, defender);
                assert_eq!(
                    prevention.effect_source.current,
                    Some(game.handle_for(defender, protected).unwrap())
                );
                assert_eq!(
                    prevention.effect_oracle_id,
                    game.definition(protected.card).unwrap().oracle_id
                );
                assert_eq!(prevention.amount_before, 1.into());
                assert_eq!(prevention.amount_after, Quantity::zero());
                assert_eq!(prevention.budget_remaining, None);
                assert!(
                    game.resolve_damage_effect_pair(
                        PlayerId(0),
                        &prevention.effect,
                        &prevention.damage
                    )
                    .is_err()
                );
                public_answer(
                    &mut game,
                    defender,
                    PlayerResponse::DamageEffect {
                        effect: prevention.effect.clone(),
                        damage: prevention.damage.clone(),
                    },
                );
                assert!(matches!(
                    game.advance(Budget::new(1)),
                    Advance::Suspended { .. }
                ));
                recover(&game);
                let mut forged = game.damage_transaction().unwrap().clone();
                let application = forged.applications.last_mut().unwrap();
                let EffectKey::IntrinsicProtectionFromEverything { source } =
                    &mut application.effect
                else {
                    panic!("expected applied intrinsic prevention");
                };
                assert_eq!(*source, protected);
                source.incarnation += 1;
                let entry = &mut forged.entries[application.damage.0 as usize];
                *entry.applied.last_mut().unwrap() = application.effect.clone();
                assert!(game.validate_damage_transaction(&forged).is_err());
                chose_prevention = true;
                continue;
            }
            Request::Priority { .. } if game.turn().step == TurnStep::EndCombat => break,
            Request::Priority { .. } => Response::Pass,
            unexpected => panic!("unexpected combat decision {unexpected:?}"),
        };
        game.respond(r.player(), r.decision_id(), response).unwrap();
    }
    assert!(chose_prevention);
    assert_eq!(game.turn().step, TurnStep::EndCombat);
    assert_eq!(
        game.zone(object(&game, 0)),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    assert_eq!(game.zone(protected), Some(Zone::Battlefield));
    assert_eq!(
        game.current(protected).unwrap().marked_damage,
        Quantity::zero()
    );
    assert!(!game.current(protected).unwrap().deathtouch_damage);
    assert_eq!(game.state.rules.players[0].life, 40.into());
}

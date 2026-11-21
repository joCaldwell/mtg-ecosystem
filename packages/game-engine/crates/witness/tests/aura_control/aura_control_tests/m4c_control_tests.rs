//! UNQUALIFIED drafts using real paid Control Magic/removal and concession.
//! No fake controller writer. Private Prepared roots are not legal pod admission.
use super::*;
const IMOTI: &str =
    include_str!("../../../../../tests/fixtures/ramos/eca32dcd-6845-433e-a631-ed1f0ee78f25.json");
const RAMOS: &str =
    include_str!("../../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json");
const APEX: &str =
    include_str!("../../../../../tests/fixtures/ramos/b4d6747c-3516-4e71-be30-af007c6b1dc4.json");
fn root() -> Prepared {
    let mut rows = vec![
        ("Control Magic", 0, 0, true),
        ("Imoti, Celebrant of Bounty", 1, 1, false),
        ("Ramos, Dragon Engine", 0, 0, true),
        ("Naturalize", 0, 0, true),
        ("Apex Devastator", 0, 0, true),
        ("Disperse", 0, 0, true),
    ];
    lands(&mut rows, "Forest", 0, 40);
    lands(&mut rows, "Island", 0, 12);
    lands(&mut rows, "Forest", 1, 6);
    lands(&mut rows, "Island", 1, 4);
    let mut p = prepared_with_sources(&rows, &[IMOTI, RAMOS, APEX]);
    for seat in 0..4 {
        let owner = PlayerId(seat);
        let count = p.config.cards.iter().filter(|c| c.owner == owner).count();
        for _ in count..100 {
            p.config.cards.push(CardSetup {
                id: CardId(p.config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner,
                controller: owner,
                zone: Zone::Graveyard { owner },
            });
        }
        let mut kept = false;
        for c in p
            .config
            .cards
            .iter_mut()
            .filter(|c| c.owner == owner && matches!(c.zone, Zone::Library { .. }))
        {
            if kept {
                c.zone = Zone::Graveyard { owner };
            }
            kept = true;
        }
    }
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn borrowed(g: &mut Game, q: u64) {
    fund(g, q, "Island", 4);
    cast(g, q, 0, target(g, 1));
    control(g, 1, 0, 1, 1);
    recover(g);
}
fn grant_count(g: &Game) -> usize {
    g.state
        .rules
        .pending_triggers
        .iter()
        .filter(|t| matches!(t.ability, EffectiveTriggerAbility::Granted { .. }))
        .count()
        + g.state
            .rules
            .stack
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    StackKind::Triggered {
                        ability: EffectiveTriggerAbility::Granted { .. },
                        ..
                    }
                )
            })
            .count()
}
#[test]
fn m4c_real_control_magic_transfer_paid_aura_removal_and_source_reentry() {
    both(root(), |g, q| {
        borrowed(g, q);
        let first = object(g, 1);
        fund(g, q, "Forest", 6);
        announce(g, q, 2, vec![]);
        assert_eq!(grant_count(g), 1);
        recover(g);
        fund(g, q, "Forest", 2);
        cast(g, q, 3, target(g, 0));
        control(g, 1, 1, 1, 1);
        // The removal resolved above the original; the frozen grant still
        // belonged to caster zero until it finished its no-hit cascade.
        if g.trace.mode == TraceMode::Detailed {
            assert!(g.events().iter().any(|e| matches!(e,Event::TriggerCollected { player: PlayerId(0), ability: EffectiveTriggerAbility::Granted { grant_source, .. }, .. } if *grant_source == first)));
        }
        fund(g, q, "Forest", 10);
        announce(g, q, 4, vec![]);
        assert_eq!(grant_count(g), 0);
        recover(g);
        resolve(g, q, |_| {});
        // A real return creates a new incarnation. It cannot substitute for
        // the old public/historical grant source or inherit the removed Aura.
        fund(g, q, "Island", 2);
        cast(g, q, 5, target(g, 1));
        assert_ne!(object(g, 1), first);
        recover(g);
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        fund(g, q, "Forest", 4);
        fund(g, q, "Island", 1);
        announce(g, q, 1, vec![]);
        assert_eq!(grant_count(g), 0);
        resolve(g, q, |_| {});
        assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
        assert_ne!(object(g, 1), first);
        control(g, 1, 1, 1, 1);
        recover(g);
    });
}
#[test]
fn m4c_borrowed_grantor_owner_concedes_before_proposal_during_payment_or_after_collection() {
    for when in 0..3 {
        both(root(), |g, q| {
            borrowed(g, q);
            if when == 0 {
                g.concede(PlayerId(1)).unwrap();
                request(g, q);
            }
            fund(g, q, "Forest", 6);
            if when == 1 {
                answer(g, q, Response::Cast { card: CardId(2) });
                assert!(matches!(request(g, q), Request::Mana { .. }));
                recover(g);
                g.concede(PlayerId(1)).unwrap();
                let r = request(g, q);
                assert!(matches!(r, Request::Mana { .. }));
                answer(g, q, Response::FinishMana { undo_mana: false });
                let Request::SpendMana {
                    cost, available, ..
                } = request(g, q)
                else {
                    panic!()
                };
                let mut available = available.clone();
                let mut left = cost.generic.clone();
                let mut spending = BTreeMap::new();
                for (color, n) in &mut available {
                    let used = n.clone().min(left.clone());
                    left.subtract(&used);
                    if used > Quantity::zero() {
                        spending.insert(*color, used);
                    }
                }
                assert_eq!(left, Quantity::zero());
                answer(g, q, Response::SpendMana { spending });
                request(g, q);
            } else {
                announce(g, q, 2, vec![]);
            }
            assert_eq!(grant_count(g), usize::from(when == 2));
            if when == 2 {
                g.concede(PlayerId(1)).unwrap();
                request(g, q);
                assert_eq!(
                    grant_count(g),
                    1,
                    "source owner's departure cannot remove caster's invocation"
                );
                let entry = g
                    .state
                    .rules
                    .stack
                    .iter()
                    .find(|s| {
                        matches!(
                            s.kind,
                            StackKind::Triggered {
                                ability: EffectiveTriggerAbility::Granted { .. },
                                ..
                            }
                        )
                    })
                    .unwrap();
                assert_eq!(entry.controller, PlayerId(0));
                let StackKind::Triggered {
                    grant: Some(provenance),
                    ..
                } = &entry.kind
                else {
                    panic!()
                };
                assert!(g.object_id(provenance.source_facts.object.card).is_none());
            }
            recover(g);
            resolve(g, q, |_| {});
            recover(g);
        });
    }
}

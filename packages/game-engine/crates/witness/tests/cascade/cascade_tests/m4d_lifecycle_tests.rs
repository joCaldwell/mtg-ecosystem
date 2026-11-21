//! Parent private paid lifecycle witnesses; no legal-pod or consumer admission.
use super::m4a_tests::replace;
use super::m4c_tests::{grants, public_proof, settle};
use super::m4d_tests::{flags, root, witness};
use super::*;
fn named(g: &Game, name: &str, zone: Zone) -> ObjectId {
    g.state
        .rules
        .objects
        .values()
        .find(|o| o.zone == zone && g.definition(o.id.card).unwrap().name == name)
        .unwrap()
        .id
}
fn prepared(original: &str, library: &[&str], board: &[&str]) -> Prepared {
    let mut p = root("Control Magic", library, board);
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(lowered(include_str!(
        "../../../../../tests/fixtures/9d8fe3f8-5027-4f9e-999e-e277caa96478.json"
    )));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.config.cards[1].oracle_id = catalog
        .definitions
        .iter()
        .find(|d| d.name == original)
        .unwrap()
        .oracle_id
        .clone();
    p.catalog_digest = digest(catalog).unwrap();
    for name in ["Island", "Island", "Mountain"] {
        replace(&mut p, PlayerId(0), name, Zone::Battlefield);
    }
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn cast_target(g: &mut Game, card: ObjectId, target: ObjectId) -> StackId {
    answer(g, Response::Cast { card: card.card });
    m3c_tests::finish_announcement(g, false, Some(target), None);
    g.state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell() && s.source.card == card.card)
        .unwrap()
        .id
}
#[test]
fn m4d_borrowed_sorcerer_departure_before_or_after_free_child_completion() {
    for when in 0..3 {
        let mut p = prepared("Control Magic", &["Dismantling Blow"], &[]);
        replace(
            &mut p,
            PlayerId(1),
            "Wild-Magic Sorcerer",
            Zone::Battlefield,
        );
        replace(
            &mut p,
            PlayerId(0),
            "Bloodbraid Elf",
            Zone::Hand { owner: PlayerId(0) },
        );
        replace(&mut p, PlayerId(0), "Arcane Signet", Zone::Battlefield);
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Off);
        let sorcerer = named(&g, "Wild-Magic Sorcerer", Zone::Battlefield);
        let magic = named(&g, "Control Magic", Zone::Hand { owner: PlayerId(0) });
        let aura = cast_target(&mut g, magic, sorcerer);
        m4a_tests::resolve_entry(&mut g, aura);
        assert_eq!(g.current(sorcerer).unwrap().controller, PlayerId(0));
        assert_eq!(flags(&g, 0), (true, false));
        public_proof(&g);
        if when == 0 {
            g.concede(PlayerId(1)).unwrap();
            request(&mut g);
        }
        paid(&mut g, "Bloodbraid Elf");
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        let Request::AdditionalCosts { candidates, .. } = request(&mut g) else {
            panic!()
        };
        answer(
            &mut g,
            Response::AdditionalCosts {
                selected: vec![candidates[0].identity.clone()],
            },
        );
        let target = named(&g, "Arcane Signet", Zone::Battlefield);
        answer(
            &mut g,
            Response::Targets {
                objects: vec![target],
            },
        );
        assert!(matches!(request(&mut g), Request::Mana { .. }));
        assert_eq!(
            flags(&g, 0),
            (true, false),
            "proposal never consumes exile history"
        );
        if when == 1 {
            g.concede(PlayerId(1)).unwrap();
            request(&mut g);
        }
        m3c_tests::finish_announcement(&mut g, true, Some(target), None);
        let child = g
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.kind.is_spell() && s.cast_from == Zone::Exile)
            .unwrap()
            .id;
        assert_eq!(flags(&g, 0), (true, true));
        assert_eq!(grants(&g, child).len(), usize::from(when == 2));
        if when == 2 {
            let entry = grants(&g, child)[0];
            assert!(
                matches!(entry.kind, StackKind::Triggered { ability: EffectiveTriggerAbility::Granted { grant_source, .. }, .. } if grant_source == sorcerer)
            );
            assert_eq!(entry.controller, PlayerId(0));
            assert!(witness(&g, child).history.first_from_exile);
            g.concede(PlayerId(1)).unwrap();
            request(&mut g);
            assert_eq!(
                grants(&g, child).len(),
                1,
                "historical grant survives owner departure"
            );
        }
        public_proof(&g);
    }
}
#[test]
fn m4d_nexus_bounce_reentry_preserves_consumed_player_history() {
    let mut p = prepared("Disperse", &["Forest"], &["Maelstrom Nexus"]);
    replace(
        &mut p,
        PlayerId(0),
        "Disperse",
        Zone::Hand { owner: PlayerId(0) },
    );
    for setup in &mut p.config.cards {
        if setup.owner == PlayerId(0)
            && matches!(setup.zone, Zone::Hand { .. })
            && p.catalog
                .definition_by_oracle_id(&setup.oracle_id)
                .unwrap()
                .name
                == "Forest"
        {
            setup.zone = Zone::Graveyard { owner: setup.owner };
        }
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let first = named(&g, "Maelstrom Nexus", Zone::Battlefield);
    let card = named(&g, "Disperse", Zone::Hand { owner: PlayerId(0) });
    let original = cast_target(&mut g, card, first);
    assert_eq!(grants(&g, original).len(), 1);
    assert!(witness(&g, original).history.first_overall);
    public_proof(&g);
    settle(&mut g);
    assert!(g.current(first).is_err());
    let nexus = paid(&mut g, "Maelstrom Nexus");
    assert!(grants(&g, nexus).is_empty());
    settle(&mut g);
    assert_ne!(named(&g, "Maelstrom Nexus", Zone::Battlefield), first);
    let later = paid(&mut g, "Ramos, Dragon Engine");
    assert!(grants(&g, later).is_empty());
    assert_eq!(flags(&g, 0), (true, false));
    public_proof(&g);
    settle(&mut g);
    let number = g.turn().number;
    for _ in 0..160 {
        if g.turn().number > number {
            break;
        }
        let r = request(&mut g);
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                candidates, count, ..
            } => Response::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
            other => panic!("reset turn {other:?}"),
        };
        answer(&mut g, response);
        request(&mut g);
    }
    assert!(g.turn().number > number);
    assert_ne!(g.state.rules.active, Some(PlayerId(0)));
    assert_eq!(flags(&g, 0), (false, false));
    for _ in 0..4 {
        if request(&mut g).player() == PlayerId(0) {
            break;
        }
        answer(&mut g, Response::Pass);
    }
    let card = named(&g, "Disperse", Zone::Hand { owner: PlayerId(0) });
    let target = named(&g, "Maelstrom Nexus", Zone::Battlefield);
    let opponent_turn_cast = cast_target(&mut g, card, target);
    assert_eq!(grants(&g, opponent_turn_cast).len(), 1);
    assert!(witness(&g, opponent_turn_cast).history.first_overall);
    public_proof(&g);
}
#[test]
fn m4d_countered_original_or_grant_never_restores_firstness() {
    for counter_original in [false, true] {
        let mut g = Game::new(prepared(
            "Apex Devastator",
            &["Grizzly Bears"],
            &["Maelstrom Nexus"],
        ));
        let original = paid(&mut g, "Apex Devastator");
        let grant = grants(&g, original)[0].id;
        answer(&mut g, Response::Pass);
        assert_eq!(request(&mut g).player(), PlayerId(1));
        let counter = paid_by(
            &mut g,
            if counter_original {
                "Counterspell"
            } else {
                "Stifle"
            },
            PlayerId(1),
            Some(if counter_original { original } else { grant }),
        );
        m4a_tests::resolve_entry(&mut g, counter);
        assert_eq!(flags(&g, 0), (true, false));
        assert_eq!(flags(&g, 1), (true, false));
        assert_eq!(
            g.state.rules.stack.iter().any(|s| s.id == grant),
            counter_original
        );
        if counter_original {
            offer(&mut g);
            assert_eq!(witness(&g, original).history.first_overall, true);
        }
        public_proof(&g);
    }
}
#[test]
fn m4d_departed_active_consumed_seat_keeps_bits_until_real_turn_transition() {
    let mut p = prepared("Ramos, Dragon Engine", &["Forest"], &[]);
    for setup in &mut p.config.cards {
        if matches!(setup.zone, Zone::Hand { .. })
            && p.catalog
                .definition_by_oracle_id(&setup.oracle_id)
                .unwrap()
                .name
                == "Forest"
        {
            setup.zone = Zone::Graveyard { owner: setup.owner };
        }
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    paid(&mut g, "Ramos, Dragon Engine");
    settle(&mut g);
    let number = g.turn().number;
    g.concede(PlayerId(0)).unwrap();
    g.advance(Budget::new(1));
    assert_eq!(g.turn().number, number);
    assert_eq!(flags(&g, 0), (true, false));
    assert_eq!(g.state.rules.active, None);
    public_proof(&g);
    for _ in 0..160 {
        if g.turn().number != number {
            break;
        }
        assert_eq!(flags(&g, 0), (true, false));
        let r = request(&mut g);
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                candidates, count, ..
            } => Response::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
            other => panic!("departed turn {other:?}"),
        };
        answer(&mut g, response);
        request(&mut g);
    }
    assert!(g.turn().number > number);
    assert!(
        g.state
            .rules
            .cast_history
            .iter()
            .all(|r| !r.any_cast && !r.exile_cast)
    );
    public_proof(&g);
}

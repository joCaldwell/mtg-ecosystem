//! Private exact-source witnesses, CR 406.3, 607.2a and 702.75a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::entry_copy_tests::protege_definition;
use super::*;
const BRIDGE: &str =
    include_str!("../../../../../tests/fixtures/ramos/7cb9e29f-835f-4155-a2a5-4b778866c773.json");
const ACTOR: PlayerId = PlayerId(0);
const OPPONENT: PlayerId = PlayerId(1);

fn bridge_definition() -> Definition {
    let path = std::env::var_os("MTG_LINKED_HIDEAWAY_ARCHIVE_PROBE")
        .expect("supply frozen hideaway archive");
    let archive = std::fs::read_to_string(path).expect("read frozen hideaway archive");
    assert_eq!(
        sha256(archive.as_bytes()),
        "114f814949a6c0ddf680ac569113b15452e9a9efb025cbd79e04f58220d9fe4f"
    );
    let mut rows = archive.lines();
    let manifest = rows.next().unwrap();
    let record = rows.next().unwrap();
    assert!(rows.next().is_none());
    let frozen: serde_json::Value = serde_json::from_str(BRIDGE).unwrap();
    let rebound: serde_json::Value = serde_json::from_str(record).unwrap();
    assert_eq!(rebound["source"], frozen["source"]);
    match mtg_witness::compile(manifest, record) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        failure => panic!("complete rebound Mosswort Bridge: {failure:?}"),
    }
}

fn observe(g: &Game, p: PlayerId) -> Observation {
    let QueryValue::Observation(view) = finish_query(g, g.observe(p, Budget::new(1))) else {
        panic!("expected observation")
    };
    view
}

fn enter(g: &mut Game, bridge: CardId) {
    for _ in 0..128 {
        let r = request(g, 128);
        let reply = if matches!(r, Request::Mulligan { .. }) {
            Response::Mulligan { take: false }
        } else if r.player() == ACTOR && g.main_phase_action(ACTOR) {
            g.respond(ACTOR, r.decision_id(), Response::PlayLand { card: bridge })
                .unwrap();
            return;
        } else {
            pass(&r)
        };
        g.respond(r.player(), r.decision_id(), reply).unwrap();
    }
    panic!("bridge land play missing");
}

fn reach_choice(g: &mut Game) -> Vec<ObjectId> {
    for _ in 0..32 {
        let r = request(g, 128);
        if let Request::Hideaway {
            player, candidates, ..
        } = r
        {
            assert_eq!(player, ACTOR);
            return candidates;
        }
        g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
    }
    panic!("missing hideaway choice");
}

fn choose(g: &mut Game, index: usize) {
    let QueryValue::Decision(Some(PlayerRequest::Hideaway {
        token, candidates, ..
    })) = finish_query(g, g.player_decision_for(ACTOR, Budget::new(1)))
    else {
        panic!("missing private candidates")
    };
    let QueryValue::Validated(receipt) = finish_query(
        g,
        g.check_player_response(
            ACTOR,
            token,
            PlayerResponse::Hideaway {
                object: candidates[index].clone(),
            },
            Budget::new(1),
        ),
    ) else {
        panic!("missing receipt")
    };
    g.submit(receipt).unwrap();
    assert!(matches!(request(g, 128), Request::Priority { .. }));
}

#[test]
#[ignore = "requires MTG_LINKED_HIDEAWAY_ARCHIVE_PROBE"]
fn mosswort_hideaway_real_setup_random_bottom_and_exact_link() {
    let catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-staples-ir.json"
    ))
    .unwrap();
    let (pod, bridge) =
        super::random_discard_tests::seeded_pod_with_card(catalog, bridge_definition());
    let mut g = Game::with_trace(pod, TraceMode::Off);
    enter(&mut g, bridge);
    let looked = reach_choice(&mut g);
    assert_eq!(looked.len(), 4);
    let source = g.object_id(bridge).unwrap();
    assert!(g.current(source).unwrap().tapped);
    let [
        Instruction::Hideaway {
            count: 4,
            source_block: block,
        },
    ] = g.definition(bridge).unwrap().triggered[0]
        .program
        .instructions
        .as_slice()
    else {
        panic!("exact Hideaway 4 entry trigger")
    };
    let block = *block;
    let before = g.state.rules.libraries[&ACTOR].clone();
    let mut rng = g.state.rules.match_state.as_ref().unwrap().random.clone();
    let mut expected = vec![looked[0], looked[2], looked[3]];
    for upper in (2..=3u32).rev() {
        let index = loop {
            if let Some(i) = setup::sample_shuffle_word(rng.word().unwrap(), upper) {
                break i;
            }
        };
        expected.swap((upper - 1) as usize, index as usize);
    }
    assert!(observe(&g, OPPONENT).decision.is_none());
    assert!(looked.iter().all(|id| g.handle_for(OPPONENT, *id).is_err()));
    choose(&mut g, 1);
    let library = &g.state.rules.libraries[&ACTOR];
    assert_eq!(library[..before.len() - 4], before[4..]);
    assert_eq!(
        library[before.len() - 4..],
        expected.iter().map(|o| o.card).collect::<Vec<_>>()
    );
    assert!(expected.iter().all(|o| g.current(*o).is_ok()));
    assert_eq!(
        g.state
            .rules
            .match_state
            .as_ref()
            .unwrap()
            .random
            .word_position,
        rng.word_position
    );
    let hidden = g.object_id(looked[1].card).unwrap();
    assert_ne!(hidden, looked[1]);
    assert_eq!(g.linked_hideaway_cards(source, block), [hidden]);
    assert!(g.characteristics(hidden).is_err());
    recover(&g);
    let row = |g: &Game, viewer| {
        observe(g, viewer)
            .view
            .objects
            .into_iter()
            .find(|o| o.face_down)
            .unwrap()
    };
    let owner = row(&g, ACTOR);
    let opponent = row(&g, OPPONENT);
    assert_eq!(
        owner.oracle_id,
        g.definition(hidden.card).unwrap().oracle_id
    );
    assert!(
        opponent.oracle_id.is_empty()
            && opponent.characteristics.is_none()
            && opponent.mana_ability_used.is_none()
    );
    assert_eq!(
        opponent.exiled_by,
        Some(g.handle_for(OPPONENT, source).unwrap())
    );
    // Existing exact incarnation keeps look permission through control changes.
    // This fixture mutation probes the layer-2 hook separately from replay.
    g.state
        .rules
        .objects
        .get_mut(&source.card)
        .unwrap()
        .default_controller = OPPONENT;
    g.reconcile_controllers().unwrap();
    assert_eq!(row(&g, OPPONENT).oracle_id, owner.oracle_id);
    g.state
        .rules
        .objects
        .get_mut(&source.card)
        .unwrap()
        .default_controller = ACTOR;
    g.reconcile_controllers().unwrap();
    assert_eq!(row(&g, OPPONENT).oracle_id, owner.oracle_id);
    let bounced = g
        .move_object(source, Zone::Hand { owner: ACTOR })
        .unwrap()
        .successor;
    let returned = g.move_object(bounced, Zone::Battlefield).unwrap().successor;
    assert!(g.linked_hideaway_cards(returned, block).is_empty());
    assert!(g.turn_up_linked_hideaway(returned, block, hidden).is_err());
    let receipt = g.turn_up_linked_hideaway(source, block, hidden).unwrap();
    assert!(receipt.matches(source, block, hidden));
    g.restore_hideaway_link(receipt.clone()).unwrap();
    assert_eq!(g.linked_hideaway_cards(source, block), [hidden]);
    let moved = g
        .move_object(hidden, Zone::Hand { owner: ACTOR })
        .unwrap()
        .successor;
    g.move_object(moved, Zone::Exile).unwrap();
    assert!(g.restore_hideaway_link(receipt).is_err());
    assert!(g.linked_hideaway_cards(source, block).is_empty());
}

fn short_library(size: usize) -> Game {
    let mut p = prepared(&[("Forest", 0, 0, true)]);
    let bridge = bridge_definition();
    p.config.cards[0].oracle_id = bridge.oracle_id.clone();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(bridge);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    let mut kept = 0;
    let island = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Island")
        .unwrap()
        .oracle_id
        .clone();
    for card in &mut p.config.cards {
        if card.zone == (Zone::Library { owner: ACTOR }) {
            if kept >= size {
                card.zone = Zone::Hand { owner: ACTOR };
            } else if kept == 1 {
                card.oracle_id = island.clone();
            }
            kept += 1;
        }
    }
    p.profile_digest = digest(&p.config).unwrap();
    Game::new(p)
}

#[test]
#[ignore = "requires the pinned Protege and linked hideaway archive probes"]
fn protege_copied_mosswort_hideaway_link_survives_source_departure() {
    let mut rows = vec![
        ("Forest", 0, 0, true),
        ("Grizzly Bears", 0, 0, true),
        ("Yargle and Multani", 0, 0, false),
        ("Beast Within", 0, 0, true),
    ];
    lands(&mut rows, "Island", 0, 6);
    lands(&mut rows, "Forest", 0, 4);
    let mut pod = prepared(&rows);
    let bridge = bridge_definition();
    let protege = protege_definition();
    pod.config.cards[0].oracle_id = bridge.oracle_id.clone();
    pod.config.cards[1].oracle_id = protege.oracle_id.clone();
    // Cascade returns the remaining land to the library before the copy enters.
    // With at most two cards, neither hideaway needs a random permutation.
    let first_library_card = rows.len() as u32;
    pod.config.cards.retain(|card| {
        card.zone != (Zone::Library { owner: ACTOR }) || card.id.0 < first_library_card + 2
    });
    let catalog = Arc::make_mut(&mut pod.catalog);
    catalog.definitions.extend([bridge, protege]);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();

    let mut g = Game::with_trace(pod, TraceMode::Off);
    enter(&mut g, CardId(0));
    let looked = reach_choice(&mut g);
    assert_eq!(looked.len(), 2);
    choose(&mut g, 0);
    let donor = object(&g, 0);
    let donor_hidden = object(&g, looked[0].card.0);

    announce(&mut g, 128, 1, vec![]);
    for _ in 0..8 {
        answer(&mut g, 128, Response::Pass);
    }
    assert!(
        matches!(request(&mut g, 128), Request::EntryCopy { candidates, .. } if candidates == [donor])
    );
    answer(
        &mut g,
        128,
        Response::EntryCopy {
            object: Some(donor),
        },
    );
    let [copy_candidate]: [ObjectId; 1] = reach_choice(&mut g).try_into().unwrap();
    assert_eq!(copy_candidate.card, looked[1].card);
    let copy = object(&g, 1);
    assert!(g.current(copy).unwrap().tapped);
    choose(&mut g, 0);
    let copy_hidden = object(&g, copy_candidate.card.0);
    assert_eq!(g.linked_hideaway_cards(donor, 0), [donor_hidden]);
    assert_eq!(g.linked_hideaway_cards(copy, 0), [copy_hidden]);
    assert_eq!(g.state.rules.turn.lands_played, 1);
    assert!(g.characteristics(copy_hidden).is_err());

    let mut ready = false;
    for _ in 0..400 {
        let r = request(&mut g, 128);
        if g.active_player() == Some(ACTOR)
            && g.turn().step == TurnStep::Upkeep
            && matches!(r, Request::Priority { player: ACTOR, .. })
        {
            ready = true;
            break;
        }
        g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
    }
    assert!(ready, "copied Bridge should untap before the next draw");
    answer(
        &mut g,
        128,
        Response::ActivateAbility {
            object: copy,
            ability: 0,
        },
    );
    let forest = object(&g, 10);
    answer(&mut g, 128, Response::ActivateMana { object: forest });
    answer(&mut g, 128, Response::FinishMana { undo_mana: false });
    answer(
        &mut g,
        128,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::G, 1.into())]),
        },
    );

    announce(&mut g, 128, 3, vec![TargetEntity::Object { object: copy }]);
    for _ in 0..8 {
        answer(&mut g, 128, Response::Pass);
    }
    assert!(
        matches!(request(&mut g, 128), Request::PlayHideawayLinked { candidates, .. } if candidates == [copy_hidden])
    );
    let successor = object(&g, 1);
    assert_ne!(successor, copy);
    assert_eq!(g.zone(successor), Some(Zone::Graveyard { owner: ACTOR }));
    assert!(g.linked_hideaway_cards(successor, 0).is_empty());
    recover(&g);

    answer(
        &mut g,
        128,
        Response::PlayHideawayLinked {
            object: Some(copy_hidden),
        },
    );
    assert!(matches!(request(&mut g, 128), Request::Priority { .. }));
    assert_eq!(
        g.zone(object(&g, copy_hidden.card.0)),
        Some(Zone::Battlefield)
    );
    assert_eq!(g.state.rules.turn.lands_played, 1);
    assert!(g.linked_hideaway_cards(copy, 0).is_empty());
    assert_eq!(g.linked_hideaway_cards(donor, 0), [donor_hidden]);
}

#[test]
#[ignore = "requires MTG_LINKED_HIDEAWAY_ARCHIVE_PROBE"]
fn mosswort_hideaway_short_library_absent_source_and_private_alternatives() {
    let mut g = short_library(2);
    enter(&mut g, CardId(0));
    let looked = reach_choice(&mut g);
    assert_eq!(looked.len(), 2);
    assert_ne!(
        g.definition(looked[0].card).unwrap().oracle_id,
        g.definition(looked[1].card).unwrap().oracle_id
    );
    let source = g.object_id(CardId(0)).unwrap();
    let source_handle = g.handle_for(OPPONENT, source).unwrap();
    g.state
        .rules
        .objects
        .get_mut(&source.card)
        .unwrap()
        .default_controller = OPPONENT;
    g.reconcile_controllers().unwrap();
    g.move_object(source, Zone::Hand { owner: ACTOR }).unwrap();
    g.refresh_visibility().unwrap();
    let mut alternative = g.fork();
    choose(&mut g, 0);
    choose(&mut alternative, 1);
    assert_eq!(observe(&g, OPPONENT), observe(&alternative, OPPONENT));
    let row = observe(&g, OPPONENT)
        .view
        .objects
        .into_iter()
        .find(|o| o.face_down)
        .unwrap();
    assert_eq!(row.exiled_by, Some(source_handle));
    assert!(row.oracle_id.is_empty());
    assert!(g.current(looked[1]).is_ok());
    assert_eq!(g.state.rules.libraries[&ACTOR], [looked[1].card]);
    let hidden = g.object_id(looked[0].card).unwrap();
    assert_eq!(g.linked_hideaway_cards(source, 0), [hidden]);
    assert!(
        g.linked_hideaway_cards(g.object_id(source.card).unwrap(), 0)
            .is_empty()
    );
}

#[test]
#[ignore = "requires MTG_LINKED_HIDEAWAY_ARCHIVE_PROBE"]
fn mosswort_hideaway_empty_library_and_departed_chooser() {
    let mut g = short_library(0);
    enter(&mut g, CardId(0));
    for _ in 0..32 {
        let r = request(&mut g, 128);
        assert!(matches!(r, Request::Priority { .. }));
        if g.state.rules.stack.is_empty() && g.state.rules.pending_triggers.is_empty() {
            break;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    assert!(g.current(g.object_id(CardId(0)).unwrap()).unwrap().tapped);
    assert!(g.state.rules.stack.is_empty());
    assert!(g.state.rules.objects.values().all(|o| o.hideaway.is_none()));
    recover(&g);

    let mut g = short_library(2);
    enter(&mut g, CardId(0));
    reach_choice(&mut g);
    g.concede(ACTOR).unwrap();
    let r = request(&mut g, 128);
    assert_ne!(r.player(), ACTOR);
    assert!(g.state.rules.objects.values().all(|o| o.hideaway.is_none()));
    recover(&g);
}

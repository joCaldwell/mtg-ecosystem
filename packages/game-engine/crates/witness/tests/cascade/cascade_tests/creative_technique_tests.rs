//! Paid, frozen-source resolution in a private real seeded Commander setup.
use super::*;
use mtg_witness::temporary::pass;

const PLAYER: PlayerId = PlayerId(0);
const SPELL: &str = "Creative Technique";

fn scenario(hit: bool) -> Prepared {
    let archive = std::fs::read_to_string(
        std::env::var_os("MTG_CREATIVE_TECHNIQUE_ARCHIVE_PROBE")
            .expect("pinned two-line Creative Technique archive"),
    )
    .unwrap();
    assert_eq!(
        mtg_contracts::sha256(archive.as_bytes()),
        "86c721c13151e4ed8f82be4154f0202e8fbd337a3454902f45c33979bc7f7a9e"
    );
    let lines: Vec<_> = archive.lines().collect();
    let [manifest, row] = lines.as_slice() else {
        panic!("expected one source");
    };
    assert_eq!(
        mtg_contracts::sha256(row.as_bytes()),
        "e7f74eeafcd6a4ba0a23cdeb1f02cc3b6bbeb643b83bdef426f949904639f49b"
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(manifest, row)
    else {
        panic!("complete Creative Technique must lower");
    };
    assert_eq!(definition.name, SPELL);
    let mut p = prepared("Shardless Agent", &["Grizzly Bears"]);
    p.config.profile = "private-seeded-reveal-and-cast".into();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    for c in &mut p.config.cards {
        if matches!(c.zone, Zone::Command { .. }) {
            continue;
        }
        c.zone = Zone::Library { owner: c.owner };
        if c.owner == PLAYER {
            let name = match c.id.0 {
                1 => SPELL,
                2 if hit => "Yargle and Multani",
                42 => "Sol Ring",
                60 => "Mind Stone",
                n => ["Forest", "Island", "Mountain"][(n % 3) as usize],
            };
            c.oracle_id = catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone();
        }
    }
    finish_seeded_setup(p, 86)
}

fn next_main(g: &mut Game) {
    for _ in 0..500 {
        let r = request(g);
        if matches!(r, Request::Priority { player: PLAYER, .. })
            && g.active_player() == Some(PLAYER)
            && g.turn().step == TurnStep::PrecombatMain
            && g.state.rules.stack.is_empty()
        {
            return;
        }
        let response = if matches!(r, Request::Mulligan { .. }) {
            Response::Mulligan { take: false }
        } else {
            pass(&r)
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("expected private main phase")
}

fn tap(g: &mut Game, card: u32) {
    let object = g.object_id(CardId(card)).unwrap();
    answer(g, Response::ActivateMana { object });
}

fn choose_cast(g: &mut Game, perform: bool) {
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::OptionalEffect {
        token,
        player: PLAYER,
    }))) = g.player_decision_for(PLAYER, Budget::unlimited())
    else {
        panic!("expected controller option")
    };
    let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
        PLAYER,
        token,
        PlayerResponse::OptionalEffect { perform },
        Budget::unlimited(),
    ) else {
        panic!("invalid optional choice")
    };
    g.submit(receipt).unwrap();
}

fn pay_source(g: &mut Game) -> StackId {
    next_main(g);
    answer(g, Response::PlayLand { card: CardId(38) });
    paid(g, "Sol Ring");
    next_main(g);
    tap(g, 42);
    paid(g, "Mind Stone");
    next_main(g);
    let turn = g.turn().number;
    while g.turn().number == turn {
        let r = request(g);
        if g.turn().number == turn {
            g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
        }
    }
    next_main(g);
    answer(g, Response::PlayLand { card: CardId(25) });
    tap(g, 42);
    tap(g, 60);
    let owner = paid(g, SPELL);
    offer(g);
    assert_ne!(g.top().unwrap().id, owner, "decline the real cast trigger");
    choose_cast(g, false);
    for _ in 0..16 {
        let r = request(g);
        if g.top().unwrap().id == owner {
            for _ in 0..4 {
                answer(g, Response::Pass);
            }
            return owner;
        }
        g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
    }
    panic!("missing paid source resolution");
}

fn shuffled<T>(cards: &mut [T], rng: &mut MatchRandom) {
    for i in (1..cards.len()).rev() {
        loop {
            if let Some(j) = setup::sample_shuffle_word(rng.word().unwrap(), i as u32 + 1) {
                cards.swap(i, j as usize);
                break;
            }
        }
    }
}

#[test]
#[ignore = "requires the pinned MTG_CREATIVE_TECHNIQUE_ARCHIVE_PROBE"]
fn creative_technique_paid_shuffle_reveal_and_cast() {
    for hit in [true, false] {
        let mut g = Game::with_trace(scenario(hit), TraceMode::Off);
        let owner = pay_source(&mut g);
        let initial = g.state.rules.match_state.as_ref().unwrap().clone();
        let mut rng = initial.random.clone();
        let mut shuffled_library = g.state.rules.libraries[&PLAYER].clone();
        shuffled(&mut shuffled_library, &mut rng);
        let count = if hit {
            shuffled_library
                .iter()
                .position(|c| *c == CardId(2))
                .unwrap()
        } else {
            shuffled_library.len()
        };
        assert!(count >= 2);
        let prefix = &shuffled_library[..count];
        let mut bottom = prefix.to_vec();
        shuffled(&mut bottom, &mut rng);
        assert_ne!(&bottom, prefix);
        let expected: Vec<_> = shuffled_library[count + usize::from(hit)..]
            .iter()
            .chain(&bottom)
            .copied()
            .collect();
        let incarnations: Vec<_> = prefix.iter().map(|c| g.object_id(*c).unwrap()).collect();
        let mut revealed = 0;
        let mut reached_boundary = false;
        for _ in 0..500 {
            let step = g.advance(Budget::new(1));
            // The whole-library shuffle and the private bottom swaps publish no
            // intermediate order. Every newly public position is the next card.
            let positions = g.state.knowledge.visibility[1]
                .history
                .last()
                .unwrap()
                .to_public()
                .known_library_positions;
            if positions.len() > revealed {
                assert_eq!(positions.len(), revealed + 1);
                assert_eq!(
                    g.state.rules.match_state.as_ref().unwrap().library_shuffles[0],
                    initial.library_shuffles[0] + 1
                );
                for (i, row) in positions.iter().enumerate() {
                    assert_eq!(row.library_owner, PLAYER);
                    assert_eq!(row.position as usize, i + 1);
                    assert_eq!(
                        row.oracle_id,
                        g.state.rules.cards[&shuffled_library[i]].oracle_id
                    );
                }
                revealed = positions.len();
            }
            match step {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(Request::OptionalEffect { player: PLAYER, .. })
                    if hit =>
                {
                    reached_boundary = true;
                    break;
                }
                Advance::AwaitingDecision(Request::Priority { .. }) if !hit => {
                    reached_boundary = true;
                    break;
                }
                other => panic!("unexpected reveal boundary {other:?}"),
            }
        }
        assert!(reached_boundary);
        assert_eq!(revealed, count + usize::from(hit));
        assert_eq!(g.state.rules.libraries[&PLAYER], expected);
        assert_eq!(g.state.rules.match_state.as_ref().unwrap().random, rng);
        for land in incarnations {
            assert_eq!(
                g.current(land).unwrap().zone,
                Zone::Library { owner: PLAYER }
            );
        }
        assert!(g.known_library_positions(PlayerId(1)).unwrap().is_empty());
        if !hit {
            assert!(g.state.rules.stack.is_empty());
            assert!(
                g.state
                    .rules
                    .objects
                    .values()
                    .all(|o| o.zone != Zone::Exile)
            );
            continue;
        }
        let winner = g.object_id(CardId(2)).unwrap();
        assert_eq!(g.current(winner).unwrap().zone, Zone::Exile);
        assert_eq!(g.top().unwrap().id, owner);
        assert!(g.request_header(PlayerId(1)).unwrap().is_none());
        let QueryResult::Complete(QueryValue::Observation(view)) =
            g.observe(PLAYER, Budget::unlimited())
        else {
            panic!("expected caster observation");
        };
        assert_eq!(view.view.players[0].exile.len(), 1);
        assert_eq!(
            view.decision_context
                .unwrap()
                .instruction
                .unwrap()
                .source_block,
            1
        );
        let mut decline = g.fork();
        choose_cast(&mut decline, false);
        assert!(matches!(request(&mut decline), Request::Priority { .. }));
        assert_eq!(decline.current(winner).unwrap().zone, Zone::Exile);
        assert!(decline.state.rules.stack.is_empty());
        let mut departure = g.fork();
        choose_cast(&mut departure, true);
        departure.concede(PLAYER).unwrap();
        assert!(matches!(request(&mut departure), Request::Priority { .. }));
        assert!(departure.state.rules.stack.is_empty());
        choose_cast(&mut g, true);
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        assert!(matches!(g.state.rules.frame, Frame::Payment { .. }));
        prove(&g);
        assert!(matches!(request(&mut g), Request::Priority { .. }));
        let [child] = g.state.rules.stack.as_slice() else {
            panic!("only the completed free child remains");
        };
        assert_eq!(child.source.card, winner.card);
        assert_eq!(child.source.incarnation, winner.incarnation + 1);
        assert_eq!(child.cast_from, Zone::Exile);
        assert_eq!(child.paid.as_ref(), Some(&zero_mana_cost()));
        assert_eq!(
            child.alternative_cost,
            Some(cascade::AlternativeCost::WithoutManaCost)
        );
        assert!(g.state.rules.cast_history[0].exile_cast);
        assert!(g.stack_mana_value(child).unwrap() > Quantity::from(5));
    }
}

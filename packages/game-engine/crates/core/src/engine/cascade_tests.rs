//! Private complete-source paid witnesses. No reviewed profile/pod admission.
//! Midgame roots below are seedless; randomized multi-card groups require the
//! separate real-setup witnesses, never an injected midgame recovery seed.
use super::*;
const MANIFEST: &str =
    include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
const AGENT: &str =
    include_str!("../../../../tests/fixtures/ramos/2afbaa9a-c171-4a8b-90f3-5250d8498356.json");
const ELF: &str =
    include_str!("../../../../tests/fixtures/ramos/3f0c9466-5ab9-4205-a84f-b4b27b5a678e.json");
fn lowered(raw: &str) -> Definition {
    match mtg_compiler::compile(MANIFEST, raw) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        other => panic!("complete source: {other:?}"),
    }
}
fn prepared(original: &str, library: &[&str]) -> Prepared {
    prepared_with_records(original, library, &[])
}
fn prepared_with_records(original: &str, library: &[&str], extra: &[&str]) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    for raw in [
        AGENT,
        ELF,
        include_str!("../../../../tests/fixtures/ramos/8b755881-a72d-4e21-a369-d2924eb4585a.json"),
        include_str!("../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json"),
        include_str!("../../../../tests/fixtures/ramos/bc71ebf6-2056-41f7-be35-b2e5c34afa99.json"),
        include_str!("../../../../tests/fixtures/ramos/a3fb7228-e76b-4e96-a40e-20b5fed75685.json"),
    ] {
        let d = lowered(raw);
        if !c.definitions.iter().any(|old| old.oracle_id == d.oracle_id) {
            c.definitions.push(d);
        }
    }
    for path in [
        include_str!("../../../../tests/fixtures/counterspell.json"),
        include_str!("../../../../tests/fixtures/stifle.json"),
        include_str!("../../../../tests/fixtures/grizzly-bears.json"),
        include_str!("../../../../tests/fixtures/unsummon.json"),
        include_str!("../../../../tests/fixtures/healing-salve.json"),
        include_str!("../../../../tests/fixtures/village-rites.json"),
        include_str!("../../../../tests/fixtures/bone-splinters.json"),
        include_str!("../../../../tests/fixtures/earthquake.json"),
    ] {
        let d = lowered(path);
        if !c.definitions.iter().any(|old| old.oracle_id == d.oracle_id) {
            c.definitions.push(d);
        }
    }
    let mtg_compiler::CompileResult::Lowered { definition: kutzil } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/3233af43-7826-4895-b67e-03c6102c2cd5.json"
        ),
    ) else {
        panic!("complete prohibition source")
    };
    c.definitions.push(*kutzil);
    for raw in extra {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete combined source must lower")
        };
        c.definitions.push(*definition);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let id = |name: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .oracle_id
            .clone()
    };
    let mut cards = vec![];
    let mut commanders = vec![];
    for seat in 0..4 {
        let owner = PlayerId(seat);
        commanders.push(CardId(cards.len() as u32));
        cards.push(CardSetup {
            id: CardId(cards.len() as u32),
            oracle_id: id(if seat == 0 {
                "Ramos, Dragon Engine"
            } else {
                "Yargle and Multani"
            }),
            owner,
            controller: owner,
            zone: Zone::Command { owner },
        });
        let mut add = |name: &str, zone: Zone| {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: id(name),
                owner,
                controller: owner,
                zone,
            })
        };
        if seat == 0 {
            add(original, Zone::Hand { owner });
            for name in library {
                add(name, Zone::Library { owner });
            }
            for name in [
                "Forest",
                "Forest",
                "Forest",
                "Forest",
                "Island",
                "Island",
                "Mountain",
                "Mountain",
                "Wall of Wood",
            ] {
                add(name, Zone::Battlefield);
            }
            for _ in 0..89 - library.len() {
                add("Forest", Zone::Hand { owner });
            }
        } else {
            for _ in 0..99 {
                add("Forest", Zone::Library { owner });
            }
        }
    }
    assert_eq!(cards.len(), 400);
    let config = ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::PrecombatMain,
            ..Default::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "cascade-private-source-witness".into(),
        commanders,
        cards,
    };
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(c),
        config,
        match_config: None,
    }
}

fn request(g: &mut Game) -> Request {
    loop {
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            other => panic!("{other:?}"),
        }
    }
}
fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn prove(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace, g.trace);
}
fn paid(g: &mut Game, name: &str) -> StackId {
    paid_by(g, name, PlayerId(0), None)
}
fn paid_by(g: &mut Game, name: &str, player: PlayerId, target: Option<StackId>) -> StackId {
    let source = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            g.definition(o.id.card).unwrap().name == name
                && matches!(
                    o.zone,
                    Zone::Hand { owner } | Zone::Command { owner } if owner == player
                )
        })
        .unwrap()
        .id;
    answer(g, Response::Cast { card: source.card });
    loop {
        let r = request(g);
        let response = match &r {
            Request::Mana { cost, .. } => {
                if g.can_pay(r.player(), cost).unwrap() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let needed = cost
                        .colored
                        .iter()
                        .find(|c| {
                            let required =
                                cost.colored.iter().filter(|symbol| *symbol == *c).count();
                            g.mana(r.player(), **c).unwrap_or_default()
                                < Quantity::from(
                                    i64::try_from(required).expect("test symbol count"),
                                )
                        })
                        .copied();
                    let source = g
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| {
                            o.zone == Zone::Battlefield
                                && o.controller == r.player()
                                && !o.tapped
                                && g.definition(o.id.card)
                                    .unwrap()
                                    .characteristics
                                    .has_type(CardKind::Land)
                                && g.validate_mana_activation(r.player(), o.id).is_ok()
                                && g.mana_abilities(o.id).is_ok_and(|v| {
                                    !v.is_empty() && needed.is_none_or(|c| v.contains(&c))
                                })
                        })
                        .min_by_key(|o| {
                            usize::from(
                                needed.is_none()
                                    && g.mana_abilities(o.id).unwrap().contains(&Color::U),
                            )
                        })
                        .unwrap()
                        .id;
                    Response::ActivateMana { object: source }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut remaining = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for color in &cost.colored {
                    remaining.get_mut(color).unwrap().subtract(&1.into());
                    spending.entry(*color).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (color, n) in remaining {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(color).or_default().add(&used);
                    }
                }
                Response::SpendMana { spending }
            }
            Request::StackTargets { .. } => Response::StackTargets {
                entries: vec![target.expect("explicit counter target")],
            },
            Request::Targets { candidates, .. } => {
                assert_eq!(candidates.len(), 1, "explicit sole creature target");
                Response::Targets {
                    objects: candidates.clone(),
                }
            }
            Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                order: candidates.clone(),
            },
            Request::Priority { .. } => break,
            other => panic!("paid source request {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    let spell = g
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.source.card == source.card && s.kind.is_spell())
        .unwrap();
    assert!(spell.paid.is_some());
    spell.id
}
fn offer(g: &mut Game) {
    for _ in 0..20 {
        let r = request(g);
        if matches!(r, Request::OptionalEffect { .. }) {
            return;
        }
        assert!(matches!(r, Request::Priority { .. }));
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("offer not reached");
}

#[test]
fn cast_mana_value_sweep_counts_paid_and_free_casts_at_resolution() {
    let mut prepared = prepared("Grizzly Bears", &["Sol Ring", "Wall of Wood"]);
    let wounded = m4a_tests::replace(
        &mut prepared,
        PlayerId(1),
        "Wall of Wood",
        Zone::Battlefield,
    );
    let caster = PlayerId(0);
    let archive = std::env::var_os("MTG_CAST_MANA_VALUE_SWEEP_ARCHIVE_PROBE")
        .map(|path| std::fs::read_to_string(path).expect("read the pinned one-record archive"));
    if let Some(archive) = &archive {
        let mut lines = archive.lines();
        let manifest = lines.next().unwrap();
        let record = lines.next().unwrap();
        assert!(lines.next().is_none());
        assert_eq!(
            mtg_contracts::sha256(record.as_bytes()),
            "2df6ba718258e19da7296786458c8f24f3bf56421272b6e15a63b4baea44a4ab",
        );
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(manifest, record)
        else {
            panic!("the exact source must lower");
        };
        let catalog = Arc::make_mut(&mut prepared.catalog);
        catalog.definitions.push(*definition);
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        prepared.catalog_digest = digest(catalog).unwrap();
        m4a_tests::replace(
            &mut prepared,
            caster,
            "Call Forth the Tempest",
            Zone::Hand { owner: caster },
        );
        for _ in 0..3 {
            m4a_tests::replace(&mut prepared, caster, "Mountain", Zone::Battlefield);
        }
    }
    prepared.profile_digest = digest(&prepared.config).unwrap();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let prior = paid(&mut game, "Grizzly Bears");
    let own_creature = game.top().unwrap().source.card;
    m4a_tests::resolve_entry(&mut game, prior);
    assert_eq!(game.state.rules.cast_history[0].mana_value, 2.into());
    if archive.is_none() {
        // The ordinary paid ledger witness runs even without the private archive.
        prove(&game);
        return;
    }
    let original = paid(&mut game, "Call Forth the Tempest");
    assert_eq!(game.state.rules.cast_history[0].mana_value, 10.into());
    for _ in 0..2 {
        offer(&mut game);
        answer(&mut game, Response::OptionalEffect { perform: true });
        assert!(matches!(request(&mut game), Request::Priority { .. }));
        let child = game.top().unwrap();
        assert_eq!(child.cast_from, Zone::Exile);
        assert_eq!(child.paid.as_ref(), Some(&zero_mana_cost()));
        assert_eq!(game.stack_mana_value(child).unwrap(), 1.into());
        let child_id = child.id;
        m4a_tests::resolve_entry(&mut game, child_id);
    }
    assert_eq!(game.state.rules.cast_history[0].mana_value, 12.into());
    let mut restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    m4a_tests::resolve_entry(&mut game, original);
    m4a_tests::resolve_entry(&mut restored, original);
    assert_eq!(game.state, restored.state);
    assert_eq!(game.trace, restored.trace);
    assert_eq!(
        game.state.rules.objects[&own_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(
        game.state.rules.objects[&wounded].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    let damage = game.trace.records.iter().find_map(|record| {
        let EvidenceRecord::Batch { batch } = &record.record else { return None };
        if !matches!(batch.cause, BatchCause::Instruction { stack_id, source_block: 1, .. } if stack_id == original) {
            return None;
        }
        batch.occurrences.iter().find_map(|r| match &r.actual {
            EventOutcome::DamageCommitted { actual, .. } => Some(actual),
            _ => None,
        })
    }).expect("damage from the original spell");
    assert_eq!(damage.len(), 1);
    assert_eq!(damage[0].amount, 4.into());
}

#[test]
fn cascade_paid_original_free_creature_exact_parent_and_replay() {
    for raw_name in ["Shardless Agent", "Bloodbraid Elf"] {
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut g = Game::with_trace(prepared(raw_name, &["Forest", "Grizzly Bears"]), mode);
            let original = paid(&mut g, raw_name);
            prove(&g);
            offer(&mut g);
            prove(&g);
            let parent = g.top().unwrap().id;
            let Frame::Cascade { progress } = &g.state.rules.frame else {
                panic!()
            };
            assert_eq!(progress.stage, cascade::CascadeStage::Offer);
            assert!(g.decision_for(PlayerId(1)).unwrap().is_none());
            let wrong = g.check_response(
                PlayerId(1),
                g.state.rules.revision,
                Response::OptionalEffect { perform: true },
            );
            assert!(wrong.is_err());
            answer(&mut g, Response::OptionalEffect { perform: true });
            for _ in 0..20 {
                match g.advance(Budget::new(1)) {
                    Advance::Suspended { .. } => {
                        prove(&g);
                    }
                    Advance::AwaitingDecision(Request::Priority { .. }) => break,
                    other => panic!("free casting leaked window: {other:?}"),
                }
            }
            assert!(!g.state.rules.stack.iter().any(|s| s.id == parent));
            assert!(g.state.rules.stack.iter().any(|s| s.id == original));
            let child = g.top().unwrap();
            assert_eq!(
                g.definition(child.source.card).unwrap().name,
                "Grizzly Bears"
            );
            assert_eq!(child.cast_from, Zone::Exile);
            assert_eq!(
                child.alternative_cost,
                Some(cascade::AlternativeCost::WithoutManaCost)
            );
            assert_eq!(child.paid.as_ref(), Some(&zero_mana_cost()));
            assert_eq!(g.stack_mana_value(child).unwrap(), 2.into());
            assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 1);
            assert_eq!(g.state.rules.match_state, None);
            prove(&g);
        }
    }
}
#[test]
fn cascade_decline_empty_nohit_and_no_failed_draw() {
    for library in [vec![], vec!["Forest"], vec!["Grizzly Bears"]] {
        let mut g = Game::new(prepared("Shardless Agent", &library));
        paid(&mut g, "Shardless Agent");
        for _ in 0..20 {
            let r = request(&mut g);
            if g.state.rules.stack.len() == 1 {
                break;
            }
            let response = match r.clone() {
                Request::Priority { .. } => Response::Pass,
                Request::OptionalEffect { .. } => Response::OptionalEffect { perform: false },
                other => panic!("{other:?}"),
            };
            g.respond(r.player(), r.decision_id(), response).unwrap();
        }
        assert_eq!(g.state.rules.stack.len(), 1);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), library.len());
        assert!(!g.state.rules.players[0].failed_draw);
        prove(&g);
    }
}
#[test]
fn cascade_bloodbraid_into_agent_collects_then_resolves_nested_trigger_normally() {
    let mut g = Game::new(prepared(
        "Bloodbraid Elf",
        &["Shardless Agent", "Grizzly Bears"],
    ));
    let original = paid(&mut g, "Bloodbraid Elf");
    offer(&mut g);
    let outer = g.top().unwrap().id;
    answer(&mut g, Response::OptionalEffect { perform: true });
    let r = request(&mut g);
    assert!(matches!(r, Request::Priority { .. }));
    assert!(!g.state.rules.stack.iter().any(|s| s.id == outer));
    assert_eq!(g.state.rules.stack.len(), 3);
    assert_eq!(g.state.rules.stack[0].id, original);
    assert!(matches!(
        g.state.rules.stack[2].kind,
        StackKind::Triggered { .. }
    ));
    assert!(g.state.rules.stack[1].kind.is_spell());
    prove(&g);
    offer(&mut g);
    answer(&mut g, Response::OptionalEffect { perform: true });
    request(&mut g);
    assert_eq!(g.state.rules.stack.len(), 3);
    assert!(g.top().unwrap().kind.is_spell());
    prove(&g);
}
#[test]
fn cascade_free_target_modes_x_and_mandatory_additional_sacrifice() {
    for (name, mode) in [
        ("Unsummon", None),
        ("Healing Salve", Some(0)),
        ("Healing Salve", Some(1)),
        ("Earthquake", None),
        ("Village Rites", None),
        ("Bone Splinters", None),
    ] {
        let mut g = Game::new(prepared("Shardless Agent", &[name]));
        paid(&mut g, "Shardless Agent");
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        loop {
            let r = request(&mut g);
            prove(&g);
            let response = match r.clone() {
                Request::Modes { .. } => Response::Modes {
                    mode: mode.unwrap(),
                },
                Request::Targets { candidates, .. } => Response::Targets {
                    objects: vec![candidates[0]],
                },
                Request::EntityTargets { candidates, .. } => Response::EntityTargets {
                    entities: vec![candidates[0]],
                },
                Request::PlayerTargets { candidates, .. } => Response::PlayerTargets {
                    players: vec![candidates[0]],
                },
                Request::SacrificeCost { candidates, .. } => Response::SacrificeCost {
                    object: candidates[0],
                },
                Request::Priority { .. } => break,
                other => panic!("free spell {name} exposed {other:?}"),
            };
            g.respond(r.player(), r.decision_id(), response).unwrap();
        }
        let child = g.top().unwrap();
        assert_eq!(g.definition(child.source.card).unwrap().name, name);
        assert_eq!(child.paid.as_ref(), Some(&zero_mana_cost()));
        if name == "Earthquake" {
            assert_eq!(child.x, Some(0.into()));
            assert_eq!(g.stack_mana_value(child).unwrap(), 1.into());
        }
        if matches!(name, "Village Rites" | "Bone Splinters") {
            assert!(child.sacrificed.is_some());
        }
        prove(&g);
    }
}
#[test]
fn cascade_offer_public_queries_foreign_stale_tokens_and_rehashed_progress_reject() {
    let mut g = Game::new(prepared("Shardless Agent", &["Forest", "Grizzly Bears"]));
    paid(&mut g, "Shardless Agent");
    offer(&mut g);
    let before = g.state.clone();
    let QueryResult::Complete(QueryValue::Observation(own)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!()
    };
    assert!(matches!(
        own.decision,
        Some(PlayerRequest::OptionalEffect { .. })
    ));
    let token = own.decision.unwrap().token().clone();
    let QueryResult::Incomplete { work, .. } = g.check_player_response(
        PlayerId(0),
        token.clone(),
        PlayerResponse::OptionalEffect { perform: true },
        Budget::new(1),
    ) else {
        panic!()
    };
    let saved = g.save_work(&work).unwrap();
    let work = g.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Validated(validated)) =
        g.resume_work(work, Budget::unlimited())
    else {
        panic!()
    };
    assert_eq!(g.state, before);
    for viewer in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        let QueryResult::Complete(QueryValue::Observation(other)) =
            g.observe(viewer, Budget::unlimited())
        else {
            panic!()
        };
        assert!(other.decision.is_none());
        assert!(other.view.players[0].exile.len() == 2);
        assert!(matches!(
            g.check_player_response(
                viewer,
                token.clone(),
                PlayerResponse::OptionalEffect { perform: true },
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
    }
    let mut forged = g.fork();
    let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
    frame["progress"]["threshold"] = "4".into();
    forged.state.rules.frame = serde_json::from_value(frame).unwrap();
    assert!(
        Game::restore(
            forged.prepared.as_ref().clone(),
            &forged.snapshot().unwrap()
        )
        .is_err()
    );
    g.submit(validated).unwrap();
    request(&mut g);
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::OptionalEffect { perform: true },
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    prove(&g);
}
#[test]
fn cascade_caster_departure_cancels_offer_without_rehoming_exile() {
    let mut g = Game::new(prepared(
        "Bloodbraid Elf",
        &["Shardless Agent", "Grizzly Bears"],
    ));
    paid(&mut g, "Bloodbraid Elf");
    offer(&mut g);
    g.concede(PlayerId(0)).unwrap();
    request(&mut g);
    assert!(
        g.state
            .rules
            .objects
            .values()
            .all(|o| g.state.rules.cards[&o.id.card].owner != PlayerId(0))
    );
    assert!(
        g.state
            .rules
            .stack
            .iter()
            .all(|s| s.controller != PlayerId(0))
    );
    prove(&g);
}
// This starts the real 99-card setup. No seed is added to a midgame state.
fn seeded_prepared(seed: u8) -> Prepared {
    let mut p = prepared("Shardless Agent", &["Grizzly Bears"]);
    let id = |name: &str| {
        p.catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let basics = [id("Forest"), id("Island"), id("Mountain")];
    for card in &mut p.config.cards {
        if matches!(card.zone, Zone::Command { .. }) {
            continue;
        }
        card.zone = Zone::Library { owner: card.owner };
        if card.owner == PlayerId(0) && card.id.0 >= 3 {
            card.oracle_id = basics[(card.id.0 % 3) as usize].clone();
        }
    }
    finish_seeded_setup(p, seed)
}

fn finish_seeded_setup(mut p: Prepared, seed: u8) -> Prepared {
    p.config.initial.turn_step = TurnStep::Untap;
    let mut config =
        MatchConfig::for_profile("commander-staples-v1", [seed; 32], PlayerId(0)).unwrap();
    config.decks = std::array::from_fn(|seat| {
        let owner = PlayerId(seat as u8);
        let mut counts = BTreeMap::<String, u32>::new();
        for card in p.config.cards.iter().filter(|c| c.owner == owner) {
            *counts.entry(card.oracle_id.clone()).or_default() += 1;
        }
        let commander = p
            .config
            .cards
            .iter()
            .find(|c| c.owner == owner && matches!(c.zone, Zone::Command { .. }))
            .unwrap()
            .oracle_id
            .clone();
        Decklist {
            commander,
            cards: counts
                .into_iter()
                .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
                .collect(),
        }
    });
    p.match_config = Some(config);
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
    p
}
#[test]
fn cascade_real_seeded_setup_private_random_bottom_preserves_prefix_history_and_recovery() {
    for trace_mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut found = None;
        for seed in 0..128 {
            let mut g = Game::with_trace(seeded_prepared(seed), trace_mode);
            for _ in 0..4 {
                let r = request(&mut g);
                assert!(matches!(r, Request::Mulligan { .. }));
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Mulligan { take: false },
                )
                .unwrap();
            }
            request(&mut g);
            if g.state.rules.objects.values().any(|o| {
                o.zone == (Zone::Hand { owner: PlayerId(0) })
                    && g.definition(o.id.card).unwrap().name == "Shardless Agent"
            }) {
                found = Some(g);
                break;
            }
        }
        let mut g = found.expect("paid seeded opening witness");
        for _ in 0..1000 {
            let r = request(&mut g);
            if matches!(
                r,
                Request::Priority {
                    player: PlayerId(0),
                    ..
                }
            ) && g.active_player() == Some(PlayerId(0))
                && matches!(
                    g.turn().step,
                    TurnStep::PrecombatMain | TurnStep::PostcombatMain
                )
            {
                let lands = g
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(0))
                    .collect::<Vec<_>>();
                if lands.len() >= 3
                    && lands
                        .iter()
                        .any(|o| g.mana_abilities(o.id).unwrap().contains(&Color::G))
                    && lands
                        .iter()
                        .any(|o| g.mana_abilities(o.id).unwrap().contains(&Color::U))
                {
                    break;
                }
                if g.turn().lands_played == 0 {
                    let present = lands
                        .iter()
                        .flat_map(|o| g.mana_abilities(o.id).unwrap())
                        .collect::<Vec<_>>();
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| {
                            o.zone == (Zone::Hand { owner: PlayerId(0) })
                                && g.definition(o.id.card)
                                    .unwrap()
                                    .characteristics
                                    .has_type(CardKind::Land)
                        })
                        .min_by_key(|o| {
                            usize::from(
                                g.definition(o.id.card)
                                    .unwrap()
                                    .characteristics
                                    .subtypes
                                    .iter()
                                    .filter_map(|s| mtg_contracts::basic_land_color(s))
                                    .all(|c| present.contains(&c)),
                            )
                        })
                        .unwrap()
                        .id;
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::PlayLand { card: land.card },
                    )
                    .unwrap();
                    continue;
                }
            }
            let response = match r.clone() {
                Request::Priority { .. } => Response::Pass,
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                Request::Discard {
                    count, candidates, ..
                } => Response::Discard {
                    objects: candidates
                        .into_iter()
                        .filter(|o| g.definition(o.card).unwrap().name != "Shardless Agent")
                        .take(count)
                        .collect(),
                },
                other => panic!("seeded development {other:?}"),
            };
            g.respond(r.player(), r.decision_id(), response).unwrap();
        }
        paid(&mut g, "Shardless Agent");
        prove(&g);
        // Real scan may find the one supported hit or exhaust after drawing it.
        let shuffles = g
            .state
            .rules
            .match_state
            .as_ref()
            .unwrap()
            .library_shuffles
            .clone();
        let mut prefix = None;
        let mut randomized = false;
        let mut budget_parallel: Option<Game> = None;
        let mut privacy_checked = false;
        let mut verifier = SnapshotVerifier::new(g.prepared.as_ref().clone(), trace_mode);
        for _ in 0..1000 {
            verifier.verify(&g.snapshot().unwrap()).unwrap();
            let stage = match &g.state.rules.frame {
                Frame::Cascade { progress } => Some(progress.stage),
                _ => None,
            };
            if stage == Some(cascade::CascadeStage::Randomize) {
                randomized = true;
                if budget_parallel.is_none() {
                    budget_parallel = Some(g.fork());
                }
                let before = g.state.clone();
                let _ = g.advance(Budget::new(0));
                assert_eq!(g.state, before);
                prefix.get_or_insert(g.state.rules.libraries[&PlayerId(0)].clone());
            }
            if stage == Some(cascade::CascadeStage::Bottom) && !privacy_checked {
                let mut forged = g.fork();
                let mut value = serde_json::to_value(&forged.state.rules.frame).unwrap();
                let permutation = value["progress"]["permutation"].as_array_mut().unwrap();
                assert!(permutation.len() > 1);
                permutation.reverse();
                forged.state.rules.frame = serde_json::from_value(value).unwrap();
                assert!(
                    Game::restore(
                        forged.prepared.as_ref().clone(),
                        &forged.snapshot().unwrap()
                    )
                    .is_err()
                );
                let mut genuine = g.fork();
                for viewer in [PlayerId(0), PlayerId(1)] {
                    assert_eq!(
                        observe_counted(&g, viewer),
                        observe_counted(&forged, viewer)
                    );
                }
                genuine.advance(Budget::new(1));
                forged.advance(Budget::new(1));
                for viewer in [PlayerId(0), PlayerId(1)] {
                    assert_eq!(
                        observe_counted(&genuine, viewer),
                        observe_counted(&forged, viewer)
                    );
                }
                privacy_checked = true;
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(r) => {
                    if randomized && matches!(r, Request::Priority { .. }) {
                        break;
                    }
                    let response = match r.clone() {
                        Request::Priority { .. } => Response::Pass,
                        Request::OptionalEffect { .. } => {
                            Response::OptionalEffect { perform: false }
                        }
                        other => panic!("seeded cascade {other:?}"),
                    };
                    g.respond(r.player(), r.decision_id(), response).unwrap();
                }
                other => panic!("seeded cascade {other:?}"),
            }
        }
        assert!(randomized && privacy_checked);
        let mut parallel = budget_parallel.unwrap();
        let r = parallel.advance(Budget::unlimited());
        assert!(matches!(
            r,
            Advance::AwaitingDecision(Request::Priority { .. })
        ));
        assert_eq!(parallel.state, g.state);
        assert!(g.state.rules.libraries[&PlayerId(0)].starts_with(prefix.as_ref().unwrap()));
        assert_eq!(
            g.state.rules.match_state.as_ref().unwrap().library_shuffles,
            shuffles
        );
        prove(&g);
    }
}
#[test]
fn cascade_strict_equal_above_skips_and_uncastable_first_hit_stops_scan() {
    for skip in ["Cultivate", "Bloodbraid Elf"] {
        let mut g = Game::new(prepared("Shardless Agent", &[skip, "Grizzly Bears"]));
        paid(&mut g, "Shardless Agent");
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        request(&mut g);
        let library = &g.state.rules.libraries[&PlayerId(0)];
        assert_eq!(library.len(), 1);
        assert_eq!(g.definition(library[0]).unwrap().name, skip);
        prove(&g);
    }
    let mut p = prepared("Shardless Agent", &["Unsummon", "Grizzly Bears"]);
    for card in &mut p.config.cards {
        if p.catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == card.oracle_id)
            .unwrap()
            .characteristics
            .has_type(CardKind::Creature)
            && card.zone == Zone::Battlefield
        {
            card.zone = Zone::Hand { owner: card.owner };
        }
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    paid(&mut g, "Shardless Agent");
    for _ in 0..10 {
        let r = request(&mut g);
        if g.state.rules.stack.len() == 1 {
            break;
        }
        assert!(matches!(r, Request::Priority { .. }));
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    let library = &g.state.rules.libraries[&PlayerId(0)];
    assert_eq!(g.definition(library[0]).unwrap().name, "Grizzly Bears");
    assert_eq!(g.definition(library[1]).unwrap().name, "Unsummon");
    prove(&g);
}
#[test]
fn cascade_commander_bottom_replacement_precedes_move_with_child_above_owner() {
    for to_command in [false, true] {
        let mut p = prepared("Shardless Agent", &["Grizzly Bears"]);
        let commander = p.config.commanders[0];
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == commander)
            .unwrap()
            .zone = Zone::Library { owner: PlayerId(0) };
        let hit = p
            .config
            .cards
            .iter()
            .find(|c| {
                c.owner == PlayerId(0)
                    && c.id != commander
                    && matches!(c.zone, Zone::Library { .. })
            })
            .unwrap()
            .id;
        p.config.initial.libraries = Some(BTreeMap::from([
            (PlayerId(0), vec![commander, hit]),
            (PlayerId(1), (101..200).map(CardId).collect()),
            (PlayerId(2), (201..300).map(CardId).collect()),
            (PlayerId(3), (301..400).map(CardId).collect()),
        ]));
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        paid(&mut g, "Shardless Agent");
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        let r = request(&mut g);
        assert!(matches!(
            r,
            Request::CommanderReturn {
                destination: Zone::Library { .. },
                ..
            }
        ));
        prove(&g);
        let child = g.top().unwrap().id;
        assert!(g.top().unwrap().kind.is_spell());
        g.respond(
            r.player(),
            r.decision_id(),
            Response::CommanderReturn { to_command },
        )
        .unwrap();
        request(&mut g);
        assert_eq!(g.top().unwrap().id, child);
        let zone = g.state.rules.objects[&commander].zone;
        assert_eq!(
            zone,
            if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Library { owner: PlayerId(0) }
            }
        );
        prove(&g);
    }
}
#[test]
fn cascade_second_mana_value_guard_is_a_synthetic_internal_control() {
    let mut g = Game::new(prepared("Shardless Agent", &["Grizzly Bears"]));
    paid(&mut g, "Shardless Agent");
    offer(&mut g);
    answer(&mut g, Response::OptionalEffect { perform: true });
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    // Explicitly synthetic resulting-value predicate control. No modified
    // snapshot is presented as a reachable source/recovery witness.
    let top = g.top().unwrap().clone();
    let Frame::Payment { cast } = g.state.rules.frame.clone() else {
        panic!("free payment")
    };
    let CastCompletion::Resolution { permission, .. } = &cast.completion else {
        panic!()
    };
    assert!(g.resulting_permission_matches(permission, &top).unwrap());
    let mut too_large = top.clone();
    too_large.x = Some(3.into());
    assert!(
        !g.resulting_permission_matches(permission, &too_large)
            .unwrap()
    );
    let mut wrong = top;
    wrong.alternative_cost = None;
    assert!(!g.resulting_permission_matches(permission, &wrong).unwrap());
}
#[test]
fn cascade_producer_identity() {
    eprintln!(
        "M3_PRODUCER {}",
        serde_json::json!({"build_id":crate::BUILD_ID,"profile":crate::BUILD_PROFILE,"opt_level":crate::BUILD_OPT_LEVEL,"debug_assertions":crate::BUILD_DEBUG_ASSERTIONS,"rustc":crate::BUILD_RUSTC,"target":crate::BUILD_TARGET})
    );
    assert_eq!(crate::BUILD_OPT_LEVEL, "2");
    assert!(cfg!(debug_assertions));
}
#[test]
fn cascade_paid_ramos_and_self_triggers_share_real_cast_occurrence() {
    let mut p = prepared("Shardless Agent", &["Grizzly Bears"]);
    let forest = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Forest")
        .unwrap()
        .oracle_id
        .clone();
    for card in p
        .config
        .cards
        .iter_mut()
        .filter(|c| {
            c.owner == PlayerId(0)
                && c.zone == (Zone::Hand { owner: PlayerId(0) })
                && c.oracle_id == forest
        })
        .take(6)
    {
        card.zone = Zone::Battlefield;
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let ramos = paid(&mut g, "Ramos, Dragon Engine");
    while g.state.rules.stack.iter().any(|s| s.id == ramos) {
        answer(&mut g, Response::Pass);
        request(&mut g);
    }
    let spell = paid(&mut g, "Shardless Agent");
    let triggers = g
        .state
        .rules
        .stack
        .iter()
        .filter_map(|s| {
            if let StackKind::Triggered {
                witness: TriggerEventWitness::SpellCast { cast },
                ..
            } = &s.kind
            {
                Some((s, cast))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(triggers.len(), 2);
    assert!(triggers.iter().all(|(_, cast)| cast.stack_id == spell));
    assert_eq!(triggers[0].1, triggers[1].1);
    // Both observers retain the same occurrence before any later resolution.
    if g.definition(g.top().unwrap().source.card).unwrap().name == "Shardless Agent" {
        // Placement was a real accepted order. The cascade is above Ramos in
        // this default order, so its own free cast remains pending until later.
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        request(&mut g);
        // Child has zero mana paid but its green color still triggered Ramos.
        assert!(
            g.state
                .rules
                .stack
                .iter()
                .filter(|s| matches!(s.kind, StackKind::Triggered { .. }))
                .count()
                >= 2
        );
    }
    prove(&g);
}
#[test]
fn cascade_prohibition_synthetic_guard_does_not_claim_reachable_paid_context() {
    let mut p = prepared("Shardless Agent", &["Grizzly Bears"]);
    let source = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Kutzil, Malamet Exemplar")
        .unwrap()
        .oracle_id
        .clone();
    let row = p
        .config
        .cards
        .iter_mut()
        .find(|c| c.owner == PlayerId(0) && c.zone == Zone::Battlefield && c.oracle_id != FOREST)
        .unwrap();
    row.oracle_id = source;
    row.controller = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    paid(&mut g, "Shardless Agent");
    offer(&mut g);
    // Synthetic turn change, not a paid-source prohibition exit or a restored
    // origin: these own-main sources cannot legally meet this ban at the offer.
    g.state.rules.turn_owner = PlayerId(1);
    g.state.rules.active = Some(PlayerId(1));
    assert!(
        g.check_response(
            PlayerId(0),
            g.state.rules.revision,
            Response::OptionalEffect { perform: true }
        )
        .is_err()
    );
    assert!(g.decision_for(PlayerId(0)).unwrap().is_none());
}
#[test]
fn cascade_complete_kicker_private_root_nested_signet_ramos_failure_keep_undo() {
    // Complete kicker supplies the extra mana requirement. Initial counters/mana
    // remain synthetic: this private composition is not a legal opening or
    // accepted recovery authority. Genuine paid roots are in m3c_tests.
    for undo in [false, true] {
        let mut p = prepared_with_records(
            "Bloodbraid Elf",
            &["Dismantling Blow"],
            &[include_str!(
                "../../../../tests/fixtures/dismantling-blow.json"
            )],
        );
        let signet = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == "Golgari Signet")
            .unwrap()
            .oracle_id
            .clone();
        let forest = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == "Forest")
            .unwrap()
            .oracle_id
            .clone();
        let row = p
            .config
            .cards
            .iter_mut()
            .find(|c| {
                c.owner == PlayerId(0)
                    && c.oracle_id == forest
                    && c.zone == (Zone::Hand { owner: PlayerId(0) })
            })
            .unwrap();
        row.oracle_id = signet.clone();
        row.zone = Zone::Battlefield;
        let row = p
            .config
            .cards
            .iter_mut()
            .find(|c| {
                c.owner == PlayerId(0)
                    && c.oracle_id == forest
                    && c.zone == (Zone::Hand { owner: PlayerId(0) })
            })
            .unwrap();
        row.oracle_id = signet;
        row.zone = Zone::Battlefield;
        let ramos = p.config.commanders[0];
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == ramos)
            .unwrap()
            .zone = Zone::Battlefield;
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        g.state
            .rules
            .objects
            .get_mut(&ramos)
            .unwrap()
            .plus_one_counters = 5.into();
        paid(&mut g, "Bloodbraid Elf");
        offer(&mut g);
        g.state.rules.players[0].mana.insert(Color::R, 1.into());
        let before = g.state.rules.frame.clone();
        let unrelated = g.state.rules.players[0].mana.clone();
        let hit = if let Frame::Cascade { progress } = &g.state.rules.frame {
            serde_json::to_value(progress).unwrap()["hit"].clone()
        } else {
            panic!()
        };
        answer(&mut g, Response::OptionalEffect { perform: true });
        let Request::AdditionalCosts { candidates, .. } = request(&mut g) else {
            panic!("kicker choice")
        };
        answer(
            &mut g,
            Response::AdditionalCosts {
                selected: vec![candidates[0].identity.clone()],
            },
        );
        let signets = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && g.definition(o.id.card).unwrap().name == "Golgari Signet"
            })
            .map(|o| o.id)
            .collect::<Vec<_>>();
        answer(
            &mut g,
            Response::Targets {
                objects: vec![signets[0]],
            },
        );
        assert!(matches!(request(&mut g), Request::Mana { .. }));
        let signet = signets[0];
        let ramos_object = g.state.rules.objects[&ramos].id;
        answer(&mut g, Response::ActivateMana { object: signet });
        answer(
            &mut g,
            Response::ActivateMana {
                object: ramos_object,
            },
        );
        answer(&mut g, Response::FinishMana { undo_mana: false });
        answer(
            &mut g,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::U, 1.into())]),
            },
        );
        answer(&mut g, Response::ActivateMana { object: signets[1] });
        answer(&mut g, Response::FinishMana { undo_mana: false });
        answer(
            &mut g,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::U, 1.into())]),
            },
        );
        assert!(matches!(request(&mut g), Request::Mana { .. }));
        assert!(
            !g.can_pay(PlayerId(0), &g.top().unwrap().total_cost)
                .unwrap()
        );
        answer(&mut g, Response::FinishMana { undo_mana: undo });
        request(&mut g);
        assert!(matches!(g.state.rules.frame, Frame::Cascade { .. }));
        let Frame::Cascade { progress } = &g.state.rules.frame else {
            panic!()
        };
        assert_eq!(serde_json::to_value(progress).unwrap()["hit"], hit);
        assert_eq!(g.state.rules.frame, before);
        if undo {
            assert_eq!(g.state.rules.players[0].mana, unrelated);
            assert_eq!(g.state.rules.objects[&ramos].plus_one_counters, 5.into());
            assert!(!g.mana_ability_used(ramos_object));
            assert!(!g.current(signet).unwrap().tapped);
        } else {
            assert_eq!(g.state.rules.objects[&ramos].plus_one_counters, 0.into());
            assert!(g.mana_ability_used(ramos_object));
            assert!(g.current(signet).unwrap().tapped);
        }
        assert!(
            Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).is_err(),
            "synthetic prefix must not acquire recovery authority"
        );
        answer(&mut g, Response::OptionalEffect { perform: true });
        answer(&mut g, Response::AdditionalCosts { selected: vec![] });
        answer(
            &mut g,
            Response::Targets {
                objects: vec![signets[0]],
            },
        );
        request(&mut g);
        assert!(g.state.rules.stack.iter().any(|s| s.kind.is_spell()
            && g.definition(s.source.card).unwrap().name == "Dismantling Blow"
            && s.paid.as_ref() == Some(&zero_mana_cost())));
        assert!(
            matches!(g.top().unwrap().kind, StackKind::Triggered { .. }),
            "Ramos child trigger waits until parent finishes"
        );
    }
}
fn observe_counted(g: &Game, viewer: PlayerId) -> (Observation, usize) {
    let mut result = g.observe(viewer, Budget::new(1));
    let mut units = 1;
    loop {
        match result {
            QueryResult::Complete(QueryValue::Observation(o)) => return (o, units),
            QueryResult::Incomplete { work, .. } => {
                units += 1;
                result = g.resume_work(work, Budget::new(1));
            }
            _ => panic!("observation control"),
        }
    }
}
#[test]
fn cascade_paid_counter_original_uses_lki_and_counter_trigger_performs_no_program() {
    for (counter_name, counter_original) in [("Counterspell", true), ("Stifle", false)] {
        let mut p = prepared("Shardless Agent", &["Forest", "Grizzly Bears"]);
        let oracle = |name: &str| {
            p.catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone()
        };
        let counter = oracle(counter_name);
        let island = oracle("Island");
        let ramos = oracle("Ramos, Dragon Engine");
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == p.config.commanders[1])
            .unwrap()
            .oracle_id = ramos;
        let mut rows = p
            .config
            .cards
            .iter_mut()
            .filter(|c| c.owner == PlayerId(1) && matches!(c.zone, Zone::Library { .. }));
        let row = rows.next().unwrap();
        row.oracle_id = counter;
        row.zone = Zone::Hand { owner: PlayerId(1) };
        for row in rows.take(2) {
            row.oracle_id = island.clone();
            row.zone = Zone::Battlefield;
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        let original = paid(&mut g, "Shardless Agent");
        let cascade = g.top().unwrap().id;
        let initial_library = g.state.rules.libraries[&PlayerId(0)].clone();
        answer(&mut g, Response::Pass);
        assert!(matches!(
            request(&mut g),
            Request::Priority {
                player: PlayerId(1),
                ..
            }
        ));
        let counter = paid_by(
            &mut g,
            counter_name,
            PlayerId(1),
            Some(if counter_original { original } else { cascade }),
        );
        while g.state.rules.stack.iter().any(|s| s.id == counter) {
            answer(&mut g, Response::Pass);
            request(&mut g);
        }
        prove(&g);
        if counter_original {
            assert!(!g.state.rules.stack.iter().any(|s| s.id == original));
            offer(&mut g);
            answer(&mut g, Response::OptionalEffect { perform: true });
            request(&mut g);
            assert_eq!(g.state.rules.stack.len(), 1);
            assert_eq!(
                g.definition(g.top().unwrap().source.card).unwrap().name,
                "Grizzly Bears"
            );
        } else {
            assert!(!g.state.rules.stack.iter().any(|s| s.id == cascade));
            assert_eq!(g.state.rules.libraries[&PlayerId(0)], initial_library);
            assert_eq!(g.top().unwrap().id, original);
        }
        prove(&g);
    }
}
#[test]
fn cascade_rehashed_missing_parent_exile_membership_cursor_and_child_forgeries_reject() {
    let mut g = Game::new(prepared("Shardless Agent", &["Forest", "Grizzly Bears"]));
    paid(&mut g, "Shardless Agent");
    offer(&mut g);
    for mutation in 0..5 {
        let mut forged = g.fork();
        let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
        match mutation {
            0 => {
                forged.state.rules.stack.pop();
            }
            1 => {
                frame["progress"]["exiled"]
                    .as_array_mut()
                    .unwrap()
                    .reverse();
            }
            2 => {
                frame["progress"]["frame"]["cursor"] = 1.into();
            }
            3 => {
                frame["progress"]["source_block"] = 999.into();
            }
            4 => {
                let duplicate = frame["progress"]["exiled"][0].clone();
                frame["progress"]["exiled"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
            }
            _ => unreachable!(),
        }
        forged.state.rules.frame = serde_json::from_value(frame).unwrap();
        assert!(
            Game::restore(
                forged.prepared.as_ref().clone(),
                &forged.snapshot().unwrap()
            )
            .is_err(),
            "forgery {mutation}"
        );
    }
    answer(&mut g, Response::OptionalEffect { perform: true });
    g.advance(Budget::new(1));
    prove(&g);
    let mut forged = g.fork();
    let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
    frame["cast"]["completion"]["progress"]["child"] =
        serde_json::to_value(g.top().unwrap().id).unwrap();
    forged.state.rules.frame = serde_json::from_value(frame).unwrap();
    assert!(
        Game::restore(
            forged.prepared.as_ref().clone(),
            &forged.snapshot().unwrap()
        )
        .is_err()
    );
}
#[test]
fn cascade_accepted_cast_and_decline_survive_last_target_owner_departure_and_recover() {
    for perform in [true, false] {
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut p = prepared("Shardless Agent", &["Unsummon"]);
            // Ordinary owner-controlled creature: no invented control transition.
            // Swap one basic's ownership to retain exactly 100 cards for each seat.
            let wall = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == "Wall of Wood")
                .unwrap()
                .oracle_id
                .clone();
            let target = p
                .config
                .cards
                .iter_mut()
                .find(|c| c.oracle_id == wall && c.zone == Zone::Battlefield)
                .unwrap();
            target.owner = PlayerId(1);
            target.controller = PlayerId(1);
            let basic = p
                .config
                .cards
                .iter_mut()
                .find(|c| c.owner == PlayerId(1) && matches!(c.zone, Zone::Library { .. }))
                .unwrap();
            basic.owner = PlayerId(0);
            basic.controller = PlayerId(0);
            basic.zone = Zone::Hand { owner: PlayerId(0) };
            for owner in [PlayerId(0), PlayerId(1)] {
                assert_eq!(
                    p.config.cards.iter().filter(|c| c.owner == owner).count(),
                    100
                );
            }
            p.profile_digest = digest(&p.config).unwrap();
            let mut g = Game::with_trace(p, mode);
            let original = paid(&mut g, "Shardless Agent");
            offer(&mut g);
            answer(&mut g, Response::OptionalEffect { perform });
            assert!(g.state.rules.pending.is_some());
            g.concede(PlayerId(1)).unwrap();
            prove(&g);
            let report = g.advance_with_report(Budget::new(1));
            assert!(matches!(report.advance, Advance::Suspended { .. }));
            assert!(g.state.rules.pending.is_none());
            assert!(report.processed.iter().any(|p| matches!(
                p.identity,
                CoreInputIdentity::Decision { .. }
            ) && p.cause
                == InputProcessingCause::DepartureStep));
            assert!(matches!(g.state.rules.frame, Frame::Cascade { .. }));
            prove(&g);
            assert!(matches!(request(&mut g), Request::Priority { .. }));
            assert_eq!(g.state.rules.stack.len(), 1);
            assert_eq!(g.top().unwrap().id, original);
            assert_eq!(
                g.state.rules.next_stack_id, 2,
                "no child proposal/cast occurs after losing the full target domain"
            );
            let library = &g.state.rules.libraries[&PlayerId(0)];
            assert_eq!(library.len(), 1);
            assert_eq!(g.definition(library[0]).unwrap().name, "Unsummon");
            prove(&g);
        }
    }
}

#[test]
fn cascade_paid_aura_control_free_controlled_target_and_owner_departure_recover() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        // These are complete archived sources in a private, seedless midgame.
        // A single cast hit leaves no group to randomize; no invented RNG root
        // or source admission is needed for the combined witness.
        let mut p = prepared_with_records(
            "Bloodbraid Elf",
            &["Dive Down"],
            &[
                include_str!(
                    "../../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json"
                ),
                include_str!(
                    "../../../../tests/fixtures/2f990b54-fbf3-4949-85bb-9ba39710e72a.json"
                ),
            ],
        );
        let id = |name: &str| {
            p.catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone()
        };
        let (wall, forest, magic) = (id("Wall of Wood"), id("Forest"), id("Control Magic"));
        let target_card = {
            let row = p
                .config
                .cards
                .iter_mut()
                .find(|c| c.oracle_id == wall)
                .unwrap();
            row.owner = PlayerId(1);
            row.controller = PlayerId(1);
            row.id
        };
        // Exchange one basic so both players still own exactly 100 cards.
        let row = p
            .config
            .cards
            .iter_mut()
            .find(|c| c.owner == PlayerId(1) && matches!(c.zone, Zone::Library { .. }))
            .unwrap();
        row.owner = PlayerId(0);
        row.controller = PlayerId(0);
        row.zone = Zone::Hand { owner: PlayerId(0) };
        p.config
            .cards
            .iter_mut()
            .find(|c| {
                c.owner == PlayerId(0)
                    && c.oracle_id == forest
                    && c.zone == Zone::Hand { owner: PlayerId(0) }
            })
            .unwrap()
            .oracle_id = magic;
        for owner in [PlayerId(0), PlayerId(1)] {
            assert_eq!(
                p.config.cards.iter().filter(|c| c.owner == owner).count(),
                100
            );
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, mode);
        let target = g.object_id(target_card).unwrap();
        assert_eq!(g.current(target).unwrap().controller, PlayerId(1));
        paid(&mut g, "Control Magic");
        for _ in 0..16 {
            let r = request(&mut g);
            if g.state.rules.stack.is_empty() {
                break;
            }
            assert!(matches!(r, Request::Priority { .. }));
            g.respond(r.player(), r.decision_id(), Response::Pass)
                .unwrap();
        }
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(g.current(target).unwrap().controller, PlayerId(0));
        assert_eq!(g.current(target).unwrap().default_controller, PlayerId(1));
        assert_eq!(g.state.rules.cards[&target.card].owner, PlayerId(1));
        prove(&g);

        let original = paid(&mut g, "Bloodbraid Elf");
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        let r = request(&mut g);
        let Request::Targets { candidates, .. } = &r else {
            panic!("controlled-target free cast: {r:?}")
        };
        assert_eq!(candidates, &[target]);
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![target],
            },
        )
        .unwrap();
        assert!(matches!(request(&mut g), Request::Priority { .. }));
        let child = g.top().unwrap().id;
        let child_card = g.top().unwrap().source.card;
        assert_ne!(child, original);
        assert_eq!(g.definition(child_card).unwrap().name, "Dive Down");
        assert!(g.top().unwrap().paid.is_some());
        assert_eq!(g.state.rules.stack.len(), 2);
        assert_eq!(g.state.rules.stack[0].id, original);
        prove(&g);
        g.concede(PlayerId(1)).unwrap();
        prove(&g);
        for _ in 0..32 {
            let r = request(&mut g);
            if g.state.rules.stack.is_empty() {
                break;
            }
            assert!(matches!(r, Request::Priority { .. }));
            g.respond(r.player(), r.decision_id(), Response::Pass)
                .unwrap();
        }
        assert!(g.state.rules.stack.is_empty());
        assert!(g.object_id(target_card).is_none());
        assert_eq!(
            g.zone(g.object_id(child_card).unwrap()),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        if mode == TraceMode::Detailed {
            assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == child)));
        }
        prove(&g);
    }
}

#[path = "m3c_tests.rs"]
mod m3c_tests;

#[path = "m4a_tests.rs"]
mod m4a_tests;

#[path = "m4b_cascade_tests.rs"]
mod m4b_cascade_tests;

#[path = "m4c_tests.rs"]
mod m4c_tests;

#[path = "m4d_lifecycle_tests.rs"]
mod m4d_lifecycle_tests;
#[path = "m4d_tests.rs"]
mod m4d_tests;
#[path = "m4e_tests.rs"]
mod m4e_tests;

#[test]
fn cascade_complete_opt_child_scry_draw_preserves_original_and_private_recovery() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let p = prepared_with_records(
            "Shardless Agent",
            &["Opt", "Forest"],
            &[include_str!(
                "../../../../tests/fixtures/713332c1-5bd8-400f-bfff-c1ca0697a043.json"
            )],
        );
        let mut g = Game::with_trace(p, mode);
        let original = paid(&mut g, "Shardless Agent");
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        let r = loop {
            let r = request(&mut g);
            if matches!(r, Request::Scry { .. }) {
                break r;
            }
            assert!(
                matches!(r, Request::Priority { .. }),
                "unexpected child choice {r:?}"
            );
            answer(&mut g, Response::Pass);
        };
        let Request::Scry { candidates, .. } = &r else {
            unreachable!()
        };
        assert_eq!(candidates.len(), 1);
        let looked = candidates[0];
        assert!(g.state.rules.stack.iter().any(|s| s.id == original));
        let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
            unreachable!()
        };
        assert_eq!(g.definition(frame.spell.source.card).unwrap().name, "Opt");
        assert_eq!(frame.spell.paid.as_ref(), Some(&zero_mana_cost()));
        let child = frame.spell.id;
        prove(&g);
        answer(
            &mut g,
            Response::Scry {
                top: vec![],
                bottom: candidates.clone(),
            },
        );
        prove(&g);
        loop {
            let r = request(&mut g);
            if !g.state.rules.stack.iter().any(|s| s.id == child) {
                break;
            }
            assert!(matches!(r, Request::Priority { .. }));
            answer(&mut g, Response::Pass);
        }
        assert!(g.state.rules.stack.iter().any(|s| s.id == original));
        let drawn = g.current(g.object_id(looked.card).unwrap()).unwrap();
        assert_eq!(drawn.zone, Zone::Hand { owner: PlayerId(0) });
        assert_ne!(drawn.id, looked);
        prove(&g);
    }
}

#[test]
#[ignore = "requires the frozen one-card MTG_CASCADE_LAND_ENTRY_ARCHIVE_PROBE"]
fn cascade_land_entry_paid_averna_before_hit_or_bottom() {
    let archive = std::fs::read_to_string(
        std::env::var_os("MTG_CASCADE_LAND_ENTRY_ARCHIVE_PROBE").expect("external Averna sheet"),
    )
    .unwrap();
    assert_eq!(
        mtg_contracts::sha256(archive.as_bytes()),
        "77f05799c86e9b978130d58909100259cdca766cafedab30b7f8483826e92c4c"
    );
    let mut lines = archive.lines();
    let manifest = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert_eq!(
        mtg_contracts::sha256(row.as_bytes()),
        "5472a36613d5d9a84db4d240c6e244a1da42ec3c5a0daa2f30613326c059ae46"
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(manifest, row)
    else {
        panic!("complete Averna source must lower");
    };
    let player = PlayerId(0);
    for hit in [true, false] {
        let mut library = vec!["Forest", "Island"];
        if hit {
            library.push("Sol Ring");
        }
        let mut p = prepared("Shardless Agent", &library);
        let c = Arc::make_mut(&mut p.catalog);
        c.definitions.push((*definition).clone());
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        c.validate().unwrap();
        p.catalog_digest = digest(c).unwrap();
        m4a_tests::replace(
            &mut p,
            player,
            &definition.name,
            Zone::Hand { owner: player },
        );
        let unrelated = m4a_tests::replace(&mut p, player, "Forest", Zone::Exile);
        p.profile_digest = digest(&p.config).unwrap();

        let mut g = Game::with_trace(p, TraceMode::Off);
        let hand_land = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.zone == (Zone::Hand { owner: player })
                    && g.definition(o.id.card).unwrap().name == "Forest"
            })
            .unwrap()
            .id
            .card;
        answer(&mut g, Response::PlayLand { card: hand_land });
        let averna = paid(&mut g, &definition.name);
        m4a_tests::resolve_entry(&mut g, averna);
        let original = paid(&mut g, "Shardless Agent");
        let parent = g.top().unwrap().id;
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        let Request::CascadeLandEntry {
            candidates: lands, ..
        } = request(&mut g)
        else {
            panic!("land offer immediately after the scan")
        };
        assert_eq!(lands.len(), 2);
        assert_eq!(g.definition(lands[0].card).unwrap().name, "Forest");
        let QueryResult::Complete(QueryValue::Observation(observation)) =
            g.observe(player, Budget::unlimited())
        else {
            panic!("complete land observation")
        };
        let QueryResult::Incomplete { work, .. } = g.player_decision_for(player, Budget::new(2))
        else {
            panic!("one reconstructed choice row expected")
        };
        let saved = g.save_work(&work).unwrap();
        let QueryResult::Complete(QueryValue::Decision(decision)) =
            g.resume_work(g.restore_work(&saved).unwrap(), Budget::unlimited())
        else {
            panic!("complete restored land decision")
        };
        assert_eq!(observation.decision, decision);
        assert!(observation.view.priority.is_none());
        let Some(PlayerRequest::CascadeLandEntry {
            token, candidates, ..
        }) = decision
        else {
            panic!("public land offer")
        };
        assert_eq!(
            candidates,
            lands
                .iter()
                .map(|o| g.handle_for(player, *o).unwrap())
                .collect::<Vec<_>>()
        );
        if hit {
            let mut decline = g.fork();
            answer(&mut decline, Response::CascadeLandEntry { object: None });
            assert!(matches!(
                request(&mut decline),
                Request::OptionalEffect { .. }
            ));
            assert!(
                lands
                    .iter()
                    .all(|id| decline.current(*id).unwrap().zone == Zone::Exile)
            );
        }
        let foreign = g
            .handle_for(player, g.object_id(unrelated).unwrap())
            .unwrap();
        assert!(matches!(
            g.check_player_response(
                player,
                token.clone(),
                PlayerResponse::CascadeLandEntry {
                    object: Some(foreign)
                },
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
        let QueryResult::Complete(QueryValue::Validated(choice)) = g.check_player_response(
            player,
            token.clone(),
            PlayerResponse::CascadeLandEntry {
                object: Some(candidates[0].clone()),
            },
            Budget::unlimited(),
        ) else {
            panic!("accepted exact public land")
        };
        g.submit(choice).unwrap();
        if hit {
            assert!(matches!(request(&mut g), Request::OptionalEffect { .. }));
            assert!(matches!(
                g.check_player_response(
                    player,
                    token,
                    PlayerResponse::OptionalEffect { perform: true },
                    Budget::unlimited()
                ),
                QueryResult::Rejected(_)
            ));
            assert!(g.state.rules.stack.iter().any(|s| s.id == parent));
            prove(&g);
            answer(&mut g, Response::OptionalEffect { perform: true });
        }
        assert!(matches!(request(&mut g), Request::Priority { .. }));
        let entered = &g.state.rules.objects[&lands[0].card];
        assert_eq!(entered.id.incarnation, lands[0].incarnation + 1);
        assert_eq!(
            (entered.zone, entered.controller, entered.tapped),
            (Zone::Battlefield, player, true)
        );
        assert_eq!(g.turn().lands_played, 1);
        assert_eq!(g.state.rules.objects[&unrelated].zone, Zone::Exile);
        assert_eq!(g.state.rules.libraries[&player], [lands[1].card]);
        assert!(!g.state.rules.stack.iter().any(|s| s.id == parent));
        if hit {
            let child = g.top().unwrap();
            assert_eq!(g.definition(child.source.card).unwrap().name, "Sol Ring");
            assert_eq!(child.cast_from, Zone::Exile);
            assert_eq!(child.paid.as_ref(), Some(&zero_mana_cost()));
            assert!(g.state.rules.stack.iter().any(|s| s.id == original));
        } else {
            assert_eq!(g.top().unwrap().id, original);
        }
    }
}

#[path = "creative_technique_tests.rs"]
mod creative_technique_tests;

#[path = "linked_play_tests.rs"]
mod linked_play_tests;

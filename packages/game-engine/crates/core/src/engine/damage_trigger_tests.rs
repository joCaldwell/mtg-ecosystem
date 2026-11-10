//! Unexecuted D2c-e candidate tests. Compile complete pinned source rows at
//! test time; no fabricated expected IR or public hosting/search expansion.
use super::*;
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
fn catalog() -> Catalog {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cards-data-d2ce");
    let manifest = std::fs::read_to_string(root.join("archive-manifest-current.json")).unwrap();
    let provenance: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("provenance.json")).unwrap())
            .unwrap();
    for row in provenance["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            [4538, 9081, 12306, 14941, 19442, 21636, 28189]
                .contains(&r["archive_line"].as_u64().unwrap())
        })
    {
        let raw = std::fs::read_to_string(root.join(row["file"].as_str().unwrap())).unwrap();
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(&manifest, &raw)
        else {
            panic!("complete source {} did not lower", row["file"]);
        };
        c.definitions.push(*definition);
    }
    c.schema_version = 25;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    c
}
fn prepared(sources: &[(&str, u8, u8, Zone)]) -> Prepared {
    let c = catalog();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "step-program-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.clear();
    for (id, (oracle, owner, controller, zone)) in sources.iter().enumerate() {
        config.cards.push(CardSetup {
            id: CardId(id as u32),
            oracle_id: (*oracle).into(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: *zone,
        });
    }
    for seat in 0..4 {
        for _ in 0..8 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: SWAMP.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    for seat in 0..4 {
        for oracle in [
            SWAMP, SWAMP, SWAMP, SWAMP, SWAMP, SWAMP, PLAINS, FOREST, MOUNTAIN, ISLAND, ISLAND,
        ] {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: oracle.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Battlefield,
            });
        }
    }
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        catalog: Arc::new(c),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("step witness at {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("step witness budget")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game) {
    let r = settle(g);
    let (player, decision_id) = (r.player(), r.decision_id());
    let response = match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers { order: candidates },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates.into_iter().take(count).collect(),
        },
        other => panic!("unexpected passive request {other:?}"),
    };
    g.respond(player, decision_id, response).unwrap();
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert!(restored.state == g.state);
    assert_eq!(restored.trace, g.trace);
    let replayed = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        g.trace.mode,
    )
    .unwrap();
    assert!(replayed.state == g.state);
    assert_eq!(replayed.trace, g.trace);
}
fn to_step(g: &mut Game, number: u64, step: TurnStep) {
    for _ in 0..500 {
        let _ = settle(g);
        if g.state.rules.turn.number == number && g.state.rules.turn.step == step {
            return;
        }
        passive(g);
    }
    panic!("missing step {number}/{step:?}")
}
fn placed(g: &mut Game) {
    loop {
        match settle(g) {
            Request::Priority { .. } => return,
            Request::OrderTriggers { candidates, .. } => {
                answer(g, Response::OrderTriggers { order: candidates })
            }
            r => panic!("placement {r:?}"),
        }
    }
}
fn resolve(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
    let _ = settle(g);
}

const FOX: &str = "51f4eb1f-abf2-425a-b78e-cafac177da89";
const THIEF: &str = "637c5583-4683-4ae4-8b4e-f5da42a772c7";
const SPY: &str = "813c0654-7e3b-435b-b7a5-148f0c36a4be";
const STEALER: &str = "8f9535f2-f00c-40d3-9997-d1c0ece15f9f";
const DRIDER: &str = "1db76752-31ed-4a16-986a-d0852ba7d812";
const CAPTAIN: &str = "3c75234b-e367-46da-8454-be4315f9b9de";
const SHRINEKEEPER: &str = "bb50b4f4-f0da-4283-821b-6d74bd20752f";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
fn edit(p: &mut Prepared, id: &str, f: impl FnOnce(&mut Definition)) {
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == id)
        .unwrap();
    f(d);
    d.capabilities = d.requirements();
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
}
fn priority(g: &mut Game, player: u8) {
    for _ in 0..4 {
        if settle(g).player() == PlayerId(player) {
            return;
        }
        answer(g, Response::Pass);
    }
    panic!("missing priority");
}
fn pay_input(g: &mut Game) {
    let Request::SpendMana {
        player,
        cost,
        available,
        ..
    } = settle(g)
    else {
        panic!("spending")
    };
    let remaining = g.payment_pool(player, &cost).unwrap();
    let spending = available
        .into_iter()
        .filter_map(|(color, mut amount)| {
            amount.subtract(&remaining.get(&color).cloned().unwrap_or_default());
            (amount > Quantity::zero()).then_some((color, amount))
        })
        .collect();
    answer(g, Response::SpendMana { spending });
}
fn fund(g: &mut Game) {
    let player = settle(g).player();
    let lands = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.controller == player
                && !o.tapped
                && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
        })
        .map(|o| o.id)
        .collect::<Vec<_>>();
    for object in lands {
        answer(g, Response::ActivateMana { object });
    }
    answer(g, Response::FinishMana { undo_mana: false });
    pay_input(g);
}
fn cast(g: &mut Game, card: u32) {
    answer(g, Response::Cast { card: CardId(card) });
    fund(g);
    placed(g);
}
fn pass_until(g: &mut Game, pred: impl Fn(&Game) -> bool) {
    for _ in 0..300 {
        if pred(g) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => passive(g),
            x => panic!("{x:?}"),
        }
    }
    panic!("missing boundary");
}
fn modes() -> [TraceMode; 3] {
    [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed]
}
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn declare(g: &mut Game, attacks: &[(u32, u8)]) {
    to_step(g, 1, TurnStep::DeclareAttackers);
    let attacks = attacks
        .iter()
        .map(|(c, p)| (object(g, *c), PlayerId(*p)))
        .collect();
    answer(g, Response::Attackers { attacks });
}
fn damage_boundary(g: &mut Game) {
    pass_until(g, |g| {
        matches!(g.state.rules.frame, Frame::DamageEffects { .. })
    });
}
fn pending(g: &mut Game) {
    pass_until(g, |g| !g.state.rules.pending_triggers.is_empty());
}
fn named(name: &str) -> String {
    catalog()
        .definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone()
}
fn library(g: &Game, player: u8) -> usize {
    g.state.rules.libraries[&PlayerId(player)].len()
}
fn witness(g: &Game) -> TriggerEventWitness {
    // Pending fields stay private: the wire shape is itself part of the test.
    serde_json::from_value(
        serde_json::to_value(&g.state.rules.pending_triggers[0]).unwrap()["witness"].clone(),
    )
    .unwrap()
}
fn actual(g: &Game) -> ActualDamage {
    let TriggerEventWitness::DamageDealt { damage, .. } = witness(g) else {
        panic!("damage witness")
    };
    *damage
}
#[test]
fn d2ce_seven_complete_sources_ir_to_runtime_all_traces() {
    for mode in modes() {
        for source in [FOX, THIEF, SPY, STEALER, DRIDER, CAPTAIN, SHRINEKEEPER] {
            let mut g = Game::with_trace(prepared(&[(source, 2, 0, Zone::Battlefield)]), mode);
            let n = library(&g, 0);
            declare(&mut g, &[(0, 1)]);
            pending(&mut g);
            assert_eq!(g.state.rules.pending_triggers.len(), 1);
            if source == CAPTAIN {
                assert_eq!(g.state.rules.turn.step, TurnStep::FirstStrikeDamage);
            }
            let d = actual(&g);
            assert_eq!(d.source.object, object(&g, 0));
            assert_eq!(d.source.owner, PlayerId(2));
            assert_eq!(d.controller, PlayerId(0));
            assert_eq!(
                d.recipient,
                DamageRecipient::Player {
                    player: PlayerId(1)
                }
            );
            assert_eq!(
                Some(d.amount.clone()),
                g.characteristics(object(&g, 0)).unwrap().power
            );
            recover(&g);
            placed(&mut g);
            recover(&g);
            resolve(&mut g);
            recover(&g);
            assert_eq!(g.state.rules.next_trigger_id, 1);
            if [DRIDER, CAPTAIN].contains(&source) {
                assert_eq!(library(&g, 0), n);
                assert_eq!(g.state.rules.tokens.len(), 1);
                let token = g.state.rules.tokens.keys().next().unwrap();
                let fact = g.object_before(g.object_id(*token).unwrap()).unwrap();
                assert_eq!(fact.controller, PlayerId(0));
                assert_eq!(
                    fact.characteristics.power,
                    Some(if source == DRIDER { 2.into() } else { 1.into() })
                );
                assert_eq!(fact.characteristics.toughness, Some(1.into()));
                if source == DRIDER {
                    assert!(fact.keywords.contains(&KeywordAbility::Reach));
                    assert!(fact.keywords.contains(&KeywordAbility::Menace));
                }
            } else {
                assert_eq!(library(&g, 0), n - 1);
            }
            assert_eq!(
                g.state.rules.players[0].life,
                if source == SHRINEKEEPER {
                    41.into()
                } else {
                    40.into()
                }
            );
        }
    }
}
#[test]
fn d2ce_distinct_sources_same_player_and_first_double_strike_commits() {
    for mode in modes() {
        for double in [false, true] {
            let mut p = prepared(&[
                (FOX, 0, 0, Zone::Battlefield),
                (THIEF, 0, 0, Zone::Battlefield),
            ]);
            if double {
                edit(&mut p, FOX, |d| {
                    d.keywords.push(KeywordAbility::DoubleStrike)
                });
            }
            let mut g = Game::with_trace(p, mode);
            declare(&mut g, &[(0, 1), (1, 1)]);
            pending(&mut g);
            if double {
                assert_eq!(g.state.rules.pending_triggers.len(), 1);
                let first = witness(&g);
                placed(&mut g);
                resolve(&mut g);
                pending(&mut g);
                assert_eq!(g.state.rules.pending_triggers.len(), 2);
                assert_ne!(first, witness(&g));
            } else {
                assert_eq!(g.state.rules.pending_triggers.len(), 2);
            }
            placed(&mut g);
            while !g.state.rules.stack.is_empty() {
                resolve(&mut g);
            }
            assert_eq!(g.state.rules.next_trigger_id, if double { 3 } else { 2 });
            recover(&g);
        }
    }
}
#[test]
fn d2ce_partial_full_prevention_zero_and_doubling_actual_amount() {
    let healer = named("Samite Healer");
    let fog = named("Fog");
    let furnace = named("Furnace of Rath");
    for mode in modes() {
        for (prevent, double) in [
            (0, false),
            (1, false),
            (2, false),
            (0, true),
            (1, true),
            (2, true),
        ] {
            let mut rows = vec![
                (FOX, 0, 0, Zone::Battlefield),
                (&healer, 0, 0, Zone::Battlefield),
                (&fog, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            ];
            if double {
                rows.push((&furnace, 3, 3, Zone::Battlefield));
            }
            let mut g = Game::with_trace(prepared(&rows), mode);
            if prevent == 2 {
                cast(&mut g, 2);
                resolve(&mut g);
            }
            if prevent == 1 {
                let h = object(&g, 1);
                answer(
                    &mut g,
                    Response::ActivateAbility {
                        object: h,
                        ability: 0,
                    },
                );
                answer(
                    &mut g,
                    Response::EntityTargets {
                        entities: vec![TargetEntity::Player {
                            player: PlayerId(1),
                        }],
                    },
                );
                placed(&mut g);
                resolve(&mut g);
            }
            declare(&mut g, &[(0, 1)]);
            // Fox has power two in the pinned source. Shield/Furnace order
            // is a real affected-player decision, with recovery at that choice.
            for _ in 0..150 {
                if !g.state.rules.pending_triggers.is_empty()
                    || g.state.rules.players[1].life < 40.into()
                {
                    break;
                }
                match g.advance(Budget::new(1)) {
                    Advance::Suspended { .. } => (),
                    Advance::AwaitingDecision(r) => {
                        if let Request::DamageEffect { candidates, .. } = r {
                            recover(&g);
                            let c = &candidates[0];
                            answer(
                                &mut g,
                                Response::DamageEffect {
                                    effect: c.effect.clone(),
                                    damage: c.damage,
                                },
                            );
                        } else {
                            passive(&mut g);
                        }
                    }
                    x => panic!("{x:?}"),
                }
                if prevent == 2
                    && matches!(g.state.rules.frame, Frame::Priority)
                    && g.state.rules.turn.step == TurnStep::CombatDamage
                {
                    break;
                }
            }
            if prevent == 2 {
                assert_eq!(g.state.rules.next_trigger_id, 0);
                assert_eq!(g.state.rules.players[1].life, 40.into());
            } else {
                assert_eq!(g.state.rules.pending_triggers.len(), 1);
                let d = actual(&g);
                let expected = 40
                    - g.state.rules.players[1]
                        .life
                        .to_string()
                        .parse::<i64>()
                        .unwrap();
                assert_eq!(d.amount, expected.into());
                assert!(d.amount > Quantity::zero());
                placed(&mut g);
                resolve(&mut g);
            }
            recover(&g);
        }
        // Explicit synthetic zero-power source: no damage occurrence exists.
        let mut p = prepared(&[(FOX, 0, 0, Zone::Battlefield)]);
        edit(&mut p, FOX, |d| d.characteristics.power = Some(0.into()));
        let mut g = Game::with_trace(p, mode);
        declare(&mut g, &[(0, 1)]);
        to_step(&mut g, 1, TurnStep::PostcombatMain);
        assert_eq!(g.state.rules.next_trigger_id, 0);
        recover(&g);
    }
}
#[test]
fn d2ce_trample_split_lethal_source_observed_before_sba_and_separate_lki() {
    let bear = named("Grizzly Bears");
    for mode in modes() {
        let mut p = prepared(&[
            (FOX, 2, 0, Zone::Battlefield),
            (&bear, 1, 1, Zone::Battlefield),
        ]);
        // Explicit synthetic composition: 3/2 trample Fox and a real 2/2 blocker.
        edit(&mut p, FOX, |d| {
            d.keywords.push(KeywordAbility::Trample);
            d.characteristics.power = Some(3.into());
        });
        let mut g = Game::with_trace(p, mode);
        let original = object(&g, 0);
        let blocker = object(&g, 1);
        declare(&mut g, &[(0, 1)]);
        for _ in 0..30 {
            if matches!(settle(&mut g), Request::Blockers { .. }) {
                break;
            }
            passive(&mut g);
        }
        answer(
            &mut g,
            Response::Blockers {
                blocks: vec![(blocker, original)],
            },
        );
        for _ in 0..30 {
            if matches!(settle(&mut g), Request::Damage { .. }) {
                break;
            }
            passive(&mut g);
        }
        answer(
            &mut g,
            Response::TrampleDamage {
                assignments: vec![(blocker, 2.into())],
                player: PlayerId(1),
                amount: 1.into(),
            },
        );
        pending(&mut g);
        let frozen = witness(&g);
        assert_eq!(g.state.rules.pending_triggers.len(), 1);
        assert_eq!(actual(&g).amount, 1.into());
        assert_eq!(g.current(original).unwrap().marked_damage, 2.into());
        assert_eq!(actual(&g).source.marked_damage, 0.into());
        recover(&g);
        placed(&mut g);
        assert!(g.current(original).is_err());
        assert!(g.current(blocker).is_err());
        let top = g.top().unwrap();
        assert_eq!(top.source, original);
        assert_eq!(top.source_facts.as_ref().unwrap().marked_damage, 2.into());
        assert!(matches!(&top.kind,StackKind::Triggered {witness,..} if witness==&frozen));
        recover(&g);
        let n = library(&g, 0);
        resolve(&mut g);
        assert_eq!(library(&g, 0), n - 1);
        recover(&g);
    }
}
#[test]
fn d2ce_exact_source_noncombat_and_life_loss_never_trigger() {
    let sorcerer = named("Prodigal Sorcerer");
    let feast = named("Soul Feast");
    let activation = catalog()
        .definitions
        .iter()
        .find(|d| d.oracle_id == sorcerer)
        .unwrap()
        .activated
        .clone();
    for mode in modes() {
        let mut p = prepared(&[
            (FOX, 2, 0, Zone::Battlefield),
            (&feast, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        ]);
        edit(&mut p, FOX, |d| d.activated = activation.clone());
        let mut g = Game::with_trace(p, mode);
        let source = object(&g, 0);
        answer(
            &mut g,
            Response::ActivateAbility {
                object: source,
                ability: 0,
            },
        );
        answer(
            &mut g,
            Response::EntityTargets {
                entities: vec![TargetEntity::Player {
                    player: PlayerId(1),
                }],
            },
        );
        placed(&mut g);
        resolve(&mut g);
        assert_eq!(g.state.rules.players[1].life, 39.into());
        assert_eq!(g.state.rules.next_trigger_id, 0);
        recover(&g);
        answer(&mut g, Response::Cast { card: CardId(1) });
        answer(
            &mut g,
            Response::PlayerTargets {
                players: vec![PlayerId(1)],
            },
        );
        fund(&mut g);
        placed(&mut g);
        resolve(&mut g);
        assert_eq!(g.state.rules.next_trigger_id, 0);
        recover(&g);
    }
}
#[test]
fn d2ce_pending_placed_resolving_recovery_and_coherent_forgery() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (FOX, 2, 0, Zone::Battlefield),
                (FOX, 2, 0, Zone::Battlefield),
            ]),
            mode,
        );
        declare(&mut g, &[(0, 1), (1, 1)]);
        let (mut pending_seen, mut placed_seen, mut resolving_seen) = (false, false, false);
        for _ in 0..180 {
            recover(&g);
            pending_seen |= !g.state.rules.pending_triggers.is_empty();
            placed_seen |= !g.state.rules.stack.is_empty();
            resolving_seen |= matches!(g.state.rules.frame, Frame::Resolve { .. });
            let mut s: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
            let mut paths = vec![];
            for i in 0..g.state.rules.pending_triggers.len() {
                paths.push(format!("/state/pending_triggers/{i}/witness"));
            }
            for i in 0..g.state.rules.stack.len() {
                paths.push(format!("/state/stack/{i}/kind/witness"));
            }
            if matches!(g.state.rules.frame, Frame::Resolve { .. }) {
                paths.push("/state/frame/frame/spell/kind/witness".into());
            }
            if !paths.is_empty() {
                for field in [
                    "kind",
                    "amount",
                    "member",
                    "source",
                    "recipient",
                    "transaction",
                ] {
                    let mut forged = s.clone();
                    for path in &paths {
                        let w = forged.pointer_mut(path).unwrap();
                        match field {
                            "kind" => w["kind"] = "noncombat".into(),
                            "amount" => w["damage"]["amount"] = "7".into(),
                            "member" => w["damage"]["id"] = 1.into(),
                            "source" => w["damage"]["source"]["object"]["card"] = 1.into(),
                            "recipient" => w["damage"]["recipient"]["player"] = 3.into(),
                            "transaction" => w["transaction"] = serde_json::Value::Null,
                            _ => unreachable!(),
                        }
                    }
                    // Keep identity-bearing invocation/source copies coherent for
                    // the source lie, instead of failing only a cross-field check.
                    if field == "source" {
                        let handles = serde_json::to_value(
                            g.capture_trigger_source_handles(object(&g, 1)).unwrap(),
                        )
                        .unwrap();
                        for path in &paths {
                            let parent = path.trim_end_matches("/witness");
                            let entry = parent.trim_end_matches("/kind");
                            let node = forged.pointer_mut(entry).unwrap();
                            node["source"]["card"] = 1.into();
                            node["source_facts"]["object"]["card"] = 1.into();
                            if parent.ends_with("/kind") {
                                node["kind"]["source_handles"] = handles.clone();
                            } else {
                                node["source_handles"] = handles.clone();
                            }
                        }
                    }
                    forged["state_sha256"] = digest(&forged["state"]).unwrap().into();
                    if ["amount", "member", "source", "recipient", "transaction"].contains(&field) {
                        let decoded: Snapshot = serde_json::from_value(forged.clone()).unwrap();
                        let mut structural = g.fork();
                        structural.state = decoded.state;
                        structural.validate_trigger_state().unwrap();
                        if let Frame::Resolve { frame } = &structural.state.rules.frame {
                            structural.validate_resolving(frame).unwrap();
                        }
                    }
                    assert!(
                        Game::restore(g.prepared.as_ref().clone(), &forged.to_string()).is_err(),
                        "{field}"
                    );
                }
                // Unknown and zero/negative members fail strict structural checks.
                for (field, value) in [
                    ("amount", serde_json::json!("0")),
                    ("amount", serde_json::json!("-1")),
                    ("id", serde_json::json!(u32::MAX)),
                ] {
                    let w = s.pointer_mut(&paths[0]).unwrap();
                    w["damage"][field] = value;
                    s["state_sha256"] = digest(&s["state"]).unwrap().into();
                    assert!(Game::restore(g.prepared.as_ref().clone(), &s.to_string()).is_err());
                }
            }
            if library(&g, 0) == 6 {
                break;
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(_) => passive(&mut g),
                x => panic!("{x:?}"),
            }
        }
        assert!(pending_seen && placed_seen && resolving_seen);
        assert_eq!(g.state.rules.next_trigger_id, 2);
        recover(&g);
    }
}
#[test]
fn d2ce_source_reentry_does_not_rebind_historical_witness() {
    let unsummon = named("Unsummon");
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (FOX, 0, 0, Zone::Battlefield),
                (&unsummon, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            ]),
            mode,
        );
        declare(&mut g, &[(0, 1)]);
        pending(&mut g);
        let old = object(&g, 0);
        let frozen = witness(&g);
        placed(&mut g);
        answer(&mut g, Response::Cast { card: CardId(1) });
        answer(&mut g, Response::Targets { objects: vec![old] });
        fund(&mut g);
        placed(&mut g);
        resolve(&mut g);
        assert!(g.current(old).is_err());
        recover(&g);
        // Explicit private event composition tests a fresh incarnation while
        // the independently triggered invocation remains on the stack.
        let fresh = object(&g, 0);
        g.commit(
            ProposedEvent::Move {
                object: fresh,
                destination: Zone::Battlefield,
            },
            BatchCause::Fixture,
        )
        .unwrap();
        assert_ne!(object(&g, 0), old);
        assert!(
            matches!(&g.top().unwrap().kind,StackKind::Triggered {witness,..} if witness==&frozen)
        );
        let n = library(&g, 0);
        resolve(&mut g);
        assert_eq!(library(&g, 0), n - 1);
        // Fixture events are intentionally not accepted-input replay claims.
    }
}
#[test]
fn d2ce_owner_controller_recipient_departures_pending_and_placed_all_traces() {
    for mode in modes() {
        for in_pending in [false, true] {
            for departing in 0..4 {
                let mut g = Game::with_trace(prepared(&[(FOX, 2, 0, Zone::Battlefield)]), mode);
                declare(&mut g, &[(0, 1)]);
                pending(&mut g);
                let frozen = witness(&g);
                if !in_pending {
                    placed(&mut g);
                }
                g.concede(PlayerId(departing)).unwrap();
                placed(&mut g);
                recover(&g);
                assert_eq!(g.state.rules.stack.len(), usize::from(departing != 0));
                if departing != 0 {
                    assert!(
                        matches!(&g.top().unwrap().kind,StackKind::Triggered {witness,..} if witness==&frozen)
                    );
                    let n = library(&g, 0);
                    resolve(&mut g);
                    assert_eq!(library(&g, 0), n - 1);
                    recover(&g);
                }
            }
        }
    }
}
#[test]
fn d2ce_stifle_counters_invocation_without_refunding_combat_damage() {
    let stifle = named("Stifle");
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (FOX, 2, 0, Zone::Battlefield),
                (&stifle, 1, 1, Zone::Hand { owner: PlayerId(1) }),
            ]),
            mode,
        );
        declare(&mut g, &[(0, 1)]);
        pending(&mut g);
        placed(&mut g);
        let n = library(&g, 0);
        let id = g.top().unwrap().id;
        priority(&mut g, 1);
        answer(&mut g, Response::Cast { card: CardId(1) });
        answer(&mut g, Response::StackTargets { entries: vec![id] });
        fund(&mut g);
        placed(&mut g);
        resolve(&mut g);
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(library(&g, 0), n);
        assert_eq!(g.state.rules.players[1].life, 38.into());
        assert_eq!(g.state.rules.next_trigger_id, 1);
        recover(&g);
    }
}
fn member(g: &Game, card: u32, player: u8, amount: i64) -> DamageOccurrence {
    let source = object(g, card);
    DamageOccurrence {
        source,
        controller: g.current(source).unwrap().controller,
        recipient: DamageRecipient::Player {
            player: PlayerId(player),
        },
        amount: amount.into(),
    }
}
#[test]
fn d2ce_legacy_actual_members_any_player_and_combat_duplicate_atomic_rejection() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (FOX, 2, 0, Zone::Battlefield),
                (FOX, 2, 0, Zone::Battlefield),
            ]),
            mode,
        );
        let a = member(&g, 0, 0, 1);
        let b = member(&g, 1, 0, 2);
        // Genuine legacy typed combat leaf; no current combat flags. Self is
        // still "a player", independent of the damage-source controller.
        g.commit(
            ProposedEvent::CombatDamage {
                damage: vec![a.clone(), b],
            },
            BatchCause::Fixture,
        )
        .unwrap();
        assert_eq!(g.state.rules.pending_triggers.len(), 2);
        assert_eq!(g.state.rules.players[0].life, 37.into());
        assert!(matches!(
            witness(&g),
            TriggerEventWitness::DamageDealt {
                transaction: None,
                kind: DamageKind::Combat,
                ..
            }
        ));
        let state = g.state.clone();
        let trace = g.trace.clone();
        let journal = g.replay_journal();
        assert!(
            g.commit(
                ProposedEvent::CombatDamage {
                    damage: vec![a.clone(), a.clone()]
                },
                BatchCause::Fixture
            )
            .is_err()
        );
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        assert_eq!(g.replay_journal(), journal);
        // Preparation fails before consuming a transaction ID or clearing choices.
        assert!(g.prepare_combat_damage(vec![a.clone(), a]).is_err());
        assert_eq!(g.state, state);
    }
}
#[test]
fn d2ce_legacy_damage_capacity_boundary_typed_fault_and_atomic_preservation() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (FOX, 2, 0, Zone::Battlefield),
                (FOX, 1, 1, Zone::Battlefield),
            ]),
            mode,
        );
        let capacity = g.damage_limit();
        let target = object(&g, 1);
        let mut edge = member(&g, 0, 1, 1);
        edge.recipient = DamageRecipient::Creature { object: target };
        // The legacy leaf permits repeated creature edges. These form a valid
        // at-limit input without violating combat source/player uniqueness.
        // Direct fixture commits do not run later SBAs or claim input replay.
        let outcome = g
            .commit(
                ProposedEvent::CombatDamage {
                    damage: vec![edge.clone(); capacity],
                },
                BatchCause::Fixture,
            )
            .unwrap();
        assert!(
            matches!(outcome, EventOutcome::DamageDealt { damage, .. } if damage.len() == capacity)
        );
        assert_eq!(
            g.current(target).unwrap().marked_damage,
            (capacity as i64).into()
        );
        assert!(g.state.rules.pending_triggers.is_empty());
        assert!(g.typed_fault().is_none());
        for legacy in [true, false] {
            let mut overflow = g.fork();
            let mut before = overflow.state.clone();
            let trace = overflow.trace.clone();
            let journal = overflow.replay_journal();
            let error = if legacy {
                overflow
                    .commit(
                        ProposedEvent::CombatDamage {
                            damage: vec![edge.clone(); capacity + 1],
                        },
                        BatchCause::Fixture,
                    )
                    .unwrap_err()
            } else {
                // Preserve the existing capacity fault even when an oversized
                // preparation also contains duplicate player edges.
                let player_edge = member(&overflow, 0, 1, 1);
                overflow
                    .prepare_combat_damage(vec![player_edge; capacity + 1])
                    .unwrap_err()
            };
            assert_eq!(error, "damage capacity exceeded");
            let expected = RuleFault::DamageCapacityExceeded {
                capacity,
                requested: capacity + 1,
            };
            assert_eq!(overflow.typed_fault(), Some(&expected));
            before.rules.typed_fault = Some(expected);
            assert_eq!(overflow.state, before);
            assert_eq!(overflow.trace, trace);
            assert_eq!(overflow.replay_journal(), journal);
        }
    }
}
#[test]
fn d2ce_transaction_restore_validation_rejects_split_combat_player_member() {
    let mut g = Game::new(prepared(&[(FOX, 0, 0, Zone::Battlefield)]));
    let d = member(&g, 0, 1, 2);
    g.prepare_combat_damage(vec![d]).unwrap();
    let mut tx = g.damage_transaction().unwrap().clone();
    tx.entries[0].amount = 1.into();
    tx.entries[0].original_amount = 1.into();
    let mut split = tx.entries[0].clone();
    split.id = DamageId(1);
    tx.entries.push(split);
    assert!(
        g.validate_damage_transaction(&tx)
            .unwrap_err()
            .contains("duplicate combat source/player")
    );
    tx.kind = DamageKind::Noncombat;
    // The same guard is deliberately absent for noncombat. This synthetic
    // unbound continuation still rejects for its independent kind/shape fault.
    assert!(
        !g.validate_damage_transaction(&tx)
            .unwrap_err()
            .contains("duplicate combat source/player")
    );
}
#[test]
fn d2ce_repeated_noncombat_and_direct_life_events_do_not_restrict_or_trigger() {
    let mut g = Game::new(prepared(&[(FOX, 0, 0, Zone::Battlefield)]));
    let source = g.object_before(object(&g, 0)).unwrap();
    let event = ProposedEvent::NoncombatDamage {
        damage: Box::new(NoncombatDamageOccurrence {
            controller: PlayerId(0),
            source,
            recipient: DamageRecipient::Player {
                player: PlayerId(1),
            },
            amount: 1.into(),
        }),
    };
    g.commit_batch(
        BatchCause::Fixture,
        EventGroup::Ordered {
            children: vec![
                EventGroup::Occurrence {
                    proposed: event.clone(),
                },
                EventGroup::Occurrence { proposed: event },
                EventGroup::Occurrence {
                    proposed: ProposedEvent::LoseLife {
                        player: PlayerId(1),
                        amount: 2.into(),
                    },
                },
            ],
        },
    )
    .unwrap();
    assert_eq!(g.state.rules.players[1].life, 36.into());
    assert_eq!(g.state.rules.next_trigger_id, 0);
}
#[test]
fn d2ce_departure_before_commit_does_not_observe_absent_source() {
    for mode in modes() {
        let mut g = Game::with_trace(prepared(&[(FOX, 2, 0, Zone::Battlefield)]), mode);
        declare(&mut g, &[(0, 1)]);
        damage_boundary(&mut g);
        g.concede(PlayerId(2)).unwrap();
        to_step(&mut g, 1, TurnStep::PostcombatMain);
        assert_eq!(g.state.rules.next_trigger_id, 0);
        recover(&g);
    }
}
#[test]
fn d2ce_commit_faults_roll_back_life_commander_ids_pending_trace_journal_and_charges() {
    let commander = named("Isamaru, Hound of Konda");
    for mode in modes() {
        for fault in ["trigger_id", "history", "stack", "batch_id"] {
            let mut p = prepared(&[
                (FOX, 0, 0, Zone::Battlefield),
                (FOX, 0, 0, Zone::Battlefield),
                (&commander, 0, 0, Zone::Battlefield),
            ]);
            // Private scenario witness: designate an actually eligible source,
            // so this transaction includes real physical Commander damage.
            p.config.commanders.push(CardId(2));
            p.profile_digest = digest(&p.config).unwrap();
            let mut g = Game::with_trace(p, mode);
            declare(&mut g, &[(0, 1), (1, 1), (2, 1)]);
            damage_boundary(&mut g);
            if fault == "trigger_id" {
                g.state.rules.next_trigger_id = u64::MAX;
            }
            if fault == "batch_id" {
                g.state.rules.next_batch_id = u64::MAX;
            }
            if fault == "history" {
                let n = g.history_charge();
                g.state.rules.retained_auxiliary_charge += capacities::HISTORY_LIMIT - n;
            }
            if fault == "stack" {
                let mut sample = g.fork();
                pending(&mut sample);
                let t = sample.state.rules.pending_triggers[0].clone();
                g.state
                    .rules
                    .pending_triggers
                    .resize(capacities::STACK_LIMIT - 1, t);
            }
            let before = g.state.clone();
            let trace = g.trace.clone();
            let journal = g.replay_journal();
            assert!(
                matches!(g.advance(Budget::new(1)), Advance::Fault(_)),
                "{fault}"
            );
            let mut after = g.state.clone();
            after.rules.frame = before.rules.frame.clone();
            after.rules.typed_fault = before.rules.typed_fault.clone();
            assert_eq!(after, before, "{fault}");
            assert_eq!(g.trace, trace);
            assert_eq!(g.replay_journal(), journal);
        }
    }
}
#[test]
fn d2ce_hosting_search_remain_excluded_and_reporting_key_stable() {
    assert_eq!(
        TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player
        }
        .reporting_key(),
        "damage_dealt"
    );
    let c = catalog();
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut p = match_profile("commander-foundations-v1").unwrap().clone();
        p.search = search;
        p.hosted = hosted;
        assert!(
            matches!(p.validate_requested_features(&c),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::DamageTriggerV1))
        );
    }
}
#[test]
fn d2ce_multi_source_finite_shield_allocation_restore_and_no_recollection() {
    let salve = named("Healing Salve");
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (FOX, 0, 0, Zone::Battlefield),
                (FOX, 0, 0, Zone::Battlefield),
                (&salve, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            ]),
            mode,
        );
        answer(&mut g, Response::Cast { card: CardId(2) });
        answer(&mut g, Response::Modes { mode: 1 });
        answer(
            &mut g,
            Response::EntityTargets {
                entities: vec![TargetEntity::Player {
                    player: PlayerId(1),
                }],
            },
        );
        fund(&mut g);
        placed(&mut g);
        resolve(&mut g);
        declare(&mut g, &[(0, 1), (1, 1)]);
        let mut allocation_seen = false;
        for _ in 0..150 {
            if !g.state.rules.pending_triggers.is_empty() {
                break;
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(Request::DamageEffect { candidates, .. }) => {
                    recover(&g);
                    let c = &candidates[0];
                    answer(
                        &mut g,
                        Response::DamageEffect {
                            effect: c.effect.clone(),
                            damage: c.damage,
                        },
                    );
                }
                Advance::AwaitingDecision(Request::DamageAllocation {
                    candidates,
                    remaining,
                    required,
                    ..
                }) => {
                    assert_eq!(remaining, 3.into());
                    assert_eq!(required, 3.into());
                    assert_eq!(candidates.len(), 2);
                    allocation_seen = true;
                    recover(&g);
                    // The real Salve has three points: prevent the second Fox's
                    // two damage and one from the first, leaving one actual
                    // damage member and exactly one invocation.
                    let prevented_source = g
                        .damage_transaction()
                        .unwrap()
                        .entries
                        .iter()
                        .find(|e| e.source.object.card == CardId(1))
                        .unwrap()
                        .id;
                    let allocations = candidates
                        .iter()
                        .map(|c| {
                            (
                                c.damage,
                                if c.damage == prevented_source {
                                    2.into()
                                } else {
                                    1.into()
                                },
                            )
                        })
                        .collect();
                    answer(&mut g, Response::DamageAllocation { allocations });
                }
                Advance::AwaitingDecision(_) => passive(&mut g),
                x => panic!("{x:?}"),
            }
        }
        assert!(allocation_seen);
        assert_eq!(g.state.rules.pending_triggers.len(), 1);
        assert_eq!(actual(&g).source.object.card, CardId(0));
        assert_eq!(actual(&g).amount, 1.into());
        assert_eq!(g.state.rules.players[1].life, 39.into());
        recover(&g);
        placed(&mut g);
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.rules.next_trigger_id, 1);
        recover(&g);
    }
}
#[test]
fn d2ce_sba_deaths_join_damage_invocation_in_common_apnap_placement() {
    let bear = named("Grizzly Bears");
    let artist = named("Blood Artist");
    for mode in modes() {
        let mut p = prepared(&[
            (FOX, 2, 0, Zone::Battlefield),
            (&bear, 1, 1, Zone::Battlefield),
            (&artist, 3, 3, Zone::Battlefield),
        ]);
        edit(&mut p, FOX, |d| {
            d.characteristics.power = Some(3.into());
            d.keywords.push(KeywordAbility::Trample);
        });
        let mut g = Game::with_trace(p, mode);
        let a = object(&g, 0);
        let b = object(&g, 1);
        declare(&mut g, &[(0, 1)]);
        for _ in 0..30 {
            if matches!(settle(&mut g), Request::Blockers { .. }) {
                break;
            }
            passive(&mut g);
        }
        answer(
            &mut g,
            Response::Blockers {
                blocks: vec![(b, a)],
            },
        );
        for _ in 0..30 {
            if matches!(settle(&mut g), Request::Damage { .. }) {
                break;
            }
            passive(&mut g);
        }
        answer(
            &mut g,
            Response::TrampleDamage {
                assignments: vec![(b, 2.into())],
                player: PlayerId(1),
                amount: 1.into(),
            },
        );
        pending(&mut g);
        assert_eq!(g.state.rules.pending_triggers.len(), 1);
        let mut order_players = vec![];
        for _ in 0..20 {
            recover(&g);
            match settle(&mut g) {
                Request::Priority { .. } => break,
                Request::OrderTriggers {
                    player, candidates, ..
                } => {
                    order_players.push(player);
                    answer(&mut g, Response::OrderTriggers { order: candidates });
                }
                Request::PlayerTargets { .. } => answer(
                    &mut g,
                    Response::PlayerTargets {
                        players: vec![PlayerId(1)],
                    },
                ),
                r => panic!("APNAP {r:?}"),
            }
        }
        assert_eq!(order_players, vec![PlayerId(3)]); // controller 0 has just one automatic placement
        assert_eq!(
            g.state
                .rules
                .stack
                .iter()
                .map(|s| s.controller)
                .collect::<Vec<_>>(),
            vec![PlayerId(0), PlayerId(3), PlayerId(3)]
        );
        assert!(g.current(a).is_err());
        assert!(g.current(b).is_err());
        recover(&g);
    }
}
#[test]
fn d2ce_preparation_transaction_id_fault_rolls_back_choices_and_every_id() {
    for mode in modes() {
        let mut g = Game::with_trace(prepared(&[(FOX, 0, 0, Zone::Battlefield)]), mode);
        declare(&mut g, &[(0, 1)]);
        pass_until(&mut g, |g| matches!(g.state.rules.frame, Frame::Damage));
        g.state.rules.next_damage_transaction_id = u64::MAX;
        let before = g.state.clone();
        let trace = g.trace.clone();
        let journal = g.replay_journal();
        assert!(matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
        let mut after = g.state.clone();
        after.rules.frame = before.rules.frame.clone();
        after.rules.typed_fault = before.rules.typed_fault.clone();
        assert_eq!(after, before);
        assert_eq!(g.trace, trace);
        assert_eq!(g.replay_journal(), journal);
    }
}
#[test]
fn d2ce_damage_witness_strict_unknown_fields_and_branch_independence() {
    let mut g = Game::new(prepared(&[(FOX, 0, 0, Zone::Battlefield)]));
    declare(&mut g, &[(0, 1)]);
    pending(&mut g);
    recover(&g);
    let mut w = serde_json::to_value(witness(&g)).unwrap();
    w["unknown"] = true.into();
    assert!(serde_json::from_value::<TriggerEventWitness>(w).is_err());
    let mut w = serde_json::to_value(witness(&g)).unwrap();
    w["damage"]["unknown"] = true.into();
    assert!(serde_json::from_value::<TriggerEventWitness>(w).is_err());
    let before = g.state.clone();
    let mut fork = g.fork();
    fork.concede(PlayerId(3)).unwrap();
    placed(&mut fork);
    resolve(&mut fork);
    recover(&fork);
    assert_eq!(g.state, before);
    recover(&g);
}

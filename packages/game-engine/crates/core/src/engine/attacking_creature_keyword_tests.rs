//! Complete frozen Ohran Frostfang and ordinary combat; no card definition is
//! edited to manufacture a granted keyword. The control-transition probe below
//! is explicitly a private kernel probe, not an admitted control spell.
use super::*;

const OHRAN: &str = "b99ada26-9a61-4175-9fb8-15a106960220";
const DREADMAW: &str = "08c7db90-c0cf-4482-b7ee-bb033e5996d2";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";

fn prepared(rows: &[(&str, u8, u8)]) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/b99ada26-9a61-4175-9fb8-15a106960220.json"
        ),
    ) else {
        panic!("complete Ohran Frostfang source must lower");
    };
    assert!(definition.characteristics.snow);
    assert!(
        definition
            .capabilities
            .contains(&Capability::SnowSupertypeV1)
    );
    assert!(
        definition
            .capabilities
            .contains(&Capability::AttackingCreatureKeywordsV1)
    );
    assert!(definition.keywords.is_empty());
    c.definitions.push(*definition);
    c.schema_version = 33;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "attacking-creature-keywords-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(id, (oracle, owner, controller))| CardSetup {
            id: CardId(id as u32),
            oracle_id: (*oracle).into(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: Zone::Battlefield,
        })
        .collect();
    for seat in 0..4 {
        for _ in 0..4 {
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
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        catalog: Arc::new(c),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn request(g: &mut Game) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            x => panic!("attacking grant at {:?}: {x:?}", g.state.rules.frame),
        }
    }
    panic!("attacking grant budget");
}
fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game, r: Request) {
    let response = match &r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        x => panic!("unexpected passive request {x:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn declare(g: &mut Game, attacks: &[(u32, u8)], blocks: &[(u32, u32)]) {
    for _ in 0..100 {
        let r = request(g);
        if matches!(r, Request::Attackers { .. }) {
            break;
        }
        passive(g, r);
    }
    let attacks = attacks
        .iter()
        .map(|(c, p)| (object(g, *c), PlayerId(*p)))
        .collect();
    answer(g, Response::Attackers { attacks });
    for _ in 0..100 {
        let r = request(g);
        if matches!(r, Request::Blockers { .. }) {
            let blocks = blocks
                .iter()
                .map(|(b, a)| (object(g, *b), object(g, *a)))
                .collect();
            g.respond(r.player(), r.decision_id(), Response::Blockers { blocks })
                .unwrap();
            return;
        }
        passive(g, r);
    }
    panic!("missing blockers");
}
fn damage_request(g: &mut Game) -> Request {
    for _ in 0..100 {
        let r = request(g);
        if matches!(r, Request::Damage { .. }) {
            return r;
        }
        passive(g, r);
    }
    panic!("missing damage assignment");
}
fn end_combat(g: &mut Game) {
    for _ in 0..200 {
        let r = request(g);
        if g.state.rules.turn.step == TurnStep::EndCombat {
            return;
        }
        passive(g, r);
    }
    panic!("missing end combat");
}
fn postcombat_main(g: &mut Game) {
    for _ in 0..100 {
        let r = request(g);
        if g.state.rules.turn.step == TurnStep::PostcombatMain {
            assert!(matches!(r, Request::Priority { .. }));
            return;
        }
        passive(g, r);
    }
    panic!("missing postcombat main");
}
fn placed(g: &mut Game) {
    for _ in 0..100 {
        let r = request(g);
        if !g.state.rules.stack.is_empty() {
            assert!(matches!(r, Request::Priority { .. }));
            return;
        }
        passive(g, r);
    }
    panic!("missing damage trigger");
}
fn hand(g: &Game, player: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone
                == (Zone::Hand {
                    owner: PlayerId(player),
                })
        })
        .count()
}
fn touch(g: &Game, card: u32) -> bool {
    g.current_keywords(object(g, card))
        .contains(&KeywordAbility::Deathtouch)
}
fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation");
    };
    o
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace, g.trace);
    let replayed = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace, g.trace);
}

#[test]
fn attacking_creature_keywords_rearline_source_trample_lethal_one_public_assignment_and_draw() {
    let mut g = Game::with_trace(
        prepared(&[(OHRAN, 2, 0), (DREADMAW, 0, 0), (OHRAN, 1, 1)]),
        TraceMode::Detailed,
    );
    declare(&mut g, &[(1, 1)], &[(2, 1)]);
    let r = damage_request(&mut g);
    let blocker = object(&g, 2);
    assert!(
        matches!(r,Request::Damage {ref lethal_requirements,player_recipient:Some(PlayerId(1)),..} if lethal_requirements==&vec![(blocker,1.into())])
    );
    assert!(touch(&g, 1));
    assert!(!touch(&g, 0));
    assert!(!touch(&g, 2));
    recover(&g);
    let before = g.state.clone();
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::TrampleDamage {
                assignments: vec![(blocker, 0.into())],
                player: PlayerId(1),
                amount: 6.into()
            }
        )
        .is_err()
    );
    assert_eq!(before, g.state);
    let Some(PlayerRequest::Damage {
        token,
        recipients,
        lethal_requirements,
        ..
    }) = view(&g, 0).decision
    else {
        panic!("public damage request");
    };
    assert_eq!(lethal_requirements[0].1, 1.into());
    assert!(view(&g, 1).decision.is_none());
    let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
        PlayerId(0),
        token,
        PlayerResponse::TrampleDamage {
            assignments: vec![(recipients[0].clone(), 1.into())],
            player: PlayerId(1),
            amount: 5.into(),
        },
        Budget::unlimited(),
    ) else {
        panic!("public assignment");
    };
    g.submit(receipt).unwrap();
    end_combat(&mut g);
    assert_eq!(g.state.rules.players[1].life, 35.into());
    assert_eq!(
        g.state.rules.objects[&CardId(2)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_eq!(g.current(object(&g, 1)).unwrap().marked_damage, 2.into());
    assert_eq!(hand(&g, 0), 1);
    assert_eq!(hand(&g, 1), 0);
    recover(&g);
}

#[test]
fn attacking_creature_keywords_scope_uses_attack_membership_and_includes_source_itself() {
    let mut g = Game::new(prepared(&[
        (OHRAN, 0, 0),
        (DREADMAW, 0, 0),
        (OHRAN, 1, 1),
        (DREADMAW, 0, 0),
    ]));
    for card in 0..4 {
        assert!(!touch(&g, card));
    }
    declare(&mut g, &[(0, 1)], &[(2, 0)]);
    assert!(touch(&g, 0));
    assert!(!touch(&g, 1));
    assert!(!touch(&g, 2));
    assert!(!touch(&g, 3));
    // Rear-line readiness does not substitute for actual attack membership.
    assert!(!g.current(object(&g, 1)).unwrap().tapped);
    end_combat(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(2)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert!(g.object_id(CardId(0)).is_some());
    // CR 508.1k/511.3: attacking status lasts through the end-of-combat
    // step, and disappears only as the combat phase ends.
    assert!(touch(&g, 0));
    assert_eq!(hand(&g, 0), 0);
    recover(&g);
    postcombat_main(&mut g);
    assert!(!touch(&g, 0));
    let mut unblocked = Game::new(prepared(&[(OHRAN, 0, 0)]));
    declare(&mut unblocked, &[(0, 1)], &[]);
    assert!(touch(&unblocked, 0));
    end_combat(&mut unblocked);
    assert_eq!(unblocked.state.rules.players[1].life, 38.into());
    assert_eq!(hand(&unblocked, 0), 1);
    assert!(touch(&unblocked, 0));
    postcombat_main(&mut unblocked);
    assert!(!touch(&unblocked, 0));
    recover(&unblocked);
}

#[test]
fn attacking_creature_keywords_duplicate_grants_are_idempotent_but_observers_draw_twice() {
    let mut g = Game::new(prepared(&[(OHRAN, 0, 0), (OHRAN, 2, 0), (DREADMAW, 0, 0)]));
    declare(&mut g, &[(2, 1)], &[]);
    assert_eq!(
        g.current_keywords(object(&g, 2))
            .iter()
            .filter(|k| **k == KeywordAbility::Deathtouch)
            .count(),
        1
    );
    placed(&mut g);
    assert_eq!(g.state.rules.stack.len(), 2);
    recover(&g);
    end_combat(&mut g);
    assert_eq!(g.state.rules.players[1].life, 34.into());
    assert_eq!(hand(&g, 0), 2);
    assert_eq!(g.state.rules.next_trigger_id, 2);
    recover(&g);
}

#[test]
fn attacking_creature_keywords_owner_departure_changes_future_preparation_not_frozen_damage() {
    for frozen in [false, true] {
        let mut g = Game::new(prepared(&[(OHRAN, 2, 0), (DREADMAW, 0, 0), (OHRAN, 1, 1)]));
        declare(&mut g, &[(1, 1)], &[(2, 1)]);
        if frozen {
            let _ = damage_request(&mut g);
            let blocker = object(&g, 2);
            answer(
                &mut g,
                Response::TrampleDamage {
                    assignments: vec![(blocker, 1.into())],
                    player: PlayerId(1),
                    amount: 5.into(),
                },
            );
            for _ in 0..100 {
                if g.damage_transaction().is_ok() {
                    break;
                }
                assert!(matches!(
                    g.advance(Budget::new(1)),
                    Advance::Suspended { .. }
                ));
            }
            assert!(
                g.damage_transaction()
                    .unwrap()
                    .entries
                    .iter()
                    .any(|e| e.source.object.card == CardId(1)
                        && e.source.keywords.contains(&KeywordAbility::Deathtouch))
            );
        }
        g.concede(PlayerId(2)).unwrap();
        let _ = g.advance(Budget::new(1));
        assert!(g.object_id(CardId(0)).is_none());
        assert!(!touch(&g, 1));
        if !frozen {
            let r = damage_request(&mut g);
            let blocker = object(&g, 2);
            assert!(
                matches!(r,Request::Damage {ref lethal_requirements,..} if lethal_requirements==&vec![(blocker,6.into())])
            );
            answer(
                &mut g,
                Response::TrampleDamage {
                    assignments: vec![(blocker, 6.into())],
                    player: PlayerId(1),
                    amount: 0.into(),
                },
            );
        }
        end_combat(&mut g);
        assert_eq!(
            g.state.rules.players[1].life,
            if frozen { 35.into() } else { 40.into() }
        );
        assert_eq!(
            g.state.rules.objects[&CardId(2)].zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        assert_eq!(hand(&g, 0), 0);
        recover(&g);
    }
}

#[test]
fn attacking_creature_keywords_private_kernel_control_transition_rechecks_current_controller() {
    // No control-changing spell is admitted here. Mutate only the kernel's
    // current object controller to probe the continuous predicate; no replay or
    // card-level support claim is made for these synthetic control transitions.
    let mut g = Game::new(prepared(&[
        (OHRAN, 0, 0),
        (DREADMAW, 0, 0),
        (DREADMAW, 0, 0),
    ]));
    declare(&mut g, &[(1, 1)], &[]);
    assert!(touch(&g, 1));
    assert!(!touch(&g, 2));
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .controller = PlayerId(3);
    assert!(!touch(&g, 1));
    g.state
        .rules
        .objects
        .get_mut(&CardId(0))
        .unwrap()
        .controller = PlayerId(3);
    assert!(touch(&g, 1));
    assert!(!touch(&g, 2));
    g.state
        .rules
        .objects
        .get_mut(&CardId(0))
        .unwrap()
        .controller = PlayerId(0);
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .controller = PlayerId(0);
    assert!(touch(&g, 1));
}

#[test]
fn attacking_creature_keywords_snow_public_source_facts_restore_and_feature_admission() {
    let p = prepared(&[(OHRAN, 2, 0), (DREADMAW, 0, 0)]);
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::SnowSupertypeV1)&&capabilities.contains(&Capability::AttackingCreatureKeywordsV1))
        );
    }
    let mut g = Game::new(p);
    for viewer in 0..4 {
        let o = view(&g, viewer);
        assert!(
            o.view
                .objects
                .iter()
                .find(|o| o.oracle_id == OHRAN)
                .unwrap()
                .characteristics
                .as_ref()
                .unwrap()
                .snow
        );
        assert!(
            !o.view
                .objects
                .iter()
                .any(|o| matches!(o.zone, Zone::Library { .. }))
        );
    }
    declare(&mut g, &[(1, 1)], &[]);
    placed(&mut g);
    recover(&g);
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let source = forged.state.rules.stack[0].source_facts.as_mut().unwrap();
    assert!(source.characteristics.snow);
    source.characteristics.snow = false;
    forged.state_sha256 = digest(&forged.state).unwrap();
    assert!(
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&forged).unwrap()
        )
        .is_err()
    );
}

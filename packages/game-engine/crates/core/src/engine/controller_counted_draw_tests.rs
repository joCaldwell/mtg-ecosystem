//! Private complete-source paid draws; no deck/profile admission. CR121.2/121.4,
//! 608.2c/704.4/800.4/903.9b, frozen 20260925.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const HARMONIZE: &str =
    include_str!("../../../../tests/fixtures/7eff84f1-f772-497a-b350-bbc93d0230f7.json");
const CONCENTRATE: &str =
    include_str!("../../../../tests/fixtures/a3d287ec-1a39-4f91-acf1-7c1d2c00ed89.json");
const TIDINGS: &str =
    include_str!("../../../../tests/fixtures/72897780-094d-4a21-8b1c-419a9defd2fb.json");
const INGENUITY: &str =
    include_str!("../../../../tests/fixtures/ce588fd3-642e-4df9-8061-6fcfc6e81270.json");
fn setup(name: &'static str, raw: &str, land: &'static str) -> Prepared {
    let mut rows = vec![(name, 0, 0, true)];
    lands(&mut rows, land, 0, 5);
    prepared_with_sources(&rows, &[raw])
}
fn library(g: &Game) -> Vec<ObjectId> {
    g.state.rules.libraries[&PlayerId(0)]
        .iter()
        .map(|c| g.object_id(*c).unwrap())
        .collect()
}
fn step(g: &mut Game) {
    match g.advance(Budget::new(1)) {
        Advance::Suspended { .. } => (),
        Advance::AwaitingDecision(r) => {
            g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
        }
        other => panic!("draw prefix {other:?}"),
    }
}
#[test]
fn controller_counted_draw_paid_complete_sources_exact_order_and_privacy() {
    for (name, raw, land, count) in [
        ("Harmonize", HARMONIZE, "Forest", 3usize),
        ("Concentrate", CONCENTRATE, "Island", 3),
        ("Tidings", TIDINGS, "Island", 4),
        ("Jace's Ingenuity", INGENUITY, "Island", 3),
    ] {
        both(setup(name, raw, land), |g, q| {
            let before = library(g);
            announce(g, q, 0, vec![]);
            assert!(g.state.rules.stack.last().unwrap().paid.is_some());
            recover(g);
            resolve(g, q, |_| {});
            assert_eq!(library(g), before[count..]);
            assert_eq!(g.action_hand(PlayerId(0)).len(), count);
            let state = g.state.clone();
            let trace = g.trace.clone();
            for old in &before[..count] {
                let now = g.object_id(old.card).unwrap();
                assert_eq!(now.incarnation, old.incarnation + 1);
                let o = g.current(now).unwrap();
                assert_eq!(o.zone, Zone::Hand { owner: PlayerId(0) });
                assert!(g.handle_for(PlayerId(0), now).is_ok());
                for seat in 1..4 {
                    assert!(g.handle_for(PlayerId(seat), now).is_err());
                    assert!(g.visible_object(PlayerId(seat), o).unwrap().is_none());
                }
            }
            for seat in 0..4 {
                finish_query(g, g.observe(PlayerId(seat), Budget::new(0)));
            }
            assert_eq!(g.state, state);
            assert_eq!(g.trace, trace);
        });
    }
}
#[test]
fn controller_counted_draw_suspended_progress_departure_and_replay_forgery() {
    both(setup("Concentrate", CONCENTRATE, "Island"), |g, q| {
        announce(g, q, 0, vec![]);
        let mut found = false;
        for _ in 0..1000 {
            if matches!(&g.state.rules.frame,Frame::Resolve{frame} if frame.draw_progress.as_ref().is_some_and(|p|p.completed==1.into()))
            {
                found = true;
                break;
            }
            step(g);
        }
        assert!(found);
        recover(g);
        let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let Frame::Resolve { frame } = &mut bad.state.rules.frame else {
            panic!("draw")
        };
        let progress = frame.draw_progress.as_mut().unwrap();
        progress.completed = 2.into();
        progress.remaining = 1.into();
        let mut candidate = g.fork();
        candidate.state = bad.state.clone();
        candidate.validate_state().unwrap();
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
        let QueryResult::Incomplete { work, .. } = g.observe(PlayerId(1), Budget::new(0)) else {
            panic!("work")
        };
        let other_hands = (1..4)
            .map(|s| g.action_hand(PlayerId(s)).len())
            .collect::<Vec<_>>();
        g.concede(PlayerId(0)).unwrap();
        recover(g);
        resolve(g, q, |_| {});
        assert!(!g.is_live(PlayerId(0)));
        assert_eq!(
            other_hands,
            (1..4)
                .map(|s| g.action_hand(PlayerId(s)).len())
                .collect::<Vec<_>>()
        );
        let state = g.state.clone();
        let trace = g.trace.clone();
        assert!(matches!(
            g.resume_work(work, Budget::unlimited()),
            QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
        ));
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
    });
}
#[test]
fn controller_counted_draw_commander_choice_consumes_one_attempt() {
    // Setup-provided tucked commander, not a paid tuck or legal-pod claim.
    for to_command in [false, true] {
        let mut p = setup("Concentrate", CONCENTRATE, "Island");
        let id = p.config.initial_libraries()[&PlayerId(0)][0];
        let oracle = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == "Caelorna, Coral Tyrant")
            .unwrap()
            .oracle_id
            .clone();
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .oracle_id = oracle;
        p.config.commanders = vec![id];
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            let before = library(g);
            announce(g, q, 0, vec![]);
            let r = loop {
                let r = request(g, q);
                if matches!(r, Request::CommanderReturn { .. }) {
                    break r;
                }
                answer(g, q, pass(&r));
            };
            recover(g);
            g.respond(
                r.player(),
                r.decision_id(),
                Response::CommanderReturn { to_command },
            )
            .unwrap();
            resolve(g, q, |_| {});
            assert_eq!(library(g), before[3..]);
            assert_eq!(
                g.action_hand(PlayerId(0)).len(),
                if to_command { 2 } else { 3 }
            );
            assert_eq!(
                g.current(g.object_id(id).unwrap()).unwrap().zone,
                if to_command {
                    Zone::Command { owner: PlayerId(0) }
                } else {
                    Zone::Hand { owner: PlayerId(0) }
                }
            );
        });
    }
}
#[test]
fn controller_counted_draw_short_library_finishes_attempts_before_sba() {
    let mut p = setup("Concentrate", CONCENTRATE, "Island");
    let keep = p.config.initial_libraries()[&PlayerId(0)][0];
    p.config
        .cards
        .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }) || c.id == keep);
    p.config.initial.libraries = None;
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        announce(g, q, 0, vec![]);
        let mut failed_prefix = false;
        let mut complete = false;
        for _ in 0..1000 {
            if let Frame::Resolve { frame } = &g.state.rules.frame {
                if frame
                    .draw_progress
                    .as_ref()
                    .is_some_and(|p| p.failed == 1.into())
                {
                    assert!(g.is_live(PlayerId(0)));
                    failed_prefix = true;
                }
                if matches!(frame.outcomes.first(),Some(InstructionOutcome::DrawAttempts{attempted,failed,..})if *attempted==3.into()&&*failed==2.into())
                {
                    assert!(g.is_live(PlayerId(0)));
                    recover(g);
                    complete = true;
                    break;
                }
            }
            step(g);
        }
        assert!(failed_prefix && complete);
        resolve(g, q, |_| {});
        assert!(!g.is_live(PlayerId(0)));
    });
}

const CRAVING: &str =
    include_str!("../../../../tests/fixtures/78725353-9274-420a-b722-add0f43c444e.json");
const AMBITION: &str =
    include_str!("../../../../tests/fixtures/84de4fec-2f38-4293-93d3-b3882c5aac14.json");
#[test]
fn controller_counted_draw_life_paid_sources_and_commander_finish_draws_before_loss() {
    for (name, raw, commander) in [
        ("Ancient Craving", CRAVING, false),
        ("Ambition's Cost", AMBITION, false),
        ("Ancient Craving", CRAVING, true),
    ] {
        let mut p = setup(name, raw, "Swamp");
        if commander {
            // Explicit setup-provided tucked commander, no paid-tuck/pod claim.
            let id = p.config.initial_libraries()[&PlayerId(0)][0];
            let oracle = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == "Caelorna, Coral Tyrant")
                .unwrap()
                .oracle_id
                .clone();
            p.config
                .cards
                .iter_mut()
                .find(|c| c.id == id)
                .unwrap()
                .oracle_id = oracle;
            p.config.commanders = vec![id];
            p.profile_digest = digest(&p.config).unwrap();
        }
        both(p, |g, q| {
            let before = library(g);
            announce(g, q, 0, vec![]);
            assert!(g.state.rules.stack.last().unwrap().paid.is_some());
            let mut completed = false;
            let mut replaced = false;
            for _ in 0..1000 {
                assert_eq!(g.state.rules.players[0].life, 40.into());
                if matches!(&g.state.rules.frame,Frame::Resolve{frame} if frame.cursor==1&&matches!(frame.outcomes.first(),Some(InstructionOutcome::DrawAttempts{attempted,failed,..})if *attempted==3.into()&&*failed==0.into()))
                {
                    completed = true;
                    break;
                }
                match g.advance(Budget::new(1)) {
                    Advance::Suspended { .. } => (),
                    Advance::AwaitingDecision(r) => {
                        let response = if matches!(r, Request::CommanderReturn { .. }) {
                            assert!(commander && !replaced);
                            recover(g);
                            replaced = true;
                            Response::CommanderReturn { to_command: true }
                        } else {
                            pass(&r)
                        };
                        g.respond(r.player(), r.decision_id(), response).unwrap();
                    }
                    other => panic!("draw/life prefix {other:?}"),
                }
            }
            assert!(completed);
            assert_eq!(replaced, commander);
            assert_eq!(library(g), before[3..]);
            assert_eq!(
                g.action_hand(PlayerId(0)).len(),
                if commander { 2 } else { 3 }
            );
            recover(g);
            resolve(g, q, |_| {});
            assert_eq!(g.state.rules.players[0].life, 37.into());
            if q == 1 {
                let losses=batches(g).into_iter().flat_map(|b|b.occurrences.iter()).filter(|o|matches!(&o.actual,EventOutcome::LifeLost{player:PlayerId(0),amount,previous,current}if *amount==3.into()&&*previous==40.into()&&*current==37.into())).count();
                assert_eq!(losses, 1);
            }
        });
    }
}
#[test]
fn controller_counted_draw_life_departure_and_failed_draws_preserve_tail_order() {
    for depart in [false, true] {
        let mut p = setup("Ancient Craving", CRAVING, "Swamp");
        if !depart {
            p.config.initial.life[0] = 3.into();
            let keep = p.config.initial_libraries()[&PlayerId(0)][0];
            p.config
                .cards
                .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }) || c.id == keep);
            p.config.initial.libraries = None;
            p.profile_digest = digest(&p.config).unwrap();
        }
        both(p, |g, q| {
            announce(g, q, 0, vec![]);
            let mut completed = false;
            for _ in 0..1000 {
                if matches!(&g.state.rules.frame,Frame::Resolve{frame}if frame.cursor==1) {
                    completed = true;
                    break;
                }
                step(g);
            }
            assert!(completed);
            assert!(g.is_live(PlayerId(0)));
            let Frame::Resolve { frame } = &g.state.rules.frame else {
                panic!("draw completed")
            };
            assert!(
                matches!(frame.outcomes.first(),Some(InstructionOutcome::DrawAttempts{attempted,failed,..})if *attempted==3.into()&&*failed==if depart{0.into()}else{2.into()})
            );
            assert_eq!(
                g.state.rules.players[0].life,
                if depart { 40.into() } else { 3.into() }
            );
            recover(g);
            if depart {
                g.concede(PlayerId(0)).unwrap();
                recover(g);
            } else {
                step(g);
                assert!(g.is_live(PlayerId(0)));
                assert_eq!(g.state.rules.players[0].life, 0.into());
                recover(g);
            }
            resolve(g, q, |_| {});
            assert!(!g.is_live(PlayerId(0)));
            for seat in 1..4 {
                assert_eq!(g.state.rules.players[seat].life, 40.into());
                assert!(g.action_hand(PlayerId(seat as u8)).is_empty());
            }
            if depart && q == 1 {
                assert!(
                    !batches(g)
                        .into_iter()
                        .flat_map(|b| &b.occurrences)
                        .any(|o| matches!(o.actual, EventOutcome::LifeLost { .. }))
                );
            }
        });
    }
}

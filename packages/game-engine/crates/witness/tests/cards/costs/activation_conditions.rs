//! "Activate only if" conditions from frozen R5 (CR 602.1b, 602.5, 605.3a,
//! 506.1, 102.1). One measurement decides whether an activation is offered
//! and whether it may begin; the counts it read stay with the activation
//! and a snapshot whose counts fail the condition is rejected.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_core::ManaAbilityIdentity;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const TEMPLE: &str = record!("cfdd5dc6-593e-495a-8cfe-3a56b3c4c7df");
const SPIRE: &str = record!("55a519b4-61cb-448a-875b-4d6dbe00580f");
const VERGE: &str = record!("d71bda4c-3dee-4398-8fd0-f77d8743b887");
const MOX_OPAL: &str = record!("de2440de-e948-4811-903c-0bbe376ff64d");
const STADIUM: &str = record!("4034bec6-e3c7-4d3f-81df-7c903977a606");
const TOME: &str = record!("65589538-d3c1-4dfd-a328-71f57f556212");
const CERTARCH: &str = record!("337533fb-2783-493c-856f-c41a83cca443");
const TRAINEE: &str = record!("1d7d340d-fe6f-4314-97ba-e0bf36a4ef6c");
const GHOST_TOWN: &str = record!("f2c861d3-b302-4e84-b647-099551007269");

const CONDITIONED: &[Cell] = &[Cell::ActivationRestriction];

fn mana(object: ObjectId) -> Action<Rules> {
    Action::ActivateMana { object }
}

/// The top stack entry's recorded activation-time counts.
fn evidence(g: &Game) -> Option<Vec<u32>> {
    match &g.state.rules.stack.last()?.kind {
        StackKind::Activated { condition, .. } => condition.as_ref().map(|c| c.counts.clone()),
        _ => None,
    }
}

/// A fork whose top activation carries `counts` instead.
fn forge(g: &Game, counts: Option<Vec<u32>>) -> Game {
    let mut f = g.fork();
    let StackKind::Activated { condition, .. } = &mut f.state.rules.stack.last_mut().unwrap().kind
    else {
        panic!("expected an activated stack entry")
    };
    *condition = counts.map(|counts| stack_costs::ActivationConditionCheck { counts });
    f
}

/// Temple of the False God: "{T}: Add {C}{C}. Activate only if you control
/// five or more lands." Fool's Tome: "{2}, {T}: Draw a card. Activate only
/// if you have no cards in hand." With five lands the Temple pays the Tome,
/// which draws; with four it is offered neither at priority nor in the
/// Tome's mana window.
#[test]
fn temple_of_the_false_god_pays_only_with_five_lands() {
    for lands in [4u32, 5] {
        let mut b = board(&[(TEMPLE, CONDITIONED), (TOME, CONDITIONED)])
            .permanent_or_hand("Temple of the False God", 0, 0, false)
            .permanent_or_hand("Fool's Tome", 0, 0, false);
        for _ in 1..lands {
            b = b.permanent_or_hand("Forest", 0, 0, false);
        }
        let mut g = Game::with_trace(b.libraries("Forest", 3).prepare(), TraceMode::Detailed);
        let temple = Driver::object(&g, 0);
        let tome = Driver::object(&g, 1);
        assert_eq!(actions(&mut g).contains(&mana(temple)), lands == 5);
        assert!(actions(&mut g).contains(&activate(tome, 0)));
        Driver::action(&mut g, activate(tome, 0));
        let window = Driver::request(&mut g, u64::MAX);
        let offered = window
            .index(&Candidate::Action {
                action: mana(temple),
            })
            .is_some();
        assert_eq!(offered, lands == 5);
        if lands == 4 {
            Driver::pay(&mut g, &[2, 3]);
        } else {
            Driver::pay(&mut g, &[0]);
            assert!(g.current(temple).unwrap().tapped);
        }
        // CR 602.5: the Tome read an empty hand as its activation began.
        assert_eq!(evidence(&g), Some(vec![0]));
        checked(&g);
        settle(&mut g);
        assert_eq!(hand_size(&g, 0), 1);
        // A card in hand: the Tome is no longer offered.
        assert!(!actions(&mut g).contains(&activate(tome, 0)));
        assert!(g.validate_activation(PlayerId(0), tome, 0).is_err());
    }
}

/// A snapshot whose activation recorded counts that fail its condition, or
/// that lost or gained evidence, is rejected (CR 602.5).
#[test]
fn forged_activation_condition_evidence_is_rejected() {
    let mut b = board(&[(STADIUM, CONDITIONED), (TOME, CONDITIONED)])
        .permanent_or_hand("Nomad Stadium", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Sol Ring", 0, 0, false);
    for _ in 0..7 {
        b = b.card("Forest", 0, 0, Zone::Graveyard { owner: PlayerId(0) });
    }
    let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
    let stadium = Driver::object(&g, 0);
    let before = life(&g, 0);
    Driver::action(&mut g, activate(stadium, 0));
    Driver::pay(&mut g, &[1]);
    assert_eq!(evidence(&g), Some(vec![7]));
    checked(&g);
    // Missing, failing, malformed and impossible counts.
    for counts in [None, Some(vec![6]), Some(vec![7, 7]), Some(vec![u32::MAX])] {
        let f = forge(&g, counts.clone());
        assert!(f.validate_state().is_err(), "{counts:?}");
    }
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&before, 4));
    // Sacrificed as a cost, the source left; the paid ability still resolved.
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));

    // Six cards in the graveyard: not offered, and never begun.
    let mut b = board(&[(STADIUM, CONDITIONED)])
        .permanent_or_hand("Nomad Stadium", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false);
    for _ in 0..6 {
        b = b.card("Forest", 0, 0, Zone::Graveyard { owner: PlayerId(0) });
    }
    let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
    let stadium = Driver::object(&g, 0);
    assert!(!actions(&mut g).contains(&activate(stadium, 0)));
    assert!(g.validate_activation(PlayerId(0), stadium, 0).is_err());
}

/// Spire of Industry: "{T}, Pay 1 life: Add one mana of any color. Activate
/// only if you control an artifact." Gloomlake Verge: "{T}: Add {B}.
/// Activate only if you control an Island or a Swamp." Each printed choice
/// is offered exactly when its condition holds; the Spire's colored mana is
/// then played and spent.
#[test]
fn printed_mana_choices_follow_their_conditions() {
    let printed = |index| Candidate::ManaAbility {
        ability: ManaAbilityIdentity::Printed { index },
    };
    for with in [false, true] {
        let mut b = board(&[
            (SPIRE, CONDITIONED),
            (VERGE, CONDITIONED),
            (TOME, CONDITIONED),
        ])
        .permanent_or_hand("Spire of Industry", 0, 0, false)
        .permanent_or_hand("Gloomlake Verge", 0, 0, false);
        if with {
            // Fool's Tome is the artifact; the Swamp satisfies the Verge.
            b = b
                .permanent_or_hand("Fool's Tome", 0, 0, false)
                .permanent_or_hand("Swamp", 0, 0, false);
        }
        let mut g = Game::with_trace(b.libraries("Forest", 2).prepare(), TraceMode::Detailed);
        let [spire, verge] = [0, 1].map(|i| Driver::object(&g, i));
        for source in [spire, verge] {
            assert!(actions(&mut g).contains(&mana(source)));
            let mut f = g.fork();
            let answer = f.next_test_decision().activate_mana(source);
            Driver::answer(&mut f, u64::MAX, answer);
            let d = Driver::request(&mut f, u64::MAX);
            let expected: Vec<_> = if with {
                vec![printed(0), printed(1)]
            } else {
                vec![printed(0)]
            };
            assert_eq!(d.candidates, expected, "{d:?}");
        }
        if !with {
            continue;
        }
        // Spend the Spire's chosen color and the Verge's black on the Tome.
        let tome = Driver::object(&g, 2);
        let before = life(&g, 0);
        Driver::action(&mut g, activate(tome, 0));
        for (source, index, color) in [(spire, 1, Color::G), (verge, 1, Color::B)] {
            let d = Driver::request(&mut g, u64::MAX);
            let slot = d
                .index(&Candidate::Action {
                    action: mana(source),
                })
                .unwrap();
            Driver::answer(
                &mut g,
                u64::MAX,
                Answer::Pay {
                    pay: PayAnswer::Activate { source: slot },
                },
            );
            let d = Driver::request(&mut g, u64::MAX);
            g.respond(d.player, d.id, d.pick_one(&printed(index)).unwrap())
                .unwrap();
            if color == Color::G {
                let d = Driver::request(&mut g, u64::MAX);
                g.respond(
                    d.player,
                    d.id,
                    d.pick_one(&Candidate::Color { color }).unwrap(),
                )
                .unwrap();
            }
        }
        // Back in the Tome's mana window.
        assert!(matches!(
            Driver::request(&mut g, u64::MAX).reason,
            Reason::Mana { .. }
        ));
        // The Spire's receipt kept the one artifact it saw (CR 602.5); a
        // receipt claiming none is rejected.
        let evidence = |g: &Game| match &g.state.rules.frame {
            Frame::Mana { cast } | Frame::Payment { cast } => cast
                .mana_activations
                .iter()
                .filter_map(|r| r.permanent.as_ref()?.condition.as_ref())
                .map(|e| (e.index, e.check.counts.clone()))
                .collect::<Vec<_>>(),
            _ => panic!("expected the Tome's mana window"),
        };
        // Spire: one artifact (the Tome). Verge: one Island or Swamp.
        assert_eq!(evidence(&g), [(1, vec![1]), (1, vec![1])]);
        let mut forged = g.fork();
        if let Frame::Mana { cast } | Frame::Payment { cast } = &mut forged.state.rules.frame {
            for r in &mut cast.mana_activations {
                if let Some(e) = r.permanent.as_mut().and_then(|p| p.condition.as_mut()) {
                    e.check.counts = vec![0];
                }
            }
        }
        assert!(forged.validate_state().is_err());
        checked(&g);
        // Finish the window and spend the pool.
        Driver::pay(&mut g, &[]);
        assert_eq!(life(&g, 0), plus(&before, -1));
        assert!(g.current(spire).unwrap().tapped && g.current(verge).unwrap().tapped);
        checked(&g);
        settle(&mut g);
        assert_eq!(hand_size(&g, 0), 1);
    }
}

/// Mox Opal: "Metalcraft — {T}: Add one mana of any color. Activate only if
/// you control three or more artifacts." It counts itself.
#[test]
fn mox_opal_counts_itself_among_three_artifacts() {
    for others in [1usize, 2] {
        let mut b = board(&[(MOX_OPAL, CONDITIONED)]).permanent_or_hand("Mox Opal", 0, 0, false);
        for _ in 0..others {
            b = b.permanent_or_hand("Darksteel Relic", 0, 0, false);
        }
        let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let mox = Driver::object(&g, 0);
        assert_eq!(actions(&mut g).contains(&mana(mox)), others == 2);
    }
}

/// Vedalken Certarch: "Metalcraft — {T}: Tap target artifact, creature, or
/// land. Activate only if you control three or more artifacts." Greenhilt
/// Trainee: "{T}: Target creature gets +4/+4 until end of turn. Activate
/// only if this creature's power is 4 or greater." Each reads the current
/// board, including a resolved pump on the source itself (CR 208.1, 613).
#[test]
fn stack_abilities_read_the_current_board() {
    let prepared = board(&[(CERTARCH, CONDITIONED), (TRAINEE, CONDITIONED)])
        .permanent_or_hand("Vedalken Certarch", 0, 0, false)
        .permanent_or_hand("Greenhilt Trainee", 0, 0, false)
        .permanent_or_hand("Sol Ring", 0, 0, false)
        .permanent_or_hand("Darksteel Relic", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Giant Growth", 0, 0, true)
        .permanent_or_hand("Darksteel Relic", 0, 0, true)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let [certarch, trainee] = [0, 1].map(|i| Driver::object(&g, i));
    assert!(!actions(&mut g).contains(&activate(certarch, 0)));
    assert!(!actions(&mut g).contains(&activate(trainee, 0)));
    // A third artifact enters: metalcraft now holds.
    let relic = Driver::object(&g, 6);
    Driver::cast(&mut g, relic.card);
    Driver::pay(&mut g, &[]);
    settle(&mut g);
    assert!(actions(&mut g).contains(&activate(certarch, 0)));
    // Giant Growth makes the Trainee 5/6.
    let growth = Driver::object(&g, 5);
    Driver::cast(&mut g, growth.card);
    let d = Driver::request(&mut g, u64::MAX);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Object { object: trainee }).unwrap(),
    )
    .unwrap();
    Driver::pay(&mut g, &[4]);
    settle(&mut g);
    assert!(actions(&mut g).contains(&activate(trainee, 0)));
    checked(&g);
}

/// Ghost Town: "{0}: Return this land to its owner's hand. Activate only if
/// it's not your turn." (CR 102.1)
#[test]
fn ghost_town_returns_only_on_another_players_turn() {
    let prepared = board(&[(GHOST_TOWN, CONDITIONED)])
        .permanent_or_hand("Ghost Town", 0, 0, false)
        .permanent_or_hand("Ghost Town", 1, 1, false)
        .prepare();
    let g = Game::with_trace(prepared, TraceMode::Detailed);
    let ok = |card, seat| {
        g.validate_activation(PlayerId(seat), Driver::object(&g, card), 0)
            .is_ok()
    };
    assert!(!ok(0, 0));
    assert!(ok(1, 1));
}

const UNDERHANDED: &str = record!("870185fc-7b05-47d3-95ff-97fbf12796d4");

/// A fork whose current frame has every recorded condition check replaced
/// by `counts`; returns it with the number of checks replaced.
fn forge_frame(g: &Game, counts: &[u32]) -> (Game, usize) {
    fn walk(v: &mut serde_json::Value, with: &serde_json::Value) -> usize {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(check) = m.get_mut("check") {
                    *check = with.clone();
                    return 1;
                }
                m.values_mut().map(|x| walk(x, with)).sum()
            }
            serde_json::Value::Array(a) => a.iter_mut().map(|x| walk(x, with)).sum(),
            _ => 0,
        }
    }
    let mut frame = serde_json::to_value(&g.state.rules.frame).unwrap();
    let n = walk(&mut frame, &serde_json::json!({ "counts": counts }));
    let mut f = g.fork();
    f.state.rules.frame = serde_json::from_value(frame).unwrap();
    (f, n)
}

/// Every forged check in the current frame is rejected: counts that fail
/// "you control an artifact", and the wrong number of counts.
fn rejects_forged_checks(g: &Game) {
    g.validate_state().unwrap();
    for counts in [&[0][..], &[1, 1, 1]] {
        let (f, n) = forge_frame(g, counts);
        assert_eq!(n, 1, "{counts:?}");
        assert!(f.validate_state().is_err(), "{counts:?}");
    }
}

/// Spire of Industry's colored ability, chosen and paid from the decision
/// `d` that offers it; `before_color` sees the suspended paid ability.
fn play_spire(g: &mut Game, spire: ObjectId, before_color: impl Fn(&Game)) {
    let d = Driver::request(g, u64::MAX);
    let slot = d
        .index(&Candidate::Action {
            action: mana(spire),
        })
        .unwrap();
    Driver::answer(
        g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Activate { source: slot },
        },
    );
    let d = Driver::request(g, u64::MAX);
    let printed = Candidate::ManaAbility {
        ability: ManaAbilityIdentity::Printed { index: 1 },
    };
    g.respond(d.player, d.id, d.pick_one(&printed).unwrap())
        .unwrap();
    let d = Driver::request(g, u64::MAX);
    assert!(matches!(d.reason, Reason::ManaColor { .. }), "{d:?}");
    // Paid but awaiting its color: the suspended ability's own receipt.
    before_color(g);
    let black = Candidate::Color { color: Color::B };
    g.respond(d.player, d.id, d.pick_one(&black).unwrap())
        .unwrap();
}

/// Underhanded Designs: "Whenever an artifact you control enters, you may pay
/// {1}." Spire's conditioned mana pays it during resolution; the payment's
/// held receipt and the suspended Spire activation both keep their evidence.
#[test]
fn resolution_payment_receipts_keep_condition_evidence() {
    let prepared = board(&[(UNDERHANDED, CONDITIONED), (SPIRE, CONDITIONED)])
        .permanent_or_hand("Underhanded Designs", 0, 0, false)
        .permanent_or_hand("Spire of Industry", 0, 0, false)
        .permanent_or_hand("Sol Ring", 0, 0, false)
        .permanent_or_hand("Darksteel Relic", 0, 0, true)
        .libraries("Forest", 3)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let spire = Driver::object(&g, 1);
    let relic = Driver::object(&g, 3);
    Driver::cast(&mut g, relic.card);
    Driver::pay(&mut g, &[]);
    loop {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::OptionalEffect {
            let accept = d.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap();
            g.respond(d.player, d.id, accept).unwrap();
            break;
        }
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    play_spire(&mut g, spire, rejects_forged_checks);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{d:?}");
    assert!(matches!(
        g.state.rules.frame,
        Frame::ResolutionPayment { .. }
    ));
    rejects_forged_checks(&g);
}

/// Izzet Signet: "{1}, {T}: Add {U}{R}." Spire's conditioned mana pays the
/// Signet's own mana cost; the Signet activation holds that receipt.
#[test]
fn nested_mana_ability_receipts_keep_condition_evidence() {
    let prepared = board(&[(SPIRE, CONDITIONED)])
        .permanent_or_hand("Izzet Signet", 0, 0, false)
        .permanent_or_hand("Spire of Industry", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let signet = Driver::object(&g, 0);
    let spire = Driver::object(&g, 1);
    let answer = g.next_test_decision().activate_mana(signet);
    Driver::answer(&mut g, u64::MAX, answer);
    play_spire(&mut g, spire, rejects_forged_checks);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(
        matches!(d.reason, Reason::Mana { source: Some(s) } if s == signet),
        "{d:?}"
    );
    assert!(matches!(g.state.rules.frame, Frame::ManaAbility { .. }));
    rejects_forged_checks(&g);
}

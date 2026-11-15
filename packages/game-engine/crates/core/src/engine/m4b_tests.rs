//! SOURCE-ONLY drafts: paid frozen source, private midgame roots, no admission.
//! CR 611.3a-c, 702.10, 302.6, 400.7; pinned CR20260925 URL in parent module.
use super::*;
const WANDERER: &str =
    include_str!("../../../../tests/fixtures/ramos/ad9b7fbc-61c8-43ee-a65c-99206fd1e4df.json");

fn haste(g: &Game, id: u32, expected: bool) {
    assert_eq!(g.has_keyword(object(g, id), KeywordAbility::Haste), expected);
}
fn wanderer(g: &mut Game, q: u64) {
    fund(g, q, "Forest", 6);
    fund(g, q, "Island", 1);
    fund(g, q, "Mountain", 1);
    answer(g, q, Response::Cast { card: CardId(0) });
    loop {
        let r = request(g, q);
        let response = match &r {
            Request::Mana { .. } => Response::FinishMana { undo_mana: false },
            Request::SpendMana { cost, available, .. } => Response::SpendMana {
                spending: spending(cost, available),
            },
            Request::OrderTriggers { candidates, .. } => Response::OrderTriggers { order: candidates.clone() },
            Request::Priority { .. } => break,
            x => panic!("paid Wanderer {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    assert!(g.state.rules.stack.iter().find(|s| s.source.card == CardId(0)).unwrap().paid.is_some());
    resolve(g, q, |_| {});
    assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Battlefield);
}
#[test]
fn m4b_paid_live_haste_control_departure_reentry_and_recovery() {
    let mut rows = vec![
        ("Maelstrom Wanderer", 0, 0, true),
        ("Grizzly Bears", 0, 0, true),
        ("Llanowar Elves", 0, 0, true),
        ("Control Magic", 1, 1, true),
        ("Grizzly Bears", 1, 1, false),
        ("Unsummon", 0, 0, true),
        ("Disperse", 0, 0, true),
        ("Unsummon", 0, 0, true),
        ("Control Magic", 0, 0, true),
        ("Disperse", 0, 0, true),
        ("Merfolk Looter", 0, 0, true),
    ];
    lands(&mut rows, "Forest", 0, 12);
    lands(&mut rows, "Island", 0, 12);
    lands(&mut rows, "Mountain", 0, 4);
    lands(&mut rows, "Island", 1, 4);
    let mut p = prepared_with_sources(&rows, &[WANDERER]);
    // Keep one land in the seedless library: each no-hit cascade bottoms one
    // object and needs no random permutation. Surplus basics start in the
    // graveyard so cleanup cannot discard the later return spells. Preserve
    // every physical card in this private midgame root.
    let mut kept = false;
    for c in &mut p.config.cards {
        if c.owner == PlayerId(0) && matches!(c.zone, Zone::Library { .. }) {
            if kept { c.zone = Zone::Graveyard { owner: c.owner }; }
            kept = true;
        }
    }
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        cast(g, q, 1, vec![]);
        cast(g, q, 2, vec![]);
        fund(g, q, "Island", 1);
        cast(g, q, 10, vec![]);
        let bear = object(g, 1);
        let elf = object(g, 2);
        assert!(!g.current(bear).unwrap().attack_ready);
        assert!(!g.creatures(PlayerId(0), true).contains(&bear));
        assert!(g.validate_permanent_mana_activation(PlayerId(0), elf).is_err());
        let r = request(g, q);
        let before = g.state.clone();
        assert!(g.respond(r.player(), r.decision_id(), Response::ActivateAbility {
            object: object(g, 10), ability: 0,
        }).is_err());
        assert_eq!(g.state, before);
        wanderer(g, q);
        for id in [0, 1, 2, 10] { haste(g, id, true); }
        haste(g, 4, false);
        assert!(g.creatures(PlayerId(0), true).contains(&bear));
        assert!(g.creatures(PlayerId(0), true).contains(&object(g, 0)));
        assert!(g.validate_permanent_mana_activation(PlayerId(0), elf).is_ok());
        // Accepted tap payment and a real attack declaration on independent forks.
        let mut tapped = g.fork();
        answer(&mut tapped, q, Response::ActivateMana { object: elf });
        request(&mut tapped, q);
        assert!(tapped.current(elf).unwrap().tapped);
        recover(&tapped);
        let mut looter = g.fork();
        answer(&mut looter, q, Response::ActivateAbility { object: object(g, 10), ability: 0 });
        request(&mut looter, q);
        assert!(looter.current(object(g, 10)).unwrap().tapped);
        recover(&looter);
        let mut attack = g.fork();
        let mut declared = false;
        for _ in 0..100 {
            let r = request(&mut attack, q);
            if let Request::Attackers { candidates, .. } = &r {
                assert!(candidates.contains(&bear));
                answer(&mut attack, q, Response::Attackers { attacks: vec![(bear, PlayerId(1))] });
                declared = true;
                break;
            }
            answer(&mut attack, q, pass(&r));
        }
        assert!(declared);
        request(&mut attack, q);
        recover(&attack);
        let mut absent = g.fork();
        cast(&mut absent, q, 7, target(g, 0));
        assert!(!absent.creatures(PlayerId(0), true).contains(&bear));
        assert!(absent.validate_permanent_mana_activation(PlayerId(0), elf).is_err());
        let r = request(&mut absent, q);
        let before = absent.state.clone();
        assert!(absent.respond(r.player(), r.decision_id(), Response::ActivateAbility {
            object: object(g, 10), ability: 0,
        }).is_err());
        assert_eq!(absent.state, before);
        recover(&absent);
        recover(g);
        // New entrants join a live set; successor recipients are not captured IDs.
        cast(g, q, 5, target(g, 1));
        haste(g, 1, false);
        cast(g, q, 1, vec![]);
        assert_ne!(object(g, 1), bear);
        haste(g, 1, true);
        // A recipient's actual control change joins/leaves the same live set.
        cast(g, q, 8, target(g, 4));
        control(g, 4, 0, 1, 1);
        haste(g, 4, true);
        assert!(!g.current(object(g, 4)).unwrap().attack_ready);
        assert!(g.creatures(PlayerId(0), true).contains(&object(g, 4)));
        recover(g);
        cast(g, q, 9, target(g, 8));
        control(g, 4, 1, 1, 1);
        haste(g, 4, false);
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        cast(g, q, 3, target(g, 0));
        control(g, 0, 1, 0, 0);
        for id in [0, 4] { haste(g, id, true); }
        for id in [1, 2] { haste(g, id, false); }
        assert!(!g.current(object(g, 0)).unwrap().attack_ready);
        assert!(g.creatures(PlayerId(1), true).contains(&object(g, 0)));
        visible(g, 3, 0, 1);
        recover(g);
        // Remove the authentic Aura at opponent priority; no fake control producer.
        until_player(g, q, PlayerId(0));
        cast(g, q, 6, target(g, 3));
        control(g, 0, 0, 0, 0);
        haste(g, 4, false);
        for id in [0, 1, 2, 10] { haste(g, id, true); }
        assert!(!g.current(object(g, 0)).unwrap().attack_ready);
        recover(g);
        until_player(g, q, PlayerId(0));
        let source = object(g, 0);
        cast(g, q, 7, target(g, 0));
        for id in [0, 1, 2, 4, 10] { haste(g, id, false); }
        recover(g);
        let turn = g.turn().number;
        own_main(g, q, 0, turn);
        wanderer(g, q);
        assert_ne!(object(g, 0), source);
        for id in [0, 1, 2, 10] { haste(g, id, true); }
        haste(g, 4, false);
        assert!(g.state.rules.effects.is_empty(), "static grants create no temporary effects");
        // A forged normalized control cache cannot manufacture a different live set.
        let mut bad = g.fork();
        bad.state.rules.objects.get_mut(&CardId(0)).unwrap().controller = PlayerId(1);
        assert!(Game::restore(bad.prepared.as_ref().clone(), &bad.snapshot().unwrap()).is_err());
        // Synthetic hidden-order/privacy probe, never a source/control producer.
        let mut hidden = g.fork();
        hidden.state.rules.libraries.get_mut(&PlayerId(2)).unwrap().swap(0, 1);
        assert!(Game::restore(hidden.prepared.as_ref().clone(), &hidden.snapshot().unwrap()).is_err());
        for seat in 0..4 {
            let before = g.state.clone();
            let QueryResult::Complete(QueryValue::Observation(a)) = g.observe(PlayerId(seat), Budget::unlimited()) else { panic!() };
            let QueryResult::Complete(QueryValue::Observation(b)) = hidden.observe(PlayerId(seat), Budget::unlimited()) else { panic!() };
            assert_eq!(a, b);
            assert_eq!(before, g.state);
        }
    });
}

fn spending(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> BTreeMap<Color, Quantity> {
    let mut pool = available.clone();
    let mut out = BTreeMap::<Color, Quantity>::new();
    for c in &cost.colored {
        pool.get_mut(c).unwrap().subtract(&1.into());
        out.entry(*c).or_default().add(&1.into());
    }
    let mut generic = cost.generic.clone();
    for (c, n) in pool {
        let used = n.min(generic.clone());
        generic.subtract(&used);
        if used > Quantity::zero() {
            out.entry(c).or_default().add(&used);
        }
    }
    assert_eq!(generic, 0.into());
    out
}

//! SOURCE-ONLY draft: complete paid Wanderer plus Ramos, no legal-pod claim.
use super::*;
const WANDERER: &str =
    include_str!("../../../../../tests/fixtures/ramos/ad9b7fbc-61c8-43ee-a65c-99206fd1e4df.json");
#[test]
fn m4b_paid_wanderer_two_identified_siblings_one_ramos_event_and_replay() {
    let mut p = prepared_with_records("Maelstrom Wanderer", &[], &[WANDERER]);
    let oracle = |name: &str| {
        p.catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let forest = oracle("Forest");
    let replacements = [
        "Forest", "Forest", "Island", "Island", "Mountain", "Mountain",
    ]
    .map(oracle);
    for (c, id) in p
        .config
        .cards
        .iter_mut()
        .filter(|c| {
            c.owner == PlayerId(0) && c.oracle_id == forest && matches!(c.zone, Zone::Hand { .. })
        })
        .take(6)
        .zip(replacements)
    {
        c.zone = Zone::Battlefield;
        c.oracle_id = id;
    }
    assert_eq!(p.config.cards.len(), 400);
    // Ramos supplies the appropriate five-color commander identity for this root.
    p.profile_digest = digest(&p.config).unwrap();
    for mode in [TraceMode::Detailed, TraceMode::Off] {
        let mut g = Game::with_trace(p.clone(), mode);
        let ramos = paid(&mut g, "Ramos, Dragon Engine");
        for _ in 0..100 {
            if !g.state.rules.stack.iter().any(|s| s.id == ramos) {
                break;
            }
            answer(&mut g, Response::Pass);
            request(&mut g);
        }
        let original = paid(&mut g, "Maelstrom Wanderer");
        let siblings = g
            .state
            .rules
            .stack
            .iter()
            .filter_map(|s| match &s.kind {
                StackKind::Triggered {
                    ability,
                    witness: TriggerEventWitness::SpellCast { cast },
                    ..
                } if cast.stack_id == original
                    && s.source
                        == g.state
                            .rules
                            .stack
                            .iter()
                            .find(|s| s.id == original)
                            .unwrap()
                            .source =>
                {
                    Some((s.id, *ability))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(siblings.len(), 2);
        assert_ne!(siblings[0].0, siblings[1].0);
        assert_eq!(
            siblings.iter().map(|s| s.1).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                EffectiveTriggerAbility::Intrinsic { index: 0 },
                EffectiveTriggerAbility::Intrinsic { index: 1 },
            ])
        );
        let observers = g
            .state
            .rules
            .stack
            .iter()
            .filter(|s| {
                matches!(&s.kind,
            StackKind::Triggered { witness: TriggerEventWitness::SpellCast { cast }, .. }
            if cast.stack_id == original)
                    && g.definition(s.source.card).unwrap().name == "Ramos, Dragon Engine"
            })
            .count();
        assert_eq!(observers, 1);
        prove(&g);
        let mut one_remaining = false;
        for _ in 0..100 {
            if !g.state.rules.stack.iter().any(|s| s.id == original) {
                break;
            }
            answer(&mut g, Response::Pass);
            request(&mut g);
            let remaining = siblings
                .iter()
                .filter(|(id, _)| g.state.rules.stack.iter().any(|s| s.id == *id))
                .count();
            if remaining == 1 {
                one_remaining = true;
                assert!(g.state.rules.stack.iter().any(|s| s.id == original));
                prove(&g);
            }
        }
        assert!(
            one_remaining,
            "siblings complete separately above the original"
        );
        assert!(g.state.rules.stack.is_empty());
        let ramos = g
            .state
            .rules
            .objects
            .values()
            .find(|o| g.definition(o.id.card).unwrap().name == "Ramos, Dragon Engine")
            .unwrap();
        assert_eq!(ramos.plus_one_counters, 3.into());
        assert!(g.has_keyword(ramos.id, KeywordAbility::Haste));
        prove(&g);
    }
}

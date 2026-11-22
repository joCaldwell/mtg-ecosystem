use super::*;
use mtg_witness::aura::prepared_with_sources;
use mtg_witness::equipped::{answer, both, object, pay_to_priority, recover, request};

const PATH: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/d683d985-9888-4d21-8b5f-69e69ce4a03b.json");
const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");

fn prepared() -> Prepared {
    let mut p = prepared_with_sources(
        &[
            ("Grizzly Bears", 0, 0, true),
            ("Plains", 0, 0, false),
            ("Grizzly Bears", 2, 1, false),
        ],
        &[],
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(HEADER, PATH)
    else {
        panic!("exact R4 source");
    };
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    // Inject the precombat battlefield for this private witness, then use the
    // real seeded setup and library machinery for every accepted action.
    let mut match_config =
        MatchConfig::for_profile("commander-staples-v1", [36; 32], PlayerId(0)).unwrap();
    for (seat, deck) in match_config.decks.iter_mut().enumerate() {
        deck.commander.clear();
        let mut counts = BTreeMap::<String, u32>::new();
        for row in p
            .config
            .cards
            .iter()
            .filter(|c| usize::from(c.owner.0) == seat)
        {
            *counts.entry(row.oracle_id.clone()).or_default() += 1;
        }
        deck.cards = counts
            .into_iter()
            .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
            .collect();
    }
    p.match_config = Some(match_config);
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
    p
}

#[test]
fn slice6a_paid_optional_search_keeps_lki_chooser_and_exact_shuffle() {
    for (perform, find) in [(false, false), (true, false), (true, true)] {
        both(prepared(), |g, q| {
            for _ in 0..4 {
                assert!(matches!(request(g, q), Request::Mulligan { .. }));
                answer(g, q, Response::Mulligan { take: false });
            }
            let caster = PlayerId(0);
            let chooser = PlayerId(1);
            let target = object(g, 2);
            assert!(matches!(request(g, q), Request::Priority { player, .. } if player == caster));
            let library = g.state.rules.libraries[&chooser].clone();
            let land = object(g, library[0].0);
            answer(g, q, Response::Cast { card: CardId(0) });
            pay_to_priority(g, q, vec![TargetEntity::Object { object: target }]);
            let choice = loop {
                let r = request(g, q);
                if !matches!(r, Request::Priority { .. }) {
                    break r;
                }
                answer(g, q, Response::Pass);
            };
            assert!(matches!(choice, Request::OptionalEffect { player, .. } if player == chooser));
            assert_eq!(g.current(object(g, 2)).unwrap().zone, Zone::Exile);
            let before = g.state.clone();
            assert!(
                g.respond(
                    caster,
                    choice.decision_id(),
                    Response::OptionalEffect { perform }
                )
                .is_err()
            );
            assert_eq!(before, g.state);
            recover(g);
            answer(g, q, Response::OptionalEffect { perform });

            if perform {
                let search = request(g, q);
                assert!(
                    matches!(&search, Request::SearchLands { player, candidates, maximum, .. }
                    if *player == chooser && *maximum == 1 && candidates.contains(&land))
                );
                let QueryResult::Complete(QueryValue::Observation(obs)) =
                    g.observe(caster, Budget::new(4096))
                else {
                    panic!("public observation");
                };
                assert!(obs.decision.is_none());
                recover(g);
                answer(
                    g,
                    q,
                    Response::SearchLands {
                        objects: if find { vec![land] } else { vec![] },
                    },
                );
            }
            request(g, q);
            let result = g.current(object(g, land.card.0)).unwrap();
            if find {
                assert_eq!(result.zone, Zone::Battlefield);
                assert_eq!(result.controller, chooser);
                assert!(result.tapped);
                assert_eq!(g.state.rules.libraries[&chooser].len(), library.len() - 1);
            } else {
                assert_eq!(result.id, land);
                if perform {
                    assert_eq!(
                        g.state.rules.libraries[&chooser]
                            .iter()
                            .copied()
                            .collect::<BTreeSet<_>>(),
                        library.iter().copied().collect(),
                    );
                } else {
                    assert_eq!(g.state.rules.libraries[&chooser], library);
                }
            }
            if g.trace.mode == TraceMode::Detailed {
                let count = g.trace().events.iter().filter(|e| matches!(e, Event::Committed { proposed: ProposedEvent::FinishLibraryShuffle { player }, .. } if *player == chooser)).count();
                assert_eq!(count, usize::from(perform));
            }
            recover(g);
        });
    }
}

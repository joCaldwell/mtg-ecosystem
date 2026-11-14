//! Private complete-source casts, not pool/profile/consumer admission.
//! All new native tests are UNRUN here; the parent owns combined execution.
//! CR 109.5, 400.7, 608.2b/c, 611.2a/c, 702.11b, 702.12, 514.2:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! No same-incarnation control-change acceptance: this base has no control-
//! changing Instruction/StaticEffect or supported Control Magic source fixture.
use super::equipped_indestructible_tests::{answer, both, cast, object, recover, request};
use super::temporary_indestructible_tests::{
    announce, pass, target, through_cleanup, until_player,
};
use super::*;

struct Witness {
    name: &'static str,
    raw: &'static str,
    land: &'static str,
    cost: usize,
    modifier: Option<(i64, i64)>,
    keywords: &'static [KeywordAbility],
}
const SOURCES: &[Witness] = &[
    Witness {
        name: "Masterful Flourish",
        raw: include_str!("../../../../tests/fixtures/1cb419cc-d8e4-4676-987f-089cebccc374.json"),
        land: "Swamp",
        cost: 1,
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::Indestructible],
    },
    Witness {
        name: "Dive Down",
        raw: include_str!("../../../../tests/fixtures/2f990b54-fbf3-4949-85bb-9ba39710e72a.json"),
        land: "Island",
        cost: 1,
        modifier: Some((0, 3)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        name: "Blossoming Defense",
        raw: include_str!("../../../../tests/fixtures/5a851367-1c4a-4cc9-a9c3-3d2775986b4c.json"),
        land: "Forest",
        cost: 1,
        modifier: Some((2, 2)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        name: "Ranger's Guile",
        raw: include_str!("../../../../tests/fixtures/694cb93a-bcc5-44b0-a76c-19ae4a4e5e0f.json"),
        land: "Forest",
        cost: 1,
        modifier: Some((1, 1)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        name: "Alesha's Legacy",
        raw: include_str!("../../../../tests/fixtures/c4229cc8-11e5-4221-b00d-86969701e21b.json"),
        land: "Swamp",
        cost: 2,
        modifier: None,
        keywords: &[KeywordAbility::Deathtouch, KeywordAbility::Indestructible],
    },
];

// These private ScenarioConfig owner/controller differences are initial setup,
// never a claim that gameplay changed a permanent's controller.
fn prepared(rows: &[(&str, u8, u8, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(name, _, _, hand)| {
            (
                if SOURCES.iter().any(|s| s.name == *name) {
                    "Giant Growth"
                } else {
                    *name
                },
                *hand,
            )
        })
        .collect();
    let mut p = equipped_indestructible_tests::prepared(&placeholders, false);
    let mut c = (*p.catalog).clone();
    for witness in SOURCES {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            witness.raw,
        ) else {
            panic!("complete frozen source {}", witness.name)
        };
        assert_eq!(definition.name, witness.name);
        assert_eq!(
            definition.spell.as_ref().unwrap().targets[0].domain,
            TargetDomain::ControlledBattlefieldCreature
        );
        for (i, (name, _, _, _)) in rows.iter().enumerate() {
            if *name == witness.name {
                p.config.cards[i].oracle_id = definition.oracle_id.clone();
            }
        }
        c.definitions.push(*definition);
    }
    for (i, (_, owner, controller, hand)) in rows.iter().enumerate() {
        let card = &mut p.config.cards[i];
        card.owner = PlayerId(*owner);
        card.controller = PlayerId(*controller);
        card.zone = if *hand {
            Zone::Hand { owner: card.owner }
        } else {
            Zone::Battlefield
        };
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn stats(g: &Game, id: u32, p: i64, t: i64) {
    let c = g.derived_characteristics(object(g, id)).unwrap();
    assert_eq!(c.power, Some(p.into()));
    assert_eq!(c.toughness, Some(t.into()));
}
fn assert_grant(g: &Game, witness: &Witness, id: u32) {
    let (p, t) = witness.modifier.unwrap_or((0, 0));
    stats(g, id, 2 + p, 2 + t);
    assert_eq!(
        g.state.rules.effects.len(),
        1 + usize::from(witness.modifier.is_some())
    );
    let grant = g.state.rules.effects.last().unwrap();
    assert!(
        matches!(&grant.kind, ResolvedEffectKind::GrantKeywords { target, keywords }
        if *target == object(g, id) && keywords == witness.keywords)
    );
    assert_eq!(grant.origin.source_block, 0);
    assert_eq!(grant.origin.cursor, u32::from(witness.modifier.is_some()));
    for keyword in witness.keywords {
        assert_eq!(
            g.current_keywords(object(g, id))
                .iter()
                .filter(|k| *k == keyword)
                .count(),
            1
        );
    }
}
fn resolve(g: &mut Game, q: u64) {
    while !g.state.rules.stack.is_empty() {
        answer(g, q, Response::Pass);
        request(g, q);
    }
}
fn finish_query(g: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..10000 {
        result = match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => g.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("query rejected {reason:?}"),
        };
    }
    panic!("bounded query");
}

#[test]
fn controlled_creature_protection_paid_five_sources_cleanup_and_normalized_forgery() {
    for witness in SOURCES {
        both(
            prepared(&[
                (witness.name, 0, 0, true),
                ("Grizzly Bears", 2, 0, false),
                (witness.land, 0, 0, false),
                (witness.land, 0, 0, false),
            ]),
            |g, q| {
                cast(g, q, 0, target(g, 1));
                assert_grant(g, witness, 1);
                assert_eq!(g.state.rules.cards[&CardId(1)].owner, PlayerId(2));
                assert_eq!(g.current(object(g, 1)).unwrap().controller, PlayerId(0));
                assert!(matches!(
                    g.current(object(g, 0)).unwrap().zone,
                    Zone::Graveyard { .. }
                ));
                assert_eq!(
                    g.state.rules.objects.values().filter(|o| o.tapped).count(),
                    witness.cost
                );
                recover(g); // Reachable positive restore/replay before normalized forgeries.
                let wire = g.snapshot().unwrap();
                for variant in 0..8 {
                    let mut bad: Snapshot = strict_read(&wire).unwrap();
                    let e = bad.state.rules.effects.last_mut().unwrap();
                    let ResolvedEffectKind::GrantKeywords { target, keywords } = &mut e.kind else {
                        unreachable!()
                    };
                    match variant {
                        0 => keywords.clear(),
                        1 => keywords.push(keywords[0]),
                        2 => *keywords = vec![KeywordAbility::Haste],
                        3 => {
                            *keywords = if witness.keywords == [KeywordAbility::Hexproof] {
                                vec![KeywordAbility::Indestructible]
                            } else {
                                vec![KeywordAbility::Hexproof]
                            }
                        }
                        4 => target.incarnation += 1,
                        5 => e.origin.source_block += 1,
                        6 => e.origin.cursor += 1,
                        _ => e.expires_in_cleanup_of_turn += 1,
                    }
                    bad.state_sha256 = digest(&bad.state).unwrap();
                    assert!(
                        Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap())
                            .is_err(),
                        "{} normalized forgery {variant}",
                        witness.name
                    );
                }
                through_cleanup(g, q);
                assert!(g.state.rules.effects.is_empty());
                stats(g, 1, 2, 2);
                assert!(g.current_keywords(object(g, 1)).is_empty());
                for controller in 0..4 {
                    assert!(g.targetable_by(object(g, 1), PlayerId(controller)));
                }
            },
        );
    }
}

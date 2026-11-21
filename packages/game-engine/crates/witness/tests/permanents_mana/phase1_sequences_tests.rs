//! Paid frozen-source witnesses using both one-step and unrestricted budgets.
use super::composed_spell_tests::cursor;
use super::*;
use mtg_witness::aura::prepared_with_sources;
use mtg_witness::equipped::{answer, both, cast, object, recover, request};
use mtg_witness::temporary::{announce, pass, target};

const BONES: &str =
    include_str!("../../../../tests/fixtures/5bf4d8d9-a2b2-4dba-ac05-9d4470a89db2.json");
const EXPLORE: &str =
    include_str!("../../../../tests/fixtures/b8f566ea-8283-4afa-9ac8-737e26419283.json");
const RAPID: &str =
    include_str!("../../../../tests/fixtures/06692cd9-ac2f-4a32-8fd1-043ba3c0fe71.json");
const WING: &str =
    include_str!("../../../../tests/fixtures/7fbf6afc-8a16-447b-bdef-b7d0b640f76c.json");
const MASTER: &str =
    include_str!("../../../../tests/fixtures/9e0c5919-aed6-4d98-85c8-2658be78bdf0.json");

fn library(g: &Game) -> Vec<CardId> {
    g.state.rules.libraries[&PlayerId(0)]
        .iter()
        .copied()
        .collect()
}

#[test]
fn phase1_sequences_private_scry_precedes_draw_and_life_in_replay() {
    both(
        prepared_with_sources(
            &[
                ("Read the Bones", 0, 0, true),
                ("Swamp", 0, 0, false),
                ("Swamp", 0, 0, false),
                ("Swamp", 0, 0, false),
            ],
            &[BONES],
        ),
        |g, q| {
            let original = library(g);
            announce(g, q, 0, vec![]);
            let (decision, looked) = loop {
                let r = request(g, q);
                if let Request::Scry { candidates, .. } = &r {
                    break (r.clone(), candidates.clone());
                }
                assert!(!g.state.rules.stack.is_empty());
                answer(g, q, pass(&r));
            };
            assert_eq!(looked, [object(g, original[0].0), object(g, original[1].0)]);
            assert_eq!(library(g), original);
            assert_eq!(g.state.rules.players[0].life, 40.into());
            for card in &looked {
                assert!(g.handle_for(PlayerId(0), *card).is_ok());
                assert!(g.handle_for(PlayerId(1), *card).is_err());
            }
            recover(g);
            g.respond(
                decision.player(),
                decision.decision_id(),
                Response::Scry {
                    top: vec![],
                    bottom: looked,
                },
            )
            .unwrap();
            recover(g);
            assert!(matches!(request(g, q), Request::Priority { .. }));
            let expected: Vec<_> = original[4..]
                .iter()
                .chain(&original[..2])
                .copied()
                .collect();
            assert_eq!(library(g), expected);
            for card in &original[2..4] {
                assert_eq!(
                    g.current(object(g, card.0)).unwrap().zone,
                    Zone::Hand { owner: PlayerId(0) }
                );
            }
            assert_eq!(g.state.rules.players[0].life, 38.into());
        },
    );
}

#[test]
fn phase1_sequences_explore_grants_a_turn_allowance_before_its_draw() {
    both(
        prepared_with_sources(
            &[
                ("Explore", 0, 0, true),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, true),
            ],
            &[EXPLORE],
        ),
        |g, q| {
            answer(g, q, Response::PlayLand { card: CardId(2) });
            request(g, q);
            assert!(!g.has_land_play_allowance(PlayerId(0)));
            let card = library(g)[0];
            announce(g, q, 0, vec![]);
            cursor(g, 0);
            assert!(!g.has_land_play_allowance(PlayerId(0)));
            cursor(g, 1);
            assert!(g.has_land_play_allowance(PlayerId(0)));
            assert_eq!(library(g)[0], card);
            recover(g);
            assert!(matches!(request(g, q), Request::Priority { .. }));
            assert_eq!(
                g.current(object(g, card.0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
            assert!(g.has_land_play_allowance(PlayerId(0)));
            answer(g, q, Response::PlayLand { card });
            request(g, q);
            assert!(!g.has_land_play_allowance(PlayerId(0)));
        },
    );
}

#[test]
fn phase1_sequences_rapid_pays_the_legal_target_controller_even_when_destroy_fails() {
    for case in ["destroyed", "indestructible", "illegal"] {
        both(
            prepared_with_sources(
                &[
                    ("Rapid Hybridization", 0, 0, true),
                    (
                        "Grizzly Bears",
                        if case == "illegal" { 1 } else { 0 },
                        1,
                        false,
                    ),
                    ("Forest", 0, 0, false),
                    ("Island", 0, 0, false),
                    ("Withstand Death", 0, 0, true),
                ],
                &[
                    RAPID,
                    include_str!(
                        "../../../../tests/fixtures/c462adf3-4ade-4b90-bdec-7247b080471f.json"
                    ),
                ],
            ),
            |g, q| {
                if case == "indestructible" {
                    cast(g, q, 4, target(g, 1));
                }
                let bear = object(g, 1);
                announce(g, q, 0, target(g, 1));
                if case == "illegal" {
                    g.concede(PlayerId(1)).unwrap();
                } else {
                    cursor(g, 0);
                    cursor(g, 1);
                    assert!(g.state.rules.tokens.is_empty());
                    recover(g);
                }
                for _ in 0..12 {
                    let r = request(g, q);
                    if g.state.rules.stack.is_empty() {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                assert!(g.state.rules.stack.is_empty());
                if case == "illegal" {
                    assert!(g.state.rules.tokens.is_empty());
                    return;
                }
                if case == "indestructible" {
                    assert_eq!(object(g, 1), bear);
                } else {
                    assert_ne!(object(g, 1), bear);
                    assert_eq!(
                        g.current(object(g, 1)).unwrap().zone,
                        Zone::Graveyard { owner: PlayerId(0) }
                    );
                }
                let (token, _) = g.state.rules.tokens.first_key_value().unwrap();
                assert_eq!(g.state.rules.tokens.len(), 1);
                assert_eq!(
                    g.current(object(g, token.0)).unwrap().controller,
                    PlayerId(1)
                );
            },
        );
    }
}

#[test]
fn phase1_sequences_static_subtype_filters_follow_live_sources_and_token_entry() {
    both(
        prepared_with_sources(
            &[
                ("Master Splicer", 0, 0, true),
                ("Wing Splicer", 1, 0, false),
                ("Grizzly Bears", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Swords to Plowshares", 0, 0, true),
            ],
            &[WING, MASTER],
        ),
        |g, q| {
            cast(g, q, 0, vec![]);
            let (token, _) = g.state.rules.tokens.first_key_value().unwrap();
            let golem = object(g, token.0);
            assert!(g.has_keyword(golem, KeywordAbility::Flying));
            assert!(!g.has_keyword(object(g, 2), KeywordAbility::Flying));
            assert_eq!(g.characteristics(golem).unwrap().power, Some(4.into()));
            recover(g);
            cast(g, q, 8, target(g, 1));
            assert!(!g.has_keyword(golem, KeywordAbility::Flying));
            assert_eq!(g.characteristics(golem).unwrap().power, Some(4.into()));
        },
    );
}

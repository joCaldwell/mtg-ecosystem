use super::*;
use mtg_witness::aura::prepared_with_sources;
use mtg_witness::equipped::{answer, both, cast, object, pay_to_priority, recover, request};

const RECKLESS: &str =
    include_str!("../../../../tests/fixtures/584cf0fd-112a-4ca6-9c0e-1f3228a7d325.json");
const ACT: &str =
    include_str!("../../../../tests/fixtures/19ad338a-c89f-406d-ad1a-222c22044e1b.json");
const PLAYER: PlayerId = PlayerId(0);

fn prepared(name: &'static str, raw: &'static str) -> Prepared {
    let mut p = prepared_with_sources(
        &[
            ("Mountain", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
            (name, 0, 0, true),
            ("Forest", 0, 0, true),
        ],
        &[raw],
    );
    // The first two cards of the real library are a creature and a land.
    p.config.cards[6].oracle_id = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap()
        .oracle_id
        .clone();
    p.config.cards[9].zone = Zone::Exile;
    p.config.initial.libraries = None;
    p.profile_digest = digest(&p.config).unwrap();
    p
}

fn until_main(game: &mut Game, q: u64, owner: PlayerId, after_turn: u64) {
    for _ in 0..500 {
        let r = request(game, q);
        if matches!(r, Request::Priority { player, .. } if player == owner)
            && game.state.rules.turn.number > after_turn
            && game.main_phase_action(owner)
        {
            return;
        }
        let response = match &r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates.iter().take(*count).copied().collect(),
            },
            _ => panic!("unexpected turn decision {r:?}"),
        };
        game.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("missing later main phase");
}

#[test]
fn composition_wave_d_paid_exile_play_preserves_incarnations_payment_and_next_turn() {
    both(prepared("Reckless Impulse", RECKLESS), |g, q| {
        let first = g.state.rules.turn.number;
        let old = object(g, 6);
        cast(g, q, 4, vec![]);
        let creature = object(g, 6);
        let land = object(g, 7);
        assert_ne!(creature, old);
        assert!(g.may_play_exile(PLAYER, creature));
        assert!(g.may_play_exile(PLAYER, land));
        assert!(!g.may_play_exile(PlayerId(1), land));
        assert!(!g.may_play_exile(PLAYER, object(g, 9)));
        assert!(!g.may_play_exile(PLAYER, old));
        recover(g);

        let mut forged = g.fork();
        let unrelated = object(&forged, 9);
        let ResolvedEffectKind::PlayExiled { objects, .. } =
            &mut forged.state.rules.effects[0].kind
        else {
            panic!("expected play permission");
        };
        objects[1] = unrelated;
        assert!(
            Game::restore(
                forged.prepared.as_ref().clone(),
                &forged.snapshot().unwrap()
            )
            .is_err()
        );

        // Propose and undo through ordinary mana confirmation. The unchanged
        // exiled incarnation remains authorized for a later paid cast.
        answer(
            g,
            q,
            Response::Cast {
                card: creature.card,
            },
        );
        let Request::Mana { cost, .. } = request(g, q) else {
            panic!("expected normal mana cost");
        };
        assert_eq!(
            cost,
            ManaCost {
                generic: 1.into(),
                colored: vec![Color::G]
            }
        );
        recover(g);
        answer(g, q, Response::FinishMana { undo_mana: true });
        request(g, q);
        assert!(g.may_play_exile(PLAYER, creature));

        answer(
            g,
            q,
            Response::Cast {
                card: creature.card,
            },
        );
        pay_to_priority(g, q, vec![]);
        assert_eq!(g.top().unwrap().cast_from, Zone::Exile);
        assert_eq!(g.top().unwrap().alternative_cost, None);
        recover(g);
        while !g.state.rules.stack.is_empty() {
            answer(g, q, Response::Pass);
            request(g, q);
        }
        assert!(!g.may_play_exile(PLAYER, creature));
        until_main(g, q, PlayerId(1), first);
        assert!(g.may_play_exile(PLAYER, land));
        assert!(g.validate_land_play(PlayerId(1), land.card).is_err());
        until_main(g, q, PLAYER, first);
        assert!(g.validate_land_play(PLAYER, land.card).is_ok());
        answer(g, q, Response::PlayLand { card: land.card });
        request(g, q);
        assert_eq!(g.current(object(g, 7)).unwrap().zone, Zone::Battlefield);
        assert!(g.validate_land_play(PLAYER, CardId(5)).is_err());
        let second = g.state.rules.turn.number;
        until_main(g, q, PlayerId(1), second);
        assert!(g.state.rules.effects.is_empty());
    });
}

#[test]
fn composition_wave_d_same_turn_window_leaves_exiled_cards_after_cleanup() {
    both(prepared("Act on Impulse", ACT), |g, q| {
        let first = g.state.rules.turn.number;
        cast(g, q, 4, vec![]);
        let land = object(g, 7);
        assert!(g.may_play_exile(PLAYER, land));
        until_main(g, q, PlayerId(1), first);
        assert!(!g.may_play_exile(PLAYER, land));
        assert_eq!(g.current(land).unwrap().zone, Zone::Exile);
        assert!(g.state.rules.effects.is_empty());
    });
}

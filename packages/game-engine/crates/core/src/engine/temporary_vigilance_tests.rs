//! Paid CR702.20/508.1f/611.2c/514.2 witness; no new combat algorithm.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const PROPORTIONS: &str =
    include_str!("../../../../tests/fixtures/1e03aad2-6426-4d18-a0e5-bc3d8e6b7b11.json");
#[test]
fn temporary_vigilance_paid_capture_attack_without_tap_damage_and_cleanup() {
    for grant in [false, true] {
        let mut rows = vec![
            ("Preposterous Proportions", 0, 0, true),
            ("Grizzly Bears", 0, 0, false),
            ("Grizzly Bears", 0, 0, true),
            ("Grizzly Bears", 1, 1, false),
        ];
        rows.extend(std::iter::repeat_n(("Forest", 0, 0, false), 9));
        both(prepared_with_sources(&rows, &[PROPORTIONS]), |g, q| {
            if grant {
                cast(g, q, 0, vec![]);
                assert_eq!(g.state.rules.effects.len(), 1);
                assert_eq!(
                    g.characteristics(object(g, 1)).unwrap().power,
                    Some(12.into())
                );
                assert!(!g.has_keyword(object(g, 3), KeywordAbility::Vigilance));
                for v in 0..4 {
                    assert!(
                        matches!(g.visible_effects(PlayerId(v)).unwrap().as_slice(),[VisibleEffect::GrantKeywordsSet{keywords,..}] if keywords==&[KeywordAbility::Vigilance])
                    );
                }
                let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
                let ResolvedEffectKind::GrantKeywordsSet { keywords, .. } =
                    &mut bad.state.rules.effects[0].kind
                else {
                    panic!("grant")
                };
                *keywords = vec![KeywordAbility::Flying];
                bad.state_sha256 = digest(&bad.state).unwrap();
                assert!(
                    Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err()
                );
            }
            until_player(g, q, PlayerId(0));
            cast(g, q, 2, vec![]);
            assert!(!g.has_keyword(object(g, 2), KeywordAbility::Vigilance));
            assert_eq!(
                g.characteristics(object(g, 2)).unwrap().power,
                Some(2.into())
            );
            for _ in 0..100 {
                let r = request(g, q);
                if matches!(r, Request::Attackers { .. }) {
                    break;
                }
                answer(g, q, pass(&r));
            }
            answer(
                g,
                q,
                Response::Attackers {
                    attacks: vec![(object(g, 1), PlayerId(1))],
                },
            );
            assert_eq!(g.current(object(g, 1)).unwrap().tapped, !grant);
            recover(g);
            let turn = g.turn().number;
            for _ in 0..300 {
                let r = request(g, q);
                if g.turn().number > turn {
                    break;
                }
                answer(g, q, pass(&r));
            }
            assert!(g.turn().number > turn);
            assert_eq!(
                g.state.rules.players[1].life,
                Quantity::from(if grant { 28 } else { 38 })
            );
            assert!(!g.has_keyword(object(g, 1), KeywordAbility::Vigilance));
            assert_eq!(
                g.characteristics(object(g, 1)).unwrap().power,
                Some(2.into())
            );
            assert!(g.state.rules.effects.is_empty());
            recover(g);
        });
    }
}

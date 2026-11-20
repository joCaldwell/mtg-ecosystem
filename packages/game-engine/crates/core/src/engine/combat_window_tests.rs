use super::aura_control_tests::prepared_with_sources;
use super::equipped_indestructible_tests::{answer, both, cast, object, pay_and_resolve, recover};
use super::temporary_indestructible_tests::target;
use super::*;

#[test]
fn composition_wave_c_paid_land_activation_installs_a_replayable_restriction() {
    const PASSAGE: &str =
        include_str!("../../../../tests/fixtures/f29dc596-2121-4421-8463-15f6c2e8b9b3.json");
    both(
        prepared_with_sources(
            &[
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Rogue's Passage", 0, 0, false),
                ("Grizzly Bears", 0, 0, false),
                ("Grizzly Bears", 1, 1, false),
            ],
            &[PASSAGE],
        ),
        |g, q| {
            let bearer = object(g, 5);
            assert!(!g.has_restriction(bearer, CreatureRestriction::CantBeBlocked));
            let passage = object(g, 4);
            answer(
                g,
                q,
                Response::ActivateAbility {
                    object: passage,
                    ability: 0,
                },
            );
            pay_and_resolve(g, q, target(g, 5));
            assert!(g.current(passage).unwrap().tapped);
            assert!(g.has_restriction(bearer, CreatureRestriction::CantBeBlocked));
            assert!(!g.has_restriction(object(g, 6), CreatureRestriction::CantBeBlocked));
            assert!(
                matches!(&g.state.rules.effects[0].kind,ResolvedEffectKind::CombatRestriction{target,..} if *target==bearer)
            );
            recover(g);
        },
    );
}

#[test]
fn composition_wave_c_paid_cast_uses_spell_subtype_not_card_kind() {
    const SRAM: &str =
        include_str!("../../../../tests/fixtures/7e00b0cd-d212-4604-ba07-da21f4fe00b0.json");
    both(
        prepared_with_sources(
            &[
                ("Sram, Senior Edificer", 0, 0, false),
                ("Vulshok Morningstar", 0, 0, true),
                ("Grizzly Bears", 0, 0, true),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
            ],
            &[SRAM],
        ),
        |g, q| {
            let n = g.state.rules.libraries[&PlayerId(0)].len();
            cast(g, q, 2, vec![]);
            assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), n);
            cast(g, q, 1, vec![]);
            assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), n - 1);
            recover(g);
        },
    );
}

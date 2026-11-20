use super::aura_control_tests::prepared_with_sources;
use super::equipped_indestructible_tests::{both, cast, object, recover};

#[test]
fn composition_wave_e_paid_enchantment_applies_signed_stats_by_controller() {
    const SURGE: &str =
        include_str!("../../../../tests/fixtures/fb1755a0-3334-419b-8cb5-5a3ac7fa5b13.json");
    both(
        prepared_with_sources(
            &[
                ("Mountain", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Flowstone Surge", 0, 0, true),
                ("Grizzly Bears", 0, 0, false),
                ("Grizzly Bears", 0, 1, false),
                ("Grizzly Bears", 1, 0, false),
            ],
            &[SURGE],
        ),
        |g, q| {
            cast(g, q, 2, vec![]);
            for (id, expected_power, expected_toughness) in [(3, 3, 1), (4, 2, 2), (5, 3, 1)] {
                let c = g.characteristics(object(g, id)).unwrap();
                assert_eq!(c.power, Some(expected_power.into()));
                assert_eq!(c.toughness, Some(expected_toughness.into()));
            }
            recover(g);
        },
    );
}

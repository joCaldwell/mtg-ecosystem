//! Private catalog-domain regressions; no widened public admission is claimed.
use super::*;

#[test]
fn all_catalogs_share_the_same_capacity_policy() {
    let fixtures = [
        "match",
        "keywords",
        "combat-keywords",
        "tokens",
        "staples",
        "mana",
        "four-colors",
        "attachments",
        "removal",
        "sacrifice",
        "wipes",
    ];
    assert_eq!(fixtures.len(), MATCH_PROFILES.len());
    for (profile, fixture) in MATCH_PROFILES.iter().zip(fixtures) {
        let raw = std::fs::read_to_string(format!(
            "{}/../../tests/fixtures/expected-{fixture}-ir.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let prepared = Prepared::admit_pod(
            Catalog::read(&raw).unwrap(),
            MatchConfig::for_profile(profile.name, [0; 32], PlayerId(0)).unwrap(),
        )
        .unwrap();
        let game = Game::new(prepared);
        assert_eq!(game.token_capacity(), capacities::TOKEN_CAPACITY);
        assert_eq!(game.damage_limit(), capacities::DAMAGE_LIMIT);
        assert_eq!(game.stack_limit(), capacities::STACK_LIMIT);
        assert_eq!(game.event_group_limit(), capacities::EVENT_GROUP_LIMIT);
        assert_eq!(game.effect_limit(), capacities::EFFECT_LIMIT);
    }
}

#[test]
fn cost_reduction_is_driven_by_static_ir_under_any_catalog_label() {
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let reducer = catalog
        .definitions
        .iter()
        .find(|d| {
            d.static_abilities
                .iter()
                .any(|s| s.effect == StaticEffect::ReduceGenericByCreatureCount)
        })
        .unwrap();
    let reducer_id = reducer.oracle_id.clone();
    let mut expected = reducer.characteristics.mana_cost.clone();
    expected.generic.subtract(&3.into());
    if expected.generic.is_negative() {
        expected.generic = Quantity::zero();
    }
    for label in ["independent-catalog", MATCH_PROFILE, WIPES_PROFILE] {
        let mut prepared = Prepared::admit_pod(
            catalog.clone(),
            MatchConfig::for_profile(WIPES_PROFILE, [0; 32], PlayerId(0)).unwrap(),
        )
        .unwrap();
        prepared.config.profile = label.into();
        let mut count = 0;
        for card in &mut prepared.config.cards {
            let d = catalog
                .definitions
                .iter()
                .find(|d| d.oracle_id == card.oracle_id)
                .unwrap();
            if d.characteristics.kind == CardKind::Creature && count < 3 {
                card.zone = Zone::Battlefield;
                count += 1;
            }
        }
        assert_eq!(count, 3);
        let card = prepared
            .config
            .cards
            .iter()
            .find(|c| c.oracle_id == reducer_id)
            .unwrap();
        let (id, owner) = (card.id, card.owner);
        let game = Game::new(prepared);
        assert_eq!(
            game.casting_cost(id, owner, Zone::Hand { owner }).unwrap(),
            expected
        );
    }
}

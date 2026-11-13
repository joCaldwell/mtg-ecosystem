//! Explicit development admission, without granting hosted/search capabilities.
use mtg_contracts::{Capability, Catalog};
use mtg_core::*;

const PROFILE: &str = "commander-blocking-targets-v1";

fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-blocking-matches-ir.json"
    ))
    .unwrap()
}

fn oracle(catalog: &Catalog, name: &str) -> String {
    catalog
        .definitions
        .iter()
        .find(|definition| definition.name == name)
        .unwrap()
        .oracle_id
        .clone()
}

#[test]
fn blocking_profile_adds_one_complete_definition_and_admits_four_legal_decks() {
    let catalog = catalog();
    let original = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    assert_eq!(catalog.schema_version, 1);
    assert_eq!(catalog.definitions.len(), original.definitions.len() + 1);
    for definition in &original.definitions {
        assert_eq!(
            catalog
                .definitions
                .iter()
                .find(|candidate| candidate.oracle_id == definition.oracle_id),
            Some(definition)
        );
    }
    let profile = match_profile(PROFILE).unwrap();
    assert_eq!(profile.catalog(), &catalog);
    assert!(!profile.hosted && !profile.search);
    let config = MatchConfig::for_profile(PROFILE, [41; 32], PlayerId(0)).unwrap();
    let pod = validate_pod(&catalog, &config).unwrap();
    assert_eq!(pod.physical_config().cards.len(), 400);
    let righteousness = oracle(&catalog, "Righteousness");
    for (seat, deck) in config.decks.iter().enumerate() {
        assert_eq!(deck.cards.iter().map(|row| row.count).sum::<u32>(), 100);
        assert_eq!(
            deck.cards
                .iter()
                .filter(|row| row.oracle_id == righteousness)
                .map(|row| row.count)
                .sum::<u32>(),
            u32::from(seat.is_multiple_of(2))
        );
    }
    Prepared::admit_pod(catalog, config).unwrap();
}

#[test]
fn blocking_profile_retains_singleton_color_and_feature_rejection() {
    let catalog = catalog();
    let config = MatchConfig::for_profile(PROFILE, [41; 32], PlayerId(0)).unwrap();
    let righteousness = oracle(&catalog, "Righteousness");
    let plains = oracle(&catalog, "Plains");
    let forest = oracle(&catalog, "Forest");
    let mut duplicate = config.clone();
    for row in &mut duplicate.decks[0].cards {
        if row.oracle_id == righteousness {
            row.count += 1;
        } else if row.oracle_id == plains {
            row.count -= 1;
        }
    }
    assert!(matches!(
        validate_pod(&catalog, &duplicate),
        Err(MatchAdmissionError::SingletonName {
            player: PlayerId(0),
            ..
        })
    ));
    let mut off_color = config;
    off_color.decks[1]
        .cards
        .iter_mut()
        .find(|row| row.oracle_id == forest)
        .unwrap()
        .count -= 1;
    off_color.decks[1].cards.push(DeckEntry {
        oracle_id: righteousness.clone(),
        count: 1,
    });
    assert!(matches!(
        validate_pod(&catalog, &off_color),
        Err(MatchAdmissionError::ColorIdentity { player: PlayerId(1), oracle_id })
            if oracle_id == righteousness
    ));
    for feature in [Feature::Hosting, Feature::Search] {
        let mut requested = match_profile(PROFILE).unwrap().clone();
        match feature {
            Feature::Hosting => requested.hosted = true,
            Feature::Search => requested.search = true,
        }
        assert!(matches!(
            requested.validate_requested_features(&catalog),
            Err(ProfileError::UnreviewedCapabilities { feature: actual, capabilities })
                if actual == feature
                    && capabilities.contains(&Capability::BlockingCreatureTargetV1)
        ));
    }
}

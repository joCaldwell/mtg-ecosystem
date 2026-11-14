//! Explicit development admission, without granting hosted/search capabilities.
use mtg_contracts::{Capability, CardKind, Catalog, Color};
use mtg_core::*;
use std::collections::BTreeMap;

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
fn development_profile_adds_eleven_complete_definitions_and_admits_four_legal_decks() {
    let catalog = catalog();
    let original = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    assert_eq!(catalog.schema_version, 1);
    assert_eq!(catalog.definitions.len(), original.definitions.len() + 11);
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
    let swift_response = oracle(&catalog, "Swift Response");
    let assassinate = oracle(&catalog, "Assassinate");
    let burst = oracle(&catalog, "Burst of Energy");
    let pressure = oracle(&catalog, "Pressure Point");
    let icy = oracle(&catalog, "Icy Manipulator");
    let royal = oracle(&catalog, "Royal Assassin");
    let ertai = oracle(&catalog, "Ertai, Wizard Adept");
    let temporal = oracle(&catalog, "Temporal Adept");
    let seeker = oracle(&catalog, "Seeker of Skybreak");
    let elf = oracle(&catalog, "Llanowar Elves");
    assert!(
        catalog
            .definition_by_oracle_id(&seeker)
            .unwrap()
            .capabilities
            .contains(&Capability::TapSourceUntapCreatureV1)
    );
    assert!(
        catalog
            .definition_by_oracle_id(&temporal)
            .unwrap()
            .capabilities
            .contains(&Capability::ActivatedPermanentOwnerHandV1)
    );
    assert!(profile.commanders.contains(&ertai));
    assert!(
        catalog
            .definition_by_oracle_id(&royal)
            .unwrap()
            .capabilities
            .contains(&Capability::TapSourceDestroyV1)
    );
    assert!(
        catalog
            .definition_by_oracle_id(&ertai)
            .unwrap()
            .capabilities
            .contains(&Capability::ActivatedSpellCounterV1)
    );
    assert!(
        catalog
            .definition_by_oracle_id(&icy)
            .unwrap()
            .capabilities
            .contains(&Capability::ActivatedTapPermanentV1)
    );
    for (seat, deck) in config.decks.iter().enumerate() {
        assert_eq!(deck.cards.iter().map(|row| row.count).sum::<u32>(), 100);
        assert_eq!(
            deck.cards
                .iter()
                .filter(|row| row.oracle_id == righteousness)
                .map(|row| row.count)
                .sum::<u32>(),
            u32::from(seat == 0)
        );
        for (oracle_id, expected) in [
            (&swift_response, u32::from(seat == 0)),
            (&burst, u32::from(seat == 0)),
            (&pressure, u32::from(seat == 0)),
            (&icy, 1),
            (&royal, u32::from(matches!(seat, 1 | 3))),
            (&ertai, u32::from(seat == 2)),
            (&temporal, u32::from(seat == 2)),
            (&seeker, u32::from(matches!(seat, 1 | 3))),
            (&elf, u32::from(matches!(seat, 1 | 3))),
            (
                &oracle(&catalog, "Forest"),
                if matches!(seat, 1 | 3) { 40 } else { 0 },
            ),
            (
                &oracle(&catalog, "Swamp"),
                if matches!(seat, 1 | 3) { 43 } else { 0 },
            ),
            (&assassinate, u32::from(matches!(seat, 1 | 3))),
        ] {
            assert_eq!(
                deck.cards
                    .iter()
                    .filter(|row| row.oracle_id == *oracle_id)
                    .map(|row| row.count)
                    .sum::<u32>(),
                expected
            );
        }
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
    let mut off_color = config.clone();
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
    for (seat, basic, added) in [
        (1, forest.clone(), oracle(&catalog, "Swift Response")),
        (0, plains.clone(), oracle(&catalog, "Assassinate")),
    ] {
        let mut wrong_color = config.clone();
        wrong_color.decks[seat]
            .cards
            .iter_mut()
            .find(|row| row.oracle_id == basic)
            .unwrap()
            .count -= 1;
        wrong_color.decks[seat].cards.push(DeckEntry {
            oracle_id: added.clone(),
            count: 1,
        });
        assert!(matches!(validate_pod(&catalog, &wrong_color),
            Err(MatchAdmissionError::ColorIdentity { player, oracle_id })
                if player == PlayerId(seat as u8) && oracle_id == added));
    }
    // The new blue seat must retain the same singleton and color guards.
    let island = oracle(&catalog, "Island");
    let azure = oracle(&catalog, "Azure Mage");
    let mut blue_duplicate = config.clone();
    for row in &mut blue_duplicate.decks[2].cards {
        if row.oracle_id == azure {
            row.count += 1;
        }
        if row.oracle_id == island {
            row.count -= 1;
        }
    }
    assert!(matches!(
        validate_pod(&catalog, &blue_duplicate),
        Err(MatchAdmissionError::SingletonName {
            player: PlayerId(2),
            ..
        })
    ));
    for added in [righteousness, plains] {
        let mut wrong_color = config.clone();
        wrong_color.decks[2]
            .cards
            .iter_mut()
            .find(|row| row.oracle_id == island)
            .unwrap()
            .count -= 1;
        wrong_color.decks[2].cards.push(DeckEntry {
            oracle_id: added.clone(),
            count: 1,
        });
        assert!(matches!(validate_pod(&catalog, &wrong_color),
            Err(MatchAdmissionError::ColorIdentity { player: PlayerId(2), oracle_id }) if oracle_id == added));
    }
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
                    && capabilities.contains(&Capability::TappedPermanentTargetV1)
                    && capabilities.contains(&Capability::UntapPermanentV1)
                    && capabilities.contains(&Capability::ActivatedTapPermanentV1)
                    && capabilities.contains(&Capability::TapSourceDestroyV1)
                    && capabilities.contains(&Capability::ActivatedSpellCounterV1)
                    && capabilities.contains(&Capability::ActivatedPermanentOwnerHandV1)
                    && capabilities.contains(&Capability::TapSourceUntapCreatureV1)
        ));
    }
}

#[test]
fn blue_default_has_exact_source_backed_composition_and_preserves_other_seats() {
    let catalog = catalog();
    let profile = match_profile(PROFILE).unwrap();
    let config = MatchConfig::for_profile(PROFILE, [41; 32], PlayerId(0)).unwrap();
    let blue = &config.decks[2];
    let expected = [
        (
            "Ertai, Wizard Adept",
            "f2902dfd-888f-4b02-b8ad-1056f0059fc3",
            1,
        ),
        ("Azure Mage", "bbf9fd47-10e1-4749-b06f-4fe72200afc7", 1),
        (
            "Hapless Researcher",
            "240a349a-79f8-4e11-9a05-a78148e70a62",
            1,
        ),
        ("Merfolk Looter", "67362406-b1ca-49e2-800d-9050bfe8742a", 1),
        ("Counterspell", "cc187110-1148-4090-bbb8-e205694a39f5", 1),
        ("Stifle", "b3b00911-ece7-4484-bc36-f211ce72b6cc", 1),
        ("Time Ebb", "30cc8f7b-3c28-40f5-8f8f-157e8212280b", 1),
        ("Unsummon", "837182db-1bf3-4a2c-bd01-1af9d9873561", 1),
        ("Temporal Adept", "fd9b5462-6578-4bf6-8026-23eaf5af3eee", 1),
        ("Icy Manipulator", "3608f1f7-8dc5-4dd1-ae91-c830e1de9529", 1),
        ("Island", "b2c6aa39-2d2a-459c-a555-fb48ba993373", 90),
    ];
    let actual: BTreeMap<_, _> = blue
        .cards
        .iter()
        .map(|row| (row.oracle_id.as_str(), row.count))
        .collect();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(_, id, count)| (*id, *count))
            .collect()
    );
    assert_eq!(blue.cards.len(), expected.len());
    assert_eq!(blue.cards.iter().map(|row| row.count).sum::<u32>(), 100);
    assert_eq!(blue.commander, expected[0].1);
    for (name, id, count) in expected {
        let definition = catalog.definition_by_oracle_id(id).unwrap();
        assert_eq!(definition.name, name);
        assert!(
            definition
                .characteristics
                .color_identity
                .iter()
                .all(|color| *color == Color::U)
        );
        assert_eq!(count > 1, definition.characteristics.basic);
        let source: serde_json::Value =
            serde_json::from_str(&definition.provenance.source_json).unwrap();
        assert_eq!(source["name"], name);
        assert_eq!(source["oracle_id"], id);
        assert_eq!(source["legalities"]["commander"], "legal");
        assert_eq!(
            source["color_identity"],
            serde_json::to_value(&definition.characteristics.color_identity).unwrap()
        );
    }
    let commander = catalog.definition_by_oracle_id(&blue.commander).unwrap();
    assert!(commander.characteristics.legendary);
    assert!(commander.characteristics.has_type(CardKind::Creature));
    assert_eq!(commander.characteristics.color_identity, vec![Color::U]);

    // Exact pre-change compositions: this seat-only admission does not rewrite
    // the white or either black/green default deck.
    let white = [
        "Yoked Ox",
        "Sacred Nectar",
        "Icy Manipulator",
        "Healing Hands",
        "Righteousness",
        "Swift Response",
        "Healing Salve",
        "Samite Healer",
        "Burst of Energy",
        "Isamaru, Hound of Konda",
        "Pressure Point",
    ];
    let black_green = [
        "Cruel Edict",
        "Grizzly Bears",
        "Fog",
        "Blood Artist",
        "Icy Manipulator",
        "Giant Growth",
        "Llanowar Elves",
        "Essence Warden",
        "Seeker of Skybreak",
        "Disfigure",
        "Soul Feast",
        "Slaughterhouse Bouncer",
        "Yargle and Multani",
        "Royal Assassin",
        "Wall of Wood",
        "Elvish Visionary",
        "Assassinate",
    ];
    for (seat, commander, singletons, basics) in [
        (
            0,
            "Isamaru, Hound of Konda",
            white.as_slice(),
            vec![("Plains", 89)],
        ),
        (
            1,
            "Yargle and Multani",
            black_green.as_slice(),
            vec![("Swamp", 43), ("Forest", 40)],
        ),
        (
            3,
            "Yargle and Multani",
            black_green.as_slice(),
            vec![("Swamp", 43), ("Forest", 40)],
        ),
    ] {
        let deck = &config.decks[seat];
        assert_eq!(deck.commander, oracle(&catalog, commander));
        let expected: BTreeMap<_, _> = singletons
            .iter()
            .map(|name| (oracle(&catalog, name), 1))
            .chain(
                basics
                    .into_iter()
                    .map(|(name, count)| (oracle(&catalog, name), count)),
            )
            .collect();
        assert_eq!(deck.cards.len(), expected.len());
        assert_eq!(
            deck.cards
                .iter()
                .map(|row| (row.oracle_id.clone(), row.count))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
    }
    assert_eq!(
        profile.commanders,
        [
            "Isamaru, Hound of Konda",
            "Yargle and Multani",
            "Ertai, Wizard Adept"
        ]
        .map(|name| oracle(&catalog, name))
    );
    assert!(!profile.completion.rotate);
    assert_eq!(profile.completion.homogeneous_seats, [0, 1]);
    assert_eq!(
        profile.commander_policy.banned_list,
        "../commander-banned-2026-10-15.json"
    );
    assert!(matches!(
        profile.commander_policy.color_identity,
        ColorIdentityPolicy::CommanderSubset
    ));
    assert!(matches!(
        profile.commander_policy.eligibility,
        CommanderEligibility::LegendaryCreature
    ));
    Prepared::admit_pod(catalog, config).unwrap();
}

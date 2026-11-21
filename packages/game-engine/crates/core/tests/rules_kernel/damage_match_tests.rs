//! Admission and seeded setup for the explicit finite damage-trigger match
//! profile. Full-game/replay evidence belongs to the separate CLI execution.
use super::*;

const PROFILE: &str = "commander-damage-triggers-v1";
const COMMANDER: &str = "333cc91b-01ab-4491-a993-d8304fc382c6";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const OTTER: &str = "5dee663f-9e2e-4503-8086-fc51664fdf29";
const COMMANDO: &str = "48936751-429b-4c40-88f5-72acde6278cc";
const CUTPURSE: &str = "56c1a548-c80c-432b-bf92-d4ef6117b298";
const DEMON: &str = "2e55fc03-779c-4a71-beb7-aa0ade738c24";

fn profile() -> &'static MatchProfile {
    match_profile(PROFILE).expect("the explicit finite damage-trigger profile")
}
fn config() -> MatchConfig {
    MatchConfig::for_profile(PROFILE, [0x35; 32], PlayerId(2)).unwrap()
}
fn request(g: &mut Game, quantum: u64) -> Request {
    for _ in 0..4_000 {
        match g.advance(Budget::new(quantum)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("damage-match setup: {other:?}"),
        }
    }
    panic!("damage-match setup exceeded its bounded probe")
}

#[test]
fn damage_match_profile_binds_complete_catalog_and_exact_source_identity() {
    let p = profile();
    let c = p.catalog();
    assert_eq!(p.name, PROFILE);
    assert!(!p.hosted && !p.search);
    assert!(!p.completion.rotate);
    assert_eq!(p.completion.homogeneous_seats, [0, 1]);
    assert!(p.decks.iter().all(|deck| deck == &p.decks[0]));

    assert_eq!(c.schema_version, mtg_contracts::CURRENT_IR_VERSION);
    assert_eq!(c.definitions.len(), 32);
    assert_eq!(digest(c).unwrap(), p.catalog_sha256);
    assert_eq!(p.commanders, [COMMANDER]);
    c.validate().unwrap();
    for d in &c.definitions {
        let raw: serde_json::Value = strict_read(&d.provenance.source_json).unwrap();
        assert_eq!(raw["oracle_id"], d.oracle_id);
        assert_eq!(raw["name"], d.name);
        assert_eq!(raw["legalities"]["commander"], "legal");
        assert_eq!(
            sha256(d.provenance.source_json.as_bytes()),
            d.provenance.source_sha256
        );
        assert_eq!(
            sha256(format!("[{}]", d.provenance.abilities_json).as_bytes()),
            d.provenance.ast_sha256
        );
        assert_eq!(d.capabilities, d.requirements());
    }
    let commander = c.definition_by_oracle_id(COMMANDER).unwrap();
    assert_eq!(commander.name, "Sivitri Scarzam");
    assert!(commander.characteristics.legendary);
    assert_eq!(commander.characteristics.kind, CardKind::Creature);
    // CR 903.4/903.5c: color identity is membership, not an ordering. Keep the
    // source's [B, U] bytes unchanged and compare the exact supported set.
    assert_eq!(
        commander
            .characteristics
            .color_identity
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([Color::B, Color::U])
    );
    assert!(
        commander.activated.is_empty()
            && commander.triggered.is_empty()
            && commander.static_abilities.is_empty()
    );
    for deck in &p.decks {
        assert_eq!(deck.commander, COMMANDER);
        assert_eq!(
            deck.cards.iter().map(|r| u64::from(r.count)).sum::<u64>(),
            100
        );
        let mut nonbasic = 0;
        for row in &deck.cards {
            let d = c.definition_by_oracle_id(&row.oracle_id).unwrap();
            assert!(
                d.characteristics
                    .color_identity
                    .iter()
                    .all(|color| commander.characteristics.color_identity.contains(color))
            );
            if d.characteristics.basic {
                assert!([ISLAND, SWAMP].contains(&row.oracle_id.as_str()));
                assert_eq!(row.count, 35);
            } else {
                assert_eq!(row.count, 1);
                nonbasic += 1;
            }
        }
        assert_eq!(nonbasic, 30);
        for id in [COMMANDER, OTTER, COMMANDO, CUTPURSE, DEMON, ISLAND, SWAMP] {
            assert!(deck.cards.iter().any(|row| row.oracle_id == id));
        }
    }
}

#[test]
fn damage_match_four_legal_decks_seeded_setup_and_fork_recovery() {
    let p = profile();
    let prepared = Prepared::admit_pod(p.catalog().clone(), config()).unwrap();
    assert_eq!(prepared.catalog_digest(), p.catalog_sha256);
    assert_eq!(
        prepared.match_config().unwrap().starting_player,
        PlayerId(2)
    );
    assert_eq!(prepared.config.cards.len(), 400);
    for seat in 0..4 {
        let owner = PlayerId(seat);
        let cards: Vec<_> = prepared
            .config
            .cards
            .iter()
            .filter(|c| c.owner == owner)
            .collect();
        assert_eq!(cards.len(), 100);
        assert_eq!(
            cards
                .iter()
                .filter(|c| c.zone == (Zone::Command { owner }))
                .count(),
            1
        );
        assert_eq!(
            cards
                .iter()
                .filter(|c| c.zone == (Zone::Library { owner }))
                .count(),
            99
        );
        assert!(cards.iter().all(|c| c.controller == owner));
    }
    let mut small = Game::new(prepared.clone());
    let mut large = small.fork();
    let first = request(&mut small, 1);
    assert_eq!(first, request(&mut large, u64::MAX));
    assert_eq!(small.state, large.state);
    assert!(matches!(
        first,
        Request::Mulligan {
            player: PlayerId(2),
            ..
        }
    ));
    for seat in [2, 3, 0, 1] {
        let r = request(&mut small, 1);
        assert!(matches!(r, Request::Mulligan { player, .. } if player == PlayerId(seat)));
        small
            .respond(
                r.player(),
                r.decision_id(),
                Response::Mulligan { take: false },
            )
            .unwrap();
    }
    assert!(matches!(
        request(&mut small, 1),
        Request::Priority {
            player: PlayerId(2),
            ..
        }
    ));
    for seat in 0..4 {
        let owner = PlayerId(seat);
        assert_eq!(
            small
                .state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == (Zone::Hand { owner }))
                .count(),
            7
        );
        assert_eq!(small.state.rules.libraries[&owner].len(), 92);
    }
    let restored = Game::restore(prepared.clone(), &small.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, small.state);
    let replayed = Game::replay_to(
        prepared,
        &small.replay_journal(),
        Some(small.state.rules.rule_steps),
        small.trace.mode,
    )
    .unwrap();
    assert_eq!(replayed.state, small.state);
    let unchanged = small.state.clone();
    let mut branch = small.fork();
    branch.concede(PlayerId(3)).unwrap();
    assert_eq!(small.state, unchanged);
    assert_ne!(branch.state, small.state);
}

#[test]
fn damage_match_admission_rejects_invalid_decks_and_changed_catalog_identity() {
    let c = profile().catalog();
    let original = config();
    let mut changed = original.clone();
    changed.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == ISLAND)
        .unwrap()
        .count -= 1;
    assert!(matches!(
        Prepared::admit_pod(c.clone(), changed),
        Err(MatchAdmissionError::InvalidDeckSize { total: 99, .. })
    ));
    let mut changed = original.clone();
    changed.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == ISLAND)
        .unwrap()
        .count -= 1;
    changed.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == OTTER)
        .unwrap()
        .count += 1;
    assert!(matches!(
        Prepared::admit_pod(c.clone(), changed),
        Err(MatchAdmissionError::SingletonName { .. })
    ));
    // Exact retained Drider source is executable, but outside this reviewed pool.
    let drider = "1db76752-31ed-4a16-986a-d0852ba7d812";
    assert!(c.definition_by_oracle_id(drider).is_none());
    let mut changed = original.clone();
    changed.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == OTTER)
        .unwrap()
        .oracle_id = drider.into();
    assert!(
        matches!(Prepared::admit_pod(c.clone(), changed), Err(MatchAdmissionError::UnsupportedCard { oracle_id, .. }) if oracle_id == drider)
    );
    let mut changed = original.clone();
    changed.decks[0].commander = OTTER.into();
    assert!(matches!(
        Prepared::admit_pod(c.clone(), changed),
        Err(MatchAdmissionError::UnsupportedCommander { .. })
    ));
    let mut changed = original.clone();
    changed.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == COMMANDER)
        .unwrap()
        .oracle_id = ISLAND.into();
    assert!(matches!(
        Prepared::admit_pod(c.clone(), changed),
        Err(MatchAdmissionError::CommanderCount { count: 0, .. })
    ));
    let mut changed_catalog = c.clone();
    changed_catalog.definitions.pop();
    assert!(matches!(
        Prepared::admit_pod(changed_catalog, original),
        Err(MatchAdmissionError::InvalidCatalog { .. })
    ));
}

#[test]
fn damage_match_hosting_search_flags_and_each_new_capability_stay_unreviewed() {
    let p = profile();
    let c = p.catalog();
    let witnesses = [
        (OTTER, Capability::SourceDamageTriggerV2),
        (COMMANDO, Capability::OptionalControllerDrawV1),
        (CUTPURSE, Capability::EventPlayerDiscardV1),
        (DEMON, Capability::EventPlayerSacrificeV1),
    ];
    for feature in [Feature::Search, Feature::Hosting] {
        assert!(
            matches!(p.require_feature(c, feature), Err(ProfileError::FeatureNotRequested { feature: f }) if f == feature)
        );
        for (id, capability) in witnesses {
            let definition = c.definition_by_oracle_id(id).unwrap().clone();
            assert!(definition.requirements().contains(&capability));
            // Independently select each real complete definition, rather than
            // allowing another new mechanic to cause the admission rejection.
            let only = Catalog {
                schema_version: c.schema_version,
                rules_edition: c.rules_edition.clone(),
                rules_sha256: c.rules_sha256.clone(),
                definitions: vec![definition],
            };
            only.validate().unwrap();
            let mut requested = p.clone();
            requested.search = feature == Feature::Search;
            requested.hosted = feature == Feature::Hosting;
            assert!(
                matches!(requested.validate_requested_features(&only), Err(ProfileError::UnreviewedCapabilities { feature: f, capabilities }) if f == feature && capabilities.contains(&capability))
            );
        }
    }
}

//! Private catalog-domain regressions; no widened public admission is claimed.
use super::*;

#[test]
fn all_catalogs_share_the_same_capacity_policy() {
    for profile in MATCH_PROFILES.iter() {
        let prepared = Prepared::admit_pod(
            profile.catalog().clone(),
            MatchConfig::for_profile(&profile.name, [0; 32], PlayerId(0)).unwrap(),
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
    for label in [
        "independent-catalog",
        "commander-foundations-v1",
        "commander-wipes-v1",
    ] {
        let mut prepared = Prepared::admit_pod(
            catalog.clone(),
            MatchConfig::for_profile("commander-wipes-v1", [0; 32], PlayerId(0)).unwrap(),
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

#[test]
fn profile_data_is_strict_and_every_default_pod_is_admitted() {
    for profile in MATCH_PROFILES.iter() {
        validate_pod(
            profile.catalog(),
            &MatchConfig::for_profile(&profile.name, [0; 32], PlayerId(0)).unwrap(),
        )
        .unwrap();
        let mut raw = serde_json::to_value(profile).unwrap();
        raw["commander_policy"]["eligibility"] = "any_card".into();
        assert!(strict_read::<MatchProfile>(&raw.to_string()).is_err());
        raw = serde_json::to_value(profile).unwrap();
        raw["commander_policy"]["color_identity"] = "ignore".into();
        assert!(strict_read::<MatchProfile>(&raw.to_string()).is_err());
        raw = serde_json::to_value(profile).unwrap();
        raw["unexpected"] = true.into();
        assert!(strict_read::<MatchProfile>(&raw.to_string()).is_err());
    }
}

#[test]
fn feature_requests_reject_unreviewed_mechanics_and_ignore_profile_names() {
    let wipes = match_profile("commander-wipes-v1").unwrap();
    for (flag, feature) in [("search", Feature::Search), ("hosted", Feature::Hosting)] {
        let mut raw = serde_json::to_value(wipes).unwrap();
        raw[flag] = true.into();
        raw["name"] = "unreviewed-probe".into();
        let err = MatchProfile::load(
            &raw.to_string(),
            &canonical(wipes.catalog()).unwrap(),
            &wipes.banned_list_raw,
        )
        .unwrap_err();
        assert!(
            matches!(err, ProfileError::UnreviewedCapabilities { feature: f, ref capabilities }
            if f == feature && capabilities.contains(&Capability::BoardWipesV1)),
            "{err:?}"
        );
    }
    for p in MATCH_PROFILES.iter() {
        p.validate_requested_features(p.catalog()).unwrap();
        if p.search || p.hosted {
            let mut raw = serde_json::to_value(p).unwrap();
            raw["name"] = "renamed-reviewed-probe".into();
            MatchProfile::load(
                &raw.to_string(),
                &canonical(p.catalog()).unwrap(),
                &p.banned_list_raw,
            )
            .unwrap();
        }
    }
}

#[test]
fn build_profile_loader_rejects_malformed_data_and_out_of_range_layouts() {
    let p = &MATCH_PROFILES[0];
    for (field, value) in [
        (
            "completion",
            serde_json::json!({"rotate": true, "homogeneous_seats": [0, 4]}),
        ),
        ("search", serde_json::json!("yes")),
        ("decks", serde_json::json!([])),
        ("unexpected", serde_json::json!(true)),
    ] {
        let mut raw = serde_json::to_value(p).unwrap();
        raw[field] = value;
        assert!(matches!(
            MatchProfile::load(
                &raw.to_string(),
                &canonical(p.catalog()).unwrap(),
                &p.banned_list_raw
            ),
            Err(ProfileError::InvalidProfile { .. })
        ));
    }
}

#[test]
fn generic_simultaneous_trigger_bound_fits_shared_stack_policy() {
    for p in MATCH_PROFILES.iter() {
        let c = p.catalog();
        // Nonbasic sources are singleton per seat. Basic lands and generated
        // token definitions have no triggers in this IR vocabulary.
        assert!(
            c.definitions
                .iter()
                .filter(|d| d.characteristics.basic)
                .all(|d| d.triggered.is_empty())
        );
        let sources = 4 * c
            .definitions
            .iter()
            .map(|d| d.triggered.len())
            .sum::<usize>();
        let simultaneous_creatures = 400 + capacities::TOKEN_CAPACITY as usize;
        // Leave over half the ceiling for existing stack entries, enters
        // triggers and terminal token-generation/death waves.
        assert!(
            simultaneous_creatures * sources < capacities::STACK_LIMIT / 2,
            "{}: {sources} trigger sources",
            p.name
        );
    }
}

#[test]
fn dated_policy_selection_binds_the_selected_file_and_its_metadata() {
    let p = &MATCH_PROFILES[0];
    let mut raw = serde_json::to_value(p).unwrap();
    raw["commander_policy"]["banned_list"] = "../commander-banned-synthetic-test.json".into();
    let mut list: serde_json::Value = strict_read(&p.banned_list_raw).unwrap();
    // Synthetic metadata and a subset of authentic records exercise selection,
    // without asserting a future real-world banned list.
    list["retrieved_date"] = "synthetic-test".into();
    let removed = list["named_bans"].as_array_mut().unwrap().remove(0);
    let id = removed["oracle_id"].as_str().unwrap();
    assert!(p.banned_commander_card(id).is_some());
    let selected = MatchProfile::load(
        &raw.to_string(),
        &canonical(p.catalog()).unwrap(),
        &list.to_string(),
    )
    .unwrap();
    assert!(selected.banned_commander_card(id).is_none());
    assert_ne!(selected.policy_digest(), p.policy_digest());
}

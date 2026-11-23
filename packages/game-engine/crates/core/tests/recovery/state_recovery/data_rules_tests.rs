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

/// Composition is the only lowering pipeline, so its marker capability names
/// no mechanic: a reviewed pod stays admitted when its definitions carry it,
/// and an unreviewed mechanic beside it is still rejected.
#[test]
fn composed_marker_is_reviewed_but_never_admits_a_mechanic() {
    for p in MATCH_PROFILES.iter().filter(|p| p.search || p.hosted) {
        let mut catalog = p.catalog().clone();
        for definition in &mut catalog.definitions {
            definition.composed = true;
        }
        assert!(
            catalog.definitions[0]
                .requirements()
                .contains(&Capability::ComposedAbilityV1)
        );
        p.validate_requested_features(&catalog).unwrap();
        let wipes = Catalog::read(include_str!(
            "../../../../../tests/fixtures/expected-phase1-r4-ir.json"
        ))
        .unwrap();
        catalog
            .definitions
            .extend(wipes.definitions.iter().cloned());
        assert!(matches!(
            p.validate_requested_features(&catalog),
            Err(ProfileError::UnreviewedCapabilities { ref capabilities, .. })
                if capabilities.contains(&Capability::BoardWipesV1)
                    && !capabilities.contains(&Capability::ComposedAbilityV1)
        ));
    }
}

#[test]
fn feature_requests_reject_unreviewed_mechanics_and_ignore_profile_names() {
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
        assert!(c.cast_trigger_fanout_bound(400).unwrap() < capacities::STACK_LIMIT / 2);
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

#[test]
fn flash_and_enters_tapped_are_not_implicitly_hosted_or_search_reviewed() {
    let profile = match_profile("commander-foundations-v1").unwrap();
    for enters_tapped in [false, true] {
        let mut catalog = profile.catalog().clone();
        catalog.schema_version = mtg_contracts::CURRENT_IR_VERSION;
        let d = catalog
            .definitions
            .iter_mut()
            .find(|d| d.characteristics.kind == CardKind::Creature)
            .unwrap();
        // This private capability probe has no printed source labels.
        d.source_labels = None;
        let capability = if enters_tapped {
            d.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped,
            });
            Capability::EntersTappedV1
        } else {
            d.keywords.push(KeywordAbility::Flash);
            d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
            Capability::FlashV1
        };
        d.capabilities = d.requirements();
        catalog.validate().unwrap();
        for (search, hosted, feature) in [
            (true, false, Feature::Search),
            (false, true, Feature::Hosting),
        ] {
            let mut probe = profile.clone();
            probe.search = search;
            probe.hosted = hosted;
            assert!(
                matches!(probe.validate_requested_features(&catalog), Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature && capabilities.contains(&capability))
            );
        }
    }
}

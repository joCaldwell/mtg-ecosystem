//! Dynamic admission witnesses; test-only, outside the engine build identity.
use super::*;

fn facts(types: &str, oracle_text: &str, keywords: &[&str]) -> BulkCardFacts {
    BulkCardFacts::from_source(&serde_json::json!({
        "name": "Fixture", "layout": "normal", "type_line": types, "oracle_text": oracle_text,
        "keywords": keywords, "color_identity": ["W"], "legalities": {"commander":"legal"},
    }))
    .unwrap()
}

#[test]
fn dynamic_pairing_requires_the_exact_pregame_keyword_and_doctor_types() {
    let creature = "Legendary Creature — Human";
    let partner = facts(
        creature,
        "Flying\nPartner (You can have two commanders if both have partner.)",
        &["Partner"],
    );
    let second = facts(creature, "Partner", &["Partner"]);
    assert_eq!(pairing(&partner, &second), Some(CommanderPairing::Partner));
    for text in [
        "Partner with Fixture",
        "Partner—Friends forever",
        "Choose a Background",
        "Create a token with partner.",
    ] {
        let variant = facts(creature, text, &["Partner"]);
        assert_eq!(pairing(&partner, &variant), None);
    }
    let companion = facts(
        creature,
        "Doctor's companion (You can have two commanders if the other is the Doctor.)",
        &["Doctor's companion"],
    );
    let doctor = facts("Legendary Creature — Time Lord Doctor", "Flying", &[]);
    assert_eq!(
        pairing(&companion, &doctor),
        Some(CommanderPairing::DoctorsCompanion)
    );
    assert_eq!(pairing(&doctor, &companion), pairing(&companion, &doctor));
    assert_eq!(pairing(&companion, &companion), None);
    let extra = facts("Legendary Creature — Time Lord Doctor Elf", "", &[]);
    let changeling = facts(
        "Legendary Creature — Time Lord Doctor",
        "Changeling",
        &["Changeling"],
    );
    let nonlegendary = facts("Creature — Time Lord Doctor", "", &[]);
    for invalid in [extra, changeling, nonlegendary, partner] {
        assert_eq!(pairing(&companion, &invalid), None);
    }
}

// Exact executable Foundations definitions isolate the physical rules.
// Pairing legality is covered separately by authenticated archive facts.
fn mixed_roster() -> DynamicPodTemplate<'static> {
    static ARTIFACT: LazyLock<BulkArtifact> = LazyLock::new(foundations_artifact);
    let profile = match_profile("commander-foundations-v1").unwrap();
    let config = MatchConfig::for_profile(&profile.name, [19; 32], PlayerId(0)).unwrap();
    let mut rows = DynamicMatchConfig::from_match_config(&config).decks;
    let primary = config.decks[0].commander.as_str();
    let second = profile
        .commanders
        .iter()
        .find(|id| id.as_str() != primary)
        .unwrap();
    let basic = rows[0].iter_mut().find(|row| row.quantity > 1).unwrap();
    basic.quantity -= 1;
    rows[0].push(DynamicDeckEntry {
        oracle_id: second.clone(),
        quantity: 1,
        role: DeckRole::Commander,
    });
    let decks = rows.each_ref().map(|r| NormalizedDeck::new(r));
    let mut projection = profile.catalog().clone();
    projection.definitions.retain(|d| {
        rows.iter()
            .flatten()
            .any(|row| row.oracle_id == d.oracle_id)
    });
    DynamicPodTemplate {
        artifact: &ARTIFACT,
        decks,
        projection: ValidatedCatalog::new(projection).unwrap(),
        bulk_manifest_sha256: "test-bulk".into(),
        policy_digest: policy().unwrap().digest.clone(),
    }
}

#[test]
fn dynamic_pair_runtime_preserves_split_ledger_union_and_mixed_opening_replay() {
    let template = mixed_roster();
    let pod = template.prepare(PlayerId(0), [19; 32], 0).unwrap();
    let same = template.prepare(PlayerId(0), [19; 32], 0).unwrap();
    assert_eq!(pod.identity, same.identity);
    assert!(Arc::ptr_eq(&pod.prepared.catalog, &same.prepared.catalog));
    assert!(pod.prepared.match_config().is_none());
    assert_eq!(
        pod.normalized_config().decks[0]
            .iter()
            .filter(|row| row.role == DeckRole::Commander)
            .count(),
        2
    );
    assert_eq!(
        pod.prepared.config.commanders,
        [0, 1, 100, 200, 300].map(CardId)
    );
    assert!(template.prepare(PlayerId(0), [19; 32], 4).is_err());
    let rotated = template.prepare(PlayerId(1), [19; 32], 1).unwrap();
    assert_eq!(
        rotated.prepared.config.commanders,
        [0, 100, 101, 200, 300].map(CardId)
    );
    assert_ne!(
        rotated.identity.admission_digest,
        pod.identity.admission_digest
    );

    let prepared = pod.prepared;
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    assert_eq!(game.opening_library_sizes, [98, 99, 99, 99]);
    assert_eq!(
        game.mana_colors(PlayerId(0), &ManaProduction::CommanderColor)
            .unwrap(),
        vec![Color::W, Color::B, Color::G]
    );
    assert!(matches!(
        game.observe(PlayerId(1), Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
    ));
    let mut ledger = game.commander_ledger().clone();
    let caster = PlayerId(0);
    let defender = PlayerId(1);
    ledger
        .record_cast(CardId(0), caster, Zone::Command { owner: caster })
        .unwrap();
    assert_eq!(ledger.tax(CardId(0), caster).unwrap(), Quantity::from(2));
    assert_eq!(ledger.tax(CardId(1), caster).unwrap(), Quantity::zero());
    ledger
        .record_damage(CardId(0), defender, true, &Quantity::from(11))
        .unwrap();
    ledger
        .record_damage(CardId(1), defender, true, &Quantity::from(10))
        .unwrap();
    assert!(ledger.combat_loss_candidates().is_empty());
    ledger
        .record_damage(CardId(0), defender, true, &Quantity::from(10))
        .unwrap();
    assert_eq!(ledger.combat_loss_candidates(), vec![defender]);

    assert!(matches!(
        game.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    let resumed = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(
        resumed.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    for _ in 0..64 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::new(20_000)) else {
            panic!("bounded opening should reach a decision");
        };
        if matches!(request, Decision {reason:Reason::Priority, player, .. } if player == caster)
            && game.turn().step == TurnStep::PrecombatMain
        {
            let QueryResult::Complete(QueryValue::Observation(observed)) =
                game.observe(PlayerId(1), Budget::unlimited())
            else {
                panic!("precombat observation");
            };
            let rows: Vec<_> = observed
                .view
                .commanders
                .iter()
                .filter(|c| c.history.owner == PlayerId(0))
                .collect();
            assert_eq!(rows.len(), 2);
            assert_ne!(rows[0].designation, rows[1].designation);
            let journal = game.replay_journal();
            assert!(journal.decisions.len() > 4);
            let replay = Game::replay_with_trace(prepared, &journal, TraceMode::Off).unwrap();
            assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
            return;
        }
        let response = match &request {
            Decision {
                player,
                reason: Reason::Mulligan {
                    mulligans_taken, ..
                },
                ..
            } => Answer::Pick {
                picks: vec![vec![u32::from(*player == caster && *mulligans_taken == 0)]],
            },
            Decision {
                reason: Reason::Priority,
                ..
            } => Answer::Pick {
                picks: vec![vec![0]],
            },
            other => panic!("unexpected opening decision {other:?}"),
        };
        game.respond(request.player, request.id, response).unwrap();
    }
    panic!("mixed opening did not finish");
}

#[test]
fn stage1d_dynamic_capacity_and_copy_gates_are_fail_closed() {
    let profile = match_profile("commander-foundations-v1").unwrap();
    let factor = PHYSICAL_SOURCES + TOKEN_CAPACITY as usize;
    let boundary = (STACK_LIMIT / 2).div_ceil(factor);
    assert!(capacity_supported(profile.catalog(), boundary - 1));
    assert!(!capacity_supported(profile.catalog(), boundary));
    assert!(!capacity_supported(profile.catalog(), usize::MAX));
    assert!(copy_capacity_unreviewed(
        Capability::EnterAsCopyOfPermanentEnteredThisTurnV1
    ));
    assert!(copy_capacity_unreviewed(
        Capability::ControlledCreatureSpellCopyV1
    ));
    assert!(match_profile(DYNAMIC_PROFILE).is_none());
}

#[test]
fn headless_pods_admit_every_executing_definition() {
    // The policy identity changed deliberately: no hand-listed capability
    // set is hashed into it any more, only the tier headless pods require.
    assert_eq!(
        policy().unwrap().digest,
        digest(&(ADMISSION_DOMAIN, POLICY_SHA256, HEADLESS_TIER)).unwrap()
    );
    assert_ne!(
        policy().unwrap().digest,
        digest(&(
            "commander-dynamic-pair-admission-v3",
            POLICY_SHA256,
            HEADLESS_TIER
        ))
        .unwrap()
    );
    // Every compiled definition executes, so inspection reports no
    // capability gap for any role; only copy capacity stays fenced.
    let catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-phase1-r5-ir.json"
    ))
    .unwrap();
    let names = BTreeMap::new();
    let mut below_headless_qualified = 0;
    for definition in &catalog.definitions {
        assert!(
            mtg_contracts::kir::definition_tiers(definition)
                .unwrap()
                .admits(HEADLESS_TIER)
        );
        if !mtg_contracts::kir::definition_tiers(definition)
            .unwrap()
            .admits(mtg_contracts::kir::ExecutionTier::HeadlessQualified)
        {
            below_headless_qualified += 1;
        }
        for role in [DeckRole::Main, DeckRole::Companion] {
            let mut issues = Vec::new();
            inspect_definition(definition, None, role, 1, &names, &mut issues);
            assert!(
                issues
                    .iter()
                    .all(|issue| issue.code == CardGapCode::CopyCapacityUnreviewed),
                "{}: {issues:?}",
                definition.name
            );
        }
    }
    assert!(
        below_headless_qualified > 0,
        "fixture must include definitions admitted only below analytics"
    );
}

fn foundations_rows() -> [Vec<DynamicDeckEntry>; 4] {
    let config =
        MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(0)).unwrap();
    DynamicMatchConfig::from_match_config(&config).decks
}

#[test]
fn foundations_decks_still_pass_definition_and_capacity_admission() {
    let profile = match_profile("commander-foundations-v1").unwrap();
    let catalog = profile.catalog();
    let rows = foundations_rows();
    for deck in &rows {
        let names = BTreeMap::new();
        let commander = deck.iter().find(|r| r.role == DeckRole::Commander).unwrap();
        let colors = catalog
            .definition_by_oracle_id(&commander.oracle_id)
            .unwrap()
            .characteristics
            .color_identity
            .clone();
        for row in deck {
            let definition = catalog.definition_by_oracle_id(&row.oracle_id).unwrap();
            let mut issues = Vec::new();
            inspect_definition(
                definition,
                Some(&colors),
                row.role,
                u64::from(row.quantity),
                &names,
                &mut issues,
            );
            assert!(issues.is_empty(), "{}: {issues:?}", definition.name);
        }
    }
    // The production projection accumulates trigger slots over all four
    // seats and proves the pod's capacity bound.
    let artifact = foundations_artifact();
    let template = DynamicPodTemplate::project(&artifact, &rows, policy().unwrap()).unwrap();
    let distinct: BTreeSet<_> = rows.iter().flatten().map(|row| &row.oracle_id).collect();
    assert_eq!(
        template.catalog().catalog().definitions.len(),
        distinct.len()
    );
}

/// A verified bulk artifact holding exactly the reviewed Foundations
/// definitions, built as `catalog build-ir` publishes one: index rows carry
/// source-derived facts and every payload is bound by its byte proof.
fn foundations_artifact() -> BulkArtifact {
    use mtg_contracts::{
        JsonByteProof, accepted_archives,
        bulk_catalog::{BULK_FORMAT_VERSION, BulkIndex, BulkManifest},
        canonical,
    };
    let catalog = match_profile("commander-foundations-v1")
        .unwrap()
        .catalog()
        .clone();
    let proof = |bytes: &str| JsonByteProof {
        bytes: bytes.len() as u64,
        sha256: sha256(bytes.as_bytes()),
    };
    let rows = catalog
        .definitions
        .iter()
        .enumerate()
        .map(|(line, d)| {
            let p = &d.provenance;
            let source: serde_json::Value = serde_json::from_str(&p.source_json).unwrap();
            BulkIndexRow {
                oracle_id: d.oracle_id.clone(),
                facts: BulkCardFacts::from_source(&source).unwrap(),
                archive_line: line + 2,
                raw_row_sha256: p.source_sha256.clone(),
                record_sha256: p.source_sha256.clone(),
                source_index: p.source_index,
                source_sha256: p.source_sha256.clone(),
                ast_sha256: Some(p.ast_sha256.clone()),
                binding_sha256: Some(p.binding_sha256.clone()),
                status: BulkStatus::LoweredValidated,
                reason_code: None,
            }
        })
        .collect::<Vec<_>>();
    let first = &catalog.definitions[0].provenance;
    let archive = accepted_archives()
        .unwrap()
        .iter()
        .find(|a| a.manifest_sha256 == first.archive_manifest_sha256)
        .unwrap();
    let catalog_bytes = canonical(&catalog).unwrap();
    let index = BulkIndex { rows };
    let index_bytes = canonical(&index).unwrap();
    let producer = "b".repeat(64);
    let manifest = BulkManifest {
        format_version: BULK_FORMAT_VERSION,
        ir_version: CURRENT_IR_VERSION,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        producer_build_id: producer.clone(),
        archive_sha256: archive.file_sha256.clone().unwrap(),
        archive_manifest_sha256: archive.manifest_sha256.clone(),
        parser_revision: archive.parser_revision.clone(),
        parser_source_sha256: archive.parser_source_sha256.clone(),
        catalog: proof(&catalog_bytes),
        index: proof(&index_bytes),
        catalog_digest: sha256(catalog_bytes.as_bytes()),
        outcome_counts: BTreeMap::from([(BulkStatus::LoweredValidated, index.rows.len())]),
    };
    let manifest_bytes = canonical(&manifest).unwrap();
    let manifest_proof = proof(&manifest_bytes);
    let pin = manifest_proof.sha256.clone();
    BulkArtifact::verify(
        (manifest, manifest_proof),
        &pin,
        &producer,
        (catalog, proof(&catalog_bytes)),
        (index, proof(&index_bytes)),
    )
    .unwrap()
}

/// Ported from the retired `dynamic-witness`: through full production
/// admission (deck inspection, gaps, projection and capacity), deck row
/// order changes neither the admission identity nor the opening.
#[test]
fn deck_row_order_never_changes_admission_or_the_opening() {
    let artifact = foundations_artifact();
    let rows = foundations_rows();
    let mut reversed = rows.clone();
    for deck in &mut reversed {
        deck.reverse();
    }
    assert_ne!(rows, reversed);
    let opening = |rows: &[Vec<DynamicDeckEntry>; 4]| {
        let template = DynamicPodTemplate::admit(&artifact, rows).unwrap();
        let pod = template.prepare(PlayerId(0), [0; 32], 0).unwrap();
        let identity = pod.identity.clone();
        let normalized = pod.normalized_config().clone();
        let mut game = Game::with_trace(pod.prepared, TraceMode::Off);
        for _ in 0..100 {
            let Advance::AwaitingDecision(request) = game.advance(Budget::new(100_000)) else {
                panic!("opening must reach an ordinary decision");
            };
            if matches!(request, Decision {reason:Reason::Priority, player, .. } if player == PlayerId(0))
                && game.turn().step == TurnStep::PrecombatMain
            {
                return (
                    identity,
                    normalized,
                    game.replay_journal(),
                    game.state_digest().unwrap(),
                );
            }
            let response = match &request {
                Decision {
                    reason: Reason::Mulligan { .. },
                    ..
                } => Answer::Pick {
                    picks: vec![vec![0]],
                },
                Decision {
                    reason: Reason::Priority,
                    ..
                } => Answer::Pick {
                    picks: vec![vec![0]],
                },
                other => panic!("unexpected opening decision {other:?}"),
            };
            game.respond(request.player, request.id, response).unwrap();
        }
        panic!("opening exceeded its decision bound")
    };
    let (first, second) = (opening(&rows), opening(&reversed));
    assert!(!first.2.decisions.is_empty());
    assert_eq!(first, second);
    // The same admission path still rejects a gap: an unknown Oracle row.
    let mut gap = rows.clone();
    gap[3][1].oracle_id = "00000000-0000-0000-0000-000000000000".into();
    assert!(matches!(
        DynamicPodTemplate::admit(&artifact, &gap),
        Err(DynamicAdmissionError::DeckGaps { .. })
    ));
}

#[test]
fn stage1d_foundations_projection_earns_the_headless_qualified_tier() {
    let profile = match_profile("commander-foundations-v1").unwrap();
    assert_eq!(profile.policy_digest(), POLICY_SHA256);
    assert!(policy().is_ok());
    let ids: BTreeSet<_> = profile
        .decks
        .iter()
        .flat_map(|d| d.cards.iter().map(|row| &row.oracle_id))
        .collect();
    // The parity corpus ran these decks: every definition they use earns
    // the tier analytics and paired runs require.
    for id in ids {
        let definition = profile.catalog().definition_by_oracle_id(id).unwrap();
        assert!(
            mtg_contracts::kir::definition_tiers(definition)
                .unwrap()
                .admits(mtg_contracts::kir::ExecutionTier::HeadlessQualified),
            "{}",
            definition.name
        );
    }
}

#[test]
fn one_card_substitution_preserves_opening_orders_and_shuffle_streams_over_64_seeds() {
    let artifact = foundations_artifact();
    let rows = foundations_rows();
    let base = DynamicPodTemplate::admit(&artifact, &rows).unwrap();
    let remove = &rows[0]
        .iter()
        .find(|r| r.role == DeckRole::Main && r.quantity == 1)
        .unwrap()
        .oracle_id;
    let add = &rows[0].iter().find(|r| r.quantity > 1).unwrap().oracle_id;
    let prime = base.substitute(0, remove, add).unwrap();
    for index in 0u8..64 {
        let seed = [index; 32];
        let rotation = index as usize % 4;
        let starter = PlayerId(index / 4 % 4);
        let [a, b] = [&base, &prime].map(|template| {
            let pod = template.prepare(starter, seed, rotation).unwrap();
            let mut g = Game::with_trace(pod.prepared, TraceMode::Off);
            // 98 accepted Fisher–Yates trials per library. Stop before
            // the first draw even when one or more words were rejected.
            g.advance(Budget::new(4 * 98));
            while matches!(
                g.state
                    .rules
                    .match_state
                    .as_ref()
                    .unwrap()
                    .setup
                    .as_ref()
                    .unwrap()
                    .phase,
                SetupPhase::Shuffle { .. }
            ) {
                assert!(matches!(
                    g.advance(Budget::new(1)),
                    Advance::Suspended { .. }
                ));
            }
            g
        });
        let mut changes = 0;
        for seat in 0..4 {
            let seat = PlayerId(seat);
            let left = &a.state.rules.libraries[&seat];
            let right = &b.state.rules.libraries[&seat];
            assert_eq!(left.len(), 99);
            assert_eq!(left, right);
            for id in left {
                let old = &a.state.rules.cards[id].oracle_id;
                let new = &b.state.rules.cards[id].oracle_id;
                if old != new {
                    assert_eq!(seat.0 as usize, rotation);
                    assert_eq!((old, new), (remove, add));
                    changes += 1;
                }
            }
            let state = |g: &Game| g.state.rules.match_state.as_ref().unwrap().clone();
            let (mut x, mut y) = (state(&a), state(&b));
            for _ in 0..3 {
                // Consume unrelated library and seat-local effect words in
                // only one arm before comparing the next complete shuffle.
                for domain in [1, 2] {
                    let other = PlayerId((seat.0 + 1) % 4);
                    let stream =
                        setup::stream_id(domain, other, y.library_shuffles[seat.0 as usize])
                            .unwrap();
                    for _ in 0..17 {
                        y.random.word(stream).unwrap();
                    }
                }
                let streams = [&mut x, &mut y].map(|s| {
                    let ordinal = &mut s.library_shuffles[seat.0 as usize];
                    let stream = setup::stream_id(1, seat, *ordinal).unwrap();
                    *ordinal += 1;
                    (0..98)
                        .map(|_| s.random.word(stream).unwrap())
                        .collect::<Vec<_>>()
                });
                assert_eq!(streams[0], streams[1]);
            }
        }
        assert_eq!(changes, 1);
    }
}

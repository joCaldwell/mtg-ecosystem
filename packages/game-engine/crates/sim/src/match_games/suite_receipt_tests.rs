//! Synthetic receipt-domain fixtures only. These are deliberately unreplayed
//! envelopes, not completed games or printed card facts. Test-only receipts
//! isolate byte/slot/aggregate guards; standalone verification must reject them.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const PROFILE: &str = "commander-foundations-v1";
const BINDING_ERROR: &str = "fixed completion batch receipt index/profile/bytes mismatch";
const AGGREGATE_ERROR: &str =
    "fixed completion evidence lacks Commander-damage or ordinary-life loss witness";

struct Fixture {
    catalog: Catalog,
    directory: std::path::PathBuf,
    receipts: VerifiedBatch,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "mtg-suite-receipts-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let mut entries = vec![];
        for index in 0..18 {
            let text = canonical(&synthetic_evidence(index)).unwrap();
            fs::write(directory.join(format!("game-{index:02}.json")), &text).unwrap();
            entries.push(VerifiedBatchEntry {
                index,
                evidence_sha256: sha256(text.as_bytes()),
                // Synthetic loss facts exercise both aggregate requirements even
                // though off-mode envelopes themselves retain no typed losses.
                loss_facts: if index == 1 {
                    vec![
                        (PlayerId(1), mtg_core::PlayerLossReason::CommanderDamage),
                        (PlayerId(2), mtg_core::PlayerLossReason::Life),
                    ]
                } else {
                    vec![]
                },
            });
        }
        let receipts = VerifiedBatch {
            catalog_sha256: mtg_contracts::digest(&catalog).unwrap(),
            profile: PROFILE.into(),
            entries,
        };
        Self {
            catalog,
            directory,
            receipts,
        }
    }
    fn path(&self, index: usize) -> std::path::PathBuf {
        self.directory.join(format!("game-{index:02}.json"))
    }
    fn read(&self) -> Result<Vec<Evidence>, String> {
        read_verified_suite(&self.catalog, PROFILE, &self.directory, &self.receipts)
    }
    fn error(&self) -> String {
        self.read().err().expect("synthetic mutation must reject")
    }
    fn replace(&mut self, index: usize, e: &Evidence) {
        let text = canonical(e).unwrap();
        fs::write(self.path(index), &text).unwrap();
        // Test-only rebinding ensures the cheap guard is independently exercised.
        self.receipts.entries[index].evidence_sha256 = sha256(text.as_bytes());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn synthetic_evidence(index: usize) -> Evidence {
    let profile = mtg_core::match_profile(PROFILE).unwrap();
    let mut config = MatchConfig::for_profile(
        PROFILE,
        seed_for(PROFILE, index as u32),
        PlayerId((index % 4) as u8),
    )
    .unwrap();
    apply_completion_layout(profile, index, &mut config);
    Evidence {
        config,
        policy: Policy::Develop,
        mulligans: std::array::from_fn(|p| ((index + p) % 3) as u8),
        journal: ReplayJournal {
            decisions: vec![],
            concessions: vec![],
        },
        trace_mode: if index == 0 {
            TraceMode::Detailed
        } else {
            TraceMode::Off
        },
        losses: vec![],
        checkpoints: if index == 0 {
            (0..4)
                .map(|_| Checkpoint {
                    decisions: 0,
                    rule_boundary: 0,
                    snapshot: "synthetic invalid snapshot, never a runtime proof".into(),
                })
                .collect()
        } else {
            vec![]
        },
        outcome: "finished".into(),
        winner: Some(PlayerId(0)),
        decisions: 0,
        advances: 0,
        rule_steps: 0,
        query_calls: 0,
        work_limit: WORK_LIMIT,
        quantum: 1024,
        turns: 0,
        final_state_sha256: format!("synthetic unreplayed slot {index}"),
        surviving_seats: vec![PlayerId(0)],
        survivor_counts: vec![4, 1],
        elapsed_millis: 0,
    }
}

#[test]
fn receipt_domain_reads_all_ordered_envelopes_and_off_mode_loss_facts() {
    let fixture = Fixture::new();
    let evidence = fixture.read().unwrap();
    assert_eq!(evidence.len(), 18);
    for (index, e) in evidence.iter().enumerate() {
        verify_suite_entry(index, e).unwrap();
        assert!(e.losses.is_empty());
        assert_eq!(e.config.seed, seed_for(PROFILE, index as u32));
    }
}

#[test]
fn receipt_domain_rejects_changed_bytes_even_with_identical_decoded_evidence() {
    let fixture = Fixture::new();
    let mut text = fs::read_to_string(fixture.path(1)).unwrap();
    text.push('\n');
    let decoded: Evidence = strict_read(&text).unwrap();
    verify_suite_entry(1, &decoded).unwrap();
    fs::write(fixture.path(1), text).unwrap();
    assert_eq!(fixture.error(), BINDING_ERROR);
}

#[test]
fn receipt_domain_rejects_changed_terminal_metadata() {
    let fixture = Fixture::new();
    let mut e = synthetic_evidence(1);
    e.winner = Some(PlayerId(3));
    verify_suite_entry(1, &e).unwrap();
    fs::write(fixture.path(1), canonical(&e).unwrap()).unwrap();
    assert_eq!(fixture.error(), BINDING_ERROR);
}

#[test]
fn receipt_domain_rejects_receipt_order_and_index_changes() {
    let mut fixture = Fixture::new();
    fixture.receipts.entries.swap(1, 2);
    assert_eq!(fixture.error(), BINDING_ERROR);
    fixture.receipts.entries.swap(1, 2);
    fixture.receipts.entries[1].index = 2;
    assert_eq!(fixture.error(), BINDING_ERROR);
}

#[test]
fn receipt_domain_rejects_saved_file_order_changes() {
    let fixture = Fixture::new();
    let one = fs::read(fixture.path(1)).unwrap();
    let two = fs::read(fixture.path(2)).unwrap();
    fs::write(fixture.path(1), two).unwrap();
    fs::write(fixture.path(2), one).unwrap();
    assert_eq!(fixture.error(), "fixed completion batch manifest mismatch");
}

#[test]
fn receipt_domain_requires_exactly_eighteen_entries_and_every_saved_file() {
    let mut fixture = Fixture::new();
    let last = fixture.receipts.entries.pop().unwrap();
    assert_eq!(
        fixture.error(),
        "fixed completion batch must contain18 games"
    );
    fixture.receipts.entries.push(last);
    fixture.receipts.entries.push(VerifiedBatchEntry {
        index: 18,
        evidence_sha256: "synthetic extra entry".into(),
        loss_facts: vec![],
    });
    assert_eq!(
        fixture.error(),
        "fixed completion batch must contain18 games"
    );
    fixture.receipts.entries.pop().unwrap();
    fs::remove_file(fixture.path(17)).unwrap();
    assert!(fixture.read().is_err());
}

#[test]
fn receipt_domain_binds_catalog_and_profile() {
    let mut fixture = Fixture::new();
    fixture.catalog.definitions.pop().unwrap();
    assert_eq!(
        fixture.error(),
        "fixed completion batch receipt catalog/profile mismatch"
    );
    fixture.receipts.catalog_sha256 = mtg_contracts::digest(&fixture.catalog).unwrap();
    fixture.receipts.profile = "synthetic different profile".into();
    assert_eq!(
        fixture.error(),
        "fixed completion batch receipt catalog/profile mismatch"
    );
}

#[test]
fn receipt_domain_binds_each_evidence_profile_even_when_slot_bytes_match() {
    let mut fixture = Fixture::new();
    let mut e = synthetic_evidence(1);
    let other = mtg_core::match_profile("commander-data-admission-v1").unwrap();
    e.config =
        MatchConfig::for_profile(&other.name, seed_for(&other.name, 1), PlayerId(1)).unwrap();
    apply_completion_layout(other, 1, &mut e.config);
    verify_suite_entry(1, &e).unwrap();
    fixture.replace(1, &e);
    assert_eq!(fixture.error(), BINDING_ERROR);
}

#[test]
fn receipt_domain_still_strict_decodes_saved_evidence() {
    let mut fixture = Fixture::new();
    let mut value = serde_json::to_value(synthetic_evidence(1)).unwrap();
    value["synthetic_unknown_field"] = serde_json::json!(true);
    let text = canonical(&value).unwrap();
    fs::write(fixture.path(1), &text).unwrap();
    fixture.receipts.entries[1].evidence_sha256 = sha256(text.as_bytes());
    assert!(fixture.error().contains("unknown field"));
}

#[test]
fn receipt_domain_preserves_decode_failure_before_earlier_entry_guards() {
    let mut fixture = Fixture::new();
    let mut e = synthetic_evidence(1);
    e.quantum = 1;
    fixture.replace(1, &e);
    fs::write(fixture.path(17), "synthetic malformed JSON").unwrap();
    let expected = strict_read::<Evidence>("synthetic malformed JSON")
        .err()
        .unwrap();
    assert_eq!(fixture.error(), expected);
}

#[test]
fn receipt_domain_repeats_manifest_checkpoint_and_host_guards() {
    let mut fixture = Fixture::new();
    let mut e = synthetic_evidence(1);
    e.quantum = 1;
    fixture.replace(1, &e);
    assert_eq!(fixture.error(), "fixed completion batch manifest mismatch");
    let mut e = synthetic_evidence(1);
    e.checkpoints.push(Checkpoint {
        decisions: 0,
        rule_boundary: 0,
        snapshot: "synthetic unselected checkpoint".into(),
    });
    fixture.replace(1, &e);
    assert_eq!(fixture.error(), "selected checkpoint closure mismatch");
    let mut e = synthetic_evidence(1);
    e.query_calls = WORK_LIMIT + 1;
    fixture.replace(1, &e);
    assert_eq!(
        fixture.error(),
        "match evidence exceeds published host ceilings"
    );
}

#[test]
fn receipt_domain_requires_both_aggregate_loss_reasons() {
    for missing in [
        mtg_core::PlayerLossReason::CommanderDamage,
        mtg_core::PlayerLossReason::Life,
    ] {
        let mut fixture = Fixture::new();
        for entry in &mut fixture.receipts.entries {
            entry.loss_facts.retain(|(_, reason)| *reason != missing);
        }
        assert_eq!(fixture.error(), AGGREGATE_ERROR);
    }
}

#[test]
fn standalone_suite_still_rejects_unreplayed_synthetic_receipt_envelopes() {
    let fixture = Fixture::new();
    let mut evidence = fixture.read().unwrap();
    let replay_error = verify_replay(&fixture.catalog, &evidence[0]).err().unwrap();
    assert_eq!(
        verify_suite(&fixture.catalog, &evidence).err().unwrap(),
        replay_error
    );
    evidence[0].quantum = 1;
    assert_eq!(
        verify_suite(&fixture.catalog, &evidence).err().unwrap(),
        "fixed completion batch manifest mismatch"
    );
    evidence.pop().unwrap();
    assert_eq!(
        verify_suite(&fixture.catalog, &evidence).err().unwrap(),
        "fixed completion batch must contain18 games"
    );
}

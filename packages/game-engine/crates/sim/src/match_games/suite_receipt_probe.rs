//! Opt-in production-issued receipt check; never substitutes for acceptance.
use super::*;

// Exact pre-receipt suite assembly: read every saved envelope, then independently
// replay the complete fixed suite. The batch itself is shared by both paths.
fn legacy_assembly(catalog: &Catalog, directory: &Path) -> Result<Vec<Evidence>, String> {
    let evidence = (0..18)
        .map(|index| {
            fs::read_to_string(directory.join(format!("game-{index:02}.json")))
                .map_err(|error| error.to_string())
                .and_then(|text| strict_read(&text))
        })
        .collect::<Result<Vec<Evidence>, String>>()?;
    verify_suite(catalog, &evidence)?;
    Ok(evidence)
}

#[test]
#[ignore = "production batch plus paired suite assembly; requires fresh MTG_SUITE_RECEIPT_PROBE_OUTPUT"]
fn production_receipts_match_legacy_suite_assembly() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("MTG_SUITE_RECEIPT_PROBE_OUTPUT")
            .expect("MTG_SUITE_RECEIPT_PROBE_OUTPUT must name a fresh directory"),
    );
    assert!(directory.is_absolute());
    // Creation is exclusive; failures and every generated journal stay retained.
    fs::create_dir(&directory).unwrap();
    let batch_directory = directory.join("batch");
    let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
    let profile = "commander-foundations-v1";
    let started = Instant::now();
    // Receipts can only come from actual construction, full independent replay,
    // interaction scanning and successful persistence in production batch_impl.
    let (report, verified) = batch_impl(&catalog, profile, &batch_directory).unwrap();
    let batch_wall_ns = started.elapsed().as_nanos();
    assert_eq!(verified.entries.len(), 18);
    assert_eq!(report["passed"], true);
    assert_eq!(report["failures"], serde_json::json!([]));
    let games = report["games"].as_array().unwrap();
    assert_eq!(games.len(), 18);
    for (index, receipt) in verified.entries.iter().enumerate() {
        assert_eq!(receipt.index, index);
        let bytes = fs::read(batch_directory.join(format!("game-{index:02}.json"))).unwrap();
        assert_eq!(receipt.evidence_sha256, sha256(&bytes));
        assert_eq!(
            serde_json::to_value(&receipt.loss_facts).unwrap(),
            games[index]["verified_losses"]
        );
        assert_eq!(games[index]["outcome"], "finished");
        assert!(!games[index]["winner"].is_null());
    }
    let started = Instant::now();
    let legacy = legacy_assembly(&catalog, &batch_directory).unwrap();
    let legacy_assembly_wall_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let reused = read_verified_suite(&catalog, profile, &batch_directory, &verified).unwrap();
    let receipt_assembly_wall_ns = started.elapsed().as_nanos();
    // Complete bytes, including snapshots, journals, outcomes and all counters.
    let legacy_bytes = canonical(&legacy).unwrap();
    let reused_bytes = canonical(&reused).unwrap();
    assert_eq!(legacy_bytes, reused_bytes);
    let disk_report = fs::read_to_string(batch_directory.join("report.json")).unwrap();
    assert_eq!(disk_report, canonical(&report).unwrap());
    fs::write(directory.join("legacy-evidence.json"), &legacy_bytes).unwrap();
    fs::write(directory.join("receipt-evidence.json"), &reused_bytes).unwrap();
    let result = serde_json::json!({
        "scope":"one production foundations batch and one old-then-new suite-assembly pair; shared batch excluded from assembly comparison; not full acceptance",
        "build_id":BUILD_ID,
        "profile":profile,
        "games":18,
        "receipt_source":"production batch_impl; no fabricated or deserialized receipt",
        "batch_wall_ns":batch_wall_ns,
        "legacy_assembly_wall_ns":legacy_assembly_wall_ns,
        "receipt_assembly_wall_ns":receipt_assembly_wall_ns,
        "complete_evidence_bytes_equal":true,
        "evidence_sha256":sha256(legacy_bytes.as_bytes()),
        "persisted_report_bytes_equal":true,
        "report_sha256":sha256(disk_report.as_bytes()),
        "receipt_loss_facts_equal_persisted_report":true,
        "independent_legacy_verify_suite_passed":true,
    });
    fs::write(directory.join("result.json"), canonical(&result).unwrap()).unwrap();
    println!("MTG_SUITE_RECEIPT_PROBE={}", canonical(&result).unwrap());
}

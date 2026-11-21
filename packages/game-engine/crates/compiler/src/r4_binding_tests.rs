//! Exact frozen R4 records exercise both refreshed operand graphs and row pins.
use super::*;
use serde_json::json;

const R4: &str = include_str!("../../../tests/fixtures/archive-manifest-phase1-r4.json");

#[test]
fn refreshed_binding_graphs_consume_exact_semantics() {
    for (raw, capability, node, key, bad) in [
        (
            include_str!("../../../tests/fixtures/phase1-r4/bond-land.json"),
            Capability::ConditionalOpponentCountEntryV1,
            "player-state-condition",
            "who",
            "another-controller",
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/distinct-color-mana.json"),
            Capability::DistinctPermanentColorsV1,
            "characteristic-subject",
            "of",
            "another-object",
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/fixed-opponent-drain.json"),
            Capability::OpponentLifeDrainV1,
            "event-aggregate",
            "event",
            "another-event",
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/announced-x-opponent-drain.json"),
            Capability::OpponentLifeDrainV1,
            "event-aggregate",
            "measure",
            "life-gained",
        ),
    ] {
        for header in [R4] {
            let result = compile(header, raw);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            assert!(definition.capabilities.contains(&capability));
        }
        let forged = tests::changed_source(raw, |row| {
            let changed = row["binding"]["nodes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|n| n["node"] == node)
                .unwrap();
            changed[key] = json!(bad);
        });
        assert!(matches!(
            compile(R4, &forged),
            CompileResult::Unsupported { path, .. } if path.starts_with("binding")
        ));
    }
}

#[test]
fn refreshed_exact_rows_use_their_own_archived_provenance() {
    for (raw, capability) in [
        (
            include_str!("../../../tests/fixtures/phase1-r4/library-top-mana.json"),
            Capability::LookTopMayRevealForManaV1,
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/graveyard-replacement.json"),
            Capability::SelfGraveyardRevealShuffleV1,
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/entry-copy.json"),
            Capability::EnterAsCopyOfPermanentEnteredThisTurnV1,
        ),
    ] {
        for header in [R4] {
            let result = compile(header, raw);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            assert_eq!(
                definition.provenance.archive_manifest_sha256,
                accepted_manifest(header).unwrap().manifest_sha256
            );
            assert!(definition.capabilities.contains(&capability));
            let forged = format!("{{ {}", raw.strip_prefix('{').unwrap());
            assert!(matches!(
                compile(header, &forged),
                CompileResult::Unsupported { path, .. } | CompileResult::Invalid { path, .. }
                    if path == "record"
            ));
        }
    }
}

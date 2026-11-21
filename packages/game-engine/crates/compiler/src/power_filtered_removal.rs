//! CR 601.2c and 608.2b. The power comparison is part of the target restriction.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const SELECTION: &str = "line:0:1";

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<(TargetSpec, Instruction)>, Failure> {
    let printed = source["oracle_text"].as_str().unwrap_or_default();
    let (operation, threshold_text, instruction) =
        if let Some(text) = printed.strip_prefix("Destroy target creature with power ") {
            (
                "destroy",
                text,
                Instruction::DestroyPermanent {
                    source_block: 0,
                    target_slot: 0,
                    optional: false,
                    regeneration_forbidden: false,
                },
            )
        } else if let Some(text) = printed.strip_prefix("Exile target creature with power ") {
            (
                "exile",
                text,
                Instruction::ExilePermanent {
                    source_block: 0,
                    target_slot: 0,
                },
            )
        } else {
            return Ok(None);
        };
    let fail = || {
        unsupported(
            "power_filtered_removal",
            "requires one ordinary removal spell with one fixed creature power comparison",
        )
    };
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let number = &raw[0]["effects"][0]["steps"][0]["what"]["filter"]["properties"][0]["comparison"]
        ["value"]["value"];
    if !(number.is_i64() || number.is_u64()) {
        return Err(fail());
    }
    let value: Quantity = number.to_string().parse().map_err(|_| fail())?;
    let (op, power) = if threshold_text == format!("{value} or greater.") {
        ("ge", CreaturePowerFilter::AtLeast { value })
    } else if threshold_text == format!("{value} or less.") {
        ("le", CreaturePowerFilter::AtMost { value })
    } else {
        return Err(fail());
    };
    let filter = json!({"types":["creature"],"properties":[{"property":"power",
        "comparison":{"op":op,"value":{"amount":"fixed","value":number}}}]});
    let what = json!({"ref":"target","filter":filter,"count":{"amount":"fixed","value":1},"selectionId":SELECTION});
    if !matches!(source["type_line"].as_str(), Some("Instant" | "Sorcery"))
        || source["keywords"] != json!([])
        || source["mana_cost"]
            .as_str()
            .is_none_or(|cost| cost.is_empty() || cost.contains("{X}"))
        || raw != json!([{"kind":"spell","effects":[{"steps":[{"effect":operation,"what":what}]}]}])
    {
        return Err(fail());
    }

    let base = "/faces/0/abilities/0";
    let step = "/faces/0/abilities/0/effects/0/steps/0";
    let mut graph = Bindings::new(source["oracle_id"].as_str().ok_or_else(fail)?, source);
    graph.ability(base, "spell", "spell-controller");
    let target = graph.target(
        &graph.id(base, "ability"),
        &format!("{step}/what"),
        &what,
        SELECTION,
    );
    graph.event(base, step, operation, &target, false);
    if operation == "exile" {
        let mut description = filter;
        description["cls"] = json!("card");
        graph.node(step, "objects", "event-result", json!({
            "event":graph.id(step,"event"),"result":"objects","cardinality":"one","availability":"if-produced",
            "identity":"zone-successors","description":description,"rules":["400.7j"]
        }));
    }
    graph.validate_ordered(
        binding,
        "requires the complete power-filtered target and removal binding",
    )?;
    Ok(Some((
        TargetSpec::new(0, SELECTION, TargetDomain::BattlefieldPermanent).with_filter(
            PermanentFilter::any()
                .with_types([CardKind::Creature])
                .with_power(power),
        ),
        instruction,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;
    use std::io::{BufRead, BufReader};

    const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");

    fn changed(raw: &str, mutate: impl FnOnce(&mut Value)) -> String {
        changed_source(raw, |r| {
            mutate(r);
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        })
    }

    fn synthetic(exile: bool) -> String {
        changed(include_str!("../../../tests/fixtures/murder.json"), |r| {
            let (op, cmp, text) = if exile {
                ("exile", "le", "Exile target creature with power 7 or less.")
            } else {
                (
                    "destroy",
                    "ge",
                    "Destroy target creature with power 7 or greater.",
                )
            };
            r["source"]["oracle_text"] = json!(text);
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["text"] = json!(text);
            let step = &mut result["abilities"][0]["effects"][0]["steps"][0];
            step["effect"] = json!(op);
            step["what"]["filter"]["properties"] = json!([{"property":"power","comparison":{"op":cmp,"value":{"amount":"fixed","value":7}}}]);
            let what = step["what"].clone();
            let graph = &mut r["binding"];
            // The retained Murder fixture predates the archive's explicit binding flags.
            graph["referencesBound"] = json!(true);
            graph["engineFeatures"] = json!([]);
            graph["references"][0]["expression"] = what.clone();
            graph["nodes"][4]["expression"] = what.clone();
            graph["nodes"][5]["operation"] = json!(op);
            if exile {
                let event = &graph["nodes"][5];
                let mut description = what["filter"].clone();
                description["cls"] = json!("card");
                let result = json!({"id":event["id"].as_str().unwrap().replace("#event","#objects"),
                    "path":event["path"],"node":"event-result","event":event["id"],"result":"objects",
                    "cardinality":"one","availability":"if-produced","identity":"zone-successors",
                    "description":description,"rules":["400.7j"]});
                graph["nodes"].as_array_mut().unwrap().push(result);
            }
        })
    }

    fn check(
        raw: &str,
        kind: CardKind,
        exile: bool,
        at_least: bool,
        value: i64,
    ) -> Box<Definition> {
        let definition = match compile(HEADER, raw) {
            CompileResult::Lowered { definition } => definition,
            result => panic!("{result:?}"),
        };
        assert_eq!(definition.characteristics.kind, kind);
        use Capability::*;
        assert_eq!(
            definition.capabilities,
            BTreeSet::from([
                NormalLayoutV1,
                if kind == CardKind::Instant {
                    CastInstantV1
                } else {
                    CastSorceryV1
                },
                PermanentRemovalV1,
                PowerFilteredRemovalV1,
            ])
        );
        let p = definition.spell.as_ref().unwrap();
        let power = if at_least {
            CreaturePowerFilter::AtLeast {
                value: value.into(),
            }
        } else {
            CreaturePowerFilter::AtMost {
                value: value.into(),
            }
        };
        assert_eq!(
            p.targets,
            [TargetSpec {
                slot: 0,
                selection_id: "line:0:1".into(),
                stage: EvaluationStage::Announcement,
                chooser: Chooser::Controller,
                domain: TargetDomain::BattlefieldPermanent,
                filter: Some(PermanentFilter {
                    types: vec![CardKind::Creature],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                    power: Some(power)
                })
            }]
        );
        let instruction = if exile {
            Instruction::ExilePermanent {
                target_slot: 0,
                source_block: 0,
            }
        } else {
            Instruction::DestroyPermanent {
                target_slot: 0,
                source_block: 0,
                optional: false,
                regeneration_forbidden: false,
            }
        };
        assert_eq!(p.instructions, [instruction]);
        definition
    }

    #[test]
    fn power_filtered_removal_source_binding_and_archive() {
        for exile in [false, true] {
            let raw = synthetic(exile);
            check(&raw, CardKind::Instant, exile, !exile, 7);
            let edits: &[fn(&mut Value)] = &[
                |r| {
                    r["binding"]["references"][0]["target"] = r["binding"]["nodes"][0]["id"].clone()
                },
                |r| r["binding"]["nodes"][5]["inputs"] = json!([]),
                |r| r["binding"]["nodes"][4]["expression"]["filter"]["properties"] = json!([]),
                |r| {
                    r["faces"][0]["result"]["abilities"][0]["effects"][0]["condition"] = Value::Null
                },
                |r| r["source"]["mana_cost"] = json!("{X}{B}"),
                |r| r["source"]["keywords"] = json!(["Flashback"]),
                |r| {
                    let text = "Exile target creature with power 8 or less.";
                    r["source"]["oracle_text"] = json!(text);
                    r["faces"][0]["result"]["lines"][0]["text"] = json!(text);
                },
            ];
            for (i, edit) in edits.iter().enumerate() {
                let bad = changed(&raw, edit);
                assert!(
                    !matches!(compile(HEADER, &bad), CompileResult::Lowered { .. }),
                    "mutation {i}"
                );
            }
            for (field, value) in [
                ("optional", Value::Null),
                ("regenerationForbidden", json!(false)),
            ] {
                let bad = changed(&raw, |r| {
                    r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0][field] = value
                });
                assert!(!matches!(
                    compile(HEADER, &bad),
                    CompileResult::Lowered { .. }
                ));
            }
            if exile {
                let bad = changed(&raw, |r| {
                    r["binding"]["nodes"][6]["identity"] = json!("new-objects")
                });
                assert!(
                    matches!(compile(HEADER, &bad),CompileResult::Unsupported { path,.. } if path == "binding")
                );
            }
            for edit in [
                ("/what/filter/properties", json!([])),
                ("/what/filter/properties/0/property", json!("toughness")),
                ("/what/filter/properties/0/comparison/op", json!("gt")),
                (
                    "/what/filter/properties/0/comparison/value",
                    json!({"amount":"x"}),
                ),
                (
                    "/what/filter/properties/0/comparison/value/value",
                    Value::Null,
                ),
                (
                    "/what/filter/properties/0/comparison/value/value",
                    json!(7.5),
                ),
                ("/what/filter/types", json!(["creature", "artifact"])),
                ("/what/count/value", json!(2)),
            ] {
                let bad = changed(&raw, |r| {
                    let step =
                        &mut r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0];
                    *step.pointer_mut(edit.0).unwrap() = edit.1;
                });
                assert!(
                    !matches!(compile(HEADER, &bad), CompileResult::Lowered { .. }),
                    "{}",
                    edit.0
                );
            }
        }

        let Some(path) = std::env::var_os("MTG_POWER_FILTERED_REMOVAL_ARCHIVE_PROBE") else {
            return;
        };
        use CardKind::{Instant, Sorcery};
        let pins = [
            (
                143,
                "Stand Up for Yourself",
                "f0ff151a36fffa77beb066e5f195e5a2c18e98fa073fa5a3e77541a55903a2aa",
                Instant,
                false,
                true,
                3,
            ),
            (
                561,
                "Reaver Ambush",
                "9ce18b7fc3bc7e5bf2230c08f9449c0b325f73224d75a74614a6fd5ed3fdef55",
                Instant,
                true,
                false,
                3,
            ),
            (
                5053,
                "Defeat",
                "95fcedce873a112bef2020f305796aaf275f677ec8013c39420a7d02fcc59ac6",
                Sorcery,
                false,
                false,
                2,
            ),
            (
                11161,
                "Vanquish the Weak",
                "0bf1ff5eca8528d064e0478ad51cd2728bc434eac03fc615aefc65eeead935d5",
                Instant,
                false,
                false,
                3,
            ),
            (
                15113,
                "Smite the Monstrous",
                "b2194b8e1310956a3e16aa16f9897367d96cf3d3c53d7e9e030214a51e78c7bc",
                Instant,
                false,
                true,
                4,
            ),
            (
                21505,
                "Reave Soul",
                "b9ba4de1608aa4c2b8250710eae5cc293656e877f3edbb621980320d3c08dc7b",
                Sorcery,
                false,
                false,
                3,
            ),
            (
                22699,
                "Bring to Trial",
                "0369fd7ae16bca102c2053c9486e869a07c4fe6cbf55b11cedc163fdfcd8c215",
                Sorcery,
                true,
                true,
                4,
            ),
            (
                32684,
                "Blade Banish",
                "23d65ed12f45603d7c9ad0c4e7d0377a315eb35cc9ae14830e75d1fef2fe09af",
                Instant,
                true,
                true,
                4,
            ),
            (
                33350,
                "Legion's Judgment",
                "55710cc5eb0db14a7a742ccc51c11b3e5c4e920d3fe4db7d0583ceb5c8bd569f",
                Sorcery,
                false,
                true,
                4,
            ),
            (
                36137,
                "Grotesque Demise",
                "388d28d13bf57faf7c3752ccd5ca2e9ebb70eb6c19bc33e4a101326c79d77cf7",
                Instant,
                true,
                false,
                3,
            ),
        ];
        let mut rows = BufReader::new(std::fs::File::open(path).unwrap())
            .lines()
            .map(Result::unwrap)
            .peekable();
        let first: Value = serde_json::from_str(rows.peek().unwrap()).unwrap();
        let full = first["recordType"] == "manifest";
        if full {
            assert_eq!(first, serde_json::from_str::<Value>(HEADER).unwrap());
        }
        let mut previous = 0;
        for (line, name, digest, kind, exile, at_least, value) in pins {
            let raw = rows
                .nth(if full { line - previous - 1 } else { 0 })
                .unwrap();
            previous = line;
            assert_eq!(sha256(raw.as_bytes()), digest, "archive row {line}");
            let d = check(&raw, kind, exile, at_least, value);
            assert_eq!(d.name, name);
            assert_eq!(d.provenance.source_index as usize, line - 2);
        }
        if !full {
            assert!(rows.next().is_none());
        }
    }
}

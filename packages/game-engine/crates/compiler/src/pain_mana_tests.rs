//! Exact archived witnesses for source recognition, not executable admission.
//! All changed records below are synthetic, rehashed semantic fence probes.
use super::*;

const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const SOURCES: [(&str, &str, &str, [Color; 2]); 5] = [
    (
        "Sulfurous Springs",
        include_str!("../../../tests/fixtures/ramos/f5c38c01-4a40-469f-91a0-7479daf4e8e7.json"),
        "05bdc6d2db3521b796e3a806ccc75a729827a3a5297dfbcd2831ea7aff3d980f",
        [Color::B, Color::R],
    ),
    (
        "Yavimaya Coast",
        include_str!("../../../tests/fixtures/ramos/40b36bc6-c185-4bda-99e7-0118953c2c97.json"),
        "41eecbd66431e4c98c7c821cb31e6fcbe5836ec8def2a45dfc92751a446445db",
        [Color::G, Color::U],
    ),
    (
        "Talisman of Creativity",
        include_str!("../../../tests/fixtures/ramos/14d2979d-5728-42d7-a027-0eb1f754655d.json"),
        "37b5d5a8ffc9cccff3f4b040e9d5f59f7e8b90abf95e8c8b62117289d801839f",
        [Color::U, Color::R],
    ),
    (
        "Talisman of Curiosity",
        include_str!("../../../tests/fixtures/ramos/8c34b089-aad1-476e-958a-3077bf1bbb51.json"),
        "b0565847cd8bab70ac13b6834bf21c2c48be398390416acd45d2361e7a9a309f",
        [Color::G, Color::U],
    ),
    (
        "Talisman of Unity",
        include_str!("../../../tests/fixtures/ramos/e5fcc5d7-6a60-4a5b-9d02-6c30041a95b9.json"),
        "b02c7a81f173f140d1257b31a9d471c9e87673726ec285f33db3e4fc978f5884",
        [Color::G, Color::W],
    ),
];

fn inspect(raw: &str) -> Result<SourceFamily, Failure> {
    let record: Record = decode(raw, "record")?;
    let source: Source = decode(record.source.get(), "source")?;
    let result: ParseResult = decode(record.faces[0].result.get(), "faces[0].result")?;
    let binding = decode(record.binding.get(), "binding")?;
    analyze(&result, &binding, &record.oracle_identity, &source)
}

fn changed(raw: &str, mutate: impl FnOnce(&mut Value)) -> String {
    crate::tests::changed_source(raw, |record| {
        mutate(record);
        let result = &mut record["faces"][0]["result"];
        let abilities = result["abilities"].as_array().unwrap().clone();
        for (line, ability) in result["lines"].as_array_mut().unwrap().iter_mut().zip(abilities) {
            line["ability"] = ability;
        }
    })
}

fn runtime_rejected(raw: &str) {
    let family = inspect(raw).unwrap();
    let failure = family.runtime_rejection();
    assert_eq!(
        compile(MANIFEST, raw),
        CompileResult::Unsupported {
            path: failure.path,
            reason: failure.reason,
        },
    );
}

#[test]
fn pain_mana_exact_five_source_foundation_stays_runtime_rejected() {
    for (name, raw, hash, colors) in SOURCES {
        assert_eq!(sha256(raw.as_bytes()), hash, "{name}");
        assert_eq!(
            inspect(raw).unwrap(),
            SourceFamily { colors: colors.to_vec(), damage: 1.into() },
            "{name}",
        );
        // This runs the ordinary archive/source/AST integrity boundary first.
        // Recognition cannot publish a partially executable definition/catalog.
        runtime_rejected(raw);
    }
}

#[test]
fn pain_mana_synthetic_parameters_and_raw_fences() {
    let raw = SOURCES[0].1;
    for amount in [2u64, u64::MAX] {
        let parameter = changed(raw, |record| {
            record["source"]["name"] = json!("Synthetic finite mana source");
            record["faces"][0]["result"]["name"] = json!("Synthetic finite mana source");
            record["binding"]["nodes"][1]["name"] = json!("Synthetic finite mana source");
            let options = json!([
                {"mana":"fixed","symbols":["W"]},
                {"mana":"fixed","symbols":["G"]},
                {"mana":"fixed","symbols":["U"]},
            ]);
            record["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["mana"]["options"] = options;
            record["faces"][0]["result"]["abilities"][1]["effects"][1]["steps"][0]["amount"]["value"] = json!(amount);
            let text = format!("{{T}}: Add {{W}}, {{G}}, or {{U}}. This land deals {amount} damage to you.");
            record["source"]["oracle_text"] = json!(format!("{{T}}: Add {{C}}.\n{text}"));
            record["faces"][0]["result"]["lines"][1]["text"] = json!(text);
        });
        assert_eq!(inspect(&parameter).unwrap(), SourceFamily {
            colors: vec![Color::W, Color::G, Color::U],
            damage: amount.to_string().parse().unwrap(),
        });
        runtime_rejected(&parameter);
    }
    // Each mutation is rehashed and its line/aggregate AST mirror repaired.
    // Fail the source recognizer itself, not merely the runtime rejection gate.
    let paths_and_values = [
        ("/faces/0/result/abilities/0/costs/0", json!({"cost":"pay-life","amount":{"amount":"fixed","value":1}})),
        ("/faces/0/result/abilities/0/effects/0/steps/0/mana/symbols", json!(["C","C"])),
        ("/faces/0/result/abilities/1/restriction", Value::Null),
        ("/faces/0/result/abilities/1/abilityWord", Value::Null),
        ("/faces/0/result/abilities/1/effects/0/bindings", json!([])),
        ("/faces/0/result/abilities/1/effects/0/condition", Value::Null),
        ("/faces/0/result/abilities/1/effects/0/steps/0/who/of", Value::Null),
        ("/faces/0/result/abilities/1/effects/0/steps/0/mana/options", json!([])),
        ("/faces/0/result/abilities/1/effects/0/steps/0/mana/options", json!([{"mana":"fixed","symbols":["R"]},{"mana":"fixed","symbols":["R"]}])),
        ("/faces/0/result/abilities/1/effects/0/steps/0/mana/options", json!([{"mana":"fixed","symbols":["C"]},{"mana":"fixed","symbols":["R"]}])),
        ("/faces/0/result/abilities/1/effects/0/steps/0/mana/options/0", json!({"mana":"fixed","symbols":["B","R"]})),
        ("/faces/0/result/abilities/1/effects/0/steps/0/mana/options/0", json!({"mana":"choice","options":[{"mana":"fixed","symbols":["B"]}]})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/to/ref", json!({"player":"owner"})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/source", json!({"ref":"self"})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/amount", json!({"amount":"fixed","value":0})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/amount", json!({"amount":"fixed","value":-1})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/amount", json!({"amount":"fixed","value":1.0})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/amount", json!({"amount":"fixed","value":"1"})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/amount", json!({"amount":"fixed","value":18446744073709551616.0})),
        ("/faces/0/result/abilities/1/effects/1/steps/0/amount", json!({"amount":"fixed","value":1,"extra":true})),
        // Plausible parameter with unchanged printed source must also reject.
        ("/faces/0/result/abilities/1/effects/1/steps/0/amount/value", json!(2)),
    ];
    for (path, value) in paths_and_values {
        let altered = changed(raw, |record| {
            // Allow insertion of the explicit-null keys in these probes.
            if let Some(slot) = record.pointer_mut(path) {
                *slot = value;
            } else {
                let (parent, field) = path.rsplit_once('/').unwrap();
                record.pointer_mut(parent).unwrap()[field] = value;
            }
        });
        assert!(inspect(&altered).is_err(), "{path}");
        assert!(!matches!(compile(MANIFEST, &altered), CompileResult::Lowered { .. }), "{path}");
    }
    let swapped = changed(raw, |record| {
        record["faces"][0]["result"]["abilities"][1]["effects"].as_array_mut().unwrap().swap(0, 1);
    });
    assert!(inspect(&swapped).is_err());
    let extra = changed(raw, |record| {
        let third = record["faces"][0]["result"]["abilities"][0].clone();
        record["faces"][0]["result"]["abilities"].as_array_mut().unwrap().push(third);
    });
    assert!(inspect(&extra).is_err());
    let mirror = crate::tests::changed_source(raw, |record| {
        record["faces"][0]["result"]["lines"][1]["ability"]["effects"][1]["steps"][0]["amount"]["value"] = json!(2);
    });
    assert!(inspect(&mirror).is_err());
}

#[test]
fn pain_mana_rehashed_binding_closure_fences() {
    let raw = SOURCES[0].1;
    let changes: [fn(&mut Value); 10] = [
        |record| { record["binding"]["nodes"].as_array_mut().unwrap().remove(4); },
        |record| {
            let duplicate = record["binding"]["nodes"][4].clone();
            record["binding"]["nodes"].as_array_mut().unwrap().push(duplicate);
        },
        |record| { record["binding"]["nodes"][6]["role"] = json!("source-controller"); },
        |record| { record["binding"]["nodes"][8]["inputs"].as_array_mut().unwrap().swap(0, 1); },
        |record| { record["binding"]["nodes"][9]["availability"] = json!("when-performed"); },
        |record| { record["binding"]["references"][6]["target"] = record["binding"]["nodes"][6]["id"].clone(); },
        |record| { record["binding"]["references"][7]["target"] = record["binding"]["nodes"][3]["id"].clone(); },
        |record| {
            let duplicate = record["binding"]["references"][7].clone();
            record["binding"]["references"][6] = duplicate;
        },
        |record| { record["binding"]["references"][6]["rules"] = json!(["109.5"]); },
        |record| { record["binding"]["engineFeatures"] = json!(["event-history"]); },
    ];
    for (index, mutate) in changes.into_iter().enumerate() {
        let altered = changed(raw, mutate);
        let error = inspect(&altered).unwrap_err();
        assert_eq!(error.path, "binding.mana", "probe{index}");
        assert_eq!(compile(MANIFEST, &altered), CompileResult::Unsupported {
            path: error.path, reason: error.reason,
        }, "probe{index}");
    }
    for binding in [Value::Null, json!([]), json!("bound"), json!({"nodes":null})] {
        let altered = changed(raw, |record| record["binding"] = binding);
        assert_eq!(inspect(&altered).unwrap_err().path, "binding.mana");
        assert!(matches!(compile(MANIFEST, &altered), CompileResult::Unsupported { .. }));
    }
    // Order has no semantics; exact graph membership still has authority.
    let reordered = changed(raw, |record| {
        record["binding"]["nodes"].as_array_mut().unwrap().reverse();
        record["binding"]["references"].as_array_mut().unwrap().reverse();
    });
    assert_eq!(inspect(&reordered).unwrap(), inspect(raw).unwrap());
    runtime_rejected(&reordered);
}

#[test]
fn pain_mana_existing_singletons_and_city_boundary() {
    for (_, raw) in fixtures::MANA_RECORDS {
        assert!(matches!(compile(fixtures::MANIFEST, raw), CompileResult::Lowered { .. }));
    }
    let ramos = include_str!("../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json");
    let CompileResult::Lowered { definition } = compile(MANIFEST, ramos) else {
        panic!("existing non-tap counter mana must remain admitted");
    };
    assert_eq!(definition.mana_abilities.len(), 1);
    assert!(!definition.mana_abilities[0].costs.tap);
    assert!(definition.mana_abilities[0].costs.remove_source_plus_one_counters.is_some());
    let city = include_str!("../../../tests/fixtures/ramos/f25351e3-539b-4bbc-b92d-6480acf4d722.json");
    assert_eq!(sha256(city.as_bytes()), "0dbc9db808d37e7f3899690dbf2896401184e5456871408d1f654566ac96924e");
    assert!(inspect(city).is_err());
    assert!(matches!(compile(MANIFEST, city), CompileResult::Unsupported { .. }));
}

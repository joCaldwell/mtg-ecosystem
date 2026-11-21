//! CR 109.5, 400.7, 601.2c and 608.2b–c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn creature_target(what: &Value) -> Result<&str, Failure> {
    let id = what["selectionId"].as_str().unwrap_or("");
    if *what
        != json!({"ref":"target","filter":{"types":["creature"],"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
                       "count":{"amount":"fixed","value":1},"selectionId":id})
    {
        return Err(unsupported(
            "effects.return",
            "unsupported graveyard target",
        ));
    }
    Ok(id)
}

pub(super) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !d
        .capabilities
        .contains(&Capability::GraveyardBattlefieldReturnV1)
    {
        // A substituted source and AST may still carry the archived
        // battlefield-successor event. Require the instruction that consumes it.
        let step = "/faces/0/abilities/0/effects/0/steps/0";
        if let Some(nodes) = binding["nodes"].as_array()
            && nodes.iter().any(|n| n["path"] == step && n["node"] == "event" && n["operation"] == "move-zone")
            && nodes.iter().any(|n| {
                n["path"] == step && n["node"] == "event-result" && n["identity"] == "zone-successors"
                    && n["description"] == json!({"types":["creature"],"cls":"permanent","zone":{"zone":"graveyard","owner":"your"}})
            })
        {
            return Err(unsupported("binding", "unconsumed graveyard battlefield return"));
        }
        return Ok(());
    }
    // Definition validation already reconciled this original ability with every
    // executable operand and the complete printed source.
    let abilities: Value = decode(&d.provenance.abilities_json, "abilities")?;
    let effects = &abilities[0]["effects"];
    let what = &effects[0]["steps"][0]["what"];
    let mut graph = Bindings::new(&d.oracle_id, source);
    let base = "/faces/0/abilities/0";
    graph.ability(base, "spell", "spell-controller");
    let controller = graph.id(base, "you");
    let who = json!({"player":"you"});
    let step = format!("{base}/effects/0/steps/0");
    let path = format!("{step}/what");
    graph.reference(&format!("{step}/who"), &who, &controller, "109.5");
    graph.reference(
        &format!("{path}/filter/zone/owner"),
        &who,
        &controller,
        "109.5",
    );
    let target = graph.target(
        &graph.id(base, "ability"),
        &path,
        what,
        creature_target(what)?,
    );
    graph.event(base, &step, "move-zone", &target, false);
    let mut description = what["filter"].clone();
    description["cls"] = json!("permanent");
    graph.node(&step, "objects","event-result",json!({"event":graph.id(&step,"event"),
        "result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors",
        "description":description,"rules":["400.7j"]}));
    if effects.get(1).is_some() {
        let gain = format!("{base}/effects/1/steps/0");
        graph.reference(&format!("{gain}/who"), &who, &controller, "109.5");
        graph.event(base, &gain, "gain-life", &controller, false);
        graph.result(&gain, "quantity", "measured-quantity", None);
    }
    graph.validate_ordered(
        binding,
        "requires the complete graveyard battlefield return binding",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;
    use std::io::{BufRead, BufReader};

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const HAND: &str = include_str!("../../../tests/fixtures/raise-dead.json");

    // Adapt a different supported spell so default coverage uses independent
    // source, target and graph inputs even without the external frozen archive.
    fn synthetic(gain: Option<u32>) -> String {
        changed_source(HAND, |r| {
            let mut text =
                "Return target creature card from your graveyard to the battlefield.".to_owned();
            let result = &mut r["faces"][0]["result"];
            let effects = result["abilities"][0]["effects"].as_array_mut().unwrap();
            effects[0]["steps"][0]["effect"] = json!("move-zone");
            effects[0]["steps"][0]["to"] = json!({"zone":"battlefield"});
            if let Some(value) = gain {
                text.push_str(&format!(" You gain {value} life."));
                effects.push(
                    json!({"steps":[{"effect":"gain-life","who":{"player":"you"},
                    "amount":{"amount":"fixed","value":value}}]}),
                );
            }
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            result["lines"][0]["text"] = json!(text);
            r["source"]["oracle_text"] = json!(text);
            let b = &mut r["binding"];
            let base = "/faces/0/abilities/0";
            let step = format!("{base}/effects/0/steps/0");
            let root = format!("card:{}", b["identity"].as_str().unwrap());
            let controller = format!("{root}{base}#you");
            let nodes = b["nodes"].as_array_mut().unwrap();
            nodes[5]["operation"] = json!("move-zone");
            nodes.push(json!({"node":"event-result","event":format!("{root}{step}#event"),"result":"objects",
                "id":format!("{root}{step}#objects"),"path":step,"cardinality":"one","availability":"if-produced","identity":"zone-successors",
                "description":{"types":["creature"],"cls":"permanent","zone":{"zone":"graveyard","owner":"your"}},"rules":["400.7j"]}));
            let gain_step = format!("{base}/effects/1/steps/0");
            if gain.is_some() {
                nodes.push(json!({"node":"event","id":format!("{root}{gain_step}#event"),"path":gain_step,
                    "scope":format!("{root}{base}#ability"),"operation":"gain-life","inputs":[controller],
                    "availability":"when-performed","rules":["608.2c"]}));
                nodes.push(json!({"node":"event-result","id":format!("{root}{gain_step}#quantity"),"path":gain_step,
                    "event":format!("{root}{gain_step}#event"),"result":"quantity","identity":"measured-quantity",
                    "availability":"if-produced","cardinality":"one","rules":["608.2h"]}));
            }
            let actor = |step: &str| {
                json!({"path":format!("{step}/who"),"expression":{"player":"you"},
                "target":controller,"binding":"bound","rules":["109.5"]})
            };
            let refs = b["references"].as_array_mut().unwrap();
            refs.insert(0, actor(&step));
            if gain.is_some() {
                refs.push(actor(&gain_step));
            }
            let count = refs.len();
            b["counts"] = json!({"bound":count,"obligations":count,"unresolved":0});
            b["referencesBound"] = json!(true);
            b["engineFeatures"] = json!([]);
        })
    }

    fn check(raw: &str, gain: Option<u32>) -> Box<Definition> {
        let result = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = result else {
            panic!("{result:?}");
        };
        let p = d.spell.as_ref().unwrap();
        assert_eq!(
            p.targets,
            [TargetSpec {
                slot: 0,
                selection_id: "line:0:1".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::OwnGraveyardCreatureCard,
                filter: None
            }]
        );
        let mut instructions = vec![Instruction::ReturnOwnGraveyardCreatureToBattlefield {
            target_slot: 0,
            source_block: 0,
        }];
        let mut caps = BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastSorceryV1,
            Capability::GraveyardRecursionV1,
            Capability::GraveyardBattlefieldReturnV1,
        ]);
        if let Some(value) = gain {
            instructions.push(Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: i64::from(value).into(),
                source_block: 0,
            });
            caps.insert(Capability::LifeGainV1);
        }
        assert_eq!(p.instructions, instructions);
        assert_eq!(d.capabilities, caps);
        d
    }

    #[test]
    fn graveyard_battlefield_return() {
        for gain in [None, Some(3), Some(11)] {
            check(&synthetic(gain), gain);
        }
        let raw = synthetic(Some(3));
        let mutations: &[(&str, Value)] = &[
            ("/binding/nodes/4/timing", json!("resolution")),
            ("/binding/nodes/5/inputs", json!([])),
            ("/binding/nodes/6/identity", json!("same-objects")),
            ("/binding/nodes/7/operation", json!("lose-life")),
            ("/binding/nodes/8/cardinality", json!("many")),
            ("/binding/references/1/target", json!("owner")),
            (
                "/binding/references/3/expression",
                json!({"player":"opponent"}),
            ),
            ("/binding/engineFeatures", json!(["control"])),
            (
                "/faces/0/result/abilities/0/effects/0/steps/0/what/filter/zone/owner",
                json!("any"),
            ),
            (
                "/faces/0/result/abilities/0/effects/0/steps/0/what/filter/zone/zone",
                json!("exile"),
            ),
            (
                "/faces/0/result/abilities/0/effects/0/steps/0/what/filter/types",
                json!(["artifact"]),
            ),
            (
                "/faces/0/result/abilities/0/effects/0/steps/0/to",
                json!({"zone":"battlefield","controller":"you"}),
            ),
            (
                "/faces/0/result/abilities/0/effects/1/steps/0/who",
                json!({"player":"you","of":null}),
            ),
            (
                "/faces/0/result/abilities/0/effects/1/steps/0/amount",
                json!({"amount":"x"}),
            ),
            ("/source/keywords", json!(["Flash"])),
        ];
        for (path, value) in mutations {
            let bad = changed_source(&raw, |r| {
                *r.pointer_mut(path).unwrap() = value.clone();
                let result = &mut r["faces"][0]["result"];
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(
                !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                "{path}"
            );
        }
        // The same two clauses on an instant are an ordinary card: the block
        // pipeline lowers them, marked composed, at instant timing.
        let instant = changed_source(&raw, |r| r["source"]["type_line"] = json!("Instant"));
        let d = crate::tests::composed_near_miss(crate::tests::synthetic_compile(MANIFEST, &instant));
        let original = check(&raw, Some(3));
        let (p, q) = (d.spell.as_ref().unwrap(), original.spell.as_ref().unwrap());
        assert_eq!(d.characteristics.kind, CardKind::Instant);
        assert_eq!(p.timing, SpellTiming::Instant);
        assert_eq!((&p.targets, &p.instructions), (&q.targets, &q.instructions));
        for value in [Value::Null, json!(true), json!(false)] {
            let bad = changed_source(&raw, |r| {
                let result = &mut r["faces"][0]["result"];
                result["abilities"][0]["effects"][0]["steps"][0]["tapped"] = value;
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(!matches!(
                compile(MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
        let substituted = changed_source(&raw, |r| {
            let original: Value = serde_json::from_str(HAND).unwrap();
            let result = &mut r["faces"][0]["result"];
            result["abilities"] = original["faces"][0]["result"]["abilities"].clone();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(!matches!(
            compile(MANIFEST, &substituted),
            CompileResult::Lowered { .. }
        ));
        let stale_graph = changed_source(HAND, |r| {
            r["binding"] = serde_json::from_str::<Value>(&raw).unwrap()["binding"].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &stale_graph), CompileResult::Unsupported{path,..} if path == "binding")
        );
        let extra_clause = changed_source(&raw, |r| {
            let text = format!(
                "{} Draw a card.",
                r["source"]["oracle_text"].as_str().unwrap()
            );
            r["source"]["oracle_text"] = json!(text);
            r["faces"][0]["result"]["lines"][0]["text"] = json!(text);
        });
        assert!(!matches!(
            compile(MANIFEST, &extra_clause),
            CompileResult::Lowered { .. }
        ));

        let Some(path) = std::env::var_os("MTG_GRAVEYARD_BATTLEFIELD_RETURN_ARCHIVE_PROBE") else {
            return;
        };
        let pins = [
            (
                7299,
                "Breath of Life",
                "66c278645e5d996918969f95f0f071a8216c4de67a8e2aab6b7920c3caffd091",
                None,
            ),
            (
                10527,
                "Rise Again",
                "f4871bb65776bb3f2e9f1dea22223e9780d32c19a65240710b6b10b2c1252a70",
                None,
            ),
            (
                19774,
                "Resurrection",
                "22ae5bd0adb68fb18a4e34cb2650b321496a7c1af19934ecc18acca62732cd29",
                None,
            ),
            (
                20958,
                "False Defeat",
                "c221a599a248557bf6671b864e8178093d6c989fe5e5a2b3efce6c4ea782f76f",
                None,
            ),
            (
                28222,
                "Zombify",
                "90b4d2ebe2665ccb03acf24b71f656dd5c1c2c27f0504944ce869a2f010cd75f",
                None,
            ),
            (
                38542,
                "Emerge from the Cocoon",
                "e88c54cbd14928a7bad58ee3a85fc66042674eddb1e36ccfd35a5f3e68cbcce6",
                Some(3),
            ),
        ];
        let lines = BufReader::new(std::fs::File::open(path).unwrap()).lines();
        let mut full = false;
        let mut seen = 0;
        for (index, line) in lines.enumerate() {
            let raw = line.unwrap();
            if index == 0 {
                let first: Value = serde_json::from_str(&raw).unwrap();
                full = first["recordType"] == "manifest";
                if full {
                    assert_eq!(raw, MANIFEST.trim_end());
                }
            }
            if full && (seen == pins.len() || index + 1 != pins[seen].0) {
                continue;
            }
            let (row, name, digest, gain) = pins[seen];
            assert_eq!(sha256(raw.as_bytes()), digest, "row {row}");
            let d = check(&raw, gain);
            assert_eq!(d.name, name);
            assert_eq!(d.provenance.source_index as usize + 2, row);
            seen += 1;
        }
        assert_eq!(seen, pins.len());
    }
}

//! CR 109.5, 601.2c, 603.3d, 603.5 and 608.2b–d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    const BASE: &str = "/faces/0/abilities/0";
    const STEP: &str = "/faces/0/abilities/0/effects/0/steps/0";
    const WHAT: &str = "/faces/0/abilities/0/effects/0/steps/0/what";
    let message = "requires a fully bound controller graveyard card return";
    if !d
        .capabilities
        .contains(&Capability::OwnGraveyardCardReturnV1)
    {
        // A replaced printed sentence and AST must also consume or replace the
        // archived return event, including its distinct untyped target.
        if let Some(nodes) = binding["nodes"].as_array()
            && nodes.iter().any(|n| {
                n["path"] == WHAT
                    && n["node"] == "selection"
                    && n["expression"]["filter"]
                        == json!({"cls":"card","zone":{"zone":"graveyard","owner":"your"}})
                    && nodes.iter().any(|e| {
                        e["path"] == STEP
                            && e["node"] == "event"
                            && e["operation"] == "return-to-hand"
                            && e["inputs"] == json!([n["id"]])
                    })
            })
        {
            return Err(unsupported("binding", message));
        }
        return Ok(());
    }
    // The contracts validator has already reconciled the unmodified archive AST
    // with printed text, characteristics, exact target and instruction.
    let abilities: Value = decode(&d.provenance.abilities_json, "abilities")?;
    let entry = d.characteristics.kind == CardKind::Creature;
    let mut graph = Bindings::new(&d.oracle_id, source);
    if entry {
        graph.ability(BASE, "triggered", "trigger-controller");
        let trigger = format!("{BASE}/trigger");
        let subject = &abilities[0]["trigger"]["what"];
        let object = graph.id("/source", "object");
        graph.reference(&format!("{trigger}/what"), subject, &object, "201.5");
        graph.event(BASE, &trigger, "trigger:enters", &object, false);
        graph.result(
            &trigger,
            "objects",
            "same-objects",
            Some(&json!({"types":["creature"]})),
        );
    } else {
        graph.ability(BASE, "spell", "spell-controller");
    }
    graph.reference(
        &format!("{WHAT}/filter/zone/owner"),
        &json!({"player":"you"}),
        &graph.id(BASE, "you"),
        "109.5",
    );
    let what = &abilities[0]["effects"][0]["steps"][0]["what"];
    let selection = what["selectionId"]
        .as_str()
        .ok_or_else(|| unsupported("binding", message))?;
    let target = graph.target(&graph.id(BASE, "ability"), WHAT, what, selection);
    graph.event(BASE, STEP, "return-to-hand", &target, entry);
    graph.validate_ordered(binding, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;
    use std::io::{BufRead, BufReader};

    const SPELL: &str = include_str!("../../../tests/fixtures/raise-dead.json");
    const ETB: &str = include_str!("../../../tests/fixtures/gravedigger.json");
    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");

    // Independently adapt the old typed sources and the spell's bound target
    // and event. The historic Gravedigger graph bound only the trigger subject.
    fn synthetic(kind: CardKind) -> String {
        let entry = kind == CardKind::Creature;
        changed_source(if entry { ETB } else { SPELL }, |r| {
            let text = r["source"]["oracle_text"]
                .as_str()
                .unwrap()
                .replace("target creature card", "target card");
            r["source"]["oracle_text"] = json!(text);
            let result = &mut r["faces"][0]["result"];
            let ability = &mut result["abilities"][0];
            ability["effects"][0]["steps"][0]["what"]["filter"]
                .as_object_mut()
                .unwrap()
                .remove("types");
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            result["lines"][0]["text"] = json!(text);

            let spell: Value = serde_json::from_str(SPELL).unwrap();
            let mut tail = spell["binding"].to_string().replace(
                spell["oracleIdentity"].as_str().unwrap(),
                r["oracleIdentity"].as_str().unwrap(),
            );
            if entry {
                tail = tail
                    .replace("line%3A0%3A1", "line%3A0%3A8")
                    .replace("line:0:1", "line:0:8");
            }
            let tail: Value = serde_json::from_str(&tail).unwrap();
            let mut target = tail["nodes"][4].clone();
            target["expression"]["filter"]
                .as_object_mut()
                .unwrap()
                .remove("types");
            let mut reference = tail["references"][1].clone();
            reference["expression"] = target["expression"].clone();
            let mut event = tail["nodes"][5].clone();
            if entry {
                event["optional"] = json!(true);
                event["decider"] = r["binding"]["nodes"][3]["id"].clone();
            }
            if kind == CardKind::Instant {
                r["source"]["type_line"] = json!("Instant");
                r["binding"]["nodes"][1]["typeLine"] = json!("Instant");
            }
            let b = &mut r["binding"];
            let nodes = b["nodes"].as_array_mut().unwrap();
            nodes.truncate(if entry { 6 } else { 4 });
            nodes.extend([target, event]);
            let refs = b["references"].as_array_mut().unwrap();
            refs.truncate(usize::from(entry));
            refs.extend([tail["references"][0].clone(), reference]);
            let count = refs.len();
            b["counts"] = json!({"obligations":count,"bound":count,"unresolved":0});
            b["fullyBound"] = json!(true);
            b["referencesBound"] = json!(true);
            b["diagnostics"] = json!([]);
            b["engineFeatures"] = json!([]);
        })
    }

    fn check(raw: &str, kind: CardKind) -> Box<Definition> {
        let result = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = result else {
            panic!("{result:?}")
        };
        assert_eq!(d.characteristics.kind, kind);
        let entry = kind == CardKind::Creature;
        let p = if entry {
            &d.triggered[0].program
        } else {
            d.spell.as_ref().unwrap()
        };
        assert_eq!(
            p.targets,
            [TargetSpec {
                slot: 0,
                selection_id: if entry { "line:0:8" } else { "line:0:1" }.into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::OwnGraveyardCard,
                filter: None
            }]
        );
        assert_eq!(
            p.instructions,
            [Instruction::ReturnOwnGraveyardCardToHand {
                target_slot: 0,
                optional: entry,
                source_block: 0
            }]
        );
        use Capability::*;
        let mut caps = BTreeSet::from([NormalLayoutV1, OwnGraveyardCardReturnV1]);
        match kind {
            CardKind::Creature => caps.extend([
                BaseCreatureV1,
                CastCreatureV1,
                TriggeredAbilityV1,
                SelfEntersBattlefieldV1,
            ]),
            CardKind::Sorcery => {
                caps.insert(CastSorceryV1);
            }
            CardKind::Instant => {
                caps.insert(CastInstantV1);
            }
            _ => unreachable!(),
        }
        assert_eq!(d.capabilities, caps);
        d
    }

    #[test]
    fn own_graveyard_card_return() {
        for kind in [CardKind::Creature, CardKind::Sorcery, CardKind::Instant] {
            let raw = synthetic(kind);
            check(&raw, kind);
            for change in 0..13 {
                let bad = changed_source(&raw, |r| {
                    let a = &mut r["faces"][0]["result"]["abilities"][0];
                    let step = &mut a["effects"][0]["steps"][0];
                    match change {
                        0 => step["what"]["filter"]["zone"]["owner"] = json!("any"),
                        1 => step["to"] = json!("owner"),
                        2 => step["what"]["filter"]["types"] = json!(["creature"]),
                        3 => step["optional"] = Value::Null,
                        4 => step["what"]["count"]["value"] = json!(2),
                        5 => step["what"]["selectionId"] = json!("line:1:1"),
                        6 => a["abilityWord"] = json!("Delirium"),
                        7 => r["binding"]["fullyBound"] = json!(false),
                        8 => {
                            let n = r["binding"]["nodes"]
                                .as_array_mut()
                                .unwrap()
                                .last_mut()
                                .unwrap();
                            n["inputs"] = json!(["source"]);
                        }
                        9 => {
                            let n = r["binding"]["nodes"]
                                .as_array_mut()
                                .unwrap()
                                .last_mut()
                                .unwrap();
                            n["decider"] = json!("owner");
                        }
                        10 => {
                            let refs = r["binding"]["references"].as_array_mut().unwrap();
                            let owner = refs
                                .iter_mut()
                                .find(|v| v["path"].as_str().unwrap().ends_with("/zone/owner"))
                                .unwrap();
                            owner["target"] = json!("opponent");
                        }
                        11 => r["source"]["keywords"] = json!(["Flash"]),
                        _ => {
                            let text = format!(
                                "{} Draw a card.",
                                r["source"]["oracle_text"].as_str().unwrap()
                            );
                            r["source"]["oracle_text"] = json!(text);
                            r["faces"][0]["result"]["lines"][0]["text"] = json!(text);
                        }
                    }
                    let result = &mut r["faces"][0]["result"];
                    result["lines"][0]["ability"] = result["abilities"][0].clone();
                });
                assert!(
                    !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                    "{kind:?} mutation {change}"
                );
            }
        }
        for (old, kind) in [(SPELL, CardKind::Sorcery), (ETB, CardKind::Creature)] {
            let untyped: Value = serde_json::from_str(&synthetic(kind)).unwrap();
            let stale = changed_source(old, |r| r["binding"] = untyped["binding"].clone());
            assert!(
                matches!(compile(MANIFEST,&stale),CompileResult::Unsupported { path,.. } if path == "binding")
            );
            let CompileResult::Lowered { definition } = compile(MANIFEST, old) else {
                panic!("legacy return")
            };
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::GraveyardRecursionV1)
            );
            assert!(
                !definition
                    .capabilities
                    .contains(&Capability::OwnGraveyardCardReturnV1)
            );
        }

        let Some(path) = std::env::var_os("MTG_OWN_GRAVEYARD_CARD_RETURN_ARCHIVE_PROBE") else {
            return;
        };
        let pins = [
            (
                7272,
                "Eternal Witness",
                CardKind::Creature,
                "feab01f792f10e115969d47513975742beb8087168fedcbae3b79cfdd5417a8e",
            ),
            (
                12194,
                "Auroral Procession",
                CardKind::Instant,
                "a37fdb09f19cbe6e3b1ff1bdda5e27fc0c01d200ebd9d9d639720d0ebd849781",
            ),
            (
                13714,
                "Recollect",
                CardKind::Sorcery,
                "b2191ff622ebcb6db071031e5559914d197c1a6f31d78d8e30cd4b5a96c2f5b5",
            ),
            (
                20880,
                "Elven Cache",
                CardKind::Sorcery,
                "4d41514c086a3e4ae91ad1939f474e716f4dd261b8547562261e9a8d32775196",
            ),
            (
                34818,
                "Regrowth",
                CardKind::Sorcery,
                "e30eb8e20388a74c70f86bfa41bb238d7728d7c6aa67b27882714d588a1f00c7",
            ),
        ];
        let mut lines = BufReader::new(std::fs::File::open(path).unwrap()).lines();
        let first = lines.next().unwrap().unwrap();
        let full = first == MANIFEST.trim_end();
        let mut pins = pins.into_iter().peekable();
        for (i, line) in std::iter::once(Ok(first)).chain(lines).enumerate() {
            let row = line.unwrap();
            if full && pins.peek().is_none_or(|p| p.0 != i + 1) {
                continue;
            }
            let (number, name, kind, digest) = pins.next().expect("unexpected extra probe row");
            assert_eq!(sha256(row.as_bytes()), digest, "physical row {number}");
            let d = check(&row, kind);
            assert_eq!(d.name, name);
            assert_eq!(d.provenance.source_index as usize + 2, number);
            if full && pins.peek().is_none() {
                break;
            }
        }
        assert!(pins.peek().is_none(), "missing probe row");
    }
}

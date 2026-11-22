//! CR 603.3d/603.5 and 608.2b–d. The target is required even if the
//! trigger controller may decline the return when the ability resolves.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

fn fail() -> Failure {
    unsupported(
        "abilities.creature-entry-return",
        "requires a complete bound self-entry creature return",
    )
}

pub(crate) fn flying_binding(graph: &mut Bindings<'_>, base: &str) {
    graph.ability(base, "keywords", "static-source-controller");
    let path = format!("{base}/keywords/0");
    let scope = graph.id(base, "ability");
    let you = graph.node(
        &path,
        "you",
        "runtime-player",
        json!({
            "scope":scope,"role":"static-source-controller","rules":["109.5","702.9"]
        }),
    );
    graph.node(
        &path,
        "keyword",
        "keyword-ability",
        json!({
            "scope":scope,"keyword":"flying","abilities":["static"],"you":you,"rules":["702.9"]
        }),
    );
}

fn return_source(
    graph: &mut Bindings<'_>,
    block: u32,
    opponent: bool,
    optional: bool,
) -> (String, Value) {
    let base = format!("/faces/0/abilities/{block}");
    graph.ability(&base, "triggered", "trigger-controller");
    let scope = graph.id(&base, "ability");
    let trigger = format!("{base}/trigger");
    let subject = json!({"ref":"this","noun":"creature"});
    let source_id = graph.id("/source", "object");
    graph.reference(&format!("{trigger}/what"), &subject, &source_id, "201.5");
    graph.event(&base, &trigger, "trigger:enters", &source_id, false);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"types":["creature"]})),
    );

    let step = format!("{base}/effects/0/steps/0");
    let path = format!("{step}/what");
    let mut filter = json!({"types":["creature"]});
    if opponent {
        filter["control"] = json!("opponent");
        let path = format!("{path}/filter/control");
        let opponents = graph.node(&path, "opponents", "derived-player", json!({
            "scope":scope,"role":"opponents","of":graph.id(&base,"you"),"rules":["102.2","109.5"]
        }));
        graph.reference(&path, &json!({"implicit":"opponents"}), &opponents, "102.2");
    }
    let selection = format!("line:{block}:{}", if optional { 8 } else { 6 });
    let what = json!({"ref":"target","filter":filter,"count":{"amount":"fixed","value":1},"selectionId":selection});
    let target = graph.target(&scope, &path, &what, &selection);
    if opponent {
        graph.node(
            &format!("{step}/participant-0"),
            "controller-of-described",
            "derived-player",
            json!({"scope":scope,"role":"controller","of":target,"rules":["108.4"]}),
        );
    }
    let to = format!("{step}/to");
    let owner = graph.node(
        &to,
        "owner",
        "derived-player",
        json!({"scope":scope,"role":"owner","of":target,"rules":["108.3"]}),
    );
    graph.reference(
        &to,
        &json!({"player":"owner","of":{"ref":"it"}}),
        &owner,
        "108.3",
    );
    graph.event(&base, &step, "return-to-hand", &target, optional);
    let mut effect = json!({"effect":"return-to-hand","what":what,"to":"owner"});
    if optional {
        effect["optional"] = json!(true);
    }

    let may = if optional { "you may " } else { "" };
    let control = if opponent {
        " an opponent controls"
    } else {
        ""
    };
    (
        format!(
            "When this creature enters, {may}return target creature{control} to its owner's hand."
        ),
        json!({"kind":"triggered","trigger":{"trigger":"enters","what":subject},"effects":[{"steps":[effect]}]}),
    )
}

pub(crate) fn validate_source(
    d: &Definition,
    labels: &SourceLabels,
    original: &[Box<RawValue>],
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::CreatureEntryReturnV1) {
        // A partial or replaced AST must not route the printed return through a different program.
        if labels.blocks.iter().any(|line| {
            line.starts_with("When this creature enters, ")
                && line.contains("return target creature")
                && line.contains("to its owner's hand")
        }) {
            return Err(fail());
        }
        return Ok(());
    }

    let [trigger] = d.triggered.as_slice() else {
        return Err(fail());
    };
    let [
        Instruction::ReturnEntryTargetCreature {
            source_block,
            optional,
            ..
        },
    ] = trigger.program.instructions.as_slice()
    else {
        return Err(fail());
    };
    let printed = labels.text.lines().collect::<Vec<_>>();
    let flying = d.keywords == [KeywordAbility::Flying];
    if original.len() != 1 + usize::from(flying)
        || printed.len() != original.len()
        || labels.triggered != [*source_block]
        || source["keywords"] != if flying { json!(["Flying"]) } else { json!([]) }
    {
        return Err(fail());
    }
    let mut graph = Bindings::new(&d.oracle_id, source);
    for (block, raw) in original.iter().enumerate() {
        let expected = if block == *source_block as usize {
            let opponent = trigger.program.targets[0]
                .filter
                .as_ref()
                .ok_or_else(fail)?
                .opponent;
            let (line, ability) = return_source(&mut graph, *source_block, opponent, *optional);
            if labels.blocks[block] != line || printed[block] != line {
                return Err(fail());
            }
            ability
        } else {
            if labels.blocks[block] != "Flying"
                || !matches!(
                    printed[block],
                    "Flying"
                        | "Flying (This creature can't be blocked except by creatures with flying or reach.)"
                )
            {
                return Err(fail());
            }
            flying_binding(&mut graph, &format!("/faces/0/abilities/{block}"));
            json!({"kind":"keywords","keywords":[{"keyword":"flying"}]})
        };
        if decode::<Value>(raw.get(), "abilities")? != expected {
            return Err(fail());
        }
    }
    graph.validate(binding, "requires complete creature entry return bindings")
}

#[cfg(test)]
#[test]
fn creature_entry_return() {
    let trigger: SourceTrigger =
        serde_json::from_value(json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}}))
            .unwrap();
    for (optional, opponent) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut graph = Bindings::new("test", &json!({}));
        let (_, raw) = return_source(&mut graph, 0, opponent, optional);
        let effects: Vec<Sentence> = serde_json::from_value(raw["effects"].clone()).unwrap();
        let ability = lower_trigger(&trigger, &effects, None, 0).unwrap();
        assert_eq!(ability.condition, TriggerCondition::SelfEntersBattlefield);
        assert_eq!(ability.program.targets.len(), 1);
        assert_eq!(
            ability.program.targets[0].domain,
            TargetDomain::BattlefieldPermanent
        );
        assert_eq!(
            ability.program.targets[0].filter.as_ref().unwrap().opponent,
            opponent
        );
        assert_eq!(
            ability.program.instructions,
            [Instruction::ReturnEntryTargetCreature {
                target_slot: 0,
                optional,
                source_block: 0
            }]
        );
    }

    let Some(path) = std::env::var_os("MTG_CREATURE_ENTRY_RETURN_ARCHIVE_PROBE") else {
        return;
    };
    const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    let rows = std::fs::read_to_string(path).unwrap();
    // Exact archive lines and bytes, printed Flying, target restricted to opponents, optional return.
    let expected = [
        (
            2810,
            "Mist Raven",
            "e5a54fa8e36eb4fd68fa749df8df01132391d1b72585bdae1b540bdeb889c2e5",
            true,
            false,
            false,
        ),
        (
            6284,
            "Riddlemaster Sphinx",
            "f14f7af722c4af9eb3764eed58cf36d1253488b6de36376874e319b016543506",
            true,
            true,
            true,
        ),
        (
            11756,
            "Bigfin Bouncer",
            "6b566f0753f802ca0d27b08429c48a9bfeeed5f65a6b7398aa0af1d9313241c0",
            false,
            true,
            false,
        ),
        (
            12625,
            "Iceridge Serpent",
            "df4e085a715a54b10ff8335fddde4fed19a82bf59144123bfb5f9829298ece46",
            false,
            true,
            false,
        ),
        (
            14737,
            "Angler Drake",
            "759abf298764ed065249cb87fd5e48b736245b0ed302a0973f9870b6f424163b",
            true,
            false,
            true,
        ),
        (
            15407,
            "Voidwielder",
            "4c4618e35a34ab8622374fdf6e031c2f5aea8298d3b0cda095463aac11de74cd",
            false,
            false,
            true,
        ),
        (
            15579,
            "Man-o'-War",
            "8e62a23cc61f85ac465ad710e9503020f9bd329ecb2d3f7521377a1255760881",
            false,
            false,
            false,
        ),
        (
            32927,
            "Separatist Voidmage",
            "f778f17e00332c08cc92ea385bb85d5df5d663a47d738eaaccffde90f16cd5e6",
            false,
            false,
            true,
        ),
        (
            33865,
            "Aether Adept",
            "223c6542305c5ab3ae5fca562cb344262b483e2adfb25a9123ef5c8d40beb44b",
            false,
            false,
            false,
        ),
        (
            38292,
            "Exclusion Mage",
            "cb7f61ba5e563578d8461f50bc2b4336b12438582bfddd111a7e2e1eb1a5e4f0",
            false,
            true,
            false,
        ),
    ];
    assert_eq!(rows.lines().count(), expected.len());
    for (row, (line, name, digest, flying, opponent, optional)) in rows.lines().zip(expected) {
        assert_eq!(sha256(row.as_bytes()), digest);
        let d = match compile(MANIFEST, row) {
            CompileResult::Lowered { definition } => definition,
            rejected => panic!("line {line}: {rejected:?}"),
        };
        assert_eq!(d.provenance.source_index + 2, line);
        assert_eq!(
            d.provenance.archive_manifest_sha256,
            sha256(MANIFEST.trim_end().as_bytes())
        );
        assert_eq!(d.name, name);
        assert_eq!(d.keywords.contains(&KeywordAbility::Flying), flying);
        assert!(d.capabilities.contains(&Capability::CreatureEntryReturnV1));
        let p = &d.triggered[0].program;
        assert_eq!(p.targets[0].filter.as_ref().unwrap().opponent, opponent);
        assert_eq!(
            p.instructions,
            [Instruction::ReturnEntryTargetCreature {
                target_slot: 0,
                optional,
                source_block: u32::from(flying)
            }]
        );

        // Use the optional opposing target to exercise all three distinct bindings.
        if flying && opponent && optional {
            let rebuilt = tests::changed_source(row, |_| {});
            assert!(matches!(
                compile(MANIFEST, &rebuilt),
                CompileResult::Lowered { .. }
            ));
            for mutation in 0..10 {
                let wrong = tests::changed_source(row, |r| {
                    let result = &mut r["faces"][0]["result"];
                    let a = &mut result["abilities"][1];
                    let step = &mut a["effects"][0]["steps"][0];
                    match mutation {
                        0 => {
                            step.as_object_mut().unwrap().remove("optional");
                        }
                        1 => {
                            step["what"]["filter"]
                                .as_object_mut()
                                .unwrap()
                                .remove("control");
                        }
                        2 => step["what"]["count"]["value"] = json!(2),
                        3 => a["condition"] = Value::Null,
                        4 => step["to"] = json!("your"),
                        _ => {}
                    }
                    result["lines"][1]["ability"] = result["abilities"][1].clone();
                    if mutation == 8 {
                        let text = format!(
                            "{} Then draw a card.",
                            result["lines"][1]["text"].as_str().unwrap()
                        );
                        result["lines"][1]["text"] = json!(text);
                        r["source"]["oracle_text"] = json!(format!(
                            "{} Then draw a card.",
                            r["source"]["oracle_text"].as_str().unwrap()
                        ));
                    }
                    if mutation == 9 {
                        let a = &mut r["faces"][0]["result"]["abilities"][1];
                        a["effects"][0]["steps"][0] = json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}});
                        let replaced = a.clone();
                        r["faces"][0]["result"]["lines"][1]["ability"] = replaced;
                    }
                    for node in r["binding"]["nodes"].as_array_mut().unwrap() {
                        match mutation {
                            5 if node["node"] == "selection" => {
                                node["timing"] = json!("resolution")
                            }
                            6 if node["role"] == "owner" => node["of"] = json!("source"),
                            7 if node["operation"] == "return-to-hand" => {
                                node["decider"] = json!("owner")
                            }
                            _ => {}
                        }
                    }
                });
                assert!(
                    !matches!(compile(MANIFEST, &wrong), CompileResult::Lowered { .. }),
                    "mutation {mutation}"
                );
            }
        }
    }
}

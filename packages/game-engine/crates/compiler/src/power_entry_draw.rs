//! CR 109.5, 603.4, 603.6a and 603.10: the entry subject and the controller's
//! intervening battlefield predicate bind independently of the draw recipient.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn shape_error() -> Failure {
    unsupported(
        "abilities.power-entry-draw",
        "requires complete power-filtered entry draw source",
    )
}

pub(super) fn minimum_power(
    filter: &TriggerFilter,
    count: &Count,
    entering: bool,
) -> Result<Quantity, Failure> {
    let Some([property]) = &filter.properties else {
        return Err(shape_error());
    };
    if !fixed_one(count)
        || filter.types != ["creature"]
        || filter.control.as_deref() != entering.then_some("you")
        || (!entering && filter.other)
        || filter.non_token
        || !filter.non_subtypes.is_empty()
        || property.property != "power"
        || property.comparison.op != "ge"
        || property.comparison.value.amount != "fixed"
    {
        return Err(shape_error());
    }
    Ok(i64::from(property.comparison.value.value).into())
}

fn is_entry_draw_line(line: &str) -> bool {
    (line.starts_with("Whenever a creature you control with power ")
        || line.starts_with("Whenever another creature you control with power "))
        && line.contains(" or greater enters, ")
        || line.starts_with("When this enchantment enters, if you control a creature with power ")
}

// Validate the raw archive independently of serde defaults. A null qualifier,
// an omitted predicate, or a different binding for "you" changes the witness.
fn power_subject(power: &Quantity, entering: bool, other: bool) -> Result<Value, Failure> {
    let n: u32 = power.to_string().parse().map_err(|_| shape_error())?;
    let mut filter = json!({"types":["creature"], "properties":[
        {"property":"power","comparison":{"op":"ge","value":{"amount":"fixed","value":n}}}
    ]});
    if entering {
        filter["control"] = json!("you");
    }
    if other {
        filter["other"] = json!(true);
    }
    Ok(json!({"ref":"filter","filter":filter,"count":{"amount":"fixed","value":1}}))
}

// Construct the expected diagnostic graph from executable operands, never the reverse.
struct Bindings<'a> {
    identity: &'a str,
    nodes: Vec<Value>,
    references: Vec<Value>,
}

impl Bindings<'_> {
    fn id(&self, path: &str, suffix: &str) -> String {
        format!("card:{}{path}#{suffix}", self.identity)
    }

    fn node(&mut self, path: &str, suffix: &str, kind: &str, mut fields: Value) -> String {
        let id = self.id(path, suffix);
        fields["id"] = json!(id);
        fields["path"] = json!(path);
        fields["node"] = json!(kind);
        self.nodes.push(fields);
        id
    }

    fn reference(&mut self, path: &str, expression: &Value, target: &str, rule: &str) {
        self.references
            .push(json!({"path":path,"expression":expression,
            "binding":"bound","target":target,"rules":[rule]}));
    }

    fn ability(&mut self, path: &str, kind: &str, role: &str) {
        let ability = self.node(
            path,
            "ability",
            "ability",
            json!({
            "source":self.id("/source","object"),"face":self.id("/faces/0","face"),
            "abilityKind":kind,"you":self.id(path,"you"),"rules":["113","109.5"]}),
        );
        self.node(
            path,
            "you",
            "runtime-player",
            json!({"scope":ability,"role":role,"rules":["109.5"]}),
        );
    }

    fn selection(
        &mut self,
        base: &str,
        path: &str,
        what: &Value,
        cardinality: &str,
        timing: &str,
    ) -> String {
        let query = self.node(
            path,
            "query",
            "selection",
            json!({
            "scope":self.id(base,"ability"),"expression":what,"cardinality":cardinality,
            "timing":timing,"selection":"evaluate-reference","rules":["608.2h"]}),
        );
        self.reference(path, what, &query, "608.2h");
        if what["filter"]["control"] == "you" {
            self.reference(
                &format!("{path}/filter/control"),
                &json!({"player":"you"}),
                &self.id(base, "you"),
                "109.5",
            );
        }
        if what["filter"]["other"] == true {
            self.reference(
                &format!("{path}/filter/other"),
                &json!({"ref":"self"}),
                &self.id("/source", "object"),
                "201.5",
            );
        }
        query
    }

    fn event(&mut self, base: &str, path: &str, operation: &str, input: &str, optional: bool) {
        let mut fields = json!({"scope":self.id(base,"ability"),"operation":operation,
            "inputs":[input],"availability":"when-performed","rules":["608.2c"]});
        if optional {
            fields["optional"] = json!(true);
            fields["decider"] = json!(self.id(base, "you"));
        }
        self.node(path, "event", "event", fields);
    }

    fn result(&mut self, path: &str, kind: &str, identity: &str, description: Option<&Value>) {
        let mut fields = json!({"event":self.id(path,"event"),"result":kind,"cardinality":"one",
            "availability":"if-produced","identity":identity,"rules":["608.2h"]});
        if let Some(description) = description {
            fields["description"] = description.clone();
        }
        self.node(path, kind, "event-result", fields);
    }

    fn validate(&self, binding: &Value) -> Result<(), Failure> {
        let same_members = |actual: &Value, expected: &[Value]| {
            actual.as_array().is_some_and(|items| {
                items.len() == expected.len() && expected.iter().all(|item| items.contains(item))
            })
        };
        // All twelve archive-v1 envelope fields are checked below.
        if binding.as_object().is_none_or(|fields| fields.len() != 12)
            || binding["artifactKind"] != "oracle-reference-binding"
            || binding["schemaVersion"] != 1
            || binding["executable"] != false
            || binding["rules"]
                != json!({"edition":RULES_EDITION,
                "url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"})
            || binding["identity"] != self.identity
            || binding["fullyBound"] != true
            || binding["referencesBound"] != true
            || binding["diagnostics"] != json!([])
            || binding["engineFeatures"] != json!([])
            || binding["counts"]
                != json!({"obligations":self.references.len(),"bound":self.references.len(),"unresolved":0})
            || !same_members(&binding["nodes"], &self.nodes)
            || !same_members(&binding["references"], &self.references)
        {
            return Err(unsupported(
                "binding",
                "requires complete power-filtered entry draw bindings",
            ));
        }
        Ok(())
    }
}

fn trigger_source(
    graph: &mut Bindings<'_>,
    block: u32,
    ability: &TriggeredAbility,
) -> Result<(String, Value), Failure> {
    let optional = match ability.program.instructions.as_slice() {
        [
            Instruction::DrawCards {
                recipient: None,
                count,
                source_block,
            },
        ] if *source_block == block && *count == 1.into() => false,
        [Instruction::OptionalDrawCards { source_block }] if *source_block == block => true,
        _ => return Err(shape_error()),
    };
    let base = format!("/faces/0/abilities/{block}");
    graph.ability(&base, "triggered", "trigger-controller");
    let trigger = format!("{base}/trigger");
    let what_path = format!("{trigger}/what");
    let draw = format!("{base}/effects/0/steps/0");
    let you = graph.id(&base, "you");
    let actor = json!({"player":"you"});
    let mut step = json!({"effect":"draw","who":actor,"amount":{"amount":"fixed","value":1}});
    if optional {
        step["optional"] = json!(true);
    }
    let mut raw = json!({"kind":"triggered","effects":[{"steps":[step]}]});
    let (prefix, what, input, description) = match (&ability.condition, &ability.intervening_if) {
        (
            TriggerCondition::ControlledCreatureEntersWithPower {
                minimum_power,
                other,
            },
            None,
        ) => {
            let what = power_subject(minimum_power, true, *other)?;
            let query = graph.selection(&base, &what_path, &what, "one", "trigger-match");
            let article = if *other { "another" } else { "a" };
            let prefix = format!(
                "Whenever {article} creature you control with power {minimum_power} or greater enters"
            );
            let filter = what["filter"].clone();
            (prefix, what, query, filter)
        }
        (
            TriggerCondition::SelfEntersBattlefield,
            Some(InterveningCondition::ControllerHasCreatureWithPower { minimum_power }),
        ) if !optional => {
            let what = json!({"ref":"this","noun":"enchantment"});
            let source = graph.id("/source", "object");
            graph.reference(&what_path, &what, &source, "201.5");
            let condition = power_subject(minimum_power, false, false)?;
            graph.selection(
                &base,
                &format!("{base}/condition/what"),
                &condition,
                "one",
                "resolution",
            );
            graph.reference(&format!("{base}/condition/who"), &actor, &you, "109.5");
            raw["condition"] = json!({"condition":"control","who":actor,"what":condition});
            let prefix = format!(
                "When this enchantment enters, if you control a creature with power {minimum_power} or greater"
            );
            (prefix, what, source, json!({"types":["enchantment"]}))
        }
        _ => return Err(shape_error()),
    };
    raw["trigger"] = json!({"trigger":"enters","what":what});
    graph.event(&base, &trigger, "trigger:enters", &input, false);
    graph.result(&trigger, "objects", "same-objects", Some(&description));
    graph.reference(&format!("{draw}/who"), &actor, &you, "109.5");
    graph.event(&base, &draw, "draw", &you, optional);
    graph.result(
        &draw,
        "objects",
        "new-objects",
        Some(&json!({"cls":"card"})),
    );
    graph.result(&draw, "quantity", "measured-quantity", None);
    let may = if optional { "you may " } else { "" };
    Ok((format!("{prefix}, {may}draw a card."), raw))
}

fn static_source(
    graph: &mut Bindings<'_>,
    block: u32,
    ability: &StaticAbility,
) -> Result<(String, Value), Failure> {
    let StaticEffect::ModifyCreatures {
        scope: CreatureScope::Controller,
        keywords,
        ..
    } = &ability.effect
    else {
        return Err(shape_error());
    };
    let (keyword, rule) = match keywords.as_slice() {
        [KeywordAbility::Trample] => ("trample", "702.19"),
        [KeywordAbility::Haste] => ("haste", "702.10"),
        _ => return Err(shape_error()),
    };
    let base = format!("/faces/0/abilities/{block}");
    graph.ability(&base, "static", "static-source-controller");
    let what = json!({"ref":"each","filter":{"types":["creature"],"control":"you"}});
    let query = graph.selection(
        &base,
        &format!("{base}/effect/what"),
        &what,
        "many",
        "continuous",
    );
    let gained = format!("{base}/effect/modifiers/0/abilities/0");
    let recipient = graph.node(
        &gained,
        "recipient",
        "recipient",
        json!({"scope":graph.id(&base,"ability"),
        "of":query,"each":true,"rules":["113.10","201.5a"]}),
    );
    let scope = graph.node(
        &gained,
        "ability",
        "ability",
        json!({"source":recipient,
        "face":graph.id("/faces/0","face"),"abilityKind":"keywords","you":graph.id(&gained,"you"),
        "rules":["113","113.10"]}),
    );
    graph.node(
        &gained,
        "you",
        "runtime-player",
        json!({"scope":scope,"role":"static-source-controller",
        "rules":["109.5","301.5d","303.4e"]}),
    );
    let path = format!("{gained}/keyword");
    let actor = graph.node(
        &path,
        "you",
        "runtime-player",
        json!({"scope":scope,
        "role":"static-source-controller","rules":["109.5",rule]}),
    );
    graph.node(
        &path,
        "keyword",
        "keyword-ability",
        json!({"scope":scope,"keyword":keyword,
        "abilities":["static"],"you":actor,"rules":[rule]}),
    );
    let raw = json!({"kind":"static","effect":{"static":"modify","what":what,
        "modifiers":[{"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":keyword}}]}]}});
    Ok((format!("Creatures you control have {keyword}."), raw))
}

pub(super) fn validate_source(
    d: &Definition,
    labels: &SourceLabels,
    original: &[Box<RawValue>],
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !d
        .capabilities
        .contains(&Capability::PowerFilteredCreatureEntryV1)
    {
        // A partial AST must not drop the printed filter and reach a legacy trigger.
        if labels.blocks.iter().any(|line| is_entry_draw_line(line)) {
            return Err(shape_error());
        }
        return Ok(());
    }
    if d.triggered.len() + d.static_abilities.len() != original.len() {
        return Err(shape_error());
    }

    let mut graph = Bindings {
        identity: &d.oracle_id,
        nodes: vec![],
        references: vec![],
    };
    let source_id = graph.node(
        "/source",
        "object",
        "source",
        json!({"identity":d.oracle_id,"rules":["201.5"]}),
    );
    graph.node(
        "/faces/0",
        "face",
        "face",
        json!({
            "source": source_id, "sourceFaceIndex": 0, "name": source["name"],
            "layout": source["layout"], "typeLine": source["type_line"],
            "manaCost": source["mana_cost"], "rules": ["712"]
        }),
    );
    let check = |block: u32, (text, raw): (String, Value)| -> Result<(), Failure> {
        let i = block as usize;
        if labels.blocks[i] != text || decode::<Value>(original[i].get(), "abilities")? != raw {
            return Err(shape_error());
        }
        Ok(())
    };
    for (&block, ability) in labels.triggered.iter().zip(&d.triggered) {
        check(block, trigger_source(&mut graph, block, ability)?)?;
    }
    for (&block, ability) in labels.statics.iter().zip(&d.static_abilities) {
        check(block, static_source(&mut graph, block, ability)?)?;
    }
    graph.validate(binding)
}

#[cfg(test)]
#[test]
fn power_entry_draw() {
    // Exercise the typed adapter even without the private archive.
    let subject = json!({"ref":"filter","filter":{"types":["creature"],"control":"you",
        "properties":[{"property":"power","comparison":{"op":"ge","value":{"amount":"fixed","value":7}}}]},
        "count":{"amount":"fixed","value":1}});
    let draw: Sentence = serde_json::from_value(json!({"steps":[
        {"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}
    ]}))
    .unwrap();
    let lower_subject = |event: &str, subject: &Value| {
        let trigger = serde_json::from_value(json!({"trigger":event,"what":subject}))
            .map_err(|_| shape_error())?;
        lower_trigger(&trigger, std::slice::from_ref(&draw), None, 2)
    };
    let trigger = lower_subject("enters", &subject).unwrap();
    assert_eq!(
        trigger.condition,
        TriggerCondition::ControlledCreatureEntersWithPower {
            minimum_power: 7.into(),
            other: false
        }
    );
    assert_eq!(
        trigger.program.instructions,
        [Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 2
        }]
    );
    assert!(lower_subject("dies", &subject).is_err());
    for (path, value) in [
        ("/filter/properties/0/comparison/op", json!("gt")),
        ("/filter/properties/0/property", json!("toughness")),
        (
            "/filter/properties",
            json!([
                subject["filter"]["properties"][0],
                subject["filter"]["properties"][0]
            ]),
        ),
        ("/filter/control", json!("opponent")),
        ("/count/value", json!(2)),
    ] {
        let mut changed = subject.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(lower_subject("enters", &changed).is_err(), "{path}");
    }

    let self_entry = SourceTrigger::Enters {
        what: TriggerObject::This {
            noun: "enchantment".into(),
        },
    };
    let condition = |what: &Value| -> SourceCondition {
        serde_json::from_value(json!({"condition":"control","who":{"player":"you"},"what":what}))
            .unwrap()
    };
    // The controller is bound by the condition, not by the subject's filter.
    assert!(
        lower_trigger(
            &self_entry,
            std::slice::from_ref(&draw),
            Some(&condition(&subject)),
            0
        )
        .is_err()
    );
    let mut controlled = subject.clone();
    controlled["filter"]
        .as_object_mut()
        .unwrap()
        .remove("control");
    let self_draw = lower_trigger(&self_entry, &[draw], Some(&condition(&controlled)), 0).unwrap();
    assert_eq!(self_draw.condition, TriggerCondition::SelfEntersBattlefield);
    assert_eq!(
        self_draw.intervening_if,
        Some(InterveningCondition::ControllerHasCreatureWithPower {
            minimum_power: 7.into()
        })
    );

    let Some(path) = std::env::var_os("MTG_POWER_ENTRY_DRAW_ARCHIVE_PROBE") else {
        return;
    };
    let rows = std::fs::read_to_string(path).unwrap();
    // Archive line, SHA-256 of exact record, threshold, 'other', optional,
    // filtered trigger source block, optional static keyword.
    let expected = [
        (
            2923,
            "e137e566b5055f3fcdccbdef0a8ce692f43582b0ab1ae679178b624d4fe2cc14",
            3,
            true,
            true,
            0,
            None,
        ),
        (
            7340,
            "c89a02d95ed20698b957461693af38e487fc54206f8319411ce2928f0adf5ca8",
            4,
            false,
            false,
            2,
            Some(KeywordAbility::Trample),
        ),
        (
            32872,
            "e2b58aee58edd73984e179abd2596f6d1d71a8b9c678e02975318e377acc278a",
            3,
            false,
            false,
            0,
            None,
        ),
        (
            34766,
            "1b51d7b3f22d6839fd36ba0f27a2a6f8a549d36debe47ec6bc1856aa41b00c7b",
            4,
            false,
            true,
            1,
            Some(KeywordAbility::Haste),
        ),
    ];
    assert_eq!(rows.lines().count(), expected.len());
    for (row, (line, digest, power, other, optional, block, keyword)) in rows.lines().zip(expected)
    {
        assert_eq!(sha256(row.as_bytes()), digest);
        let d = match compile(fixtures::MANIFEST, row) {
            CompileResult::Lowered { definition } => definition,
            rejected => panic!("line {line}: {rejected:?}"),
        };
        assert_eq!(d.provenance.source_index + 2, line);
        assert!(
            d.capabilities
                .contains(&Capability::PowerFilteredCreatureEntryV1)
        );
        let a = d.triggered.last().unwrap();
        assert_eq!(
            a.condition,
            TriggerCondition::ControlledCreatureEntersWithPower {
                minimum_power: power.into(),
                other
            }
        );
        assert_eq!(a.intervening_if, None);
        assert_eq!(
            a.program.instructions,
            [if optional {
                Instruction::OptionalDrawCards {
                    source_block: block,
                }
            } else {
                Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: block,
                }
            }]
        );
        assert_eq!(d.static_abilities.len(), usize::from(keyword.is_some()));
        if let Some(keyword) = keyword {
            assert_eq!(
                d.static_abilities[0].effect,
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::Controller,
                    power: 0.into(),
                    toughness: 0.into(),
                    keywords: vec![keyword],
                    restrictions: vec![]
                }
            );
        }
        if keyword == Some(KeywordAbility::Trample) {
            assert_eq!(d.triggered.len(), 2);
            let self_entry = &d.triggered[0];
            assert_eq!(
                self_entry.condition,
                TriggerCondition::SelfEntersBattlefield
            );
            assert_eq!(
                self_entry.intervening_if,
                Some(InterveningCondition::ControllerHasCreatureWithPower {
                    minimum_power: 4.into()
                })
            );
            assert_eq!(
                self_entry.program.instructions,
                [Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 0
                }]
            );
        } else {
            assert_eq!(d.triggered.len(), 1);
        }
        let rejected = |change: &dyn Fn(&mut Value)| {
            let changed = tests::changed_source(row, change);
            let result = compile(fixtures::MANIFEST, &changed);
            assert!(
                matches!(result, CompileResult::Unsupported { .. }),
                "line {line}: {result:?}"
            );
        };
        let block = block as usize;
        for (pointer, value) in [
            ("/trigger/what/filter/nonToken", json!(false)),
            ("/effects/0/steps/0/who/of", Value::Null),
            (
                "/trigger/what/filter/properties/0/comparison/value/value",
                json!(9),
            ),
        ] {
            rejected(&|r| {
                let result = &mut r["faces"][0]["result"];
                let a = &mut result["abilities"][block];
                let (parent, field) = pointer.rsplit_once('/').unwrap();
                a.pointer_mut(parent).unwrap()[field] = value.clone();
                result["lines"][block]["ability"] = a.clone();
            });
        }
        rejected(&|r| {
            let refs = r["binding"]["references"].as_array_mut().unwrap();
            let path = format!("/faces/0/abilities/{block}/trigger/what/filter/control");
            refs.iter_mut().find(|r| r["path"] == path).unwrap()["target"] = json!("foreign");
        });
        if optional {
            rejected(&|r| {
                let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
                nodes.iter_mut().find(|n| n["operation"] == "draw").unwrap()["decider"] =
                    json!("foreign");
            });
        }
        if keyword == Some(KeywordAbility::Trample) {
            rejected(&|r| {
                let result = &mut r["faces"][0]["result"];
                result["abilities"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("condition");
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            rejected(&|r| {
                let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
                nodes.iter_mut().find(|n| n["node"] == "recipient").unwrap()["of"] =
                    json!("foreign");
            });
        }
    }
}

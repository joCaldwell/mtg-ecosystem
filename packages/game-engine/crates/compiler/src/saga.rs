//! Complete single-face chapter source and its ordered controller bindings.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn bind(
    source: &Value,
    id: &str,
    binding: &Value,
    parsed: &ParseResult,
    abilities: &[Ability],
) -> Result<Option<Vec<mtg_contracts::LifeChapter>>, Failure> {
    let has_chapter = abilities
        .iter()
        .any(|a| matches!(a, Ability::Chapter { .. }))
        || binding["nodes"]
            .as_array()
            .is_some_and(|nodes| nodes.iter().any(|n| n["abilityKind"] == "chapter"));
    if !has_chapter && source["layout"] != "saga" {
        return Ok(None);
    }
    let invalid = || {
        unsupported(
            "abilities.chapter",
            "requires a complete supported ordinary chapter program",
        )
    };
    if source["type_line"] != "Enchantment — Saga" || source["layout"] != "saga" {
        return Err(invalid());
    }
    let text = source["oracle_text"].as_str().ok_or_else(invalid)?;
    let chapters = mtg_contracts::life_chapters(text).map_err(|_| invalid())?;
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if chapters.len() != abilities.len()
        || raw != Value::Array(chapters.iter().map(|c| c.ast()).collect())
        || !parsed
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .eq(mtg_contracts::chapter_lines(text).lines())
    {
        return Err(invalid());
    }
    let mut graph = Bindings::new(id, source);
    for (chapter, ability) in chapters.iter().zip(abilities) {
        let Ability::Chapter {
            chapters: numbers,
            effects,
        } = ability
        else {
            return Err(invalid());
        };
        if !chapter
            .numbers
            .iter()
            .map(|n| n.get())
            .eq(numbers.iter().copied())
            || effects.len() != 1
        {
            return Err(invalid());
        }
        let base = format!("/faces/0/abilities/{}", chapter.source_block);
        let step = format!("{base}/effects/0/steps/0");
        let you = graph.id(&base, "you");
        graph.ability(&base, "chapter", "trigger-controller");
        let scope = graph.id(&base, "ability");
        let event = |g: &mut Bindings<'_>, path: &str, scope: &str, op: &str, input: &str| {
            g.node(path,"event","event",json!({"scope":scope,"operation":op,"inputs":[input],"availability":"when-performed","rules":["608.2c"]}));
        };
        let you_ref = |g: &mut Bindings<'_>, path: &str| {
            g.reference(path, &json!({"player":"you"}), &you, "109.5")
        };
        match chapter.effect {
            mtg_contracts::ChapterEffect::Fixed(ref fixed) => {
                bind_fixed_chapter(
                    &mut graph,
                    &base,
                    &scope,
                    &you,
                    fixed,
                    &raw[chapter.source_block as usize],
                )?;
            }
            mtg_contracts::ChapterEffect::Gain { .. } => {
                you_ref(&mut graph, &format!("{step}/who"));
                event(&mut graph, &step, &scope, "gain-life", &you);
                graph.result(&step, "quantity", "measured-quantity", None);
            }
            mtg_contracts::ChapterEffect::DrawLose { .. } => {
                you_ref(&mut graph, &format!("{step}/who"));
                event(&mut graph, &step, &scope, "draw", &you);
                graph.node(&step,"objects","event-result",json!({"event":graph.id(&step,"event"),"result":"objects","cardinality":"many",
                    "availability":"if-produced","identity":"new-objects","description":{"cls":"card"},"rules":["608.2h"]}));
                graph.result(&step, "quantity", "measured-quantity", None);
                let loss = format!("{base}/effects/0/steps/1");
                you_ref(&mut graph, &format!("{loss}/who"));
                event(&mut graph, &loss, &scope, "lose-life", &you);
                graph.result(&loss, "quantity", "measured-quantity", None);
            }
            mtg_contracts::ChapterEffect::DelayedEntry { .. } => {
                let delayed = graph.id(&step, "delayed-triggered");
                graph.node(
                    &step,
                    "delayed-triggered",
                    "ability",
                    json!({"source":graph.id("/source","object"),"face":graph.id("/faces/0","face"),
                    "abilityKind":"delayed-triggered","you":you,"rules":["603.7"]}),
                );
                let trigger = format!("{step}/trigger");
                let what = format!("{trigger}/what");
                let query = graph.id(&what, "query");
                let filter = json!({"types":["creature"],"control":"you"});
                let expression =
                    json!({"ref":"filter","filter":filter,"count":{"amount":"fixed","value":1}});
                you_ref(&mut graph, &format!("{what}/filter/control"));
                graph.node(&what,"query","selection",json!({"scope":delayed,"expression":expression,"cardinality":"one","timing":"trigger-match","selection":"evaluate-reference","rules":["608.2h"]}));
                graph.reference(&what, &expression, &query, "608.2h");
                event(&mut graph, &trigger, &delayed, "trigger:enters", &query);
                graph.result(&trigger, "objects", "same-objects", Some(&filter));
                let loss = format!("{step}/effects/0/steps/0");
                let who = format!("{loss}/who");
                let opponents = graph.id(&who, "query");
                let selector = json!({"player":"each-opponent"});
                graph.node(&who,"query","selection",json!({"scope":delayed,"expression":selector,"cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
                graph.reference(&who, &selector, &opponents, "608.2h");
                event(&mut graph, &loss, &delayed, "lose-life", &opponents);
                graph.node(&loss,"quantity","event-result",json!({"event":graph.id(&loss,"event"),"result":"quantity","cardinality":"unknown","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}));
                let gain = format!("{step}/effects/0/steps/1");
                you_ref(&mut graph, &format!("{gain}/who"));
                event(&mut graph, &gain, &delayed, "gain-life", &you);
                graph.result(&gain, "quantity", "measured-quantity", None);
            }
        }
    }
    graph.validate_ordered(
        binding,
        "requires exact complete chapter actor and event graph",
    )?;
    Ok(Some(chapters))
}

fn bind_fixed_chapter(
    graph: &mut Bindings<'_>,
    base: &str,
    scope: &str,
    you: &str,
    effect: &FixedChapter,
    raw: &Value,
) -> Result<(), Failure> {
    let step = format!("{base}/effects/0/steps/0");
    let player = json!({"player":"you"});
    let event = |g: &mut Bindings<'_>, path: &str, operation: &str, input: &str| {
        g.node(
            path,
            "event",
            "event",
            json!({"scope":scope,"operation":operation,
            "inputs":[input],"availability":"when-performed","rules":["608.2c"]}),
        );
    };
    let target = |g: &mut Bindings<'_>, key: &str| -> Result<String, Failure> {
        let path = format!("{step}/{key}");
        let expr = &raw["effects"][0]["steps"][0][key];
        let selection = expr["selectionId"].as_str().ok_or_else(|| {
            unsupported(
                "binding",
                "chapter target requires its exact announced identity",
            )
        })?;
        g.reference(&format!("{path}/filter/control"), &player, you, "109.5");
        Ok(g.target(scope, &path, expr, selection))
    };
    match effect {
        FixedChapter::Token { .. } => {
            graph.reference(&format!("{step}/who"), &player, you, "109.5");
            event(graph, &step, "create-token", you);
            graph.result(
                &step,
                "objects",
                "new-objects",
                Some(&json!({"cls":"token","types":["creature"]})),
            );
            graph.result(&step, "quantity", "measured-quantity", None);
        }
        FixedChapter::Counters { .. } => {
            graph.reference(&format!("{step}/who"), &player, you, "109.5");
            let chosen = target(graph, "on")?;
            event(graph, &step, "put-counters", &chosen);
            graph.result(&step, "quantity", "measured-quantity", None);
        }
        FixedChapter::Trample { .. } => {
            let chosen = target(graph, "what")?;
            event(graph, &step, "pump", &chosen);
            let grant = format!("{base}/effects/0/steps/1");
            let what = format!("{grant}/what");
            graph.reference(&format!("{what}/filter/control"), &player, you, "109.5");
            graph.reference(
                &what,
                &raw["effects"][0]["steps"][1]["what"],
                &chosen,
                "601.2c",
            );
            let path = format!("{grant}/abilities/0");
            let recipient = graph.node(
                &path,
                "recipient",
                "recipient",
                json!({
                    "scope":scope,"of":chosen,"each":false,"rules":["113.10","201.5a"]
                }),
            );
            let grant_scope = graph.node(
                &path,
                "ability",
                "ability",
                json!({
                    "source":recipient,"face":graph.id("/faces/0","face"),
                    "abilityKind":"keywords","you":graph.id(&path,"you"),"rules":["113","113.10"]
                }),
            );
            graph.node(&path, "you", "runtime-player", json!({
                "scope":grant_scope,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]
            }));
            let keyword = format!("{path}/keyword");
            let keyword_you = graph.node(
                &keyword,
                "you",
                "runtime-player",
                json!({
                    "scope":grant_scope,"role":"static-source-controller","rules":["109.5","702.19"]
                }),
            );
            graph.node(
                &keyword,
                "keyword",
                "keyword-ability",
                json!({"scope":grant_scope,
                "keyword":"trample","abilities":["static"],"you":keyword_you,"rules":["702.19"]}),
            );
            event(graph, &grant, "gain-abilities", &chosen);
        }
    }
    Ok(())
}

#[test]
fn fixed_saga_chapters_keep_tokens_counters_and_grant_in_their_blocks() {
    const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-phase1-r3.json");
    const ROW: &str = include_str!("../../../tests/fixtures/phase1-r3/origin-of-the-hulk.json");
    let result = compile(HEADER, ROW);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let [first, second, third] = definition.triggered.as_slice() else {
        panic!("expected three separate chapters");
    };
    assert!(matches!(
        first.program.instructions.as_slice(),
        [Instruction::CreateCreatureTokens {
            count: 1,
            source_block: 0,
            ..
        }]
    ));
    assert!(matches!(second.program.instructions.as_slice(), [
        Instruction::PutPlusOneCounters { amount, source_block: 1, .. }
    ] if *amount == Quantity::from(2)));
    assert!(matches!(
        third.program.instructions.as_slice(),
        [Instruction::GrantKeywords {
            modifier: Some(_),
            source_block: 2,
            ..
        }]
    ));
    assert!(saga_targeted_keyword_program(&third.program));
    assert_eq!(
        third.program.targets[0].domain,
        TargetDomain::ControlledBattlefieldCreature
    );

    let forged_binding = tests::changed_source(ROW, |row| {
        let nodes = row["binding"]["nodes"].as_array_mut().unwrap();
        let target = nodes
            .iter_mut()
            .find(|node| node["node"] == "selection")
            .unwrap();
        target["timing"] = json!("resolution");
    });
    assert!(
        matches!(compile(HEADER, &forged_binding), CompileResult::Unsupported { path, .. }
        if path == "binding")
    );

    let mut forged_ir = *definition;
    forged_ir.triggered[2].program.targets[0].domain = TargetDomain::BattlefieldCreature;
    forged_ir.capabilities = forged_ir.requirements();
    assert!(forged_ir.validate().is_err());
}

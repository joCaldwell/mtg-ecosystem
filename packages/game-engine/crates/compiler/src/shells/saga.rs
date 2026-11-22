//! Complete single-face chapter source and its ordered controller bindings.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(crate) fn bind(
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
            chapters: numbers, ..
        } = ability
        else {
            return Err(invalid());
        };
        if !chapter
            .numbers
            .iter()
            .map(|n| n.get())
            .eq(numbers.iter().copied())
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
            mtg_contracts::ChapterEffect::Impulse { .. } => {
                impulse_play::bind_block(&mut graph, &base, &raw[chapter.source_block as usize]);
            }
            mtg_contracts::ChapterEffect::DelayedCastCounter => {
                bind_delayed_cast(
                    &mut graph,
                    &step,
                    &you,
                    &raw[chapter.source_block as usize]["effects"][0]["steps"][0],
                )?;
            }
            mtg_contracts::ChapterEffect::Ordinary(ref effect) => {
                bind_ordinary_chapter(
                    &mut graph,
                    &base,
                    effect,
                    &raw[chapter.source_block as usize]["effects"][0]["steps"][0],
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

fn bind_delayed_cast(
    graph: &mut Bindings<'_>,
    step: &str,
    you: &str,
    raw: &Value,
) -> Result<(), Failure> {
    let delayed = graph.node(
        step,
        "delayed-triggered",
        "ability",
        json!({"source":graph.id("/source","object"),"face":graph.id("/faces/0","face"),
            "abilityKind":"delayed-triggered","you":you,"rules":["603.7"]}),
    );
    let event = |g: &mut Bindings<'_>, at: &str, op: &str, input: &str| {
        g.node(
            at,
            "event",
            "event",
            json!({"scope":delayed,"operation":op,"inputs":[input],
            "availability":"when-performed","rules":["608.2c"]}),
        );
    };
    let trigger = format!("{step}/trigger");
    let player = json!({"player":"you"});
    graph.reference(&format!("{trigger}/who"), &player, you, "109.5");
    event(graph, &trigger, "trigger:cast", you);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"cls":"spell"})),
    );

    let child = format!("{step}/effects/0/steps/0");
    let target = format!("{child}/on");
    let expr = &raw["effects"][0]["steps"][0]["on"];
    let selection = expr["selectionId"]
        .as_str()
        .ok_or_else(|| unsupported("binding", "missing delayed cast target"))?;
    graph.reference(&format!("{child}/who"), &player, you, "109.5");
    graph.reference(&format!("{target}/filter/control"), &player, you, "109.5");
    let selected = graph.target(&delayed, &target, expr, selection);
    event(graph, &child, "put-counters", &selected);
    graph.result(&child, "quantity", "measured-quantity", None);
    Ok(())
}

fn bind_ordinary_chapter(
    graph: &mut Bindings<'_>,
    base: &str,
    effect: &OrdinaryChapter,
    raw: &Value,
) -> Result<(), Failure> {
    let step = format!("{base}/effects/0/steps/0");
    let scope = graph.id(base, "ability");
    let you = graph.id(base, "you");
    let damage = matches!(effect, OrdinaryChapter::DamageOpponentCreature(_));
    let (op, target_path, expr) = match effect {
        OrdinaryChapter::DamageOpponentCreature(_) => {
            let source = graph.id("/source", "object");
            graph.reference(&format!("{step}/source"), &raw["source"], &source, "201.5");
            ("damage", "to/ref", &raw["to"]["ref"])
        }
        OrdinaryChapter::DestroyOpponentArtifact => ("destroy", "what", &raw["what"]),
        OrdinaryChapter::FixedMana { .. } => {
            graph.reference(&format!("{step}/who"), &raw["who"], &you, "109.5");
            graph.event(base, &step, "add-mana", &you, false);
            return Ok(());
        }
    };
    let target_path = format!("{step}/{target_path}");
    let control = format!("{target_path}/filter/control");
    let opponents = graph.node(
        &control,
        "opponents",
        "derived-player",
        json!({"scope":scope,"role":"opponents","of":you,"rules":["102.2","109.5"]}),
    );
    graph.reference(
        &control,
        &json!({"implicit":"opponents"}),
        &opponents,
        "102.2",
    );
    let selection = expr["selectionId"]
        .as_str()
        .ok_or_else(|| unsupported("binding", "missing chapter target selection"))?;
    let chosen = graph.target(&scope, &target_path, expr, selection);
    if damage {
        graph.node(
            &step,
            "event",
            "event",
            json!({"scope":scope,"operation":op,"inputs":[graph.id("/source","object"),chosen],
                "availability":"when-performed","rules":["608.2c"]}),
        );
    }
    let participant = if damage { 1 } else { 0 };
    graph.node(
        &format!("{step}/participant-{participant}"),
        "controller-of-described",
        "derived-player",
        json!({"scope":scope,"role":"controller","of":chosen,"rules":["108.4"]}),
    );
    if damage {
        graph.result(&step, "quantity", "measured-quantity", None);
    } else {
        graph.event(base, &step, op, &chosen, false);
    }
    Ok(())
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
    const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
    const ROW: &str = include_str!("../../../../tests/fixtures/phase1-r4/origin-of-the-hulk.json");
    let result = compile(HEADER, ROW);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let [first, second, third] = definition.triggered.as_slice() else {
        panic!("expected three separate chapters");
    };
    assert!(matches!(
        first.program.instructions.as_slice(),
        [i] if i.creature_token().is_some()
            && i.created_tokens().is_some_and(|(_, count)| count == 1)
            && i.source_block() == 0
    ));
    assert!(matches!(second.program.instructions.as_slice(), [
        kernel!(PutCounters { counter: kir::CounterKind::PlusOnePlusOne, amount: fixed!(amount), .. } @ 1)
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

#[test]
fn shared_chapter_mana_damage_and_destroy_use_exact_operands() {
    const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
    const ROW: &str =
        include_str!("../../../../tests/fixtures/phase1-r4/burn-burn-tree-and-fern.json");
    let result = compile(HEADER, ROW);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let [damage, destroy, third, fourth] = definition.triggered.as_slice() else {
        panic!("expected four independent chapter abilities");
    };
    assert!(saga_damage_target(&damage.program.targets[0]));
    assert!(matches!(
        destroy.program.instructions.as_slice(),
        [kernel!(Destroy { what: target_object!(0), regeneration_forbidden: false } @ 1)]
    ));
    assert_eq!(third.program, fourth.program);
    assert_ne!(third.condition, fourth.condition);
    let forged = tests::changed_source(ROW, |row| {
        let nodes = row["binding"]["nodes"].as_array_mut().unwrap();
        nodes.iter_mut().find(|n| n["role"] == "opponents").unwrap()["role"] = json!("controller");
    });
    assert!(
        matches!(compile(HEADER, &forged), CompileResult::Unsupported { path, .. } if path == "binding")
    );
    let mut forged_ir = (*definition).clone();
    forged_ir.triggered[3].condition = third.condition.clone();
    assert!(forged_ir.validate().is_err());
    let mut forged_ir = *definition;
    forged_ir.triggered[0].program.targets[0]
        .filter
        .as_mut()
        .unwrap()
        .opponent = false;
    assert!(forged_ir.validate().is_err());
}

#[test]
fn shared_chapter_mana_and_exile_programs_bind_the_delayed_cast_separately() {
    const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
    const ROW: &str =
        include_str!("../../../../tests/fixtures/phase1-r4/showdown-of-the-skalds.json");
    let result = compile(HEADER, ROW);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let [exile, second, third] = definition.triggered.as_slice() else {
        panic!("expected three chapters")
    };
    assert!(matches!(
        exile.program.instructions.as_slice(),
        [Instruction::ExileLibraryForPlay {
            count: 4,
            window: PlayWindow::ThroughNextControllerTurn,
            source_block: 0
        }]
    ));
    assert_eq!(second.program, third.program);
    let [
        Instruction::RegisterControllerSpellCastThisTurn {
            program,
            source_block: 1,
        },
    ] = second.program.instructions.as_slice()
    else {
        panic!("expected a new deferred target occurrence");
    };
    assert!(second.program.targets.is_empty());
    assert_eq!(
        program.targets[0].domain,
        TargetDomain::ControlledBattlefieldCreature
    );
    assert_eq!(program.targets[0].selection_id, "line:1:21");
    let program = program.clone();
    let mut lowered = *definition;
    lowered.triggered[1].program.instructions[0] =
        Instruction::RegisterControllerCreatureEntryThisTurn {
            program,
            source_block: 1,
        };
    lowered.capabilities = lowered.requirements();
    assert!(lowered.validate().is_err());

    let forged = tests::changed_source(ROW, |row| {
        let references = row["binding"]["references"].as_array_mut().unwrap();
        references
            .iter_mut()
            .find(|row| row["path"].as_str().unwrap().ends_with("/trigger/who"))
            .unwrap()["target"] = json!("unbound");
    });
    assert!(!matches!(
        compile(HEADER, &forged),
        CompileResult::Lowered { .. }
    ));
}

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

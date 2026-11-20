//! Bind the live controller, exact source exclusion and continuous recipient
//! query of one printed stat modifier (CR 109.5, 201.5, 611.3a/b, 613.4c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    effect: &SourceStaticEffect,
    block: usize,
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<StaticAbility>, Failure> {
    if !matches!(
        kind,
        CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
    ) || !matches!(effect, SourceStaticEffect::Modify { .. })
    {
        return Ok(None);
    }
    let line = &result.lines[block];
    if mtg_contracts::scoped_static::printed(&line.text).is_some() {
        return Ok(None);
    }
    let Some((ability, _)) = mtg_contracts::scoped_creature_stats::printed(&line.text) else {
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "static.scope",
            "requires complete printed scoped creature stats and siblings",
        )
    };
    let mut graph = bindings::Bindings::new(identity, source);
    for (index, source_line) in result.lines.iter().enumerate() {
        let root = format!("/faces/0/abilities/{index}");
        let archived: Value = decode(source_line.ability.get(), "lines.ability")?;
        if let Some((_, ast)) = mtg_contracts::scoped_creature_stats::printed(&source_line.text)
            .filter(|_| mtg_contracts::scoped_static::printed(&source_line.text).is_none())
        {
            if archived != ast {
                return Err(fail());
            }
            bind_stat(&mut graph, &root, &ast);
        } else if let Some((keywords, ast)) =
            mtg_contracts::scoped_creature_stats::printed_keywords(&source_line.text)
        {
            if archived != ast {
                return Err(fail());
            }
            basic_landwalk::bind_block(&mut graph, &root, keywords.into_iter(), "static.scope")?;
        } else {
            return Err(fail());
        }
    }
    graph.validate(
        binding,
        "requires exact live controller, filters and intrinsic keywords",
    )?;
    Ok(Some(ability))
}

fn bind_stat(graph: &mut bindings::Bindings<'_>, root: &str, ast: &Value) {
    let path = format!("{root}/effect/what");
    let expression = &ast["effect"]["what"];
    graph.ability(root, "static", "static-source-controller");
    let query = graph.node(
        &path,
        "query",
        "selection",
        json!({
            "scope":graph.id(root,"ability"),"expression":expression,"cardinality":"many",
            "timing":"continuous","selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    if expression["filter"]["other"] == true {
        graph.reference(
            &format!("{path}/filter/other"),
            &json!({"ref":"self"}),
            &graph.id("/source", "object"),
            "201.5",
        );
    }
    graph.reference(
        &format!("{path}/filter/control"),
        &json!({"player":"you"}),
        &graph.id(root, "you"),
        "109.5",
    );
    graph.reference(&path, expression, &query, "608.2h");
}

#[cfg(test)]
#[test]
fn composition_wave_e_checks_signed_modifiers_and_intrinsic_siblings() {
    let header = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    for (raw, power, toughness) in [
        (
            include_str!("../../../tests/fixtures/fb1755a0-3334-419b-8cb5-5a3ac7fa5b13.json"),
            1,
            -1,
        ),
        (
            include_str!("../../../tests/fixtures/e220138c-5fc6-487d-9fa9-f21b2fb1f12c.json"),
            2,
            2,
        ),
        (
            include_str!("../../../tests/fixtures/06157e34-dbe1-4936-a718-3b1d70726794.json"),
            0,
            1,
        ),
    ] {
        let compiled = compile(header, raw);
        let CompileResult::Lowered { mut definition } = compiled else {
            panic!("{compiled:?}");
        };
        assert!(matches!(
            definition.static_abilities.as_slice(),
            [StaticAbility { effect: StaticEffect::ControllerScopedCreatureStats { power: p, toughness: t, .. }, .. }]
                if *p == Quantity::from(power) && *t == Quantity::from(toughness)
        ));
        if power == 0 {
            assert_eq!(definition.keywords, [KeywordAbility::Flying]);
            let bad_graph = super::tests::changed_source(raw, |row| {
                let nodes = row["binding"]["nodes"].as_array_mut().unwrap();
                let keyword = nodes
                    .iter_mut()
                    .find(|n| n["node"] == "keyword-ability")
                    .unwrap();
                keyword["keyword"] = "haste".into();
            });
            assert!(!matches!(
                compile(header, &bad_graph),
                CompileResult::Lowered { .. }
            ));

            definition.keywords.clear();
            definition.capabilities = definition.requirements();
            assert!(definition.validate().is_err());
        } else {
            definition.static_abilities.clear();
            definition.source_labels = None;
            definition.capabilities = definition.requirements();
            assert!(definition.validate().is_err());
        }
    }
}

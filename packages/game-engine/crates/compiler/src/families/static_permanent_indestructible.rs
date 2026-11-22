//! CR 109.5, 113.6, 611.3, 613.1f, 702.12 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn selector(effect: &SourceStaticEffect) -> Option<&Value> {
    let SourceStaticEffect::Modify { what, modifiers } = effect else {
        return None;
    };
    if *what != json!({"ref":"each","filter":{"other":true,"cls":"permanent","control":"you"}})
        || modifiers.as_slice()
            != [
                json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"indestructible"}}]}),
            ]
    {
        return None;
    }
    Some(what)
}

pub(crate) fn lower(effect: &SourceStaticEffect) -> Option<StaticAbility> {
    selector(effect)?;
    Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::GrantOtherControlledPermanentsIndestructible,
    })
}

pub(crate) fn bind(
    effect: &SourceStaticEffect,
    binding: &Value,
    card: &Source,
    block: usize,
    text: &str,
) -> Result<(), Failure> {
    let Some(what) = selector(effect) else {
        return Ok(());
    };
    let path = format!("/faces/0/abilities/{block}");
    if text != "Other permanents you control have indestructible." {
        return Err(unsupported(
            &path,
            "unsupported complete static indestructible sentence",
        ));
    }
    let root = format!("card:{}", card.oracle_id);
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let ability = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let query_path = format!("{path}/effect/what");
    let query = format!("{root}{query_path}#query");
    let gained_path = format!("{path}/effect/modifiers/0/abilities/0");
    let gained = format!("{root}{gained_path}#ability");
    let recipient = format!("{root}{gained_path}#recipient");
    let gained_you = format!("{root}{gained_path}#you");
    let keyword_path = format!("{gained_path}/keyword");
    let keyword_you = format!("{root}{keyword_path}#you");
    let expected_nodes = [
        json!({"id":source,"path":"/source","node":"source","identity":card.oracle_id}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
            "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost}),
        json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,
            "abilityKind":"static","you":you}),
        json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"static-source-controller"}),
        json!({"id":query,"path":query_path,"node":"selection","scope":ability,
            "expression":what,"cardinality":"many","timing":"continuous","selection":"evaluate-reference"}),
        json!({"id":recipient,"path":gained_path,"node":"recipient","scope":ability,"of":query,"each":true}),
        json!({"id":gained,"path":gained_path,"node":"ability","source":recipient,"face":face,
            "abilityKind":"keywords","you":gained_you}),
        json!({"id":gained_you,"path":gained_path,"node":"runtime-player","scope":gained,
            "role":"static-source-controller"}),
        json!({"id":keyword_you,"path":keyword_path,"node":"runtime-player","scope":gained,
            "role":"static-source-controller"}),
        json!({"id":format!("{root}{keyword_path}#keyword"),"path":keyword_path,"node":"keyword-ability",
            "scope":gained,"keyword":"indestructible","abilities":["static"],"you":keyword_you}),
    ];
    let nodes = binding["nodes"]
        .as_array()
        .ok_or_else(|| unsupported(&path, "missing static grant binding nodes"))?;
    if binding["identity"] != card.oracle_id
        || binding["artifactKind"] != "oracle-reference-binding"
        || binding["schemaVersion"] != 1
        || binding["executable"] != false
        || binding["rules"]["edition"] != RULES_EDITION
        || expected_nodes.iter().any(|expected| {
            let mut found = nodes.iter().filter(|node| node["id"] == expected["id"]);
            found.next().is_none_or(|node| {
                expected
                    .as_object()
                    .unwrap()
                    .iter()
                    .any(|(key, value)| node[key] != *value)
            }) || found.next().is_some()
        })
    {
        return Err(unsupported(
            &path,
            "unbound static indestructible source or recipient",
        ));
    }
    let effect_path = format!("{path}/effect/");
    let references = binding["references"]
        .as_array()
        .ok_or_else(|| unsupported(&path, "missing static grant binding references"))?
        .iter()
        .filter(|reference| {
            reference["path"]
                .as_str()
                .is_some_and(|p| p.starts_with(&effect_path))
        })
        .collect::<Vec<_>>();
    if references.len() != 3 {
        return Err(unsupported(
            &path,
            "unsupported static indestructible reference set",
        ));
    }
    for (reference_path, expression, target) in [
        (
            format!("{query_path}/filter/other"),
            json!({"ref":"self"}),
            source,
        ),
        (
            format!("{query_path}/filter/control"),
            json!({"player":"you"}),
            you,
        ),
        (query_path, what.clone(), query),
    ] {
        let mut found = references
            .iter()
            .filter(|reference| reference["path"] == reference_path);
        if !matches!(found.next(), Some(reference)
            if reference["binding"] == "bound"
                && reference["expression"] == expression
                && reference["target"] == target)
            || found.next().is_some()
        {
            return Err(unsupported(
                &reference_path,
                "unbound static indestructible selector",
            ));
        }
    }
    Ok(())
}

//! Whole static stack-spell grant binder; diagnostic fullyBound is not authority.
//! CR 109.5, 113.10, 601.2i, 611.3, 613.1f, 702.85 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    effect: &SourceStaticEffect,
    block: u32,
) -> Result<Option<StaticAbility>, Failure> {
    let SourceStaticEffect::Modify { what, modifiers } = effect else {
        return Ok(None);
    };
    if what["filter"]["cls"] != "spell" {
        return Ok(None);
    }
    let operand = &what["filter"]["properties"][0]["comparison"]["value"]["value"];
    let number = match operand {
        Value::Number(n) => Some(n),
        _ => None,
    };
    let minimum: Quantity = number
        .and_then(|n| n.to_string().parse().ok())
        .filter(|q: &Quantity| !q.is_negative())
        .ok_or_else(|| {
            unsupported(
                "static.spell-grant",
                "expected exact nonnegative fixed quantity",
            )
        })?;
    if *what
        != json!({"ref":"each","filter":{"cls":"spell","cast":{"by":{"player":"you"}},
        "properties":[{"property":"mana-value","comparison":{"op":"ge","value":{"amount":"fixed","value":operand}}}]}})
        || *modifiers
            != vec![
                json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"cascade"}}]}),
            ]
    {
        return Err(unsupported(
            "static.spell-grant",
            "unsupported complete selector or grant",
        ));
    }
    Ok(Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::GrantSpellCascade {
            mana_value_at_least: minimum,
            program: canonical_cascade_program(block),
        },
    }))
}

pub(super) fn bind(
    effect: &SourceStaticEffect,
    binding: &Value,
    identity: &str,
    block: u32,
) -> Result<(), Failure> {
    if lower(effect, block)?.is_none() {
        return Ok(());
    }
    let SourceStaticEffect::Modify { what, .. } = effect else {
        unreachable!()
    };
    let path = format!("/faces/0/abilities/{block}");
    let root = format!("card:{identity}");
    let ability = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let query = format!("{root}{path}/effect/what#query");
    let nested_path = format!("{path}/effect/modifiers/0/abilities/0");
    let nested = format!("{root}{nested_path}#ability");
    let recipient = format!("{root}{nested_path}#recipient");
    let keyword_you = format!("{root}{nested_path}/keyword#you");
    let nodes = binding["nodes"]
        .as_array()
        .ok_or_else(|| unsupported(&path, "missing grant binding nodes"))?;
    let required = [
        json!({"id":ability,"path":path,"node":"ability","abilityKind":"static","source":format!("{root}/source#object"),"you":you}),
        json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"static-source-controller"}),
        json!({"id":query,"path":format!("{path}/effect/what"),"node":"selection","scope":ability,"expression":what,"cardinality":"many","timing":"continuous","selection":"evaluate-reference"}),
        json!({"id":recipient,"path":nested_path,"node":"recipient","scope":ability,"of":query,"each":true}),
        json!({"id":nested,"path":nested_path,"node":"ability","source":recipient,"abilityKind":"keywords","you":format!("{root}{nested_path}#you")}),
        json!({"id":format!("{root}{nested_path}#you"),"path":nested_path,"node":"runtime-player","scope":nested,"role":"static-source-controller"}),
        json!({"id":keyword_you,"path":format!("{nested_path}/keyword"),"node":"runtime-player","scope":nested,"role":"trigger-controller"}),
        json!({"id":format!("{root}{nested_path}/keyword#keyword"),"path":format!("{nested_path}/keyword"),"node":"keyword-ability","scope":nested,"keyword":"cascade","abilities":["triggered"],"you":keyword_you}),
    ];
    if binding["identity"] != identity
        || required.iter().any(|expected| {
            let found: Vec<_> = nodes.iter().filter(|n| n["id"] == expected["id"]).collect();
            found.len() != 1
                || expected
                    .as_object()
                    .unwrap()
                    .iter()
                    .any(|(key, value)| found[0][key] != *value)
        })
    {
        return Err(unsupported(
            &path,
            "unbound static grant recipient/controller",
        ));
    }
    for (suffix, expression, target) in [
        ("/effect/what/filter/cast/by", json!({"player":"you"}), you),
        ("/effect/what", what.clone(), query),
    ] {
        let refs: Vec<_> = binding["references"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["path"] == format!("{path}{suffix}"))
            .collect();
        if refs.len() != 1
            || refs[0]["binding"] != "bound"
            || refs[0]["expression"] != expression
            || refs[0]["target"] != target
        {
            return Err(unsupported(&path, "unbound static grant selector"));
        }
    }
    Ok(())
}

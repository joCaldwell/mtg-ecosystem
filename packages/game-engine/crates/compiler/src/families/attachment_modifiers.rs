use super::*;
use serde_json::{Value, json};

pub(crate) fn lower(
    block: usize,
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
) -> Result<Option<StaticAbility>, Failure> {
    let line = &result.lines[block];
    let Some((ability, raw)) = mtg_contracts::attachment_modifiers::printed(&line.text) else {
        return Ok(None);
    };
    if decode::<Value>(line.ability.get(), "static modifier")? != raw {
        return Err(unsupported(
            "static modifier",
            "requires exact attached creature modifier",
        ));
    }
    let base = format!("/faces/0/abilities/{block}");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&base, "static", "static-source-controller");
    let scope = graph.id(&base, "ability");
    let effect = &raw["effect"];
    let (target, prefix, modifiers, keyword, rule) = if effect["static"] == "conditional" {
        let target = graph.attachment(
            &base,
            &format!("{base}/effect/condition/what"),
            &effect["condition"]["what"],
        );
        graph.reference_rules(
            &format!("{base}/effect/effect/what"),
            &effect["effect"]["what"],
            &target,
            &["608.2k", "400.7j", "608.2h"],
        );
        (
            target,
            format!("{base}/effect/effect"),
            &effect["effect"]["modifiers"],
            "hexproof",
            "702.11",
        )
    } else {
        let target = graph.attachment(&base, &format!("{base}/effect/what"), &effect["what"]);
        (
            target,
            format!("{base}/effect"),
            &effect["modifiers"],
            "protection",
            "702.16",
        )
    };
    if let Some(index) = modifiers
        .as_array()
        .and_then(|mods| mods.iter().position(|m| m["modifier"] == "abilities"))
    {
        let path = format!("{prefix}/modifiers/{index}/abilities/0");
        let recipient = graph.node(
            &path,
            "recipient",
            "recipient",
            json!({"scope":scope,"of":target,"each":false,"rules":["113.10","201.5a"]}),
        );
        let grant = graph.node(
            &path,
            "ability",
            "ability",
            json!({"source":recipient,"face":graph.id("/faces/0","face"),
            "abilityKind":"keywords","you":graph.id(&path,"you"),"rules":["113","113.10"]}),
        );
        graph.node(&path,"you","runtime-player",json!({"scope":grant,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]}));
        let kw = format!("{path}/keyword");
        let you = graph.node(
            &kw,
            "you",
            "runtime-player",
            json!({"scope":grant,"role":"static-source-controller","rules":["109.5",rule]}),
        );
        graph.node(&kw,"keyword","keyword-ability",json!({"scope":grant,"keyword":keyword,"abilities":["static"],"you":you,"rules":[rule]}));
    }
    graph.validate_block(
        binding,
        &base,
        "incomplete attached creature modifier graph",
    )?;
    Ok(Some(ability))
}

#[test]
fn conditional_equipped_legendary_hexproof_requires_the_printed_gate() {
    let manifest = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    let source =
        include_str!("../../../../tests/fixtures/c01aafb8-3da5-4eb1-8731-2a223e747d63.json");
    let CompileResult::Lowered { definition } = compile(manifest, source) else {
        panic!("{:?}", compile(manifest, source));
    };
    assert_eq!(definition.static_abilities.len(), 2);
    assert_eq!(definition.activated.len(), 1);
    assert!(mtg_contracts::attachment_modifiers::conditional_hexproof(
        &definition.static_abilities[1].effect
    ));
    let mut omitted = (*definition).clone();
    omitted.static_abilities.pop();
    omitted.capabilities = omitted.requirements();
    assert!(omitted.validate().is_err());

    let mut changed = *definition;
    let StaticEffect::ModifyCreatures { power, .. } = &mut changed.static_abilities[0].effect
    else {
        panic!("missing printed stats");
    };
    *power = 9.into();
    assert!(changed.validate().is_err());
    *changed.static_abilities.first_mut().unwrap() = omitted.static_abilities[0].clone();
    changed.activated[0].cost = ActivationCost::Mana {
        cost: ManaCost {
            generic: 0.into(),
            colored: vec![],
        },
    };
    assert!(changed.validate().is_err());
}

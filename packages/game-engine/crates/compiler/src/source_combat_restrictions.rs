//! CR 509.1b: bind the complete printed restriction and its archived operands.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use mtg_contracts::source_combat_restrictions::{
    flying_attackers, printed_restriction, restriction_ast, source_operand, static_ability,
};
use serde_json::{Value, json};

pub(super) fn restriction_from_ast(effect: &SourceStaticEffect) -> Option<CreatureRestriction> {
    let operand = source_operand();
    Some(match effect {
        SourceStaticEffect::Cant { what, action } if *what == operand && action == "be-blocked" => {
            CreatureRestriction::CantBeBlocked
        }
        SourceStaticEffect::CombatRule { what, rule }
            if *what == operand
                && *rule
                    == json!({
                        "rule":"blocking-filter","mode":"only","attackers":flying_attackers()
                    }) =>
        {
            CreatureRestriction::CanBlockOnlyFlying
        }
        _ => return None,
    })
}

pub(super) fn lower(
    effect: &SourceStaticEffect,
    block: usize,
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    kind: CardKind,
) -> Result<Option<StaticAbility>, Failure> {
    let Some(restriction) = restriction_from_ast(effect) else {
        return Ok(None);
    };
    let operand = source_operand();
    let fail = || {
        unsupported(
            "abilities.static",
            "requires exact printed source combat restriction",
        )
    };
    let ast: Vec<Value> = decode(result.abilities.get(), "abilities.static")?;
    let line = result.lines.get(block).ok_or_else(fail)?;
    let line_ast: Value = decode(line.ability.get(), "abilities.static")?;
    if kind != CardKind::Creature
        || source["layout"] != "normal"
        || ast.get(block) != Some(&line_ast)
        || restriction_ast(restriction) != Some(line_ast)
        || printed_restriction(&line.text) != Some(restriction)
    {
        return Err(fail());
    }

    let root = format!("/faces/0/abilities/{block}");
    let identity = source["oracle_id"].as_str().ok_or_else(fail)?;
    let mut graph = Bindings::new(identity, source);
    graph.ability(&root, "static", "static-source-controller");
    graph.reference(
        &format!("{root}/effect/what"),
        &operand,
        &graph.id("/source", "object"),
        "201.5",
    );
    if restriction == CreatureRestriction::CanBlockOnlyFlying {
        let path = format!("{root}/effect/rule/attackers");
        let attackers = flying_attackers();
        let selector = graph.node(
            &path,
            "query",
            "selection",
            json!({"scope":graph.id(&root,"ability"),"expression":attackers,
                "cardinality":"many","timing":"continuous","selection":"evaluate-reference",
                "rules":["608.2h"]}),
        );
        graph.reference(&path, &attackers, &selector, "608.2h");
    }
    graph.validate_block(
        binding,
        &root,
        "requires complete source combat restriction bindings",
    )?;
    Ok(Some(static_ability(restriction)))
}

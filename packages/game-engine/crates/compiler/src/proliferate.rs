//! CR 603.6a and 701.34a (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Source,
    result: &ParseResult,
    shape: &Value,
    binding: &Value,
    kind: CardKind,
) -> Result<Option<Ability>, Failure> {
    if let Some(ast) = mtg_contracts::proliferate::landfall_ast(&source.oracle_text) {
        if decode::<Value>(result.abilities.get(), "abilities")? != ast {
            return Err(unsupported(
                "abilities.trigger",
                "requires complete controller land entry and Proliferate",
            ));
        }
        return Ok(Some(Ability::BoundTrigger(
            mtg_contracts::proliferate::landfall(),
        )));
    }
    let timing = match kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ => return Ok(None),
    };
    let Some((program, ast)) = mtg_contracts::proliferate::pump(&source.oracle_text, timing) else {
        return Ok(None);
    };
    if decode::<Value>(result.abilities.get(), "abilities")? != ast {
        return Err(unsupported(
            "abilities",
            "requires the complete creature modifier before Proliferate",
        ));
    }
    let root = "/faces/0/abilities/0";
    let pump = format!("{root}/effects/0/steps/0");
    let proliferate = format!("{root}/effects/1/steps/0");
    let mut graph = bindings::Bindings::new(&source.oracle_id, shape);
    graph.ability(root, "spell", "spell-controller");
    let controller = graph.id(root, "you");
    let scope = graph.id(root, "ability");
    let target = graph.target(
        &scope,
        &format!("{pump}/what"),
        &ast[0]["effects"][0]["steps"][0]["what"],
        "line:0:0",
    );
    graph.event(root, &pump, "pump", &target, false);
    graph.reference(
        &format!("{proliferate}/who"),
        &json!({"player":"you"}),
        &controller,
        "109.5",
    );
    graph.event(root, &proliferate, "keyword-action", &controller, false);
    graph.validate(
        binding,
        "requires the complete modifier and Proliferate reference graph",
    )?;
    Ok(Some(Ability::ComposedSpell(program)))
}

#[test]
fn proliferate_preserves_the_preceding_creature_modifier() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let raw = include_str!("../../../tests/fixtures/e4912bc3-bee9-4a2f-a13e-3a99018f8a65.json");
    let CompileResult::Lowered { definition } = compile(manifest, raw) else {
        panic!("{:?}", compile(manifest, raw));
    };
    let mut removed = *definition;
    removed.spell.as_mut().unwrap().instructions.remove(0);
    removed.capabilities = removed.requirements();
    assert!(removed.validate().is_err());
}

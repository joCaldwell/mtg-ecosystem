use super::*;
use serde_json::{Value, json};

pub(crate) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !mtg_contracts::resolution_mana::candidate(text) {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "resolution.mana",
            "requires its complete printed spell, operands, and cost",
        )
    };
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let Some(p) = mtg_contracts::resolution_mana::printed(text, &raw, kind) else {
        return Ok(None);
    };
    let mut graph = bindings::Bindings::new(identity, source);
    let who = json!({"player":"you"});
    let mut block = 0;
    if p.additional_sacrifice.is_some() {
        let base = "/faces/0/abilities/0";
        let what = format!("{base}/costs/0/what");
        graph.ability(base, "additional-cost", "prospective-caster");
        let yes = graph.id(base, "you");
        let scope = graph.id(base, "ability");
        let expr = &raw[0]["costs"][0]["what"];
        let selected=graph.node(&what,"query","selection",json!({"scope":scope,"expression":expr,"cardinality":"one","timing":"payment","selection":"evaluate-reference","rules":["608.2h"]}));
        graph.reference(&format!("{base}/costs/0/payer"), &who, &yes, "109.5");
        graph.reference(&what, expr, &selected, "608.2h");
        block = 1;
    }
    let base = format!("/faces/0/abilities/{block}");
    let step = format!("{base}/effects/0/steps/0");
    graph.ability(&base, "spell", "spell-controller");
    let you = graph.id(&base, "you");
    graph.reference(&format!("{step}/who"), &who, &you, "109.5");
    let [Instruction::AddResolutionMana { quantity, .. }] = p.instructions.as_slice() else {
        return Err(fail());
    };
    if matches!(
        quantity,
        ManaResolutionQuantity::PerMatchingPermanent { .. }
    ) {
        let scope = graph.id(&base, "ability");
        let path = format!("{step}/forEach/control");
        let opponents = graph.node(
            &path,
            "opponents",
            "derived-player",
            json!({"scope":scope,"role":"opponents","of":you,"rules":["102.2","109.5"]}),
        );
        graph.reference(&path, &json!({"implicit":"opponents"}), &opponents, "102.2");
        graph.node(&step,"event","event",json!({"scope":scope,"operation":"add-mana","inputs":[you],"availability":"when-performed","repeated":"for-each","rules":["608.2c"]}));
    } else {
        graph.event(&base, &step, "add-mana", &you, false);
    }
    graph.validate(binding, "requires a complete resolution mana binding graph")?;
    Ok(Some(p))
}

#[cfg(test)]
#[test]
fn composition_wave_b_resolution_mana_has_exact_quantities_and_prepayment() {
    let manifest = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    for (raw, sacrifice, variable) in [
        (
            include_str!("../../../../tests/fixtures/53f7c868-b03e-4fc2-8dcf-a75bbfa3272b.json"),
            false,
            false,
        ),
        (
            include_str!("../../../../tests/fixtures/6385fd0d-256d-4400-9319-afa8170024db.json"),
            true,
            false,
        ),
        (
            include_str!("../../../../tests/fixtures/a8dba58b-2956-492e-ae30-49db2ae68e53.json"),
            false,
            true,
        ),
    ] {
        let compiled = compile(manifest, raw);
        let CompileResult::Lowered { definition } = compiled else {
            panic!("{compiled:?}")
        };
        let p = definition.spell.as_ref().unwrap();
        assert_eq!(p.additional_sacrifice.is_some(), sacrifice);
        assert!(
            matches!(p.instructions.as_slice(),[Instruction::AddResolutionMana {quantity,..}]
            if matches!(quantity,ManaResolutionQuantity::PerMatchingPermanent {..}) == variable)
        );
        if sacrifice {
            let mut forged = (*definition).clone();
            forged.spell.as_mut().unwrap().additional_sacrifice = None;
            forged.capabilities = forged.requirements();
            assert!(forged.validate().is_err());
        }
        if variable {
            let forged = super::tests::changed_source(raw, |r| {
                let refs = r["binding"]["references"].as_array_mut().unwrap();
                let reference = refs
                    .iter_mut()
                    .find(|r| r["path"].as_str().unwrap().ends_with("/forEach/control"))
                    .unwrap();
                reference["rules"] = json!(["109.5"]);
            });
            assert!(!matches!(
                compile(manifest, &forged),
                CompileResult::Lowered { .. }
            ));
        }
    }
}

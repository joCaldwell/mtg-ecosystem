use super::*;
use crate::bindings::Bindings;
use mtg_contracts::{colored_library_program, colored_library_source};
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<SpellProgram>, Failure> {
    let Some((colors, ast)) = colored_library_source(&source.oracle_text) else {
        return Ok(None);
    };
    if source.type_line != "Sorcery"
        || !matches!(result.lines.as_slice(),[line] if line.text==source.oracle_text)
        || decode::<Value>(result.abilities.get(), "abilities")? != ast
    {
        return Err(unsupported(
            "abilities",
            "incomplete colored library search",
        ));
    }
    let base = "/faces/0/abilities/0";
    let search = format!("{base}/effects/0/steps/0");
    let path = format!("{search}/for");
    let mut g = Bindings::new(&source.oracle_id, shape);
    g.ability(base, "spell", "spell-controller");
    let scope = g.id(base, "ability");
    let you = g.id(base, "you");
    let steps = &ast[0]["effects"];
    let selection = &steps[0]["steps"][0]["for"];
    let slots: Vec<_> = selection["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| &m["filter"]["colors"][0])
        .collect();
    let allocation=g.node(&path,"allocation","distinct-library-search",json!({"scope":scope,"chooser":you,"libraryOwner":you,"slots":slots,"allocation":"distinct-objects","perSlot":"zero-or-one","cardinality":"many","rules":["608.2d","701.23b"]}));
    for (i, operation) in ["search", "reveal", "move-zone", "shuffle"]
        .into_iter()
        .enumerate()
    {
        let p = if i == 0 {
            search.clone()
        } else {
            format!("{base}/effects/1/steps/{}", i - 1)
        };
        let inputs = match i {
            0 | 1 => json!([allocation, you]),
            2 => json!([allocation]),
            _ => json!([you]),
        };
        g.node(&p,"event","event",json!({"scope":scope,"operation":operation,"inputs":inputs,"availability":"when-performed","rules":["608.2c"]}));
        g.reference(&format!("{p}/who"), &json!({"player":"you"}), &you, "109.5");
        match i {
            0 => {
                g.reference(
                    &format!("{p}/zones/of/0/owner"),
                    &json!({"player":"you"}),
                    &you,
                    "109.5",
                );
                g.reference_rules(&path, selection, &allocation, &["608.2d", "701.23b"]);
            }
            1 | 2 => {
                g.reference_rules(
                    &format!("{p}/what"),
                    &steps[1]["steps"][i - 1]["what"],
                    &allocation,
                    &["608.2k", "400.7j", "608.2h"],
                );
                if i == 2 {
                    g.reference(
                        &format!("{p}/to/owner"),
                        &json!({"player":"you"}),
                        &you,
                        "109.5",
                    );
                    g.result(&p, "quantity", "measured-quantity", None);
                }
            }
            _ => g.reference(
                &format!("{p}/library/owner"),
                &json!({"player":"you"}),
                &you,
                "109.5",
            ),
        }
    }
    g.validate(
        binding,
        "colored library search needs owned shared slots and bound reveal/move aggregate",
    )?;
    Ok(Some(colored_library_program(colors)))
}

#[test]
fn colored_library_search_requires_complete_bound_aggregate() {
    let raw = include_str!("../../../tests/fixtures/synthetic-colored-library.json");
    let result = compile(fixtures::MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    let mut stripped = *definition;
    stripped.spell.as_mut().unwrap().instructions = vec![Instruction::DrawCards {
        recipient: None,
        count: 1.into(),
        source_block: 0,
    }];
    stripped.capabilities = stripped.requirements();
    assert!(stripped.validate().is_err());
    for i in 0..3 {
        let forged = crate::tests::changed_source(raw, |r| match i {
            0 => r["binding"]["references"][4]["target"] = json!("foreign-aggregate"),
            1 => r["binding"]["nodes"][4]["perSlot"] = json!("exactly-one"),
            _ => r["binding"]["diagnostics"] = json!([{"code":"missing-antecedent"}]),
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &forged),
            CompileResult::Lowered { .. }
        ));
    }
}

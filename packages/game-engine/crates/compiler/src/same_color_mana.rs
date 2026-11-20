//! CR 106.1, 106.3, 107.3c, 109.5, 605.1a, 605.3b and 608.2d/h:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    ability: &Ability,
    parsed: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
    printed: &str,
) -> Result<Option<ManaAbility>, Failure> {
    let Ability::Activated { effects, .. } = ability else {
        return Ok(None);
    };
    if !effects
        .iter()
        .flat_map(|sentence| &sentence.steps)
        .any(|step| {
            matches!(
                step,
                Step::AddMana {
                    mana: SourceMana::AnyOneColor { .. },
                    ..
                }
            )
        })
    {
        return Ok(None);
    }

    let bad = || {
        unsupported(
            "mana",
            "requires one complete tap-only same-color mana production ability",
        )
    };
    let [sentence] = effects.as_slice() else {
        return Err(bad());
    };
    let [
        Step::AddMana {
            mana: SourceMana::AnyOneColor { amount },
            ..
        },
    ] = sentence.steps.as_slice()
    else {
        return Err(bad());
    };

    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    let mut expected = json!([{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[
        {"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"any-one-color","amount":amount}}
    ]}]}]);
    let root = "/faces/0/abilities/0";
    let mut graph = bindings::Bindings::new(identity, source);
    listed_mana::bind_tap_mana(&mut graph, root);
    let quantity = if *amount == json!({"amount":"x"}) {
        let definition = json!({"amount":"count","of":{"types":["enchantment"],"control":"you"}});
        expected[0]["effects"][0]["bindings"] = json!([{"variable":"x","value":definition}]);
        if printed
            != "{T}: Add X mana of any one color, where X is the number of enchantments you control."
        {
            return Err(bad());
        }
        let path = format!("{root}/effects/0/bindings/0");
        let variable = graph.node(
            &path,
            "number",
            "defined-number",
            json!({
                "scope":graph.id(root,"ability"),"definition":definition,"rules":["107.3c"]
            }),
        );
        graph.reference(
            &format!("{root}/effects/0/steps/0/mana/amount"),
            amount,
            &variable,
            "107.3c",
        );
        graph.reference(
            &format!("{path}/value/of/control"),
            &json!({"player":"you"}),
            &graph.id(root, "you"),
            "109.5",
        );
        SingleColorManaAmount::ControllerEnchantments
    } else {
        let value = amount["value"]
            .as_i64()
            .filter(|value| *value > 0)
            .ok_or_else(bad)?;
        let printed = printed
            .strip_prefix("{T}: Add ")
            .and_then(|text| text.strip_suffix(" mana of any one color."))
            .ok_or_else(bad)?;
        if *amount != json!({"amount":"fixed","value":value})
            || !matches_fixed_amount(printed, value)
        {
            return Err(bad());
        }
        SingleColorManaAmount::Fixed {
            quantity: value.into(),
        }
    };
    if raw != expected {
        return Err(bad());
    }
    graph.validate(
        binding,
        "requires the complete same-color mana binding graph",
    )?;
    Ok(Some(ManaAbility {
        self_damage: None,
        restriction: None,
        costs: PermanentActivationCosts {
            tap: true,
            mana: None,
            sacrifice: false,
            remove_source_plus_one_counters: None,
        },
        production: ManaProduction::AnyOneColor { amount: quantity },
        source_block: 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const LOTUS: &str =
        include_str!("../../../tests/fixtures/9a02a9a7-39d9-4763-85d3-747a0540b60b.json");
    const WEAVER: &str =
        include_str!("../../../tests/fixtures/acfe7ec0-0606-4d5e-b1fa-25f0c7aeec47.json");

    #[test]
    fn same_color_mana_requires_exact_fixed_or_counted_output_and_graph() {
        for (source, amount) in [
            (LOTUS, SingleColorManaAmount::Fixed { quantity: 3.into() }),
            (WEAVER, SingleColorManaAmount::ControllerEnchantments),
        ] {
            let result = compile(MANIFEST, source);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            let [ability] = definition.mana_abilities.as_slice() else {
                panic!()
            };
            assert_eq!(ability.production, ManaProduction::AnyOneColor { amount });
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::SameColorManaV1)
            );
            if source == LOTUS {
                let mut bad = *definition;
                bad.mana_abilities[0].production = ManaProduction::AnyOneColor {
                    amount: SingleColorManaAmount::Fixed { quantity: 4.into() },
                };
                assert!(bad.validate().is_err(), "quantity must match the source");
            }
        }
        let changed = crate::tests::changed_source(LOTUS, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][0]["mana"]["amount"]["value"] = json!(2);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(!matches!(
            compile(MANIFEST, &changed),
            CompileResult::Lowered { .. }
        ));

        let changed = crate::tests::changed_source(WEAVER, |record| {
            let reference = record["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|reference| {
                    reference["path"]
                        == "/faces/0/abilities/0/effects/0/bindings/0/value/of/control"
                })
                .unwrap();
            reference["target"] = json!("foreign:controller");
        });
        assert!(
            matches!(compile(MANIFEST, &changed), CompileResult::Unsupported { path, .. } if path == "binding")
        );
    }
}

//! Standalone target-player draws. CR 121.2, 601.2c and 608.2b-c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn lower(
    effects: &[Sentence],
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
) -> Result<bool, Failure> {
    let [
        Sentence {
            bindings,
            steps,
            condition: None,
            condition_kind: None,
        },
    ] = effects
    else {
        return Ok(false);
    };
    let [
        Step::Draw {
            who,
            amount: DrawAmount::Fixed { value },
            optional: false,
        },
    ] = steps.as_slice()
    else {
        return Ok(false);
    };
    // Keep the existing unrolled lowering for one and two draws.
    if *value <= 2 || who.player != "target-player" || !bindings.is_empty() {
        return Ok(false);
    }
    if program.additional_sacrifice.is_some() || !program.optional_additional_costs.is_empty() {
        return Err(unsupported("counted draw", "requires ordinary spell costs"));
    }
    let player = life_player(program, slots, who, Context::Spell, None)?;
    program
        .instructions
        .push(Instruction::draw(player, i64::from(*value).into(), 0));
    Ok(true)
}

pub(crate) fn validate_source(binding: &Value, identity: &str, raw: &str) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "requires complete standalone counted target-player draw",
        )
    };
    let mut abilities: Value = decode(raw, "abilities")?;
    let sentence = abilities[0]["effects"][0]
        .as_object_mut()
        .ok_or_else(fail)?;
    if sentence.remove("bindings").is_some_and(|v| v != json!([])) {
        return Err(fail());
    }
    let step = sentence["steps"][0].as_object_mut().ok_or_else(fail)?;
    if step.remove("optional").is_some_and(|v| v != false) {
        return Err(fail());
    }
    let who = step.get("who").ok_or_else(fail)?.clone();
    let selection = fixed_mill::target_selection(&who).ok_or_else(fail)?;
    if abilities
        != json!([{"kind":"spell","effects":[{"steps":[{
            "effect":"draw","who":who,"amount":abilities[0]["effects"][0]["steps"][0]["amount"]
        }]}]}])
    {
        return Err(fail());
    }

    let scope = format!("card:{identity}/faces/0/abilities/0#ability");
    let you = format!("card:{identity}/faces/0/abilities/0#you");
    let step_path = "/faces/0/abilities/0/effects/0/steps/0";
    let who_path = format!("{step_path}/who");
    let encoded = selection
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect::<String>();
    let target = format!("{scope}/targets/{encoded}");
    let event = format!("card:{identity}{step_path}#event");
    let [reference] = binding["references"]
        .as_array()
        .ok_or_else(fail)?
        .as_slice()
    else {
        return Err(fail());
    };
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["counts"] != json!({"obligations":1,"bound":1,"unresolved":0})
        || reference["path"] != who_path
        || reference["expression"] != who
        || reference["binding"] != "bound"
        || reference["target"] != target
        || nodes.len() != 8
        || nodes.iter().filter(|n| n["node"] == "selection").count() != 1
        || !nodes.iter().any(|n| {
            n["id"] == target
                && n["path"] == who_path
                && n["node"] == "selection"
                && n["scope"] == scope
                && n["expression"] == who
                && n["timing"] == "stack-announcement"
                && n["cardinality"] == "one"
                && n["selection"] == "target"
        })
        || !nodes.iter().any(|n| {
            n["id"] == you
                && n["node"] == "runtime-player"
                && n["scope"] == scope
                && n["role"] == "spell-controller"
        })
        || !nodes.iter().any(|n| {
            n["id"] == event
                && n["path"] == step_path
                && n["node"] == "event"
                && n["scope"] == scope
                && n["operation"] == "draw"
                && n["inputs"] == json!([target])
                && n["availability"] == "when-performed"
        })
    {
        return Err(fail());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;

    const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    const OPPORTUNITY: &str =
        include_str!("../../../../tests/fixtures/1f544a9f-c238-4858-be19-d6cd7d023dcc.json");

    #[test]
    fn targeted_counted_draw_exact_witness_preserves_legacy_target_draw_caps() {
        let CompileResult::Lowered { definition: d } = compile(HEADER, OPPORTUNITY) else {
            panic!("{:?}", compile(HEADER, OPPORTUNITY));
        };
        assert_eq!(d.characteristics.mana_cost.generic, 4.into());
        assert_eq!(d.characteristics.mana_cost.colored, [Color::U, Color::U]);
        let p = d.spell.as_ref().unwrap();
        assert!(targeted_counted_draw_program(p));
        assert_eq!(
            p.instructions,
            [Instruction::draw(PlayerExpr::target(0), 4.into(), 0)]
        );
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastInstantV1,
                Capability::AnyPlayerTargetV1,
                Capability::TargetedCountedDrawV1,
            ])
        );

        for raw in [
            include_str!("../../../../tests/fixtures/8f32ceb2-92c2-4dde-bf73-40bb79c3fcef.json"),
            include_str!("../../../../tests/fixtures/c6207f6a-a624-4754-88f5-dbe700c841ff.json"),
        ] {
            let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
                panic!("{:?}", compile(HEADER, raw));
            };
            assert!(!d.capabilities.contains(&Capability::TargetedCountedDrawV1));
            assert!(d.capabilities.contains(&Capability::TargetPlayerDrawV1));
            assert_eq!(
                d.spell
                    .as_ref()
                    .unwrap()
                    .instructions
                    .iter()
                    .filter(|i| {
                        matches!(i, kernel!(Draw { player: target_player!(0), amount: fixed!(count) } @ 0) if *count == Quantity::from(1))
                    })
                    .count(),
                2
            );
        }
    }

    #[test]
    fn targeted_counted_draw_rejects_raw_qualifiers_and_invalid_binding() {
        for case in 0..3 {
            let changed = changed_source(OPPORTUNITY, |r| {
                let result = &mut r["faces"][0]["result"];
                match case {
                    0 => {
                        result["abilities"][0]["effects"][0]["steps"][0]["who"]["of"] = Value::Null;
                        result["lines"][0]["ability"] = result["abilities"][0].clone();
                    }
                    1 => r["binding"]["references"][0]["target"] = json!("foreign"),
                    _ => r["source"]["mana_cost"] = json!(""),
                }
            });
            assert!(matches!(
                compile(HEADER, &changed),
                CompileResult::Unsupported { .. }
            ));
        }
    }
}

//! CR 109.5, 201.5 and 614.12: count the entering controller's other lands
//! against the inclusive printed threshold in the common prior battlefield.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
    let SourceStaticEffect::EntersTapped {
        what,
        condition_kind,
        condition: Some(condition),
    } = effect
    else {
        return Ok(None);
    };
    if condition["of"]["other"] != true {
        return Ok(None);
    }
    let bad = || {
        unsupported(
            "static.entry",
            "unsupported complete other-land entry condition",
        )
    };
    let operator = condition["comparison"]["op"].as_str().ok_or_else(bad)?;
    let comparison = match operator {
        "ge" => LandCountComparison::AtLeast,
        "le" => LandCountComparison::AtMost,
        _ => return Err(bad()),
    };
    let threshold = condition["comparison"]["value"]["value"]
        .as_i64()
        .filter(|n| *n >= 0)
        .ok_or_else(bad)?;
    if *what != json!({"ref":"this","noun":"land"})
        || condition_kind.as_ref() != Some(&json!("unless"))
        || *condition
            != json!({
                "condition":"control-count","who":{"player":"you"},"of":{"other":true,"types":["land"]},
                "comparison":{"op":operator,"value":{"amount":"fixed","value":threshold}}
            })
    {
        return Err(bad());
    }
    Ok(Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessControllerOtherLands {
            comparison,
            threshold: threshold.into(),
        },
    }))
}

pub(super) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !d
        .capabilities
        .contains(&Capability::ConditionalOtherLandEntryV1)
    {
        return Ok(());
    }
    let root = "/faces/0/abilities/0";
    let effect = format!("{root}/effect");
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    let object = graph.id("/source", "object");
    graph.ability(root, "static", "static-source-controller");
    let entering = graph.node(&effect, "entering-controller", "runtime-player", json!({
        "scope":graph.id(root,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]
    }));
    graph.reference(
        &format!("{effect}/what"),
        &json!({"ref":"this","noun":"land"}),
        &object,
        "201.5",
    );
    graph.reference(
        &format!("{effect}/condition/who"),
        &json!({"player":"you"}),
        &entering,
        "109.5",
    );
    graph.reference(
        &format!("{effect}/condition/of/other"),
        &json!({"ref":"self"}),
        &object,
        "201.5",
    );
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/1");
    graph.validate(
        binding,
        "requires the complete other-land entry binding graph",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../tests/fixtures/eb0d8093-5f93-4b25-9384-08f9731bfb28.json");

    #[test]
    fn other_land_entry_binds_inclusive_comparison_and_actor() {
        assert_eq!(
            sha256(SOURCE.trim_end().as_bytes()),
            "7c8fcba8efd58c9680dc226ee790de2750b89aa9ebc4f0eaf106bd07ede409ab"
        );
        let result = compile(MANIFEST, SOURCE);
        let CompileResult::Lowered { definition } = result else {
            panic!("expected Spirebluff Canal to lower: {result:?}");
        };
        assert_eq!(
            definition.static_abilities[0].effect,
            StaticEffect::EntersTappedUnlessControllerOtherLands {
                comparison: LandCountComparison::AtMost,
                threshold: 2.into(),
            }
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::ConditionalOtherLandEntryV1)
        );

        let reversed = crate::tests::changed_source(SOURCE, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effect"]["condition"]["comparison"]["op"] = json!("ge");
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(matches!(compile(MANIFEST, &reversed),
            CompileResult::Unsupported {path, ..} if path=="definition"));

        let wrong_actor = crate::tests::changed_source(SOURCE, |record| {
            let reference = record["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["path"] == "/faces/0/abilities/0/effect/condition/who")
                .unwrap();
            reference["target"] = json!(
                reference["target"]
                    .as_str()
                    .unwrap()
                    .replace("/effect#entering-controller", "#you")
            );
        });
        assert!(matches!(compile(MANIFEST, &wrong_actor),
            CompileResult::Unsupported {path, ..} if path=="binding"));
    }
}

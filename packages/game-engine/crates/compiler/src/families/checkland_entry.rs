//! CR 109.5, 205.3i and 614.12: an entering land checks whether its
//! prospective controller already controls a land of either printed subtype.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
    let SourceStaticEffect::EntersTapped {
        what,
        condition_kind,
        condition: Some(condition),
    } = effect
    else {
        return Ok(None);
    };
    if condition["condition"] != "control" {
        return Ok(None);
    }

    let bad = || unsupported("static.entry", "unsupported complete checkland entry");
    let Some([first, second]) = condition["what"]["filter"]["anyOf"]
        .as_array()
        .map(Vec::as_slice)
    else {
        return Err(bad());
    };
    let first = first["subtypes"][0].as_str().ok_or_else(bad)?;
    let second = second["subtypes"][0].as_str().ok_or_else(bad)?;
    let subtypes = [
        BasicLandType::from_subtype(first).ok_or_else(bad)?,
        BasicLandType::from_subtype(second).ok_or_else(bad)?,
    ];
    if first == second
        || *what != json!({"ref":"this","noun":"land"})
        || condition_kind.as_ref() != Some(&json!("unless"))
        || *condition
            != json!({
                "condition":"control","who":{"player":"you"},"what":query([first, second])
            })
    {
        return Err(bad());
    }

    Ok(Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessControllerLandTypes { subtypes },
    }))
}

fn query([first, second]: [&str; 2]) -> Value {
    json!({"ref":"filter","filter":{"anyOf":[{"subtypes":[first]},{"subtypes":[second]}]},
        "count":{"amount":"fixed","value":1}})
}

pub(crate) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    let [
        StaticAbility {
            effect: StaticEffect::EntersTappedUnlessControllerLandTypes { subtypes },
            ..
        },
    ] = d.static_abilities.as_slice()
    else {
        return Ok(());
    };
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    bind_entry(&mut graph, query(subtypes.map(BasicLandType::subtype)));
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/1");
    graph.validate(
        binding,
        "requires the complete checkland entry binding graph",
    )
}

pub(crate) fn bind_entry(graph: &mut bindings::Bindings, expression: Value) {
    let root = "/faces/0/abilities/0";
    let effect = format!("{root}/effect");
    let what = format!("{effect}/condition/what");
    graph.ability(root, "static", "static-source-controller");
    let scope = graph.id(root, "ability");
    let entering = graph.node(
        &effect,
        "entering-controller",
        "runtime-player",
        json!({
            "scope":scope,"role":"entering-controller","rules":["110.2a","614.12"]
        }),
    );
    let selection = graph.node(
        &what,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":expression,"cardinality":"one","timing":"continuous",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(
        &format!("{effect}/what"),
        &json!({"ref":"this","noun":"land"}),
        &graph.id("/source", "object"),
        "201.5",
    );
    graph.reference(&what, &expression, &selection, "608.2h");
    graph.reference(
        &format!("{effect}/condition/who"),
        &json!({"player":"you"}),
        &entering,
        "109.5",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../../tests/fixtures/6a6c5e17-6465-4a1f-9d63-8a3ce2edc522.json");

    #[test]
    fn checkland_entry_requires_exact_source_and_binding() {
        assert_eq!(
            sha256(SOURCE.trim_end().as_bytes()),
            "cb59776e4e6e2ab2eea6a1ece9c3ba6199ea031a1c58c18c2d9a5be67ccfe220"
        );
        let result = compile(MANIFEST, SOURCE);
        let CompileResult::Lowered { definition } = result else {
            panic!("expected checkland to lower: {result:?}");
        };
        assert_eq!(
            definition.static_abilities[0].effect,
            StaticEffect::EntersTappedUnlessControllerLandTypes {
                subtypes: [BasicLandType::Island, BasicLandType::Mountain]
            }
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::ConditionalLandTypeEntryV1)
        );

        let wrong_filter = crate::tests::changed_source(SOURCE, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effect"]["condition"]["what"]["filter"]["anyOf"][0]["subtypes"]
                [0] = json!("Plains");
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(!matches!(
            compile(MANIFEST, &wrong_filter),
            CompileResult::Lowered { .. }
        ));

        let wrong_actor = crate::tests::changed_source(SOURCE, |record| {
            let reference = record["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|reference| reference["path"] == "/faces/0/abilities/0/effect/condition/who")
                .unwrap();
            let target = reference["target"].as_str().unwrap();
            reference["target"] = json!(target.replace("/effect#entering-controller", "#you"));
        });
        assert!(matches!(compile(MANIFEST, &wrong_actor),
            CompileResult::Unsupported {path, ..} if path == "binding"));
    }
}

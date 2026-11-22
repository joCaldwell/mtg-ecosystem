//! CR 109.5, 205.3i, 614.12a, 616.1c and 701.20a–c (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ENTRY: &str = "/faces/0/abilities/0";
const BAD: &str = "requires a complete optional two-type hand reveal entry";

pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
    let SourceStaticEffect::EntryChoice {
        what,
        who,
        costs,
        declined,
    } = effect
    else {
        return Ok(None);
    };
    if !costs
        .iter()
        .any(|c| matches!(c, conditional_lands::SourceEntryCost::Reveal { .. }))
    {
        return Ok(None);
    }
    let bad = || unsupported("static.entry", BAD);
    let [conditional_lands::SourceEntryCost::Reveal { what: card }] = costs.as_slice() else {
        return Err(bad());
    };
    let Some([first, second]) = card["filter"]["allOf"][0]["anyOf"]
        .as_array()
        .map(Vec::as_slice)
    else {
        return Err(bad());
    };
    let first = first["subtypes"][0]
        .as_str()
        .and_then(type_for_subtype)
        .ok_or_else(bad)?;
    let second = second["subtypes"][0]
        .as_str()
        .and_then(type_for_subtype)
        .ok_or_else(bad)?;
    if first == second
        || *card != query([first, second])
        || *what != json!({"ref":"this","noun":"land"})
        || *who != json!({"player":"you"})
        || *declined != json!({"tapped":true})
    {
        return Err(bad());
    }
    Ok(Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessRevealLandTypes {
            subtypes: [first, second],
        },
    }))
}

fn type_for_subtype(name: &str) -> Option<BasicLandType> {
    [
        BasicLandType::Plains,
        BasicLandType::Island,
        BasicLandType::Swamp,
        BasicLandType::Mountain,
        BasicLandType::Forest,
    ]
    .into_iter()
    .find(|kind| kind.subtype() == name)
}

fn query([first, second]: [BasicLandType; 2]) -> Value {
    json!({"ref":"filter","filter":{"allOf":[{"anyOf":[
            {"subtypes":[first.subtype()]},{"subtypes":[second.subtype()]}
        ]}],"cls":"card","zone":{"zone":"hand","owner":"your"}},"count":{"amount":"fixed","value":1}})
}

pub(crate) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    let [
        StaticAbility {
            effect: StaticEffect::EntersTappedUnlessRevealLandTypes { subtypes },
            ..
        },
    ] = d.static_abilities.as_slice()
    else {
        return Ok(());
    };
    let root = format!("{ENTRY}/effect");
    let what = format!("{root}/costs/0/what");
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    graph.ability(ENTRY, "static", "static-source-controller");
    let scope = graph.id(ENTRY, "ability");
    let controller = graph.node(
        &root,
        "entering-controller",
        "runtime-player",
        json!({
            "scope":scope,"role":"entering-controller","rules":["110.2a","614.12"]
        }),
    );
    let expression = query(*subtypes);
    let selection = graph.node(
        &what,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":expression,"cardinality":"one","timing":"payment",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(
        &format!("{root}/what"),
        &json!({"ref":"this","noun":"land"}),
        &graph.id("/source", "object"),
        "201.5",
    );
    for path in [
        format!("{root}/who"),
        format!("{root}/costs/0/payer"),
        format!("{what}/filter/zone/owner"),
    ] {
        graph.reference(&path, &json!({"player":"you"}), &controller, "109.5");
    }
    graph.reference(&what, &expression, &selection, "608.2h");
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/1");
    graph.validate(binding, BAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../../tests/fixtures/7137aae6-260d-41de-8b4e-42a8cf752697.json");

    #[test]
    fn reveal_land_entry_compiles_only_the_complete_private_hand_choice() {
        let result = compile(MANIFEST, SOURCE);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::RevealLandEntryV1)
        );
        assert_eq!(
            definition.static_abilities[0].effect,
            StaticEffect::EntersTappedUnlessRevealLandTypes {
                subtypes: [BasicLandType::Island, BasicLandType::Mountain],
            }
        );
        let wrong = crate::tests::changed_source(SOURCE, |r| {
            let references = r["binding"]["references"].as_array_mut().unwrap();
            let reference = references
                .iter_mut()
                .find(|r| r["path"] == "/faces/0/abilities/0/effect/costs/0/what/filter/zone/owner")
                .unwrap();
            reference["target"] = json!("another-seat");
        });
        assert!(
            matches!(compile(MANIFEST, &wrong), CompileResult::Unsupported { path, .. } if path == "binding")
        );
    }
}

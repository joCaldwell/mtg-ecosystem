//! CR 400.7j, 608.2c and 701.26b (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};
use std::num::NonZeroU32;

pub(crate) fn parse(suffix: &str, effect: &Value) -> Option<(NonZeroU32, Value)> {
    let minimum = effect["condition"]["comparison"]["value"]["value"]
        .as_u64()
        .and_then(|amount| u32::try_from(amount).ok())
        .and_then(NonZeroU32::new)?;
    let condition = mtg_contracts::fetched_land_untap_clause(suffix, minimum)?;
    (condition == *effect).then_some((minimum, condition))
}

pub(crate) fn bind(graph: &mut bindings::Bindings<'_>, root: &str) {
    let clause = format!("{root}/effects/1");
    let untap = format!("{clause}/steps/0");
    let successor = graph.id(&format!("{root}/effects/0/steps/1"), "objects");
    graph.node(
        &untap,
        "event",
        "event",
        json!({
            "scope":graph.id(root, "ability"),"operation":"untap","inputs":[successor],
            "availability":"when-performed","conditional":true,"rules":["608.2c"]
        }),
    );
    graph.reference(
        &format!("{clause}/condition/who"),
        &json!({"player":"you"}),
        &graph.id(root, "you"),
        "109.5",
    );
    graph.reference_rules(
        &format!("{untap}/what"),
        &mtg_contracts::fetched_land_reference(),
        &successor,
        &["608.2k", "400.7j", "608.2h"],
    );
}

pub(crate) fn decoded_activation(raw: &str) -> Result<Vec<Ability>, Failure> {
    // The caller has validated and lowered the entire archived activation.
    // Only the original cost and search sentence use the ordinary AST mirror.
    let mut shape: Value = decode(raw, "abilities")?;
    let effects = shape[0]["effects"].as_array_mut().ok_or_else(|| {
        unsupported(
            "abilities.effects",
            "missing validated conditional fetch sentences",
        )
    })?;
    effects.truncate(1);
    read_abilities(&shape.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARCHIVE: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../../tests/fixtures/0c85b8f7-0bd0-4680-9ec5-d4b110460a54.json");

    #[test]
    fn fetched_land_untap_requires_exact_followup_and_successor_binding() {
        let lowered = compile(ARCHIVE, SOURCE);
        let CompileResult::Lowered { mut definition } = lowered else {
            panic!("{lowered:?}")
        };
        assert_eq!(
            definition.activated[0].program,
            conditional_basic_land_fetch_program(true, NonZeroU32::new(4))
        );
        definition.activated[0].program =
            conditional_basic_land_fetch_program(true, NonZeroU32::new(3));
        assert!(definition.validate().is_err());

        let wrong_object = crate::tests::changed_source(SOURCE, |record| {
            let references = record["binding"]["references"].as_array_mut().unwrap();
            let successor = references
                .iter_mut()
                .find(|reference| {
                    reference["path"] == "/faces/0/abilities/0/effects/1/steps/0/what"
                })
                .unwrap();
            let target = successor["target"]
                .as_str()
                .unwrap()
                .replace("steps/1#objects", "steps/0/for#query");
            successor["target"] = json!(target);
        });
        assert!(matches!(compile(ARCHIVE, &wrong_object),
            CompileResult::Unsupported { path, .. } if path == "binding"));
    }
}

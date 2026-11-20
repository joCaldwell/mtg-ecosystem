//! CR 603.6a, 614.12 and 701.22 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn matches(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> bool {
    matches!(trigger, SourceTrigger::Enters { what: TriggerObject::This { noun } } if noun == "land")
        && block == 1
        && intervening.is_none()
        && matches!(effects, [Sentence { condition: None, condition_kind: None, bindings, steps }]
            if bindings.is_empty() && matches!(steps.as_slice(), [Step::Scry { amount }]
                if *amount == json!({"amount":"fixed","value":1})))
}

pub(super) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::LandEntryScryV1) {
        return Ok(());
    }
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    let object = graph.id("/source", "object");
    let subject = json!({"ref":"this","noun":"land"});
    let entry = "/faces/0/abilities/0";
    let effect = format!("{entry}/effect");
    graph.ability(entry, "static", "static-source-controller");
    graph.node(&effect,"entering-controller","runtime-player",json!({
        "scope":graph.id(entry,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]
    }));
    graph.reference(&format!("{effect}/what"), &subject, &object, "201.5");
    let ability = "/faces/0/abilities/1";
    let trigger = format!("{ability}/trigger");
    graph.ability(ability, "triggered", "trigger-controller");
    graph.reference(&format!("{trigger}/what"), &subject, &object, "201.5");
    graph.event(ability, &trigger, "trigger:enters", &object, false);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"types":["land"]})),
    );
    graph.node(
        &format!("{ability}/effects/0/steps/0"),
        "event",
        "event",
        json!({
            "scope":graph.id(ability,"ability"),"operation":"scry","inputs":[],
            "availability":"when-performed","rules":["608.2c"]
        }),
    );
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/2");
    graph.validate(
        binding,
        "requires complete mandatory land entry scry binding",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn land_entry_scry_authenticates_entry_and_exact_source() {
        let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
        let source =
            include_str!("../../../tests/fixtures/79f94050-d850-41ca-b1db-5ae0cf743f0a.json");
        let result = compile(manifest, source);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert_eq!(definition.triggered, [land_entry_scry_trigger()]);
        let wrong = crate::tests::changed_source(source, |row| {
            let nodes = row["binding"]["nodes"].as_array_mut().unwrap();
            let scry = nodes
                .iter_mut()
                .find(|node| node["operation"] == "scry")
                .unwrap();
            scry["optional"] = json!(true);
        });
        assert!(
            matches!(compile(manifest,&wrong), CompileResult::Unsupported { path, .. } if path == "binding")
        );
    }
}

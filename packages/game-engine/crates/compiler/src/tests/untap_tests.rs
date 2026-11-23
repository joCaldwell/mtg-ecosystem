//! Exact retained source and explicitly synthetic complete-consumption probes.
use super::*;
use serde_json::json;

const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const BURST: &str =
    include_str!("../../../../tests/fixtures/b795ecfd-31ac-4a2a-9679-97b088932d93.json");

#[test]
fn untap_behind_a_tap_cost_or_entry_trigger_is_an_ordinary_composed_ability() {
    for activated in [true, false] {
        let text = if activated {
            "{T}: Untap target permanent."
        } else {
            "When this creature enters, untap target permanent."
        };
        let raw = tests::changed_source(BURST, |record| {
            record["source"]["type_line"] = "Creature — Human".into();
            record["source"]["power"] = "1".into();
            record["source"]["toughness"] = "1".into();
            record["source"]["oracle_text"] = text.into();
            let result = &mut record["faces"][0]["result"];
            let ability = &mut result["abilities"][0];
            ability["kind"] = if activated { "activated" } else { "triggered" }.into();
            if activated {
                ability["costs"] = json!([{"cost":"tap-self"}]);
            } else {
                ability["trigger"] =
                    json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}});
            }
            result["lines"][0]["text"] = text.into();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            // Retained binding data is diagnostic. Keep its known scope/face
            // descriptions aligned; the Rust compiler owns support decisions.
            for node in record["binding"]["nodes"].as_array_mut().unwrap() {
                match node["node"].as_str() {
                    Some("face") => node["typeLine"] = "Creature — Human".into(),
                    Some("ability") => {
                        node["abilityKind"] =
                            if activated { "activated" } else { "triggered" }.into()
                    }
                    Some("runtime-player") => node["role"] = "ability-controller".into(),
                    _ => (),
                }
            }
        });
        let d = tests::composed_near_miss(crate::tests::synthetic_compile(MANIFEST, &raw));
        let program = if activated {
            let [ability] = d.activated.as_slice() else {
                panic!("one activation")
            };
            assert_eq!(ability.cost, ActivationCost::TapSource);
            assert!(d.triggered.is_empty());
            &ability.program
        } else {
            let [ability] = d.triggered.as_slice() else {
                panic!("one trigger")
            };
            assert_eq!(ability.condition, TriggerCondition::SelfEntersBattlefield);
            assert!(d.activated.is_empty());
            &ability.program
        };
        assert_eq!(
            tests::sole_target(program),
            (
                TargetDomain::BattlefieldPermanent,
                vec![],
                vec![],
                vec![],
                false,
                false
            )
        );
        assert_eq!(tests::ops(program), ["kernel"]);
    }
}

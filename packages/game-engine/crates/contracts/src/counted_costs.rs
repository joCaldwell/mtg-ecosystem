//! Source-backed admission is artifact/caster; runtime arithmetic is generic.
use serde_json::{Value, json};
pub(super) fn schema(schema: &mut Value) {
    let effect = json!({"type":"object","required":["reduce_self_generic_cost"]});
    schema["$defs"]["StaticEffect"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"reduce_self_generic_cost":{"properties":{
                "per_object":{"type":"string","pattern":"^[1-9][0-9]*$"},
                "count":{"properties":{"kind":{"const":"artifact"},"controller":{"const":"caster"}}}
            }}}
        }));
    schema["$defs"]["StaticAbility"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["effect"],"properties":{"effect":effect}},
            "then":{"properties":{"functioning_zone":{"const":"stack"}}}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{"effect":effect}}}}},
        "then":{"properties":{
            "capabilities":{"contains":{"const":"counted_self_generic_cost_reduction_v1"}},
            "spell":{"type":"object"},
            "characteristics":{"properties":{"kind":{"enum":["creature","artifact","enchantment","instant","sorcery"]},"subtypes":{"not":{"contains":{"enum":["Aura","Equipment"]}}}}}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":"counted_self_generic_cost_reduction_v1"}}}}}
    }));
}

//! Schema closure for the live controller-creature modifier family.
use super::*;

pub(super) fn schema(schema: &mut serde_json::Value) {
    // CR 611.3a-c: the live source's current controller determines recipients.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let modifier = serde_json::json!({"type":"object","required":["modify_creatures"],"properties":{
        "modify_creatures":{"required":["scope"],"properties":{"scope":{"const":"controller"}}}
    }});
    schema["$defs"]["StaticEffect"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({"if":modifier,"then":{"properties":{
            "modify_creatures":{
                "required":["scope","power","toughness","keywords","restrictions"],
                "properties":{
                    "power":{"pattern":"^(0|[1-9][0-9]*)$"},
                    "toughness":{"pattern":"^(0|[1-9][0-9]*)$"},
                    "keywords":{"uniqueItems":true,"items":{"not":{"enum":["flash","indestructible"]}}},
                    "restrictions":{"uniqueItems":true}
                },
                "anyOf":[
                    {"properties":{"power":{"not":{"const":"0"}}}},
                    {"properties":{"toughness":{"not":{"const":"0"}}}},
                    {"properties":{"keywords":{"minItems":1}}},
                    {"properties":{"restrictions":{"minItems":1}}}
                ]
            }
        }}}));
    schema["$defs"]["StaticAbility"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| serde_json::json!([])).as_array_mut().unwrap()
        .push(serde_json::json!({"if":{"required":["effect"],"properties":{"effect":modifier}},
            "then":{"properties":{"functioning_zone":{"const":"battlefield"}}}}));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap()
        .push(serde_json::json!({"if":{"required":["static_abilities"],"properties":{
            "static_abilities":{"contains":{"required":["effect"],"properties":{"effect":modifier}}}
        }},"then":{"properties":{
            "characteristics":{"properties":{
                "kind":{"enum":["creature","enchantment"]},
                "subtypes":{"not":{"contains":{"const":"Aura"}}}
            }},
            "capabilities":{"contains":{"const":Capability::AttachmentsV1}}
        }}}));
    // Scope operands still require their existing source kinds; admitting a
    // creature's Controller scope cannot turn it into an attachment source.
    for (scope, kind, subtype) in [
        ("enchanted", "enchantment", "Aura"),
        ("equipped", "artifact", "Equipment"),
    ] {
        schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap()
            .push(serde_json::json!({"if":{"required":["static_abilities"],"properties":{
                "static_abilities":{"contains":{"required":["effect"],"properties":{
                    "effect":{"type":"object","required":["modify_creatures"],"properties":{
                        "modify_creatures":{"required":["scope"],"properties":{"scope":{"const":scope}}}
                    }}
                }}}
            }},"then":{"properties":{"characteristics":{"properties":{
                "kind":{"const":kind},"subtypes":{"contains":{"const":subtype}}
            }}}}}));
    }
    schema["$defs"]["StaticEffect"]["allOf"].as_array_mut().unwrap()
        .push(serde_json::json!({"properties":{"modify_creatures":{
            "if":{"required":["scope"],"properties":{"scope":{"const":"source"}}},
            "then":{"properties":{"power":{"const":"0"},"toughness":{"const":"0"},
                "keywords":{"const":[]},"restrictions":{"const":["cant_block"]}}}
        }}}));
}

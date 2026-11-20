//! Finite land-type OR search. CR 205.3i, 305.6, 701.23b (2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BasicLandType {
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,
}
impl BasicLandType {
    pub fn from_subtype(subtype: &str) -> Option<Self> {
        match subtype {
            "Plains" => Some(Self::Plains),
            "Island" => Some(Self::Island),
            "Swamp" => Some(Self::Swamp),
            "Mountain" => Some(Self::Mountain),
            "Forest" => Some(Self::Forest),
            _ => None,
        }
    }
    pub fn color(self) -> Color {
        match self {
            Self::Plains => Color::W,
            Self::Island => Color::U,
            Self::Swamp => Color::B,
            Self::Mountain => Color::R,
            Self::Forest => Color::G,
        }
    }
    pub fn subtype(self) -> &'static str {
        match self {
            Self::Plains => "Plains",
            Self::Island => "Island",
            Self::Swamp => "Swamp",
            Self::Mountain => "Mountain",
            Self::Forest => "Forest",
        }
    }
}

/// All found cards enter together, then the controller shuffles. Hidden quality
/// searches may find zero even when singular (CR 701.23b). `force_tapped=false`
/// preserves ordinary entry effects; it never forces an untapped permanent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypedLandSearch {
    pub any_basic_land_types: Vec<BasicLandType>,
    pub maximum: Quantity,
    pub force_tapped: bool,
}
impl TypedLandSearch {
    pub fn validate(&self) -> Result<(), String> {
        if self.maximum <= Quantity::zero()
            || self.any_basic_land_types.is_empty()
            || !self.any_basic_land_types.windows(2).all(|w| w[0] < w[1])
        {
            return Err(
                "typed land search requires positive maximum and canonical nonempty types".into(),
            );
        }
        Ok(())
    }
    pub fn accepts(&self, c: &Characteristics) -> bool {
        c.has_type(CardKind::Land)
            && self
                .any_basic_land_types
                .iter()
                .any(|t| c.subtypes.iter().any(|s| s == t.subtype()))
    }
}

fn has_search(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::SearchTypedLands { .. }))
        || p.modes.iter().any(|m| has_search(&m.program))
}
pub(super) fn validate_program(p: &SpellProgram) -> Result<(), String> {
    if has_search(p)
        && (!matches!(
            p.instructions.as_slice(),
            [Instruction::SearchTypedLands {
                source_block: 0,
                ..
            }]
        ) || !p.targets.is_empty()
            || !p.modes.is_empty()
            || p.additional_sacrifice.is_some()
            || !p.optional_additional_costs.is_empty()
            || p.functioning_zone != FunctioningZone::Stack
            || p.resolution != SpellResolution::Instructions)
    {
        return Err("typed land search requires one complete ordinary program".into());
    }
    Ok(())
}
/// The one finite typed-search activation cost/program, independent of names.
pub(super) fn activation(a: &ActivatedAbility) -> bool {
    matches!(&a.cost, ActivationCost::TapPayLifeSacrificeSource { life } if !life.is_negative())
        && a.functioning_zone == FunctioningZone::Battlefield
        && a.program.timing == SpellTiming::Instant
        && has_search(&a.program)
        && validate_program(&a.program).is_ok()
}
pub(super) fn activation_definition(d: &Definition) -> bool {
    matches!(d.activated.as_slice(), [a] if activation(a))
        && d.characteristics.kind == CardKind::Land
}
pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let activated = d.activated.iter().any(|a| {
        has_search(&a.program) || matches!(a.cost, ActivationCost::TapPayLifeSacrificeSource { .. })
    });
    let present = activated
        || d.spell.as_ref().is_some_and(has_search)
        || d.triggered.iter().any(|a| has_search(&a.program));
    if !present {
        return Ok(());
    }
    let c = &d.characteristics;
    let source = if activated {
        activation_definition(d)
            && d.spell.is_none()
            && !c.basic
            && !c.snow
            && !c.legendary
            && c.subtypes.is_empty()
            && c.additional_types.is_empty()
    } else {
        matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
            && d.spell.as_ref().is_some_and(has_search)
            && d.activated.is_empty()
    };
    if !source
        || d.x_mana_cost
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
    {
        return Err("typed land search requires a complete ordinary spell or pay-life source-land activation".into());
    }
    Ok(())
}
pub(super) fn schema(s: &mut serde_json::Value) {
    use serde_json::json;
    let types = [
        BasicLandType::Plains,
        BasicLandType::Island,
        BasicLandType::Swamp,
        BasicLandType::Mountain,
        BasicLandType::Forest,
    ];
    let sets: Vec<_> = (1..32)
        .map(|mask| {
            types
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, t)| *t)
                .collect::<Vec<_>>()
        })
        .collect();
    s["$defs"]["TypedLandSearch"]["properties"]["maximum"] =
        json!({"type":"string","pattern":"^[1-9][0-9]*$"});
    s["$defs"]["TypedLandSearch"]["properties"]["any_basic_land_types"] = json!({"enum":sets});
    s["$defs"]["TypedLandSearchFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"search_typed_lands"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/TypedLandSearchFreeProgram"}}}}
    }});
    let present = json!({"not":{"$ref":"#/$defs/TypedLandSearchFreeProgram"}});
    s["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":present,"then":{"properties":{
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":"search_typed_lands"},"source_block":{"const":0}}}},
            "targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}
        }}
    }));
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","allOf":[present]}}},
        "then":{"allOf":[
            {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
             "then":{"properties":{"capabilities":{"contains":{"const":Capability::CastInstantV1}},"spell":{"properties":{"timing":{"const":"instant"}}}}},
             "else":{"properties":{"capabilities":{"contains":{"const":Capability::CastSorceryV1}},"spell":{"properties":{"timing":{"const":"main_phase"}}}}}}
        ],"properties":{
            "x_mana_cost":{"const":false},
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "capabilities":{"minItems":3,"maxItems":3,"items":{"enum":[Capability::NormalLayoutV1,Capability::TypedLandSearchV1,Capability::CastInstantV1,Capability::CastSorceryV1]},"allOf":[
                {"contains":{"const":Capability::TypedLandSearchV1}},{"contains":{"const":Capability::NormalLayoutV1}}
            ]},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0}
        }},"else":{"if":{"not":{"required":["activated"],"properties":{"activated":{"contains":{"properties":{"program":present}}}}}},"then":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::TypedLandSearchV1}}}}}}
    }));
    let cost = json!({"type":"object","required":["tap_pay_life_sacrifice_source"]});
    s["$defs"]["ActivationCost"].as_object_mut().unwrap().entry("allOf").or_insert_with(||json!([])).as_array_mut().unwrap().push(json!({"properties":{
        "tap_pay_life_sacrifice_source":{"properties":{"life":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"}}}
    }}));
    let activated = json!({"required":["activated"],"properties":{"activated":{"contains":{"anyOf":[
        {"properties":{"program":present}},
        {"allOf":[{"properties":{"cost":cost}},{"not":basic_land_fetch::activation_schema()}]}
    ]}}}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":activated,
        "then":{"properties":{
            "characteristics":{"properties":{"kind":{"const":"land"},"basic":{"const":false},"snow":{"const":false},"legendary":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},"mana_cost":{"properties":{"generic":{"const":"0"},"colored":{"maxItems":0}}},"power":{"type":"null"},"toughness":{"type":"null"}}},
            "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},"x_mana_cost":{"const":false},
            "activated":{"minItems":1,"maxItems":1,"items":{"properties":{
                "functioning_zone":{"const":"battlefield"},"cost":cost,
                "program":{"allOf":[present],"properties":{"timing":{"const":"instant"}}}
            }}},
            "triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "capabilities":{"minItems":7,"maxItems":7,"uniqueItems":true,"items":{"enum":[Capability::NormalLayoutV1,Capability::LandPlayV1,Capability::ActivatedAbilityV1,Capability::TapSourceCostV1,Capability::SacrificeSourceCostV1,Capability::PayLifeCostV1,Capability::TypedLandSearchV1]}}
        }},"else":{"if":{"not":{"required":["activated"],"properties":{"activated":{"contains":basic_land_fetch::activation_schema()}}}},
            "then":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::PayLifeCostV1}}}}}}
    }));
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"properties":{
        "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/TypedLandSearchFreeProgram"}}}}
    }}));
}

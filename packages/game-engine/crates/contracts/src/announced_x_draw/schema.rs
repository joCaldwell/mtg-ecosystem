use super::*;
use serde_json::{Value, json};

fn program_schema(shape: Shape) -> Value {
    let targets = if let Some(domain) = shape.domain() {
        json!({"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"selection_id":{"minLength":1},"domain":{"const":domain},
            "stage":{"const":"announcement"},"chooser":{"const":"controller"},"filter":{"type":"null"}
        }}})
    } else {
        json!({"maxItems":0})
    };
    let instructions: Vec<_> = shape
        .instructions()
        .into_iter()
        .map(|i| match i {
            Instruction::DiscardCards { .. } => json!({"properties":{
                "op":{"const":"discard_cards"},"count":{"const":"1"},"source_block":{"const":0},
                "recipient":{"enum":[null,"controller"]}
            }}),
            Instruction::DrawCards { .. } => json!({"properties":{
                "op":{"const":"draw_cards"},"count":{"const":"1"},"source_block":{"const":0},
                "recipient":{"type":"null"}
            }}),
            i => json!({"const":i}),
        })
        .collect();
    let mut program = json!({"properties":{
        "targets":targets,"instructions":{"minItems":instructions.len(),"maxItems":instructions.len()},
        "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
        "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}
    }});
    if shape == Shape::DrawOrDamage {
        let modes: Vec<_> = shape.modes().iter().map(|&m| json!({"properties":{"program":{
            "allOf":[program_schema(m),{"properties":{"timing":{"const":SpellTiming::MainPhase}}}]
        }}})).collect();
        program["required"] = json!(["modes"]);
        program["properties"]["timing"] = json!({"const":SpellTiming::MainPhase});
        program["properties"]["modes"] = json!({"minItems":2,"maxItems":2,"prefixItems":modes});
    } else {
        program["properties"]["instructions"]["prefixItems"] = json!(instructions);
    }
    program
}

pub(crate) fn extend_schema(schema: &mut Value) {
    let free = json!({"$ref":"#/$defs/AnnouncedXDrawFreeProgram"});
    schema["$defs"]["AnnouncedXDrawFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"enum":[
            "draw_announced_x","change_life_announced_x","deal_damage_announced_x","mill_announced_x"
        ]}}}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["Instruction"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"properties":{"op":{"enum":["draw_announced_x","change_life_announced_x"]}}},
        "then":{"properties":{"source_block":{"const":0},"recipient":{"enum":[LifeRecipient::Controller,LifeRecipient::Target{target_slot:0}]}}}
    }));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    let mut alternatives = Vec::new();
    let mut programs = Vec::new();
    for shape in Shape::all() {
        let program = program_schema(shape);
        for (kind, timing, cast) in [
            (
                CardKind::Instant,
                SpellTiming::Instant,
                Capability::CastInstantV1,
            ),
            (
                CardKind::Sorcery,
                SpellTiming::MainPhase,
                Capability::CastSorceryV1,
            ),
        ] {
            if shape == Shape::DrawOrDamage && kind != CardKind::Sorcery {
                continue;
            }
            let mut caps = BTreeSet::from([Capability::NormalLayoutV1, cast]);
            add_program_requirements(&mut caps, &shape.program(timing));
            let required: Vec<_> = caps
                .iter()
                .map(|cap| json!({"contains":{"const":cap}}))
                .collect();
            let subtypes = if shape.allows_arcane(kind) {
                json!({"enum":[[],["Arcane"]]})
            } else {
                json!({"maxItems":0})
            };
            alternatives.push(json!({"properties":{
                "capabilities":{"maxItems":caps.len(),"allOf":required},
                "characteristics":{"properties":{"kind":{"const":kind},"subtypes":subtypes}},
                "spell":{"allOf":[program,{"properties":{"timing":{"const":timing}}}]}
            }}));
        }
        programs.push(program);
    }
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"if":{"not":free},"then":{"anyOf":programs}}));
    let defs = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    defs.push(json!({"properties":{
        "activated":{"items":{"properties":{"program":free}}},
        "triggered":{"items":{"properties":{"program":free}}}
    }}));
    defs.push(
        json!({"if":has,"then":{"required":["x_mana_cost"],"properties":{
            "x_mana_cost":{"const":true},"intrinsic_zone":{"type":"null"},
            "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
            "characteristics":{"properties":{"additional_types":{"maxItems":0},
                "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "power":{"type":"null"},"toughness":{"type":"null"},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
            }}
        },"anyOf":alternatives},"else":{"properties":{"capabilities":{"not":{"contains":{"enum":[
            Capability::AnnouncedXDrawV1,Capability::AnnouncedXEffectsV1
        ]}}}}}}),
    );
    let wipe = json!({"required":["spell"],"properties":{"spell":{"type":"object","properties":{"instructions":{"contains":{
        "required":["op","amount"],"properties":{"op":{"const":"damage_set"},"amount":{"const":{"amount":"announced_x"}}}
    }}}}}});
    let counted_x = json!({"required":["spell"],"properties":{"spell":{"type":"object","properties":{"instructions":{"contains":{
        "required":["op","count"],"properties":{"op":{"enum":["discard_target_player_at_random","draw_each_player"]},"count":{"const":{"amount":"announced_x"}}}
    }}}}}});
    defs.push(json!({"if":{"anyOf":[has,wipe,counted_x]},"then":{"required":["x_mana_cost"],"properties":{"x_mana_cost":{"const":true}}},"else":{"properties":{"x_mana_cost":{"const":false}}}}));
}

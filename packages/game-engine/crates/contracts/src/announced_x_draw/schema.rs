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
            i if i.single_draw() => instruction_schema::at(instruction_schema::single_draw(), 0),
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
    // CR 107.3: an announced-X draw by one player (each player's X draw is
    // its own family).
    let x_draw = json!({"allOf":[
        instruction_schema::kernel_announced_x(&["draw"]),
        {"properties":{"node":{"properties":{"do":{"properties":{"player":{"properties":{"player":{"const":"is"}}}}}}}}}
    ]});
    schema["$defs"]["AnnouncedXDrawFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"anyOf":[
            {"required":["op"],"properties":{"op":{"enum":[
                "deal_damage_announced_x","mill_announced_x"
            ]}}},
            x_draw,
            instruction_schema::kernel_announced_x(&["gain_life","lose_life"])
        ]}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["Instruction"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":x_draw,
        "then":{"properties":{"source_block":{"const":0},"node":{"properties":{"do":{"properties":{
            "player":{"enum":[kir::PlayerExpr::you(Context::Spell),kir::PlayerExpr::target(0)]}}}}}}}
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
    let x_amount = json!({"required":["spell"],"properties":{"spell":{"type":"object","properties":{"instructions":{"contains":{"anyOf":[
        {"required":["op","amount"],"properties":{"op":{"enum":["damage_set"]},"amount":{"const":{"amount":"announced_x"}}}},
        instruction_schema::kernel_seq_announced_x()
    ]}}}}}});
    let counted_x = json!({"required":["spell"],"properties":{"spell":{"type":"object","properties":{"instructions":{"contains":{
        "anyOf":[
            {"required":["op","count"],"properties":{"op":{"const":"discard_target_player_at_random"},"count":{"const":{"amount":"announced_x"}}}},
            {"allOf":[
                instruction_schema::kernel_verb_for("draw", &kir::PlayerExpr::EachPlayer),
                instruction_schema::kernel_announced_x(&["draw"])
            ]}
        ]
    }}}}}});
    defs.push(json!({"if":{"anyOf":[has,x_amount,counted_x]},"then":{"required":["x_mana_cost"],"properties":{"x_mana_cost":{"const":true}}},"else":{"properties":{"x_mana_cost":{"const":false}}}}));
}

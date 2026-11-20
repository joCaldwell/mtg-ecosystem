//! Nontargeted exile, CR 109.2, 400.7, 608.2h, 701.11, 903.9a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub struct SetExileClause {
    pub set: ObjectSet,
    pub what: Value,
    pub successors: Value,
}

impl SetExileClause {
    pub fn parse(text: &str) -> Option<Self> {
        let (set, what, successors) = match text {
            "Exile all artifacts." => (
                ObjectSet::Artifacts,
                json!({"ref":"all","filter":{"types":["artifact"]}}),
                json!({"types":["artifact"],"cls":"card"}),
            ),
            "Exile all graveyards." => (
                ObjectSet::GraveyardCards,
                json!({"ref":"zone-cards","zone":"graveyard","owner":{"player":"each-player"}}),
                json!({"cls":"card"}),
            ),
            "Exile all cards from all graveyards." => {
                let filter =
                    json!({"cls":"card","zone":{"zone":"graveyard","owner":"all-players"}});
                (
                    ObjectSet::GraveyardCards,
                    json!({"ref":"all","filter":filter}),
                    filter,
                )
            }
            _ => return None,
        };
        Some(Self {
            set,
            what,
            successors,
        })
    }
    pub fn ability(&self) -> Value {
        json!({"kind":"spell","effects":[{"steps":[{"effect":"exile","what":self.what}]}]})
    }
}

fn reserved(p: &SpellProgram) -> bool {
    let selected =
        |set: &ObjectSet| matches!(set, ObjectSet::Artifacts | ObjectSet::GraveyardCards);
    p.instructions.iter().any(|i| match i {
        Instruction::MoveSet { set, .. }
        | Instruction::DestroySet { set, .. }
        | Instruction::PumpSet { set, .. }
        | Instruction::GrantKeywordsSet { set, .. } => selected(set),
        Instruction::DamageSet { sets, .. } => sets.iter().any(selected),
        _ => false,
    }) || p.modes.iter().any(|m| reserved(&m.program))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let fail = || "set exile requires its complete printed ordinary spell and zone".to_owned();
    if d.activated.iter().any(|a| reserved(&a.program))
        || d.triggered.iter().any(|a| reserved(&a.program))
    {
        return Err(fail());
    }
    let uses = d.spell.as_ref().is_some_and(reserved);
    let raw = &d.provenance.source_json;
    if !uses && !raw.contains("Exile all ") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let Some(clause) = SetExileClause::parse(text) else {
        return if uses
            || text
                .lines()
                .any(|line| SetExileClause::parse(line).is_some())
        {
            Err(fail())
        } else {
            Ok(())
        };
    };
    let (type_line, timing) = match d.characteristics.kind {
        CardKind::Sorcery => ("Sorcery", SpellTiming::MainPhase),
        CardKind::Instant => ("Instant", SpellTiming::Instant),
        _ => return Err(fail()),
    };
    let p = d.spell.as_ref().ok_or_else(fail)?;
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    if source["name"] != d.name
        || source["id"] != d.provenance.printing_id
        || source["oracle_id"] != d.oracle_id
        || source["layout"] != "normal"
        || source["type_line"] != type_line
        || ast != json!([clause.ability()])
        || d.characteristics.basic
        || d.characteristics.legendary
        || d.characteristics.snow
        || !d.characteristics.subtypes.is_empty()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || d.x_mana_cost
        || p.timing != timing
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !matches!(p.instructions.as_slice(),[Instruction::MoveSet{set,action:SetMove::Exile,source_block:0}] if set==&clause.set)
        || d.source_labels
            .as_ref()
            .is_some_and(|l| l.blocks.as_slice() != [text])
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn extend_schema(s: &mut Value) {
    let sets = json!({"enum":[ObjectSet::Artifacts,ObjectSet::GraveyardCards]});
    let free = json!({"$ref":"#/$defs/FixedExileSetFreeProgram"});
    s["$defs"]["FixedExileSetFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":{"anyOf":[
        {"required":["set"],"properties":{"set":sets}},{"required":["sets"],"properties":{"sets":{"contains":sets}}}
    ]}}},"modes":{"items":{"properties":{"program":free}}}}});
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    let shape = json!({"required":["spell"],"properties":{"triggered":{"maxItems":0},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
        "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"x_mana_cost":{"const":false},
        "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"legendary":{"const":false},"basic":{"const":false},
            "snow":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0}}},
        "spell":{"type":"object","properties":{"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},"targets":{"maxItems":0},
            "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":{"required":["op","action","set","source_block"],"properties":{
                "op":{"const":"move_set"},"action":{"const":"exile"},"set":sets,"source_block":{"const":0}}}}}},
        "capabilities":{"contains":{"const":Capability::BoardWipesV1}}}});
    let def = s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap();
    def.push(json!({"properties":{"triggered":{"items":{"properties":{"program":free}}},"activated":{"items":{"properties":{"program":free}}}},"if":has,"then":shape}));
    for (pattern, set) in [
        (
            r#""oracle_text"\s*:\s*"Exile all artifacts\.""#,
            ObjectSet::Artifacts,
        ),
        (
            r#""oracle_text"\s*:\s*"Exile all (cards from all )?graveyards\.""#,
            ObjectSet::GraveyardCards,
        ),
    ] {
        def.push(json!({"if":{"properties":{"provenance":{"properties":{"source_json":{"pattern":pattern}}}}},"then":{"allOf":[shape,
            {"properties":{"spell":{"properties":{"instructions":{"items":{"properties":{"set":{"const":set}}}}}}}}]}}));
    }
}

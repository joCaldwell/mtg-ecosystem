//! Bind the source spell and the separate announcement target (CR 201.5,
//! 601.2c, 608.2c, 701.8a).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn validate_binding(d: &Definition, binding: &Value) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::SelfSpellProtectionV1) {
        return Ok(());
    }
    let source: Value = decode(&d.provenance.source_json, "source")?;
    let abilities: Value = decode(&d.provenance.abilities_json, "abilities")?;
    let self_spell = &abilities[0]["effect"]["what"];
    let target = &abilities[1]["effects"][0]["steps"][0]["what"];
    let root = format!("card:{}", d.oracle_id);
    let source_id = format!("{root}/source#object");
    let face_id = format!("{root}/faces/0#face");
    let static_path = "/faces/0/abilities/0";
    let spell_path = "/faces/0/abilities/1";
    let static_id = format!("{root}{static_path}#ability");
    let spell_id = format!("{root}{spell_path}#ability");
    let static_you = format!("{root}{static_path}#you");
    let spell_you = format!("{root}{spell_path}#you");
    let destroy_path = format!("{spell_path}/effects/0/steps/0");
    let target_path = format!("{destroy_path}/what");
    let target_id = format!("{spell_id}/targets/line%3A1%3A1");
    let expected = json!({
        "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
        "identity":d.oracle_id,"rules":{
            "edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"
        },
        "nodes":[
            {"id":source_id,"path":"/source","node":"source","identity":d.oracle_id,"rules":["201.5"]},
            {"id":face_id,"path":"/faces/0","node":"face","source":source_id,"sourceFaceIndex":0,
             "name":d.name,"layout":source["layout"],"typeLine":source["type_line"],
             "manaCost":source["mana_cost"],"rules":["712"]},
            {"id":static_id,"path":static_path,"node":"ability","source":source_id,"face":face_id,
             "abilityKind":"static","you":static_you,"rules":["113","109.5"]},
            {"id":static_you,"path":static_path,"node":"runtime-player","scope":static_id,
             "role":"static-source-controller","rules":["109.5"]},
            {"id":spell_id,"path":spell_path,"node":"ability","source":source_id,"face":face_id,
             "abilityKind":"spell","you":spell_you,"rules":["113","109.5"]},
            {"id":spell_you,"path":spell_path,"node":"runtime-player","scope":spell_id,
             "role":"spell-controller","rules":["109.5"]},
            {"id":target_id,"path":target_path,"node":"selection","scope":spell_id,"expression":target,
             "cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]},
            {"id":format!("{root}{destroy_path}#event"),"path":destroy_path,"node":"event",
             "scope":spell_id,"operation":"destroy","inputs":[target_id],
             "availability":"when-performed","rules":["608.2c"]}
        ],
        "references":[
            {"path":format!("{static_path}/effect/what"),"expression":self_spell,
             "binding":"bound","target":source_id,"rules":["201.5"]},
            {"path":target_path,"expression":target,"binding":"bound","target":target_id,"rules":["601.2c"]}
        ],
        "diagnostics":[],"counts":{"obligations":2,"bound":2,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true
    });
    if *binding != expected {
        return Err(unsupported(
            "binding",
            "self-spell protection requires complete source-spell and destruction target bindings",
        ));
    }
    Ok(())
}

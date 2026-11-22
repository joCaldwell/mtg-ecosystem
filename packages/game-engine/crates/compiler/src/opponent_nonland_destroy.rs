//! CR 109.5, 205.2b, 608.2h, 701.8a/b and 806.1:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(source: &Source, binding: &Value) -> Result<Option<SpellProgram>, Failure> {
    if source.oracle_text != "Destroy all nonland permanents your opponents control." {
        return Ok(None);
    }

    // Bind the complete archive graph. Definition validation checks the raw
    // source and AST, preserving the difference between absent and null fields.
    let what = json!({"ref":"all","filter":{
        "nonTypes":["land"],"cls":"permanent","control":"opponent"
    }});
    let root = format!("card:{}", source.oracle_id);
    let source_id = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let path = "/faces/0/abilities/0";
    let scope = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let step = format!("{path}/effects/0/steps/0");
    let selection = format!("{step}/what");
    let query = format!("{root}{selection}#query");
    let control = format!("{selection}/filter/control");
    let opponents = format!("{root}{control}#opponents");
    let event = format!("{root}{step}#event");
    let expected = json!({
        "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
        "identity":source.oracle_id,"rules":{
            "edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"
        },
        "nodes":[
            {"id":source_id,"path":"/source","node":"source","identity":source.oracle_id,"rules":["201.5"]},
            {"id":face,"path":"/faces/0","node":"face","source":source_id,"sourceFaceIndex":0,
             "name":source.name,"layout":source.layout,"typeLine":source.type_line,"manaCost":source.mana_cost,
             "rules":["712"]},
            {"id":scope,"path":path,"node":"ability","source":source_id,"face":face,
             "abilityKind":"spell","you":you,"rules":["113","109.5"]},
            {"id":you,"path":path,"node":"runtime-player","scope":scope,"role":"spell-controller",
             "rules":["109.5"]},
            {"id":opponents,"path":control,"node":"derived-player","scope":scope,"role":"opponents",
             "of":you,"rules":["102.2","109.5"]},
            {"id":query,"path":selection,"node":"selection","scope":scope,"expression":what,
             "cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]},
            {"id":event,"path":step,"node":"event","scope":scope,"operation":"destroy",
             "inputs":[query],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{root}{step}#quantity"),"path":step,"node":"event-result","event":event,
             "result":"quantity","cardinality":"one","availability":"if-produced",
             "identity":"measured-quantity","rules":["608.2h"]}
        ],
        "references":[
            {"path":control,"expression":{"implicit":"opponents"},"binding":"bound","target":opponents,"rules":["102.2"]},
            {"path":selection,"expression":what,"binding":"bound","target":query,"rules":["608.2h"]}
        ],
        "diagnostics":[],"counts":{"obligations":2,"bound":2,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true
    });
    if *binding != expected {
        return Err(unsupported(
            "binding",
            "opponent nonland destruction requires a complete resolution-time controller query",
        ));
    }
    Ok(Some(SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![Instruction::act(
            Verb::Destroy {
                what: ObjectSet::OpponentNonlandPermanents.kernel(Context::Spell),
                regeneration_forbidden: false,
            },
            0,
        )],
        modes: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;

    #[test]
    fn opponent_nonland_destroy_rejects_announcement_binding() {
        const HEADER: &str =
            include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
        const SOURCE: &str =
            include_str!("../../../tests/fixtures/ramos/a6f38908-aa4f-4f99-a28e-85d11dab52e4.json");
        let variant = changed_source(SOURCE, |record| {
            record["binding"]["nodes"][5]["timing"] = json!("stack-announcement");
        });
        assert!(matches!(
            compile(HEADER, &variant),
            CompileResult::Unsupported { path, .. } if path == "binding"
        ));
    }
}

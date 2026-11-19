//! CR 201.5, 205.2b, 603.3a, 603.10a, 608.2h, 700.4, 701.8 and 701.19c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const DESTROY_TEXT: &str = "destroy all nonland permanents. They can't be regenerated.";

fn selection() -> Value {
    json!({"ref":"all","filter":{"nonTypes":["land"],"cls":"permanent"}})
}

pub(super) fn bind(source: &Source, result: &ParseResult, binding: &Value) -> Result<(), Failure> {
    if !source.oracle_text.contains(DESTROY_TEXT) {
        return Ok(());
    }

    let what = selection();
    let source_ref = json!({"ref":"self"});
    let abilities: Value = decode(result.abilities.get(), "abilities")?;
    if source.oracle_text != format!("Trample\nWhen {} dies, {DESTROY_TEXT}", source.name)
        || abilities != json!([
            {"kind":"keywords","keywords":[{"keyword":"trample"}]},
            {"kind":"triggered","trigger":{"trigger":"dies","what":source_ref},
             "effects":[{"steps":[{"effect":"destroy","what":what,"regenerationForbidden":true}]}]}
        ])
    {
        return Err(unsupported("abilities", "requires complete source-dies nonland destruction"));
    }

    // Bind the whole archive, including the unconsumed successor and quantity
    // nodes. The executable trigger only observes the actual death.
    let root = format!("card:{}", source.oracle_id);
    let source_id = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let keyword_block = "/faces/0/abilities/0";
    let keyword_scope = format!("{root}{keyword_block}#ability");
    let keyword_you = format!("{root}{keyword_block}#you");
    let keyword = format!("{keyword_block}/keywords/0");
    let keyword_controller = format!("{root}{keyword}#you");
    let path = "/faces/0/abilities/1";
    let scope = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let trigger = format!("{path}/trigger");
    let death = format!("{root}{trigger}#event");
    let step = format!("{path}/effects/0/steps/0");
    let selector = format!("{step}/what");
    let query = format!("{root}{selector}#query");
    let destruction = format!("{root}{step}#event");
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
            {"id":keyword_scope,"path":keyword_block,"node":"ability","source":source_id,"face":face,
             "abilityKind":"keywords","you":keyword_you,"rules":["113","109.5"]},
            {"id":keyword_you,"path":keyword_block,"node":"runtime-player","scope":keyword_scope,
             "role":"static-source-controller","rules":["109.5"]},
            {"id":keyword_controller,"path":keyword,"node":"runtime-player","scope":keyword_scope,
             "role":"static-source-controller","rules":["109.5","702.19"]},
            {"id":format!("{root}{keyword}#keyword"),"path":keyword,"node":"keyword-ability",
             "scope":keyword_scope,"keyword":"trample","abilities":["static"],"you":keyword_controller,
             "rules":["702.19"]},
            {"id":scope,"path":path,"node":"ability","source":source_id,"face":face,
             "abilityKind":"triggered","you":you,"rules":["113","109.5"]},
            {"id":you,"path":path,"node":"runtime-player","scope":scope,"role":"trigger-controller",
             "rules":["109.5"]},
            {"id":death,"path":trigger,"node":"event","scope":scope,"operation":"trigger:dies",
             "inputs":[source_id],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{root}{trigger}#objects"),"path":trigger,"node":"event-result","event":death,
             "result":"objects","cardinality":"one","availability":"if-produced","identity":"same-objects",
             "rules":["608.2h"]},
            {"id":format!("{root}{trigger}#successor"),"path":trigger,"node":"event-result","event":death,
             "result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors",
             "description":{"cls":"card"},"rules":["400.7e","603.6c"]},
            {"id":query,"path":selector,"node":"selection","scope":scope,"expression":what,
             "cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]},
            {"id":destruction,"path":step,"node":"event","scope":scope,"operation":"destroy",
             "inputs":[query],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{root}{step}#quantity"),"path":step,"node":"event-result","event":destruction,
             "result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity",
             "rules":["608.2h"]}
        ],
        "references":[
            {"path":format!("{trigger}/what"),"expression":source_ref,"binding":"bound","target":source_id,
             "rules":["201.5"]},
            {"path":selector,"expression":what,"binding":"bound","target":query,"rules":["608.2h"]}
        ],
        "diagnostics":[],"counts":{"obligations":2,"bound":2,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true
    });
    if *binding != expected {
        return Err(unsupported("binding", "requires complete source death and resolution-time nonland selection"));
    }
    Ok(())
}

pub(super) fn instruction(
    condition: &TriggerCondition,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: u32,
) -> Option<Instruction> {
    let [Sentence {
        condition: None,
        condition_kind: None,
        bindings,
        steps,
    }] = effects
    else {
        return None;
    };
    if *condition == TriggerCondition::SelfDies
        && intervening.is_none()
        && bindings.is_empty()
        && matches!(steps.as_slice(),
            [Step::Destroy { what, optional: None, regeneration_forbidden: Some(true) }]
                if *what == selection())
    {
        Some(Instruction::DestroySet {
            set: ObjectSet::NonlandPermanents,
            regeneration_forbidden: true,
            source_block: block,
        })
    } else {
        None
    }
}

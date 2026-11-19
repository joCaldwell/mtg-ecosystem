//! CR 109.5, 205.2b, 601.2c, 608.2b/c and 701.13a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn validate_binding(
    source: &Source,
    result: &ParseResult,
    binding: &Value,
) -> Result<(), Failure> {
    if !source
        .oracle_text
        .contains("Exile target nonland permanent. You lose ")
    {
        return Ok(());
    }

    // The ordinary lowering still consumes the effects. This verifies the
    // archive edges and their precise roles, including the unused results.
    let abilities: Value = decode(result.abilities.get(), "abilities")?;
    let what = &abilities[0]["effects"][0]["steps"][0]["what"];
    let you_expression = json!({"player":"you"});
    let root = format!("card:{}", source.oracle_id);
    let source_id = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let path = "/faces/0/abilities/0";
    let scope = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    // The sole target is the second token in the first source line.
    let target = format!("{scope}/targets/line%3A0%3A1");
    let exile = format!("{path}/effects/0/steps/0");
    let life = format!("{path}/effects/1/steps/0");
    let target_path = format!("{exile}/what");
    let exile_event = format!("{root}{exile}#event");
    let life_event = format!("{root}{life}#event");
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
            {"id":target,"path":target_path,"node":"selection","scope":scope,"expression":what,
             "cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]},
            {"id":exile_event,"path":exile,"node":"event","scope":scope,"operation":"exile",
             "inputs":[target],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{root}{exile}#objects"),"path":exile,"node":"event-result","event":exile_event,
             "result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors",
             "description":{"cls":"card","nonTypes":["land"]},"rules":["400.7j"]},
            {"id":life_event,"path":life,"node":"event","scope":scope,"operation":"lose-life",
             "inputs":[you],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{root}{life}#quantity"),"path":life,"node":"event-result","event":life_event,
             "result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity",
             "rules":["608.2h"]}
        ],
        "references":[
            {"path":target_path,"expression":what,"binding":"bound","target":target,"rules":["601.2c"]},
            {"path":format!("{life}/who"),"expression":you_expression,"binding":"bound","target":you,
             "rules":["109.5"]}
        ],
        "diagnostics":[],"counts":{"obligations":2,"bound":2,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true
    });
    if *binding != expected {
        return Err(unsupported(
            "binding",
            "nonland exile and life loss require the complete target and spell-controller bindings",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;

    const HEADER: &str =
        include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../tests/fixtures/ramos/ad09b3c3-c8e7-481c-8c45-e7f234935117.json");

    #[test]
    fn nonland_exile_life_complete_source_and_ir_fences() {
        assert_eq!(
            sha256(SOURCE.as_bytes()),
            "4aa2dee7de9601766d194fd9ab8fa583740504fc590793a90841ddbf3bf24a79"
        );
        let variant = changed_source(SOURCE, |record| {
            let text = "Exile target nonland permanent. You lose 5 life.";
            record["source"]["oracle_text"] = json!(text);
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][1]["steps"][0]["amount"]["value"] = json!(5);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            result["lines"][0]["text"] = json!(text);
        });
        let CompileResult::Lowered { definition } = compile(HEADER, &variant) else {
            panic!("{:?}", compile(HEADER, &variant));
        };
        assert_eq!(
            definition.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastInstantV1,
                Capability::PermanentRemovalV1,
                Capability::LifeLossV1,
            ])
        );
        let mut forged = *definition;
        let program = forged.spell.as_mut().unwrap();
        assert!(matches!(
            program.instructions.as_slice(),
            [Instruction::ExilePermanent { target_slot: 0, .. },
             Instruction::LoseLife { recipient: LifeRecipient::Controller, amount, .. }]
                if *amount == 5.into()
        ));
        program.instructions.pop();
        forged.capabilities = forged.requirements();
        assert!(forged.validate().is_err());

        let extra_clause = changed_source(SOURCE, |record| {
            let text = "Exile target nonland permanent. You lose 3 life. Draw a card.";
            record["source"]["oracle_text"] = json!(text);
            record["faces"][0]["result"]["lines"][0]["text"] = json!(text);
        });
        assert!(matches!(
            compile(HEADER, &extra_clause),
            CompileResult::Unsupported { .. }
        ));

        let null_selector = changed_source(SOURCE, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][1]["steps"][0]["who"]["of"] = Value::Null;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(matches!(
            compile(HEADER, &null_selector),
            CompileResult::Unsupported { .. }
        ));

        let wrong_binding = changed_source(SOURCE, |record| {
            record["binding"]["nodes"][3]["role"] = json!("source-owner");
        });
        assert!(matches!(
            compile(HEADER, &wrong_binding),
            CompileResult::Unsupported { path, .. } if path == "binding"
        ));
    }
}

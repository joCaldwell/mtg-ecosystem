//! CR 109.5, 113.7a, 120.4, 603.2c/3a/10, 605.1, 608.2h/k, 701.26a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const MANA_LINE: &str = "{T}: Add one mana of any color.";

pub(super) fn lower(
    abilities: &[Ability],
    parsed: &ParseResult,
    binding: &Value,
    card: &Source,
) -> Result<Option<TriggeredAbility>, Failure> {
    let has_trigger = abilities.iter().any(|ability| {
        matches!(
            ability,
            Ability::Triggered { trigger, .. }
                if matches!(trigger.as_ref(), SourceTrigger::BecomesTapped { .. })
        )
    });
    let has_binding = binding["nodes"].as_array().is_some_and(|nodes| {
        nodes.iter().any(|node| node["operation"] == "trigger:becomes-tapped")
    });
    if !has_trigger && !has_binding && !card.oracle_text.contains("becomes tapped") {
        return Ok(None);
    }
    let bad = || {
        unsupported(
            "trigger.becomes-tapped",
            "requires complete source tap damage and separate any-color mana ability",
        )
    };
    let Some((trigger_line, mana_line)) = card.oracle_text.split_once('\n') else {
        return Err(bad());
    };
    let [Ability::Triggered { trigger, .. }, Ability::Activated { .. }] = abilities else {
        return Err(bad());
    };
    let amount = trigger_line
        .strip_prefix("Whenever this land becomes tapped, it deals ")
        .and_then(|text| text.strip_suffix(" damage to you."))
        .and_then(|text| text.parse::<u32>().ok())
        .filter(|amount| *amount > 0)
        .ok_or_else(bad)?;
    let you = json!({"player":"you"});
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if card.type_line != "Land"
        || !card.mana_cost.is_empty()
        || !matches!(
            trigger.as_ref(),
            SourceTrigger::BecomesTapped { what: TriggerObject::This { noun } } if noun == "land"
        )
        || mana_line != MANA_LINE
        || trigger_line != format!("Whenever this land becomes tapped, it deals {amount} damage to you.")
        || raw != json!([
            {"kind":"triggered","trigger":{"trigger":"becomes-tapped","what":{"ref":"this","noun":"land"}},
             "effects":[{"steps":[{"effect":"damage","source":{"ref":"it"},
                 "amount":{"amount":"fixed","value":amount},"to":{"to":"player","ref":you}}]}]},
            {"kind":"activated","costs":[{"cost":"tap-self"}],
             "effects":[{"steps":[{"effect":"add-mana","who":you,
                 "mana":{"mana":"any-color","amount":{"amount":"fixed","value":1}}}]}]}
        ])
    {
        return Err(bad());
    }
    validate_binding(binding, card)?;

    // The ordinary activated-mana lowerer consumes the second block.
    Ok(Some(TriggeredAbility {
        death_filter: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfBecomesTapped,
        intervening_if: None,
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::DealDamageToController {
                amount: i64::from(amount).into(),
                source_block: 0,
            }],
            modes: vec![],
        },
    }))
}

fn validate_binding(binding: &Value, card: &Source) -> Result<(), Failure> {
    let root = format!("card:{}", card.oracle_id);
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let trigger = "/faces/0/abilities/0";
    let mana = "/faces/0/abilities/1";
    let trigger_scope = format!("{root}{trigger}#ability");
    let mana_scope = format!("{root}{mana}#ability");
    let controller = format!("{root}{trigger}#you");
    let activator = format!("{root}{mana}#you");
    let tap = format!("{trigger}/trigger");
    let tap_event = format!("{root}{tap}#event");
    let tapped = format!("{root}{tap}#objects");
    let damage = format!("{trigger}/effects/0/steps/0");
    let damage_event = format!("{root}{damage}#event");
    let add_mana = format!("{mana}/effects/0/steps/0");
    let expected = json!({
        "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
        "identity":card.oracle_id,"rules":{
            "edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"
        },
        "nodes":[
            {"id":source,"path":"/source","node":"source","identity":card.oracle_id,"rules":["201.5"]},
            {"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
             "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost,
             "rules":["712"]},
            {"id":trigger_scope,"path":trigger,"node":"ability","source":source,"face":face,
             "abilityKind":"triggered","you":controller,"rules":["113","109.5"]},
            {"id":controller,"path":trigger,"node":"runtime-player","scope":trigger_scope,
             "role":"trigger-controller","rules":["109.5"]},
            {"id":tap_event,"path":tap,"node":"event","scope":trigger_scope,
             "operation":"trigger:becomes-tapped","inputs":[source],"availability":"when-performed","rules":["608.2c"]},
            {"id":tapped,"path":tap,"node":"event-result","event":tap_event,
             "result":"objects","cardinality":"one","availability":"if-produced","identity":"same-objects",
             "description":{"types":["land"]},"rules":["608.2h"]},
            {"id":damage_event,"path":damage,"node":"event","scope":trigger_scope,"operation":"damage",
             "inputs":[tapped,controller],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{root}{damage}#quantity"),"path":damage,"node":"event-result","event":damage_event,
             "result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]},
            {"id":mana_scope,"path":mana,"node":"ability","source":source,"face":face,
             "abilityKind":"activated","you":activator,"rules":["113","109.5"]},
            {"id":activator,"path":mana,"node":"runtime-player","scope":mana_scope,
             "role":"activator","rules":["109.5"]},
            {"id":format!("{root}{add_mana}#event"),"path":add_mana,"node":"event","scope":mana_scope,
             "operation":"add-mana","inputs":[activator],"availability":"when-performed","rules":["608.2c"]}
        ],
        "references":[
            {"path":format!("{tap}/what"),"expression":{"ref":"this","noun":"land"},"binding":"bound","target":source,"rules":["201.5"]},
            {"path":format!("{damage}/source"),"expression":{"ref":"it"},"binding":"bound","target":tapped,"rules":["608.2k","400.7j","608.2h"]},
            {"path":format!("{damage}/to/ref"),"expression":{"player":"you"},"binding":"bound","target":controller,"rules":["109.5"]},
            {"path":format!("{mana}/costs/0/source"),"expression":{"ref":"self"},"binding":"bound","target":source,"rules":["201.5"]},
            {"path":format!("{mana}/costs/0/payer"),"expression":{"player":"you"},"binding":"bound","target":activator,"rules":["109.5"]},
            {"path":format!("{add_mana}/who"),"expression":{"player":"you"},"binding":"bound","target":activator,"rules":["109.5"]}
        ],
        "diagnostics":[],"counts":{"obligations":6,"bound":6,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true
    });
    if *binding != expected {
        return Err(unsupported(
            "binding.becomes-tapped",
            "requires exact tap, tapped source, trigger controller and mana bindings",
        ));
    }
    Ok(())
}

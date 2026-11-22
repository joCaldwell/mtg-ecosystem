//! Persistent land allowance, own-graveyard land play and controlled-entry mill.
//! CR 305.1–3, 611.3, 603.3b/603.6a and 701.17 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ALLOWANCE: &str = "You may play an additional land on each of your turns.";
const GRAVEYARD: &str = "You may play lands from your graveyard.";
const TRIGGER: &str = "Landfall — Whenever a land you control enters, mill a card.";
const INVALID: &str =
    "requires the complete persistent land allowance, graveyard permission and land-entry mill";

pub fn land_reclamation_source(text: &str) -> Option<Value> {
    if !text.split('\n').eq([ALLOWANCE, GRAVEYARD, TRIGGER]) {
        return None;
    }
    Some(json!([
        {"kind":"static","effect":{"static":"additional-land-plays","allowance":{"who":{"player":"you"},
            "additional":{"amount":"fixed","value":1}}}},
        {"kind":"static","effect":{"static":"play-permission","permission":{"who":{"player":"you"},
            "action":"play","what":{"ref":"each","filter":{"types":["land"]}},"from":{"zone":"graveyard","owner":"your"}}}},
        {"kind":"triggered","abilityWord":"Landfall","trigger":{"trigger":"enters","what":controlled_land_query()},
            "effects":[{"steps":[{"effect":"mill","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}
    ]))
}

pub fn land_reclamation_statics() -> [StaticAbility; 2] {
    [
        StaticEffect::AdditionalLandPlay,
        StaticEffect::PlayLandsFromOwnGraveyard,
    ]
    .map(|effect| StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect,
    })
}

pub fn land_reclamation_trigger() -> TriggeredAbility {
    let mut trigger = controlled_land_entry_trigger(1.into(), false);
    trigger.program.instructions = vec![Instruction::mill(
        kir::PlayerExpr::you(kir::Context::Triggered),
        kir::Amount::Fixed { value: 1.into() },
        2,
    )];
    trigger
}

pub(super) fn present(d: &Definition) -> bool {
    d.static_abilities.iter().any(|a| {
        matches!(
            a.effect,
            StaticEffect::AdditionalLandPlay | StaticEffect::PlayLandsFromOwnGraveyard
        )
    })
}

fn permanent_spell() -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
    }
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    if !present(d)
        && !d.capabilities.contains(&Capability::LandReclamationV1)
        && !(raw.contains(ALLOWANCE) && raw.contains(GRAVEYARD))
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let Some(text) = source["oracle_text"].as_str() else {
        return Err(INVALID.into());
    };
    let Some(ast) = land_reclamation_source(text) else {
        return if present(d) || d.capabilities.contains(&Capability::LandReclamationV1) {
            Err(INVALID.into())
        } else {
            Ok(())
        };
    };
    let c = &d.characteristics;
    if c.kind != CardKind::Creature
        || c.legendary
        || c.snow
        || c.basic
        || c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || !c.hybrid_mana.is_empty()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || d.spell.as_ref() != Some(&permanent_spell())
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.keywords.is_empty()
        || d.static_abilities.as_slice() != land_reclamation_statics()
        || d.triggered.as_slice() != [land_reclamation_trigger()]
        || source["layout"] != "normal"
        || source["type_line"] != format!("Creature — {}", c.subtypes.join(" "))
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["keywords"] != json!(["Mill", "Landfall"])
        || source["color_identity"] != json!(c.color_identity)
        || source["colors"] != json!(c.colors)
        || source["power"] != json!(c.power.as_ref().map(ToString::to_string))
        || source["toughness"] != json!(c.toughness.as_ref().map(ToString::to_string))
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != [ALLOWANCE, GRAVEYARD, TRIGGER]
                || labels.statics != [0, 1]
                || labels.triggered != [2]
                || !labels.activated.is_empty()
                || !labels.mana.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn selector() -> Value {
    json!({"anyOf":[
        {"required":["static_abilities"],"properties":{"static_abilities":{"contains":{
            "required":["effect"],"properties":{"effect":{"enum":[StaticEffect::AdditionalLandPlay,StaticEffect::PlayLandsFromOwnGraveyard]}}
        }}}},
        {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"You may play an additional land on each of your turns\.\\nYou may play lands from your graveyard\."#}}}}}
    ]})
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let caps = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        LandReclamationV1,
        ControlledLandEntryV1,
        TriggeredAbilityV1,
        FixedMillProgramV1,
    ];
    let cap = json!({"contains":{"const":LandReclamationV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":selector(),
        "then":{"required":["spell","static_abilities","triggered"],"properties":{
            "spell":{"const":permanent_spell()},
            "capabilities":{"items":{"enum":caps},"allOf":caps.map(|c|json!({"contains":{"const":c}}))},
            "characteristics":{"properties":{"kind":{"const":"creature"},"legendary":{"const":false},
                "basic":{"const":false},"snow":{"const":false},"hybrid_mana":{"maxItems":0},
                "additional_types":{"maxItems":0},"subtypes":{"minItems":1}}},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "activated":{"maxItems":0},"keywords":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "static_abilities":{"const":land_reclamation_statics()},"triggered":{"const":[land_reclamation_trigger()]}
        }},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let raw: Value = strict_read(include_str!(
        "../../../tests/fixtures/109cdefd-e8cc-4ac7-b6ba-2cfdef8d780f.json"
    ))
    .unwrap();
    let mut d = controlled_land_entry::witness();
    let source = &raw["source"];
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = raw["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&raw["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = raw["bindingSha256"].as_str().unwrap().into();
    d.characteristics.subtypes = vec!["Insect".into(), "Scout".into()];
    let stats = |name| source[name].as_str().unwrap().parse::<Quantity>().unwrap();
    d.characteristics.power = Some(stats("power"));
    d.characteristics.toughness = Some(stats("toughness"));
    let spell_cost = source["mana_cost"].as_str().unwrap();
    assert_eq!(spell_cost, "{2}{G}{G}");
    d.characteristics.mana_cost = ManaCost {
        generic: 2.into(),
        colored: vec![Color::G, Color::G],
    };
    d.static_abilities = land_reclamation_statics().to_vec();
    d.triggered = vec![land_reclamation_trigger()];
    d.capabilities = d.requirements();
    d
}

#[test]
fn land_reclamation_contract_rejects_missing_permissions_and_mill() {
    let mut d = witness();
    d.validate().unwrap();
    d.static_abilities.pop();
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}

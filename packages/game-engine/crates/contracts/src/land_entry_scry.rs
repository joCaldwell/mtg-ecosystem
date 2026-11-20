//! An entering land's own mandatory scry. CR 603.6a, 614.12 and 701.22
//! (2026-09-25). https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ENTRY: &str = "This land enters tapped.";
const SCRY: &str = "When this land enters, scry 1.";
const REMINDER: &str =
    " (Look at the top card of your library. You may put that card on the bottom.)";
const INVALID: &str = "requires a complete tapped dual land with one mandatory entry scry";

pub fn land_entry_scry_trigger() -> TriggeredAbility {
    TriggeredAbility {
        condition: TriggerCondition::SelfEntersBattlefield,
        death_filter: None,
        intervening_if: None,
        functioning_zone: FunctioningZone::Battlefield,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            targets: vec![],
            modes: vec![],
            instructions: vec![Instruction::Scry {
                amount: 1.into(),
                source_block: 1,
            }],
        },
    }
}

pub(super) fn present(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Land
        && d.triggered.iter().any(|trigger| {
            trigger.condition == TriggerCondition::SelfEntersBattlefield
                && controller_library::has_look(&trigger.program)
        })
}

fn tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if !present(d) && !d.capabilities.contains(&Capability::LandEntryScryV1) {
        let raw = &d.provenance.source_json;
        if raw.contains(SCRY) || raw.contains(r"\u") {
            let source: Value = strict_read(raw)?;
            if source["type_line"] == "Land"
                && source["oracle_text"]
                    .as_str()
                    .is_some_and(|text| text.lines().any(|line| line.starts_with(SCRY)))
            {
                return Err(INVALID.into());
            }
        }
        return Ok(());
    }
    let c = &d.characteristics;
    let [mana] = d.mana_abilities.as_slice() else {
        return Err(INVALID.into());
    };
    let ManaProduction::Choice { colors } = &mana.production else {
        return Err(INVALID.into());
    };
    let [first, second] = colors.as_slice() else {
        return Err(INVALID.into());
    };
    listed_mana_choice::validate(d)?;
    if c.kind != CardKind::Land
        || c.basic
        || c.legendary
        || c.snow
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || !c.colors.is_empty()
        || !c.hybrid_mana.is_empty()
        || !c.mana_cost.colored.is_empty()
        || c.mana_cost.generic != Quantity::zero()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || d.spell.is_some()
        || d.x_mana_cost
        || d.intrinsic_zone != Some(FunctioningZone::Battlefield)
        || d.static_abilities.as_slice() != [tapped()]
        || d.triggered.as_slice() != [land_entry_scry_trigger()]
        || mana.source_block != 2
    {
        return Err(INVALID.into());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(INVALID)?;
    let printed_mana = format!("{{T}}: Add {{{first:?}}} or {{{second:?}}}.");
    let lines = text.split('\n').collect::<Vec<_>>();
    let [entry, scry, mana_line] = lines.as_slice() else {
        return Err(INVALID.into());
    };
    let produced: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| INVALID)?;
    let set = |colors: &[Color]| colors.iter().copied().collect::<BTreeSet<_>>();
    if *entry != ENTRY
        || !(*scry == SCRY || scry.strip_suffix(REMINDER) == Some(SCRY))
        || *mana_line != printed_mana
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["type_line"] != "Land"
        || source["layout"] != "normal"
        || source["mana_cost"] != ""
        || source["colors"] != json!([])
        || (source["keywords"] != json!([]) && source["keywords"] != json!(["Scry"]))
        || source["color_identity"] != json!(c.color_identity)
        || c.color_identity.len() != 2
        || set(&c.color_identity) != set(colors)
        || produced.len() != 2
        || set(&produced) != set(colors)
    {
        return Err(INVALID.into());
    }
    let subject = json!({"ref":"this","noun":"land"});
    let ast = json!([
        {"kind":"static","effect":{"static":"enters-tapped","what":subject}},
        {"kind":"triggered","trigger":{"trigger":"enters","what":subject},
            "effects":[{"steps":[{"effect":"scry","amount":{"amount":"fixed","value":1}}]}]},
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana",
            "who":{"player":"you"},"mana":{"mana":"choice","options":[{"mana":"fixed","symbols":[first]},
                {"mana":"fixed","symbols":[second]}]}}]}]}
    ]);
    if strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != [ENTRY, SCRY, printed_mana.as_str()]
                || labels.statics != [0]
                || labels.triggered != [1]
                || labels.mana != [2]
                || !labels.activated.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn selector() -> Value {
    json!({"required":["characteristics","triggered"],"properties":{
        "characteristics":{"properties":{"kind":{"const":"land"}}},"triggered":{"contains":{
            "required":["condition","program"],"properties":{
                "condition":{"const":"self_enters_battlefield"},"program":{"not":{"$ref":"#/$defs/ScryFreeProgram"}}
            }
        }}
    }})
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let caps = [
        NormalLayoutV1,
        LandPlayV1,
        PermanentManaV1,
        ListedManaChoiceV1,
        EntersTappedV1,
        SelfEntersBattlefieldV1,
        TriggeredAbilityV1,
        ControllerScryV1,
        LandEntryScryV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[selector(),{"properties":{"provenance":{"properties":{
            "source_json":{"pattern":r#""oracle_text"\s*:\s*"This land enters tapped\.\\nWhen this land enters, scry 1\."#}
        }}}}]},
        "then":{"required":["intrinsic_zone","static_abilities","mana_abilities","triggered"],"properties":{
            "capabilities":{"items":{"enum":caps},"allOf":caps.map(|cap|json!({"contains":{"const":cap}}))},
            "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
            "x_mana_cost":{"const":false},"activated":{"maxItems":0},"keywords":{"maxItems":0},
            "characteristics":{"properties":{"kind":{"const":"land"},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},
                "subtypes":{"maxItems":0},"colors":{"maxItems":0},"hybrid_mana":{"maxItems":0},
                "mana_cost":{"const":ManaCost { generic: Quantity::zero(), colored: vec![] }},
                "color_identity":{"minItems":2,"maxItems":2}}},
            "static_abilities":{"const":[tapped()]},"triggered":{"const":[land_entry_scry_trigger()]},
            "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "source_block":{"const":2},"production":{"properties":{
                    "mana":{"const":"choice"},"colors":{"minItems":2,"maxItems":2}}}
            }}}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":LandEntryScryV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = land_entry_gain_life::witness();
    let record: Value = strict_read(include_str!(
        "../../../tests/fixtures/79f94050-d850-41ca-b1db-5ae0cf743f0a.json"
    ))
    .unwrap();
    let source = &record["source"];
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = record["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics.color_identity =
        serde_json::from_value(source["color_identity"].clone()).unwrap();
    d.static_abilities = vec![tapped()];
    d.triggered = vec![land_entry_scry_trigger()];
    d.mana_abilities[0].source_block = 2;
    d.mana_abilities[0].production = ManaProduction::Choice {
        colors: vec![Color::U, Color::R],
    };
    d.capabilities = d.requirements();
    d
}

#[test]
fn land_entry_scry_requires_exact_scry_size() {
    let mut definition = witness();
    definition.validate().unwrap();
    let mut omitted = definition.clone();
    omitted.triggered.clear();
    omitted.capabilities = omitted.requirements();
    assert!(omitted.validate().is_err());
    definition.triggered[0].program.instructions = vec![Instruction::Scry {
        amount: 2.into(),
        source_block: 1,
    }];
    assert!(definition.validate().is_err());
}

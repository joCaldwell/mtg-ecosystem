//! CR 305.2–3, 406.3, 607.2a/3, 608.2g–h and 702.75a: linked face-down exile.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const HIDEAWAY: &str = "Hideaway 4 (When this land enters, look at the top four cards of your library, exile one face down, then put the rest on the bottom in a random order.)";
const ENTRY: &str = "This land enters tapped.";
const MANA: &str = "{T}: Add {G}.";
const PLAY: &str = "{G}, {T}: You may play the exiled card without paying its mana cost if creatures you control have total power 10 or greater.";
const INVALID: &str = "linked hideaway requires the complete tapped land and its two printed costs";

pub fn source(text: &str) -> bool {
    text.lines().eq([HIDEAWAY, ENTRY, MANA, PLAY]) && !text.ends_with('\n')
}

pub fn abilities() -> Value {
    json!([
        {"kind":"keywords","keywords":[{"keyword":"hideaway","amount":4}]},
        {"kind":"static","effect":{"static":"enters-tapped","what":{"ref":"this","noun":"land"}}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],
            "effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]}}]}]},
        {"kind":"activated","costs":[{"cost":"mana","symbols":["G"]},{"cost":"tap-self"}],
            "effects":[{"steps":[{"effect":"play-card","optional":true,"instruction":{
                "who":{"player":"you"},"action":"play",
                "what":{"ref":"related","number":"singular","filter":{"cls":"card"},
                    "relation":{"relation":"event-result","event":"exile","scope":"contextual"}},
                "alternativeCost":{"cost":"without-mana-cost"},
                "condition":{"condition":"compare","value":{"amount":"attribute-sum","attribute":"power",
                    "among":{"ref":"each","filter":{"types":["creature"],"control":"you"}}},
                    "comparison":{"op":"ge","value":{"amount":"fixed","value":10}}}
            }}]}]}
    ])
}

fn program(instruction: Instruction) -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![instruction],
        modes: vec![],
    }
}

fn tap_cost(mana: Option<ManaCost>) -> PermanentActivationCosts {
    PermanentActivationCosts {
        remove_source_plus_one_counters: None,
        tap: true,
        mana,
        sacrifice: false,
    }
}

pub fn entry() -> TriggeredAbility {
    TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        death_filter: None,
        program: program(Instruction::Hideaway {
            count: 4,
            source_block: 0,
        }),
    }
}

pub fn mana() -> ManaAbility {
    ManaAbility {
        self_damage: None,
        restriction: None,
        costs: tap_cost(None),
        production: ManaProduction::Fixed {
            symbols: vec![Color::G],
        },
        source_block: 2,
    }
}

pub fn play() -> ActivatedAbility {
    ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: tap_cost(Some(ManaCost {
                generic: 0.into(),
                colored: vec![Color::G],
            })),
        },
        program: program(Instruction::PlayHideawayLinked {
            hideaway_block: 0,
            minimum_total_power: 10.into(),
            source_block: 3,
        }),
    }
}

pub fn tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}

const CAPABILITIES: [Capability; 12] = [
    Capability::NormalLayoutV1,
    Capability::LandPlayV1,
    Capability::EntersTappedV1,
    Capability::PermanentManaV1,
    Capability::TriggeredAbilityV1,
    Capability::SelfEntersBattlefieldV1,
    Capability::ActivatedAbilityV1,
    Capability::CompositeActivationCostV1,
    Capability::HideawayV1,
    Capability::LinkedExilePlayV1,
    Capability::RandomBottomV1,
    Capability::ResolutionCastV1,
];

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    let ast = &d.provenance.abilities_json;
    if !d.capabilities.contains(&Capability::HideawayV1)
        && !d.capabilities.contains(&Capability::LinkedExilePlayV1)
        && !raw.contains("Hideaway 4")
        && !ast.contains("\"hideaway\"")
    {
        return Ok(());
    }
    let shape: Value = strict_read(raw)?;
    let text = shape["oracle_text"].as_str().ok_or(INVALID)?;
    let c = &d.characteristics;
    if !source(text)
        || strict_read::<Value>(ast)? != abilities()
        || shape["id"] != d.provenance.printing_id
        || shape["oracle_id"] != d.oracle_id
        || shape["name"] != d.name
        || shape["layout"] != "normal"
        || shape["type_line"] != "Land"
        || shape["mana_cost"] != ""
        || shape["colors"] != json!([])
        || shape["color_identity"] != json!(["G"])
        || shape["keywords"] != json!(["Hideaway"])
        || c.kind != CardKind::Land
        || c.basic
        || c.legendary
        || c.snow
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || !c.colors.is_empty()
        || c.color_identity != [Color::G]
        || c.mana_cost.generic != Quantity::zero()
        || !c.mana_cost.colored.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.spell.is_some()
        || d.x_mana_cost
        || d.intrinsic_zone != Some(FunctioningZone::Battlefield)
        || !d.keywords.is_empty()
        || d.activated != [play()]
        || d.mana_abilities != [mana()]
        || d.triggered != [entry()]
        || d.static_abilities != [tapped()]
        || d.capabilities != BTreeSet::from(CAPABILITIES)
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != ["Hideaway 4", ENTRY, MANA, PLAY]
                || labels.activated != [3]
                || labels.triggered != [0]
                || labels.mana != [2]
                || labels.statics != [1]
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn capability_schema() -> Value {
    json!({"properties":{"capabilities":{"contains":{"const":Capability::HideawayV1}}}})
}

pub(super) fn extend_schema(schema: &mut Value) {
    let no_reserved = json!({"type":"object","properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{
            "op":{"enum":["hideaway","play_hideaway_linked"]}
        }}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/HideawayFreeProgram"}}}}
    }});
    schema["$defs"]["HideawayFreeProgram"] = no_reserved;
    let ordinary = json!({"properties":{"program":{"$ref":"#/$defs/HideawayFreeProgram"}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            capability_schema(),
            {"properties":{"capabilities":{"contains":{"const":Capability::LinkedExilePlayV1}}}},
            {"properties":{"provenance":{"properties":{"abilities_json":{"pattern":r#""keyword"\s*:\s*"hideaway""#}}}}},
            {"not":{"properties":{
                "spell":{"anyOf":[{"type":"null"},{"$ref":"#/$defs/HideawayFreeProgram"}]},
                "activated":{"items":ordinary},"triggered":{"items":ordinary}
            }}}
        ]},
        "then":{"required":["mana_abilities","triggered","activated","static_abilities","intrinsic_zone"],"properties":{
            "capabilities":{"minItems":CAPABILITIES.len(),"maxItems":CAPABILITIES.len(),"uniqueItems":true,"items":{"enum":CAPABILITIES}},
            "spell":{"type":"null"},"x_mana_cost":{"const":false},"intrinsic_zone":{"const":"battlefield"},
            "keywords":{"maxItems":0},"mana_abilities":{"const":[mana()]},
            "triggered":{"const":[entry()]},"activated":{"const":[play()]},"static_abilities":{"const":[tapped()]},
            "characteristics":{"properties":{
                "kind":{"const":"land"},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                "colors":{"maxItems":0},"color_identity":{"const":["G"]},"power":{"type":"null"},
                "toughness":{"type":"null"},"mana_cost":{"const":{"generic":"0","colored":[]}}
            }},
            "source_labels":{"properties":{"blocks":{"const":["Hideaway 4",ENTRY,MANA,PLAY]},"activated":{"const":[3]},
                "triggered":{"const":[0]},"mana":{"const":[2]},"statics":{"const":[1]},"spell_modes":{"maxItems":0}}},
            "provenance":{"properties":{
                "abilities_json":{"pattern":r#""keyword"\s*:\s*"hideaway""#},
                "source_json":{"pattern":r#""oracle_text"\s*:\s*"Hideaway 4 "#}
            }}
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let original: Value = strict_read(include_str!(
        "../../../tests/fixtures/ramos/7cb9e29f-835f-4155-a2a5-4b778866c773.json"
    ))
    .unwrap();
    let source = &original["source"];
    let mut d = each_player_draw::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics = Characteristics {
        kind: CardKind::Land,
        additional_types: vec![],
        subtypes: vec![],
        basic: false,
        legendary: false,
        snow: false,
        mana_cost: ManaCost {
            generic: 0.into(),
            colored: vec![],
        },
        colors: vec![],
        color_identity: vec![Color::G],
        power: None,
        toughness: None,
    };
    d.intrinsic_zone = Some(FunctioningZone::Battlefield);
    d.spell = None;
    d.triggered = vec![entry()];
    d.static_abilities = vec![tapped()];
    d.activated = vec![play()];
    d.mana_abilities = vec![mana()];
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}

#[test]
fn linked_hideaway() {
    let mut d = witness();
    d.validate().unwrap();
    d.activated[0].program.instructions[0] = Instruction::PlayHideawayLinked {
        hideaway_block: 1,
        minimum_total_power: 10.into(),
        source_block: 3,
    };
    assert!(d.validate().is_err());
}

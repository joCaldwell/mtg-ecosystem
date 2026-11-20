//! CR 109.5, 514.2, 601.2i, 611.2f and 702.85a: later hand casts gain cascade.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const TRIGGER_TEXT: &str = "Whenever ~ deals combat damage to a player, as you cast spells from your hand this turn, they gain cascade.";
const REMINDER: &str = " (When you cast the spell, exile cards from the top of your library until you exile a nonland card that costs less. You may cast it without paying its mana cost. Put the exiled cards on the bottom in a random order.)";
const INVALID: &str = "hand-cast cascade requires the complete ordinary combat damage trigger";

pub fn hand_cast_cascade_ability() -> TriggeredAbility {
    TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        },
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::GrantHandCastCascade { source_block: 1 }],
            modes: vec![],
        },
    }
}

pub fn hand_cast_cascade_ast() -> Value {
    json!([
        {"kind":"keywords","keywords":[{"keyword":"trample"}]},
        {"kind":"triggered","trigger":{"trigger":"deals-damage","what":{"ref":"self"},"combat":true,"to":"any"},
            "effects":[{"steps":[{"effect":"grant-cast-keyword","timing":"as-cast",
               "caster":{"player":"you"},"from":{"zone":"hand","owner":"your"},
               "what":{"recipient":"they"},"duration":{"duration":"this-turn"},"keyword":"cascade"}]}]}
    ])
}

pub fn hand_cast_cascade_source(text: &str, name: &str) -> bool {
    let short = name.split(',').next().unwrap_or(name);
    text == format!("Trample\n{}{REMINDER}", TRIGGER_TEXT.replace('~', short))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.abilities_json;
    if !d.capabilities.contains(&Capability::HandCastCascadeGrantV1)
        && !raw.contains("grant-cast-keyword")
        && !d
            .provenance
            .source_json
            .contains("as you cast spells from your hand this turn")
    {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(INVALID)?;
    let c = &d.characteristics;
    let p = d.spell.as_ref().ok_or(INVALID)?;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    if !hand_cast_cascade_source(text, &d.name)
        || strict_read::<Value>(raw)? != hand_cast_cascade_ast()
        || d.triggered != [hand_cast_cascade_ability()]
        || d.keywords != [KeywordAbility::Trample]
        || c.kind != CardKind::Creature
        || c.basic
        || !c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || p.timing != SpellTiming::MainPhase
        || p.resolution != SpellResolution::Permanent
        || p.functioning_zone != FunctioningZone::Stack
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || !p.instructions.is_empty()
        || source["layout"] != "normal"
        || source["type_line"] != format!("Legendary Creature — {}", c.subtypes.join(" "))
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != cost
        || source["power"].as_str() != c.power.as_ref().map(ToString::to_string).as_deref()
        || source["toughness"].as_str() != c.toughness.as_ref().map(ToString::to_string).as_deref()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != ["Trample", TRIGGER_TEXT]
                || labels.triggered != [1]
                || !labels.mana.is_empty()
                || !labels.statics.is_empty()
                || !labels.activated.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let contains = json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":{
        "required":["op"],"properties":{"op":{"const":"grant_hand_cast_cascade"}}
    }}}});
    let absent = json!({"not":contains});
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"modes":{"items":{"properties":{"program":absent}}}}
        }));
    let caps = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        CommanderIdentityV1,
        TriggeredAbilityV1,
        DamageTriggerV1,
        TrampleV1,
        HandCastCascadeGrantV1,
        CascadeV1,
        RandomBottomV1,
        ResolutionCastV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"spell":absent,"activated":{"items":{"properties":{"program":absent}}}},
        "if":{"anyOf":[
            {"required":["triggered"],"properties":{"triggered":{"contains":{"properties":{"program":contains}}}}},
            {"properties":{"capabilities":{"contains":{"const":HandCastCascadeGrantV1}}}},
            {"properties":{"provenance":{"properties":{"abilities_json":{"pattern":r#""effect"\s*:\s*"grant-cast-keyword""#}}}}}
        ]},
        "then":{"required":["spell","triggered"],"properties":{
            "capabilities":{"items":{"enum":caps},"minItems":caps.len(),"maxItems":caps.len(),"uniqueItems":true},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "characteristics":{"properties":{"kind":{"const":"creature"},"legendary":{"const":true},
                "basic":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},"hybrid_mana":{"maxItems":0}}},
            "keywords":{"const":["trample"]},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "static_abilities":{"maxItems":0},
            "spell":{"type":"object","properties":{"resolution":{"const":"permanent"},"timing":{"const":"main_phase"},
                "functioning_zone":{"const":"stack"},"instructions":{"maxItems":0},"targets":{"maxItems":0},
                "modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"}}},
            "triggered":{"const":[hand_cast_cascade_ability()]},
            "source_labels":{"properties":{"blocks":{"const":["Trample",TRIGGER_TEXT]},"triggered":{"const":[1]},
                "activated":{"maxItems":0},"mana":{"maxItems":0},"statics":{"maxItems":0},"spell_modes":{"maxItems":0}}},
            "provenance":{"properties":{
                "source_json":{"pattern":r#""oracle_text"\s*:\s*"Trample\\nWhenever [^"]+ deals combat damage to a player, as you cast spells from your hand this turn, they gain cascade\."#},
                "abilities_json":{"pattern":r#""effect"\s*:\s*"grant-cast-keyword""#}
            }}
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let historical: Value = strict_read(include_str!(
        "../../../tests/fixtures/ramos/9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f.json"
    ))
    .unwrap();
    let source = &historical["source"];
    let mut d = each_player_draw::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics = Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![],
        subtypes: vec!["Ogre".into(), "Wizard".into()],
        basic: false,
        legendary: true,
        snow: false,
        mana_cost: ManaCost {
            generic: 0.into(),
            colored: vec![Color::U, Color::B, Color::R, Color::G],
        },
        hybrid_mana: vec![],
        colors: vec![Color::B, Color::G, Color::R, Color::U],
        color_identity: vec![Color::B, Color::G, Color::R, Color::U],
        power: Some(5.into()),
        toughness: Some(4.into()),
    };
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.resolution = SpellResolution::Permanent;
    p.instructions.clear();
    d.keywords = vec![KeywordAbility::Trample];
    d.triggered = vec![hand_cast_cascade_ability()];
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = hand_cast_cascade_ast().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}

#[test]
fn hand_cast_cascade() {
    let mut d = witness();
    d.validate().unwrap();
    d.triggered[0].program.instructions[0] = Instruction::GrantHandCastCascade { source_block: 0 };
    assert!(d.validate().is_err());
}

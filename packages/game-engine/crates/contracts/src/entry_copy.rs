//! Complete source contract for the optional same-turn battlefield entry copy.
use super::*;
use serde_json::{Value, json};

const COPY_TEXT: &str =
    "You may have this creature enter as a copy of any permanent that entered this turn.";
const TEXT: &str = concat!(
    "Flash\n",
    "Cascade (When you cast this spell, exile cards from the top of your library until you exile a nonland card that costs less. ",
    "You may cast it without paying its mana cost. Put the exiled cards on the bottom in a random order.)\n",
    "You may have this creature enter as a copy of any permanent that entered this turn."
);
const MANIFEST_SHA: &str = "57a8e42b48647e474e0f5d670c250798467f5137efe6667c54d53ec1c2fada06";
const PARSER_SHA: &str = "2338d2861bbcd23301ee84e90cfa23cdcbacaf365d26cca4d12bb76ece0e2ab0";
const SOURCE_SHA: &str = "598aaa96290574803617fdc5dbae1ac8754085a0a37b7dd2d77b7e46a033ae89";
const AST_SHA: &str = "9a612ea960f4fde1be15bcb8f4d8f002d87b1fa4455ecc50fe5731583f079853";
const BINDING_SHA: &str = "cb81af11a8cdfa40429327e2413fbe861b9c15397d94fa0359571a61302e9c30";
const EFFECT: StaticEffect = StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn;
const CAPABILITY: Capability = Capability::EnterAsCopyOfPermanentEnteredThisTurnV1;
const CAPABILITIES: [Capability; 9] = [
    Capability::NormalLayoutV1,
    Capability::BaseCreatureV1,
    Capability::CastCreatureV1,
    Capability::FlashV1,
    Capability::TriggeredAbilityV1,
    Capability::CascadeV1,
    Capability::ResolutionCastV1,
    Capability::RandomBottomV1,
    CAPABILITY,
];

fn abilities() -> Value {
    json!([
        {"kind":"keywords","keywords":[{"keyword":"flash"}]},
        {"kind":"keywords","keywords":[{"keyword":"cascade"}]},
        {"kind":"static","effect":{
            "static":"entry-copy","what":{"ref":"this","noun":"creature"},
            "model":{"model":"choose","who":{"player":"you"},
                "filter":{"cls":"permanent","zoneChangeHistory":{
                    "event":"enters","to":{"zone":"battlefield"},"during":"this-turn"
                }},"count":1,"when":"before-entry"},
            "optional":{"who":{"player":"you"}}
        }}
    ])
}

fn characteristics() -> Characteristics {
    Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![],
        subtypes: vec!["Shapeshifter".into()],
        basic: false,
        legendary: false,
        snow: false,
        hybrid_mana: vec![],
        mana_cost: ManaCost {
            generic: 4.into(),
            colored: vec![Color::U, Color::U],
        },
        colors: vec![Color::U],
        color_identity: vec![Color::U],
        power: Some(3.into()),
        toughness: Some(1.into()),
    }
}

fn entry_static() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: EFFECT,
    }
}

fn cascade_trigger() -> TriggeredAbility {
    TriggeredAbility {
        death_filter: None,
        functioning_zone: FunctioningZone::Stack,
        condition: TriggerCondition::SelfSpellCast,
        intervening_if: None,
        program: canonical_cascade_program(1),
    }
}

fn permanent_spell() -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Permanent,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    }
}

fn labels() -> SourceLabels {
    SourceLabels {
        text: TEXT.into(),
        blocks: vec!["Flash".into(), "Cascade".into(), COPY_TEXT.into()],
        triggered: vec![1],
        statics: vec![2],
        ..SourceLabels::default()
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let p = &d.provenance;
    if !d.static_abilities.iter().any(|a| a.effect == EFFECT)
        && !d.capabilities.contains(&CAPABILITY)
        && p.archive_manifest_sha256 != MANIFEST_SHA
        && !p.source_json.contains(COPY_TEXT)
        && !p.abilities_json.contains("\"entry-copy\"")
    {
        return Ok(());
    }

    let source: Value = strict_read(&p.source_json)?;
    if source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != p.printing_id
        || source["oracle_text"] != TEXT
        || p.source_index != 0
        || p.archive_manifest_sha256 != MANIFEST_SHA
        || p.parser_sha256 != PARSER_SHA
        || p.source_sha256 != SOURCE_SHA
        || p.ast_sha256 != AST_SHA
        || p.binding_sha256 != BINDING_SHA
        || strict_read::<Value>(&p.abilities_json)? != abilities()
        || d.characteristics != characteristics()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || d.spell.as_ref() != Some(&permanent_spell())
        || d.keywords != [KeywordAbility::Flash]
        || d.triggered != [cascade_trigger()]
        || d.static_abilities != [entry_static()]
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || d.source_labels.as_ref().is_some_and(|l| *l != labels())
    {
        return Err(
            "requires the complete frozen flash, cascade, and same-turn entry-copy source".into(),
        );
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let effect = json!({"required":["effect"],"properties":{"effect":{"const":EFFECT}}});
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":effect,
            "then":{"properties":{"functioning_zone":{"const":FunctioningZone::Battlefield}}}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"required":["static_abilities"],"properties":{"static_abilities":{"contains":effect}}},
            {"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":CAPABILITY}}}},
            {"required":["provenance"],"properties":{"provenance":{"properties":{
                "archive_manifest_sha256":{"const":MANIFEST_SHA}
            }}}}
        ]},
        "then":{"required":["spell","characteristics","capabilities","provenance","static_abilities","triggered","keywords"],
            "properties":{
                "spell":{"const":permanent_spell()},
                "characteristics":{"const":characteristics()},
                "static_abilities":{"const":[entry_static()]},
                "triggered":{"const":[cascade_trigger()]},
                "keywords":{"const":[KeywordAbility::Flash]},
                "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
                "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
                "source_labels":{"const":labels()},
                "provenance":{"properties":{
                    "source_index":{"const":0},"archive_manifest_sha256":{"const":MANIFEST_SHA},
                    "parser_sha256":{"const":PARSER_SHA},"source_sha256":{"const":SOURCE_SHA},
                    "ast_sha256":{"const":AST_SHA},"binding_sha256":{"const":BINDING_SHA}
                }},
                "capabilities":{"uniqueItems":true,"minItems":CAPABILITIES.len(),
                    "maxItems":CAPABILITIES.len(),"items":{"enum":CAPABILITIES}}
        }}
    }));
}

#[cfg(test)]
mod tests;

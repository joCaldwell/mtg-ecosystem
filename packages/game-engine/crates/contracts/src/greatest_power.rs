//! CR 106.3, 107.1b, 603.5, 605.3b, 608.2h. Power is read at resolution.
use super::*;
use serde_json::{Value, json};

pub fn greatest_power_source(text: &str) -> Option<Color> {
    let (trigger, mana) = text.split_once('\n')?;
    let trigger = trigger
        .strip_prefix("Whenever ")
        .or_else(|| trigger.strip_prefix("When "))?;
    if trigger
        != concat!(
            "another creature enters, its controller may draw a card ",
            "if its power is greater than each other creature's power."
        )
    {
        return None;
    }
    let symbol = mana.strip_prefix('{')?.strip_suffix(concat!(
        "}, {T}: Add X mana in any combination of colors, ",
        "where X is the greatest power among creatures you control."
    ))?;
    match symbol {
        "W" => Some(Color::W),
        "U" => Some(Color::U),
        "B" => Some(Color::B),
        "R" => Some(Color::R),
        "G" => Some(Color::G),
        _ => None,
    }
}
pub fn greatest_power_ast(cost: Color) -> Value {
    let actor = json!({"player":"controller","of":{"ref":"it"}});
    let effect = json!({"steps":[{"effect":"draw","who":actor,"amount":{"amount":"fixed","value":1},"optional":true,"decidedBy":actor}],"condition":{"condition":"strict-attribute-extremum","what":{"ref":"it"},"attribute":"power","extremum":"maximum","among":{"types":["creature"]}},"conditionKind":"if","conditionPosition":"after"});
    let value = json!({"amount":"attribute-extremum","extremum":"maximum","attribute":"power","among":{"types":["creature"],"control":"you"}});
    json!([{"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"filter","filter":{"other":true,"types":["creature"]},"count":{"amount":"fixed","value":1}}},"effects":[effect]},
    {"kind":"activated","costs":[{"cost":"mana","symbols":[cost]},{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"combination","amount":{"amount":"x"},"options":"colors"}}],"bindings":[{"variable":"x","value":value}]}]}])
}
pub fn greatest_power_programs(color: Color) -> (SpellProgram, TriggeredAbility, ManaAbility) {
    let spell = SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        additional_sacrifice: None,
        optional_additional_costs: vec![],
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    let trigger = TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::OtherCreatureEntersBattlefield,
        death_filter: None,
        intervening_if: None,
        program: SpellProgram {
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            instructions: vec![Instruction::OptionalDrawForSoleGreatestEntrant { source_block: 0 }],
            ..spell.clone()
        },
    };
    let mana = ManaAbility {
        source_block: 1,
        costs: PermanentActivationCosts {
            tap: true,
            mana: Some(ManaCost {
                generic: Quantity::zero(),
                colored: vec![color],
            }),
            sacrifice: false,
            remove_source_plus_one_counters: None,
        },
        self_damage: None,
        restriction: None,
        production: ManaProduction::GreatestControlledPowerCombination,
    };
    (spell, trigger, mana)
}
pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let capability = d.requirements().contains(&Capability::GreatestPowerV1);
    let raw = &d.provenance.source_json;
    if !capability && !raw.contains("each other creature") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let Some(color) = greatest_power_source(text) else {
        return if capability {
            Err("unbound greatest power program".into())
        } else {
            Ok(())
        };
    };
    let (spell, trigger, mana) = greatest_power_programs(color);
    let c = &d.characteristics;
    let mut typ = String::from(if c.legendary {
        "Legendary Creature"
    } else {
        "Creature"
    });
    if !c.subtypes.is_empty() {
        typ.push_str(" — ");
        typ.push_str(&c.subtypes.join(" "));
    }
    if !capability
        || c.kind != CardKind::Creature
        || c.basic
        || !c.hybrid_mana.is_empty()
        || c.snow
        || !c.additional_types.is_empty()
        || c.power.is_none()
        || c.toughness.is_none()
        || d.name != source["name"]
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["layout"] != "normal"
        || source["type_line"] != typ
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || c.power.as_ref().map(ToString::to_string).as_deref() != source["power"].as_str()
        || c.toughness.as_ref().map(ToString::to_string).as_deref() != source["toughness"].as_str()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || ["card_faces", "defense", "loyalty"]
            .iter()
            .any(|k| !source[*k].is_null())
        || !c.color_identity.contains(&color)
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.activated.is_empty()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || d.spell.as_ref() != Some(&spell)
        || d.triggered != [trigger]
        || d.mana_abilities != [mana]
        || strict_read::<Value>(&d.provenance.abilities_json)? != greatest_power_ast(color)
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != text.lines().map(str::to_owned).collect::<Vec<_>>()
                || l.triggered != [0]
                || l.mana != [1]
                || !l.activated.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("incomplete greatest power creature family".into());
    }
    Ok(())
}
pub(super) fn extend_schema(schema: &mut Value) {
    let then: Vec<_>=MANA_COMBINATION_COLORS.into_iter().map(|color|{
        let (spell,trigger,mana)=greatest_power_programs(color);
        json!({"required":["spell","triggered","mana_abilities","capabilities"],"properties":{"spell":{"const":spell},"triggered":{"const":[trigger]},"mana_abilities":{"const":[mana]},"capabilities":{"contains":{"const":Capability::GreatestPowerV1}},"characteristics":{"properties":{"kind":{"const":"creature"}}},"activated":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0},"x_mana_cost":{"const":false}}})
    }).collect();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"anyOf":[{"properties":{"capabilities":{"contains":{"const":Capability::GreatestPowerV1}}}},{"properties":{"provenance":{"properties":{"source_json":{"pattern":"its power is greater than each other creature's power"}}}}},{"required":["mana_abilities"],"properties":{"mana_abilities":{"contains":{"properties":{"production":{"const":ManaProduction::GreatestControlledPowerCombination}}}}}},{"required":["triggered"],"properties":{"triggered":{"contains":{"properties":{"program":{"properties":{"instructions":{"contains":{"properties":{"op":{"const":"optional_draw_for_sole_greatest_entrant"}}}}}}}}}}}]},"then":{"anyOf":then}}));
}
#[cfg(test)]
pub(super) fn witness() -> Definition {
    let r: Value = strict_read(include_str!(
        "../../../tests/fixtures/synthetic-copperleaf-surveyor.json"
    ))
    .unwrap();
    let raw = &r["source"];
    let color = Color::U;
    let mut d = power_entry_draw::witness();
    d.name = raw["name"].as_str().unwrap().into();
    d.oracle_id = raw["oracle_id"].as_str().unwrap().into();
    d.characteristics = Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![],
        subtypes: vec!["Scout".into()],
        basic: false,
        legendary: false,
        snow: false,
        hybrid_mana: vec![],
        mana_cost: ManaCost {
            generic: 1.into(),
            colored: vec![color],
        },
        colors: vec![color],
        color_identity: vec![color],
        power: Some(1.into()),
        toughness: Some(3.into()),
    };
    let (spell, trigger, mana) = greatest_power_programs(color);
    d.spell = Some(spell);
    d.triggered = vec![trigger];
    d.mana_abilities = vec![mana];
    d.activated.clear();
    d.static_abilities.clear();
    d.keywords.clear();
    d.source_labels = None;
    d.provenance.printing_id = raw["id"].as_str().unwrap().into();
    d.provenance.source_json = canonical(raw).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&greatest_power_ast(color)).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = r["bindingSha256"].as_str().unwrap().into();
    d.capabilities = d.requirements();
    d
}

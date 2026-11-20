//! CR 603.2, 608.2g and 118.9. Source, selection and payment have distinct bindings.
use super::*;
use serde_json::{Value, json};

pub fn combat_hand_cast_trigger() -> TriggeredAbility {
    TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        death_filter: None,
        intervening_if: None,
        condition: TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        },
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            targets: vec![],
            modes: vec![],
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            instructions: vec![Instruction::OfferHandCastWithoutManaCost { source_block: 1 }],
        },
    }
}

pub fn combat_hand_cast_ast() -> Value {
    json!([{"kind":"keywords","keywords":[{"keyword":"flying"}]},
        {"kind":"triggered","trigger":{"trigger":"deals-damage","what":{"ref":"this","noun":"creature"},"combat":true,"to":"any"},
        "effects":[{"steps":[{"effect":"play-card","instruction":{"who":{"player":"you"},"action":"cast",
        "what":{"ref":"filter","filter":{"cls":"spell"},"count":{"amount":"fixed","value":1}},
        "from":{"zone":"hand","owner":"your"},"alternativeCost":{"cost":"without-mana-cost"}},"optional":true}]}]}])
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    let phrase = "cast a spell from your hand";
    let reserved = d.capabilities.contains(&Capability::CombatHandCastV1);
    if !reserved && !raw.contains(phrase) && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    if !reserved
        && !source["oracle_text"]
            .as_str()
            .is_some_and(|text| text.contains(phrase) && text.contains("combat damage to a player"))
    {
        return Ok(());
    }
    let c = &d.characteristics;
    let head = "Whenever this creature deals combat damage to a player";
    let action = "you may cast a spell from your hand";
    let tail = "without paying its mana cost.";
    let text = format!("Flying\n{head}, {action} {tail}");
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    let type_line = if c.subtypes.is_empty() {
        "Creature".to_owned()
    } else {
        format!("Creature — {}", c.subtypes.join(" "))
    };
    let labels_ok = d.source_labels.as_ref().is_none_or(|l| {
        l.text == text
            && l.triggered == [1]
            && l.activated.is_empty()
            && l.statics.is_empty()
            && l.mana.is_empty()
            && l.spell_modes.is_empty()
            && l.blocks.len() == 2
            && l.blocks[0] == "Flying"
            && l.blocks[1] == format!("{head}, {action} {tail}")
    });
    if !reserved
        || !labels_ok
        || c.kind != CardKind::Creature
        || !c.additional_types.is_empty()
        || c.legendary
        || c.snow
        || c.basic
        || d.x_mana_cost
        || d.keywords != [KeywordAbility::Flying]
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || d.triggered != [combat_hand_cast_trigger()]
        || d.spell.as_ref().is_none_or(|p| {
            !p.instructions.is_empty()
                || !p.targets.is_empty()
                || !p.modes.is_empty()
                || !p.optional_additional_costs.is_empty()
                || p.additional_sacrifice.is_some()
        })
        || source["id"] != d.provenance.printing_id
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["layout"] != "normal"
        || ["card_faces", "loyalty", "defense"]
            .iter()
            .any(|key| source.get(*key).is_some())
        || source["oracle_text"] != text
        || source["type_line"] != type_line
        || source["mana_cost"] != cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["keywords"] != json!(["Flying"])
        || source.get("power").cloned() != c.power.as_ref().map(|v| json!(v))
        || source.get("toughness").cloned() != c.toughness.as_ref().map(|v| json!(v))
        || strict_read::<Value>(&d.provenance.abilities_json)? != combat_hand_cast_ast()
    {
        return Err("invalid complete combat hand cast source".into());
    }
    Ok(())
}

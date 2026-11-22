//! CR 101.4, 603.2 and 701.9. Mandatory group discard has no targets.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

impl<'de> Deserialize<'de> for GroupDiscardScope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "each_player" => Ok(Self::EachPlayer),
            "each_opponent" => Ok(Self::EachOpponent),
            _ => Err(serde::de::Error::custom("unsupported group discard scope")),
        }
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    // Definition::validate independently requires exact recursive capability closure.
    if !d.capabilities.contains(&Capability::GroupHandDiscardV1) {
        return Ok(());
    }
    let err =
        || "group discard requires one ordinary spell or self-entry creature trigger".to_string();
    let c = &d.characteristics;
    let cast = d.spell.as_ref().ok_or_else(err)?;
    if c.basic
        || c.legendary
        || c.snow
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || cast.additional_sacrifice.is_some()
        || !cast.optional_additional_costs.is_empty()
        || !cast.modes.is_empty()
        || !cast.targets.is_empty()
    {
        return Err(err());
    }
    // The ordinary cast and trigger validators check kind-specific timing, zones,
    // resolution, and the empty creature cast program.
    let p = match (c.kind, d.triggered.as_slice()) {
        (CardKind::Instant | CardKind::Sorcery, [])
            if c.subtypes.is_empty() && c.additional_types.is_empty() =>
        {
            cast
        }
        (CardKind::Creature, [t])
            if matches!(c.additional_types.as_slice(), [] | [CardKind::Artifact])
                && c.subtypes.iter().all(|s| {
                    !unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES.contains(&s.as_str())
                })
                && t.condition == TriggerCondition::SelfEntersBattlefield
                && t.death_filter.is_none()
                && t.intervening_if.is_none() =>
        {
            &t.program
        }
        _ => return Err(err()),
    };
    if !p.targets.is_empty()
        || !p.optional_additional_costs.is_empty()
        || !matches!(p.instructions.as_slice(),
        [kernel!(Discard { player: kir::PlayerExpr::EachPlayer | kir::PlayerExpr::EachOpponent, amount: fixed!(count), random: false } @ 0)] if *count > Quantity::zero())
    {
        return Err(err());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let op = instruction_schema::group_discard();
    let has = json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":op}}});
    let singleton = |item: Value| json!({"minItems":1,"maxItems":1,"items":item});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"modes":{"items":{"properties":{"program":{"not":has}}}}},
        "if":has,"then":{"properties":{
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "instructions":singleton(op),"targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0}
        }}
    }));
    let ability = json!({"required":["program"],"properties":{"program":has}});
    let caps = |extra: &[Capability]| {
        let mut allowed = vec![NormalLayoutV1, GroupHandDiscardV1];
        allowed.extend_from_slice(extra);
        json!({"minItems":allowed.len(),"maxItems":allowed.len(),"uniqueItems":true,"items":{"enum":allowed}})
    };
    let spell = |kind, timing, cast| {
        json!({"properties":{
            "characteristics":{"properties":{"kind":{"const":kind},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0},
                "power":{"type":"null"},"toughness":{"type":"null"}}},
            "spell":{"allOf":[has],"properties":{"timing":{"const":timing}}},"triggered":{"maxItems":0},"capabilities":caps(&[cast])
        }})
    };
    let subtypes = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect::<Vec<_>>();
    let creature_caps = [
        BaseCreatureV1,
        CastCreatureV1,
        TriggeredAbilityV1,
        SelfEntersBattlefieldV1,
    ];
    let mut artifact_caps = creature_caps.to_vec();
    artifact_caps.push(CastArtifactV1);
    let creature = json!({"required":["triggered"],"properties":{
        "characteristics":{"required":["power","toughness"],"properties":{"kind":{"const":"creature"},
            "power":{"type":"string"},"toughness":{"type":"string","pattern":"^[1-9][0-9]*$"},
            "additional_types":{"enum":[[],["artifact"]]},"subtypes":{"items":{"not":{"enum":subtypes}}}}},
        "triggered":singleton(json!({"allOf":[ability],"properties":{
            "condition":{"const":"self_enters_battlefield"},"functioning_zone":{"const":"battlefield"},
            "death_filter":{"type":"null"},"intervening_if":{"type":"null"},"program":{"properties":{"timing":{"const":"instant"}}}
        }})),
        "spell":{"type":"object","properties":{"functioning_zone":{"const":"stack"},"timing":{"const":"main_phase"},
            "resolution":{"const":"permanent"},"instructions":{"maxItems":0},"targets":{"maxItems":0},
            "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}}
    },"if":{"properties":{"characteristics":{"required":["additional_types"],"properties":{"additional_types":{"const":["artifact"]}}}}},
    "then":{"properties":{"capabilities":caps(&artifact_caps)}},"else":{"properties":{"capabilities":caps(&creature_caps)}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"activated":{"items":{"not":ability}}},
        "if":{"anyOf":[{"properties":{"capabilities":{"contains":{"const":GroupHandDiscardV1}}}},
            {"required":["spell"],"properties":{"spell":has}},{"required":["triggered"],"properties":{"triggered":{"contains":ability}}}]},
        "then":{"required":["spell"],"properties":{
            "characteristics":{"properties":{"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false}}},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"activated":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},"static_abilities":{"maxItems":0}
        },"oneOf":[spell(CardKind::Instant,SpellTiming::Instant,CastInstantV1),
            spell(CardKind::Sorcery,SpellTiming::MainPhase,CastSorceryV1),creature]}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = controller_library::witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::discard(
        kir::PlayerExpr::EachPlayer,
        kir::Amount::Fixed { value: 3.into() },
        false,
        0,
    )];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn group_discard() {
    let spell = witness();
    spell.validate().unwrap();
    assert!(spell.capabilities.contains(&Capability::GroupHandDiscardV1));
    let op = Instruction::discard(
        kir::PlayerExpr::EachOpponent,
        kir::Amount::Fixed { value: 1.into() },
        false,
        0,
    );
    let mut creature = creature_entry_return::witness();
    creature.triggered[0].program.instructions = vec![op.clone()];
    creature.triggered[0].program.targets.clear();
    for additional in [vec![], vec![CardKind::Artifact]] {
        creature.characteristics.additional_types = additional;
        creature.capabilities = creature.requirements();
        creature.validate().unwrap();
    }
    for mutation in 0..11 {
        let mut d = creature.clone();
        let t = &mut d.triggered[0];
        match mutation {
            0 => t.condition = TriggerCondition::SelfDies,
            1 => t.intervening_if = Some(InterveningCondition::ControllerHandEmpty),
            2 => t.program.instructions.push(op.clone()),
            3 => d.keywords.push(KeywordAbility::Flying),
            4 => d.characteristics.legendary = true,
            5 => d.characteristics.snow = true,
            6 => d
                .characteristics
                .additional_types
                .push(CardKind::Enchantment),
            7 => d.triggered.push(d.triggered[0].clone()),
            8 => {
                d.spell.as_mut().unwrap().additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Creature],
                    subtypes: vec![],
                })
            }
            9 => t.program.modes.push(SpellMode {
                program: spell.spell.clone().unwrap(),
            }),
            _ => t.program.targets = targeted_discard::witness().spell.unwrap().targets,
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "creature mutation {mutation}");
    }
    for (count, source_block) in [(0, 0), (-1, 0), (1, 1)] {
        let mut d = spell.clone();
        d.spell.as_mut().unwrap().instructions = vec![Instruction::discard(
            kir::PlayerExpr::EachPlayer,
            kir::Amount::Fixed {
                value: count.into(),
            },
            false,
            source_block,
        )];
        assert!(d.validate().is_err());
    }
    let mut wrong = spell;
    wrong.capabilities.remove(&Capability::GroupHandDiscardV1);
    assert!(wrong.validate().is_err());
    let mut wire = serde_json::to_value(&op).unwrap();
    assert_eq!(
        serde_json::from_value::<Instruction>(wire.clone()).unwrap(),
        op
    );
    for scope in [json!("target_player"), json!({"each_opponent":null})] {
        wire["scope"] = scope;
        assert!(serde_json::from_value::<Instruction>(wire.clone()).is_err());
    }
    wire["scope"] = json!("each_opponent");
    wire["random"] = json!(false);
    assert!(serde_json::from_value::<Instruction>(wire).is_err());
}

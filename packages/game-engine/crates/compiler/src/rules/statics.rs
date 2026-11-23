//! Static abilities and prevention shields (CR 604, 615).
use super::*;
pub(crate) fn lower_static(effect: &SourceStaticEffect) -> Result<StaticAbility, Failure> {
    // CR 614.1c/614.12: only unconditional entry of this source is supported.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if let SourceStaticEffect::EntersTapped {
        what,
        condition_kind,
        condition,
    } = effect
    {
        if condition_kind.is_some() || condition.is_some() {
            return Err(unsupported("static", "static:enters-tapped:conditional"));
        }
        if ![
            "permanent",
            "creature",
            "artifact",
            "enchantment",
            "aura",
            "land",
        ]
        .iter()
        .any(|noun| *what == serde_json::json!({"ref":"this","noun":noun}))
        {
            return Err(unsupported(
                "static",
                "static:enters-tapped:unsupported_source",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTapped,
        });
    }
    let special = match effect {
        SourceStaticEffect::Cant { what, action }
            if *what == serde_json::json!({"ref":"this","noun":"spell"})
                && action == "be-countered" =>
        {
            Some(StaticEffect::CantBeCountered)
        }
        _ => None,
    };
    if let Some(effect) = special {
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Stack,
            effect,
        });
    }
    if matches!(effect, SourceStaticEffect::Cant { what, action } if *what == serde_json::json!({"ref":"this","noun":"creature"}) && action == "block")
    {
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::Source,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![],
                restrictions: vec![CreatureRestriction::CantBlock],
            },
        });
    }
    // CR 611.3a/613.1f: a live battlefield source continuously grants the
    // keyword to its current attacking creatures. Consume the entire AST shape;
    // neither status/ownership qualifiers nor mixed modifiers are discarded.
    if let SourceStaticEffect::Modify { what, modifiers } = effect
        && *what
            == serde_json::json!({"ref":"each","filter":{"status":["attacking"],"types":["creature"],"control":"you"}})
    {
        if modifiers.len() != 1
            || modifiers[0]
                != serde_json::json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"deathtouch"}}]})
        {
            return Err(unsupported(
                "static.modifier",
                "attacking creature grant requires exactly deathtouch",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::AttackingController,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![KeywordAbility::Deathtouch],
                restrictions: vec![],
            },
        });
    }
    let scope = |what: &serde_json::Value| {
        if *what == serde_json::json!({"ref":"equipped","noun":"creature"}) {
            Some(CreatureScope::Equipped)
        } else if *what == serde_json::json!({"ref":"enchanted","noun":"creature"}) {
            Some(CreatureScope::Enchanted)
        } else if *what
            == serde_json::json!({"ref":"each","filter":{"types":["creature"],"control":"you"}})
        {
            Some(CreatureScope::Controller)
        } else {
            None
        }
    };
    if let SourceStaticEffect::Cant { what, action } = effect {
        if scope(what) != Some(CreatureScope::Enchanted) || action != "attack-or-block" {
            return Err(unsupported("static.cant", "unsupported prohibition"));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::Enchanted,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![],
                restrictions: vec![CreatureRestriction::CantAttackOrBlock],
            },
        });
    }
    if let SourceStaticEffect::Modify { what, modifiers } = effect
        && (scope(what) != Some(CreatureScope::Controller)
            || modifiers.len() != 1
            || modifiers[0]["modifier"] != "pt"
            || modifiers[0]["power"] != modifiers[0]["toughness"]
            || modifiers[0]["power"]["amount"]["value"] == 0)
    {
        let scope =
            scope(what).ok_or_else(|| unsupported("static.scope", "unsupported modifier scope"))?;
        let mut power = Quantity::zero();
        let mut toughness = Quantity::zero();
        let mut keywords = vec![];
        let mut restrictions = vec![];
        let mut pt = false;
        for modifier in modifiers {
            #[derive(Deserialize)]
            #[serde(tag = "modifier", rename_all = "kebab-case", deny_unknown_fields)]
            enum Modifier {
                Pt {
                    power: SignedAmount,
                    toughness: SignedAmount,
                },
                Abilities {
                    abilities: Vec<serde_json::Value>,
                },
                Cant {
                    action: String,
                },
            }
            match serde_json::from_value(modifier.clone())
                .map_err(|e| unsupported("static.modifier", format!("{e}")))?
            {
                Modifier::Pt {
                    power: p,
                    toughness: t,
                } => {
                    if pt {
                        return Err(unsupported("static.modifier", "duplicate pt"));
                    }
                    pt = true;
                    power = signed_fixed(&p)?;
                    toughness = signed_fixed(&t)?;
                    if power.is_negative() || toughness.is_negative() {
                        return Err(unsupported("static.modifier", "negative static pt"));
                    }
                }
                Modifier::Abilities { abilities } => {
                    if abilities.is_empty() {
                        return Err(unsupported("static.modifier", "empty grant"));
                    }
                    for a in abilities {
                        if a.as_object().is_none_or(|o| o.len() != 2)
                            || a["gained"] != "keyword"
                            || a["keyword"].as_object().is_none_or(|o| o.len() != 1)
                        {
                            return Err(unsupported("static.modifier", "unsupported grant"));
                        }
                        let k: KeywordAbility =
                            serde_json::from_value(a["keyword"]["keyword"].clone())
                                .map_err(|e| unsupported("static.keyword", e.to_string()))?;
                        if keywords.contains(&k) {
                            return Err(unsupported("static.keyword", "duplicate grant"));
                        }
                        keywords.push(k);
                    }
                }
                Modifier::Cant { action } => {
                    let restriction = match action.as_str() {
                        "be-blocked" => CreatureRestriction::CantBeBlocked,
                        "attack-or-block" => CreatureRestriction::CantAttackOrBlock,
                        _ => return Err(unsupported("static.cant", "unsupported restriction")),
                    };
                    if restrictions.contains(&restriction) {
                        return Err(unsupported("static.cant", "duplicate restriction"));
                    }
                    restrictions.push(restriction);
                }
            }
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope,
                power,
                toughness,
                keywords,
                restrictions,
            },
        });
    }
    if let SourceStaticEffect::Modify { what, modifiers } = effect {
        if *what
            != serde_json::json!({"ref":"each","filter":{"types":["creature"],"control":"you"}})
            || modifiers.len() != 1
        {
            return Err(unsupported("abilities.static", "unsupported anthem scope"));
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Pt {
            modifier: String,
            power: SignedAmount,
            toughness: SignedAmount,
        }
        let m: Pt = serde_json::from_value(modifiers[0].clone())
            .map_err(|_| unsupported("abilities.static", "unsupported anthem modifier"))?;
        let power = signed_fixed(&m.power)?;
        let toughness = signed_fixed(&m.toughness)?;
        if m.modifier != "pt" || power <= Quantity::zero() || power != toughness {
            return Err(unsupported(
                "abilities.static",
                "anthem must give fixed +N/+N",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerCreatureAnthem { power, toughness },
        });
    }
    Err(unsupported("static", "unsupported complete static source"))
}

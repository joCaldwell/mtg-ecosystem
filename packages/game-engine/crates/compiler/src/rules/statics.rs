//! Static abilities and prevention shields (CR 604, 615).
use super::*;

pub(crate) fn lower_prevention(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    prevention: &SourcePrevention,
    duration: &SourceDuration,
    source_block: u32,
) -> Result<Instruction, Failure> {
    // CR 615.7: the finite shield budget is shared across simultaneous sources.
    // The IR retains its target and duration; allocation is a runtime choice.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if duration.duration != "this-turn"
        || prevention.event_scope.sources != "simultaneous"
        || prevention.event_scope.recipients != "simultaneous"
    {
        return Err(unsupported(
            "effects.prevention",
            "unsupported event scope or duration",
        ));
    }
    match &prevention.shield {
        SourceShield::Events {
            occurrence,
            prevent: SourcePrevent::All {},
        } if occurrence == "each" && prevention.kind == "combat" && prevention.to.is_none() => {
            Ok(Instruction::PreventCombatDamage {
                duration: Duration::UntilEndOfTurn,
                source_block,
            })
        }
        SourceShield::DamageBudget { amount } if prevention.kind == "all" => {
            let Some(targets) = &prevention.to else {
                return Err(unsupported(
                    "effects.prevention.to",
                    "missing complete any-target scope",
                ));
            };
            let [AnyTargetReference::AnyTarget { selection_id }] = targets.as_slice() else {
                return Err(unsupported(
                    "effects.prevention.to",
                    "unsupported recipient scope",
                ));
            };
            Ok(Instruction::PreventNextDamage {
                target_slot: bind_target(
                    program,
                    slots,
                    selection_id,
                    TargetDomain::AnyDamageTarget,
                )?,
                amount: fixed_positive(amount)?,
                duration: Duration::UntilEndOfTurn,
                source_block,
            })
        }
        _ => Err(unsupported(
            "effects.prevention",
            "unsupported complete prevention family",
        )),
    }
}

pub(crate) fn lower_static(
    effect: &SourceStaticEffect,
    source_block: u32,
) -> Result<StaticAbility, Failure> {
    if matches!(effect, SourceStaticEffect::CombatRule { .. }) {
        return Err(unsupported(
            "static.combat_rule",
            "unsupported complete combat rule",
        ));
    }
    if matches!(effect, SourceStaticEffect::EntryCopy { .. }) {
        // The complete graph is checked before lowering and the complete
        // printed source and raw AST before returning a definition.
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn,
        });
    }
    if matches!(effect, SourceStaticEffect::AsCascade { .. }) {
        // The complete source and occurrence graph were checked before lowering.
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::CascadeLandEntry,
        });
    }
    if let Some(entry) = conditional_lands::lower(effect)? {
        return Ok(entry);
    }
    if let Some(grant) = spell_cascade_grants::lower(effect, source_block)? {
        return Ok(grant);
    }
    if let SourceStaticEffect::PlayerRule { rule } = effect {
        return unlimited_hand_size::lower_rule(rule);
    }
    if let SourceStaticEffect::OpeningHandAction { what, who, action } = effect
        && matches!(
            what.get("ref").and_then(|s| s.as_str()),
            Some("self" | "this")
        )
        && *who == serde_json::json!({"player":"you"})
        && action == "begin-on-battlefield"
    {
        return Ok(opening_placement_ability());
    }
    if let Some(grant) = controller_land_types::lower(effect) {
        return Ok(grant);
    }
    if let Some(grant) = static_permanent_indestructible::lower(effect) {
        return Ok(grant);
    }
    if let SourceStaticEffect::Control { who, what } = effect {
        if who.player != "you" || what.reference != "enchanted" {
            return Err(unsupported(
                "static.control",
                "unsupported Aura control operands",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControlEnchantedPermanent,
        });
    }
    if let SourceStaticEffect::Prohibit { prohibition } = effect {
        // CR 101.2/109.5/601.3/611.3b: the current battlefield controller
        // defines opponents and whose turn. Consume the whole action scope;
        // archived unsupported-scope diagnostics are not executable authority.
        if *prohibition
            != serde_json::json!({"prohibition":"cast","who":{"player":"each-opponent"},
            "what":{"ref":"each","filter":{"cls":"spell"}},"when":{"during":"turn","whose":{"player":"you"}}})
        {
            return Err(unsupported(
                "static.prohibition",
                "unsupported complete casting prohibition scope",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::OpponentsCantCastDuringControllerTurn,
        });
    }

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
        SourceStaticEffect::CostModification {
            scope,
            change,
            multiplier,
        } if *scope
            == serde_json::json!({"action":"cast","spells":{"ref":"this","noun":"spell"}})
            && *change == serde_json::json!({"direction":"reduce","symbols":["1"]})
            && *multiplier
                == serde_json::json!({"amount":"count","of":{"types":["creature"],"zone":{"zone":"battlefield"}}}) =>
        {
            Some(StaticEffect::ReduceGenericByCreatureCount)
        }
        SourceStaticEffect::CostModification { .. } => {
            return Err(unsupported("static", "unsupported cost modification"));
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
    let SourceStaticEffect::Replacement { replacement } = effect else {
        return Err(unsupported(
            "abilities.static",
            "unsupported complete static source",
        ));
    };
    let SourceReplacementAction::ModifyQuantity { change } = &replacement.replace;
    if let SourceDamageEvent::PutCounters {
        on,
        counter,
        cause,
        quantity,
    } = &replacement.event
    {
        // CR 614.1a/614.4-5/616.1: bind the affected creature independently of
        // the provider, whose current controller supplies “you”. Archived
        // diagnostic binding flags do not authorize execution.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        let expected = serde_json::json!({"ref":"filter","filter":{"types":["creature"],"control":"you"},"count":{"amount":"fixed","value":1}});
        if on.len() != 1
            || on[0]
                != serde_json::from_value::<CounterPlacementSourceRecipient>(expected)
                    .expect("counter scope")
            || counter.r#type != "+1/+1"
            || cause != "any"
            || quantity.op != "ge"
            || !fixed_one(&quantity.value)
            || replacement.occurrence != "each"
        {
            return Err(unsupported(
                "abilities.static",
                "unsupported complete counter placement scope",
            ));
        }
        let modification = match (change.operation.as_str(), &change.amount, change.factor) {
            ("add", Some(amount), None) => CounterPlacementModification::Add {
                amount: fixed_positive(amount)?,
            },
            ("multiply", None, Some(factor)) if factor > 0 => {
                CounterPlacementModification::Multiply {
                    factor: i64::from(factor).into(),
                }
            }
            _ => {
                return Err(unsupported(
                    "abilities.static",
                    "unsupported counter placement quantity modification",
                ));
            }
        };
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerCreaturePlusOneCounterReplacement { modification },
        });
    }
    let SourceDamageEvent::Damage { kind, source, to } = &replacement.event else {
        unreachable!()
    };
    let permanent = |recipient: &ReplacementRecipient| {
        matches!(recipient,
        ReplacementRecipient::Permanent(FilterObject { reference, filter: ObjectFilter::Class(CardFilter { cls }), count })
        if reference == "filter" && cls == "permanent" && fixed_one(count))
    };
    let player = |recipient: &ReplacementRecipient| {
        matches!(recipient,
        ReplacementRecipient::Player(p) if p.player == "a-player" && p.selection_id.is_none() && p.of.is_none())
    };
    if kind != "all"
        || source.reference != "filter"
        || !fixed_one(&source.count)
        || replacement.occurrence != "each"
        || change.operation != "multiply"
        || change.factor != Some(2)
        || change.amount.is_some()
        || to.len() != 2
        || to.iter().filter(|r| permanent(r)).count() != 1
        || to.iter().filter(|r| player(r)).count() != 1
    {
        return Err(unsupported(
            "abilities.static",
            "unsupported complete damage replacement scope",
        ));
    }
    // Consume the complete source predicate. The old global source remains
    // exactly empty; no controller, property or type qualifier may be dropped.
    let effect = if source.filter == serde_json::json!({}) {
        StaticEffect::DoubleDamageToPermanentOrPlayer
    } else if source.filter == serde_json::json!({"types":["creature"],"control":"you"}) {
        StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
    } else {
        return Err(unsupported(
            "abilities.static",
            "unsupported complete damage replacement source",
        ));
    };
    // CR 611.3b/614.5: battlefield-only static rule, once per damage lineage.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    Ok(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect,
    })
}

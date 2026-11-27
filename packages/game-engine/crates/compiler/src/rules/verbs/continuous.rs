//! CR 611.2a,c, 613.1b,e-f, 613.4b: resolved battlefield effects.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{Animation, CharacteristicSet, Until};

// CR 608.2c: the announced target a singular pronoun names, if it names one.
pub(super) fn followup_target(cx: &StepCx<'_>, what: &ast::ObjectRef) -> Option<u32> {
    if !matches!(what, ast::ObjectRef::It(_) | ast::ObjectRef::That(_)) {
        return None;
    }
    match antecedent::permanent(cx, what) {
        Ok(Ref::Target { slot }) => Some(slot),
        _ => None,
    }
}

pub(super) fn targets_creature(cx: &StepCx<'_>, slot: u32) -> bool {
    let target = &cx.program.targets[slot as usize];
    target.domain == TargetDomain::BattlefieldCreature
        || target
            .filter
            .as_ref()
            .is_some_and(|filter| filter.types == [CardKind::Creature])
}

fn objects(cx: &mut StepCx<'_>, what: &ast::ObjectRef) -> Result<SetExpr, Failure> {
    if is_set(what) {
        return lower_board_set(what, cx.context);
    }
    if is_this_permanent(what) && cx.context != Context::Spell {
        return Ok(SetExpr::source());
    }
    if let Some(slot) = followup_target(cx, what) {
        return Ok(SetExpr::target(slot));
    }
    let slot = if is_creature_target(what) {
        let (_, selection, ..) = target_occurrence(what)?;
        if let Some(&slot) = cx.slots.get(selection) {
            slot
        } else {
            bind_creature_target(cx.program, cx.slots, what)?
        }
    } else {
        bind_permanent_target(cx.program, cx.slots, what)?
    };
    Ok(SetExpr::target(slot))
}

pub(super) fn control(
    cx: &mut StepCx<'_>,
    who: &ast::PlayerRef,
    what: &ast::ObjectRef,
    duration: Option<&serde_json::Value>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let until = match duration {
        None => Until::Indefinite,
        Some(raw) => match typed::<ast::Duration>(raw, "gain_control_duration")? {
            ast::Duration::EndOfTurn(_) => Until::EndOfTurn,
            ast::Duration::AsLongAs(d)
                if cx.context != Context::Spell
                    && matches!(&*d.condition, ast::Condition::Remains(remains)
                    if is_this_permanent(&remains.what)
                        && matches!(&*remains.zone, ast::ZoneRef {
                            zone: ast::Zone::Battlefield, owner: None, depth: None, position: None,
                        })) =>
            {
                Until::SourceLeaves
            }
            d => return Err(step("gain-control", &format!("duration:{}", variant(&d)))),
        },
    };
    let controller = match who {
        ast::PlayerRef::You(_) => PlayerExpr::you(cx.context),
        ast::PlayerRef::TargetOpponent(target) => match &**target {
            ast::PlayerRefTargetOpponent {
                count: None,
                up_to: None,
                random: None,
                selection_id: Some(id),
            } => PlayerExpr::target(bind_target(
                cx.program,
                cx.slots,
                id,
                TargetDomain::OpponentPlayer,
            )?),
            _ => return Err(step("gain-control", "qualified_player")),
        },
        ast::PlayerRef::TargetPlayer(_) => {
            lower_player_ref(cx.program, cx.slots, who.clone(), cx.context, None)?
        }
        other => {
            return Err(step(
                "gain-control",
                &format!("controller:{}", variant(other)),
            ));
        }
    };
    let what = objects(cx, what)?;
    Ok(cx.act(Verb::GainControl {
        what,
        controller,
        until,
    }))
}

pub(super) fn lower_become(
    cx: &mut StepCx<'_>,
    what: &ast::ObjectRef,
    spec: &ast::BecomeSpec,
    duration: &ast::Duration,
) -> Result<Instruction, Failure> {
    let ast::BecomeSpec {
        copy_of: None,
        copy_exceptions: None,
        types,
        subtypes,
        colors,
        power,
        toughness,
        in_addition,
    } = spec
    else {
        return Err(step("become", "characteristic_shape"));
    };
    let artifact = types.as_deref().is_some_and(|types| {
        types.len() == 2
            && types.iter().any(|t| t == "artifact")
            && types.iter().any(|t| t == "creature")
    });
    let animation = match (types.as_deref(), in_addition) {
        (None, None) if subtypes.is_none() => None,
        // CR 205.1b: "artifact creature" retains existing card types.
        (_, None) if artifact => Some(Animation {
            add_artifact: true,
            replace_creature_subtypes: subtypes.is_some(),
            creature_subtypes: subtypes.clone().unwrap_or_default(),
        }),
        (Some([kind]), Some(ast::BecomeSpecInAddition::Types)) if kind == "creature" => {
            Some(Animation {
                add_artifact: false,
                replace_creature_subtypes: false,
                creature_subtypes: subtypes.clone().unwrap_or_default(),
            })
        }
        _ => return Err(step("become", "type_operation")),
    };
    setter(
        cx,
        what,
        CharacteristicSet {
            animation,
            colors: colors
                .as_ref()
                .map(|colors| colors.iter().map(color).collect()),
            base_power: power.as_deref().map(fixed_stat).transpose()?,
            base_toughness: toughness.as_deref().map(fixed_stat).transpose()?,
            keywords: vec![],
        },
        duration,
    )
}

fn fixed_stat(amount: &ast::Amount) -> Result<Quantity, Failure> {
    match amount {
        ast::Amount::Fixed(n) => Ok(n.value.into()),
        other => Err(operand("base_stat", variant(other))),
    }
}

pub(super) fn modify(
    cx: &mut StepCx<'_>,
    what: &ast::ObjectRef,
    modifiers: &[ast::ResolvingCharacteristicModifier],
    stated_as: Option<ast::EffectModifyCharacteristicsStatedAs>,
    duration: &ast::Duration,
) -> Result<Instruction, Failure> {
    let mut values = CharacteristicSet::default();
    for modifier in modifiers {
        use ast::ResolvingCharacteristicModifier as M;
        match modifier {
            M::Types(m) if values.animation.is_none() => {
                values.animation = Some(animation(&m.change)?);
            }
            M::Colors(m) if values.colors.is_none() => {
                let ast::ResolvingCharacteristicModifierColors {
                    operation: ast::TypeChangeChosenSubtypeOperation::Set,
                    colors,
                } = &**m
                else {
                    return Err(step("modify-characteristics", "color_operation"));
                };
                let ast::ResolvingCharacteristicModifierColorsColors::Array(colors) = &**colors
                else {
                    return Err(step("modify-characteristics", "color_choice"));
                };
                values.colors = Some(colors.iter().map(color).collect());
            }
            M::Variant(m) if values.base_power.is_none() && values.base_toughness.is_none() => {
                let ast::ResolvingCharacteristicModifierVariant::Modifier(m) = &**m;
                let ast::ResolvingCharacteristicModifierVariantModifier {
                    modifier: Some(ast::LitBasePt::BasePt),
                    value_scope: None,
                    power,
                    toughness,
                } = &**m
                else {
                    return Err(step("modify-characteristics", "base_stat_shape"));
                };
                if power.is_none() && toughness.is_none() {
                    return Err(step("modify-characteristics", "missing_base_stats"));
                }
                values.base_power = power.as_deref().map(fixed_stat).transpose()?;
                values.base_toughness = toughness.as_deref().map(fixed_stat).transpose()?;
            }
            M::Abilities(m) if values.keywords.is_empty() && !m.abilities.is_empty() => {
                values.keywords = m
                    .abilities
                    .iter()
                    .map(gain_abilities::keyword)
                    .collect::<Result<_, _>>()?;
            }
            other => return Err(step("modify-characteristics", &variant(other))),
        }
    }
    // CR 611.2e: admitted objects already exist on the battlefield. "Is"
    // or "has" tied to an entering step's result needs a different route.
    let _ = stated_as;
    setter(cx, what, values, duration)
}

fn animation(change: &ast::TypeChange) -> Result<Animation, Failure> {
    use ast::TypeChangeCardTypesOperation as Op;
    let ast::TypeChange {
        card_types: Some(card_types),
        subtypes,
        chosen_subtype: None,
        supertypes: None,
    } = change
    else {
        return Err(step("modify-characteristics", "type_operation"));
    };
    let ast::TypeChangeCardTypes {
        operation: Op::Add,
        types,
    } = &**card_types
    else {
        return Err(step("modify-characteristics", "card_type_operation"));
    };
    let add_artifact = types.iter().any(|kind| kind == "artifact");
    if types.len() != 1 + usize::from(add_artifact) || !types.iter().any(|kind| kind == "creature")
    {
        return Err(step("modify-characteristics", "animation_card_types"));
    }
    let (replace_creature_subtypes, creature_subtypes) = match subtypes.as_deref() {
        None => (false, vec![]),
        Some(
            [
                ast::TypeChangeSubtypesItem {
                    family: ast::SubtypeFamily::Creature,
                    operation: operation @ (Op::Add | Op::Set),
                    types,
                },
            ],
        ) => match &**types {
            ast::TypeChangeSubtypesItemTypes::Array(types) if !types.is_empty() => {
                (*operation == Op::Set, types.clone())
            }
            _ => return Err(step("modify-characteristics", "animation_subtypes")),
        },
        _ => return Err(step("modify-characteristics", "type_family")),
    };
    Ok(Animation {
        add_artifact,
        replace_creature_subtypes,
        creature_subtypes,
    })
}

fn setter(
    cx: &mut StepCx<'_>,
    what: &ast::ObjectRef,
    values: CharacteristicSet,
    duration: &ast::Duration,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if !matches!(duration, ast::Duration::EndOfTurn(_)) {
        return Err(operand("characteristic_duration", variant(duration)));
    }
    let what = objects(cx, what)?;
    Ok(cx.act(Verb::SetCharacteristics {
        what,
        values,
        until: Until::EndOfTurn,
    }))
}

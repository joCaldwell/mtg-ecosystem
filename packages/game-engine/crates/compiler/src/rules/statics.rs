//! Static source lowering (CR 604, 611.3, 613, 118.7 and 601.2f).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{
    Allowance, Change, CombatRequirement, Cond, EntryCondition, HandSizeChange, Scope,
    SpellCostChange, StaticRule, ZoneExpr,
};

fn gap(code: &str) -> Failure {
    unsupported("static", format!("static:{code}"))
}

pub(crate) fn lower_static(value: &serde_json::Value) -> Result<Vec<StaticAbility>, Failure> {
    let effect = typed::<ast::StaticEffect>(value, "static")
        .map_err(|error| gap(&format!("ast:{}", error.reason)))?;
    lower(&effect)
}

fn ability(functioning_zone: FunctioningZone, effect: StaticEffect) -> StaticAbility {
    StaticAbility {
        functioning_zone,
        effect,
    }
}

fn rule(
    functioning_zone: FunctioningZone,
    rule: StaticRule,
    condition: Option<Cond>,
) -> StaticAbility {
    ability(functioning_zone, StaticEffect::Rule { rule, condition })
}

fn lower(effect: &ast::StaticEffect) -> Result<Vec<StaticAbility>, Failure> {
    use FunctioningZone::{Battlefield, Stack};
    use ast::StaticEffect as S;
    let one = match effect {
        S::EntersTapped(x) => {
            if !matches!(&*x.what, ast::ObjectRef::SelfObject(_))
                && !matches!(&*x.what, ast::ObjectRef::This(x) if matches!(x.noun.as_str(),
                    "permanent" | "creature" | "artifact" | "enchantment" | "aura" | "land"))
            {
                return Err(gap("enters-tapped:source"));
            }
            let effect = match (x.condition.as_deref(), x.condition_kind) {
                (None, None) => StaticEffect::EntersTapped,
                (Some(condition), Some(ast::SentenceConditionKind::Unless)) => {
                    StaticEffect::EntersTappedIf {
                        condition: entry_condition(condition)?,
                    }
                }
                _ => return Err(gap("enters-tapped:conditional")),
            };
            ability(Battlefield, effect)
        }
        S::Modify(x) => return modify(x),
        S::Conditional(x) => {
            if x.otherwise.is_some() {
                return Err(gap("conditional:otherwise"));
            }
            let guard = condition(&x.condition)?;
            let mut abilities = lower(&x.effect)?;
            for a in &mut abilities {
                let condition = match &mut a.effect {
                    StaticEffect::Continuous { condition, .. }
                    | StaticEffect::Rule { condition, .. } => condition,
                    StaticEffect::EntersTappedIf { .. } => {
                        return Err(gap("enters-tapped:nested-conditional"));
                    }
                    StaticEffect::EntersTapped => return Err(gap("enters-tapped:conditional")),
                    StaticEffect::PayLifeOrEnterTapped { .. } => {
                        return Err(gap("entry-choice:conditional"));
                    }
                    StaticEffect::EntersWithPlusOneCounters { .. } => {
                        return Err(gap("enters-with-counters:conditional"));
                    }
                };
                *condition = and(condition.take(), Some(guard.clone()));
            }
            return Ok(abilities);
        }
        S::Group(x) if !x.effects.is_empty() => {
            let mut abilities = Vec::new();
            for effect in &x.effects {
                abilities.extend(lower(effect)?);
            }
            return Ok(abilities);
        }
        S::Cant(x) => {
            let condition = qualified_condition(x.condition.as_deref(), x.condition_kind)?;
            if *x.action == ast::CantAction::BeCountered {
                let affects = scope(&x.what, true)?;
                let zone = if affects == Scope::Source {
                    Stack
                } else {
                    Battlefield
                };
                rule(zone, StaticRule::CantBeCountered { affects }, condition)
            } else {
                rule(
                    Battlefield,
                    StaticRule::Combat {
                        affects: scope(&x.what, false)?,
                        requirement: prohibition(&x.action)?,
                    },
                    condition,
                )
            }
        }
        S::MustAttack(x) => rule(
            Battlefield,
            StaticRule::Combat {
                affects: scope(&x.what, false)?,
                requirement: CombatRequirement::AttackEachCombat,
            },
            None,
        ),
        S::CombatRule(x) => combat_rule(&x.what, &x.rule)?,
        S::AdditionalLandPlays(x) => {
            let allowance = match &*x.allowance.additional {
                ast::LandPlayAllowanceAdditional::Unlimited => Allowance::Unlimited,
                ast::LandPlayAllowanceAdditional::Amount(n) => Allowance::Additional {
                    count: fixed_nonnegative(n)?,
                },
            };
            rule(
                Battlefield,
                StaticRule::AdditionalLandPlays {
                    player: player(&x.allowance.who)?,
                    allowance,
                },
                None,
            )
        }
        S::PlayerRule(x) => hand_size(&x.rule)?,
        S::CostModification(x) => spell_cost(x)?,
        S::CastTiming(x) => {
            let x = &x.scope;
            if x.from.is_some() || x.not_from.is_some() || x.occurrence.is_some() {
                return Err(gap("cast-timing:qualifier"));
            }
            rule(
                Battlefield,
                StaticRule::CastAsThoughFlash {
                    affects: scope(&x.spells, true)?,
                    caster: x
                        .caster
                        .as_deref()
                        .map(player)
                        .transpose()?
                        .unwrap_or(PlayerExpr::EachPlayer),
                },
                None,
            )
        }
        S::LandwalkForBasicLandTypes(_) => return Err(gap("landwalk-for-basic-land-types")),
        S::KeywordActivationLimit(_) => return Err(gap("keyword-activation-limit")),
        S::Group(_) => return Err(gap("group")),
        S::AsCascade(_) => return Err(gap("as-cascade")),
        S::EntryCopy(_) => return Err(gap("entry-copy")),
        S::SkipTime(_) => return Err(gap("skip-time")),
        S::LevelBand(_) => return Err(gap("level-band")),
        S::StationBand(_) => return Err(gap("station-band")),
        S::Replacement(_) => return Err(gap("replacement")),
        S::SpellAdditionalCost(_) => return Err(gap("spell-additional-cost")),
        S::AsEnters(_) => return Err(gap("entry-choice:as-enters")),
        S::AsTurnedFaceUp(_) => return Err(gap("as-turned-face-up")),
        S::AsTransforms(_) => return Err(gap("as-transforms")),
        S::EntryChoice(x) => {
            let [ast::Cost::PayLife(cost)] = x.costs.as_slice() else {
                return Err(gap("entry-choice:cost"));
            };
            if !matches!(&*x.what, ast::ObjectRef::SelfObject(_))
                && !matches!(&*x.what, ast::ObjectRef::This(x) if x.noun == "land")
            {
                return Err(gap("entry-choice:source"));
            }
            if !matches!(&*x.who, ast::PlayerRef::You(_)) || cost.multiplier.is_some() {
                return Err(gap("entry-choice:payer"));
            }
            let amount = fixed_nonnegative(&cost.amount)?;
            if amount == Quantity::zero() {
                return Err(gap("entry-choice:zero"));
            }
            ability(Battlefield, StaticEffect::PayLifeOrEnterTapped { amount })
        }
        S::PreventDamage(_) => return Err(gap("prevent-damage")),
        S::RedirectDamage(_) => return Err(gap("redirect-damage")),
        S::Prohibit(_) => return Err(gap("prohibit")),
        S::CastLimit(_) => return Err(gap("cast-limit")),
        S::LegendRuleExemption(_) => return Err(gap("legend-rule-exemption")),
        S::AdditionalTriggers(_) => return Err(gap("additional-triggers")),
        S::TriggerSuppression(_) => return Err(gap("trigger-suppression")),
        S::RetainDamageDuringCleanup(_) => return Err(gap("retain-damage-during-cleanup")),
        S::PlayPermission(x) => cast_permission(&x.permission)?,
        S::PlayPermissionChoice(_) => return Err(gap("play-permission-choice")),
        S::LookPermission(_) => return Err(gap("look-permission")),
        S::CastRestriction(_) => return Err(gap("cast-restriction")),
        S::TargetChoiceRequirement(_) => return Err(gap("target-choice-requirement")),
        S::ActivationAsThoughHaste(_) => return Err(gap("activation-as-though-haste")),
        S::LoyaltyActivationLimit(_) => return Err(gap("loyalty-activation-limit")),
        S::ActivationTiming(_) => return Err(gap("activation-timing")),
        S::TargetingAsThough(_) => return Err(gap("targeting-as-though")),
        S::ManaSpending(_) => return Err(gap("mana-spending")),
        S::PlayRevealed(_) => return Err(gap("play-revealed")),
        S::RevealFirstDraw(_) => return Err(gap("reveal-first-draw")),
        S::CommanderEligibility(_) => return Err(gap("commander-eligibility")),
        S::CommanderPregameChoice(_) => return Err(gap("commander-pregame-choice")),
        S::BeginOnBattlefield(_) => return Err(gap("opening-hand:begin-on-battlefield")),
        S::Reveal(_) => return Err(gap("reveal")),
        S::PregameDeckReveal(_) => return Err(gap("pregame-deck-reveal")),
        S::DeckCopyLimit(_) => return Err(gap("deck-copy-limit")),
        S::OptionalUntap(_) => return Err(gap("optional-untap")),
        S::UntapDuringStep(_) => return Err(gap("untap-during-step")),
        S::UntapStepLimit(_) => return Err(gap("untap-step-limit")),
        S::CastSpellAbilities(_) => return Err(gap("cast-spell-abilities")),
        S::AlternativeCost(_) => return Err(gap("alternative-cost")),
        S::DamageAsThough(_) => return Err(gap("damage-as-though")),
        S::AlternativeActivationCost(_) => return Err(gap("alternative-activation-cost")),
        S::PaymentRestriction(_) => return Err(gap("payment-restriction")),
        S::VariableConstraints(_) => return Err(gap("variable-constraints")),
        S::PlayerAbilities(_) => return Err(gap("player-abilities")),
        S::Control(_) => return Err(gap("control")),
        S::CdaPt(_) => return Err(gap("cda-pt")),
        S::CdaColors(_) => return Err(gap("cda-colors")),
        S::CdaTypes(_) => return Err(gap("cda-types")),
        S::EntersUntapped(_) => return Err(gap("enters-untapped")),
        S::EntersPrepared(_) => return Err(gap("enters-prepared")),
        S::EntersUnderControl(_) => return Err(gap("enters-under-control")),
        S::EntersWithAbilities(_) => return Err(gap("enters-with-abilities")),
        S::UntapStepSuppression(_) => return Err(gap("untap-step-suppression")),
        S::EntersWithCounters(x) => {
            let source = matches!(&*x.what, ast::ObjectRef::SelfObject(_))
                || matches!(&*x.what, ast::ObjectRef::This(x) if matches!(x.noun.as_str(),
                    "permanent" | "creature" | "artifact" | "enchantment" | "aura" | "land"));
            if !source
                || x.counter.r#type != "+1/+1"
                || x.additional.is_some()
                || x.multiplier.is_some()
                || x.condition.is_some()
                || x.condition_kind.is_some()
                || x.bindings.is_some()
            {
                return Err(gap("enters-with-counters"));
            }
            let count = fixed_nonnegative(&x.count)?;
            if count == Quantity::zero() {
                return Err(gap("enters-with-counters:zero"));
            }
            ability(
                Battlefield,
                StaticEffect::EntersWithPlusOneCounters { count },
            )
        }
        S::EntersWithCounterChoice(_) => return Err(gap("enters-with-counter-choice")),
        S::AttachmentLimit(_) => return Err(gap("attachment-limit")),
        S::PlayerCombatRule(_) => return Err(gap("player-combat-rule")),
        S::CrewContribution(_) => return Err(gap("crew-contribution")),
    };
    Ok(vec![one])
}

fn scope(what: &ast::ObjectRef, spell: bool) -> Result<Scope, Failure> {
    use ast::ObjectRef as R;
    match what {
        R::This(x)
            if (spell && x.noun == "spell")
                || (!spell
                    && matches!(
                        x.noun.as_str(),
                        "creature" | "artifact" | "enchantment" | "land" | "permanent"
                    )) =>
        {
            Ok(Scope::Source)
        }
        R::SelfObject(_) => Ok(Scope::Source),
        R::Each(x) => matching(&x.filter, spell),
        R::All(x) => matching(&x.filter, spell),
        R::Variant29(_) | R::Fortified(_) => Err(unsupported("static", "ref:attached-to:static")),
        other => Err(gap(&format!("scope:{}", variant(other)))),
    }
}

fn matching(filter: &ast::ObjectFilter, spell: bool) -> Result<Scope, Failure> {
    let mut filter = filter.clone();
    if spell && filter.cls == Some(ast::ObjectFilterCls::Spell) {
        filter.cls = None;
    } else if spell && filter.cls.is_some() {
        return Err(gap("scope:spell-class"));
    }
    Ok(Scope::Matching {
        filter: static_filter(&filter)?,
    })
}

fn static_filter(filter: &ast::ObjectFilter) -> Result<Pred, Failure> {
    lower_static_filter(filter).map_err(|e| gap(&format!("scope:{}", e.reason)))
}

fn player(who: &ast::PlayerRef) -> Result<PlayerExpr, Failure> {
    match who {
        ast::PlayerRef::You(_) => Ok(PlayerExpr::static_controller()),
        ast::PlayerRef::EachOpponent(_) => Ok(PlayerExpr::EachOpponent),
        ast::PlayerRef::EachPlayer(_) => Ok(PlayerExpr::EachPlayer),
        _ => Err(gap("player")),
    }
}

fn fixed_value(n: &ast::Amount) -> Result<i64, Failure> {
    match n {
        ast::Amount::Fixed(x) => Ok(x.value),
        _ => Err(gap(&format!("amount:{}", variant(n)))),
    }
}

fn fixed_nonnegative(n: &ast::Amount) -> Result<Quantity, Failure> {
    let n = fixed_value(n)?;
    if n < 0 {
        return Err(gap("amount:negative"));
    }
    Ok(n.into())
}

fn signed(n: &ast::SignedAmount) -> Result<Amount, Failure> {
    let n_value = fixed_nonnegative(&n.amount)?;
    Ok(Amount::fixed(match &*n.sign {
        ast::SignedAmountSign::N1 => n_value,
        ast::SignedAmountSign::Minus1 => negative(&n_value),
    }))
}

fn negative(value: &Quantity) -> Quantity {
    let mut result = Quantity::zero();
    result.subtract(value);
    result
}

fn and(first: Option<Cond>, last: Option<Cond>) -> Option<Cond> {
    match (first, last) {
        (Some(first), Some(last)) => Some(Cond::All {
            conditions: vec![first, last],
        }),
        (first, last) => first.or(last),
    }
}

fn qualified_condition(
    value: Option<&ast::Condition>,
    kind: Option<ast::SentenceConditionKind>,
) -> Result<Option<Cond>, Failure> {
    match (value, kind) {
        (None, None) => Ok(None),
        (Some(value), Some(ast::SentenceConditionKind::Unless)) => Ok(Some(Cond::Not {
            condition: Box::new(condition(value)?),
        })),
        (Some(value), _) => condition(value).map(Some),
        (None, Some(_)) => Err(gap("conditional:missing")),
    }
}

fn entry_condition(value: &ast::Condition) -> Result<EntryCondition, Failure> {
    let controller = |who: &ast::PlayerRef| matches!(who, ast::PlayerRef::You(_));
    let battlefield_filter = |filter: &ast::ObjectFilter| {
        if filter.zone.is_some() {
            return Err(gap("enters-tapped:control-zone"));
        }
        static_filter(filter)
    };
    match value {
        ast::Condition::Control(x)
            if controller(&x.who) && x.time.is_none() && x.as_of.is_none() =>
        {
            match &*x.what {
                ast::ObjectRef::Filter(f)
                    if fixed_value(&f.count)? == 1
                        && f.up_to.is_none()
                        && f.group_constraints.is_none()
                        && f.random.is_none() =>
                {
                    Ok(EntryCondition::UnlessControlled {
                        filter: battlefield_filter(&f.filter)?,
                        cmp: Cmp::AtLeast,
                        count: 1.into(),
                    })
                }
                _ => Err(gap("enters-tapped:control")),
            }
        }
        ast::Condition::ControlCount(x) if controller(&x.who) => {
            let (cmp, count) = comparison(&x.comparison)?;
            Ok(EntryCondition::UnlessControlled {
                filter: battlefield_filter(&x.of)?,
                cmp,
                count: count.into(),
            })
        }
        ast::Condition::OpponentCount(x) if controller(&x.who) => {
            let (cmp, count) = comparison(&x.comparison)?;
            Ok(EntryCondition::UnlessOpponents {
                cmp,
                count: count.into(),
            })
        }
        _ => Err(gap("enters-tapped:conditional")),
    }
}

fn comparison(cmp: &ast::Comparison) -> Result<(Cmp, i64), Failure> {
    let right = fixed_value(&cmp.value)?;
    let overflow = || gap("conditional:comparison-bound");
    let (cmp, right) = match cmp.op {
        ast::ComparisonOp::Eq => (Cmp::Exactly, right),
        ast::ComparisonOp::Ge => (Cmp::AtLeast, right),
        ast::ComparisonOp::Gt => (Cmp::AtLeast, right.checked_add(1).ok_or_else(overflow)?),
        ast::ComparisonOp::Le => (Cmp::AtMost, right),
        ast::ComparisonOp::Lt => (Cmp::AtMost, right.checked_sub(1).ok_or_else(overflow)?),
    };
    Ok((cmp, right))
}

fn compare(left: Amount, cmp: &ast::Comparison) -> Result<Cond, Failure> {
    let (cmp, right) = comparison(cmp)?;
    Ok(Cond::Compare {
        left,
        cmp,
        right: Amount::fixed(right),
    })
}

fn counted_set(
    filter: &ast::ObjectFilter,
    controller: Option<PlayerExpr>,
) -> Result<SetExpr, Failure> {
    let mut terms = filter.clone();
    let zone = match terms.zone.take().as_deref() {
        None => ZoneExpr::Battlefield,
        Some(ast::ZoneRef {
            zone: ast::Zone::Battlefield,
            depth: None,
            position: None,
            owner: None,
        }) => ZoneExpr::Battlefield,
        Some(other) => graveyard_zone(other)?,
    };
    if !matches!(zone, ZoneExpr::Battlefield) && terms.cls == Some(ast::ObjectFilterCls::Card) {
        terms.cls = None;
    }
    let mut filter = static_filter(&terms)?;
    if let Some(player) = controller {
        if !matches!(zone, ZoneExpr::Battlefield) {
            return Err(gap("conditional:control-zone"));
        }
        filter.0.push(Atom::ControlledBy { player });
    }
    Ok(SetExpr::Matching { zone, filter })
}

fn amount(value: &ast::Amount) -> Result<Amount, Failure> {
    match value {
        ast::Amount::Fixed(n) => Ok(Amount::fixed(n.value)),
        ast::Amount::Count(n) => Ok(Amount::Count {
            set: counted_set(&n.of, None)?,
        }),
        ast::Amount::Devotion(n) => {
            let colors = match &*n.colors {
                ast::AmountDevotionColors::Array((a,)) => vec![color(a)],
                ast::AmountDevotionColors::Array2((a, b)) => vec![color(a), color(b)],
                ast::AmountDevotionColors::Choice(_) => {
                    return Err(gap("conditional:chosen-devotion"));
                }
            };
            Ok(Amount::Devotion {
                player: player(&n.who)?,
                colors,
            })
        }
        other => Err(gap(&format!("conditional:amount:{}", variant(other)))),
    }
}

fn condition(value: &ast::Condition) -> Result<Cond, Failure> {
    use ast::Condition as C;
    let active = || Cond::ActivePlayer {
        player: PlayerExpr::static_controller(),
    };
    match value {
        C::YourTurn(_) => Ok(active()),
        C::NotYourTurn(_) => Ok(Cond::Not {
            condition: Box::new(active()),
        }),
        C::TurnOf(x) => Ok(Cond::ActivePlayer {
            player: player(&x.who)?,
        }),
        C::Not(x) => Ok(Cond::Not {
            condition: Box::new(condition(&x.of)?),
        }),
        C::Conditions(x) if !x.conditions.is_empty() => {
            let any = x.condition == ast::ConditionConditionsCondition::AnyOf;
            let conditions = x
                .conditions
                .iter()
                .map(|value| {
                    condition(value).map(|c| {
                        if any {
                            Cond::Not {
                                condition: Box::new(c),
                            }
                        } else {
                            c
                        }
                    })
                })
                .collect::<Result<_, _>>()?;
            let all = Cond::All { conditions };
            Ok(if any {
                Cond::Not {
                    condition: Box::new(all),
                }
            } else {
                all
            })
        }
        C::Compare(x) => compare(amount(&x.value)?, &x.comparison),
        C::Count(x) => compare(
            Amount::Count {
                set: counted_set(&x.of, None)?,
            },
            &x.comparison,
        ),
        C::ControlCount(x) => compare(
            Amount::Count {
                set: counted_set(&x.of, Some(player(&x.who)?))?,
            },
            &x.comparison,
        ),
        C::Control(x) if x.time.is_none() && x.as_of.is_none() => match &*x.what {
            ast::ObjectRef::Filter(f)
                if fixed_value(&f.count)? == 1
                    && f.up_to.is_none()
                    && f.group_constraints.is_none()
                    && f.random.is_none() =>
            {
                Ok(Cond::Exists {
                    set: counted_set(&f.filter, Some(player(&x.who)?))?,
                })
            }
            _ => Err(gap("conditional:control")),
        },
        other => Err(gap(&format!("conditional:{}", variant(other)))),
    }
}

fn prohibition(action: &ast::CantAction) -> Result<CombatRequirement, Failure> {
    use ast::CantAction as A;
    Ok(match action {
        A::Attack => CombatRequirement::CantAttack,
        A::Block => CombatRequirement::CantBlock,
        A::AttackOrBlock => CombatRequirement::CantAttackOrBlock,
        A::BeBlocked => CombatRequirement::CantBeBlocked,
        other => return Err(gap(&format!("cant:{}", variant(other)))),
    })
}

fn combat_rule(what: &ast::ObjectRef, value: &ast::CombatRule) -> Result<StaticAbility, Failure> {
    use ast::CombatRule as C;
    let (requirement, condition) = match value {
        C::AttackRequirement(x)
            if x.frequency == ast::CombatRuleAttackRequirementFrequency::EachCombat
                && x.defender.is_none()
                && x.when_attacking.is_none() =>
        {
            (
                CombatRequirement::AttackEachCombat,
                qualified_condition(x.condition.as_deref(), x.condition_kind)?,
            )
        }
        C::BlockingFilter(x)
            if x.mode == ast::CombatRuleBlockerFilterMode::Only
                && matches!(scope(&x.attackers, false)?, Scope::Matching { filter }
                if filter.0 == [Atom::Type { kind: CardKind::Creature }, Atom::Keyword { keyword: KeywordAbility::Flying }]) =>
        {
            (
                CombatRequirement::CanBlockOnlyFlying,
                qualified_condition(x.condition.as_deref(), x.condition_kind)?,
            )
        }
        other => return Err(gap(&format!("combat-rule:{}", variant(other)))),
    };
    Ok(rule(
        FunctioningZone::Battlefield,
        StaticRule::Combat {
            affects: scope(what, false)?,
            requirement,
        },
        condition,
    ))
}

fn hand_size(value: &ast::PlayerRule) -> Result<StaticAbility, Failure> {
    use ast::PlayerRuleMaximumHandSizeChange as C;
    let ast::PlayerRule::MaximumHandSize(x) = value else {
        return Err(gap(&format!("player-rule:{}", variant(value))));
    };
    let change = match &*x.change {
        C::None(_) => HandSizeChange::Unlimited,
        C::Set(x) => HandSizeChange::Set {
            size: fixed_nonnegative(&x.value)?,
        },
        C::Amount(x) => {
            let amount = fixed_nonnegative(&x.amount)?;
            let delta = match x.operation {
                ast::CostModificationChangeDirection::Increase => amount,
                ast::CostModificationChangeDirection::Reduce => negative(&amount),
            };
            HandSizeChange::Adjust { delta }
        }
    };
    Ok(rule(
        FunctioningZone::Battlefield,
        StaticRule::MaximumHandSize {
            player: player(&x.who)?,
            change,
        },
        None,
    ))
}

fn cast_permission(permission: &ast::CardPlayInstruction) -> Result<StaticAbility, Failure> {
    let ast::CardPlayInstruction {
        who,
        action: ast::CardPlayInstructionAction::Cast,
        what,
        from: None,
        from_among: None,
        alternative_cost: None,
        optional_alternative_cost: None,
        required_if_able: None,
        additional_costs: None,
        spell_abilities: None,
        timing: Some(_),
        using_keyword: None,
        as_adventure: None,
        occurrence: None,
        condition: guard,
        use_limit: None,
        mana_payment: None,
        riders: None,
    } = permission
    else {
        return Err(gap("play-permission:qualifier"));
    };
    Ok(rule(
        FunctioningZone::Battlefield,
        StaticRule::CastAsThoughFlash {
            affects: scope(what, true)?,
            caster: player(who)?,
        },
        guard.as_deref().map(condition).transpose()?,
    ))
}

fn spell_cost(x: &ast::StaticEffectCostModification) -> Result<StaticAbility, Failure> {
    let ast::CostModificationScope::Cast(scope_data) = &*x.scope else {
        return Err(gap("cost-modification:activation"));
    };
    if scope_data.from.is_some()
        || scope_data.not_from.is_some()
        || scope_data.occurrence.is_some()
        || x.bindings.is_some()
        || x.minimum_total_mana.is_some()
        || x.maximum_reduction.is_some()
        || x.colored_only.is_some()
        || x.only_color.is_some()
    {
        return Err(gap("cost-modification:qualifier"));
    }
    if x.change.symbols.is_empty() || x.change.symbols.iter().any(|s| s.contains(['{', '}'])) {
        return Err(gap("cost-modification:symbols"));
    }
    let text = x
        .change
        .symbols
        .iter()
        .map(|symbol| format!("{{{symbol}}}"))
        .collect::<String>();
    let mana = mana_cost(&text).map_err(|_| gap("cost-modification:symbols"))?;
    let change = match x.change.direction {
        ast::CostModificationChangeDirection::Increase => SpellCostChange::Increase { mana },
        ast::CostModificationChangeDirection::Reduce => SpellCostChange::Reduce { mana },
    };
    let affects = scope(&scope_data.spells, true)?;
    let zone = if affects == Scope::Source {
        FunctioningZone::Stack
    } else {
        FunctioningZone::Battlefield
    };
    let caster = scope_data
        .caster
        .as_deref()
        .map(player)
        .transpose()?
        .unwrap_or(PlayerExpr::EachPlayer);
    Ok(rule(
        zone,
        StaticRule::SpellCost {
            affects,
            caster,
            change,
            multiplier: x.multiplier.as_deref().map(amount).transpose()?,
        },
        x.condition.as_deref().map(condition).transpose()?,
    ))
}

fn graveyard_zone(zone: &ast::ZoneRef) -> Result<ZoneExpr, Failure> {
    if zone.zone != ast::Zone::Graveyard || zone.position.is_some() || zone.depth.is_some() {
        return Err(gap("conditional:zone"));
    }
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    match zone.owner.as_deref() {
        Some(Owner::Your) => Ok(ZoneExpr::Graveyard {
            of: PlayerExpr::static_controller(),
        }),
        None | Some(Owner::Any | Owner::EachPlayer | Owner::AllPlayers) => Ok(ZoneExpr::Graveyards),
        _ => Err(gap("conditional:graveyard-owner")),
    }
}

fn keyword(grant: &ast::GainedAbility) -> Result<KeywordAbility, Failure> {
    let ast::GainedAbility::Keyword(g) = grant else {
        return Err(gap(&format!("modifier:grant:{}", variant(grant))));
    };
    let ast::KeywordInstance {
        keyword,
        cost,
        cost_modifications,
        restriction,
        amount,
        bindings,
        variable_constraints,
        from,
        filter,
        materials,
        companion,
        paired_with,
        enchant_player,
        prototype,
        additional_costs,
        gift,
    } = &*g.keyword;
    if cost.is_some()
        || cost_modifications.is_some()
        || restriction.is_some()
        || amount.is_some()
        || bindings.is_some()
        || variable_constraints.is_some()
        || from.is_some()
        || filter.is_some()
        || materials.is_some()
        || companion.is_some()
        || paired_with.is_some()
        || enchant_player.is_some()
        || prototype.is_some()
        || additional_costs.is_some()
        || gift.is_some()
    {
        return Err(gap("modifier:parameterized-keyword"));
    }
    KeywordAbility::deserialize(
        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(keyword),
    )
    .map_err(|_| gap(&format!("keyword:{keyword}")))
}

fn modify(x: &ast::StaticEffectModify) -> Result<Vec<StaticAbility>, Failure> {
    if x.condition_scope.is_some()
        || x.duration.is_some()
        || x.bindings.is_some()
        || x.modifiers.is_empty()
    {
        return Err(gap("modifier:qualifier"));
    }
    let condition = x.condition.as_deref().map(condition).transpose()?;
    let affects = scope(&x.what, false)?;
    let mut changes = Vec::new();
    let mut rules = Vec::new();
    // A modifier's value or condition may refer to each recipient rather
    // than the source. Those forms need their own binding and admission.
    macro_rules! plain {
        ($m:ident) => {
            if $m.for_each.is_some()
                || $m.multiplier.is_some()
                || $m.value_scope.is_some()
                || $m.condition.is_some()
                || $m.condition_scope.is_some()
            {
                return Err(gap("modifier:qualified"));
            }
        };
    }
    use ast::StaticModifier as M;
    for modifier in &x.modifiers {
        changes.push(match modifier {
            M::Pt(m) => {
                plain!(m);
                Change::PowerToughness {
                    power: signed(&m.power)?,
                    toughness: signed(&m.toughness)?,
                }
            }
            M::BasePt(m) => {
                plain!(m);
                let (Some(power), Some(toughness)) = (&m.power, &m.toughness) else {
                    return Err(gap("modifier:partial-base-pt"));
                };
                Change::SetPowerToughness {
                    power: Amount::fixed(fixed_value(power)?),
                    toughness: Amount::fixed(fixed_value(toughness)?),
                }
            }
            M::Abilities(m) => {
                plain!(m);
                let keywords = m.abilities.iter().map(keyword).collect::<Result<_, _>>()?;
                Change::AddKeywords { keywords }
            }
            M::Colors(m) => {
                plain!(m);
                let ast::StaticModifierColorsColors::Array(colors) = &*m.colors else {
                    return Err(gap("modifier:chosen-color"));
                };
                if m.operation != ast::TypeChangeChosenSubtypeOperation::Set {
                    return Err(gap("modifier:color-operation"));
                }
                Change::SetColors {
                    colors: colors.iter().map(color).collect(),
                }
            }
            M::Types(m) => {
                plain!(m);
                let ast::TypeChange {
                    card_types: Some(types),
                    subtypes: None,
                    chosen_subtype: None,
                    supertypes: None,
                } = &*m.change
                else {
                    return Err(gap("modifier:type-operands"));
                };
                if types.operation != ast::TypeChangeCardTypesOperation::Add {
                    return Err(gap("modifier:type-operation"));
                }
                Change::AddTypes {
                    kinds: types
                        .types
                        .iter()
                        .map(|k| card_kind(k, "static"))
                        .collect::<Result<_, _>>()?,
                }
            }
            M::Cant(m) => {
                plain!(m);
                rules.push(rule(
                    FunctioningZone::Battlefield,
                    StaticRule::Combat {
                        affects: affects.clone(),
                        requirement: prohibition(&m.action)?,
                    },
                    condition.clone(),
                ));
                continue;
            }
            M::CombatRule(m) => {
                plain!(m);
                let mut ability = combat_rule(&x.what, &m.rule)?;
                if let StaticEffect::Rule {
                    condition: guard, ..
                } = &mut ability.effect
                {
                    *guard = and(guard.take(), condition.clone());
                }
                rules.push(ability);
                continue;
            }
            other => return Err(gap(&format!("modifier:{}", variant(other)))),
        });
    }
    if !rules.is_empty() && changes.iter().any(|c| c.layer() <= kir::Layer::Color) {
        return Err(gap("modifier:mixed-selection-layer"));
    }
    if !changes.is_empty() {
        rules.insert(
            0,
            ability(
                FunctioningZone::Battlefield,
                StaticEffect::Continuous {
                    affects,
                    changes,
                    condition,
                },
            ),
        );
    }
    Ok(rules)
}

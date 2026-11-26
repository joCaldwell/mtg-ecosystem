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

/// The printed ability a quoted grant adds to each recipient (CR 113.10a).
pub(crate) enum QuotedAbility {
    Mana(ManaAbility),
}

/// The block-level lowerers a static's leaves need. The shared static
/// dispatcher calls these hooks for leaves whose source semantics belong
/// to another lowerer; a new leaf (for example an entry choice) adds a hook
/// here rather than a branch in the composer.
pub(crate) struct StaticSite<'a> {
    /// Lowers the raw quoted ability at a grant leaf.
    pub quoted: &'a dyn Fn(&serde_json::Value) -> Result<QuotedAbility, Failure>,
}

pub(crate) fn lower_static(
    value: &serde_json::Value,
    site: &StaticSite<'_>,
) -> Result<Vec<StaticAbility>, Failure> {
    let effect = typed::<ast::StaticEffect>(value, "static")
        .map_err(|error| gap(&format!("ast:{}", error.reason)))?;
    lower(&effect, value, site)
}

/// CR 205.3m: "is every creature type" (archived as setting or adding all
/// creature subtypes) and nothing else.
fn every_creature_type(change: &ast::TypeChange) -> bool {
    use ast::TypeChangeCardTypesOperation::{Add, Set};
    matches!(change, ast::TypeChange {
        card_types: None,
        subtypes: Some(items),
        chosen_subtype: None,
        supertypes: None,
    } if matches!(items.as_slice(), [item] if item.family == ast::SubtypeFamily::Creature
        && matches!(item.operation, Set | Add)
        && matches!(&*item.types, ast::TypeChangeSubtypesItemTypes::All(all) if all.basic.is_none())))
}

/// CR 604.3, 702.73a: "This object is every creature type" as a
/// characteristic-defining ability of the object itself. It functions in
/// every zone, so it is a printed characteristic rather than a static
/// ability. `None` means the block is some other static.
pub(crate) fn every_creature_type_cda(value: &serde_json::Value) -> Option<Result<(), Failure>> {
    if value.get("static")?.as_str()? != "cda-types" {
        return None;
    }
    let ast::StaticEffect::CdaTypes(x) = typed::<ast::StaticEffect>(value, "static").ok()? else {
        return None;
    };
    Some(
        if matches!(&*x.what, ast::ObjectRef::This(this) if this.noun == "creature")
            && every_creature_type(&x.change)
        {
            Ok(())
        } else {
            Err(gap("cda-types"))
        },
    )
}

/// CR 604.3, 208.2a, 613.4a: "[This creature's] power [and/or toughness]
/// is equal to ..." (or a group of such sentences) as one
/// characteristic-defining ability of the source itself, functioning in
/// every zone. `None` means the block is some other static.
pub(crate) fn characteristic_pt(
    value: &serde_json::Value,
) -> Option<Result<StaticAbility, Failure>> {
    let effect = typed::<ast::StaticEffect>(value, "static").ok()?;
    let parts: Vec<&ast::StaticEffectCdaPt> = match &effect {
        ast::StaticEffect::CdaPt(x) => vec![&**x],
        ast::StaticEffect::Group(g)
            if !g.effects.is_empty()
                && g.effects
                    .iter()
                    .all(|e| matches!(e, ast::StaticEffect::CdaPt(_))) =>
        {
            g.effects
                .iter()
                .filter_map(|e| match e {
                    ast::StaticEffect::CdaPt(x) => Some(&**x),
                    _ => None,
                })
                .collect()
        }
        _ => return None,
    };
    Some((|| {
        let (mut power, mut toughness) = (None, None);
        for part in parts {
            if !matches!(&*part.what, ast::ObjectRef::SelfObject(_))
                && !matches!(&*part.what, ast::ObjectRef::This(this) if this.noun == "creature")
            {
                return Err(gap("cda-pt:source"));
            }
            let value = cda_value(&part.value)?;
            use ast::EffectMultiplyPtStat as Stat;
            for (defined, slot) in [
                (matches!(part.stat, Stat::Power | Stat::Both), &mut power),
                (
                    matches!(part.stat, Stat::Toughness | Stat::Both),
                    &mut toughness,
                ),
            ] {
                if defined {
                    if slot.is_some() {
                        return Err(gap("cda-pt:repeated-stat"));
                    }
                    *slot = Some(value.clone());
                }
            }
        }
        Ok(ability(
            FunctioningZone::Any,
            StaticEffect::DefinePowerToughness { power, toughness },
        ))
    })())
}

/// CR 208.2a: the values a characteristic-defining ability can define.
/// Each reads public facts continuously (CR 604.3); anything else rejects
/// by name under `static:cda-pt:*`.
fn cda_value(value: &ast::Amount) -> Result<Amount, Failure> {
    use ast::Amount as A;
    Ok(match value {
        A::Fixed(n) if n.value >= 0 => Amount::fixed(n.value),
        A::Count(n) => Amount::Count {
            set: cda_set(&n.of)?,
        },
        A::Plus(x) => {
            let mut terms = Vec::new();
            for term in [&x.a, &x.b] {
                match cda_value(term)? {
                    Amount::Sum { terms: inner } => terms.extend(inner),
                    other => terms.push(other),
                }
            }
            Amount::Sum { terms }
        }
        A::Twice(x) => Amount::Times {
            factor: Quantity::from(2),
            of: Box::new(cda_value(&x.of)?),
        },
        A::Devotion(_) => amount(value)?,
        // CR 700.5: the mana symbols of one color among the mana costs of
        // the permanents you control are your devotion to that color.
        A::ManaSymbols(x)
            if present_fields(&x.r#in) == ["cls", "control"]
                && x.r#in.cls == Some(ast::ObjectFilterCls::Permanent)
                && x.r#in.control == Some(ast::ObjectFilterControl::You) =>
        {
            Amount::Devotion {
                player: PlayerExpr::static_controller(),
                colors: vec![color(&x.color)],
            }
        }
        A::AttributeExtremum(x)
            if x.extremum == ast::AmountAttributeExtremumExtremum::Maximum
                && x.attribute == ast::ObjectFilterImplicitComparisonAttribute::ManaValue =>
        {
            let set = cda_set(&x.among)?;
            if matches!(
                set,
                SetExpr::Matching {
                    zone: ZoneExpr::Hand { .. },
                    ..
                }
            ) {
                return Err(gap("cda-pt:greatest:zone"));
            }
            Amount::Greatest {
                set,
                stat: kir::Stat::ManaValue,
            }
        }
        A::DistinctCount(x) => {
            use ast::AmountDistinctCountCharacteristic as C;
            let characteristic = match x.characteristic {
                C::CardType => kir::DistinctCharacteristic::CardType,
                C::BasicLandType => kir::DistinctCharacteristic::BasicLandType,
                C::Color => kir::DistinctCharacteristic::Color,
                other => return Err(gap(&format!("cda-pt:distinct:{}", variant(&other)))),
            };
            let ast::ObjectRef::Each(each) = &*x.among else {
                return Err(gap("cda-pt:distinct:among"));
            };
            Amount::Distinct {
                set: cda_set(&each.filter)?,
                characteristic,
            }
        }
        other => return Err(gap(&format!("cda-pt:amount:{}", variant(other)))),
    })
}

/// CR 400.2, 404.2: the public objects a characteristic-defining count
/// reads: permanents, graveyard cards, or a whole hand (whose size is
/// public). "You" is the source's controller, or its owner outside the
/// battlefield and stack (CR 109.4, 109.5).
fn cda_set(filter: &ast::ObjectFilter) -> Result<SetExpr, Failure> {
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    let mut terms = filter.clone();
    let zone = match terms.zone.take().as_deref() {
        None
        | Some(ast::ZoneRef {
            zone: ast::Zone::Battlefield,
            depth: None,
            position: None,
            owner: None,
        }) => ZoneExpr::Battlefield,
        Some(ast::ZoneRef {
            zone: zone @ (ast::Zone::Graveyard | ast::Zone::Hand),
            depth: None,
            position: None,
            owner,
        }) => {
            let of = match owner.as_deref() {
                Some(Owner::Your) => PlayerExpr::static_controller(),
                Some(Owner::AllOpponents | Owner::EachOpponent) => PlayerExpr::EachOpponent,
                Some(Owner::AllPlayers | Owner::EachPlayer | Owner::Any) => PlayerExpr::EachPlayer,
                _ => return Err(gap("cda-pt:zone-owner")),
            };
            match zone {
                ast::Zone::Hand => ZoneExpr::Hand { of },
                _ if of == PlayerExpr::EachPlayer => ZoneExpr::Graveyards,
                _ => ZoneExpr::Graveyard { of },
            }
        }
        Some(_) => return Err(gap("cda-pt:zone")),
    };
    if zone != ZoneExpr::Battlefield && terms.cls == Some(ast::ObjectFilterCls::Card) {
        terms.cls = None;
    }
    // A filter term outside the static tables (snow, face-down, ...)
    // rejects in the CDA family.
    let filter = lower_static_filter(&terms).map_err(|e| {
        let term = e
            .reason
            .strip_prefix("operand:filter:")
            .unwrap_or(&e.reason);
        gap(&format!("cda-pt:filter:{term}"))
    })?;
    if matches!(zone, ZoneExpr::Hand { .. }) && !filter.0.is_empty() {
        return Err(gap("cda-pt:filtered-hand"));
    }
    Ok(SetExpr::Matching { zone, filter })
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

fn lower(
    effect: &ast::StaticEffect,
    raw: &serde_json::Value,
    site: &StaticSite<'_>,
) -> Result<Vec<StaticAbility>, Failure> {
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
        S::Modify(x) => return modify(x, raw, site),
        S::Conditional(x) => {
            if x.otherwise.is_some() {
                return Err(gap("conditional:otherwise"));
            }
            let guard = condition(&x.condition)?;
            let mut abilities = lower(&x.effect, &raw["effect"], site)?;
            for a in &mut abilities {
                let condition = match &mut a.effect {
                    // CR 613.8: a condition on a layer-4 creature-type change
                    // would need dependency ordering.
                    StaticEffect::Continuous { changes, .. }
                        if changes.contains(&Change::AllCreatureTypes) =>
                    {
                        return Err(gap("modifier:all-creature-types:condition"));
                    }
                    StaticEffect::Continuous { condition, .. }
                    | StaticEffect::Rule { condition, .. } => condition,
                    StaticEffect::EntersTappedIf { .. } => {
                        return Err(gap("enters-tapped:nested-conditional"));
                    }
                    StaticEffect::EntersTapped => return Err(gap("enters-tapped:conditional")),
                    StaticEffect::PayLifeOrEnterTapped { .. }
                    | StaticEffect::RevealOrEnterTapped { .. } => {
                        return Err(gap("entry-choice:conditional"));
                    }
                    StaticEffect::EntersWithPlusOneCounters { .. } => {
                        return Err(gap("enters-with-counters:conditional"));
                    }
                    // CR 604.3a(5): a conditional value is not a CDA.
                    StaticEffect::DefinePowerToughness { .. } => {
                        return Err(gap("cda-pt:conditional"));
                    }
                };
                *condition = and(condition.take(), Some(guard.clone()));
            }
            return Ok(abilities);
        }
        S::Group(x) if !x.effects.is_empty() => {
            let mut abilities = Vec::new();
            if raw_items(raw, "effects").count() != x.effects.len() {
                return Err(gap("group:raw-shape"));
            }
            for (effect, raw) in x.effects.iter().zip(raw_items(raw, "effects")) {
                abilities.extend(lower(effect, raw, site)?);
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
        S::Replacement(x) => damage_replacement(&x.replacement)?,
        S::SpellAdditionalCost(_) => return Err(gap("spell-additional-cost")),
        S::AsEnters(_) => return Err(gap("entry-choice:as-enters")),
        S::AsTurnedFaceUp(_) => return Err(gap("as-turned-face-up")),
        S::AsTransforms(_) => return Err(gap("as-transforms")),
        S::EntryChoice(x) => {
            if !matches!(&*x.what, ast::ObjectRef::SelfObject(_))
                && !matches!(&*x.what, ast::ObjectRef::This(x) if x.noun == "land")
            {
                return Err(gap("entry-choice:source"));
            }
            if !matches!(&*x.who, ast::PlayerRef::You(_)) {
                return Err(gap("entry-choice:payer"));
            }
            let cost = match x.costs.as_slice() {
                [ast::Cost::PayLife(cost)] => cost,
                [ast::Cost::Reveal(cost)] => {
                    return Ok(vec![ability(
                        Battlefield,
                        StaticEffect::RevealOrEnterTapped {
                            filter: entry_reveal_filter(cost)?,
                        },
                    )]);
                }
                _ => return Err(gap("entry-choice:cost")),
            };
            if cost.multiplier.is_some() {
                return Err(gap("entry-choice:payer"));
            }
            let amount = fixed_nonnegative(&cost.amount)?;
            if amount == Quantity::zero() {
                return Err(gap("entry-choice:zero"));
            }
            ability(Battlefield, StaticEffect::PayLifeOrEnterTapped { amount })
        }
        S::PreventDamage(x) => static_prevention(&x.prevention)?,
        S::RedirectDamage(_) => return Err(gap("redirect-damage")),
        S::Prohibit(x) => match &*x.prohibition {
            ast::GameProhibition::PreventDamage(x) if x.when.is_none() => rule(
                Battlefield,
                StaticRule::DamageCantBePrevented {
                    event: damage_event(x.kind, x.by.as_deref(), x.to.as_deref(), None)?,
                },
                None,
            ),
            _ => return Err(gap("prohibit")),
        },
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

/// The raw JSON items beside a typed array, in order; a missing array
/// yields none, so a zip with the typed items stops at once.
fn raw_items<'a>(
    raw: &'a serde_json::Value,
    field: &str,
) -> impl Iterator<Item = &'a serde_json::Value> {
    raw[field].as_array().into_iter().flatten()
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
        R::Variant29(x) if !spell => {
            let kind = match x.r#ref {
                ast::ObjectRefVariant29Ref::Equipped => kir::AttachmentKind::Equipped,
                ast::ObjectRefVariant29Ref::Enchanted => kir::AttachmentKind::Enchanted,
            };
            let atoms = match x.noun.as_str() {
                "permanent" => vec![],
                "land" => vec![kir::Atom::Type {
                    kind: CardKind::Land,
                }],
                "artifact" => vec![kir::Atom::Type {
                    kind: CardKind::Artifact,
                }],
                "creature" => vec![kir::Atom::Type {
                    kind: CardKind::Creature,
                }],
                _ => return Err(gap("attachment-noun")),
            };
            Ok(Scope::Attachment {
                kind,
                filter: Pred(atoms),
            })
        }
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

/// CR 614.12, 701.20a: "you may reveal a [filter] card from your hand":
/// exactly one card, from the entering controller's own hand, by printed
/// characteristics (the library-card filter vocabulary).
fn entry_reveal_filter(cost: &ast::CostReveal) -> Result<Pred, Failure> {
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    let ast::ObjectRef::Filter(what) = &*cost.what else {
        return Err(gap("entry-choice:reveal:object"));
    };
    let one = matches!(&*what.count, ast::Amount::Fixed(n) if n.value == 1);
    if cost.from.is_some()
        || !one
        || what.up_to.is_some()
        || what.group_constraints.is_some()
        || what.random.is_some()
    {
        return Err(gap("entry-choice:reveal:count"));
    }
    let mut filter = (*what.filter).clone();
    match filter.zone.take().as_deref() {
        Some(ast::ZoneRef {
            zone: ast::Zone::Hand,
            owner,
            depth: None,
            position: None,
        }) if owner.as_deref() == Some(&Owner::Your) => {}
        _ => return Err(gap("entry-choice:reveal:zone")),
    }
    if filter.cls != Some(ast::ObjectFilterCls::Card) {
        return Err(gap("entry-choice:reveal:class"));
    }
    let pred = lower_card_filter(&filter)?;
    if pred.0.is_empty() {
        return Err(gap("entry-choice:reveal:unfiltered"));
    }
    Ok(pred)
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
        C::AsThough(x)
            if x.defenders.is_none() && x.attackers.is_none() && lacks_defender_to_attack(x) =>
        {
            (
                CombatRequirement::AttackAsThoughNoDefender,
                qualified_condition(x.condition.as_deref(), x.condition_kind)?,
            )
        }
        C::BlockerFilter(x) => {
            return declaration_rule(what, x.condition.as_deref(), x.condition_kind, || {
                Ok(kir::BlockRestriction::Blockers {
                    only: x.mode == ast::CombatRuleBlockerFilterMode::Only,
                    filter: declaration_set(&x.blockers)?,
                })
            });
        }
        C::BlockingFilter(x) => {
            return declaration_rule(what, x.condition.as_deref(), x.condition_kind, || {
                Ok(kir::BlockRestriction::Attackers {
                    only: x.mode == ast::CombatRuleBlockerFilterMode::Only,
                    filter: declaration_set(&x.attackers)?,
                })
            });
        }
        C::BlockerCount(x) => {
            return declaration_rule(what, x.condition.as_deref(), x.condition_kind, || {
                let ast::Amount::Fixed(n) = &*x.comparison.value else {
                    return Err(gap("combat-rule:blocker-count:amount"));
                };
                let n =
                    u32::try_from(n.value).map_err(|_| gap("combat-rule:blocker-count:amount"))?;
                use ast::CombatRuleBlockerFilterMode as M;
                use ast::ComparisonOp as O;
                // "can't be blocked by more than N" and "except by N or more".
                match (x.mode, x.comparison.op) {
                    (M::Exclude, O::Gt) if n >= 1 => Ok(kir::BlockRestriction::AtMost { count: n }),
                    (M::Only, O::Ge) if n >= 2 => Ok(kir::BlockRestriction::AtLeast { count: n }),
                    _ => Err(gap("combat-rule:blocker-count:comparison")),
                }
            });
        }
        C::AttackRestriction(x) => {
            // CR 508.1b: the engine's only attackable objects are players,
            // so "planeswalkers you control" adds no defender.
            let you = PlayerExpr::static_controller();
            let mut players = vec![];
            for defender in &x.defenders {
                match defender {
                    ast::CombatDefender::Player(p) => players.push(player(&p.who)?),
                    ast::CombatDefender::Object(o) => match &*o.what {
                        ast::ObjectRef::Each(each)
                            if each.filter.types.as_deref()
                                == Some(&["planeswalker".to_owned()])
                                && each.filter.control == Some(ast::ObjectFilterControl::You)
                                && present_fields(&each.filter).len() == 2 => {}
                        _ => return Err(gap("combat-rule:attack-restriction:defender")),
                    },
                    _ => return Err(gap("combat-rule:attack-restriction:defender")),
                }
            }
            if players != [you.clone()] {
                return Err(gap("combat-rule:attack-restriction:defender"));
            }
            let condition = qualified_condition(x.condition.as_deref(), x.condition_kind)?;
            return Ok(rule(
                FunctioningZone::Battlefield,
                StaticRule::AttackRestriction {
                    affects: declaration_scope(what)?,
                    defender: you,
                },
                condition,
            ));
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

/// CR 509.1b, 613.11: block restrictions read the affected creature and its
/// counterpart at the declaration, after every layer.
fn declaration_rule(
    what: &ast::ObjectRef,
    condition: Option<&ast::Condition>,
    kind: Option<ast::SentenceConditionKind>,
    restriction: impl FnOnce() -> Result<kir::BlockRestriction, Failure>,
) -> Result<StaticAbility, Failure> {
    let condition = qualified_condition(condition, kind)?;
    Ok(rule(
        FunctioningZone::Battlefield,
        StaticRule::Block {
            affects: declaration_scope(what)?,
            restriction: restriction()?,
        },
        condition,
    ))
}

fn declaration_scope(what: &ast::ObjectRef) -> Result<Scope, Failure> {
    match what {
        ast::ObjectRef::Each(x) => Ok(Scope::Matching {
            filter: declaration_filter(&x.filter)?,
        }),
        ast::ObjectRef::All(x) => Ok(Scope::Matching {
            filter: declaration_filter(&x.filter)?,
        }),
        other => scope(other, false),
    }
}

/// The creatures a restriction names ("Walls", "creatures with flying").
fn declaration_set(what: &ast::ObjectRef) -> Result<kir::Pred, Failure> {
    match what {
        ast::ObjectRef::Each(x) => declaration_filter(&x.filter),
        ast::ObjectRef::All(x) => declaration_filter(&x.filter),
        other => Err(gap(&format!("combat-rule:set:{}", variant(other)))),
    }
}

fn declaration_filter(filter: &ast::ObjectFilter) -> Result<kir::Pred, Failure> {
    lower_declaration_filter(filter).map_err(|e| gap(&format!("scope:{}", e.reason)))
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

fn keyword(grant: &ast::GainedAbility, raw: &serde_json::Value) -> Result<KeywordAbility, Failure> {
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
        || (filter.is_some() && keyword != "landwalk")
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
    if keyword == "landwalk" {
        return crate::compose::keywords::basic_landwalk(&raw["keyword"]["filter"])
            .ok_or_else(|| gap("modifier:landwalk:quality"));
    }
    KeywordAbility::deserialize(
        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(keyword),
    )
    .map_err(|_| gap(&format!("keyword:{keyword}")))
}

fn modify(
    x: &ast::StaticEffectModify,
    raw: &serde_json::Value,
    site: &StaticSite<'_>,
) -> Result<Vec<StaticAbility>, Failure> {
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
    let mut every_type = Vec::new();
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
    let raw_modifiers: Vec<_> = raw_items(raw, "modifiers").collect();
    if raw_modifiers.len() != x.modifiers.len() {
        return Err(gap("modifier:raw-shape"));
    }
    for (modifier, raw_modifier) in x.modifiers.iter().zip(raw_modifiers) {
        let change = match modifier {
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
                // CR 113.10, 613.1f: one layer-6 effect may add keywords and
                // quoted abilities; each kind is its own change.
                let raw_abilities: Vec<_> = raw_items(raw_modifier, "abilities").collect();
                if raw_abilities.len() != m.abilities.len() {
                    return Err(gap("modifier:raw-shape"));
                }
                let (mut keywords, mut mana) = (Vec::new(), Vec::new());
                for (grant, raw_grant) in m.abilities.iter().zip(raw_abilities) {
                    match grant {
                        ast::GainedAbility::Quoted(_) => {
                            match (site.quoted)(&raw_grant["ability"])? {
                                QuotedAbility::Mana(ability) => mana.push(ability),
                            }
                        }
                        other => keywords.push(keyword(other, raw_grant)?),
                    }
                }
                if !mana.is_empty() {
                    changes.push(Change::AddManaAbilities { abilities: mana });
                }
                if keywords.is_empty() {
                    continue;
                }
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
            M::Types(m) if every_creature_type(&m.change) => {
                plain!(m);
                // CR 205.3d: the change matters only while the recipient is
                // a creature, so it needs no type filter; it is its own
                // layer-4 effect beside the printed sentence's other changes.
                let affects = match &affects {
                    Scope::Source => Scope::Source,
                    Scope::Attachment { kind, filter }
                        if *filter
                            == Pred(vec![kir::Atom::Type {
                                kind: CardKind::Creature,
                            }]) =>
                    {
                        Scope::Attachment {
                            kind: *kind,
                            filter: Pred(vec![]),
                        }
                    }
                    _ => return Err(gap("modifier:all-creature-types:scope")),
                };
                if condition.is_some() {
                    return Err(gap("modifier:all-creature-types:condition"));
                }
                every_type.push(ability(
                    FunctioningZone::Battlefield,
                    StaticEffect::Continuous {
                        affects,
                        changes: vec![Change::AllCreatureTypes],
                        condition: None,
                    },
                ));
                continue;
            }
            M::Types(m) => {
                plain!(m);
                if let ast::TypeChange {
                    card_types: None,
                    subtypes: Some(items),
                    chosen_subtype: None,
                    supertypes: None,
                } = &*m.change
                    && let [item] = items.as_slice()
                    && item.family == ast::SubtypeFamily::Creature
                    && item.operation == ast::TypeChangeCardTypesOperation::Add
                    && let ast::TypeChangeSubtypesItemTypes::Array(subtypes) = &*item.types
                {
                    changes.push(Change::AddCreatureSubtypes {
                        subtypes: subtypes.clone(),
                    });
                    continue;
                }
                if let Some(replaced) = type_replacement(&m.change)? {
                    changes.extend(replaced);
                    continue;
                }
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
            M::LoseAbilities(m) => {
                plain!(m);
                use ast::AbilityRemoval as R;
                match &*m.removal {
                    R::All(_) => Change::RemoveAllAbilities {
                        except_own_grants: false,
                    },
                    // "loses all other abilities": the grants of this effect stay.
                    R::AllOther(_) => Change::RemoveAllAbilities {
                        except_own_grants: true,
                    },
                    R::Listed(listed) => {
                        let raw_listed: Vec<_> =
                            raw_items(&raw_modifier["removal"], "abilities").collect();
                        if raw_listed.len() != listed.abilities.len() {
                            return Err(gap("modifier:raw-shape"));
                        }
                        Change::RemoveKeywords {
                            keywords: listed
                                .abilities
                                .iter()
                                .zip(raw_listed)
                                .map(|(a, raw)| keyword(a, raw))
                                .collect::<Result<_, _>>()?,
                        }
                    }
                    R::ThisAbility(_) => return Err(gap("modifier:lose-abilities:this-ability")),
                    R::Choice(_) => return Err(gap("modifier:lose-abilities:choice")),
                }
            }
            M::CantHaveAbilities(m) => {
                plain!(m);
                Change::ForbidKeywords {
                    keywords: m
                        .abilities
                        .iter()
                        .map(|k| {
                            if !bare_keyword(k, &k.keyword) {
                                return Err(gap("modifier:parameterized-keyword"));
                            }
                            KeywordAbility::deserialize(serde::de::value::StrDeserializer::<
                                serde::de::value::Error,
                            >::new(&k.keyword))
                            .map_err(|_| gap(&format!("keyword:{}", k.keyword)))
                        })
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
        };
        changes.push(change);
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
    rules.extend(every_type);
    Ok(rules)
}

/// CR 205.1a, 305.7: card-type replacement or removal, with creature or
/// basic land subtypes replaced. `None` leaves additive shapes to the caller.
fn type_replacement(change: &ast::TypeChange) -> Result<Option<Vec<Change>>, Failure> {
    use ast::TypeChangeCardTypesOperation as Op;
    let ast::TypeChange {
        card_types,
        subtypes,
        chosen_subtype: None,
        supertypes: None,
    } = change
    else {
        return Ok(None);
    };
    let set_types = card_types
        .as_deref()
        .is_some_and(|t| t.operation != Op::Add);
    let set_subtypes = subtypes
        .iter()
        .flatten()
        .any(|item| item.operation == Op::Set);
    if !set_types && !set_subtypes {
        return Ok(None);
    }
    let mut out = Vec::new();
    if let Some(types) = card_types {
        let mut kinds = types
            .types
            .iter()
            .map(|k| card_kind(k, "static"))
            .collect::<Result<Vec<_>, _>>()?;
        kinds.sort_unstable();
        out.push(match types.operation {
            Op::Set => Change::SetCardTypes { kinds },
            Op::Remove => Change::RemoveCardTypes { kinds },
            Op::Add => return Err(gap("modifier:type-operation")),
        });
    }
    for item in subtypes.iter().flatten() {
        let ast::TypeChangeSubtypesItemTypes::Array(names) = &*item.types else {
            return Err(gap("modifier:type-operands"));
        };
        out.push(match (item.family, item.operation) {
            (ast::SubtypeFamily::Creature, Op::Set) => Change::SetCreatureSubtypes {
                subtypes: names.clone(),
            },
            (ast::SubtypeFamily::Land, Op::Set) => Change::SetLandSubtypes {
                subtypes: names.clone(),
            },
            _ => return Err(gap("modifier:type-operation")),
        });
    }
    Ok(Some(out))
}

/// CR 609.4, 702.3b: "can attack as though it didn't have defender".
pub(crate) fn lacks_defender_to_attack(x: &ast::CombatRuleAsThough) -> bool {
    x.action == ast::CombatRuleAsThoughAction::Attack
        && matches!(&*x.pretense, ast::CombatPretense::Keyword(p)
            if p.pretend == ast::CombatPretenseKeywordPretend::LacksKeyword
                && p.of == ast::CombatPretenseKeywordOf::AffectedCreature
                && bare_keyword(&p.keyword, "defender"))
}

/// The keyword alone, with no cost, amount, filter or other parameter.
fn bare_keyword(k: &ast::KeywordInstance, name: &str) -> bool {
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
    } = k;
    keyword == name
        && cost.is_none()
        && cost_modifications.is_none()
        && restriction.is_none()
        && amount.is_none()
        && bindings.is_none()
        && variable_constraints.is_none()
        && from.is_none()
        && filter.is_none()
        && materials.is_none()
        && companion.is_none()
        && paired_with.is_none()
        && enchant_player.is_none()
        && prototype.is_none()
        && additional_costs.is_none()
        && gift.is_none()
}

/// CR 609.7a: the source of damage a static damage rule names: the static's
/// own permanent, the permanent it is attached to, or every matching source,
/// read from its damage-time facts. "Source" is no characteristic.
fn damage_source_scope(what: &ast::ObjectRef) -> Result<Scope, Failure> {
    use ast::ObjectRef as R;
    match what {
        R::This(x)
            if matches!(
                x.noun.as_str(),
                "creature" | "artifact" | "enchantment" | "land" | "permanent"
            ) =>
        {
            Ok(Scope::Source)
        }
        R::SelfObject(_) => Ok(Scope::Source),
        R::Variant29(_) => scope(what, false),
        R::Filter(x)
            if one(&x.count)
                && x.up_to.is_none()
                && x.group_constraints.is_none()
                && x.random.is_none() =>
        {
            damage_source_filter(&x.filter)
        }
        R::Each(x) => damage_source_filter(&x.filter),
        R::All(x) => damage_source_filter(&x.filter),
        other => Err(gap(&format!("damage:source:{}", variant(other)))),
    }
}

fn damage_source_filter(filter: &ast::ObjectFilter) -> Result<Scope, Failure> {
    let mut filter = filter.clone();
    if filter.cls == Some(ast::ObjectFilterCls::Source) {
        filter.cls = None;
    }
    let filter = lower_declaration_filter(&filter)
        .map_err(|e| gap(&format!("damage:source:{}", e.reason)))?;
    if !kir::damage_source_pred_valid(&filter) {
        return Err(gap("damage:source:filter"));
    }
    Ok(Scope::Matching { filter })
}

/// CR 120.1: the players and permanents a static damage rule names.
fn damage_recipients(to: &[ast::AmountCounterCountOn]) -> Result<kir::DamageRecipients, Failure> {
    let mut out = kir::DamageRecipients {
        players: None,
        permanents: None,
    };
    for item in to {
        match item {
            ast::AmountCounterCountOn::PlayerRef(who) if out.players.is_none() => {
                out.players = Some(match &**who {
                    ast::PlayerRef::You(_) => PlayerExpr::static_controller(),
                    ast::PlayerRef::AnOpponent(_) | ast::PlayerRef::EachOpponent(_) => {
                        PlayerExpr::EachOpponent
                    }
                    ast::PlayerRef::APlayer(_) | ast::PlayerRef::EachPlayer(_) => {
                        PlayerExpr::EachPlayer
                    }
                    other => return Err(gap(&format!("damage:recipient:{}", variant(other)))),
                });
            }
            ast::AmountCounterCountOn::ObjectRef(what) if out.permanents.is_none() => {
                out.permanents = Some(match &**what {
                    ast::ObjectRef::Filter(x)
                        if one(&x.count)
                            && x.up_to.is_none()
                            && x.group_constraints.is_none()
                            && x.random.is_none() =>
                    {
                        recipient_filter(&x.filter)?
                    }
                    ast::ObjectRef::Each(x) => recipient_filter(&x.filter)?,
                    ast::ObjectRef::All(x) => recipient_filter(&x.filter)?,
                    other => scope(other, false)?,
                });
            }
            _ => return Err(gap("damage:recipient:repeated")),
        }
    }
    Ok(out)
}

fn recipient_filter(filter: &ast::ObjectFilter) -> Result<Scope, Failure> {
    Ok(Scope::Matching {
        filter: lower_declaration_filter(filter)
            .map_err(|e| gap(&format!("damage:recipient:{}", e.reason)))?,
    })
}

fn damage_event(
    kind: ast::ReplacementEventDamageKind,
    by: Option<&ast::ObjectRef>,
    to: Option<&[ast::AmountCounterCountOn]>,
    involving: Option<&ast::ObjectRef>,
) -> Result<kir::DamageEvent, Failure> {
    Ok(kir::DamageEvent {
        kind: super::verbs::damage_kind(kind),
        by: by.map(damage_source_scope).transpose()?,
        to: to.map(damage_recipients).transpose()?,
        involving: involving.map(damage_source_scope).transpose()?,
    })
}

/// CR 615.1, 615.10: "Prevent all [combat] damage that would be dealt
/// [to/by/to and by X]", "prevent N of that damage" and "prevent half that
/// damage". Conditions, incoming amounts, riders and source choices reject.
fn static_prevention(p: &ast::DamagePrevention) -> Result<StaticAbility, Failure> {
    if p.incoming_amount.is_some()
        || p.condition.is_some()
        || p.optional.is_some()
        || p.bindings.is_some()
        || p.after.is_some()
    {
        return Err(gap("prevent-damage:qualifier"));
    }
    use ast::DamagePreventionEventScopeSources::Each;
    let ast::DamagePreventionShield::Events(events) = &*p.shield else {
        return Err(gap("prevent-damage:shield"));
    };
    if events.occurrence != ast::EffectCreateDelayedTriggerOccurrence::Each {
        return Err(gap("prevent-damage:occurrence"));
    }
    // CR 615.10: an amount applies separately to each damage event.
    let separate = p.event_scope.sources == Each && p.event_scope.recipients == Each;
    let prevent = match &*events.prevent {
        ast::PreventionQuantity::All(_) => kir::PreventionAmount::All,
        ast::PreventionQuantity::Amount(x)
            if separate && x.prevent == ast::PreventionQuantityAmountPrevent::UpTo =>
        {
            kir::PreventionAmount::UpTo {
                amount: fixed_positive_amount(&x.amount)?,
            }
        }
        ast::PreventionQuantity::Half(x) if separate => kir::PreventionAmount::Half {
            round_up: x.round == ast::EffectTurnFaceFace::Up,
        },
        _ => return Err(gap("prevent-damage:amount")),
    };
    let by = match p.by.as_deref() {
        None => None,
        Some(ast::DamagePreventionBy::ObjectRef(what)) => Some(&**what),
        Some(_) => return Err(gap("prevent-damage:source-choice")),
    };
    Ok(rule(
        FunctioningZone::Battlefield,
        StaticRule::PreventDamage {
            event: damage_event(p.kind, by, p.to.as_deref(), p.involving.as_deref())?,
            prevent,
        },
        None,
    ))
}

/// CR 614.1a, 120.8: "If [a source] would deal damage to [recipients], it
/// deals double/triple/that much plus N damage instead."
fn damage_replacement(r: &ast::ReplacementInstruction) -> Result<StaticAbility, Failure> {
    let ast::ReplacementEvent::Damage(event) = &*r.event else {
        return Err(gap("replacement"));
    };
    if r.occurrence != ast::EffectCreateDelayedTriggerOccurrence::Each
        || r.condition.is_some()
        || r.optional.is_some()
        || event.quantity.is_some()
    {
        return Err(gap("replacement:damage:qualifier"));
    }
    let ast::ReplacementInstructionReplace::ModifyQuantity(modify) = &*r.replace else {
        return Err(gap("replacement:damage:replace"));
    };
    use ast::ReplacementInstructionReplaceModifyQuantityChange as M;
    let change = match &*modify.change {
        M::Multiply(x) if x.factor >= 2 => kir::DamageModification::Multiply {
            factor: Quantity::from(x.factor),
        },
        M::Amount(x)
            if x.operation
                == ast::ReplacementInstructionReplaceModifyQuantityChangeAmountOperation::Add =>
        {
            kir::DamageModification::Add {
                amount: fixed_positive_amount(&x.amount)?,
            }
        }
        _ => return Err(gap("replacement:damage:change")),
    };
    Ok(rule(
        FunctioningZone::Battlefield,
        StaticRule::ModifyDamage {
            event: damage_event(
                event.kind,
                event.source.as_deref(),
                event.to.as_deref(),
                None,
            )?,
            change,
        },
        None,
    ))
}

fn one(count: &ast::Amount) -> bool {
    matches!(count, ast::Amount::Fixed(n) if n.value == 1)
}

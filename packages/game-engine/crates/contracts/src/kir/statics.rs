//! Live continuous effects and rule changes (CR 604, 611.3, 613).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scope {
    /// The source itself in the zone named by the rule.
    Source,
    Matching {
        filter: Pred,
    },
    /// The supported attachment modifiers retain their exact recipient
    /// relationship. General attached-to references are independently gated.
    Attachment {
        kind: AttachmentKind,
        filter: Pred,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Equipped,
    Enchanted,
}

/// CR 614.12: evaluated against the pre-entry battlefield for the incoming controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EntryCondition {
    UnlessControlled {
        filter: Pred,
        cmp: Cmp,
        count: Quantity,
    },
    UnlessOpponents {
        cmp: Cmp,
        count: Quantity,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StaticEffect {
    EntersTapped,
    PayLifeOrEnterTapped {
        amount: Quantity,
    },
    /// CR 614.12, 701.20a: the entering controller may reveal one card
    /// matching `filter` from their hand; otherwise it enters tapped.
    RevealOrEnterTapped {
        filter: Pred,
    },
    EntersTappedIf {
        condition: EntryCondition,
    },
    EntersWithPlusOneCounters {
        count: Quantity,
    },
    Continuous {
        affects: Scope,
        changes: Vec<Change>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<Cond>,
    },
    Rule {
        rule: StaticRule,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<Cond>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub enum StaticRule {
    Combat {
        affects: Scope,
        requirement: CombatRequirement,
    },
    CantBeCountered {
        affects: Scope,
    },
    SpellCost {
        affects: Scope,
        caster: PlayerExpr,
        change: SpellCostChange,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        multiplier: Option<Amount>,
    },
    MaximumHandSize {
        player: PlayerExpr,
        change: HandSizeChange,
    },
    AdditionalLandPlays {
        player: PlayerExpr,
        allowance: Allowance,
    },
    CastAsThoughFlash {
        affects: Scope,
        caster: PlayerExpr,
    },
    /// CR 509.1b, 613.11: a block restriction on the affected creatures,
    /// read from current characteristics (including keywords and power) of
    /// the affected creature and its counterpart at each declaration.
    Block {
        affects: Scope,
        restriction: BlockRestriction,
    },
    /// CR 508.1b-c: the affected creatures can't attack `defender`. The
    /// engine's only attackable objects are players (no planeswalker or
    /// battle kinds exist), so "or planeswalkers you control" adds nothing.
    AttackRestriction {
        affects: Scope,
        defender: PlayerExpr,
    },
}

/// CR 509.1b: per-pair and per-attacker block restrictions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "restriction", rename_all = "snake_case", deny_unknown_fields)]
pub enum BlockRestriction {
    /// The affected attacker can't be blocked by creatures matching
    /// `filter` or, with `only`, except by them.
    Blockers { only: bool, filter: Pred },
    /// The affected creature can't block attackers matching `filter` or,
    /// with `only`, can block only them.
    Attackers { only: bool, filter: Pred },
    /// A nonempty block of the affected attacker uses at most `count`
    /// creatures ("can't be blocked by more than one creature").
    AtMost { count: u32 },
    /// A nonempty block of the affected attacker uses at least `count`
    /// creatures ("can't be blocked except by three or more creatures").
    AtLeast { count: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CombatRequirement {
    CantAttack,
    CantBlock,
    CantAttackOrBlock,
    CantBeBlocked,
    CanBlockOnlyFlying,
    AttackEachCombat,
    /// CR 609.4, 702.3b: the creature can attack as though it didn't have
    /// defender; it lifts no other restriction.
    AttackAsThoughNoDefender,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HandSizeChange {
    Unlimited,
    Set { size: Quantity },
    Adjust { delta: Quantity },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Allowance {
    Unlimited,
    Additional { count: Quantity },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SpellCostChange {
    Increase { mana: crate::ManaCost },
    Reduce { mana: crate::ManaCost },
}

impl StaticEffect {
    pub fn rule(rule: StaticRule) -> Self {
        Self::Rule {
            rule,
            condition: None,
        }
    }

    pub fn continuous(affects: Scope, changes: Vec<Change>) -> Self {
        Self::Continuous {
            affects,
            changes,
            condition: None,
        }
    }
}

impl PlayerExpr {
    /// CR 109.5: the controller of a live static source.
    pub fn static_controller() -> Self {
        Self::Is {
            who: Ref::Controller {
                role: ControllerRole::StaticController,
            },
        }
    }
}

impl Scope {
    pub fn filter(&self) -> Option<&Pred> {
        match self {
            Self::Source => None,
            Self::Matching { filter } | Self::Attachment { filter, .. } => Some(filter),
        }
    }
}

impl StaticEffect {
    pub(crate) fn capability(&self, kind: CardKind) -> Capability {
        match self {
            Self::EntersTappedIf { .. } => Capability::ConditionalEntryTappedV1,
            Self::PayLifeOrEnterTapped { .. } => Capability::EntryLifePaymentV1,
            Self::RevealOrEnterTapped { .. } => Capability::EntryRevealV1,
            Self::EntersTapped => Capability::EntersTappedV1,
            Self::EntersWithPlusOneCounters { .. } => Capability::EntryPlusOneCountersV1,
            Self::Rule {
                rule:
                    StaticRule::CantBeCountered {
                        affects: Scope::Source,
                    },
                condition: None,
            } => Capability::SelfSpellProtectionV1,
            Self::Rule {
                rule:
                    StaticRule::Combat {
                        affects: Scope::Source,
                        requirement: CombatRequirement::CantBlock,
                    },
                condition: None,
            } if kind == CardKind::Creature => Capability::SacrificeV1,
            Self::Continuous { changes, .. }
                if changes
                    .iter()
                    .any(|c| matches!(c, Change::AddManaAbilities { .. })) =>
            {
                Capability::GrantedManaAbilitiesV1
            }
            Self::Continuous {
                affects: Scope::Matching { filter },
                changes,
                condition: None,
            } if matches!(kind, CardKind::Creature | CardKind::Enchantment)
                && *filter == controlled_creatures() =>
            {
                if matches!(changes.as_slice(), [Change::PowerToughness { power, toughness }]
                    if power == toughness && power.as_fixed().is_some_and(|p| *p > Quantity::zero()))
                {
                    Capability::CreatureAnthemV1
                } else if changes.iter().all(|change| match change {
                    Change::PowerToughness { power, toughness } => [power, toughness]
                        .iter()
                        .all(|n| n.as_fixed().is_some_and(|n| !n.is_negative())),
                    Change::AddKeywords { keywords } => keywords
                        .iter()
                        .all(|k| *k != KeywordAbility::Indestructible),
                    _ => false,
                }) {
                    Capability::AttachmentsV1
                } else {
                    Capability::ContinuousStaticsV1
                }
            }
            Self::Continuous { .. } => Capability::ContinuousStaticsV1,
            Self::Rule { .. } => Capability::RuleStaticsV1,
        }
    }
}

fn controlled_creatures() -> Pred {
    Pred(vec![
        Atom::Type {
            kind: CardKind::Creature,
        },
        Atom::ControlledBy {
            player: PlayerExpr::static_controller(),
        },
    ])
}

fn player_is_bound(player: &PlayerExpr) -> bool {
    *player == PlayerExpr::static_controller()
        || matches!(player, PlayerExpr::EachOpponent | PlayerExpr::EachPlayer)
}

fn predicate_in_layer(filter: &Pred, layer: Layer) -> bool {
    filter.0.iter().all(|atom| match atom {
        Atom::Type { .. } | Atom::NotType { .. } | Atom::AnyType { .. } => layer > Layer::Type,
        // CR 613.8a-c: resolved animation can change creature subtypes in
        // layer 4. A layer-4 static that reads one needs dependency sorting.
        Atom::Subtype { subtype } | Atom::NotSubtype { subtype } => {
            !subtype.is_empty() && (layer > Layer::Type || !is_creature_subtype(subtype))
        }
        Atom::Legendary
        | Atom::NotLegendary
        | Atom::Basic
        | Atom::NotBasic
        | Atom::Tapped
        | Atom::Untapped
        | Atom::Attacking
        | Atom::Blocking
        | Atom::Token
        | Atom::NotToken => true,
        Atom::Color { .. } | Atom::NotColor { .. } => layer > Layer::Color,
        Atom::ControlledBy { player } | Atom::OwnedBy { player } => player_is_bound(player),
        Atom::ManaValue { value, .. } => value.as_fixed().is_some(),
        Atom::IsNot { object } => *object == Ref::Source,
        Atom::AnyOf { alternatives } => {
            !alternatives.is_empty() && alternatives.iter().all(|p| predicate_in_layer(p, layer))
        }
        Atom::Power { .. }
        | Atom::Toughness { .. }
        | Atom::Keyword { .. }
        | Atom::NotKeyword { .. }
        | Atom::Is { .. } => false,
    })
}

fn condition_set(set: &SetExpr) -> bool {
    match set {
        SetExpr::Matching { zone, filter } => {
            (matches!(zone, ZoneExpr::Battlefield | ZoneExpr::Graveyards)
                || matches!(zone, ZoneExpr::Graveyard { of } if *of == PlayerExpr::static_controller()))
                && predicate_in_layer(filter, Layer::Color)
        }
        SetExpr::One { object } => *object == Ref::Source,
        SetExpr::Pool { .. } => false,
    }
}

fn condition_amount(amount: &Amount) -> bool {
    match amount {
        Amount::Fixed { .. } => true,
        Amount::Count { set } => condition_set(set),
        Amount::Devotion { player, colors } => {
            *player == PlayerExpr::static_controller()
                && !colors.is_empty()
                && colors.iter().all(|c| *c != Color::C)
        }
        _ => false,
    }
}

fn condition_supported(condition: &Cond) -> bool {
    match condition {
        Cond::Compare { left, right, .. } => condition_amount(left) && condition_amount(right),
        Cond::Exists { set } => condition_set(set),
        Cond::ActivePlayer { player } => player_is_bound(player),
        Cond::Not { condition } => condition_supported(condition),
        Cond::All { conditions } => {
            !conditions.is_empty() && conditions.iter().all(condition_supported)
        }
        Cond::CostPaid { .. }
        | Cond::Satisfies { .. }
        | Cond::CastBy { .. }
        | Cond::ControlsOwnCommander { .. }
        | Cond::OpponentControlsMore { .. } => false,
    }
}

/// CR 509.1b, 613.11: combat restrictions read the current object at the
/// declaration, so their filters may test keywords and fixed power,
/// toughness or mana value. They bind no other object than the source.
fn declaration_pred_valid(filter: &Pred) -> bool {
    filter.0.iter().all(|atom| match atom {
        Atom::Type { .. }
        | Atom::NotType { .. }
        | Atom::Legendary
        | Atom::NotLegendary
        | Atom::Basic
        | Atom::NotBasic
        | Atom::Color { .. }
        | Atom::NotColor { .. }
        | Atom::Tapped
        | Atom::Untapped
        | Atom::Attacking
        | Atom::Blocking
        | Atom::Token
        | Atom::NotToken
        | Atom::Keyword { .. }
        | Atom::NotKeyword { .. } => true,
        Atom::AnyType { kinds } => !kinds.is_empty(),
        Atom::Subtype { subtype } | Atom::NotSubtype { subtype } => !subtype.is_empty(),
        Atom::ControlledBy { player } | Atom::OwnedBy { player } => player_is_bound(player),
        Atom::Power { value, .. }
        | Atom::Toughness { value, .. }
        | Atom::ManaValue { value, .. } => value.as_fixed().is_some(),
        Atom::IsNot { object } => *object == Ref::Source,
        Atom::AnyOf { alternatives } => {
            !alternatives.is_empty() && alternatives.iter().all(declaration_pred_valid)
        }
        Atom::Is { .. } => false,
    })
}

fn declaration_scope_valid(scope: &Scope, d: &crate::Definition, kind: CardKind) -> bool {
    let bound = match scope {
        Scope::Source => true,
        Scope::Matching { .. } => true,
        Scope::Attachment { kind, .. } => d.characteristics.subtypes.iter().any(|s| {
            s == match kind {
                AttachmentKind::Equipped => "Equipment",
                AttachmentKind::Enchanted => "Aura",
            }
        }),
    };
    bound && scope.filter().is_none_or(declaration_pred_valid) && creature_scope(scope, kind)
}

fn scope_valid(scope: &Scope, d: &crate::Definition, layer: Layer) -> bool {
    let bound = match scope {
        Scope::Source | Scope::Matching { .. } => true,
        Scope::Attachment { kind, .. } => d.characteristics.subtypes.iter().any(|s| {
            s == match kind {
                AttachmentKind::Equipped => "Equipment",
                AttachmentKind::Enchanted => "Aura",
            }
        }),
    };
    bound && scope.filter().is_none_or(|p| predicate_in_layer(p, layer))
}

// CR 613.8: only the Creature term can depend on a layer-4 change.
// The interpreter settles that term against resolving animation first.
fn only_stable_creature_filter(filter: &Pred) -> bool {
    filter.0.iter().all(|atom| {
        matches!(
            atom,
            Atom::Type {
                kind: CardKind::Creature
            }
        ) || (predicate_in_layer(&Pred(vec![atom.clone()]), Layer::Type)
            && !matches!(
                atom,
                Atom::Subtype { .. } | Atom::NotSubtype { .. } | Atom::AnyOf { .. }
            ))
    })
}

fn creature_scope(scope: &Scope, kind: CardKind) -> bool {
    match scope {
        Scope::Source => kind == CardKind::Creature,
        Scope::Matching { filter } | Scope::Attachment { filter, .. } => {
            filter.0.iter().any(|atom| match atom {
                Atom::Type { kind } => *kind == CardKind::Creature,
                // CR 205.3d,m: admitted definitions have no kindred card type.
                Atom::Subtype { subtype } => is_creature_subtype(subtype),
                _ => false,
            })
        }
    }
}

/// CR 113.10a, 605.1a, 613.1f: the admitted quoted mana abilities. A
/// granted ability belongs to each recipient, which pays its tap cost and
/// optional life (CR 118.3, 119.4) and adds the mana itself; "you" is the
/// player who activates it (CR 109.5). The activation must keep its
/// ability while the activation is open, so no admitted cost or condition
/// can change which objects have it: tapping cannot change a recipient
/// filter without tapped status and no condition guards the grant or the
/// granted ability. Riders, restrictions, mana costs and sacrifice need their
/// own admission rows.
pub fn granted_mana_supported(
    affects: &Scope,
    condition: Option<&Cond>,
    abilities: &[crate::ManaAbility],
) -> bool {
    fn untap_status(filter: &Pred) -> bool {
        filter.0.iter().any(|atom| match atom {
            Atom::Tapped | Atom::Untapped => true,
            Atom::AnyOf { alternatives } => alternatives.iter().any(untap_status),
            _ => false,
        })
    }
    let stable = affects.filter().is_none_or(|filter| !untap_status(filter));
    stable
        && condition.is_none()
        && !abilities.is_empty()
        && abilities.iter().all(|a| {
            a.costs.tap
                && !a.costs.sacrifice
                && a.costs.mana.is_none()
                && a.costs.remove_source_plus_one_counters.is_none()
                && a.costs
                    .pay_life
                    .as_ref()
                    .is_none_or(|life| *life >= Quantity::from(1))
                && a.restriction.is_none()
                && a.condition.is_none()
                && a.riders.is_empty()
                && super::tiers::mana_production_supported(&a.production)
        })
}

impl crate::StaticAbility {
    pub(crate) fn validate_static(&self, definition: &crate::Definition) -> Result<(), String> {
        use crate::FunctioningZone::{Battlefield, Stack};
        let kind = definition.characteristics.kind;
        let permanent = !matches!(kind, CardKind::Instant | CardKind::Sorcery);
        let on_battlefield = permanent && self.functioning_zone == Battlefield;
        let invalid = |reason| Err(format!("cell:static:{reason}"));
        match &self.effect {
            StaticEffect::EntersTapped if on_battlefield => Ok(()),
            StaticEffect::EntersTapped => invalid("enters-tapped"),
            StaticEffect::PayLifeOrEnterTapped { amount } => {
                if on_battlefield && kind == CardKind::Land && *amount > Quantity::zero() {
                    Ok(())
                } else {
                    invalid("entry-life-payment")
                }
            }
            StaticEffect::RevealOrEnterTapped { filter } => {
                if on_battlefield
                    && kind == CardKind::Land
                    && !filter.0.is_empty()
                    && super::check::hidden_card_filter(filter)
                {
                    Ok(())
                } else {
                    invalid("entry-reveal")
                }
            }
            StaticEffect::EntersTappedIf { condition } => {
                let supported = match condition {
                    EntryCondition::UnlessControlled { filter, count, .. } => {
                        !count.is_negative() && predicate_in_layer(filter, Layer::Color)
                    }
                    EntryCondition::UnlessOpponents { count, .. } => !count.is_negative(),
                };
                if on_battlefield && supported {
                    Ok(())
                } else {
                    invalid("conditional-entry-tapped")
                }
            }
            StaticEffect::EntersWithPlusOneCounters { count } => {
                if on_battlefield && *count > Quantity::zero() {
                    Ok(())
                } else {
                    invalid("entry-plus-one-counters")
                }
            }
            StaticEffect::Continuous {
                affects,
                changes,
                condition,
            } => {
                let Some(first_layer) = changes.iter().map(Change::layer).min() else {
                    return invalid("empty-continuous");
                };
                if !on_battlefield
                    || !scope_valid(
                        affects,
                        definition,
                        if changes
                            .iter()
                            .any(|change| matches!(change, Change::AddCreatureSubtypes { .. }))
                            && affects.filter().is_some_and(only_stable_creature_filter)
                        {
                            Layer::Color
                        } else {
                            first_layer
                        },
                    )
                    || condition
                        .as_ref()
                        .is_some_and(|c| first_layer <= Layer::Type || !condition_supported(c))
                {
                    return invalid("continuous-scope");
                }
                for change in changes {
                    let supported = match change {
                        Change::PowerToughness { power, toughness }
                        | Change::SetPowerToughness { power, toughness } => {
                            creature_scope(affects, kind)
                                && power.as_fixed().is_some()
                                && toughness.as_fixed().is_some()
                        }
                        Change::AddKeywords { keywords } => {
                            !keywords.is_empty()
                                && keywords.iter().all(|k| {
                                    match k {
                                        // CR 702.11, 702.12, 702.18: these work on
                                        // noncreature permanents as well.
                                        KeywordAbility::Hexproof
                                        | KeywordAbility::Indestructible
                                        | KeywordAbility::Shroud => true,
                                        KeywordAbility::Flash
                                        | KeywordAbility::ProtectionFromEverything
                                        | KeywordAbility::ProtectionFromEachColor => false,
                                        _ => creature_scope(affects, kind),
                                    }
                                })
                        }
                        Change::AddManaAbilities { abilities } => {
                            granted_mana_supported(affects, condition.as_ref(), abilities)
                        }
                        Change::RemoveKeywords { .. }
                        | Change::Controller { .. }
                        | Change::PowerToughnessX { .. } => false,
                        Change::SetColors { colors } => {
                            colors.iter().all(|color| *color != Color::C)
                        }
                        Change::AddCreatureSubtypes { subtypes } => {
                            creature_scope(affects, kind)
                                && affects.filter().is_none_or(only_stable_creature_filter)
                                && !subtypes.is_empty()
                                && subtypes.iter().all(|s| is_creature_subtype(s))
                                && changes.iter().filter(|c| c.layer() == Layer::Type).count() == 1
                        }
                        Change::AddTypes { kinds } => {
                            !kinds.is_empty()
                                && kinds.iter().all(|k| {
                                    matches!(k, CardKind::Artifact | CardKind::Enchantment)
                                })
                        }
                        // CR 205.3d, 205.3m: every creature type counts only
                        // while the recipient is a creature, so the change
                        // needs no type filter and its scope reads nothing a
                        // layer-4 effect changes (CR 613.8a). Scopes reaching
                        // other zones are not admitted.
                        Change::AllCreatureTypes => {
                            changes.len() == 1
                                && match affects {
                                    Scope::Source => kind == CardKind::Creature,
                                    Scope::Attachment { filter, .. } => filter.0.is_empty(),
                                    Scope::Matching { .. } => false,
                                }
                        }
                    };
                    if !supported {
                        return invalid("continuous-change");
                    }
                }
                Ok(())
            }
            StaticEffect::Rule { rule, condition } => {
                if condition.as_ref().is_some_and(|c| !condition_supported(c)) {
                    return invalid("rule-condition");
                }
                let on_stack = kind != CardKind::Land && self.functioning_zone == Stack;
                let spell_scope = |s: &Scope| {
                    !matches!(s, Scope::Attachment { .. })
                        && scope_valid(s, definition, Layer::Ability)
                };
                let supported = match rule {
                    StaticRule::Combat { affects, .. } => {
                        on_battlefield
                            && scope_valid(affects, definition, Layer::Ability)
                            && creature_scope(affects, kind)
                    }
                    StaticRule::CantBeCountered { affects } => {
                        spell_scope(affects)
                            && match affects {
                                Scope::Source => on_stack,
                                _ => on_battlefield,
                            }
                    }
                    StaticRule::SpellCost {
                        affects,
                        caster,
                        change,
                        multiplier,
                    } => {
                        let (SpellCostChange::Increase { mana } | SpellCostChange::Reduce { mana }) =
                            change;
                        spell_scope(affects)
                            && player_is_bound(caster)
                            && !mana.generic.is_negative()
                            && multiplier.as_ref().is_none_or(|m| {
                                mana.colored.is_empty()
                                    && condition_amount(m)
                                    && !m.as_fixed().is_some_and(Quantity::is_negative)
                            })
                            && match affects {
                                Scope::Source => on_stack,
                                _ => on_battlefield,
                            }
                    }
                    StaticRule::MaximumHandSize { player, change } => {
                        on_battlefield
                            && player_is_bound(player)
                            && !matches!(change, HandSizeChange::Set { size } if size.is_negative())
                    }
                    StaticRule::AdditionalLandPlays { player, allowance } => {
                        on_battlefield
                            && player_is_bound(player)
                            && !matches!(allowance, Allowance::Additional { count } if *count <= Quantity::zero())
                    }
                    StaticRule::CastAsThoughFlash { affects, caster } => {
                        on_battlefield
                            && matches!(affects, Scope::Matching { .. })
                            && spell_scope(affects)
                            && player_is_bound(caster)
                    }
                    StaticRule::Block {
                        affects,
                        restriction,
                    } => {
                        on_battlefield
                            && declaration_scope_valid(affects, definition, kind)
                            && match restriction {
                                BlockRestriction::Blockers { filter, .. }
                                | BlockRestriction::Attackers { filter, .. } => {
                                    declaration_pred_valid(filter)
                                }
                                BlockRestriction::AtMost { count } => *count >= 1,
                                BlockRestriction::AtLeast { count } => *count >= 2,
                            }
                    }
                    StaticRule::AttackRestriction { affects, defender } => {
                        on_battlefield
                            && declaration_scope_valid(affects, definition, kind)
                            && *defender == PlayerExpr::static_controller()
                    }
                };
                if supported {
                    Ok(())
                } else {
                    invalid("rule-scope")
                }
            }
        }
    }
}

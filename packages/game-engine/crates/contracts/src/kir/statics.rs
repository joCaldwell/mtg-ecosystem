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

/// CR 614.1c, 614.12: a public value chosen by the entering controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "choose", rename_all = "snake_case", deny_unknown_fields)]
pub enum EntryChoiceSpec {
    Color,
    CreatureType,
    Player { opponent: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StaticEffect {
    ChooseOnEntry {
        choice: EntryChoiceSpec,
    },
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
    /// CR 122.6, 614.1c: the placement is part of the modified entry.
    EntersWithCounters {
        kind: CounterKind,
        count: Quantity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        per: Option<SetExpr>,
    },
    /// CR 107.3m, 122.6: X comes from the resolving permanent spell.
    EntersWithAnnouncedXCounters {
        kind: CounterKind,
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
    /// CR 604.3, 613.4a, 208.2a: a characteristic-defining ability that
    /// sets the source's power and/or toughness in layer 7a. It functions
    /// in every zone (`FunctioningZone::Any`) and is read whenever the value
    /// is looked at, never locked in. An absent stat keeps its printed value.
    DefinePowerToughness {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        power: Option<Amount>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toughness: Option<Amount>,
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
    /// CR 508.1b-c: the affected creatures can't attack `defender` or,
    /// when printed, planeswalkers that player controls.
    AttackRestriction {
        #[serde(default, skip_serializing_if = "crate::is_false")]
        planeswalkers: bool,
        affects: Scope,
        defender: PlayerExpr,
    },
    /// CR 615.1, 615.10: a static prevention effect applied to each matching
    /// damage event once, under the affected recipient's choice (CR 616.1).
    PreventDamage {
        event: DamageEvent,
        prevent: PreventionAmount,
    },
    /// CR 614.1a, 120.8: "deals double/that much plus N damage instead" for
    /// each matching nonzero damage event, ordered with prevention under
    /// CR 616.1.
    ModifyDamage {
        event: DamageEvent,
        change: DamageModification,
    },
    /// CR 614.1a, 614.16: "If [an effect] would create one or more tokens
    /// / put one or more counters ..., [it creates/puts] [changed] that many
    /// instead." Each instance applies once to each event (CR 614.5); the
    /// affected player or controller orders several (CR 616.1).
    ModifyQuantity {
        event: QuantityEvent,
        change: crate::QuantityModification,
    },
    /// CR 615.12: matching damage can't be prevented. No admitted
    /// prevention effect has an additional effect, so none applies and no
    /// prevention shield is reduced.
    DamageCantBePrevented {
        event: DamageEvent,
    },
}

/// CR 614.16: whether a quantity replacement watches only events a resolving
/// spell or ability, or another replacement, produces, or any event
/// including costs and rules actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementOrigin {
    Effect,
    Any,
}

/// CR 122.1: the permanents whose counters a quantity replacement watches,
/// always among those the static's controller controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CounterRecipients {
    Permanents,
    Creatures,
}

/// The event a quantity replacement watches (CR 111.1, 122.6, 614.1a).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuantityEvent {
    /// Tokens created under the static's controller's control.
    CreateTokens { origin: ReplacementOrigin },
    /// Counters put on a controlled permanent: one kind, or any kind when
    /// `counter` is absent. An entering permanent's counters count (CR 122.6).
    PutCounters {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<CounterKind>,
        recipients: CounterRecipients,
        origin: ReplacementOrigin,
    },
}

/// CR 120.2, 510.2: the damage a damage rule or prevention effect watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DamageKindFilter {
    All,
    Combat,
    Noncombat,
}

/// CR 609.7a, 615.1, 614.1a: the damage events a static damage rule watches,
/// read when the damage would be dealt. `by` matches the damage source's
/// damage-time facts, `to` the recipient and `involving` either ("dealt to
/// and dealt by"). Absent parts match every source or recipient.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DamageEvent {
    pub kind: DamageKindFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<Scope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<DamageRecipients>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub involving: Option<Scope>,
}

/// CR 120.1: the recipients a damage rule names. A player matches
/// `players`; a permanent (only creatures are dealt damage here) matches
/// `permanents`. At least one is present.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DamageRecipients {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub players: Option<PlayerExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permanents: Option<Scope>,
}

/// CR 615.1, 615.10: how much of one damage event a prevention prevents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "prevent", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreventionAmount {
    All,
    UpTo { amount: Quantity },
    Half { round_up: bool },
}

/// CR 614.1a: the replaced damage amount.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "change", rename_all = "snake_case", deny_unknown_fields)]
pub enum DamageModification {
    Multiply { factor: Quantity },
    Add { amount: Quantity },
}

/// CR 615.1, 615.7, 611.2a: a prevention effect a resolving spell or
/// ability creates. `to` names an announced recipient and `by` the damage
/// source; absent parts match every recipient or source. `Next` is the
/// CR 615.7 shield for exactly one recipient.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPrevention {
    pub kind: DamageKindFilter,
    pub shield: PreventionShield,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<PreventionSources>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shield", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreventionShield {
    /// Prevent all matching damage until the effect expires.
    All,
    /// CR 615.7: prevent the next `amount` damage.
    Next { amount: Quantity },
}

/// CR 609.7a, 611.2c: the sources whose damage a resolved prevention
/// prevents. `One` is an announced object, bound at resolution; `Matching`
/// is read from the damage source when the damage would be dealt, since
/// prevention modifies no characteristics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "sources", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreventionSources {
    One { object: Ref },
    Matching { filter: Pred },
}

/// CR 609.7a, 120.1: a damage source's damage-time facts carry its
/// characteristics, keywords, controller, owner, tapped and combat status.
/// Filters read nothing else.
pub fn damage_source_pred_valid(filter: &Pred) -> bool {
    declaration_pred_valid(filter)
}

fn damage_scope_valid(scope: &Scope, d: &crate::Definition) -> bool {
    let bound = match scope {
        Scope::Source | Scope::Matching { .. } => true,
        Scope::Attachment { kind, .. } => d.characteristics.subtypes.iter().any(|s| {
            s == match kind {
                AttachmentKind::Equipped => "Equipment",
                AttachmentKind::Enchanted => "Aura",
            }
        }),
    };
    bound && scope.filter().is_none_or(declaration_pred_valid)
}

fn damage_event_valid(event: &DamageEvent, d: &crate::Definition) -> bool {
    let recipients = event.to.as_ref().is_none_or(|to| {
        (to.players.is_some() || to.permanents.is_some())
            && to.players.as_ref().is_none_or(player_is_bound)
            && to
                .permanents
                .as_ref()
                .is_none_or(|scope| damage_scope_valid(scope, d))
    });
    let source_scope = |scope: &Option<Scope>| {
        scope
            .as_ref()
            .is_none_or(|scope| damage_scope_valid(scope, d))
    };
    recipients
        && source_scope(&event.by)
        && source_scope(&event.involving)
        && (event.involving.is_none() || (event.by.is_none() && event.to.is_none()))
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
    AssignCombatDamageWithToughness,
    AdditionalBlocks(u32),
    UnlimitedBlocks,
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
            Self::ChooseOnEntry { .. } => Capability::EntryChoiceV1,
            Self::EntersWithCounters { .. } | Self::EntersWithAnnouncedXCounters { .. } => {
                Capability::EntryCountersV1
            }
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
            Self::Rule {
                rule: StaticRule::PreventDamage { .. },
                ..
            } => Capability::DamagePreventionV1,
            Self::Rule {
                rule: StaticRule::ModifyDamage { .. },
                ..
            } => Capability::StaticDamageReplacementV1,
            Self::Rule {
                rule: StaticRule::DamageCantBePrevented { .. },
                ..
            } => Capability::UnpreventableDamageV1,
            Self::Rule {
                rule: StaticRule::ModifyQuantity { .. },
                ..
            } => Capability::QuantityReplacementV1,
            Self::Rule { .. } => Capability::RuleStaticsV1,
            Self::DefinePowerToughness { .. } => Capability::CharacteristicPowerToughnessV1,
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
        Atom::SourceChosenColor {} | Atom::SourceChosenCreatureType {} => layer > Layer::Ability,
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
        | Atom::Is { .. }
        | Atom::Counters { .. }
        | Atom::ControllerTookSourceCombatDamage {} => false,
    })
}

fn condition_set(set: &SetExpr) -> bool {
    match set {
        SetExpr::Matching { zone, filter } => {
            ((matches!(zone, ZoneExpr::Battlefield | ZoneExpr::Graveyards)
                || matches!(zone, ZoneExpr::Graveyard { of } if *of == PlayerExpr::static_controller()))
                && predicate_in_layer(filter, Layer::Color))
                // CR 402.3: a hand's size is public, its cards are not.
                || matches!(zone, ZoneExpr::Hand { of } if *of == PlayerExpr::static_controller())
                    && filter.0.is_empty()
        }
        SetExpr::One { object } => *object == Ref::Source,
        SetExpr::Pool { .. }
        | SetExpr::Targets { .. }
        | SetExpr::Result { .. }
        | SetExpr::LiveResult { .. }
        | SetExpr::Revealed { .. } => false,
    }
}

/// CR 400.7, 614.12: an entrant counted from a graveyard is still its
/// former card there, a different object from the prospective permanent, so
/// "other" cannot exclude it. No printed entry count says "other ... card in
/// a graveyard"; such a count is rejected rather than counting the entrant.
fn graveyard_other(set: &SetExpr) -> bool {
    matches!(
        set,
        SetExpr::Matching { zone: ZoneExpr::Graveyards | ZoneExpr::Graveyard { .. }, filter }
            if filter.0.iter().any(|atom| matches!(atom, Atom::IsNot { .. }))
    )
}

fn condition_amount(amount: &Amount) -> bool {
    match amount {
        Amount::Fixed { .. } => true,
        // CR 122.1: counters of one kind on the source are public. A count of
        // all kinds is rejected first by `counts_all_counter_kinds`.
        Amount::CountersOn {
            object: Ref::Source,
            counter,
        } => counter.as_ref().is_some_and(|c| *c != CounterKind::Loyalty),
        Amount::Count { set } => condition_set(set),
        Amount::Devotion { player, colors } => {
            *player == PlayerExpr::static_controller()
                && !colors.is_empty()
                && colors.iter().all(|c| *c != Color::C)
        }
        _ => false,
    }
}

/// CR 604.3, 613.4a: a characteristic-defining count reads layers 1-6 of
/// public objects: their types, subtypes, supertypes, colors, control,
/// ownership and status, never power, toughness or abilities (which would
/// make one CDA depend on another's layer-7 result, CR 613.8).
fn cda_pred(filter: &Pred, battlefield: bool) -> bool {
    filter.0.iter().all(|atom| match atom {
        Atom::Type { .. }
        | Atom::NotType { .. }
        | Atom::Legendary
        | Atom::NotLegendary
        | Atom::Basic
        | Atom::NotBasic
        | Atom::Color { .. }
        | Atom::NotColor { .. } => true,
        Atom::AnyType { kinds } => !kinds.is_empty(),
        Atom::Subtype { subtype } | Atom::NotSubtype { subtype } => !subtype.is_empty(),
        Atom::ControlledBy { player } => battlefield && player_is_bound(player),
        Atom::OwnedBy { player } => player_is_bound(player),
        Atom::Tapped | Atom::Untapped | Atom::Token | Atom::NotToken => battlefield,
        Atom::ManaValue { value, .. } => value.as_fixed().is_some(),
        Atom::IsNot { object } => *object == Ref::Source,
        Atom::AnyOf { alternatives } => {
            !alternatives.is_empty() && alternatives.iter().all(|p| cda_pred(p, battlefield))
        }
        Atom::Attacking
        | Atom::Blocking
        | Atom::Power { .. }
        | Atom::Toughness { .. }
        | Atom::Keyword { .. }
        | Atom::NotKeyword { .. }
        | Atom::Is { .. }
        | Atom::Counters { .. }
        | Atom::SourceChosenColor {}
        | Atom::SourceChosenCreatureType {}
        | Atom::ControllerTookSourceCombatDamage {} => false,
    })
}

/// CR 400.2, 404.2: battlefield permanents, graveyard cards and (unfiltered,
/// since a hand's size is public but its cards are not) hand sizes.
fn cda_set(set: &SetExpr) -> bool {
    let SetExpr::Matching { zone, filter } = set else {
        return false;
    };
    match zone {
        ZoneExpr::Battlefield => cda_pred(filter, true),
        ZoneExpr::Graveyards => cda_pred(filter, false),
        ZoneExpr::Graveyard { of } => player_is_bound(of) && cda_pred(filter, false),
        ZoneExpr::Hand { of } => player_is_bound(of) && filter.0.is_empty(),
        ZoneExpr::Library { .. } | ZoneExpr::Exile | ZoneExpr::Stack => false,
    }
}

/// The CR 208.2a values a characteristic-defining ability can define:
/// nonnegative fixed terms, counts, multiples and sums of those, the
/// greatest mana value among permanents or graveyard cards, devotion (CR 700.5) and the number
/// of distinct card types, basic land types or colors among a set.
fn cda_amount(amount: &Amount) -> bool {
    match amount {
        Amount::Fixed { value } => !value.is_negative(),
        Amount::Count { set } => cda_set(set),
        Amount::Times { factor, of } => *factor > Quantity::zero() && cda_amount(of),
        Amount::Sum { terms } => terms.len() >= 2 && terms.iter().all(cda_amount),
        Amount::Greatest {
            set: set @ SetExpr::Matching { zone, .. },
            stat: Stat::ManaValue,
        } => !zone.hidden() && cda_set(set),
        Amount::Devotion { player, colors } => {
            *player == PlayerExpr::static_controller()
                && !colors.is_empty()
                && colors.iter().all(|c| *c != Color::C)
        }
        Amount::Distinct { set, .. } => {
            matches!(set, SetExpr::Matching { zone, .. } if !zone.hidden()) && cda_set(set)
        }
        Amount::CountChosenCreatureType { .. }
        | Amount::Var { .. }
        | Amount::StatOf { .. }
        | Amount::CountersOn { .. }
        | Amount::Total { .. }
        | Amount::Greatest { .. }
        | Amount::LifeOf { .. }
        | Amount::BlockersOf { .. }
        | Amount::OpponentsAttacked {}
        | Amount::Difference { .. }
        | Amount::Half { .. }
        | Amount::OtherCastsManaValue { .. } => false,
    }
}

/// CR 601.2f: a cost change's count is read once, as the total cost is
/// determined, outside the layer system; besides the static counts it may
/// read the greatest or total power or toughness of public battlefield
/// permanents by their current characteristics.
pub(crate) fn cost_amount(amount: &Amount) -> bool {
    match amount {
        Amount::Greatest {
            set:
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    filter,
                },
            stat: Stat::Power | Stat::Toughness,
        }
        | Amount::Total {
            set:
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    filter,
                },
            stat: Stat::Power | Stat::Toughness,
        } => predicate_in_layer(filter, Layer::Color),
        other => condition_amount(other),
    }
}

/// CR 601.2f, 602.2b: an ability's own cost change reads its multiplier and
/// condition from its source as a static ability does; a counted change
/// multiplies generic mana only.
pub fn activation_cost_change_supported(change: &crate::ActivationCostChange) -> bool {
    let (SpellCostChange::Increase { mana } | SpellCostChange::Reduce { mana }) = &change.change;
    !mana.generic.is_negative()
        && (mana.generic > Quantity::zero() || !mana.colored.is_empty())
        && !mana.colored.contains(&Color::C)
        && change.multiplier.as_ref().is_none_or(|m| {
            mana.colored.is_empty()
                && cost_amount(m)
                && !m.as_fixed().is_some_and(Quantity::is_negative)
        })
        && change.condition.as_ref().is_none_or(condition_supported)
}

/// CR 122.1: a cost change's multiplier or a static condition that counts
/// counters of all kinds on its source is rejected with its own reason until
/// that count is reviewed; one named kind is read as before.
pub(crate) fn counts_all_counter_kinds(
    multiplier: Option<&Amount>,
    condition: Option<&Cond>,
) -> bool {
    fn amount(a: &Amount) -> bool {
        matches!(a, Amount::CountersOn { counter: None, .. })
    }
    fn cond(c: &Cond) -> bool {
        match c {
            Cond::Compare { left, right, .. } => amount(left) || amount(right),
            Cond::Not { condition } => cond(condition),
            Cond::All { conditions } => conditions.iter().any(cond),
            _ => false,
        }
    }
    multiplier.is_some_and(amount) || condition.is_some_and(cond)
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
        | Cond::TurnHistory { .. }
        | Cond::ControlsOwnCommander { .. }
        | Cond::OpponentControlsMore { .. }
        | Cond::EventOpponentControlsMore { .. }
        | Cond::ControllerCreatureTotalPower { .. }
        | Cond::Measured { .. }
        | Cond::ControlledSinceLastUpkeep {} => false,
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
        Atom::Is { .. }
        | Atom::Counters { .. }
        | Atom::SourceChosenColor {}
        | Atom::SourceChosenCreatureType {}
        | Atom::ControllerTookSourceCombatDamage {} => false,
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
    bound
        && scope
            .filter()
            .is_none_or(|p| predicate_in_layer(p, layer) && choice_predicate_linked(p, d))
}

fn choice_predicate_linked(pred: &Pred, definition: &crate::Definition) -> bool {
    pred.0.iter().all(|atom| {
        let spec = match atom {
            Atom::SourceChosenColor {} => EntryChoiceSpec::Color,
            Atom::SourceChosenCreatureType {} => EntryChoiceSpec::CreatureType,
            Atom::AnyOf { alternatives } => {
                return alternatives
                    .iter()
                    .all(|p| choice_predicate_linked(p, definition));
            }
            _ => return true,
        };
        definition.static_abilities.iter().any(|ability| {
            matches!(ability.effect, StaticEffect::ChooseOnEntry { choice } if choice == spec)
        })
    })
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

/// CR 205.1a, 305.7: a layer-4 change that replaces or removes types.
fn type_replacement(change: &Change) -> bool {
    matches!(
        change,
        Change::SetCardTypes { .. }
            | Change::RemoveCardTypes { .. }
            | Change::SetCreatureSubtypes { .. }
            | Change::SetLandSubtypes { .. }
    )
}

/// CR 113.10, 613.1f: a layer-6 change that removes or forbids abilities.
fn ability_removal(change: &Change) -> bool {
    matches!(
        change,
        Change::RemoveAllAbilities { .. }
            | Change::RemoveKeywords { .. }
            | Change::ForbidKeywords { .. }
    )
}

fn distinct<T: PartialEq>(items: &[T]) -> bool {
    !items.is_empty()
        && items
            .iter()
            .enumerate()
            .all(|(i, item)| !items[..i].contains(item))
}

/// CR 205.1a: the permanent card types an effect can set, in canonical
/// order. Instants and sorceries keep their type and are not admitted.
fn replacement_kinds(kinds: &[CardKind]) -> bool {
    distinct(kinds)
        && kinds.windows(2).all(|k| k[0] < k[1])
        && kinds.iter().all(|k| {
            matches!(
                k,
                CardKind::Land | CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
            )
        })
}

/// CR 113.10b: removable keywords. Flash never functions on the battlefield
/// and printed protection from everything is read from the card.
fn removable_keywords(keywords: &[KeywordAbility]) -> bool {
    distinct(keywords)
        && keywords.iter().all(|k| {
            !matches!(
                k,
                KeywordAbility::Flash
                    | KeywordAbility::ProtectionFromEverything
                    | KeywordAbility::ProtectionFromEachColor
                    | KeywordAbility::Flanking
            )
        })
}

fn filter_requires(scope: &Scope, kind: CardKind) -> bool {
    scope
        .filter()
        .is_some_and(|f| f.0.contains(&Atom::Type { kind }))
}

/// CR 613.8: conditions on a layer-4 change read nothing any layer changes.
fn layer_free_condition(condition: &Cond) -> bool {
    match condition {
        Cond::Compare { left, right, .. } => [left, right]
            .iter()
            .all(|a| matches!(a, Amount::Fixed { .. } | Amount::Devotion { .. })),
        Cond::ActivePlayer { player } => player_is_bound(player),
        Cond::Not { condition } => layer_free_condition(condition),
        Cond::All { conditions } => conditions.iter().all(layer_free_condition),
        _ => false,
    }
}

/// Scopes for type replacement and ability removal. The interpreter orders
/// layer-4 effects by dependency (CR 613.8), so a filter may read card types
/// and the noun of an Aura or Equipment; a filtered scope reaching other
/// permanents reads only what entering permanents show before any layer
/// (CR 614.12): card types, supertypes, token status and control.
fn replacement_scope_valid(scope: &Scope, d: &crate::Definition, changes: &[Change]) -> bool {
    fn entry_stable(filter: &Pred) -> bool {
        filter.0.iter().all(|atom| match atom {
            Atom::Type { .. }
            | Atom::NotType { .. }
            | Atom::Legendary
            | Atom::NotLegendary
            | Atom::Basic
            | Atom::NotBasic
            | Atom::Token
            | Atom::NotToken => true,
            Atom::ControlledBy { player } => player_is_bound(player),
            Atom::IsNot { object } => *object == Ref::Source,
            _ => false,
        })
    }
    match scope {
        Scope::Source => !changes.iter().any(ability_removal),
        Scope::Attachment { kind, filter } => {
            d.characteristics.subtypes.iter().any(|s| {
                s == match kind {
                    AttachmentKind::Equipped => "Equipment",
                    AttachmentKind::Enchanted => "Aura",
                }
            }) && filter.0.len() <= 1
                && filter.0.iter().all(|a| matches!(a, Atom::Type { .. }))
        }
        // CR 305.7: Blood Moon's "nonbasic lands"; a removal reads layer 5.
        // An entering permanent's controller is not yet bound (CR 614.12),
        // so a scope that removes its abilities reads no control.
        Scope::Matching { filter } => {
            entry_stable(filter)
                && (!changes.iter().any(|c| {
                    matches!(
                        c,
                        Change::RemoveAllAbilities { .. } | Change::SetLandSubtypes { .. }
                    )
                }) || !filter
                    .0
                    .iter()
                    .any(|a| matches!(a, Atom::ControlledBy { .. })))
                && changes.iter().all(|c| {
                    c.layer() != Layer::Type
                        || (matches!(c, Change::SetLandSubtypes { .. })
                            && filter.0.iter().all(|a| {
                                matches!(
                                    a,
                                    Atom::Type {
                                        kind: CardKind::Land
                                    } | Atom::Basic
                                        | Atom::NotBasic
                                        | Atom::ControlledBy { .. }
                                )
                            }))
                })
        }
    }
}

fn creature_scope(scope: &Scope, kind: CardKind) -> bool {
    match scope {
        Scope::Source => kind == CardKind::Creature,
        Scope::Matching { filter } | Scope::Attachment { filter, .. } => only_creatures(filter),
    }
}

/// Whether every object `filter` admits is a creature: it names the
/// creature type or a creature subtype, or each alternative of a printed
/// union does ("Skeletons you control and other Zombies you control").
fn only_creatures(filter: &Pred) -> bool {
    filter.0.iter().any(|atom| match atom {
        Atom::Type { kind } => *kind == CardKind::Creature,
        // CR 205.3d,m: admitted definitions have no kindred card type.
        Atom::Subtype { subtype } => is_creature_subtype(subtype),
        Atom::AnyOf { alternatives } => {
            !alternatives.is_empty() && alternatives.iter().all(only_creatures)
        }
        _ => false,
    })
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
                // CR 107.4e: hybrid symbols are mana costs too.
                && a.hybrid_mana.is_empty()
                && a.costs.remove_source_counters.is_none()
                && a.costs
                    .pay_life
                    .as_ref()
                    .is_none_or(|life| *life >= Quantity::from(1))
                && a.restriction.is_none()
                && a.condition.is_none()
                && a.riders.is_empty()
                && a.spend_restriction.is_none()
                && !matches!(a.production, crate::ManaProduction::SourceChosenColor)
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
            StaticEffect::ChooseOnEntry { .. } if on_battlefield => Ok(()),
            StaticEffect::ChooseOnEntry { .. } => invalid("entry-choice"),
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
            StaticEffect::EntersWithAnnouncedXCounters { kind } => {
                if on_battlefield && kind.admitted() {
                    Ok(())
                } else {
                    invalid("entry-counters:announced-x")
                }
            }
            StaticEffect::EntersWithCounters { kind, count, per } => {
                if on_battlefield
                    && kind.admitted()
                    && *count > Quantity::zero()
                    && per.as_ref().is_none_or(|set| {
                        matches!(set, SetExpr::Matching { .. })
                            && condition_set(set)
                            && !graveyard_other(set)
                    })
                {
                    Ok(())
                } else {
                    invalid("entry-counters")
                }
            }
            StaticEffect::Continuous {
                affects,
                changes,
                condition,
            } => {
                if counts_all_counter_kinds(None, condition.as_ref()) {
                    return invalid("all-kinds-counter-count");
                }
                let Some(first_layer) = changes.iter().map(Change::layer).min() else {
                    return invalid("empty-continuous");
                };
                let replaces = changes.iter().any(type_replacement);
                let removes = changes.iter().any(ability_removal);
                let scope_ok = if replaces || removes {
                    replacement_scope_valid(affects, definition, changes)
                } else {
                    scope_valid(
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
                };
                // CR 613.8: a layer-4 condition may read only what no layer
                // changes (devotion reads mana costs and control).
                let condition_ok = |c: &Cond| {
                    condition_supported(c)
                        && if first_layer <= Layer::Type {
                            replaces
                                && !removes
                                && *affects == Scope::Source
                                && layer_free_condition(c)
                        } else {
                            !removes
                        }
                };
                if !on_battlefield
                    || !scope_ok
                    || condition.as_ref().is_some_and(|c| !condition_ok(c))
                {
                    return invalid("continuous-scope");
                }
                for change in changes {
                    let supported = match change {
                        // CR 608.2h, 613.4c: an additive change is fixed or
                        // a nonnegative count of public objects read through
                        // layers 1-5, the values a CDA may read (CR 604.3),
                        // so it never depends on a layer-7 result.
                        Change::PowerToughness { power, toughness } => {
                            creature_scope(affects, kind)
                                && [power, toughness]
                                    .into_iter()
                                    .all(|n| n.as_fixed().is_some() || cda_amount(n))
                        }
                        Change::SetPowerToughness { power, toughness } => {
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
                                        | KeywordAbility::ProtectionFromEachColor
                                        | KeywordAbility::Flanking => false,
                                        _ => creature_scope(affects, kind),
                                    }
                                })
                        }
                        Change::AddManaAbilities { abilities } => {
                            granted_mana_supported(affects, condition.as_ref(), abilities)
                        }
                        Change::Controller { .. } | Change::PowerToughnessX { .. } => false,
                        Change::RemoveKeywords { keywords }
                        | Change::ForbidKeywords { keywords } => {
                            removable_keywords(keywords) && creature_scope(affects, kind)
                        }
                        Change::RemoveAllAbilities { except_own_grants } => {
                            let grants = changes.iter().any(|c| {
                                matches!(
                                    c,
                                    Change::AddKeywords { .. } | Change::AddManaAbilities { .. }
                                )
                            });
                            *except_own_grants == grants
                        }
                        Change::SetCardTypes { kinds } => {
                            let creature = kinds.contains(&CardKind::Creature);
                            replacement_kinds(kinds)
                                && !matches!(affects, Scope::Matching { .. })
                                && (!creature
                                    || filter_requires(affects, CardKind::Creature)
                                    || changes
                                        .iter()
                                        .any(|c| matches!(c, Change::SetPowerToughness { .. })))
                        }
                        Change::RemoveCardTypes { kinds } => {
                            *kinds == [CardKind::Creature]
                                && *affects == Scope::Source
                                && kind == CardKind::Creature
                                && changes.len() == 1
                        }
                        Change::SetCreatureSubtypes { subtypes } => {
                            distinct(subtypes)
                                && subtypes.iter().all(|s| is_creature_subtype(s))
                                && !matches!(affects, Scope::Matching { .. })
                                && (creature_scope(affects, kind)
                                    || changes.iter().any(|c| {
                                        matches!(c, Change::SetCardTypes { kinds }
                                            if kinds.contains(&CardKind::Creature))
                                    }))
                        }
                        Change::SetLandSubtypes { subtypes } => {
                            distinct(subtypes)
                                && subtypes.iter().all(|s| super::is_basic_land_type(s))
                                && (filter_requires(affects, CardKind::Land)
                                    || changes.iter().any(|c| {
                                        matches!(c, Change::SetCardTypes { kinds }
                                            if kinds.contains(&CardKind::Land))
                                    }))
                        }
                        Change::SetColors { colors } => {
                            colors.iter().all(|color| *color != Color::C)
                        }
                        Change::AddCreatureSubtypes { subtypes } => {
                            !replaces
                                && creature_scope(affects, kind)
                                && affects.filter().is_none_or(only_stable_creature_filter)
                                && !subtypes.is_empty()
                                && subtypes.iter().all(|s| is_creature_subtype(s))
                                && changes.iter().filter(|c| c.layer() == Layer::Type).count() == 1
                        }
                        Change::AddTypes { kinds } => {
                            !replaces
                                && !kinds.is_empty()
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
                            !replaces
                                && changes.len() == 1
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
            StaticEffect::DefinePowerToughness { power, toughness } => {
                let c = &definition.characteristics;
                let defines = |amount: &Option<Amount>, printed: &Option<Quantity>| {
                    amount
                        .as_ref()
                        .is_none_or(|amount| printed.is_some() && cda_amount(amount))
                };
                // Each stat has at most one definition, possibly printed on
                // separate lines.
                let definitions = |power_stat: bool| {
                    definition
                        .static_abilities
                        .iter()
                        .filter(|a| match &a.effect {
                            StaticEffect::DefinePowerToughness { power, toughness } => {
                                if power_stat { power } else { toughness }.is_some()
                            }
                            _ => false,
                        })
                        .count()
                };
                if self.functioning_zone == crate::FunctioningZone::Any
                    && c.has_type(CardKind::Creature)
                    && (power.is_some() || toughness.is_some())
                    && defines(power, &c.power)
                    && defines(toughness, &c.toughness)
                    && definitions(true) <= 1
                    && definitions(false) <= 1
                {
                    Ok(())
                } else {
                    invalid("characteristic-power-toughness")
                }
            }
            StaticEffect::Rule { rule, condition } => {
                let multiplier = match rule {
                    StaticRule::SpellCost { multiplier, .. } => multiplier.as_ref(),
                    _ => None,
                };
                if counts_all_counter_kinds(multiplier, condition.as_ref()) {
                    return invalid("all-kinds-counter-count");
                }
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
                                    && cost_amount(m)
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
                    StaticRule::AttackRestriction {
                        affects, defender, ..
                    } => {
                        on_battlefield
                            && declaration_scope_valid(affects, definition, kind)
                            && *defender == PlayerExpr::static_controller()
                    }
                    StaticRule::PreventDamage { event, prevent } => {
                        on_battlefield
                            && damage_event_valid(event, definition)
                            && match prevent {
                                PreventionAmount::All | PreventionAmount::Half { .. } => true,
                                PreventionAmount::UpTo { amount } => *amount > Quantity::zero(),
                            }
                    }
                    StaticRule::ModifyDamage { event, change } => {
                        on_battlefield
                            && damage_event_valid(event, definition)
                            && match change {
                                DamageModification::Multiply { factor } => {
                                    *factor >= Quantity::from(2)
                                }
                                DamageModification::Add { amount } => *amount > Quantity::zero(),
                            }
                    }
                    StaticRule::DamageCantBePrevented { event } => {
                        on_battlefield && damage_event_valid(event, definition)
                    }
                    // CR 614.1a: tokens only multiply in the first tranche;
                    // counter kinds are object counters (CR 122.1).
                    StaticRule::ModifyQuantity { event, change } => {
                        on_battlefield
                            && condition.is_none()
                            && change.validate().is_ok()
                            && match event {
                                QuantityEvent::CreateTokens { .. } => {
                                    matches!(change, crate::QuantityModification::Multiply { .. })
                                }
                                QuantityEvent::PutCounters { counter, .. } => counter
                                    .as_ref()
                                    .is_none_or(|c| c.admitted() || *c == CounterKind::Loyalty),
                            }
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

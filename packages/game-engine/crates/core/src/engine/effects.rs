//! Finite CR 611/613 additive effects and CR 614/615/616 damage processing.
//! Applied replacement prefixes are semantic steps; life/marks remain atomic.
//! Exact46-card tables:12 creatures *4 Artists <=48 death triggers per
//! nonempty-stack episode; <=13 other entries gives <=61, below128. Empty-stack
//! permanent casting and no recursion prevent accumulating episodes. Two Healers
//! tap at most once before cleanup, so two Growths +Disfigure +Fog +two shields
//! give <=6 live resolved effects (reviewed cap16). <=24 positive combat edges.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{
    Allowance, Amount, Atom, Change, CharacteristicSet, Cmp, CombatProhibitionScope,
    CombatRequirement, Cond, HandSizeChange, Layer, PlayerExpr, Pred, Ref, Scope, SetExpr,
    SpellCostChange, StaticRule, Until, ZoneExpr,
};

macro_rules! effect_id {
    ($name:ident,$ty:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $ty);
    };
}
effect_id!(EffectId, u64);
effect_id!(DamageTransactionId, u64);
effect_id!(DamageId, u32);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectKey {
    Resolved { id: EffectId },
    IntrinsicProtectionFromEverything { source: ObjectId },
    IntrinsicProtectionFromColor { source: ObjectId, color: Color },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageKind {
    Combat,
    Noncombat,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectOrigin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_ability: Option<u32>,
    pub stack_id: StackId,
    pub source: ObjectId,
    pub controller: PlayerId,
    pub source_handles: Vec<publish::StoredObjectHandle>,
    pub cursor: u32,
    pub source_block: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedEffect {
    pub target_handles: Vec<publish::StoredObjectHandle>,
    pub id: EffectId,
    pub origin: EffectOrigin,
    pub expires_in_cleanup_of_turn: u64,
    pub kind: ResolvedEffectKind,
}
impl ResolvedEffect {
    fn lasts_until_controller_turn(&self) -> bool {
        matches!(
            self.kind,
            ResolvedEffectKind::ProtectControllerUntilNextTurn
        )
    }
    fn extends_beyond_creation_turn(&self) -> bool {
        self.lasts_until_controller_turn()
            || matches!(
                self.kind,
                ResolvedEffectKind::GainControl {
                    until: Until::Indefinite | Until::SourceLeaves,
                    ..
                }
            )
    }

    fn survives_cleanup(&self, owner: PlayerId, turn: u64) -> bool {
        matches!(
            self.kind,
            ResolvedEffectKind::GainControl {
                until: Until::Indefinite | Until::SourceLeaves,
                ..
            }
        ) || (self.extends_beyond_creation_turn()
            && (self.lasts_until_controller_turn()
                || self.origin.controller != owner
                || self.expires_in_cleanup_of_turn == turn))
    }

    pub(super) fn additional_land_plays(&self, player: PlayerId, turn: u64) -> Option<&Quantity> {
        match &self.kind {
            ResolvedEffectKind::AdditionalLandPlays { count }
                if self.origin.controller == player && self.expires_in_cleanup_of_turn == turn =>
            {
                Some(count)
            }
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResolvedEffectKind {
    RegenerationShield,
    ProtectControllerUntilNextTurn,
    RemoveIndestructibleSet {
        targets: Vec<ObjectId>,
        timestamp: u64,
    },
    GrantKeywordsSet {
        targets: Vec<ObjectId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
    },
    PumpSet {
        targets: Vec<ObjectId>,
        power: Quantity,
        toughness: Quantity,
    },
    ExileDamaged {
        targets: Vec<ObjectId>,
    },
    GrantKeywords {
        target: ObjectId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
    },
    CombatProhibition {
        affects: CombatProhibitionScope<ObjectId>,
        restriction: CombatRequirement,
    },
    GainControl {
        targets: Vec<ObjectId>,
        controller: PlayerId,
        until: Until,
    },
    SetCharacteristics {
        targets: Vec<ObjectId>,
        values: CharacteristicSet,
        timestamp: u64,
    },
    PumpAnnouncedX {
        targets: Vec<ObjectId>,
        x: Quantity,
        modifier: CreatureStatModifier,
    },

    ModifyPowerToughness {
        target: ObjectId,
        power: Quantity,
        toughness: Quantity,
    },

    PreventCombatDamage,
    AdditionalLandPlays {
        count: Quantity,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedDamage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_appearance: Option<Box<ObjectBefore>>,
    pub source_handles: Vec<publish::StoredObjectHandle>,
    pub id: DamageId,
    pub source: ObjectBefore,
    pub controller: PlayerId,
    pub recipient: DamageRecipient,
    pub original_amount: Quantity,
    pub amount: Quantity,
    pub applied: Vec<EffectKey>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageApplication {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualification: Option<Box<DamageReplacementQualification>>,
    pub damage: DamageId,
    pub effect: EffectKey,
    pub before: Quantity,
    pub after: Quantity,
    pub prevented: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageReplacementQualification {
    pub effect_source: ObjectBefore,
    pub damage_source: ObjectBefore,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActualDamage {
    pub id: DamageId,
    pub source: ObjectBefore,
    pub controller: PlayerId,
    pub recipient: DamageRecipient,
    pub amount: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageTransaction {
    pub id: DamageTransactionId,
    pub kind: DamageKind,
    pub entries: Vec<PreparedDamage>,
    pub applications: Vec<DamageApplication>,
    pub cause: BatchCause,
    pub continuation: DamageContinuation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "continuation", rename_all = "snake_case", deny_unknown_fields)]
pub enum DamageContinuation {
    Combat,
    Instruction {
        frame: Box<Resolving>,
    },
    /// CR 605.1a, 605.3b: a resolving mana ability's damage rider; the
    /// activation resumes with its next rider.
    Mana {
        activation: Box<ManaActivation>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageEffectCandidate {
    pub effect: EffectKey,
    pub damage: DamageId,
}

impl DamageRecipient {
    pub(super) fn entity(&self) -> TargetEntity {
        match *self {
            Self::Player { player } => TargetEntity::Player { player },
            Self::Creature { object } => TargetEntity::Object { object },
        }
    }
}
fn static_compare(left: &Quantity, cmp: Cmp, right: &Quantity) -> bool {
    match cmp {
        Cmp::Exactly => left == right,
        Cmp::AtMost => left <= right,
        Cmp::AtLeast => left >= right,
    }
}

// CR 613.7m: simultaneous entrants with a conflicting setter would
// need a player's timestamp choice. Only ordered or commuting setters are
// admitted at this boundary.
fn latest_setter<T: PartialEq>(
    setters: impl Iterator<Item = (u64, T)>,
) -> Result<Option<T>, String> {
    let mut latest = None;
    let mut conflicts = false;
    for (time, value) in setters {
        match &latest {
            Some((previous, _)) if time < *previous => {}
            Some((previous, earlier)) if time == *previous => conflicts |= *earlier != value,
            _ => {
                latest = Some((time, value));
                conflicts = false;
            }
        }
    }
    if conflicts {
        return Err("simultaneous continuous setters require timestamp order".into());
    }
    Ok(latest.map(|(_, value)| value))
}

impl RulesTxn {
    pub(super) fn resolved_characteristic_sets(
        &self,
        object: ObjectId,
    ) -> impl Iterator<Item = (u64, &CharacteristicSet)> {
        let present = self
            .current(object)
            .is_ok_and(|o| o.present_on_battlefield());
        self.state
            .rules
            .effects
            .iter()
            .filter_map(move |effect| match &effect.kind {
                ResolvedEffectKind::SetCharacteristics {
                    targets,
                    values,
                    timestamp,
                } if present && targets.contains(&object) => Some((*timestamp, values)),
                _ => None,
            })
    }

    fn static_player(&self, expr: &PlayerExpr, controller: PlayerId, player: PlayerId) -> bool {
        match expr {
            PlayerExpr::EachPlayer => self.is_live(player),
            PlayerExpr::EachOpponent => player != controller && self.is_live(player),
            _ => *expr == PlayerExpr::static_controller() && player == controller,
        }
    }

    fn static_predicate(
        &self,
        filter: &Pred,
        source: &Object,
        target: &Object,
        characteristics: &Characteristics,
        controller: PlayerId,
    ) -> bool {
        filter.0.iter().all(|atom| match atom {
            Atom::Type { kind } => characteristics.has_type(*kind),
            Atom::NotType { kind } => !characteristics.has_type(*kind),
            Atom::AnyType { kinds } => kinds.iter().any(|k| characteristics.has_type(*k)),
            Atom::Subtype { subtype } => characteristics.subtypes.contains(subtype),
            Atom::NotSubtype { subtype } => !characteristics.subtypes.contains(subtype),
            Atom::Basic => characteristics.basic,
            Atom::NotBasic => !characteristics.basic,
            Atom::Legendary => characteristics.legendary,
            Atom::NotLegendary => !characteristics.legendary,
            Atom::Color { color } => characteristics.colors.contains(color),
            Atom::NotColor { color } => !characteristics.colors.contains(color),
            Atom::ControlledBy { player } => {
                self.static_player(player, source.controller, controller)
            }
            Atom::OwnedBy { player } => self
                .state
                .rules
                .cards
                .get(&target.id.card)
                .is_some_and(|card| self.static_player(player, source.controller, card.owner)),
            Atom::Token => self.state.rules.tokens.contains_key(&target.id.card),
            Atom::NotToken => !self.state.rules.tokens.contains_key(&target.id.card),
            Atom::Tapped => target.tapped,
            Atom::Untapped => !target.tapped,
            Atom::Attacking => self
                .state
                .rules
                .combat
                .iter()
                .any(|attack| attack.attacker == target.id),
            Atom::Blocking => self.state.rules.blocking_creatures.contains(&target.id),
            Atom::ManaValue { cmp, value } => value.as_fixed().is_some_and(|bound| {
                static_compare(&characteristics.printed_mana_value(), *cmp, bound)
            }),
            Atom::IsNot { object } => *object == Ref::Source && source.id != target.id,
            Atom::AnyOf { alternatives } => alternatives
                .iter()
                .any(|p| self.static_predicate(p, source, target, characteristics, controller)),
            Atom::Power { .. }
            | Atom::Toughness { .. }
            | Atom::Keyword { .. }
            | Atom::NotKeyword { .. }
            | Atom::Is { .. } => false,
        })
    }

    fn static_matches(
        &self,
        scope: &Scope,
        source: &Object,
        target: &Object,
        characteristics: &Characteristics,
        controller: PlayerId,
    ) -> bool {
        match scope {
            Scope::Source => source.id == target.id,
            Scope::Matching { filter } => {
                self.static_predicate(filter, source, target, characteristics, controller)
            }
            Scope::Attachment { filter, .. } => {
                source.attached_to == Some(target.id)
                    && self.static_predicate(filter, source, target, characteristics, controller)
            }
        }
    }

    fn static_count(&self, source: &Object, set: &SetExpr) -> usize {
        match set {
            SetExpr::One {
                object: Ref::Source,
            } => 1,
            SetExpr::Matching { zone, filter } => self
                .state
                .rules
                .objects
                .values()
                .filter(|target| {
                    let present = match zone {
                        ZoneExpr::Battlefield => target.present_on_battlefield(),
                        ZoneExpr::Graveyards => matches!(target.zone, Zone::Graveyard { .. }),
                        ZoneExpr::Graveyard { of } => {
                            matches!(target.zone, Zone::Graveyard { owner }
                        if self.static_player(of, source.controller, owner))
                        }
                        _ => false,
                    };
                    present
                        && (!matches!(target.zone, Zone::Graveyard { .. })
                            || !self.state.rules.tokens.contains_key(&target.id.card))
                        && self
                            .structural_characteristics(target.id)
                            .is_ok_and(|characteristics| {
                                self.static_predicate(
                                    filter,
                                    source,
                                    target,
                                    &characteristics,
                                    target.controller,
                                )
                            })
                })
                .count(),
            _ => 0,
        }
    }

    fn static_amount(&self, source: &Object, amount: &Amount) -> Option<Quantity> {
        match amount {
            Amount::Fixed { value } => Some(value.clone()),
            Amount::Count { set } => Some((self.static_count(source, set) as i64).into()),
            Amount::Devotion { player, colors } => {
                // CR 700.5, 700.5a: a hybrid symbol contributes at most one.
                let mut count = Quantity::zero();
                for permanent in self
                    .battlefield_static_sources(None)
                    .filter(|p| self.static_player(player, source.controller, p.controller))
                {
                    let c = &self.copiable_definition(permanent.id).ok()?.characteristics;
                    let ordinary = c
                        .mana_cost
                        .colored
                        .iter()
                        .filter(|color| colors.contains(color))
                        .count();
                    let hybrid = c
                        .hybrid_mana
                        .iter()
                        .filter(|symbol| {
                            symbol
                                .colors
                                .iter()
                                .filter_map(|payment| payment.color())
                                .any(|c| colors.contains(&c))
                        })
                        .count();
                    count.add(&((ordinary + hybrid) as i64).into());
                }
                Some(count)
            }
            _ => None,
        }
    }

    pub(super) fn entry_condition(
        &self,
        entrant: &Object,
        condition: &mtg_contracts::kir::EntryCondition,
    ) -> bool {
        use mtg_contracts::kir::EntryCondition;
        match condition {
            EntryCondition::UnlessControlled { filter, cmp, count } => {
                let actual = self
                    .battlefield_static_sources(None)
                    .filter(|target| {
                        target.id != entrant.id
                            && target.controller == entrant.controller
                            && self.structural_characteristics(target.id).is_ok_and(
                                |characteristics| {
                                    self.static_predicate(
                                        filter,
                                        entrant,
                                        target,
                                        &characteristics,
                                        target.controller,
                                    )
                                },
                            )
                    })
                    .count();
                !static_compare(&(actual as i64).into(), *cmp, count)
            }
            EntryCondition::UnlessOpponents { cmp, count } => {
                let actual = self
                    .live_players()
                    .into_iter()
                    .filter(|p| *p != entrant.controller)
                    .count();
                !static_compare(&(actual as i64).into(), *cmp, count)
            }
        }
    }

    fn static_condition(&self, source: &Object, condition: &Cond) -> bool {
        match condition {
            Cond::Compare { left, cmp, right } => self
                .static_amount(source, left)
                .zip(self.static_amount(source, right))
                .is_some_and(|(left, right)| static_compare(&left, *cmp, &right)),
            Cond::Exists { set } => self.static_count(source, set) > 0,
            Cond::ActivePlayer { player } => self
                .state
                .rules
                .active
                .is_some_and(|active| self.static_player(player, source.controller, active)),
            Cond::Not { condition } => !self.static_condition(source, condition),
            Cond::All { conditions } => conditions.iter().all(|c| self.static_condition(source, c)),
            Cond::CostPaid { .. } | Cond::Satisfies { .. } | Cond::CastBy { .. } => false,
        }
    }

    fn battlefield_statics<'a>(
        &'a self,
        sources: Option<&'a [CardId]>,
    ) -> impl Iterator<Item = (&'a Object, &'a StaticEffect)> + 'a {
        self.battlefield_static_sources(sources).flat_map(|source| {
            self.static_abilities_in(source, FunctioningZone::Battlefield)
                .map(move |effect| (source, effect))
        })
    }

    fn static_abilities_in<'a>(
        &'a self,
        source: &'a Object,
        zone: FunctioningZone,
    ) -> impl Iterator<Item = &'a StaticEffect> + 'a {
        self.copiable_definition(source.id)
            .into_iter()
            .flat_map(move |definition| {
                definition
                    .static_abilities
                    .iter()
                    .filter(move |ability| ability.functioning_zone == zone)
                    .map(|ability| &ability.effect)
            })
    }

    fn unpack_static_rule<'a>(
        &self,
        source: &Object,
        effect: &'a StaticEffect,
    ) -> Option<&'a StaticRule> {
        match effect {
            StaticEffect::Rule { rule, condition }
                if condition
                    .as_ref()
                    .is_none_or(|c| self.static_condition(source, c)) =>
            {
                Some(rule)
            }
            _ => None,
        }
    }

    pub(super) fn static_changes<'a>(
        &'a self,
        target: ObjectId,
        layer: Layer,
        sources: Option<&'a [CardId]>,
    ) -> Vec<(u64, &'a Change)> {
        let Ok(target) = self.current(target) else {
            return Vec::new();
        };
        if !target.present_on_battlefield() {
            return Vec::new();
        }
        let mut matching = Vec::new();
        for (source, effect) in self.battlefield_statics(sources) {
            let StaticEffect::Continuous {
                affects,
                changes,
                condition,
            } = effect
            else {
                continue;
            };
            if !changes.iter().any(|c| c.layer() == layer) {
                continue;
            }
            // CR 613.6: the first layer fixes membership across the effect.
            let first = changes
                .iter()
                .map(Change::layer)
                .min()
                .expect("checked static changes");
            let facts = match first {
                // CR 613.8: admitted layer-4 static scopes cannot read any
                // type or subtype that another admitted layer-4 effect changes.
                Layer::Type => self
                    .copiable_definition(target.id)
                    .map(|d| std::borrow::Cow::Borrowed(&d.characteristics)),
                Layer::Color => self.structural_characteristics(target.id),
                _ => self.color_layer_with_sources(target.id, sources),
            };
            if facts
                .is_ok_and(|c| self.static_matches(affects, source, target, &c, target.controller))
                && condition
                    .as_ref()
                    .is_none_or(|c| self.static_condition(source, c))
            {
                matching.extend(
                    changes
                        .iter()
                        .filter(|c| c.layer() == layer)
                        .map(|change| (source.keyword_timestamp.unwrap_or(0), change)),
                );
            }
        }
        matching.sort_by_key(|(timestamp, _)| *timestamp);
        matching
    }

    pub(super) fn combat_statics<'a>(
        &'a self,
        target: ObjectId,
        sources: Option<&'a [CardId]>,
    ) -> impl Iterator<Item = CombatRequirement> + 'a {
        let target = self
            .current(target)
            .ok()
            .filter(|o| o.present_on_battlefield());
        self.battlefield_statics(sources)
            .filter_map(move |(source, effect)| {
                let target = target?;
                let StaticRule::Combat {
                    affects,
                    requirement,
                } = self.unpack_static_rule(source, effect)?
                else {
                    return None;
                };
                let facts = self.color_layer_with_sources(target.id, sources).ok()?;
                self.static_matches(affects, source, target, &facts, target.controller)
                    .then_some(*requirement)
            })
    }

    pub(super) fn static_land_allowance(&self, player: PlayerId, total: &mut Quantity) -> bool {
        for (source, effect) in self.battlefield_statics(None) {
            if let Some(StaticRule::AdditionalLandPlays {
                player: who,
                allowance,
            }) = self.unpack_static_rule(source, effect)
                && self.static_player(who, source.controller, player)
            {
                match allowance {
                    Allowance::Unlimited => return true,
                    Allowance::Additional { count } => total.add(count),
                }
            }
        }
        false
    }

    pub(super) fn static_hand_size(&self, player: PlayerId) -> Result<Option<Quantity>, String> {
        let mut changes: Vec<_> = self
            .battlefield_statics(None)
            .filter_map(|(source, effect)| {
                let StaticRule::MaximumHandSize {
                    player: who,
                    change,
                } = self.unpack_static_rule(source, effect)?
                else {
                    return None;
                };
                self.static_player(who, source.controller, player)
                    .then_some((source.keyword_timestamp.unwrap_or(0), change))
            })
            .collect();
        changes.sort_by_key(|(timestamp, _)| *timestamp);
        let mut size = Some(Quantity::from(7));
        for group in changes.chunk_by(|a, b| a.0 == b.0) {
            if let Some((_, set)) = group
                .iter()
                .find(|(_, change)| matches!(change, HandSizeChange::Set { .. }))
                && group.iter().any(|(_, other)| set != other)
            {
                return Err("simultaneous hand-size setters require timestamp order".into());
            }
        }
        for (_, change) in changes {
            match change {
                HandSizeChange::Unlimited => size = None,
                HandSizeChange::Set { size: new } => size = Some(new.clone()),
                HandSizeChange::Adjust { delta } => {
                    if let Some(size) = &mut size {
                        size.add(delta);
                    }
                }
            }
        }
        Ok(size.map(|size| size.max(Quantity::zero())))
    }

    fn spell_static_rules<'a>(
        &'a self,
        target: &'a Object,
    ) -> impl Iterator<Item = (&'a Object, &'a StaticRule)> + 'a {
        self.battlefield_statics(None)
            .chain(
                self.static_abilities_in(target, FunctioningZone::Stack)
                    .map(move |e| (target, e)),
            )
            .filter_map(|(source, effect)| {
                self.unpack_static_rule(source, effect)
                    .map(|rule| (source, rule))
            })
    }

    pub(super) fn has_uncounterable_static(
        &self,
        target: ObjectId,
        caster: PlayerId,
    ) -> Result<bool, String> {
        let object = self.current(target)?;
        let characteristics = &self.copiable_definition(target)?.characteristics;
        Ok(self.spell_static_rules(object).any(|(source, rule)| {
            matches!(rule, StaticRule::CantBeCountered { affects }
                if self.static_matches(affects, source, object, characteristics, caster))
        }))
    }

    pub(super) fn static_cost_changes(
        &self,
        card: CardId,
        caster: PlayerId,
    ) -> Result<Vec<SpellCostChange>, String> {
        let object = self
            .state
            .rules
            .objects
            .get(&card)
            .ok_or("missing spell object")?;
        let characteristics = &self.copiable_definition(object.id)?.characteristics;
        let mut changes = Vec::new();
        for (source, rule) in self.spell_static_rules(object) {
            let StaticRule::SpellCost {
                affects,
                caster: who,
                change,
                multiplier,
            } = rule
            else {
                continue;
            };
            if self.static_player(who, source.controller, caster)
                && self.static_matches(affects, source, object, characteristics, caster)
            {
                let mut change = change.clone();
                if let Some(amount) = multiplier {
                    let multiplier = self
                        .static_amount(source, amount)
                        .ok_or("invalid static cost multiplier")?;
                    let (SpellCostChange::Increase { mana } | SpellCostChange::Reduce { mana }) =
                        &mut change;
                    mana.generic.multiply(&multiplier);
                }
                changes.push(change);
            }
        }
        Ok(changes)
    }

    pub(super) fn has_flash_permission(
        &self,
        object: &Object,
        caster: PlayerId,
    ) -> Result<bool, String> {
        let characteristics = &self.copiable_definition(object.id)?.characteristics;
        Ok(self.spell_static_rules(object).any(|(source, rule)| {
            matches!(rule, StaticRule::CastAsThoughFlash { affects, caster: who }
                if self.static_player(who, source.controller, caster)
                    && self.static_matches(affects, source, object, characteristics, caster))
        }))
    }
}

impl RulesTxn {
    fn resolved_type_layer(
        &self,
        object: ObjectId,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        let mut facts =
            std::borrow::Cow::Borrowed(&self.copiable_definition(object)?.characteristics);
        // CR 613.7b: retained rows are in creation order, including
        // replacement of the creature subtypes added by an earlier row.
        for (_, values) in self.resolved_characteristic_sets(object) {
            let Some(animation) = &values.animation else {
                continue;
            };
            let facts = facts.to_mut();
            for kind in [CardKind::Creature, CardKind::Artifact] {
                if (kind == CardKind::Creature || animation.add_artifact) && !facts.has_type(kind) {
                    facts.additional_types.push(kind);
                }
            }
            facts.additional_types.sort_unstable();
            if animation.replace_creature_subtypes {
                facts
                    .subtypes
                    .retain(|s| !mtg_contracts::kir::is_creature_subtype(s));
            }
            for subtype in &animation.creature_subtypes {
                if !facts.subtypes.contains(subtype) {
                    facts.subtypes.push(subtype.clone());
                }
            }
        }
        Ok(facts)
    }

    pub(super) fn structural_characteristics(
        &self,
        object: ObjectId,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        let mut facts = self.resolved_type_layer(object)?;
        for (_, change) in self.static_changes(object, Layer::Type, None) {
            if let Change::AddTypes { kinds } = change {
                for kind in kinds {
                    if !facts.has_type(*kind) {
                        let additional = &mut facts.to_mut().additional_types;
                        additional.push(*kind);
                        additional.sort_unstable();
                    }
                }
            }
        }
        Ok(facts)
    }

    pub(crate) fn color_layer(
        &self,
        object: ObjectId,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        self.color_layer_with_sources(object, None)
    }

    fn color_layer_with_sources(
        &self,
        object: ObjectId,
        sources: Option<&[CardId]>,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        let mut facts = self.structural_characteristics(object)?;
        let statics = self.static_changes(object, Layer::Color, sources);
        let colors = statics
            .into_iter()
            .filter_map(|(time, change)| match change {
                Change::SetColors { colors } => Some((time, colors)),
                _ => None,
            });
        let resolved = self
            .resolved_characteristic_sets(object)
            .filter_map(|(time, values)| Some((time, values.colors.as_ref()?)));
        // CR 613.8a: static color scopes cannot read colors, and a
        // resolved setter always affects its captured objects.
        if let Some(colors) = latest_setter(colors.chain(resolved))? {
            facts.to_mut().colors.clone_from(colors);
        }
        Ok(facts)
    }

    #[cfg(feature = "testkit")]
    pub(crate) fn derived_characteristics(
        &self,
        object: ObjectId,
    ) -> Result<Characteristics, String> {
        self.derived_characteristics_with_sources(object, None)
    }

    pub(super) fn derived_characteristics_with_sources(
        &self,
        object: ObjectId,
        sources: Option<&[CardId]>,
    ) -> Result<Characteristics, String> {
        let mut c = self.color_layer_with_sources(object, sources)?.into_owned();
        if c.has_type(CardKind::Creature) {
            let statics = self.static_changes(object, Layer::PowerToughnessSet, sources);
            // CR 613.4b: a setter can leave either characteristic alone.
            // Neither supported kind reads a layer-7 result (CR 613.8a).
            let base = |power_stat| {
                let fixed = statics.iter().filter_map(|(time, change)| {
                    let Change::SetPowerToughness { power, toughness } = change else {
                        return None;
                    };
                    Some((
                        *time,
                        if power_stat { power } else { toughness }.as_fixed()?,
                    ))
                });
                let resolved =
                    self.resolved_characteristic_sets(object)
                        .filter_map(|(time, values)| {
                            Some((
                                time,
                                if power_stat {
                                    &values.base_power
                                } else {
                                    &values.base_toughness
                                }
                                .as_ref()?,
                            ))
                        });
                latest_setter(fixed.chain(resolved)).map(|value| value.cloned())
            };
            if let Some(power) = base(true)? {
                c.power = Some(power);
            }
            if let Some(toughness) = base(false)? {
                c.toughness = Some(toughness);
            }
        }
        // CR 122.1a / 613.4c: counters and additive effects share layer 7c.
        if let Ok(o) = self.current(object) {
            if let Some(p) = c.power.as_mut() {
                p.add(&o.plus_one_counters);
            }
            if let Some(t) = c.toughness.as_mut() {
                t.add(&o.plus_one_counters);
            }
        }
        for (_, change) in self.static_changes(object, Layer::PowerToughnessModify, sources) {
            if let Change::PowerToughness { power, toughness } = change {
                c.power
                    .as_mut()
                    .ok_or("static power")?
                    .add(power.as_fixed().ok_or("static power operand")?);
                c.toughness
                    .as_mut()
                    .ok_or("static toughness")?
                    .add(toughness.as_fixed().ok_or("static toughness operand")?);
            }
        }
        for e in &self.state.rules.effects {
            // CR 611.2c/613.5-6: the stat and keyword layers of one resolved
            // effect share the same captured incarnations and creation boundary.
            if let ResolvedEffectKind::PumpSet {
                targets,
                power,
                toughness,
            }
            | ResolvedEffectKind::GrantKeywordsSet {
                targets,
                modifier: Some(CreatureStatModifier { power, toughness }),
                ..
            }
            | ResolvedEffectKind::PumpAnnouncedX {
                targets,
                modifier: CreatureStatModifier { power, toughness },
                ..
            } = &e.kind
                && targets.contains(&object)
                && c.has_type(CardKind::Creature)
            {
                c.power.as_mut().ok_or("set power")?.add(power);
                c.toughness.as_mut().ok_or("set toughness")?.add(toughness);
            }
            if let ResolvedEffectKind::ModifyPowerToughness {
                target,
                power,
                toughness,
            }
            | ResolvedEffectKind::GrantKeywords {
                target,
                modifier: Some(CreatureStatModifier { power, toughness }),
                ..
            } = &e.kind
                && *target == object
                && c.has_type(CardKind::Creature)
            {
                c.power
                    .as_mut()
                    .ok_or("modifier source missing power")?
                    .add(power);
                c.toughness
                    .as_mut()
                    .ok_or("modifier source missing toughness")?
                    .add(toughness);
            }
        }
        Ok(c)
    }
    pub(crate) fn live_effect_keys(&self) -> Vec<EffectKey> {
        self.state
            .rules
            .effects
            .iter()
            .map(|e| EffectKey::Resolved { id: e.id })
            .collect()
    }
    pub(crate) fn execute_effect_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let (kind, block) = match instruction {
            kernel!(ModifyUntil { what: source_object!(), change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block) =>
            {
                let target = self.entry(frame.stack_id)?.source;
                if !self.target_is_creature(target) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ResolvedEffectKind::ModifyPowerToughness {
                        target,
                        power,
                        toughness,
                    },
                    source_block,
                )
            }
            kernel!(ModifyUntil { what: target_object!(target_slot), change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block) =>
            {
                let target = *self
                    .entry(frame.stack_id)?
                    .targets
                    .get(target_slot as usize)
                    .ok_or("missing modifier target")?;
                if !frame
                    .legal_targets
                    .get(target_slot as usize)
                    .copied()
                    .unwrap_or(false)
                    || !self.is_battlefield_creature(target)
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ResolvedEffectKind::ModifyPowerToughness {
                        target,
                        power,
                        toughness,
                    },
                    source_block,
                )
            }

            _ => return Err("unexpected effect instruction".into()),
        };
        self.commit_resolved_effect(frame, kind, block)
    }

    pub(super) fn commit_resolved_effect(
        &mut self,
        frame: Resolving,
        kind: ResolvedEffectKind,
        block: u32,
    ) -> Result<(), String> {
        let consumes_choice = false;
        let handles = match &self.entry(frame.stack_id)?.kind {
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
            StackKind::Spell => self.entry(frame.stack_id)?.effect_source_handles.clone(),
        };
        let target_handles = match &kind {
            ResolvedEffectKind::GrantKeywords { target, .. }
            | ResolvedEffectKind::ModifyPowerToughness { target, .. }
            | ResolvedEffectKind::CombatProhibition {
                affects: CombatProhibitionScope::Target { target },
                ..
            } => self.capture_trigger_source_handles(*target)?,

            _ => vec![],
        };
        let effect = ResolvedEffect {
            target_handles,
            id: EffectId(self.state.rules.next_effect_id),
            origin: EffectOrigin {
                activated_ability: self.entry(frame.stack_id)?.kind.activated_index(),
                stack_id: frame.stack_id,
                source: self.entry(frame.stack_id)?.source,
                controller: self.entry(frame.stack_id)?.controller,
                source_handles: handles,
                cursor: frame.cursor,
                source_block: block,
            },
            expires_in_cleanup_of_turn: self.state.rules.turn.number,
            kind,
        };
        let actual = self.commit(
            ProposedEvent::CreateEffect {
                effect: Box::new(effect),
            },
            BatchCause::Instruction {
                stack_id: frame.stack_id,
                player: self.entry(frame.stack_id)?.controller,
                source: self.entry(frame.stack_id)?.source,
                cursor: frame.cursor,
                source_block: block,
            },
        )?;
        if consumes_choice {
            self.rules_mut().pending = None;
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    // CR 608.2b / 611.2c: creation rechecks current control; the captured
    // effect subsequently follows only the exact incarnation, never control.

    pub(super) fn validate_combat_player_members<'a>(
        members: impl Iterator<Item = (ObjectId, &'a DamageRecipient)>,
    ) -> Result<(), String> {
        let mut pairs = BTreeSet::new();
        for (source, recipient) in members {
            if let DamageRecipient::Player { player } = recipient
                && !pairs.insert((source, *player))
            {
                return Err("duplicate combat source/player occurrence".into());
            }
        }
        Ok(())
    }
    pub(crate) fn prepare_combat_damage(
        &mut self,
        damage: Vec<DamageOccurrence>,
    ) -> Result<(), String> {
        // Keep the existing typed capacity fault ahead of semantic validation,
        // including oversized vectors that also contain duplicate player edges.
        if damage.len() > self.damage_limit() {
            return Err(self.capacity_error(RuleFault::DamageCapacityExceeded {
                capacity: self.damage_limit(),
                requested: damage.len(),
            }));
        }
        // CR 510.1b-c/510.2/603.2c: one player edge per source incarnation
        // in this combat damage step. Arbitrary vector splitting must not mint
        // extra occurrences. First/double strike use separate transactions.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        Self::validate_combat_player_members(damage.iter().map(|d| (d.source, &d.recipient)))?;
        let entries = damage
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                Ok(PreparedDamage {
                    source_appearance: None,
                    source_handles: self.capture_trigger_source_handles(d.source)?,
                    id: DamageId(i as u32),
                    source: self.object_before(d.source)?,
                    controller: d.controller,
                    recipient: d.recipient,
                    original_amount: d.amount.clone(),
                    amount: d.amount,
                    applied: vec![],
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.rules_mut().damage_choices.clear();
        self.rules_mut().player_damage_choices.clear();
        self.begin_damage(
            DamageKind::Combat,
            entries,
            BatchCause::Combat,
            DamageContinuation::Combat,
        )
    }
    pub(crate) fn prepare_noncombat_damage(
        &mut self,
        frame: Resolving,
        damage: NoncombatDamageOccurrence,
        block: u32,
    ) -> Result<(), String> {
        let cause = BatchCause::Instruction {
            stack_id: frame.stack_id,
            player: self.entry(frame.stack_id)?.controller,
            source: self.entry(frame.stack_id)?.source,
            cursor: frame.cursor,
            source_block: block,
        };
        let handles = match &self.entry(frame.stack_id)?.kind {
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
            StackKind::Spell => self.entry(frame.stack_id)?.effect_source_handles.clone(),
        };
        let entries = vec![PreparedDamage {
            source_appearance: None,
            source_handles: handles,
            id: DamageId(0),
            source: damage.source,
            controller: damage.controller,
            recipient: damage.recipient,
            original_amount: damage.amount.clone(),
            amount: damage.amount,
            applied: vec![],
        }];
        self.begin_damage(
            DamageKind::Noncombat,
            entries,
            cause,
            DamageContinuation::Instruction {
                frame: Box::new(frame),
            },
        )
    }
    pub(super) fn prepare_noncombat_set_damage(
        &mut self,
        frame: Resolving,
        amount: Quantity,
        block: u32,
        targets: Vec<TargetEntity>,
    ) -> Result<(), String> {
        let cause = BatchCause::Instruction {
            stack_id: frame.stack_id,
            player: self.entry(frame.stack_id)?.controller,
            source: self.entry(frame.stack_id)?.source,
            cursor: frame.cursor,
            source_block: block,
        };
        let handles = match &self.entry(frame.stack_id)?.kind {
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
            StackKind::Spell => self.entry(frame.stack_id)?.effect_source_handles.clone(),
        };
        let source = self.source_facts(self.entry(frame.stack_id)?)?;
        let controller = self.entry(frame.stack_id)?.controller;
        let entries = targets
            .into_iter()
            .enumerate()
            .map(|(n, t)| PreparedDamage {
                source_appearance: None,
                source_handles: handles.clone(),
                id: DamageId(n as u32),
                source: source.clone(),
                controller,
                recipient: match t {
                    TargetEntity::Player { player } => DamageRecipient::Player { player },
                    TargetEntity::Object { object } => DamageRecipient::Creature { object },
                },
                original_amount: amount.clone(),
                amount: amount.clone(),
                applied: vec![],
            })
            .collect();
        self.begin_damage(
            DamageKind::Noncombat,
            entries,
            cause,
            DamageContinuation::Instruction {
                frame: Box::new(frame),
            },
        )
    }
    pub(super) fn begin_damage(
        &mut self,
        kind: DamageKind,
        entries: Vec<PreparedDamage>,
        cause: BatchCause,
        continuation: DamageContinuation,
    ) -> Result<(), String> {
        if entries.len() > self.damage_limit() {
            return Err(self.capacity_error(RuleFault::DamageCapacityExceeded {
                capacity: self.damage_limit(),
                requested: entries.len(),
            }));
        }
        if entries.is_empty()
            || entries.len() > self.damage_limit()
            || entries.iter().any(|e| e.amount <= Quantity::zero())
        {
            return Err("invalid prepared damage group".into());
        }
        let id = DamageTransactionId(self.state.rules.next_damage_transaction_id);
        self.rules_mut().next_damage_transaction_id = self
            .state
            .rules
            .next_damage_transaction_id
            .checked_add(1)
            .ok_or("damage transaction limit")?;
        self.rules_mut().frame = Frame::DamageEffects {
            transaction: DamageTransaction {
                id,
                kind,
                entries,
                applications: vec![],
                cause,
                continuation,
            },
        };
        Ok(())
    }
    pub(crate) fn damage_transaction(&self) -> Result<&DamageTransaction, String> {
        if let Frame::DamageEffects { transaction } = &self.state.rules.frame {
            Ok(transaction)
        } else {
            Err("missing damage transaction".into())
        }
    }
    pub(super) fn recipient_chooser(&self, recipient: &DamageRecipient) -> Option<PlayerId> {
        match recipient {
            DamageRecipient::Player { player } => self.is_live(*player).then_some(*player),
            DamageRecipient::Creature { object } => self
                .is_battlefield_creature(*object)
                .then(|| self.current(*object).ok().map(|o| o.controller))
                .flatten(),
        }
    }
    /// CR 608.2h/609.7c/702.2e/702.15b-c: damage-time properties use the
    /// exact source's current public-zone appearance or its latest departure
    /// LKI. Assignment facts separately preserve quantities and legality.
    pub(super) fn damage_source_appearance(
        &self,
        damage: &PreparedDamage,
    ) -> Result<ObjectBefore, String> {
        let appearance = if let Ok(current) = self.object_before(damage.source.object)
            && current.zone == damage.source.zone
        {
            current
        } else {
            damage
                .source_appearance
                .as_deref()
                .unwrap_or(&damage.source)
                .clone()
        };
        if appearance.controller.0 >= 4 {
            return Err("invalid damage-time source controller".into());
        }
        Ok(appearance)
    }
    pub(crate) fn player_protected(&self, player: PlayerId) -> bool {
        self.state
            .rules
            .effects
            .iter()
            .any(|e| e.lasts_until_controller_turn() && e.origin.controller == player)
    }
    pub(crate) fn targetable_player(&self, player: PlayerId) -> bool {
        self.is_live(player) && !self.player_protected(player)
    }
    pub(super) fn expire_player_protection(&mut self, player: PlayerId) {
        self.rules_mut()
            .effects
            .retain(|e| !e.lasts_until_controller_turn() || e.origin.controller != player);
    }
    // CR 702.16e,h/616.1: the existing each-color grant is represented
    // by one candidate for matching prepared damage. Printed named-color
    // protection uses separate identities and consumed damage-time facts.
    fn grant_prevents_damage(
        targets: &[ObjectId],
        keywords: &[KeywordAbility],
        damage: &PreparedDamage,
    ) -> bool {
        matches!(damage.recipient, DamageRecipient::Creature { object } if targets.contains(&object))
            && keywords.contains(&KeywordAbility::ProtectionFromEachColor)
            && Self::has_color(&damage.source.characteristics)
    }

    // CR 702.16e,g and 616.1: separate printed qualities have separate
    // candidate identities even when a multicolored source matches both.
    fn intrinsic_protection_keys(&self, recipient: &DamageRecipient) -> Vec<EffectKey> {
        let DamageRecipient::Creature { object } = *recipient else {
            return vec![];
        };
        if !self
            .current(object)
            .is_ok_and(|o| o.present_on_battlefield())
        {
            return vec![];
        }
        let mut keys: Vec<_> = self
            .effective_protection_colors(object)
            .map(|color| EffectKey::IntrinsicProtectionFromColor {
                source: object,
                color,
            })
            .collect();
        if self.printed_protection_from_everything(object) {
            keys.push(EffectKey::IntrinsicProtectionFromEverything { source: object });
        }
        keys
    }

    fn application(
        &self,
        tx: &DamageTransaction,
        pair: &DamageEffectCandidate,
    ) -> Result<DamageApplication, String> {
        let d = tx
            .entries
            .iter()
            .find(|d| d.id == pair.damage)
            .ok_or("unknown original damage occurrence")?;
        if d.amount <= Quantity::zero()
            || d.applied.contains(&pair.effect)
            || self.recipient_chooser(&d.recipient).is_none()
        {
            return Err("damage effect no longer applicable".into());
        }
        let mut qualification = None;
        let (after, prevented) = match &pair.effect {
            EffectKey::IntrinsicProtectionFromEverything { source } => {
                if d.recipient != (DamageRecipient::Creature { object: *source })
                    || !self
                        .current(*source)
                        .is_ok_and(|o| o.present_on_battlefield())
                    || !self.printed_protection_from_everything(*source)
                {
                    return Err("wrong intrinsic protection source".into());
                }
                (Quantity::zero(), d.amount.clone())
            }
            EffectKey::IntrinsicProtectionFromColor { source, color } => {
                if d.recipient != (DamageRecipient::Creature { object: *source })
                    || !self
                        .current(*source)
                        .is_ok_and(|o| o.present_on_battlefield())
                    || !self
                        .effective_protection_colors(*source)
                        .any(|printed| printed == *color)
                {
                    return Err("wrong fixed-color protection source".into());
                }
                let damage_source = self.damage_source_appearance(d)?;
                if !damage_source.characteristics.colors.contains(color) {
                    return Err("damage source outside fixed-color protection".into());
                }
                qualification = Some(Box::new(DamageReplacementQualification {
                    effect_source: self.object_before(*source)?,
                    damage_source,
                }));
                (Quantity::zero(), d.amount.clone())
            }
            EffectKey::Resolved { id } => {
                let effect = self
                    .state
                    .rules
                    .effects
                    .iter()
                    .find(|e| e.id == *id)
                    .ok_or("missing resolved effect")?;
                match &effect.kind {
                    ResolvedEffectKind::ProtectControllerUntilNextTurn
                        if d.recipient
                            == (DamageRecipient::Player {
                                player: effect.origin.controller,
                            }) =>
                    {
                        (Quantity::zero(), d.amount.clone())
                    }
                    ResolvedEffectKind::GrantKeywordsSet {
                        targets, keywords, ..
                    } if Self::grant_prevents_damage(targets, keywords, d) => {
                        (Quantity::zero(), d.amount.clone())
                    }
                    ResolvedEffectKind::PreventCombatDamage if tx.kind == DamageKind::Combat => {
                        (Quantity::zero(), d.amount.clone())
                    }

                    _ => return Err("resolved effect not applicable".into()),
                }
            }
        };
        Ok(DamageApplication {
            qualification,
            damage: d.id,
            effect: pair.effect.clone(),
            before: d.amount.clone(),
            after,
            prevented,
        })
    }
    pub(crate) fn damage_effect_group(
        &self,
    ) -> Result<Option<(PlayerId, TargetEntity, Vec<DamageEffectCandidate>)>, String> {
        let Frame::DamageEffects { transaction: tx } = &self.state.rules.frame else {
            return Ok(None);
        };
        let mut order = self.state.rules.original_seating.clone();
        let anchor = order
            .iter()
            .position(|p| *p == self.state.rules.turn_owner)
            .ok_or("missing APNAP anchor")?;
        order.rotate_left(anchor);
        let mut recipients: Vec<_> = tx
            .entries
            .iter()
            .filter_map(|d| {
                self.recipient_chooser(&d.recipient)
                    .map(|p| (p, d.recipient.clone()))
            })
            .collect();
        recipients.sort_by_key(|(p, r)| {
            (
                order.iter().position(|q| q == p).unwrap_or(4),
                match r {
                    DamageRecipient::Player { player } => (0, u32::from(player.0), 0),
                    DamageRecipient::Creature { object } => (1, object.card.0, object.incarnation),
                },
            )
        });
        recipients.dedup();
        let keys = self.live_effect_keys();
        for (player, recipient) in recipients {
            let intrinsic = self.intrinsic_protection_keys(&recipient);
            let pairs = tx
                .entries
                .iter()
                .filter(|d| d.recipient == recipient)
                .flat_map(|d| {
                    keys.iter()
                        .chain(intrinsic.iter())
                        .map(move |k| DamageEffectCandidate {
                            effect: k.clone(),
                            damage: d.id,
                        })
                })
                .filter(|p| self.application(tx, p).is_ok())
                .collect::<Vec<_>>();
            if !pairs.is_empty() {
                return Ok(Some((player, recipient.entity(), pairs)));
            }
        }
        Ok(None)
    }
    fn automatic_pair(&self, pairs: &[DamageEffectCandidate]) -> bool {
        pairs
            .first()
            .is_some_and(|first| pairs.iter().all(|pair| pair.effect == first.effect))
    }

    pub(crate) fn damage_effect_request(&self) -> Result<Option<Decision<Rules>>, String> {
        let Some((player, recipient, pairs)) = self.damage_effect_group()? else {
            return Ok(None);
        };
        if self.automatic_pair(&pairs) {
            return Ok(None);
        }
        let candidates = pairs
            .into_iter()
            .map(|p| Candidate::DamageEffect {
                effect: p.effect,
                damage: p.damage,
            })
            .collect();
        Ok(Some(self.pick(
            player,
            Reason::DamageEffect { recipient },
            candidates,
            1,
            1,
        )))
    }

    pub(crate) fn damage_effect_step(&mut self) -> Result<(), String> {
        if let Some((_, _, pairs)) = self.damage_effect_group()? {
            let pair = if self.automatic_pair(&pairs) {
                self.rules_mut().pending = None;
                pairs[0].clone()
            } else {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing damage effect choice")?;
                let Candidate::DamageEffect { effect, damage } = input.one()?.clone() else {
                    return Err("damage effect response invariant".into());
                };
                DamageEffectCandidate { effect, damage }
            };
            let tx = self.damage_transaction()?.clone();

            self.commit(
                ProposedEvent::ApplyDamageEffect {
                    transaction: tx.id,
                    damage: pair.damage,
                    effect: pair.effect,
                },
                tx.cause,
            )?;
        } else {
            let tx = self.damage_transaction()?.clone();
            let actual =
                self.commit(ProposedEvent::CommitDamage { transaction: tx.id }, tx.cause)?;
            match tx.continuation {
                DamageContinuation::Combat => self.grant_turn_priority()?,
                DamageContinuation::Mana { activation } => self.resume_mana_riders(*activation)?,
                DamageContinuation::Instruction { frame } => self.finish_instruction(
                    *frame,
                    InstructionOutcome::Damage {
                        actual: Some(actual),
                    },
                )?,
            }
        }
        Ok(())
    }
    pub(super) fn revalidate_damage_departure(&mut self) -> Result<(), String> {
        if let Frame::DamageEffects { .. } = self.state.rules.frame
            && let Some(input) = &self.state.rules.pending
        {
            let valid = self.validate_pending(input).is_ok();
            if !valid {
                self.rules_mut().pending = None;
            }
        }
        Ok(())
    }
    fn reserve_effect_slot(&mut self) -> Result<(), String> {
        let requested = self.state.rules.effects.len() + 1;
        let capacity = self.effect_limit();
        if requested > capacity {
            return Err(self.capacity_error(RuleFault::EffectCapacityExceeded {
                capacity,
                requested,
            }));
        }
        Ok(())
    }
    pub(super) fn apply_effect_event(
        &mut self,
        event: &ProposedEvent,
    ) -> Result<EventOutcome, String> {
        match event {
            ProposedEvent::CreateEffect { effect } => {
                self.reserve_effect_slot()?;
                if effect.id != EffectId(self.state.rules.next_effect_id)
                    || effect.expires_in_cleanup_of_turn != self.state.rules.turn.number
                    || effect.origin.controller.0 >= 4
                    || effect.origin.source_handles.len() != 4
                {
                    return Err("invalid effect creation identity".into());
                }
                let frame = match (&self.state.rules.frame, &effect.kind) {
                    (Frame::Resolve { frame }, _) => frame,
                    _ => return Err("effect outside resolving instruction".into()),
                };
                if self.entry(frame.stack_id)?.kind.activated_index()
                    != effect.origin.activated_ability
                    || self.entry(frame.stack_id)?.source != effect.origin.source
                    || frame.stack_id != effect.origin.stack_id
                    || self.entry(frame.stack_id)?.controller != effect.origin.controller
                    || frame.cursor != effect.origin.cursor
                {
                    return Err("unbound continuous effect".into());
                }
                let instruction = self
                    .program_for(self.entry(frame.stack_id)?)?
                    .instructions
                    .get(frame.cursor as usize)
                    .ok_or("effect instruction absent")?;
                let valid = match (instruction, &effect.kind) {
                    (
                        Instruction::Kernel {
                            node: mtg_contracts::kir::Node::Do(verb),
                            source_block,
                        },
                        kind @ (ResolvedEffectKind::GainControl { .. }
                        | ResolvedEffectKind::SetCharacteristics { .. }
                        | ResolvedEffectKind::PumpAnnouncedX { .. }),
                    ) => {
                        *source_block == effect.origin.source_block
                            && self
                                .continuous_kind(
                                    frame,
                                    verb,
                                    self.state
                                        .rules
                                        .next_batch_id
                                        .checked_sub(1)
                                        .ok_or("missing effect creation batch")?,
                                )?
                                .as_ref()
                                == Some(kind)
                    }
                    (
                        kernel!(RestrictCombat { affects, restriction, .. } @ source_block),
                        kind @ ResolvedEffectKind::CombatProhibition { .. },
                    ) => {
                        *source_block == effect.origin.source_block
                            && self
                                .combat_prohibition_kind(frame, affects, *restriction)?
                                .as_ref()
                                == Some(kind)
                    }
                    (
                        i,
                        ResolvedEffectKind::PumpSet {
                            targets,
                            power: p,
                            toughness: t,
                        },
                    ) if mtg_contracts::ObjectSet::pumped(i).is_some() => i.pumped().is_some_and(
                        |(what, power, toughness, source_block)| {
                            matches!(what, mtg_contracts::kir::SetExpr::Matching { zone, filter }
                                    if self.kernel_set(frame, zone, filter)
                                        .is_ok_and(|(members, _)| *targets == members))
                                && p == power
                                && t == toughness
                                && source_block == effect.origin.source_block
                        },
                    ),

                    (
                        kernel!(ModifyUntil { what: source_object!(), change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block),
                        ResolvedEffectKind::ModifyPowerToughness {
                            target,
                            power: p,
                            toughness: t,
                        },
                    ) => {
                        *target == self.entry(frame.stack_id)?.source
                            && self.target_is_creature(*target)
                            && power == p
                            && toughness == t
                            && *source_block == effect.origin.source_block
                    }
                    (
                        kernel!(ModifyUntil {
                            what,
                            change: Change::AddKeywords { keywords },
                            until: mtg_contracts::kir::Until::EndOfTurn,
                            ..
                        } @ source_block),
                        kind @ (ResolvedEffectKind::GrantKeywords { .. }
                        | ResolvedEffectKind::GrantKeywordsSet { .. }),
                    ) => {
                        *source_block == effect.origin.source_block
                            && supported_temporary_keywords(keywords)
                            && self.keyword_grant_kind(frame, what, keywords)?.as_ref()
                                == Some(kind)
                    }

                    (
                        kernel!(ModifyUntil { what: target_object!(target_slot), change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block),
                        ResolvedEffectKind::ModifyPowerToughness {
                            target,
                            power: p,
                            toughness: t,
                        },
                    ) => {
                        self.entry(frame.stack_id)?
                            .targets
                            .get(*target_slot as usize)
                            == Some(target)
                            && frame.legal_targets.get(*target_slot as usize) == Some(&true)
                            && self.is_battlefield_creature(*target)
                            && power == p
                            && toughness == t
                            && *source_block == effect.origin.source_block
                    }

                    _ => false,
                };
                let valid_handles = match &self.entry(frame.stack_id)?.kind {
                    StackKind::Activated { source_handles, .. }
                    | StackKind::Triggered { source_handles, .. } => {
                        *source_handles == effect.origin.source_handles
                    }
                    StackKind::Spell => {
                        self.entry(frame.stack_id)?.effect_source_handles
                            == effect.origin.source_handles
                    }
                };
                let valid_target_handles = match &effect.kind {
                    ResolvedEffectKind::RemoveIndestructibleSet { targets, .. }
                    | ResolvedEffectKind::PumpSet { targets, .. }
                    | ResolvedEffectKind::GrantKeywordsSet { targets, .. }
                    | ResolvedEffectKind::GainControl { targets, .. }
                    | ResolvedEffectKind::SetCharacteristics { targets, .. }
                    | ResolvedEffectKind::PumpAnnouncedX { targets, .. }
                    | ResolvedEffectKind::ExileDamaged { targets } => {
                        targets.len() <= self.event_group_limit()
                            && effect.target_handles.len() == targets.len() * 4
                            && targets.iter().enumerate().all(|(i, o)| {
                                (0..4).all(|v| {
                                    self.object_handle_id(PlayerId(v), *o)
                                        == Ok(&effect.target_handles[i * 4 + v as usize])
                                })
                            })
                    }
                    ResolvedEffectKind::GrantKeywords { target, .. }
                    | ResolvedEffectKind::ModifyPowerToughness { target, .. }
                    | ResolvedEffectKind::CombatProhibition {
                        affects: CombatProhibitionScope::Target { target },
                        ..
                    } => {
                        effect.target_handles.len() == 4
                            && (0..4).all(|v| {
                                self.object_handle_id(PlayerId(v), *target)
                                    == Ok(&effect.target_handles[v as usize])
                            })
                    }

                    _ => effect.target_handles.is_empty(),
                };
                if !valid || !valid_handles || !valid_target_handles {
                    return Err("invalid continuous effect facts".into());
                }
                self.rules_mut().next_effect_id = self
                    .state
                    .rules
                    .next_effect_id
                    .checked_add(1)
                    .ok_or("effect identity limit")?;
                self.rules_mut().effects.push(*effect.clone());
                Ok(EventOutcome::EffectCreated {
                    effect: effect.clone(),
                })
            }
            ProposedEvent::ApplyDamageEffect {
                transaction,
                damage,
                effect,
            } => {
                let tx = self.damage_transaction()?;
                self.validate_damage_transaction(tx)?;
                if tx.id != *transaction {
                    return Err("wrong damage transaction".into());
                }
                let pair = DamageEffectCandidate {
                    effect: effect.clone(),
                    damage: *damage,
                };
                let Some((_, _, pairs)) = self.damage_effect_group()? else {
                    return Err("no applicable effects".into());
                };
                if !pairs.contains(&pair) {
                    return Err("wrong affected recipient/application".into());
                }

                let a = self.application(tx, &pair)?;

                let Frame::DamageEffects { transaction: tx } = &mut self.rules_mut().frame else {
                    return Err("missing damage frame".into());
                };
                let d = tx
                    .entries
                    .iter_mut()
                    .find(|d| d.id == *damage)
                    .ok_or("missing damage occurrence")?;
                d.amount = a.after.clone();
                d.applied.push(effect.clone());
                tx.applications.push(a.clone());
                Ok(EventOutcome::DamageEffectApplied {
                    application: Box::new(a),
                })
            }

            ProposedEvent::CommitDamage { transaction } => self.commit_actual_damage(*transaction),
            ProposedEvent::CleanupEffects => {
                let turn = self.state.rules.turn.number;
                let owner = self.state.rules.turn_owner;
                let expired = self
                    .state
                    .rules
                    .effects
                    .iter()
                    .filter(|e| !e.survives_cleanup(owner, turn))
                    .map(|e| e.id)
                    .collect();
                let previous = self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.marked_damage > Quantity::zero())
                    .map(|o| (o.id, o.marked_damage.clone()))
                    .collect();
                self.rules_mut()
                    .effects
                    .retain(|e| e.survives_cleanup(owner, turn));
                for o in self.rules_mut().objects.values_mut() {
                    o.marked_damage = Quantity::zero();
                    o.deathtouch_damage = false;
                }
                Ok(EventOutcome::EffectsCleaned { expired, previous })
            }
            _ => Err("not an effects event".into()),
        }
    }
    fn commit_actual_damage(&mut self, id: DamageTransactionId) -> Result<EventOutcome, String> {
        if !matches!(self.state.rules.frame, Frame::DamageEffects { .. }) {
            return Err("damage commit while allocation incomplete".into());
        }
        let tx = self.damage_transaction()?.clone();
        self.validate_damage_transaction(&tx)?;
        if tx.id != id || self.damage_effect_group()?.is_some() {
            return Err("incomplete damage transformation".into());
        }
        let mut actual = vec![];
        let mut source_appearances = BTreeMap::new();
        let mut cancelled = vec![];
        let mut totals: BTreeMap<(u8, u32), Quantity> = BTreeMap::new();
        let mut commanders: BTreeMap<(CardId, PlayerId), Quantity> = BTreeMap::new();
        for d in &tx.entries {
            if d.amount.is_negative() || d.original_amount <= Quantity::zero() {
                return Err("invalid final damage quantity".into());
            }
            if self.recipient_chooser(&d.recipient).is_none() {
                cancelled.push(d.id);
                continue;
            }
            if d.amount == Quantity::zero() {
                continue;
            }
            let key = match d.recipient {
                DamageRecipient::Player { player } => {
                    if tx.kind == DamageKind::Combat
                        && self
                            .state
                            .rules
                            .commanders
                            .history(d.source.object.card)
                            .is_some()
                    {
                        commanders
                            .entry((d.source.object.card, player))
                            .or_default()
                            .add(&d.amount)
                    }
                    (0, u32::from(player.0))
                }
                DamageRecipient::Creature { object } => (1, object.card.0),
            };
            totals.entry(key).or_default().add(&d.amount);
            source_appearances.insert(d.id, self.damage_source_appearance(d)?);
            actual.push(ActualDamage {
                id: d.id,
                source: d.source.clone(),
                controller: d.controller,
                recipient: d.recipient.clone(),
                amount: d.amount.clone(),
            });
        }
        // The entire group was validated above, before any recipient/history write.
        let mut results = vec![];
        for ((kind, id), amount) in totals {
            let (recipient, previous, current) = if kind == 0 {
                let player = PlayerId(id as u8);
                let (before, after) =
                    self.rules_mut().players[player.0 as usize].lose_life(&amount);
                (DamageRecipient::Player { player }, before, after)
            } else {
                let o = self
                    .rules_mut()
                    .objects
                    .get_mut(&CardId(id))
                    .ok_or("missing final damage object")?;
                let before = o.marked_damage.clone();
                o.marked_damage.add(&amount);
                (
                    DamageRecipient::Creature { object: o.id },
                    before,
                    o.marked_damage.clone(),
                )
            };
            results.push(DamageResult {
                recipient,
                previous,
                current,
            });
        }
        let mut commander_damage = vec![];
        for ((commander, recipient), amount) in commanders {
            let previous = self
                .state
                .rules
                .commanders
                .history(commander)
                .and_then(|h| h.combat_damage.get(&recipient))
                .cloned()
                .unwrap_or_default();
            self.rules_mut()
                .commanders
                .record_damage(commander, recipient, true, &amount)?;
            let mut current = previous.clone();
            current.add(&amount);
            commander_damage.push(CommanderDamageResult {
                commander,
                recipient,
                amount,
                previous,
                current,
            });
        }
        let life_gains = self.damage_keywords(&actual, Some(&source_appearances))?;
        Ok(EventOutcome::DamageCommitted {
            life_gains,
            transaction: id,
            actual,
            results,
            commander_damage,
            cancelled,
        })
    }
    pub(crate) fn validate_effect_state(&self) -> Result<(), String> {
        if self.state.rules.effects.len() > self.effect_limit() {
            return Err("effect capacity".into());
        }
        if !self
            .state
            .rules
            .effects
            .windows(2)
            .all(|rows| rows[0].id < rows[1].id)
        {
            return Err("effect creation order invariant".into());
        }
        let mut last_setter = None;
        for e in &self.state.rules.effects {
            if e.id.0 >= self.state.rules.next_effect_id
                || e.origin.controller.0 >= 4
                || if e.extends_beyond_creation_turn() {
                    e.expires_in_cleanup_of_turn == 0
                        || e.expires_in_cleanup_of_turn > self.state.rules.turn.number
                } else {
                    e.expires_in_cleanup_of_turn != self.state.rules.turn.number
                }
                || e.origin.source_handles.len() != 4
                || !self.state.rules.cards.contains_key(&e.origin.source.card)
            {
                return Err("effect state identity invariant".into());
            }
            if let ResolvedEffectKind::SetCharacteristics { timestamp, .. } = &e.kind {
                if *timestamp >= self.state.rules.next_batch_id
                    || last_setter.is_some_and(|previous| previous >= *timestamp)
                {
                    return Err("characteristic setter timestamp invariant".into());
                }
                last_setter = Some(*timestamp);
            }
            if e.origin.stack_id.0 >= self.state.rules.next_stack_id {
                return Err("effect origin stack identity invariant".into());
            }

            let programs = self.origin_programs(&e.origin)?;
            let instructions = programs
                .iter()
                .filter_map(|p| p.instructions.get(e.origin.cursor as usize));
            let mut bound = false;
            for instruction in instructions {
                bound |= match (instruction, &e.kind) {
                    (
                        kernel!(GainControl { what, controller, until } @ source_block),
                        ResolvedEffectKind::GainControl {
                            targets,
                            controller: recipient,
                            until: duration,
                        },
                    ) => {
                        *source_block == e.origin.source_block
                            && until == duration
                            && self.is_live(*recipient)
                            && match controller {
                                PlayerExpr::Is {
                                    who: Ref::Controller { .. },
                                } => *recipient == e.origin.controller,
                                PlayerExpr::Is {
                                    who: Ref::Target { .. },
                                } => true,
                                _ => false,
                            }
                            && self.valid_continuous_capture(e, what, targets)
                            && (*duration != Until::SourceLeaves
                                || self.control_source_remains(e.origin.source))
                            && targets.iter().any(|&object| {
                                self.current(object)
                                    .is_ok_and(|o| o.zone == Zone::Battlefield)
                            })
                    }
                    (
                        kernel!(SetCharacteristics { what, values, until: Until::EndOfTurn } @ source_block),
                        ResolvedEffectKind::SetCharacteristics {
                            targets,
                            values: saved,
                            timestamp,
                        },
                    ) => {
                        *source_block == e.origin.source_block
                            && values == saved
                            && *timestamp < self.state.rules.next_batch_id
                            && self.valid_continuous_capture(e, what, targets)
                    }
                    (
                        kernel!(ModifyUntil { what, change: Change::PowerToughnessX { power, toughness },
                            until: Until::EndOfTurn, .. } @ source_block),
                        ResolvedEffectKind::PumpAnnouncedX {
                            targets,
                            x,
                            modifier,
                        },
                    ) => {
                        *source_block == e.origin.source_block
                            && !x.is_negative()
                            && power.with_x(x) == modifier.power
                            && toughness.with_x(x) == modifier.toughness
                            && self.valid_continuous_capture(e, what, targets)
                            && targets.iter().all(|object| {
                                self.copiable_definition(object).is_ok_and(|d| {
                                    self.historical_creature_possible(&d.characteristics)
                                })
                            })
                    }
                    (
                        kernel!(RestrictCombat { affects, restriction, .. } @ source_block),
                        ResolvedEffectKind::CombatProhibition {
                            affects: saved_scope,
                            restriction: saved_restriction,
                        },
                    ) => {
                        use CombatProhibitionScope as S;
                        *source_block == e.origin.source_block
                            && restriction == saved_restriction
                            && match (affects, saved_scope) {
                                (
                                    S::Target {
                                        target: Ref::Target { .. },
                                    },
                                    S::Target { target },
                                ) => {
                                    e.target_handles.len() == 4
                                        && self.state.rules.cards.contains_key(&target.card)
                                }
                                (S::Creatures {}, S::Creatures {})
                                | (S::CreaturesWithoutFlying {}, S::CreaturesWithoutFlying {}) => {
                                    e.target_handles.is_empty()
                                }
                                _ => false,
                            }
                    }
                    (
                        kernel!(ModifyUntil { what: mtg_contracts::kir::SetExpr::Matching { .. }, change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block),
                        ResolvedEffectKind::PumpSet {
                            targets,
                            power: p,
                            toughness: t,
                        },
                    ) => {
                        p == power
                            && t == toughness
                            && *source_block == e.origin.source_block
                            && self.valid_set_effect_targets(e, targets, None)
                    }

                    (
                        kernel!(ModifyUntil { what: source_object!(), change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block),
                        ResolvedEffectKind::ModifyPowerToughness {
                            target,
                            power: p,
                            toughness: t,
                        },
                    ) => {
                        *target == e.origin.source
                            && power == p
                            && toughness == t
                            && *source_block == e.origin.source_block
                    }
                    (
                        kernel!(ModifyUntil {
                            what,
                            change: Change::AddKeywords { keywords },
                            until: mtg_contracts::kir::Until::EndOfTurn,
                            ..
                        } @ source_block),
                        ResolvedEffectKind::GrantKeywords {
                            target,
                            modifier: None,
                            keywords: grants,
                        },
                    ) => {
                        keywords == grants
                            && supported_temporary_keywords(grants)
                            && *source_block == e.origin.source_block
                            && e.target_handles.len() == 4
                            && self.state.rules.cards.contains_key(&target.card)
                            && match what {
                                SetExpr::One {
                                    object: Ref::Source,
                                } => {
                                    e.origin.activated_ability.is_some()
                                        && *target == e.origin.source
                                }
                                SetExpr::One {
                                    object: Ref::Target { .. },
                                } => self.historical_creature_possible(
                                    &self.copiable_definition(target)?.characteristics,
                                ),
                                _ => false,
                            }
                    }

                    (
                        kernel!(ModifyUntil {
                            what: SetExpr::Matching { zone: ZoneExpr::Battlefield, .. },
                            change: Change::AddKeywords { keywords },
                            until: mtg_contracts::kir::Until::EndOfTurn,
                            ..
                        } @ source_block),
                        ResolvedEffectKind::GrantKeywordsSet {
                            targets,
                            modifier: None,
                            keywords: grants,
                        },
                    ) => {
                        keywords == grants
                            && supported_temporary_keywords(grants)
                            && *source_block == e.origin.source_block
                            && !targets.is_empty()
                            // This structural check uses only the permanent
                            // kinds, while replay proves the captured filter.
                            && self.valid_set_effect_targets(
                                e,
                                targets,
                                Some(&mtg_contracts::ObjectSet::ControllerPermanents),
                            )
                    }

                    (
                        kernel!(ModifyUntil { what: target_object!(_), change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block),
                        ResolvedEffectKind::ModifyPowerToughness {
                            target,
                            power: p,
                            toughness: t,
                        },
                    ) => {
                        power == p
                            && toughness == t
                            && *source_block == e.origin.source_block
                            && e.target_handles.len() == 4
                            && self.state.rules.cards.contains_key(&target.card)
                            && self.historical_creature_possible(
                                &self.copiable_definition(target)?.characteristics,
                            )
                    }

                    _ => false,
                };
            }
            if !bound {
                return Err("effect definition/provenance invariant".into());
            }
        }

        if let Frame::DamageEffects { transaction: tx } = &self.state.rules.frame {
            self.validate_damage_transaction(tx)?;
            for (i, d) in tx.entries.iter().enumerate() {
                let unique = d.applied.iter().collect::<BTreeSet<_>>();
                if d.id != DamageId(i as u32)
                    || d.original_amount <= Quantity::zero()
                    || d.amount.is_negative()
                    || d.controller.0 >= 4
                    || d.source.controller.0 >= 4
                    || unique.len() != d.applied.len()
                    || !self.state.rules.cards.contains_key(&d.source.object.card)
                {
                    return Err("prepared damage invariant".into());
                }
            }
        }
        Ok(())
    }

    fn valid_continuous_capture(
        &self,
        effect: &ResolvedEffect,
        what: &SetExpr,
        targets: &[ObjectId],
    ) -> bool {
        !targets.is_empty()
            && self.valid_set_effect_targets(
                effect,
                targets,
                Some(&mtg_contracts::ObjectSet::ControllerPermanents),
            )
            && targets.iter().all(|id| {
                self.state.rules.objects.get(&id.card).map_or_else(
                    || {
                        self.state.rules.tokens.contains_key(&id.card)
                            || !self.is_live(self.state.rules.cards[&id.card].owner)
                    },
                    |current| {
                        id.incarnation < current.id.incarnation
                            || (current.id == *id && current.zone == Zone::Battlefield)
                    },
                )
            })
            && match what {
                SetExpr::One {
                    object: Ref::Source,
                } => targets == [effect.origin.source],
                SetExpr::One {
                    object: Ref::Target { .. },
                } => targets.len() == 1,
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    ..
                } => targets.windows(2).all(|w| w[0].card < w[1].card),
                _ => false,
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageTransactionBefore {
    pub id: DamageTransactionId,
    pub kind: DamageKind,
    pub entries: Vec<PreparedDamage>,
    pub applications: Vec<DamageApplication>,
}
impl From<&DamageTransaction> for DamageTransactionBefore {
    fn from(tx: &DamageTransaction) -> Self {
        Self {
            id: tx.id,
            kind: tx.kind,
            entries: tx.entries.clone(),
            applications: tx.applications.clone(),
        }
    }
}

impl RulesTxn {
    /// Returns the live keys it published handles for. They depend on rules
    /// state alone, so one publication reuses them for every viewer.
    pub(crate) fn refresh_effect_handles(&mut self) -> Result<Vec<EffectKey>, String> {
        let keys = self.live_effect_keys();
        // A printed keyword is already on the public creature. Only a pending
        // damage occurrence needs a command handle for the affected controller.
        let mut intrinsic: [Vec<EffectKey>; 4] = Default::default();
        if let Ok(tx) = self.damage_transaction() {
            for d in &tx.entries {
                let Some(controller) = self.recipient_chooser(&d.recipient) else {
                    continue;
                };
                if d.amount <= Quantity::zero() {
                    continue;
                }
                let handles = intrinsic
                    .get_mut(controller.0 as usize)
                    .ok_or("invalid protection controller")?;
                for key in self.intrinsic_protection_keys(&d.recipient) {
                    let candidate = DamageEffectCandidate {
                        effect: key.clone(),
                        damage: d.id,
                    };
                    if !handles.contains(&key) && self.application(tx, &candidate).is_ok() {
                        handles.push(key);
                    }
                }
            }
        }
        let damages = self
            .damage_transaction()
            .ok()
            .map(|tx| tx.entries.iter().map(|d| (tx.id, d.id)).collect::<Vec<_>>())
            .unwrap_or_default();
        for (v, intrinsic) in intrinsic.iter().enumerate() {
            let k = &mut self.state.knowledge.visibility[v];
            k.effect_handles
                .retain(|(key, _)| keys.contains(key) || intrinsic.contains(key));
            k.damage_handles.retain(|(key, _)| damages.contains(key));
            for key in keys.iter().chain(intrinsic) {
                if !k.effect_handles.iter().any(|(id, _)| id == key) {
                    let handle = publish::HandleId::new(
                        v as u8,
                        k.next_effect_handle,
                        k.namespace.as_deref(),
                    );
                    k.next_effect_handle = k
                        .next_effect_handle
                        .checked_add(1)
                        .ok_or("effect handle limit")?;
                    k.effect_handles.push((key.clone(), handle));
                }
            }
            for key in &damages {
                if !k.damage_handles.iter().any(|(id, _)| id == key) {
                    let handle = DamageHandle(k.scoped(format!("v{v}-d{}", k.next_damage_handle)));
                    k.next_damage_handle = k
                        .next_damage_handle
                        .checked_add(1)
                        .ok_or("damage handle limit")?;
                    k.damage_handles.push((*key, handle));
                }
            }
        }
        Ok(keys)
    }
    pub(crate) fn effect_handle(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<EffectHandle, String> {
        self.effect_handle_id(viewer, key).map(EffectHandle::from)
    }

    pub(super) fn effect_handle_id(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<&publish::HandleId<b'e'>, String> {
        self.state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| {
                k.effect_handles
                    .iter()
                    .find(|(id, _)| id == key)
                    .map(|(_, h)| h)
            })
            .ok_or_else(|| "unknown visible effect".into())
    }
    fn effect_origin_view<O: for<'a> From<&'a publish::StoredObjectHandle>>(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<VisibleEffectSource<O>, String> {
        let (source, controller, announced) = match key {
            EffectKey::IntrinsicProtectionFromEverything { source }
            | EffectKey::IntrinsicProtectionFromColor { source, .. } => (
                *source,
                self.current(*source)?.controller,
                self.object_handle_id(viewer, *source).map(O::from)?,
            ),
            EffectKey::Resolved { id } => {
                let e = self
                    .state
                    .rules
                    .effects
                    .iter()
                    .find(|e| e.id == *id)
                    .ok_or("missing visible effect")?;
                (
                    e.origin.source,
                    e.origin.controller,
                    e.origin
                        .source_handles
                        .get(viewer.0 as usize)
                        .ok_or("missing effect source reference")?
                        .into(),
                )
            }
        };
        Ok(VisibleEffectSource {
            source: VisibleTarget {
                announced,
                current: self.object_handle_id(viewer, source).map(O::from).ok(),
            },
            oracle_id: if matches!(key, EffectKey::IntrinsicProtectionFromColor { .. }) {
                self.visible_ability_oracle_id(source)?.into()
            } else {
                self.state
                    .rules
                    .cards
                    .get(&source.card)
                    .ok_or("missing physical effect source")?
                    .oracle_id
                    .clone()
            },
            controller,
        })
    }
    pub(crate) fn visible_effect(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<VisibleEffect, String> {
        self.project_effect(viewer, key)
    }

    fn project_effect<
        H: for<'a> From<&'a publish::HandleId<b'e'>>,
        O: for<'a> From<&'a publish::StoredObjectHandle>,
    >(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<VisibleEffect<H, O>, String> {
        let handle = H::from(self.effect_handle_id(viewer, key)?);
        let origin = self.effect_origin_view(viewer, key)?;
        let EffectKey::Resolved { id } = key else {
            return Err("intrinsic prevention is visible on the damage occurrence".into());
        };
        let e = self
            .state
            .rules
            .effects
            .iter()
            .find(|e| e.id == *id)
            .ok_or("missing effect row")?;
        let target_view = |object| -> Result<VisibleTarget<O>, String> {
            Ok(VisibleTarget {
                announced: e
                    .target_handles
                    .get(viewer.0 as usize)
                    .ok_or("missing historical effect target")?
                    .into(),
                current: self.object_handle_id(viewer, object).map(O::from).ok(),
            })
        };
        let set_views = |targets: &[ObjectId]| -> Result<Vec<VisibleTarget<O>>, String> {
            targets
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    Ok(VisibleTarget {
                        announced: e
                            .target_handles
                            .get(i * 4 + viewer.0 as usize)
                            .ok_or("missing set historical handle")?
                            .into(),
                        current: self.object_handle_id(viewer, *o).map(O::from).ok(),
                    })
                })
                .collect()
        };
        Ok(match &e.kind {
            ResolvedEffectKind::GainControl {
                targets,
                controller,
                until,
            } => VisibleEffect::GainControl {
                handle,
                origin,
                targets: set_views(targets)?,
                controller: *controller,
                while_source_remains: *until == Until::SourceLeaves,
                expires_in_cleanup_of_turn: (*until == Until::EndOfTurn)
                    .then_some(e.expires_in_cleanup_of_turn),
            },
            ResolvedEffectKind::SetCharacteristics {
                targets, values, ..
            } => VisibleEffect::SetCharacteristics {
                handle,
                origin,
                targets: set_views(targets)?,
                values: values.clone(),
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::PumpAnnouncedX {
                targets, modifier, ..
            } => VisibleEffect::PumpSet {
                handle,
                origin,
                targets: set_views(targets)?,
                power: modifier.power.clone(),
                toughness: modifier.toughness.clone(),
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::CombatProhibition {
                affects,
                restriction,
            } => {
                use CombatProhibitionScope as S;
                VisibleEffect::CombatProhibition {
                    handle,
                    origin,
                    affects: match affects {
                        S::Target { target } => S::Target {
                            target: target_view(*target)?,
                        },
                        S::Creatures {} => S::Creatures {},
                        S::CreaturesWithoutFlying {} => S::CreaturesWithoutFlying {},
                    },
                    restriction: *restriction,
                    expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
                }
            }
            ResolvedEffectKind::RegenerationShield => VisibleEffect::RegenerationShield {
                handle,
                origin,
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },

            ResolvedEffectKind::RemoveIndestructibleSet { targets, .. } => {
                VisibleEffect::RemoveIndestructibleSet {
                    handle,
                    origin,
                    targets: set_views(targets)?,
                    expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
                }
            }
            ResolvedEffectKind::PumpSet {
                targets,
                power,
                toughness,
            } => VisibleEffect::PumpSet {
                handle,
                origin,
                targets: set_views(targets)?,
                power: power.clone(),
                toughness: toughness.clone(),
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::GrantKeywordsSet {
                targets,
                modifier,
                keywords,
            } => VisibleEffect::GrantKeywordsSet {
                handle,
                origin,
                targets: set_views(targets)?,
                modifier: modifier.clone(),
                keywords: keywords.clone(),
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::ExileDamaged { targets } => VisibleEffect::ExileDamaged {
                handle,
                origin,
                targets: set_views(targets)?,
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::GrantKeywords {
                target,
                modifier,
                keywords,
            } => VisibleEffect::GrantKeywords {
                handle,
                origin,
                target: target_view(*target)?,
                modifier: modifier.clone(),
                keywords: keywords.clone(),
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::ModifyPowerToughness {
                target,
                power,
                toughness,
            } => VisibleEffect::ModifyPowerToughness {
                handle,
                origin,
                target: target_view(*target)?,
                power: power.clone(),
                toughness: toughness.clone(),
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::ProtectControllerUntilNextTurn => {
                VisibleEffect::ProtectControllerUntilNextTurn { handle, origin }
            }
            ResolvedEffectKind::PreventCombatDamage => VisibleEffect::PreventCombatDamage {
                handle,
                origin,
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::AdditionalLandPlays { count } => {
                VisibleEffect::AdditionalLandPlays {
                    handle,
                    origin,
                    count: count.clone(),
                    expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
                }
            }
        })
    }
    #[cfg(feature = "testkit")]
    pub(crate) fn visible_effects(&self, viewer: PlayerId) -> Result<Vec<VisibleEffect>, String> {
        self.visible_effects_from(viewer, &self.live_effect_keys())
    }
    /// `keys` are `live_effect_keys()` of this unchanged rules state.
    pub(super) fn visible_effects_from<
        H: for<'a> From<&'a publish::HandleId<b'e'>>,
        O: for<'a> From<&'a publish::StoredObjectHandle>,
    >(
        &self,
        viewer: PlayerId,
        keys: &[EffectKey],
    ) -> Result<Vec<VisibleEffect<H, O>>, String> {
        keys.iter()
            .map(|key| self.project_effect(viewer, key))
            .collect()
    }
    pub(crate) fn visible_damage_effect_candidate(
        &self,
        viewer: PlayerId,
        pair: &DamageEffectCandidate,
    ) -> Result<VisibleDamageEffectCandidate, String> {
        let tx = self.damage_transaction()?;
        let a = self.application(tx, pair)?;
        let d = tx
            .entries
            .iter()
            .find(|d| d.id == pair.damage)
            .ok_or("missing damage occurrence")?;
        let kind = match pair.effect {
            EffectKey::IntrinsicProtectionFromEverything { .. } => {
                DamageEffectKind::ProtectionFromEverything
            }
            EffectKey::IntrinsicProtectionFromColor { .. } => DamageEffectKind::ProtectionFromColor,
            EffectKey::Resolved { id } => match self
                .state
                .rules
                .effects
                .iter()
                .find(|e| e.id == id)
                .ok_or("missing applicable effect")?
                .kind
            {
                ResolvedEffectKind::GrantKeywordsSet { .. } => {
                    DamageEffectKind::ProtectionFromEachColor
                }
                ResolvedEffectKind::ProtectControllerUntilNextTurn => {
                    DamageEffectKind::ProtectionFromEverything
                }
                ResolvedEffectKind::PreventCombatDamage => DamageEffectKind::PreventCombatDamage,

                _ => return Err("non-damage effect in choice".into()),
            },
        };
        // The grant supplies the durable effect handle. The creature that
        // actually has protection is the source of this prevention ability.
        let origin = if kind == DamageEffectKind::ProtectionFromEachColor {
            let DamageRecipient::Creature { object } = d.recipient else {
                return Err("protection requires a creature recipient".into());
            };
            let handle = self.handle_for(viewer, object)?;
            VisibleEffectSource {
                source: VisibleTarget {
                    announced: handle.clone(),
                    current: Some(handle),
                },
                oracle_id: self.definition(object.card)?.oracle_id.clone(),
                controller: self.current(object)?.controller,
            }
        } else {
            self.effect_origin_view(viewer, &pair.effect)?
        };

        let label = match pair.effect {
            EffectKey::IntrinsicProtectionFromColor { color, .. } => {
                fixed_color_protection::color_name(color)
                    .map(|name| format!("Protection from {name}"))
            }
            _ => self
                .effect_decision_context(viewer, &pair.effect)
                .and_then(|c| c.instruction.map(|i| i.text)),
        };
        Ok(VisibleDamageEffectCandidate {
            label,
            effect: self.effect_handle(viewer, &pair.effect)?,
            damage: self
                .state
                .knowledge
                .visibility
                .get(viewer.0 as usize)
                .and_then(|k| {
                    k.damage_handles
                        .iter()
                        .find(|(id, _)| *id == (tx.id, pair.damage))
                        .map(|(_, h)| h.clone())
                })
                .ok_or("missing damage handle")?,
            kind,
            effect_source: origin.source,
            effect_oracle_id: origin.oracle_id,
            effect_controller: origin.controller,
            damage_source: VisibleTarget {
                announced: d
                    .source_handles
                    .get(viewer.0 as usize)
                    .ok_or("missing damage source reference")?
                    .into(),
                current: self.handle_for(viewer, d.source.object).ok(),
            },
            damage_oracle_id: self
                .state
                .rules
                .cards
                .get(&d.source.object.card)
                .ok_or("missing physical damage source")?
                .oracle_id
                .clone(),
            amount_before: a.before.clone(),
            amount_after: a.after,
        })
    }
    pub(crate) fn resolve_damage_effect_pair(
        &self,
        viewer: PlayerId,
        effect: &EffectHandle,
        damage: &DamageHandle,
    ) -> Result<(EffectKey, DamageId), String> {
        let k = self
            .state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .ok_or("invalid viewer")?;
        let handle = publish::HandleId::parse(&effect.0);
        let effect = k
            .effect_handles
            .iter()
            .find(|(_, h)| *h == handle)
            .map(|(k, _)| k.clone())
            .ok_or("invalid effect handle")?;
        let tx = self.damage_transaction()?;
        let damage = k
            .damage_handles
            .iter()
            .find(|((id, _), h)| *id == tx.id && h == damage)
            .map(|((_, d), _)| *d)
            .ok_or("invalid damage handle")?;
        Ok((effect, damage))
    }
}

impl RulesTxn {
    fn characteristic_set_vocabulary(&self) -> impl Iterator<Item = &CharacteristicSet> {
        self.prepared.catalog.definitions.iter().flat_map(|d| {
            d.spell_programs()
                .chain(d.activated.iter().map(|ability| &ability.program))
                .chain(d.triggered.iter().map(|ability| &ability.program))
                .flat_map(|p| &p.instructions)
                .filter_map(|i| match i.verb() {
                    Some(mtg_contracts::kir::Verb::SetCharacteristics { values, .. }) => {
                        Some(values)
                    }
                    _ => None,
                })
        })
    }

    /// Structural lower bound for witnesses that retain only an identity.
    /// The replay journal authenticates the exact layer-4 history.
    pub(super) fn historical_creature_possible(&self, printed: &Characteristics) -> bool {
        printed.has_type(CardKind::Creature)
            || (!matches!(printed.kind, CardKind::Instant | CardKind::Sorcery)
                && self
                    .characteristic_set_vocabulary()
                    .any(|values| values.animation.is_some()))
    }

    // Copiable identity is immutable within each incarnation. Independent
    // replay proves the continuous changes present at the capture boundary.
    pub(crate) fn validate_characteristic_identity(
        &self,
        source: ObjectId,
        c: &Characteristics,
    ) -> Result<(), String> {
        let mut base = self.copiable_definition(source)?.characteristics.clone();
        if c.has_type(CardKind::Creature) != c.power.is_some()
            || c.power.is_some() != c.toughness.is_some()
        {
            return Err("modified source has invalid stat shape".into());
        }
        // Replay authenticates which provider existed at the original capture.
        base.power = c.power.clone();
        base.toughness = c.toughness.clone();
        if base.additional_types != c.additional_types || base.colors != c.colors {
            let vocabulary = || {
                self.prepared
                    .catalog
                    .definitions
                    .iter()
                    .flat_map(|d| &d.static_abilities)
                    .filter_map(|a| match &a.effect {
                        StaticEffect::Continuous { changes, .. } => Some(changes),
                        _ => None,
                    })
                    .flatten()
            };
            if base.additional_types.iter().any(|kind| !c.has_type(*kind))
                || c.additional_types.contains(&c.kind)
                || c.additional_types.windows(2).any(|kinds| kinds[0] >= kinds[1])
                || c.additional_types.iter().any(|kind| {
                    !base.has_type(*kind)
                        && !vocabulary().any(|change| matches!(change, Change::AddTypes { kinds } if kinds.contains(kind)))
                        && !(!matches!(base.kind, CardKind::Instant | CardKind::Sorcery)
                            && c.has_type(CardKind::Creature)
                            && self.characteristic_set_vocabulary().any(|values| {
                                values.animation.as_ref().is_some_and(|a| match kind {
                                    CardKind::Creature => true,
                                    CardKind::Artifact => a.add_artifact,
                                    _ => false,
                                })
                            }))
                })
                || (base.colors != c.colors
                    && !vocabulary().any(|change| matches!(change, Change::SetColors { colors } if *colors == c.colors))
                    && !self.characteristic_set_vocabulary().any(|values| values.colors.as_ref() == Some(&c.colors)))
            {
                return Err("historical type or color outside admitted static changes".into());
            }
        }
        if base.subtypes != c.subtypes {
            let animations = || {
                self.characteristic_set_vocabulary()
                    .filter_map(|value| value.animation.as_ref())
            };
            if !c.has_type(CardKind::Creature)
                || c.subtypes.iter().any(|s| {
                    !base.subtypes.contains(s)
                        && !animations().any(|a| a.creature_subtypes.contains(s))
                })
                || base.subtypes.iter().any(|s| {
                    !c.subtypes.contains(s)
                        && (!mtg_contracts::kir::is_creature_subtype(s)
                            || !animations().any(|a| a.replace_creature_subtypes))
                })
            {
                return Err("historical subtype outside admitted animation".into());
            }
            base.subtypes.clone_from(&c.subtypes);
        }
        base.additional_types.clone_from(&c.additional_types);
        base.colors.clone_from(&c.colors);
        if base != *c {
            return Err("modified source changed printed identity".into());
        }
        Ok(())
    }
    // CR 104.3a/800.4a, 510.2, 608.2h: from attachments-v1 an Aura owned by
    // another player can leave with its conceding owner while prepared damage
    // awaits a choice. The frozen characteristics and keywords stay
    // authoritative; departure-invariant facts must still match.
    pub(super) fn frozen_damage_source_matches(
        &self,
        frozen: &ObjectBefore,
        current: ObjectBefore,
    ) -> bool {
        ObjectBefore {
            characteristics: frozen.characteristics.clone(),
            keywords: frozen.keywords.clone(),
            ..current
        } == *frozen
    }
    fn validate_source_appearance(
        &self,
        damage: &PreparedDamage,
        fact: &ObjectBefore,
    ) -> Result<(), String> {
        self.validate_characteristic_identity(fact.object, &fact.characteristics)?;
        if fact.controller.0 >= 4
            || fact.default_controller.0 >= 4
            || fact.marked_damage.is_negative()
            || self
                .state
                .rules
                .cards
                .get(&fact.object.card)
                .is_none_or(|c| c.owner != fact.owner)
            || (fact.attack_ready && !damage.source.attack_ready)
            || (fact.controller != damage.source.controller && fact.zone != Zone::Battlefield)
            || (ObjectBefore {
                controller: fact.controller,
                attack_ready: fact.attack_ready,
                characteristics: fact.characteristics.clone(),
                keywords: fact.keywords.clone(),
                ..damage.source.clone()
            }) != *fact
        {
            return Err("invalid replacement source identity/consumed facts".into());
        }
        Ok(())
    }
    pub(super) fn prepared_damage_source_matches(
        &self,
        damage: &PreparedDamage,
        current: ObjectBefore,
    ) -> bool {
        if self.frozen_damage_source_matches(&damage.source, current.clone()) {
            return true;
        }
        // Only the bounded fact refreshed by a typed ControlChanged can
        // justify changed control/readiness. Every other consumed fact remains
        // bound, and coherent historical lies still require input replay.
        damage.source_appearance.as_deref().is_some_and(|latest| {
            self.validate_source_appearance(damage, latest).is_ok()
                && self.frozen_damage_source_matches(latest, current)
        })
    }
    pub(crate) fn validate_damage_transaction(&self, tx: &DamageTransaction) -> Result<(), String> {
        let existing_limit = self
            .damage_limit()
            .saturating_mul(self.effect_limit().saturating_add(2));
        if tx.entries.is_empty()
            || tx.entries.len() > self.damage_limit()
            // At most one additional intrinsic source per original occurrence.
            || tx.applications.len() > existing_limit.saturating_add(tx.entries.len())
            || (tx.applications.len() > existing_limit && tx.applications.iter().filter(|a|
                !matches!(a.effect, EffectKey::IntrinsicProtectionFromEverything { .. }
                    | EffectKey::IntrinsicProtectionFromColor { .. })
            ).count() > existing_limit)

            || tx.id.0 >= self.state.rules.next_damage_transaction_id
        {
            return Err("damage transaction resource/identity invariant".into());
        }
        if tx.kind == DamageKind::Combat {
            Self::validate_combat_player_members(
                tx.entries.iter().map(|d| (d.source.object, &d.recipient)),
            )?;
        }
        let mut powers: BTreeMap<ObjectId, Quantity> = BTreeMap::new();
        for (i, d) in tx.entries.iter().enumerate() {
            let physical = self
                .state
                .rules
                .cards
                .get(&d.source.object.card)
                .ok_or("missing damage physical source")?;
            if d.id != DamageId(i as u32)
                || d.source.owner != physical.owner
                || d.controller.0 >= 4
                || d.source.controller.0 >= 4
                || d.source.default_controller.0 >= 4
                || d.source_handles.len() != 4
                || d.original_amount <= Quantity::zero()
                || d.amount.is_negative()
            {
                return Err("damage source identity/quantity invariant".into());
            }
            self.validate_characteristic_identity(d.source.object, &d.source.characteristics)?;
            if let Some(fact) = &d.source_appearance {
                self.validate_source_appearance(d, fact)?;
            }
            let source_definition = self.copiable_definition(d.source.object)?;
            let expected_keywords = source_definition.keywords.as_slice();
            // This bounds the vocabulary of possible LKI grants, not their
            // occurrence. Departure-invariant facts below and restore's input
            // replay still authenticate the captured historical appearance.
            // Validated ordinary spell grants belong here alongside static ones.
            let battlefield_creature = d.source.zone == Zone::Battlefield
                && d.source.characteristics.has_type(CardKind::Creature);
            let keywords_valid = expected_keywords.iter().all(|k| d.source.keywords.contains(k))
                && d.source.keywords.iter().enumerate().all(|(index, keyword)| {
                    !d.source.keywords[..index].contains(keyword)
                        && (expected_keywords.contains(keyword)
                            || (battlefield_creature && (source_definition.activated.iter()
                                .filter_map(mtg_contracts::activated_source_keyword::keywords)
                                .any(|keywords| keywords.contains(keyword))
                                || self.prepared.catalog.definitions.iter().any(|definition|
                                    definition.static_abilities.iter().any(|a| matches!(&a.effect,
                                        StaticEffect::Continuous { changes, .. } if changes.iter().any(|change|
                                            matches!(change, Change::AddKeywords { keywords } if keywords.contains(keyword)))))))))
                });
            if !keywords_valid {
                return Err(
                    "damage source keyword LKI outside admitted intrinsic/granted abilities".into(),
                );
            }
            if let Ok(current) = self.object_before(d.source.object)
                && !self.prepared_damage_source_matches(d, current)
            {
                return Err("prepared damage source changed consumed facts".into());
            }
            if tx.kind == DamageKind::Combat {
                if d.source.zone != Zone::Battlefield
                    || d.source.characteristics.kind != CardKind::Creature
                    || d.source.controller != d.controller
                {
                    return Err("invalid assigned combat source".into());
                }
                powers
                    .entry(d.source.object)
                    .or_default()
                    .add(&d.original_amount);
            }
        }
        match (&tx.kind, &tx.continuation) {
            (DamageKind::Combat, DamageContinuation::Combat) => {
                if tx.cause != BatchCause::Combat {
                    return Err("wrong combat damage cause".into());
                }
                for (source, assigned) in powers {
                    let fact = &tx
                        .entries
                        .iter()
                        .find(|d| d.source.object == source)
                        .ok_or("missing combat source")?
                        .source;
                    let power = fact
                        .characteristics
                        .power
                        .as_ref()
                        .ok_or("missing combat power")?;
                    if &assigned != power {
                        return Err("combat assigned total differs from frozen power".into());
                    }
                }
            }
            (DamageKind::Noncombat, DamageContinuation::Mana { activation }) => {
                self.validate_mana_rider_damage(tx, activation)?;
            }
            (DamageKind::Noncombat, DamageContinuation::Instruction { frame }) => {
                self.validate_resolving(frame)?;
                let instruction = self
                    .program_for(self.entry(frame.stack_id)?)?
                    .instructions
                    .get(frame.cursor as usize)
                    .ok_or("missing damage continuation instruction")?;
                if instruction.set_damage().is_some() {
                    self.validate_wipe_damage_transaction(tx, frame)?;
                } else {
                    let (target_slot, amount, source_block, recipient) = match instruction {
                        kernel!(Damage {
                            to: mtg_contracts::kir::DamageTo::Ref {
                                who: mtg_contracts::kir::Ref::Target { slot: _ }
                            },
                            amount: fixed!(_),
                            ..
                        })
                        | kernel!(Damage {
                            to: mtg_contracts::kir::DamageTo::Ref {
                                who: mtg_contracts::kir::Ref::Target { slot: _ }
                            },
                            amount: announced_x!(),
                            ..
                        }) => {
                            let (slot, amount, block, target) = self.directed_damage_operands(
                                self.entry(frame.stack_id)?,
                                instruction,
                            )?;
                            (Some(slot), amount, block, target)
                        }

                        kernel!(Damage { to: mtg_contracts::kir::DamageTo::Ref { who: mtg_contracts::kir::Ref::Controller { .. } }, amount: fixed!(amount), .. } @ source_block) => {
                            (
                                None,
                                amount,
                                *source_block,
                                TargetEntity::Player {
                                    player: self.entry(frame.stack_id)?.controller,
                                },
                            )
                        }
                        _ => return Err("damage continuation not damage instruction".into()),
                    };
                    let expected = BatchCause::Instruction {
                        stack_id: frame.stack_id,
                        player: self.entry(frame.stack_id)?.controller,
                        source: self.entry(frame.stack_id)?.source,
                        cursor: frame.cursor,
                        source_block,
                    };
                    let d = tx.entries.first().ok_or("empty noncombat damage")?;
                    if tx.entries.len() != 1
                        || tx.cause != expected
                        || d.original_amount != *amount
                        || recipient != d.recipient.entity()
                        || target_slot.is_some_and(|slot| {
                            frame.legal_targets.get(slot as usize) != Some(&true)
                        })
                        || d.controller != self.entry(frame.stack_id)?.controller
                        || !self.prepared_damage_source_matches(
                            d,
                            self.source_facts(self.entry(frame.stack_id)?)?,
                        )
                    {
                        return Err("unbound noncombat damage transaction".into());
                    }
                }
            }
            _ => return Err("wrong damage continuation kind".into()),
        }
        let mut amounts = tx
            .entries
            .iter()
            .map(|d| (d.id, d.original_amount.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut used: BTreeMap<DamageId, Vec<EffectKey>> = BTreeMap::new();
        for a in &tx.applications {
            let d = tx
                .entries
                .iter()
                .find(|d| d.id == a.damage)
                .ok_or("unknown application occurrence")?;
            let before = amounts
                .get_mut(&a.damage)
                .ok_or("missing application amount")?;
            let uses = used.entry(a.damage).or_default();
            if *before != a.before || a.before <= Quantity::zero() || uses.contains(&a.effect) {
                return Err("repeated/out-of-order damage effect prefix".into());
            }
            let (after, prevented) = match &a.effect {
                EffectKey::IntrinsicProtectionFromEverything { source } => {
                    // The recipient may since have departed. The original
                    // occurrence binds its incarnation and retained copiable
                    // identity. Restore replays the accepted inputs.
                    if d.recipient != (DamageRecipient::Creature { object: *source })
                        || !self.printed_protection_from_everything(*source)
                        || a.qualification.is_some()
                    {
                        return Err("invalid intrinsic prevention prefix".into());
                    }
                    (Quantity::zero(), a.before.clone())
                }
                EffectKey::IntrinsicProtectionFromColor { source, color } => {
                    let qualification = a
                        .qualification
                        .as_deref()
                        .ok_or("missing consumed fixed-color protection facts")?;
                    let protector = &qualification.effect_source;
                    self.validate_characteristic_identity(
                        protector.object,
                        &protector.characteristics,
                    )?;
                    self.validate_source_appearance(d, &qualification.damage_source)?;
                    // Replay proves the already-consumed recipient and damage-time
                    // colors even when either object or its color provider left.
                    if d.recipient != (DamageRecipient::Creature { object: *source })
                        || protector.object != *source
                        || protector.zone != Zone::Battlefield
                        || !protector.characteristics.has_type(CardKind::Creature)
                        || protector.controller.0 >= 4
                        || protector.default_controller.0 >= 4
                        || protector.marked_damage.is_negative()
                        || self
                            .state
                            .rules
                            .cards
                            .get(&source.card)
                            .is_none_or(|card| card.owner != protector.owner)
                        || !protector
                            .keywords
                            .iter()
                            .any(|k| k.fixed_protection_color() == Some(*color))
                        || !qualification
                            .damage_source
                            .characteristics
                            .colors
                            .contains(color)
                    {
                        return Err("invalid fixed-color prevention prefix".into());
                    }
                    (Quantity::zero(), a.before.clone())
                }
                EffectKey::Resolved { id } => {
                    if a.qualification.is_some() {
                        return Err(
                            "resolved damage effect cannot carry protection qualification".into(),
                        );
                    }
                    let e = self
                        .state
                        .rules
                        .effects
                        .iter()
                        .find(|e| e.id == *id)
                        .ok_or("missing applied resolved effect")?;
                    match &e.kind {
                        ResolvedEffectKind::GrantKeywordsSet {
                            targets, keywords, ..
                        } if Self::grant_prevents_damage(targets, keywords, d) => {
                            (Quantity::zero(), a.before.clone())
                        }
                        ResolvedEffectKind::ProtectControllerUntilNextTurn
                            if d.recipient
                                == (DamageRecipient::Player {
                                    player: e.origin.controller,
                                }) =>
                        {
                            (Quantity::zero(), a.before.clone())
                        }
                        ResolvedEffectKind::PreventCombatDamage
                            if tx.kind == DamageKind::Combat =>
                        {
                            (Quantity::zero(), a.before.clone())
                        }

                        _ => return Err("applied effect wrong domain".into()),
                    }
                }
            };
            if a.after != after || a.prevented != prevented {
                return Err("invalid transformed amount prefix".into());
            }
            *before = after;
            uses.push(a.effect.clone());
        }
        for d in &tx.entries {
            if amounts.get(&d.id) != Some(&d.amount)
                || used.get(&d.id).cloned().unwrap_or_default() != d.applied
            {
                return Err("damage ledger/prefix mismatch".into());
            }
        }

        Ok(())
    }
}

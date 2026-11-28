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
    Resolved {
        id: EffectId,
    },
    IntrinsicProtectionFromEverything {
        source: ObjectId,
    },
    IntrinsicProtectionFromColor {
        source: ObjectId,
        color: Color,
    },
    /// CR 615.1, 614.1a: the static damage ability at `ability` of the
    /// battlefield permanent `source`'s static abilities.
    StaticAbility {
        source: ObjectId,
        ability: u32,
    },
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
    /// CR 113.7a, 712.18: the face whose program created the effect.
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub source_face: CardFace,
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
    /// CR 613.7b: the time the effect was created.
    pub timestamp: u64,
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
                ResolvedEffectKind::SetCharacteristics {
                    until: Until::ControllerNextTurn | Until::SourceLeaves,
                    ..
                }
            )
            || matches!(
                self.kind,
                ResolvedEffectKind::GainControl {
                    until: Until::Indefinite | Until::SourceLeaves,
                    ..
                }
            )
    }

    fn survives_cleanup(&self) -> bool {
        self.extends_beyond_creation_turn()
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
    RegenerationShield {
        target: ObjectId,
    },
    ProtectControllerUntilNextTurn,
    RemoveIndestructibleSet {
        targets: Vec<ObjectId>,
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
        until: Until,
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

    /// CR 615.1, 615.7: prevent matching damage this turn; `remaining` is
    /// the CR 615.7 shield still left.
    PreventDamage {
        kind: mtg_contracts::kir::DamageKindFilter,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<TargetEntity>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<damage_rules::PreventedSources>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remaining: Option<Quantity>,
    },
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
    Combat {
        // Frozen at assignment, before replacement choices or concessions.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        toughness_sources: BTreeSet<ObjectId>,
    },
    Instruction {
        frame: Box<Resolving>,
    },
    /// CR 605.1a, 605.3b: a resolving mana ability's damage rider; the
    /// activation resumes with its next rider.
    Mana {
        activation: Box<ManaActivation>,
    },
    /// CR 608.2c: an evaluated kernel step; its saved task resumes with the
    /// damage actually dealt.
    Kernel {
        resume: FrameLink,
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
            Self::Permanent { object } => TargetEntity::Object { object },
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

/// A setter's place in the timestamp order: its timestamp and, for a static
/// ability, its source's APNAP rank (`Object::keyword_rank`); `None` for a
/// resolved effect.
type Stamp = (u64, Option<u8>);

// CR 613.7m: simultaneous objects of different controllers go in APNAP
// order. CR 613.7n puts an entrant's own static effect before a
// simultaneous resolved setter on it. Conflicting same-controller statics
// still require a choice.
fn latest_setter<T: PartialEq>(
    setters: impl Iterator<Item = (Stamp, T)>,
) -> Result<Option<T>, String> {
    let all: Vec<_> = setters.collect();
    let Some(time) = all.iter().map(|((time, _), _)| *time).max() else {
        return Ok(None);
    };
    let mut latest: Vec<(Option<u8>, T)> = all
        .into_iter()
        .filter(|((t, _), _)| *t == time)
        .map(|((_, rank), value)| (rank, value))
        .collect();
    let top = latest.iter().map(|(rank, _)| rank.unwrap_or(u8::MAX)).max();
    latest.retain(|(rank, _)| Some(rank.unwrap_or(u8::MAX)) == top);
    let (_, value) = latest.remove(0);
    if latest.iter().any(|(_, other)| *other != value) {
        return Err("simultaneous continuous setters require timestamp order".into());
    }
    Ok(Some(value))
}

impl RulesTxn {
    /// CR 613.7m: the APNAP rank a static source's timestamp was given.
    fn source_rank(&self, source: ObjectId) -> u8 {
        self.current(source).map_or(0, |o| o.keyword_rank)
    }

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
                    targets, values, ..
                } if present && targets.contains(&object) => Some((effect.timestamp, values)),
                _ => None,
            })
    }

    pub(super) fn static_player(
        &self,
        expr: &PlayerExpr,
        controller: PlayerId,
        player: PlayerId,
    ) -> bool {
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
        filter
            .0
            .iter()
            .all(|atom| self.static_atom(atom, source, target, characteristics, controller))
    }

    fn static_atom(
        &self,
        atom: &Atom,
        source: &Object,
        target: &Object,
        characteristics: &Characteristics,
        controller: PlayerId,
    ) -> bool {
        match atom {
            Atom::Type { kind } => characteristics.has_type(*kind),
            Atom::NotType { kind } => !characteristics.has_type(*kind),
            Atom::AnyType { kinds } => kinds.iter().any(|k| characteristics.has_type(*k)),
            Atom::Subtype { subtype } => characteristics.has_subtype(subtype),
            Atom::NotSubtype { subtype } => !characteristics.has_subtype(subtype),
            Atom::Basic => characteristics.basic,
            Atom::NotBasic => !characteristics.basic,
            Atom::Legendary => characteristics.legendary,
            Atom::NotLegendary => !characteristics.legendary,
            Atom::Color { color } => characteristics.colors.contains(color),
            Atom::NotColor { color } => !characteristics.colors.contains(color),
            Atom::SourceChosenColor {} => matches!(source.entry_selection,
                Some(EntrySelection::Color { color }) if characteristics.colors.contains(&color)),
            Atom::SourceChosenCreatureType {} => matches!(source.entry_selection,
                Some(EntrySelection::CreatureType { kind }) if characteristics.has_subtype(kind.name())),
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
            | Atom::Is { .. }
            | Atom::Counters { .. } => false,
        }
    }

    /// CR 509.1b, 613.11: a combat restriction's filter against the current
    /// object at a declaration, after every layer, including its keywords
    /// and power. "You" is the restriction source's controller.
    pub(super) fn declaration_matches(
        &self,
        filter: &Pred,
        source: &Object,
        target: ObjectId,
    ) -> Result<bool, String> {
        let Ok(object) = self.current(target) else {
            return Ok(false);
        };
        if !object.present_on_battlefield() {
            return Ok(false);
        }
        let characteristics = self.characteristics(target)?;
        let keywords = self.current_keywords(target)?;
        Ok(self.declaration_predicate(filter, source, object, &characteristics, &keywords))
    }

    fn declaration_predicate(
        &self,
        filter: &Pred,
        source: &Object,
        target: &Object,
        characteristics: &Characteristics,
        keywords: &[KeywordAbility],
    ) -> bool {
        let stat = |value: &Option<Quantity>, cmp: &Cmp, bound: &Amount| {
            value
                .as_ref()
                .zip(bound.as_fixed())
                .is_some_and(|(value, bound)| static_compare(value, *cmp, bound))
        };
        filter.0.iter().all(|atom| match atom {
            Atom::Power { cmp, value } => stat(&characteristics.power, cmp, value),
            Atom::Toughness { cmp, value } => stat(&characteristics.toughness, cmp, value),
            Atom::Keyword { keyword } => keywords.contains(keyword),
            Atom::NotKeyword { keyword } => !keywords.contains(keyword),
            Atom::AnyOf { alternatives } => alternatives
                .iter()
                .any(|p| self.declaration_predicate(p, source, target, characteristics, keywords)),
            other => self.static_atom(other, source, target, characteristics, target.controller),
        })
    }

    /// The objects a combat restriction affects, read like its filters.
    pub(super) fn declaration_scope(
        &self,
        scope: &Scope,
        source: &Object,
        target: ObjectId,
    ) -> Result<bool, String> {
        match scope {
            Scope::Source => Ok(source.id == target && self.current(target).is_ok()),
            Scope::Matching { filter } => self.declaration_matches(filter, source, target),
            Scope::Attachment { filter, .. } => Ok(source.attached_to == Some(target)
                && self.declaration_matches(filter, source, target)?),
        }
    }

    /// CR 509.1b, 508.1c: the live block and attack restrictions.
    pub(super) fn static_declaration_rules(
        &self,
    ) -> Result<impl Iterator<Item = (&Object, &StaticRule)>, String> {
        Ok(self
            .live_static_rules(self.battlefield_statics(None)?)?
            .into_iter()
            .filter(|(_, rule)| {
                matches!(
                    rule,
                    StaticRule::Block { .. } | StaticRule::AttackRestriction { .. }
                )
            }))
    }

    pub(super) fn static_matches(
        &self,
        scope: &Scope,
        source: &Object,
        target: &Object,
        characteristics: &Characteristics,
        controller: PlayerId,
    ) -> bool {
        // CR 614.12: the entrant's own statics can affect the entrant
        // during lookahead; they are not yet in effect for other objects.
        if self
            .prospective_entry
            .as_ref()
            .is_some_and(|entry| entry.entrant == source.id)
            && source.id != target.id
        {
            return false;
        }
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

    /// CR 614.12: counts read the prior zone occupancy. In particular the
    /// entrant does not yet add its mana symbols to devotion (THB Release Notes).
    /// https://magic.wizards.com/en/news/feature/theros-beyond-death-release-notes-2020-01-10
    fn objects_for_static_counts(&self) -> impl Iterator<Item = &Object> {
        let entry = self.prospective_entry.as_ref();
        self.state
            .rules
            .objects
            .values()
            .filter(move |object| entry.is_none_or(|entry| entry.entrant != object.id))
            .chain(entry.and_then(|entry| entry.previous.as_ref()))
    }

    /// A failed layer read is an error, never a smaller count.
    pub(super) fn static_count(&self, source: &Object, set: &SetExpr) -> Result<usize, String> {
        Ok(match set {
            SetExpr::One {
                object: Ref::Source,
            } => 1,
            SetExpr::Matching { zone, filter } => {
                let mut count = 0usize;
                for target in self.objects_for_static_counts() {
                    let present = match zone {
                        ZoneExpr::Battlefield => target.present_on_battlefield(),
                        ZoneExpr::Graveyards => matches!(target.zone, Zone::Graveyard { .. }),
                        ZoneExpr::Graveyard { of } => {
                            matches!(target.zone, Zone::Graveyard { owner }
                        if self.static_player(of, source.controller, owner))
                        }
                        _ => false,
                    };
                    if present
                        && (!matches!(target.zone, Zone::Graveyard { .. })
                            || !self.state.rules.tokens.contains_key(&target.id.card))
                    {
                        let characteristics = self.structural_characteristics(target.id)?;
                        count += usize::from(self.static_predicate(
                            filter,
                            source,
                            target,
                            &characteristics,
                            target.controller,
                        ));
                    }
                }
                count
            }
            _ => 0,
        })
    }

    fn static_amount(&self, source: &Object, amount: &Amount) -> Result<Option<Quantity>, String> {
        Ok(match amount {
            Amount::Fixed { value } => Some(value.clone()),
            Amount::Count { set } => Some((self.static_count(source, set)? as i64).into()),
            // CR 700.5: statics admit only their controller's devotion
            // (`kir::statics`).
            Amount::Devotion { player, colors } => {
                if *player != PlayerExpr::static_controller() {
                    return Ok(None);
                }
                Some(self.devotion(source.controller, colors)?)
            }
            _ => None,
        })
    }

    /// CR 700.5, 700.5a: the mana symbols of `colors` among the mana costs
    /// of the permanents `player` controls; a hybrid symbol contributes at
    /// most one.
    pub(super) fn devotion(&self, player: PlayerId, colors: &[Color]) -> Result<Quantity, String> {
        let mut count = Quantity::zero();
        for permanent in self
            .objects_for_static_counts()
            .filter(|p| p.present_on_battlefield() && p.controller == player)
        {
            let c = &self.copiable_definition(permanent.id)?.characteristics;
            count.add(&(c.devotion_symbols(colors) as i64).into());
        }
        Ok(count)
    }

    pub(super) fn entry_condition(
        &self,
        entrant: &Object,
        condition: &mtg_contracts::kir::EntryCondition,
    ) -> Result<bool, String> {
        use mtg_contracts::kir::EntryCondition;
        Ok(match condition {
            EntryCondition::UnlessControlled { filter, cmp, count } => {
                let mut actual = 0;
                for target in self.battlefield_static_sources(None).filter(|target| {
                    target.id.card != entrant.id.card && target.controller == entrant.controller
                }) {
                    let characteristics = self.structural_characteristics(target.id)?;
                    if self.static_predicate(
                        filter,
                        entrant,
                        target,
                        &characteristics,
                        target.controller,
                    ) {
                        actual += 1;
                    }
                }
                !static_compare(&Quantity::from(actual), *cmp, count)
            }
            EntryCondition::UnlessOpponents { cmp, count } => {
                let actual = self
                    .live_players()
                    .into_iter()
                    .filter(|p| *p != entrant.controller)
                    .count();
                !static_compare(&(actual as i64).into(), *cmp, count)
            }
        })
    }

    pub(super) fn static_condition(
        &self,
        source: &Object,
        condition: &Cond,
    ) -> Result<bool, String> {
        Ok(match condition {
            Cond::Compare { left, cmp, right } => self
                .static_amount(source, left)?
                .zip(self.static_amount(source, right)?)
                .is_some_and(|(left, right)| static_compare(&left, *cmp, &right)),
            Cond::Exists { set } => self.static_count(source, set)? > 0,
            Cond::ActivePlayer { player } => self
                .state
                .rules
                .active
                .is_some_and(|active| self.static_player(player, source.controller, active)),
            Cond::Not { condition } => !self.static_condition(source, condition)?,
            Cond::All { conditions } => {
                for c in conditions {
                    if !self.static_condition(source, c)? {
                        return Ok(false);
                    }
                }
                true
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
        })
    }

    /// The battlefield statics that exist after ability removal (CR 305.7,
    /// 613.6, 613.8a); a rule or a layer-7 effect needs its source's
    /// abilities after layer 6.
    fn battlefield_statics<'a>(
        &'a self,
        sources: Option<&'a [CardId]>,
    ) -> Result<Vec<(&'a Object, &'a StaticEffect)>, String> {
        let setters = self.land_type_setters(sources);
        let alive = self.alive_removals(sources)?;
        let screen = setters || !alive.is_empty();
        let mut statics = Vec::new();
        for (source, key, effect) in self.indexed_battlefield_statics(sources) {
            if screen {
                let first = match effect {
                    StaticEffect::Continuous { changes, .. } => {
                        changes.iter().map(Change::layer).min()
                    }
                    _ => None,
                };
                if !self.static_exists(source, key, first, &alive, setters)? {
                    continue;
                }
            }
            statics.push((source, effect));
        }
        Ok(statics)
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
    ) -> Result<Option<&'a StaticRule>, String> {
        Ok(match effect {
            StaticEffect::Rule { rule, condition }
                if self.condition_holds(source, condition.as_ref())? =>
            {
                Some(rule)
            }
            _ => None,
        })
    }

    /// CR 604.1: a static with no condition, or whose condition holds now.
    pub(super) fn condition_holds(
        &self,
        source: &Object,
        condition: Option<&Cond>,
    ) -> Result<bool, String> {
        condition.map_or(Ok(true), |c| self.static_condition(source, c))
    }

    /// The live rules of `statics`, keeping their order.
    fn live_static_rules<'a>(
        &self,
        statics: impl IntoIterator<Item = (&'a Object, &'a StaticEffect)>,
    ) -> Result<Vec<(&'a Object, &'a StaticRule)>, String> {
        let mut rules = vec![];
        for (source, effect) in statics {
            if let Some(rule) = self.unpack_static_rule(source, effect)? {
                rules.push((source, rule));
            }
        }
        Ok(rules)
    }

    /// The layer's applicable static changes with the object whose static
    /// ability generates each, in timestamp order (CR 613.7a).
    pub(super) fn static_changes_by_source<'a>(
        &'a self,
        target: ObjectId,
        layer: Layer,
        sources: Option<&'a [CardId]>,
    ) -> Result<Vec<(u64, ObjectId, &'a Change)>, String> {
        Ok(self
            .static_changes_keyed(target, layer, sources)?
            .into_iter()
            .map(|(time, key, change)| (time, key.source, change))
            .collect())
    }

    /// [`Self::static_changes_by_source`] naming each generating ability.
    pub(super) fn static_changes_keyed<'a>(
        &'a self,
        target: ObjectId,
        layer: Layer,
        sources: Option<&'a [CardId]>,
    ) -> Result<Vec<(u64, ability_layers::StaticKey, &'a Change)>, String> {
        let Ok(target) = self.current(target) else {
            return Ok(Vec::new());
        };
        if !target.present_on_battlefield() {
            return Ok(Vec::new());
        }
        let setters = self.land_type_setters(sources);
        let alive = if layer >= Layer::Ability {
            self.alive_removals(sources)?
        } else {
            vec![]
        };
        let mut type_layer = None;
        let mut matching = Vec::new();
        for (source, key, effect) in self.indexed_battlefield_statics(sources) {
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
            let member = match first {
                // CR 613.8: layer 4 orders its effects by dependency.
                Layer::Type => {
                    if type_layer.is_none() {
                        type_layer = Some(self.type_layer(target.id)?);
                    }
                    type_layer
                        .as_ref()
                        .is_some_and(|t| t.applied.contains(&key))
                }
                _ => {
                    let facts = match first {
                        Layer::Color => self.structural_characteristics(target.id),
                        _ => self.color_layer_with_sources(target.id, sources),
                    };
                    self.static_matches(affects, source, target, facts?.as_ref(), target.controller)
                        && self.condition_holds(source, condition.as_ref())?
                        && self.static_exists(source, key, Some(first), &alive, setters)?
                }
            };
            if member {
                matching.extend(
                    changes
                        .iter()
                        .filter(|c| c.layer() == layer)
                        .map(|change| (source.keyword_timestamp.unwrap_or(0), key, change)),
                );
            }
        }
        matching.sort_by_key(|(timestamp, key, _)| (*timestamp, self.source_rank(key.source)));
        Ok(matching)
    }

    pub(super) fn combat_statics(
        &self,
        target: ObjectId,
        sources: Option<&[CardId]>,
    ) -> Result<Vec<CombatRequirement>, String> {
        let Ok(target) = self.current(target) else {
            return Ok(vec![]);
        };
        if !target.present_on_battlefield() {
            return Ok(vec![]);
        }
        let mut rules = self
            .live_static_rules(self.battlefield_statics(sources)?)?
            .into_iter()
            .filter_map(|(source, rule)| match rule {
                StaticRule::Combat {
                    affects,
                    requirement,
                } => Some((source, affects, *requirement)),
                _ => None,
            })
            .peekable();
        if rules.peek().is_none() {
            return Ok(vec![]);
        }
        let facts = self.color_layer_with_sources(target.id, sources)?;
        Ok(rules
            .filter_map(|(source, affects, requirement)| {
                self.static_matches(affects, source, target, &facts, target.controller)
                    .then_some(requirement)
            })
            .collect())
    }

    pub(super) fn static_land_allowance(
        &self,
        player: PlayerId,
        total: &mut Quantity,
    ) -> Result<bool, String> {
        for (source, effect) in self.battlefield_statics(None)? {
            if let Some(StaticRule::AdditionalLandPlays {
                player: who,
                allowance,
            }) = self.unpack_static_rule(source, effect)?
                && self.static_player(who, source.controller, player)
            {
                match allowance {
                    Allowance::Unlimited => return Ok(true),
                    Allowance::Additional { count } => total.add(count),
                }
            }
        }
        Ok(false)
    }

    pub(super) fn static_hand_size(&self, player: PlayerId) -> Result<Option<Quantity>, String> {
        let mut changes: Vec<_> = self
            .live_static_rules(self.battlefield_statics(None)?)?
            .into_iter()
            .filter_map(|(source, rule)| {
                let StaticRule::MaximumHandSize {
                    player: who,
                    change,
                } = rule
                else {
                    return None;
                };
                self.static_player(who, source.controller, player)
                    .then_some((
                        (source.keyword_timestamp.unwrap_or(0), source.keyword_rank),
                        change,
                    ))
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
    ) -> Result<impl Iterator<Item = (&'a Object, &'a StaticRule)> + 'a, String> {
        Ok(self
            .live_static_rules(
                self.battlefield_statics(None)?.into_iter().chain(
                    self.static_abilities_in(target, FunctioningZone::Stack)
                        .map(move |e| (target, e)),
                ),
            )?
            .into_iter())
    }

    pub(super) fn has_uncounterable_static(
        &self,
        target: ObjectId,
        caster: PlayerId,
    ) -> Result<bool, String> {
        let object = self.current(target)?;
        let characteristics = &self.copiable_definition(target)?.characteristics;
        Ok(self.spell_static_rules(object)?.any(|(source, rule)| {
            matches!(rule, StaticRule::CantBeCountered { affects }
                if self.static_matches(affects, source, object, characteristics, caster))
        }))
    }

    pub(super) fn static_cost_changes(
        &self,
        proposed: ObjectId,
        caster: PlayerId,
    ) -> Result<Vec<SpellCostChange>, String> {
        let object = self
            .state
            .rules
            .objects
            .get(&proposed.card)
            .ok_or("missing spell object")?;
        let object = Object {
            entry_selection: None,
            id: proposed,
            controller: caster,
            ..object.clone()
        };
        let characteristics = &self.copiable_definition(proposed)?.characteristics;
        let mut changes = Vec::new();
        for (source, rule) in self.spell_static_rules(&object)? {
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
                && self.static_matches(affects, source, &object, characteristics, caster)
            {
                let mut change = change.clone();
                if let Some(amount) = multiplier {
                    let multiplier = self
                        .static_amount(source, amount)?
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
        face: CardFace,
        face_down: bool,
        caster: PlayerId,
    ) -> Result<bool, String> {
        let object = Object {
            entry_selection: None,
            id: ObjectId { face, ..object.id },
            face_down,
            ..object.clone()
        };
        // CR 708.4: a face-down cast reads the face-down spell's values.
        let characteristics = if face_down {
            &face_down::face_down_definition().characteristics
        } else {
            &self.copiable_definition(object.id)?.characteristics
        };
        Ok(self.spell_static_rules(&object)?.any(|(source, rule)| {
            matches!(rule, StaticRule::CastAsThoughFlash { affects, caster: who }
                if self.static_player(who, source.controller, caster)
                    && self.static_matches(affects, source, &object, characteristics, caster))
        }))
    }
}

impl RulesTxn {
    /// Layer 4 (CR 613.1d). A printed characteristic-defining ability
    /// (changeling, CR 702.73a) is already in the copiable values and
    /// applies first (CR 613.3); resolved animations and admitted static
    /// type changes then apply in timestamp order (CR 613.7), so a later
    /// creature-subtype replacement removes every creature type and a later
    /// "is every creature type" restores them.
    pub(super) fn structural_characteristics(
        &self,
        object: ObjectId,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        Ok(self.type_layer(object)?.facts)
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
        let statics = self.static_changes_by_source(object, Layer::Color, sources)?;
        let colors = statics
            .into_iter()
            .filter_map(|(time, source, change)| match change {
                Change::SetColors { colors } => {
                    Some(((time, Some(self.source_rank(source))), colors))
                }
                _ => None,
            });
        let resolved = self
            .resolved_characteristic_sets(object)
            .filter_map(|(time, values)| Some(((time, None), values.colors.as_ref()?)));
        // CR 613.8a: static color scopes cannot read colors, and a
        // resolved setter always affects its captured objects.
        if let Some(colors) = latest_setter(colors.chain(resolved))? {
            facts.to_mut().colors.clone_from(colors);
        }
        Ok(facts)
    }

    /// CR 604.3, 613.4a, 208.2a: layer 7a. The object's own
    /// characteristic-defining power and toughness apply in every zone and
    /// are read each time the value is looked at. Counts read only layers
    /// 1-6 of other objects (`kir::statics`), so no CDA reads another's
    /// layer-7 result.
    pub(super) fn characteristic_power_toughness(
        &self,
        object: ObjectId,
        c: &mut Characteristics,
    ) -> Result<(), String> {
        let Ok(source) = self.current(object) else {
            return Ok(());
        };
        let mut lost = None;
        for effect in self.static_abilities_in(source, FunctioningZone::Any) {
            let StaticEffect::DefinePowerToughness { power, toughness } = effect else {
                continue;
            };
            // CR 613.1f, 613.4a: a CDA removed in layer 6 no longer defines
            // the stat, which keeps its printed value (`*` is 0, CR 208.2a).
            if lost.is_none() {
                lost = Some(self.printed_abilities_lost(object)?);
            }
            if lost == Some(true) {
                return Ok(());
            }
            for (amount, stat) in [(power, &mut c.power), (toughness, &mut c.toughness)] {
                if let (Some(amount), Some(stat)) = (amount, stat.as_mut()) {
                    *stat = self.characteristic_amount(source, amount)?;
                }
            }
        }
        Ok(())
    }

    fn characteristic_amount(&self, source: &Object, amount: &Amount) -> Result<Quantity, String> {
        Ok(match amount {
            Amount::Fixed { value } => value.clone(),
            Amount::Count { set } => {
                let count = self.characteristic_set(source, set)?.len();
                Quantity::from(i64::try_from(count).map_err(|_| "CDA count exceeds object limit")?)
            }
            Amount::Times { factor, of } => {
                let mut value = self.characteristic_amount(source, of)?;
                value.multiply(factor);
                value
            }
            Amount::Sum { terms } => {
                let mut sum = Quantity::zero();
                for term in terms {
                    sum.add(&self.characteristic_amount(source, term)?);
                }
                sum
            }
            // CR 202.3: zero when no permanent matches (CR 208.2a).
            Amount::Greatest {
                set,
                stat: mtg_contracts::kir::Stat::ManaValue,
            } => self
                .characteristic_set(source, set)?
                .into_iter()
                .map(|(_, c)| c.printed_mana_value())
                .fold(Quantity::zero(), Quantity::max),
            Amount::Devotion { player, colors } if *player == PlayerExpr::static_controller() => {
                self.devotion(source.controller, colors)?
            }
            Amount::Distinct {
                set,
                characteristic,
            } => {
                use mtg_contracts::kir::DistinctCharacteristic as D;
                let mut seen = BTreeSet::new();
                // CR 205.2a: card types are counted over every represented
                // type (`CardKind::CARD_TYPES`, exhaustive by construction).
                let mut types = [false; CardKind::CARD_TYPES.len()];
                for (_, c) in self.characteristic_set(source, set)? {
                    match characteristic {
                        D::CardType => {
                            for kind in
                                std::iter::once(c.kind).chain(c.additional_types.iter().copied())
                            {
                                types[kind.card_type_index()] = true;
                            }
                        }
                        // CR 205.3i: the five basic land types.
                        D::BasicLandType => seen.extend(
                            c.subtypes
                                .iter()
                                .filter(|s| {
                                    matches!(
                                        s.as_str(),
                                        "Plains" | "Island" | "Swamp" | "Mountain" | "Forest"
                                    )
                                })
                                .cloned(),
                        ),
                        // CR 105.1: colorless is not a color.
                        D::Color => seen.extend(
                            c.colors
                                .iter()
                                .filter(|color| **color != Color::C)
                                .map(|color| format!("{color:?}")),
                        ),
                    }
                }
                let card_types = types.iter().filter(|counted| **counted).count();
                Quantity::from((seen.len() + card_types) as i64)
            }
            _ => return Err("characteristic-defining amount has no evaluation".into()),
        })
    }

    /// The objects a characteristic-defining count reads, with their
    /// layer 1-5 characteristics. Tokens cease to exist in graveyards
    /// (CR 111.7); phased-out permanents are treated as though they don't
    /// exist (CR 702.26b).
    pub(super) fn characteristic_set(
        &self,
        source: &Object,
        set: &SetExpr,
    ) -> Result<Vec<(ObjectId, Characteristics)>, String> {
        let SetExpr::Matching { zone, filter } = set else {
            return Err("characteristic-defining set has no evaluation".into());
        };
        let mut members = Vec::new();
        for target in self.objects_for_static_counts() {
            let present = match (zone, target.zone) {
                (ZoneExpr::Battlefield, _) => target.present_on_battlefield(),
                (ZoneExpr::Graveyards, Zone::Graveyard { .. }) => true,
                (ZoneExpr::Graveyard { of }, Zone::Graveyard { owner })
                | (ZoneExpr::Hand { of }, Zone::Hand { owner }) => {
                    self.static_player(of, source.controller, owner)
                }
                _ => false,
            };
            if !present
                || (target.zone != Zone::Battlefield
                    && self.state.rules.tokens.contains_key(&target.id.card))
            {
                continue;
            }
            let characteristics = self.color_layer(target.id)?;
            if self.static_predicate(filter, source, target, &characteristics, target.controller) {
                members.push((target.id, characteristics.into_owned()));
            }
        }
        Ok(members)
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
        // CR 208.3, 301.7a–b: outside the battlefield, retain printed
        // stats. On the battlefield, layer 4 determines their availability.
        if self.current(object)?.zone == Zone::Battlefield && !c.has_type(CardKind::Creature) {
            c.power = None;
            c.toughness = None;
        }
        self.characteristic_power_toughness(object, &mut c)?;
        if c.has_type(CardKind::Creature) {
            let statics =
                self.static_changes_by_source(object, Layer::PowerToughnessSet, sources)?;
            // CR 613.4b: a setter can leave either characteristic alone.
            // Neither supported kind reads a layer-7 result (CR 613.8a).
            let base = |power_stat| {
                let fixed = statics.iter().filter_map(|(time, source, change)| {
                    let Change::SetPowerToughness { power, toughness } = change else {
                        return None;
                    };
                    Some((
                        (*time, Some(self.source_rank(*source))),
                        if power_stat { power } else { toughness }.as_fixed()?,
                    ))
                });
                let resolved =
                    self.resolved_characteristic_sets(object)
                        .filter_map(|(time, values)| {
                            Some((
                                (time, None),
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
            let minus = o.counter_count(&mtg_contracts::kir::CounterKind::MinusOneMinusOne);
            if let Some(p) = c.power.as_mut() {
                p.add(&o.plus_one_counters);
                p.subtract(&minus);
            }
            if let Some(t) = c.toughness.as_mut() {
                t.add(&o.plus_one_counters);
                t.subtract(&minus);
            }
        }
        for (_, source, change) in
            self.static_changes_by_source(object, Layer::PowerToughnessModify, sources)?
        {
            if let Change::PowerToughness { power, toughness } = change {
                // CR 608.2h, 611.3a, 613.4c: a counted modification ("+1/+1
                // for each ...") is recounted whenever the stat is read. Its
                // count reads only layers 1-5 of the counted objects
                // (`kir::statics`), so it never reads a layer-7 result.
                let source = self.current(source)?;
                let delta = |amount: &Amount| match amount.as_fixed() {
                    Some(value) => Ok(value.clone()),
                    None => self.characteristic_amount(source, amount),
                };
                c.power.as_mut().ok_or("static power")?.add(&delta(power)?);
                c.toughness
                    .as_mut()
                    .ok_or("static toughness")?
                    .add(&delta(toughness)?);
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
    /// CR 603.10: the exact incarnation the resolving trigger's event bound.
    pub(super) fn event_object_target(&self, frame: &Resolving) -> Result<ObjectId, String> {
        Ok(self
            .entry(frame.stack_id)?
            .kind
            .observed_object()?
            .last_known
            .object)
    }

    pub(super) fn commit_resolved_effect(
        &mut self,
        frame: Resolving,
        kind: ResolvedEffectKind,
        block: u32,
    ) -> Result<(), String> {
        let actual = self.create_resolved_effect(&frame, kind, block)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }

    pub(in crate::engine) fn create_resolved_effect(
        &mut self,
        frame: &Resolving,
        kind: ResolvedEffectKind,
        block: u32,
    ) -> Result<EventOutcome, String> {
        let handles = match &self.entry(frame.stack_id)?.kind {
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
            StackKind::Spell => self.entry(frame.stack_id)?.effect_source_handles.clone(),
        };
        let target_handles = match &kind {
            ResolvedEffectKind::RemoveIndestructibleSet { targets, .. }
            | ResolvedEffectKind::PumpSet { targets, .. }
            | ResolvedEffectKind::GrantKeywordsSet { targets, .. }
            | ResolvedEffectKind::GainControl { targets, .. }
            | ResolvedEffectKind::SetCharacteristics { targets, .. }
            | ResolvedEffectKind::PumpAnnouncedX { targets, .. }
            | ResolvedEffectKind::ExileDamaged { targets } => {
                let capacity = self.event_group_limit();
                if targets.len() > capacity {
                    return Err(self.capacity_error(RuleFault::EventGroupCapacityExceeded {
                        capacity,
                        requested: targets.len(),
                    }));
                }
                let mut handles = Vec::new();
                for target in targets {
                    handles.extend(self.capture_trigger_source_handles(*target)?);
                }
                handles
            }
            ResolvedEffectKind::RegenerationShield { target }
            | ResolvedEffectKind::GrantKeywords { target, .. }
            | ResolvedEffectKind::ModifyPowerToughness { target, .. }
            | ResolvedEffectKind::CombatProhibition {
                affects: CombatProhibitionScope::Target { target },
                ..
            } => self.capture_trigger_source_handles(*target)?,
            kind @ ResolvedEffectKind::PreventDamage { .. } => {
                match Self::prevention_object(kind) {
                    Some(object) => self.capture_trigger_source_handles(object)?,
                    None => vec![],
                }
            }

            _ => vec![],
        };
        let effect = ResolvedEffect {
            target_handles,
            id: EffectId(self.state.rules.next_effect_id),
            timestamp: self.state.rules.timestamps.next(),
            origin: EffectOrigin {
                activated_ability: self.entry(frame.stack_id)?.kind.activated_index(),
                source_face: self.entry(frame.stack_id)?.program_face(),
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
        self.commit(
            ProposedEvent::CreateEffect {
                effect: Box::new(effect),
            },
            self.instruction_cause(frame, block)?,
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
        let sources = damage.iter().map(|d| d.source).collect::<BTreeSet<_>>();
        let mut toughness_sources = BTreeSet::new();
        for source in sources {
            if self.assigns_with_toughness(source)? {
                toughness_sources.insert(source);
            }
        }
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
        self.rules_mut().defender_damage_choices.clear();
        self.begin_damage(
            DamageKind::Combat,
            entries,
            BatchCause::Combat,
            DamageContinuation::Combat { toughness_sources },
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
    pub(super) fn recipient_chooser(
        &self,
        recipient: &DamageRecipient,
    ) -> Result<Option<PlayerId>, String> {
        Ok(match recipient {
            DamageRecipient::Player { player } => self.is_live(*player).then_some(*player),
            DamageRecipient::Permanent { object } => self
                .is_permanent_damage_recipient(*object)?
                .then(|| self.current(*object).ok().map(|o| o.controller))
                .flatten(),
        })
    }
    /// CR 608.2h/609.7c/702.2e/702.15b-c: damage-time properties use the
    /// exact source's current public-zone appearance or its LKI when it has
    /// left or phased out (702.26b). Assignment facts preserve quantities and legality.
    pub(super) fn damage_source_appearance(
        &self,
        damage: &PreparedDamage,
    ) -> Result<ObjectBefore, String> {
        let appearance = if let Ok(current) = self.current_source_facts(damage.source.object)
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
    // CR 800.4m: if the controller left, the effect ends at the boundary
    // where their seat would have taken a turn.
    pub(super) fn expire_characteristics_at_turn_start(
        &mut self,
        previous: PlayerId,
        next: PlayerId,
    ) {
        let seats = self.state.rules.players.len() as u8;
        let distance = |p: PlayerId| (p.0 + seats - previous.0 + seats - 1) % seats;
        let boundary = distance(next);
        self.rules_mut().effects.retain(|effect| {
            !matches!(
                effect.kind,
                ResolvedEffectKind::SetCharacteristics {
                    until: Until::ControllerNextTurn,
                    ..
                }
            ) || distance(effect.origin.controller) > boundary
        });
    }
    // CR 702.16e,h/616.1: the existing each-color grant is represented
    // by one candidate for matching prepared damage. Printed named-color
    // protection uses separate identities and consumed damage-time facts.
    fn grant_prevents_damage(
        targets: &[ObjectId],
        keywords: &[KeywordAbility],
        damage: &PreparedDamage,
    ) -> bool {
        matches!(damage.recipient, DamageRecipient::Permanent { object } if targets.contains(&object))
            && keywords.contains(&KeywordAbility::ProtectionFromEachColor)
            && Self::has_color(&damage.source.characteristics)
    }

    // CR 702.16e,g and 616.1: separate printed qualities have separate
    // candidate identities even when a multicolored source matches both.
    fn intrinsic_protection_keys(
        &self,
        recipient: &DamageRecipient,
    ) -> Result<Vec<EffectKey>, String> {
        let DamageRecipient::Permanent { object } = *recipient else {
            return Ok(vec![]);
        };
        if !self
            .current(object)
            .is_ok_and(|o| o.present_on_battlefield())
        {
            return Ok(vec![]);
        }
        let mut keys: Vec<_> = self
            .effective_protection_colors(object)?
            .map(|color| EffectKey::IntrinsicProtectionFromColor {
                source: object,
                color,
            })
            .collect();
        if self.printed_protection_from_everything(object)? {
            keys.push(EffectKey::IntrinsicProtectionFromEverything { source: object });
        }
        Ok(keys)
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
            || self.recipient_chooser(&d.recipient)?.is_none()
        {
            return Err("damage effect no longer applicable".into());
        }
        let mut qualification = None;
        let (after, prevented) = match &pair.effect {
            EffectKey::IntrinsicProtectionFromEverything { source } => {
                if d.recipient != (DamageRecipient::Permanent { object: *source })
                    || !self
                        .current(*source)
                        .is_ok_and(|o| o.present_on_battlefield())
                    || !self.printed_protection_from_everything(*source)?
                {
                    return Err("wrong intrinsic protection source".into());
                }
                (Quantity::zero(), d.amount.clone())
            }
            EffectKey::IntrinsicProtectionFromColor { source, color } => {
                if d.recipient != (DamageRecipient::Permanent { object: *source })
                    || !self
                        .current(*source)
                        .is_ok_and(|o| o.present_on_battlefield())
                    || !self
                        .effective_protection_colors(*source)?
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
                    ResolvedEffectKind::PreventDamage { .. } => {
                        self.resolved_prevention_application(tx, d, effect)?
                    }

                    _ => return Err("resolved effect not applicable".into()),
                }
            }
            EffectKey::StaticAbility { source, ability } => {
                qualification = Some(Box::new(DamageReplacementQualification {
                    effect_source: self.object_before(*source)?,
                    damage_source: self.damage_source_appearance(d)?,
                }));
                self.static_damage_application(tx.kind, d, *source, *ability)?
            }
        };
        // CR 615.12: no prevention effect, protection included, prevents
        // damage that can't be prevented; replacements still apply.
        if prevented > Quantity::zero() && self.damage_unpreventable(tx.kind, d)? {
            return Err("damage can't be prevented".into());
        }
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
        let mut recipients: Vec<_> = vec![];
        for d in &tx.entries {
            if let Some(p) = self.recipient_chooser(&d.recipient)? {
                recipients.push((p, d.recipient));
            }
        }
        recipients.sort_by_key(|(p, r)| {
            (
                order.iter().position(|q| q == p).unwrap_or(4),
                match r {
                    DamageRecipient::Player { player } => (0, u32::from(player.0), 0),
                    DamageRecipient::Permanent { object } => (1, object.card.0, object.incarnation),
                },
            )
        });
        recipients.dedup();
        let mut keys = self.live_effect_keys();
        keys.extend(self.static_damage_keys()?);
        for (player, recipient) in recipients {
            let intrinsic = self.intrinsic_protection_keys(&recipient)?;
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
    /// One effect applies without a choice, except that a CR 615.7 shield
    /// offered for several simultaneous events lets the recipient choose
    /// which damage it prevents.
    fn automatic_pair(&self, pairs: &[DamageEffectCandidate]) -> bool {
        let Some(first) = pairs.first() else {
            return false;
        };
        let shield = match first.effect {
            EffectKey::Resolved { id } => self.state.rules.effects.iter().any(|e| {
                e.id == id
                    && matches!(
                        e.kind,
                        ResolvedEffectKind::PreventDamage {
                            remaining: Some(_),
                            ..
                        }
                    )
            }),
            _ => false,
        };
        pairs.iter().all(|pair| pair.effect == first.effect) && (pairs.len() == 1 || !shield)
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
            if let DamageContinuation::Kernel { resume } = tx.continuation {
                let batch = self.commit_batch(
                    tx.cause,
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::CommitDamage { transaction: tx.id },
                    },
                )?;
                return self.finish_kernel_damage(resume, &batch);
            }
            let actual =
                self.commit(ProposedEvent::CommitDamage { transaction: tx.id }, tx.cause)?;
            match tx.continuation {
                DamageContinuation::Kernel { .. } => unreachable!("kernel damage continuation"),
                DamageContinuation::Combat { .. } => self.grant_turn_priority()?,
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
        common: Option<&Self>,
    ) -> Result<EventOutcome, String> {
        match event {
            ProposedEvent::CreateEffect { effect } => {
                self.reserve_effect_slot()?;
                let timestamp = common.unwrap_or(self).state.rules.timestamps.next();
                if effect.id != EffectId(self.state.rules.next_effect_id)
                    || effect.timestamp != timestamp
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
                let entry = self.entry(frame.stack_id)?;
                if entry.kind.activated_index() != effect.origin.activated_ability
                    || entry.source != effect.origin.source
                    || entry.program_face() != effect.origin.source_face
                    || frame.stack_id != effect.origin.stack_id
                    || entry.controller != effect.origin.controller
                    || frame.cursor != effect.origin.cursor
                {
                    return Err("unbound continuous effect".into());
                }
                let instruction = self
                    .program_for(entry)?
                    .instructions
                    .get(frame.cursor as usize)
                    .ok_or("effect instruction absent")?;
                let valid = match (instruction, &effect.kind) {
                    (
                        kernel!(Regenerate { what } @ source_block),
                        ResolvedEffectKind::RegenerationShield { target },
                    ) => {
                        *source_block == effect.origin.source_block
                            && self.regeneration_target(frame, what)? == Some(*target)
                    }
                    (
                        Instruction::Kernel {
                            node: mtg_contracts::kir::Node::Do(verb),
                            source_block,
                        },
                        kind @ (ResolvedEffectKind::GainControl { .. }
                        | ResolvedEffectKind::SetCharacteristics { .. }
                        | ResolvedEffectKind::PumpAnnouncedX { .. }
                        | ResolvedEffectKind::PumpSet { .. }
                        | ResolvedEffectKind::ModifyPowerToughness { .. }
                        | ResolvedEffectKind::GrantKeywords { .. }
                        | ResolvedEffectKind::GrantKeywordsSet { .. }),
                    ) => {
                        *source_block == effect.origin.source_block
                            && self.continuous_kind(frame, verb)?.as_ref() == Some(kind)
                    }
                    (
                        kernel!(AdditionalLandPlays { amount, .. } @ source_block),
                        ResolvedEffectKind::AdditionalLandPlays { count },
                    ) => {
                        self.is_live(effect.origin.controller)
                            && *source_block == effect.origin.source_block
                            && amount.as_fixed() == Some(count)
                            && *count > Quantity::zero()
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
                        kernel!(PreventDamage { prevention, until: Until::EndOfTurn } @ source_block),
                        kind @ ResolvedEffectKind::PreventDamage { .. },
                    ) => {
                        *source_block == effect.origin.source_block
                            && self.prevention_kind(frame, prevention)?.as_ref() == Some(kind)
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
                    ResolvedEffectKind::RegenerationShield { target }
                    | ResolvedEffectKind::GrantKeywords { target, .. }
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
                    kind @ ResolvedEffectKind::PreventDamage { .. } => {
                        match Self::prevention_object(kind) {
                            Some(target) => {
                                effect.target_handles.len() == 4
                                    && (0..4).all(|v| {
                                        self.object_handle_id(PlayerId(v), target)
                                            == Ok(&effect.target_handles[v as usize])
                                    })
                            }
                            None => effect.target_handles.is_empty(),
                        }
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
                self.allocate_timestamp(common)?;
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
                let expired = self
                    .state
                    .rules
                    .effects
                    .iter()
                    .filter(|e| !e.survives_cleanup())
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
                self.rules_mut().effects.retain(|e| e.survives_cleanup());
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
            if self.recipient_chooser(&d.recipient)?.is_none() {
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
                DamageRecipient::Permanent { object } => (1, object.card.0),
            };
            totals.entry(key).or_default().add(&d.amount);
            source_appearances.insert(d.id, self.damage_source_appearance(d)?);
            actual.push(ActualDamage {
                id: d.id,
                source: d.source.clone(),
                controller: d.controller,
                recipient: d.recipient,
                amount: d.amount.clone(),
            });
        }
        // The entire group was validated above, before any recipient/history write.
        let mut results = vec![];
        for ((kind, id), amount) in totals {
            if kind == 0 {
                let player = PlayerId(id as u8);
                let (previous, current) =
                    self.rules_mut().players[player.0 as usize].lose_life(&amount);
                results.push(DamageResult {
                    recipient: DamageRecipient::Player { player },
                    previous,
                    current,
                });
            } else {
                let object = self
                    .state
                    .rules
                    .objects
                    .get(&CardId(id))
                    .ok_or("missing final damage object")?
                    .id;
                if let Some(result) = self.apply_permanent_damage(object, &amount)? {
                    results.push(result);
                }
            }
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
        let life_gains = self.damage_keywords(&actual, &source_appearances)?;
        self.consume_prevention_shields(&tx)?;
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
        // Members of one simultaneous occurrence share a rules timestamp.
        // Effect IDs still order the records and are unique within the cohort.
        if !self
            .state
            .rules
            .effects
            .windows(2)
            .all(|rows| rows[0].id < rows[1].id && rows[0].timestamp <= rows[1].timestamp)
        {
            return Err("effect creation order invariant".into());
        }
        for e in &self.state.rules.effects {
            if !self.state.rules.timestamps.contains(e.timestamp)
                || e.id.0 >= self.state.rules.next_effect_id
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
            if e.origin.stack_id.0 >= self.state.rules.next_stack_id
                // CR 712.18: only a transforming card's program can come
                // from a face other than the one its incarnation has.
                || (e.origin.source_face != e.origin.source.face
                    && !self.definition(e.origin.source.card)?.transforms())
            {
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
                        kernel!(Regenerate { what } @ source_block),
                        ResolvedEffectKind::RegenerationShield { target },
                    ) => {
                        *source_block == e.origin.source_block
                            && self
                                .current(*target)
                                .is_ok_and(|o| o.zone == Zone::Battlefield)
                            && e.target_handles.len() == 4
                            && (0..4).all(|viewer| {
                                self.object_handle_id(PlayerId(viewer), *target).is_ok_and(
                                    |handle| *handle == e.target_handles[viewer as usize],
                                )
                            })
                            && match what {
                                SetExpr::One {
                                    object: Ref::Source,
                                } => *target == e.origin.source,
                                SetExpr::One {
                                    object: Ref::Target { .. },
                                } => true,
                                _ => false,
                            }
                    }
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
                        kernel!(AdditionalLandPlays { amount, .. } @ source_block),
                        ResolvedEffectKind::AdditionalLandPlays { count },
                    ) => {
                        *source_block == e.origin.source_block
                            && amount.as_fixed() == Some(count)
                            && *count > Quantity::zero()
                            && e.target_handles.is_empty()
                    }
                    (
                        kernel!(SetCharacteristics { what, values, until } @ source_block),
                        ResolvedEffectKind::SetCharacteristics {
                            targets,
                            values: saved,
                            until: duration,
                        },
                    ) => {
                        let mut expected = values.clone();
                        let valid_values = if expected.base_from.take().is_some() {
                            expected.base_power.clone_from(&saved.base_power);
                            expected.base_toughness.clone_from(&saved.base_toughness);
                            saved.base_power.as_ref().is_some_and(|p| !p.is_negative())
                                && saved.base_power == saved.base_toughness
                                && expected == *saved
                        } else {
                            values == saved
                        };
                        *source_block == e.origin.source_block
                            && until == duration
                            && (until != &Until::SourceLeaves
                                || self.control_source_remains(e.origin.source))
                            && valid_values
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
                                self.object_creature_possible(*object).unwrap_or(false)
                            })
                    }
                    (
                        kernel!(PreventDamage { prevention, until: Until::EndOfTurn } @ source_block),
                        kind @ ResolvedEffectKind::PreventDamage {
                            kind: saved_kind,
                            to,
                            by,
                            remaining,
                        },
                    ) => {
                        use mtg_contracts::kir::{PreventionShield, PreventionSources};
                        let known =
                            |object: &ObjectId| self.state.rules.cards.contains_key(&object.card);
                        *source_block == e.origin.source_block
                            && *saved_kind == prevention.kind
                            && to.is_some() == prevention.to.is_some()
                            && match to {
                                Some(TargetEntity::Player { player }) => player.0 < 4,
                                Some(TargetEntity::Object { object }) => known(object),
                                None => true,
                            }
                            && match (&prevention.by, by) {
                                (None, None) => true,
                                (
                                    Some(PreventionSources::One { .. }),
                                    Some(damage_rules::PreventedSources::One { object }),
                                ) => known(object),
                                (
                                    Some(PreventionSources::Matching { filter }),
                                    Some(damage_rules::PreventedSources::Matching {
                                        filter: saved,
                                    }),
                                ) => filter == saved,
                                _ => false,
                            }
                            && match (&prevention.shield, remaining) {
                                (PreventionShield::All, None) => true,
                                (PreventionShield::Next { amount }, Some(left)) => {
                                    *left > Quantity::zero() && left <= amount
                                }
                                _ => false,
                            }
                            && e.target_handles.len()
                                == if Self::prevention_object(kind).is_some() {
                                    4
                                } else {
                                    0
                                }
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
                                // CR 611.2c: the ability's own source.
                                (
                                    S::Target {
                                        target: Ref::Source,
                                    },
                                    S::Target { target },
                                ) => {
                                    *target == e.origin.source
                                        && e.target_handles.len() == 4
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
                            ..
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
                                } => self.object_creature_possible(*target)?,
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
                            ..
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
                        kernel!(ModifyUntil { what: target_object!(_) | event_object!(), change: mtg_contracts::kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: mtg_contracts::kir::Until::EndOfTurn, .. } @ source_block),
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
                            && self.object_creature_possible(*target)?
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
            let statics = self.static_damage_keys()?;
            for d in &tx.entries {
                let Some(controller) = self.recipient_chooser(&d.recipient)? else {
                    continue;
                };
                if d.amount <= Quantity::zero() {
                    continue;
                }
                let handles = intrinsic
                    .get_mut(controller.0 as usize)
                    .ok_or("invalid protection controller")?;
                for key in self
                    .intrinsic_protection_keys(&d.recipient)?
                    .into_iter()
                    .chain(statics.iter().cloned())
                {
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
            | EffectKey::IntrinsicProtectionFromColor { source, .. }
            | EffectKey::StaticAbility { source, .. } => (
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
                targets,
                values,
                until,
            } => VisibleEffect::SetCharacteristics {
                handle,
                origin,
                targets: set_views(targets)?,
                values: values.clone(),
                expires_in_cleanup_of_turn: (*until == Until::EndOfTurn)
                    .then_some(e.expires_in_cleanup_of_turn),
                expires_at_controller_next_turn: *until == Until::ControllerNextTurn,
                while_source_remains: *until == Until::SourceLeaves,
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
            ResolvedEffectKind::RegenerationShield { target } => {
                VisibleEffect::RegenerationShield {
                    handle,
                    origin,
                    target: target_view(*target)?,
                    expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
                }
            }

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
                ..
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
                ..
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
            ResolvedEffectKind::PreventDamage {
                kind,
                to,
                by,
                remaining,
            } => VisibleEffect::PreventDamage {
                handle,
                origin,
                kind: *kind,
                to_player: match to {
                    Some(TargetEntity::Player { player }) => Some(*player),
                    _ => None,
                },
                to_object: match to {
                    Some(TargetEntity::Object { object }) => Some(target_view(*object)?),
                    _ => None,
                },
                by_object: match by {
                    Some(damage_rules::PreventedSources::One { object }) => {
                        Some(target_view(*object)?)
                    }
                    _ => None,
                },
                by_filter: match by {
                    Some(damage_rules::PreventedSources::Matching { filter }) => {
                        Some(filter.clone())
                    }
                    _ => None,
                },
                remaining: remaining.clone(),
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
            EffectKey::StaticAbility { source, ability } => match self
                .live_damage_rule(source, ability)?
                .ok_or("missing static damage rule")?
                .1
            {
                mtg_contracts::kir::StaticRule::ModifyDamage { .. } => {
                    DamageEffectKind::ModifyDamage
                }
                _ => DamageEffectKind::PreventDamage,
            },
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
                ResolvedEffectKind::PreventDamage { .. } => DamageEffectKind::PreventDamage,

                _ => return Err("non-damage effect in choice".into()),
            },
        };
        // The grant supplies the durable effect handle. The creature that
        // actually has protection is the source of this prevention ability.
        let origin = if kind == DamageEffectKind::ProtectionFromEachColor {
            let DamageRecipient::Permanent { object } = d.recipient else {
                return Err("protection requires a creature recipient".into());
            };
            let handle = self.handle_for(viewer, object)?;
            VisibleEffectSource {
                source: VisibleTarget {
                    announced: handle.clone(),
                    current: Some(handle),
                },
                // CR 708.5: a face-down creature's identity stays hidden.
                oracle_id: self.visible_ability_oracle_id(object)?.to_owned(),
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
    fn catalog_verbs(&self) -> impl Iterator<Item = &mtg_contracts::kir::Verb> {
        self.prepared.catalog.definitions.iter().flat_map(|d| {
            d.spell_programs()
                .chain(d.activated.iter().flat_map(|ability| ability.programs()))
                .chain(d.triggered.iter().flat_map(|ability| ability.programs()))
                .flat_map(|p| &p.instructions)
                .filter_map(Instruction::verb)
        })
    }

    fn characteristic_set_vocabulary(&self) -> impl Iterator<Item = &CharacteristicSet> {
        self.catalog_verbs().filter_map(|verb| match verb {
            mtg_contracts::kir::Verb::SetCharacteristics { values, .. } => Some(values),
            _ => None,
        })
    }

    // CR 510.2, 613.1f: the captured keywords include resolved and static
    // grants. A static grant can disappear after capture. Admission bounds
    // the historical vocabulary, and restore replays the actual capture.
    fn historical_granted_keyword(&self, keyword: KeywordAbility) -> bool {
        use mtg_contracts::kir::Verb;
        self.catalog_verbs().any(|verb| match verb {
            Verb::ModifyUntil { change: Change::AddKeywords { keywords }, .. } => keywords.contains(&keyword),
            Verb::SetCharacteristics { values, .. } => values.keywords.contains(&keyword),
            _ => false,
        }) || self.prepared.catalog.definitions.iter().any(|d| {
            d.static_abilities.iter().any(|a| match &a.effect {
                StaticEffect::Continuous { changes, .. } => changes.iter().any(|change|
                    matches!(change, Change::AddKeywords { keywords } if keywords.contains(&keyword))),
                _ => false,
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

    /// `historical_creature_possible` for one object: also a card with morph,
    /// which may have been a face-down 2/2 creature when the history was
    /// made (CR 708.2a, 708.8). Replay authenticates which it was.
    pub(super) fn object_creature_possible(&self, object: ObjectId) -> Result<bool, String> {
        Ok(
            self.historical_creature_possible(&self.copiable_definition(object)?.characteristics)
                || self.definition(object.card)?.face_down.is_some(),
        )
    }

    /// `validate_characteristic_identity` for captured facts, whose transform
    /// count names the face they were captured with (CR 712.18).
    /// CR 122.1, 608.2h: retained counters feed last-known reads, so they
    /// hold only admitted kinds, exactly as a permanent's own counters do.
    pub(crate) fn validate_fact_counters(fact: &ObjectBefore) -> Result<(), String> {
        if !tokens::counter_kinds_admitted(&fact.counters) {
            return Err("captured counter kind admission".into());
        }
        if fact.plus_one_counters.is_negative()
            || fact.loyalty_counters.is_negative()
            || !tokens::counter_stacks_valid(&fact.counters)
        {
            return Err("captured counter facts".into());
        }
        Ok(())
    }

    pub(crate) fn validate_fact_identity(&self, fact: &ObjectBefore) -> Result<(), String> {
        Self::validate_fact_counters(fact)?;
        self.validate_characteristic_identity(
            fact.object,
            fact.transforms,
            fact.face_down,
            fact.zone,
            &fact.characteristics,
        )
    }

    // Copiable identity is immutable within each incarnation except for the
    // face a transforming permanent has up, which `transforms` records.
    // Independent replay proves the continuous changes present at the
    // capture boundary.
    pub(crate) fn validate_characteristic_identity(
        &self,
        source: ObjectId,
        transforms: u32,
        face_down: bool,
        zone: Zone,
        c: &Characteristics,
    ) -> Result<(), String> {
        let definition = self.definition(source.card)?;
        if transforms > 0 && !definition.transforms() {
            return Err("captured facts transformed a card that does not transform".into());
        }
        // CR 708.2: face-down facts have the face-down copiable values.
        if face_down && (transforms > 0 || definition.face_down.is_none()) {
            return Err("captured face-down facts for a card that cannot be face down".into());
        }
        let face = self.fact_definition(source, transforms, face_down)?;
        let mut base = if definition.layout == FaceLayout::Split && zone != Zone::Stack {
            if source.face != CardFace::Front {
                return Err("captured split half outside the stack".into());
            }
            definition.card_characteristics().into_owned()
        } else {
            face.characteristics.clone()
        };
        let has_stats =
            c.has_type(CardKind::Creature) || (zone != Zone::Battlefield && base.power.is_some());
        if has_stats != c.power.is_some() || c.power.is_some() != c.toughness.is_some() {
            return Err("modified source has invalid stat shape".into());
        }
        // Replay authenticates which provider existed at the original capture.
        base.power = c.power.clone();
        base.toughness = c.toughness.clone();
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
                .chain(
                    self.characteristic_set_vocabulary()
                        .filter_map(|values| values.type_change.as_ref()),
                )
        };
        // CR 205.1a, 305.7: admitted type replacement can change any card
        // type or subtype to one its vocabulary names; replay proves which.
        fn replaced(change: &Change) -> Option<(&[CardKind], &[String])> {
            match change {
                Change::SetCardTypes { kinds } => Some((kinds.as_slice(), &[])),
                Change::SetCreatureSubtypes { subtypes } | Change::SetLandSubtypes { subtypes } => {
                    Some((&[], subtypes.as_slice()))
                }
                Change::RemoveCardTypes { .. } => Some((&[], &[])),
                _ => None,
            }
        }
        if vocabulary().any(|change| replaced(change).is_some()) {
            let kinds_ok = std::iter::once(c.kind)
                .chain(c.additional_types.iter().copied())
                .all(|kind| {
                    base.has_type(kind)
                        || vocabulary().any(|change| match change {
                            Change::AddTypes { kinds } => kinds.contains(&kind),
                            other => replaced(other).is_some_and(|(k, _)| k.contains(&kind)),
                        })
                        || (matches!(kind, CardKind::Creature | CardKind::Artifact)
                            && self
                                .characteristic_set_vocabulary()
                                .any(|values| values.animation.is_some()))
                });
            let subtypes_ok = c.subtypes.iter().all(|s| {
                base.subtypes.contains(s)
                    || vocabulary().any(|change| match change {
                        Change::AddCreatureSubtypes { subtypes } => subtypes.contains(s),
                        other => replaced(other).is_some_and(|(_, names)| names.contains(s)),
                    })
                    || self
                        .characteristic_set_vocabulary()
                        .filter_map(|value| value.animation.as_ref())
                        .any(|a| a.creature_subtypes.contains(s))
            });
            if !kinds_ok
                || !subtypes_ok
                || c.additional_types.contains(&c.kind)
                || c.additional_types
                    .windows(2)
                    .any(|kinds| kinds[0] >= kinds[1])
            {
                return Err("historical type outside admitted type replacement".into());
            }
            base.kind = c.kind;
            base.additional_types.clone_from(&c.additional_types);
            base.subtypes.clone_from(&c.subtypes);
            base.all_creature_types = c.all_creature_types;
        }
        if (base.additional_types != c.additional_types || base.colors != c.colors)
            && (base.additional_types.iter().any(|kind| !c.has_type(*kind))
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
                    && !self.characteristic_set_vocabulary().any(|values| values.colors.as_ref() == Some(&c.colors))))
        {
            return Err("historical type or color outside admitted static changes".into());
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
                        && !vocabulary().any(|change| {
                            matches!(change, Change::AddCreatureSubtypes { subtypes } if subtypes.contains(s))
                        })
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
        // CR 613.1d: a layer-4 static can make the object every creature
        // type, and a resolved creature-subtype replacement can remove it.
        if base.all_creature_types != c.all_creature_types {
            let gained = c.all_creature_types
                && vocabulary().any(|change| matches!(change, Change::AllCreatureTypes));
            let lost = !c.all_creature_types
                && self
                    .characteristic_set_vocabulary()
                    .filter_map(|value| value.animation.as_ref())
                    .any(|a| a.replace_creature_subtypes);
            if !gained && !lost {
                return Err("historical creature types outside admitted changes".into());
            }
            base.all_creature_types = c.all_creature_types;
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
            transforms: frozen.transforms,
            face_down: frozen.face_down,
            characteristics: frozen.characteristics.clone(),
            keywords: frozen.keywords.clone(),
            plus_one_counters: frozen.plus_one_counters.clone(),
            counters: frozen.counters.clone(),
            // Reversible-row facts outside the damage source's consumed identity.
            loyalty_counters: frozen.loyalty_counters.clone(),
            loyalty_used_turn: frozen.loyalty_used_turn,
            attached_to: frozen.attached_to,
            attachment_timestamp: frozen.attachment_timestamp,
            phased_out: frozen.phased_out,
            hideaway: frozen.hideaway.clone(),
            adventure_player: frozen.adventure_player,
            ..current
        } == *frozen
    }
    fn validate_source_appearance(
        &self,
        damage: &PreparedDamage,
        fact: &ObjectBefore,
    ) -> Result<(), String> {
        self.validate_fact_identity(fact)?;
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
                transforms: fact.transforms,
                face_down: fact.face_down,
                controller: fact.controller,
                // CR 702.30a: a control change restarts control history.
                control_upkeeps: fact.control_upkeeps,
                attack_ready: fact.attack_ready,
                characteristics: fact.characteristics.clone(),
                keywords: fact.keywords.clone(),
                plus_one_counters: fact.plus_one_counters.clone(),
                counters: fact.counters.clone(),
                loyalty_counters: fact.loyalty_counters.clone(),
                loyalty_used_turn: fact.loyalty_used_turn,
                attached_to: fact.attached_to,
                attachment_timestamp: fact.attachment_timestamp,
                phased_out: fact.phased_out,
                hideaway: fact.hideaway.clone(),
                adventure_player: fact.adventure_player,
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
        // Static damage abilities are bounded by the unique (event, ability)
        // prefix below; each needs a distinct live battlefield ability.
        let applications = tx
            .applications
            .iter()
            .filter(|a| !matches!(a.effect, EffectKey::StaticAbility { .. }))
            .count();
        if tx.entries.is_empty()
            || tx.entries.len() > self.damage_limit()
            // At most one additional intrinsic source per original occurrence.
            || applications > existing_limit.saturating_add(tx.entries.len())
            || (applications > existing_limit && tx.applications.iter().filter(|a|
                !matches!(a.effect, EffectKey::IntrinsicProtectionFromEverything { .. }
                    | EffectKey::IntrinsicProtectionFromColor { .. }
                    | EffectKey::StaticAbility { .. })
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
        let mut assigned_by_source: BTreeMap<ObjectId, Quantity> = BTreeMap::new();
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
            self.validate_fact_identity(&d.source)?;
            if let Some(fact) = &d.source_appearance {
                self.validate_source_appearance(d, fact)?;
            }
            // CR 712.18: the face the damage source had when its facts were
            // captured.
            let source_definition =
                self.fact_definition(d.source.object, d.source.transforms, d.source.face_down)?;
            let expected_keywords = source_definition.keywords.as_slice();
            let battlefield_creature = d.source.zone == Zone::Battlefield
                && d.source.characteristics.has_type(CardKind::Creature);
            // CR 113.10, 305.7: an admitted static can remove printed keywords.
            let removable = d.source.zone == Zone::Battlefield
                && self.prepared.catalog.definitions.iter().any(|d| {
                    d.static_abilities.iter().any(|a| {
                        matches!(&a.effect, StaticEffect::Continuous { changes, .. }
                            if changes.iter().any(|c| matches!(c,
                                Change::RemoveAllAbilities { .. }
                                | Change::RemoveKeywords { .. }
                                | Change::ForbidKeywords { .. }
                                | Change::SetLandSubtypes { .. })))
                    })
                });
            let keywords_valid = expected_keywords
                .iter()
                .all(|k| removable || d.source.keywords.contains(k))
                && d.source
                    .keywords
                    .iter()
                    .enumerate()
                    .all(|(index, keyword)| {
                        !d.source.keywords[..index].contains(keyword)
                            && (expected_keywords.contains(keyword)
                                || (d.source.zone == Zone::Battlefield
                                    && (self.historical_granted_keyword(*keyword)
                                        // CR 122.1b: a keyword counter it had.
                                        || d.source.counters.iter().any(|stack| {
                                            stack.kind
                                                == mtg_contracts::kir::CounterKind::Keyword {
                                                    keyword: *keyword,
                                                }
                                        })
                                        // CR 702.84a: unearth grants haste.
                                        || (*keyword == KeywordAbility::Haste
                                            && (d.source.unearthed
                                                // CR 702.109a: so does dash.
                                                || d.source.dashed
                                                // CR 702.141a: so does encore.
                                                || matches!(
                                                    self.state.rules.tokens.get(&d.source.object.card),
                                                    Some(tokens::TokenIdentity { spec: tokens::TokenTemplate::EncoreCopy { .. }, .. })
                                                ))))))
                    });
            if !keywords_valid {
                return Err(
                    "damage source keyword LKI outside admitted intrinsic/granted abilities".into(),
                );
            }
            if let Ok(current) = self.current_source_facts(d.source.object)
                && !self.prepared_damage_source_matches(d, current)
            {
                return Err("prepared damage source changed consumed facts".into());
            }
            if tx.kind == DamageKind::Combat {
                if !battlefield_creature || d.source.controller != d.controller {
                    return Err("invalid assigned combat source".into());
                }
                assigned_by_source
                    .entry(d.source.object)
                    .or_default()
                    .add(&d.original_amount);
            }
        }
        match (&tx.kind, &tx.continuation) {
            (DamageKind::Combat, DamageContinuation::Combat { toughness_sources }) => {
                if tx.cause != BatchCause::Combat {
                    return Err("wrong combat damage cause".into());
                }
                if toughness_sources
                    .iter()
                    .any(|source| !assigned_by_source.contains_key(source))
                {
                    return Err("unknown toughness assignment source".into());
                }
                for (source, assigned) in assigned_by_source {
                    let fact = &tx
                        .entries
                        .iter()
                        .find(|d| d.source.object == source)
                        .ok_or("missing combat source")?
                        .source;
                    let characteristic = if toughness_sources.contains(&source) {
                        &fact.characteristics.toughness
                    } else {
                        &fact.characteristics.power
                    };
                    if characteristic.as_ref() != Some(&assigned) {
                        return Err(
                            "combat assigned total differs from frozen characteristic".into()
                        );
                    }
                }
            }
            (DamageKind::Noncombat, DamageContinuation::Mana { activation }) => {
                self.validate_mana_rider_damage(tx, activation)?;
            }
            (DamageKind::Noncombat, DamageContinuation::Kernel { resume }) => {
                self.validate_kernel_damage(tx, *resume)?;
            }
            (DamageKind::Noncombat, DamageContinuation::Instruction { frame }) => {
                self.validate_resolving(frame)?;
                let instruction = self
                    .program_for(self.entry(frame.stack_id)?)?
                    .instructions
                    .get(frame.cursor as usize)
                    .ok_or("missing damage continuation instruction")?;
                let kernel!(Fight { first, second } @ source_block) = instruction else {
                    return Err("damage continuation does not name a fight".into());
                };
                self.validate_fight_damage(tx, frame, first, second, *source_block)?;
            }
            _ => return Err("wrong damage continuation kind".into()),
        }
        let mut amounts = tx
            .entries
            .iter()
            .map(|d| (d.id, d.original_amount.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut used: BTreeMap<DamageId, Vec<EffectKey>> = BTreeMap::new();
        let mut shields: BTreeMap<EffectId, Quantity> = BTreeMap::new();
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
                    if d.recipient != (DamageRecipient::Permanent { object: *source })
                        || !self.printed_protection_from_everything(*source)?
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
                    self.validate_fact_identity(protector)?;
                    self.validate_source_appearance(d, &qualification.damage_source)?;
                    // Replay proves the already-consumed recipient and damage-time
                    // colors even when either object or its color provider left.
                    if d.recipient != (DamageRecipient::Permanent { object: *source })
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
                        ResolvedEffectKind::PreventDamage { remaining, .. } => {
                            // CR 615.7: the shield's value before this
                            // transaction less what earlier members used.
                            let prevented = match remaining {
                                None => a.before.clone(),
                                Some(left) => {
                                    let mut left = left.clone();
                                    left.subtract(&shields.get(id).cloned().unwrap_or_default());
                                    left.min(a.before.clone())
                                }
                            };
                            if prevented <= Quantity::zero() {
                                return Err("overdrawn prevention shield prefix".into());
                            }
                            shields.entry(*id).or_default().add(&prevented);
                            let mut after = a.before.clone();
                            after.subtract(&prevented);
                            (after, prevented)
                        }

                        _ => return Err("applied effect wrong domain".into()),
                    }
                }
                EffectKey::StaticAbility { source, ability } => {
                    // The consumed static source may since have departed;
                    // its consumed facts bind the applied ability.
                    let qualification = a
                        .qualification
                        .as_deref()
                        .ok_or("missing consumed static damage facts")?;
                    let holder = &qualification.effect_source;
                    self.validate_fact_identity(holder)?;
                    self.validate_source_appearance(d, &qualification.damage_source)?;
                    if holder.object != *source || holder.zone != Zone::Battlefield {
                        return Err("invalid static damage prefix".into());
                    }
                    // CR 712.18: the face the holder had up when its
                    // ability applied, even after it departed (CR 800.4a).
                    let definition = self.fact_definition(
                        ObjectId {
                            card: source.card,
                            ..holder.object
                        },
                        holder.transforms,
                        holder.face_down,
                    )?;
                    let Some(StaticEffect::Rule { rule, .. }) = definition
                        .static_abilities
                        .get(*ability as usize)
                        .map(|a| &a.effect)
                    else {
                        return Err("static damage prefix names no rule".into());
                    };
                    Self::static_rule_amounts(rule, &a.before)?
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

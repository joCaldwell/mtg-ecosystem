//! Finite CR 611/613 additive effects and CR 614/615/616 damage processing.
//! Applied replacement prefixes are semantic steps; life/marks remain atomic.
//! Exact46-card tables:12 creatures *4 Artists <=48 death triggers per
//! nonempty-stack episode; <=13 other entries gives <=61, below128. Empty-stack
//! permanent casting and no recursion prevent accumulating episodes. Two Healers
//! tap at most once before cleanup, so two Growths +Disfigure +Fog +two shields
//! give <=6 live resolved effects (reviewed cap16). <=24 positive combat edges.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

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
    Static { source: ObjectId, ability: u32 },
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
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
    }

    fn survives_cleanup(&self, owner: PlayerId, turn: u64) -> bool {
        self.extends_beyond_creation_turn()
            && (self.lasts_until_controller_turn()
                || self.origin.controller != owner
                || self.expires_in_cleanup_of_turn == turn)
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
    Mana {
        activation: Box<ManaActivation>,
        departed: bool,
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
impl RulesTxn {
    // CR 205.3i, 305.6–7, 611.3 and 613.1d. Preserve printed order and add
    // missing basic subtypes once, independent of the number of granting sources.
    fn add_basic_land_types(c: &mut Characteristics) {
        for subtype in ["Plains", "Island", "Swamp", "Mountain", "Forest"] {
            if !c.subtypes.iter().any(|s| s == subtype) {
                c.subtypes.push(subtype.into());
            }
        }
    }

    pub(super) fn apply_land_subtype_grants(
        &self,
        object: ObjectId,
        c: &mut Characteristics,
    ) -> Result<(), String> {
        self.apply_land_subtype_grants_with_sources(object, c, None)
    }

    fn apply_land_subtype_grants_with_sources(
        &self,
        object: ObjectId,
        c: &mut Characteristics,
        sources: Option<&[CardId]>,
    ) -> Result<(), String> {
        if !self.catalog_has_land_type_grant || !c.has_type(CardKind::Land) {
            return Ok(());
        }
        let land = self.current(object)?;
        if !land.present_on_battlefield() {
            return Ok(());
        }
        for provider in self
            .battlefield_static_sources(sources)
            .filter(|o| o.controller == land.controller)
        {
            if self
                .copiable_definition(provider.id)?
                .static_abilities
                .iter()
                .any(|a| {
                    a.functioning_zone == FunctioningZone::Battlefield
                        && a.effect == StaticEffect::GrantControllerLandsAllBasicTypes
                })
            {
                Self::add_basic_land_types(c);
                break;
            }
        }
        Ok(())
    }
    pub(super) fn structural_characteristics(
        &self,
        object: ObjectId,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        let definition = self.copiable_definition(object)?;
        let bestowed = self.current(object).is_ok_and(|o| match o.zone {
            Zone::Battlefield => {
                o.attached_to.is_some() && mtg_contracts::bestow_alternative(definition).is_some()
            }
            Zone::Stack => self.is_bestowed_spell(object),
            _ => false,
        });
        if bestowed {
            Ok(std::borrow::Cow::Owned(
                self.proposed_cast_characteristics(object.card, true)?,
            ))
        } else {
            Ok(std::borrow::Cow::Borrowed(&definition.characteristics))
        }
    }
    // CR 105.2, 611.3, 613.1b/e: battlefield membership and control
    // have already been reconciled before the color layer is evaluated.
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
        let base = self.structural_characteristics(object)?;
        let target = self.current(object)?;
        if !target.present_on_battlefield() {
            return Ok(base);
        }
        let all_colors = self.static_all_colors_timestamp(&base, target.controller, sources)?;
        let mut base = base;
        if all_colors.is_some() {
            base.to_mut().colors = MANA_COMBINATION_COLORS.to_vec();
        }
        Ok(base)
    }

    pub(super) fn battlefield_color_layer<'a>(
        &self,
        mut base: std::borrow::Cow<'a, Characteristics>,
        controller: PlayerId,
    ) -> Result<std::borrow::Cow<'a, Characteristics>, String> {
        if self
            .static_all_colors_timestamp(&base, controller, None)?
            .is_some()
        {
            base.to_mut().colors = MANA_COMBINATION_COLORS.to_vec();
        }
        Ok(base)
    }

    fn static_all_colors_timestamp(
        &self,
        base: &Characteristics,
        controller: PlayerId,
        sources: Option<&[CardId]>,
    ) -> Result<Option<u64>, String> {
        let mut latest = None;
        if self.catalog_has_nonland_color_grant && !base.has_type(CardKind::Land) {
            for source in self
                .battlefield_static_sources(sources)
                .filter(|s| s.controller == controller)
            {
                if self
                    .copiable_definition(source.id)?
                    .static_abilities
                    .iter()
                    .any(|a| {
                        a.effect == StaticEffect::ControllerNonlandsAllColors
                            && a.functioning_zone == FunctioningZone::Battlefield
                    })
                {
                    let timestamp = source.keyword_timestamp.unwrap_or(0);
                    latest = latest.max(Some(timestamp));
                }
            }
        }
        Ok(latest)
    }

    pub(crate) fn permanent_colors(
        &self,
        scope: PermanentColorScope,
        controller: PlayerId,
    ) -> Result<BTreeSet<Color>, String> {
        // CR 105.1–2/110.1: multicolored permanents contribute each color.
        // Neither color identity nor the mana a permanent can produce is a color.
        let mut colors = BTreeSet::new();
        for permanent in self.state.rules.objects.values().filter(|object| {
            object.present_on_battlefield()
                && match scope {
                    PermanentColorScope::Controller => object.controller == controller,
                }
        }) {
            colors.extend(
                self.color_layer(permanent.id)?
                    .colors
                    .iter()
                    .copied()
                    .filter(|color| *color != Color::C),
            );
        }
        Ok(colors)
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
        self.apply_land_subtype_grants_with_sources(object, &mut c, sources)?;
        // CR 122.1a / 613.4c: counters and additive effects share layer 7c.
        if let Ok(o) = self.current(object) {
            if let Some(p) = c.power.as_mut() {
                p.add(&o.plus_one_counters);
            }
            if let Some(t) = c.toughness.as_mut() {
                t.add(&o.plus_one_counters);
            }
        }
        // CR 611.3a-c/613.4c: a static source continuously defines its affected
        // set. Source/controller/zone changes therefore need no cached grants.
        if c.kind == CardKind::Creature
            && let Ok(o) = self.current(object)
            && o.present_on_battlefield()
        {
            for ability in &self.copiable_definition(object)?.static_abilities {
                if let StaticEffect::ModifySourceByDistinctPermanentColors {
                    scope,
                    power_per_color,
                    toughness_per_color,
                } = &ability.effect
                {
                    let colors = self.permanent_colors(*scope, o.controller)?;
                    let count = Quantity::from(colors.len() as i64);
                    for (stat, per_color) in [
                        (&mut c.power, power_per_color),
                        (&mut c.toughness, toughness_per_color),
                    ] {
                        let mut bonus = per_color.clone();
                        bonus.multiply(&count);
                        stat.as_mut()
                            .ok_or("missing source power or toughness")?
                            .add(&bonus);
                    }
                }
            }
            // The scoped modifiers below only consume earlier-layer types,
            // subtypes and control. No recipient depends on another 7c bonus.
            for source in self
                .battlefield_static_sources(sources)
                .filter(|s| s.controller == o.controller)
            {
                for a in &self.copiable_definition(source.id)?.static_abilities {
                    let (power, toughness) = match &a.effect {
                        StaticEffect::ControllerCreatureAnthem { power, toughness } => {
                            (power, toughness)
                        }
                        StaticEffect::ControllerScopedCreatureStats {
                            filter,
                            other,
                            power,
                            toughness,
                        } if a.functioning_zone == FunctioningZone::Battlefield
                            && (!*other || source.id != object)
                            && match filter {
                                ControllerCreatureStatFilter::Any => true,
                                ControllerCreatureStatFilter::Artifact => {
                                    c.has_type(CardKind::Artifact)
                                }
                                ControllerCreatureStatFilter::Subtypes { subtypes } => {
                                    subtypes.iter().all(|subtype| c.subtypes.contains(subtype))
                                }
                            } =>
                        {
                            (power, toughness)
                        }
                        _ => continue,
                    };
                    c.power.as_mut().ok_or("anthem creature power")?.add(power);
                    c.toughness
                        .as_mut()
                        .ok_or("anthem creature toughness")?
                        .add(toughness);
                }
            }
        }
        if self.target_is_creature(object) {
            for a in self.static_modifications_with_sources(object, sources) {
                if let StaticEffect::ModifyCreatures {
                    power, toughness, ..
                } = a
                {
                    c.power.as_mut().ok_or("attachment power")?.add(power);
                    c.toughness
                        .as_mut()
                        .ok_or("attachment toughness")?
                        .add(toughness);
                }
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
            } = &e.kind
                && targets.contains(&object)
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
    pub(crate) fn static_damage_effects(&self) -> Vec<EffectKey> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield())
            .flat_map(|o| {
                self.copiable_definition(o.id)
                    .into_iter()
                    .flat_map(move |d| {
                        d.static_abilities
                            .iter()
                            .enumerate()
                            .filter(|(_, a)| {
                                matches!(a.effect,
                            StaticEffect::DoubleDamageToPermanentOrPlayer
                            | StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer)
                            })
                            .map(move |(i, _)| EffectKey::Static {
                                source: o.id,
                                ability: i as u32,
                            })
                    })
            })
            .collect()
    }
    pub(crate) fn live_effect_keys(&self) -> Vec<EffectKey> {
        self.state
            .rules
            .effects
            .iter()
            .map(|e| EffectKey::Resolved { id: e.id })
            .chain(self.static_damage_effects())
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
            kernel!(ModifyUntil { what: source_object!(), change: mtg_contracts::kir::Change::AddKeywords { keywords }, .. } @ source_block) =>
            {
                let target = self.entry(frame.stack_id)?.source;
                if !self.target_is_creature(target) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ResolvedEffectKind::GrantKeywords {
                        target,
                        modifier: None,
                        keywords,
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
            | ResolvedEffectKind::ModifyPowerToughness { target, .. } => {
                self.capture_trigger_source_handles(*target)?
            }

            _ => vec![],
        };
        let effect = ResolvedEffect {
            target_handles,
            id: EffectId(self.state.rules.next_effect_id),
            origin: EffectOrigin {
                activated_ability: self.entry(frame.stack_id)?.kind.activated_index(),
                mode: self.entry(frame.stack_id)?.mode,
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
        mut entries: Vec<PreparedDamage>,
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
        // CR 609.7c/614.4: scoped static damage replacements need bounded
        // source-property LKI. No supported transformation creates battlefield
        // permanents during this transaction, so an absent scoped provider
        // cannot appear later. Furnace-only transactions retain their bytes.
        if self.static_damage_effects().iter().any(|key| {
            let EffectKey::Static { source, ability } = key else {
                return false;
            };
            self.copiable_definition(source).is_ok_and(|d| {
                d.static_abilities[*ability as usize].effect
                    == StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
            })
        }) {
            for entry in &mut entries {
                entry.source_appearance = Some(Box::new(entry.source.clone()));
            }
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
            EffectKey::Static { source, ability } => {
                let o = self.current(*source)?;
                let a = self
                    .copiable_definition(source)?
                    .static_abilities
                    .get(*ability as usize)
                    .ok_or("missing static effect")?;
                if !o.present_on_battlefield() || a.functioning_zone != FunctioningZone::Battlefield
                {
                    return Err("inactive static damage effect".into());
                }
                match a.effect {
                    StaticEffect::DoubleDamageToPermanentOrPlayer => {}
                    StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer => {
                        let damage_source = self.damage_source_appearance(d)?;
                        if !damage_source.characteristics.has_type(CardKind::Creature)
                            || damage_source.controller != o.controller
                        {
                            return Err(
                                "damage source outside controlled creature replacement".into()
                            );
                        }
                        qualification = Some(Box::new(DamageReplacementQualification {
                            effect_source: self.object_before(*source)?,
                            damage_source,
                        }));
                    }
                    _ => return Err("unsupported static damage effect".into()),
                }
                let mut after = d.amount.clone();
                after.add(&d.amount);
                (after, Quantity::zero())
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
    pub(crate) fn damage_effect_request(&self) -> Result<Option<Request>, String> {
        let Some((player, recipient, candidates)) = self.damage_effect_group()? else {
            return Ok(None);
        };
        if self.automatic_pair(&candidates) {
            return Ok(None);
        }
        Ok(Some(Request::DamageEffect {
            decision_id: self.state.rules.revision,
            player,
            recipient,
            candidates,
        }))
    }
    pub(super) fn validate_damage_effect(
        &self,
        player: PlayerId,
        effect: &EffectKey,
        damage: DamageId,
    ) -> Result<(), String> {
        let Some(Request::DamageEffect {
            player: p,
            candidates,
            ..
        }) = self.damage_effect_request()?
        else {
            return Err("no damage effect choice".into());
        };
        if player != p
            || !candidates.contains(&DamageEffectCandidate {
                effect: effect.clone(),
                damage,
            })
        {
            return Err("invalid damage effect application pair".into());
        }
        Ok(())
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
                let Response::DamageEffect { effect, damage } = input.response else {
                    return Err("damage effect response invariant".into());
                };
                self.validate_damage_effect(input.player, &effect, damage)?;
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
            let first_trigger = self.state.rules.next_trigger_id;
            let actual =
                self.commit(ProposedEvent::CommitDamage { transaction: tx.id }, tx.cause)?;
            match tx.continuation {
                DamageContinuation::Combat => self.grant_turn_priority()?,
                DamageContinuation::Mana {
                    activation,
                    departed,
                } => self.finish_mana_damage(*activation, actual, first_trigger, departed)?,
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
            let valid = match &input.response {
                Response::DamageEffect { effect, damage } => self
                    .validate_damage_effect(input.player, effect, *damage)
                    .is_ok(),
                _ => false,
            };
            if !valid {
                self.rules_mut().pending = None;
            }
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
                if self.entry(frame.stack_id)?.mode != effect.origin.mode
                    || self.entry(frame.stack_id)?.kind.activated_index()
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
                            what: source_object!(),
                            change: mtg_contracts::kir::Change::AddKeywords { keywords },
                            ..
                        } @ source_block),
                        ResolvedEffectKind::GrantKeywords {
                            target,
                            modifier: None,
                            keywords: grants,
                        },
                    ) => {
                        effect.origin.activated_ability.is_some()
                            && effect.origin.mode.is_none()
                            && *target == self.entry(frame.stack_id)?.source
                            && self.target_is_creature(*target)
                            && keywords == grants
                            && supported_temporary_keywords(grants)
                            && *source_block == effect.origin.source_block
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
                    | ResolvedEffectKind::ExileDamaged { targets } => {
                        effect.target_handles.len() == targets.len() * 4
                            && targets.iter().enumerate().all(|(i, o)| {
                                (0..4).all(|v| {
                                    self.object_handle_id(PlayerId(v), *o)
                                        == Ok(&effect.target_handles[i * 4 + v as usize])
                                })
                            })
                    }
                    ResolvedEffectKind::GrantKeywords { target, .. }
                    | ResolvedEffectKind::ModifyPowerToughness { target, .. } => {
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
                self.rules_mut().delayed_chapters.clear();
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
        let mut ids = BTreeSet::new();
        if self.state.rules.effects.len() > self.effect_limit() {
            return Err("effect capacity".into());
        }
        for e in &self.state.rules.effects {
            if !ids.insert(e.id)
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
                            what: source_object!(),
                            change: mtg_contracts::kir::Change::AddKeywords { keywords },
                            ..
                        } @ source_block),
                        ResolvedEffectKind::GrantKeywords {
                            target,
                            modifier: None,
                            keywords: grants,
                        },
                    ) => {
                        // Origin resolution uses the historical incarnation's immutable
                        // copiable definition. Replay proves capture and its handles.
                        e.origin.activated_ability.is_some()
                            && e.origin.mode.is_none()
                            && *target == e.origin.source
                            && keywords == grants
                            && supported_temporary_keywords(grants)
                            && *source_block == e.origin.source_block
                            && e.target_handles.len() == 4
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
                            && self.copiable_definition(target)?.characteristics.kind
                                == CardKind::Creature
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct EffectHandle(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct DamageHandle(pub String);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DamageEffectKind {
    DoubleDamage,
    PreventCombatDamage,
    ProtectionFromEachColor,
    ProtectionFromEverything,
    ProtectionFromColor,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleDamageEffectCandidate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub effect: EffectHandle,
    pub damage: DamageHandle,
    pub kind: DamageEffectKind,
    pub effect_source: VisibleTarget,
    pub effect_oracle_id: String,
    pub effect_controller: PlayerId,
    pub damage_source: VisibleTarget,
    pub damage_oracle_id: String,
    pub amount_before: Quantity,
    pub amount_after: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleEffectSource")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub struct VisibleEffectSource<O = ObjectHandle> {
    pub source: VisibleTarget<O>,
    pub oracle_id: String,
    pub controller: PlayerId,
}
impl<O> VisibleEffectSource<O> {
    fn convert<T: for<'a> From<&'a O>>(self) -> VisibleEffectSource<T> {
        VisibleEffectSource {
            source: self.source.convert(),
            oracle_id: self.oracle_id,
            controller: self.controller,
        }
    }
}
// Declare each effect once. Stored rows keep the issued integer while the
// public projection exposes its viewer-scoped text.
macro_rules! effect_field {
    ($value:ident) => {
        $value
    };
    ($value:ident, nested) => {
        $value.convert()
    };
    ($value:ident, targets) => {
        $value.into_iter().map(|row| row.convert()).collect()
    };
    ($value:ident, handles) => {
        $value.iter().map(P::from).collect()
    };
}
macro_rules! visible_effects {
    ($( $variant:ident $(($handle:ident))? {
        $( $(#[$attr:meta])* $field:ident: $ty:ty $(=> $conversion:ident)?, )*
    }, )*) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
        #[schemars(rename = "VisibleEffect")]
        pub enum VisibleEffect<H = EffectHandle, O = ObjectHandle> {
            $( $variant { $( $handle: H, )? $( $(#[$attr])* $field: $ty, )* }, )*
        }
        impl<H,O> VisibleEffect<H,O> {
            pub(in crate::engine) fn map_handle<T, P: for<'a> From<&'a O>>(self, convert: impl FnOnce(H) -> T) -> VisibleEffect<T,P> {
                match self {
                    $( Self::$variant { $( $handle, )? $( $field, )* } =>
                        VisibleEffect::$variant { $( $handle: convert($handle), )? $( $field: effect_field!($field $(, $conversion)?), )* }, )*
                }
            }
        }
    }
}

visible_effects! {
    RegenerationShield(handle) {
        origin: VisibleEffectSource<O> => nested,
        expires_in_cleanup_of_turn: u64,
    },
    DelayedChapter {
        origin: VisibleEffectSource<O> => nested,
        chapter: u32,
        expires_in_cleanup_of_turn: u64,
    },
    ProtectControllerUntilNextTurn(handle) {
        origin: VisibleEffectSource<O> => nested,
    },
    RemoveIndestructibleSet(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        expires_in_cleanup_of_turn: u64,
    },
    GrantKeywordsSet(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
        expires_in_cleanup_of_turn: u64,
    },
    PumpSet(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        power: Quantity,
        toughness: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    ExileDamaged(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        expires_in_cleanup_of_turn: u64,
    },
    GrantKeywords(handle) {
        origin: VisibleEffectSource<O> => nested,
        target: VisibleTarget<O> => nested,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
        expires_in_cleanup_of_turn: u64,
    },
    ModifyPowerToughness(handle) {
        origin: VisibleEffectSource<O> => nested,
        target: VisibleTarget<O> => nested,
        power: Quantity,
        toughness: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    PreventCombatDamage(handle) {
        origin: VisibleEffectSource<O> => nested,
        expires_in_cleanup_of_turn: u64,
    },
    AdditionalLandPlays(handle) {
        origin: VisibleEffectSource<O> => nested,
        count: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    DoubleDamageToPermanentOrPlayer(handle) {
        origin: VisibleEffectSource<O> => nested,
    },
    DoubleControllerCreatureDamageToPermanentOrPlayer(handle) {
        origin: VisibleEffectSource<O> => nested,
    },
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
            EffectKey::Static { source, .. }
            | EffectKey::IntrinsicProtectionFromEverything { source }
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
            let EffectKey::Static { source, ability } = key else {
                return Err("intrinsic prevention is visible on the damage occurrence".into());
            };
            return match self
                .copiable_definition(source)?
                .static_abilities
                .get(*ability as usize)
                .ok_or("missing visible static effect")?
                .effect
            {
                StaticEffect::DoubleDamageToPermanentOrPlayer => {
                    Ok(VisibleEffect::DoubleDamageToPermanentOrPlayer { handle, origin })
                }
                StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer => Ok(
                    VisibleEffect::DoubleControllerCreatureDamageToPermanentOrPlayer {
                        handle,
                        origin,
                    },
                ),
                _ => Err("unsupported visible static damage effect".into()),
            };
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
        let mut effects = keys
            .iter()
            .map(|key| self.project_effect(viewer, key))
            .collect::<Result<Vec<_>, _>>()?;
        for row in &self.state.rules.delayed_chapters {
            effects.push(self.visible_delayed_chapter(viewer, row)?);
        }
        Ok(effects)
    }
    pub(super) fn visible_delayed_chapter<H, O: for<'a> From<&'a publish::StoredObjectHandle>>(
        &self,
        viewer: PlayerId,
        row: &DelayedChapterRegistration,
    ) -> Result<VisibleEffect<H, O>, String> {
        let source = row.source;
        let announced = row
            .source_handles()?
            .get(viewer.0 as usize)
            .ok_or("missing delayed source reference")?
            .into();
        Ok(VisibleEffect::DelayedChapter {
            origin: VisibleEffectSource {
                source: VisibleTarget {
                    announced,
                    current: self.object_handle_id(viewer, source).map(O::from).ok(),
                },
                oracle_id: self.visible_ability_oracle_id(source)?.into(),
                controller: row.controller,
            },
            chapter: row.chapter()?,
            expires_in_cleanup_of_turn: row.turn,
        })
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
            EffectKey::Static { .. } => DamageEffectKind::DoubleDamage,
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
        self.validate_damage_effect(viewer, &effect, damage)?;
        Ok((effect, damage))
    }
}

impl RulesTxn {
    // Copiable identity is immutable within each incarnation. Historical P/T is derived at its
    // capture boundary; independent replay proves those past additive effects.
    pub(crate) fn validate_characteristic_identity(
        &self,
        source: ObjectId,
        zone: Zone,
        c: &Characteristics,
    ) -> Result<(), String> {
        // A frozen battlefield appearance can precede this incarnation's
        // transform. Replay proves whether it was captured on the old face.
        if zone == Zone::Battlefield
            && self.back_face_sequence(source).is_some()
            && let Some(front_value) = self.physical_transform_mana_value(source)
            && front_value == c.printed_mana_value()
        {
            let mut front = self.definition(source.card)?.characteristics.clone();
            if front.power.is_some() == c.power.is_some()
                && front.toughness.is_some() == c.toughness.is_some()
            {
                front.power = c.power.clone();
                front.toughness = c.toughness.clone();
                if self.catalog_has_nonland_color_grant && front.colors != c.colors {
                    front.colors = vec![Color::W, Color::U, Color::B, Color::R, Color::G];
                }
                if front == *c {
                    return Ok(());
                }
            }
        }
        // Historical appearances may precede detachment of this same incarnation.
        let mut base = if matches!(zone, Zone::Stack | Zone::Battlefield)
            && c.subtypes == ["Aura"]
            && mtg_contracts::bestow_alternative(self.copiable_definition(source)?).is_some()
        {
            self.proposed_cast_characteristics(source.card, true)?
        } else {
            self.copiable_definition(source)?.characteristics.clone()
        };
        if base.power.is_some() != c.power.is_some()
            || base.toughness.is_some() != c.toughness.is_some()
        {
            return Err("modified source lost printed stat shape".into());
        }
        if base.subtypes != c.subtypes
            && zone == Zone::Battlefield
            && base.has_type(CardKind::Land)
            && self.catalog_has_land_type_grant
        {
            Self::add_basic_land_types(&mut base);
        }
        // Replay authenticates which provider existed at the original capture.
        base.power = c.power.clone();
        base.toughness = c.toughness.clone();
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
        self.validate_characteristic_identity(fact.object, fact.zone, &fact.characteristics)?;
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
        let scoped_live = self.static_damage_effects().iter().any(|key| {
            let EffectKey::Static { source, ability } = key else {
                return false;
            };
            self.copiable_definition(source).is_ok_and(|d| {
                d.static_abilities[*ability as usize].effect
                    == StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
            })
        });
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
            self.validate_characteristic_identity(
                d.source.object,
                d.source.zone,
                &d.source.characteristics,
            )?;
            if let Some(fact) = &d.source_appearance {
                self.validate_source_appearance(d, fact)?;
            } else if scoped_live {
                return Err("scoped replacement missing bounded source facts".into());
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
                                        StaticEffect::ModifyCreatures { keywords, .. } if keywords.contains(keyword)))))))
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
            (
                DamageKind::Noncombat,
                DamageContinuation::Mana {
                    activation,
                    departed,
                },
            ) => self.validate_mana_damage(tx, activation, *departed)?,
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

                        kernel!(Damage { to: mtg_contracts::kir::DamageTo::Ref { who: mtg_contracts::kir::Ref::EventPlayer { role: mtg_contracts::kir::EventPlayerRole::ActivatingPlayer } }, amount: fixed!(amount), .. } @ source_block) => {
                            (
                                None,
                                amount,
                                *source_block,
                                TargetEntity::Player {
                                    player: Self::mana_tap_activator(self.entry(frame.stack_id)?)?,
                                },
                            )
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
                        protector.zone,
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
                EffectKey::Static { source, ability } => {
                    let effect = self
                        .copiable_definition(source)?
                        .static_abilities
                        .get(*ability as usize)
                        .ok_or("unknown applied static effect")?;
                    match (&effect.effect, &a.qualification) {
                        (StaticEffect::DoubleDamageToPermanentOrPlayer, None) => {}
                        (
                            StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer,
                            Some(qualification),
                        ) => {
                            let provider = &qualification.effect_source;
                            self.validate_characteristic_identity(
                                provider.object,
                                provider.zone,
                                &provider.characteristics,
                            )?;
                            self.validate_source_appearance(d, &qualification.damage_source)?;
                            if d.source_appearance.is_none()
                                || provider.object != *source
                                || provider.zone != Zone::Battlefield
                                || provider.controller.0 >= 4
                                || provider.default_controller.0 >= 4
                                || provider.marked_damage.is_negative()
                                || self
                                    .state
                                    .rules
                                    .cards
                                    .get(&source.card)
                                    .is_none_or(|c| c.owner != provider.owner)
                                || !qualification
                                    .damage_source
                                    .characteristics
                                    .has_type(CardKind::Creature)
                                || qualification.damage_source.controller != provider.controller
                            {
                                return Err("invalid scoped doubling qualification".into());
                            }
                        }
                        _ => return Err("invalid static doubling qualification shape".into()),
                    }
                    let mut after = a.before.clone();
                    after.add(&a.before);
                    (after, Quantity::zero())
                }
                EffectKey::Resolved { id } => {
                    if a.qualification.is_some() {
                        return Err(
                            "resolved damage effect cannot carry static qualification".into()
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

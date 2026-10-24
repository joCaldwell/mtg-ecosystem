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
    pub mode: Option<u32>,
    pub stack_id: StackId,
    pub source: ObjectId,
    pub controller: PlayerId,
    pub source_handles: Vec<ObjectHandle>,
    pub cursor: u32,
    pub source_block: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedEffect {
    pub target_handles: Vec<ObjectHandle>,
    pub id: EffectId,
    pub origin: EffectOrigin,
    pub expires_in_cleanup_of_turn: u64,
    pub kind: ResolvedEffectKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResolvedEffectKind {
    ModifyPowerToughness {
        target: ObjectId,
        power: Quantity,
        toughness: Quantity,
    },
    PreventCombatDamage,
    PreventNextDamage {
        target: TargetEntity,
        remaining: Quantity,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedDamage {
    pub source_handles: Vec<ObjectHandle>,
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
    pub damage: DamageId,
    pub effect: EffectKey,
    pub before: Quantity,
    pub after: Quantity,
    pub prevented: Quantity,
    pub budget_before: Option<Quantity>,
    pub budget_after: Option<Quantity>,
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
pub(super) struct DamageTransaction {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allocation_groups: Vec<DamageAllocationRecord>,
    pub id: DamageTransactionId,
    pub kind: DamageKind,
    pub entries: Vec<PreparedDamage>,
    pub applications: Vec<DamageApplication>,
    pub cause: BatchCause,
    pub continuation: Box<DamageContinuation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "continuation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum DamageContinuation {
    Combat,
    Instruction { frame: Box<Resolving> },
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
impl Game {
    pub(super) fn derived_characteristics(
        &self,
        object: ObjectId,
    ) -> Result<Characteristics, String> {
        let mut c = self.definition(object.card)?.characteristics.clone();
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
        if self.prepared.catalog.schema_version >= 15
            && c.kind == CardKind::Creature
            && let Ok(o) = self.current(object)
            && o.zone == Zone::Battlefield
        {
            for source in self
                .state
                .objects
                .values()
                .filter(|s| s.zone == Zone::Battlefield && s.controller == o.controller)
            {
                for a in &self.definition(source.id.card)?.static_abilities {
                    if let StaticEffect::ControllerCreatureAnthem { power, toughness } = &a.effect {
                        c.power.as_mut().ok_or("anthem creature power")?.add(power);
                        c.toughness
                            .as_mut()
                            .ok_or("anthem creature toughness")?
                            .add(toughness);
                    }
                }
            }
        }
        if self.attachments_enabled() && self.target_is_creature(object) {
            for a in self.creature_modifications(object) {
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
        for e in &self.state.effects {
            if let ResolvedEffectKind::ModifyPowerToughness {
                target,
                power,
                toughness,
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
    pub(super) fn static_damage_effects(&self) -> Vec<EffectKey> {
        self.state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
            .flat_map(|o| {
                self.definition(o.id.card).into_iter().flat_map(move |d| {
                    d.static_abilities
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| a.effect == StaticEffect::DoubleDamageToPermanentOrPlayer)
                        .map(move |(i, _)| EffectKey::Static {
                            source: o.id,
                            ability: i as u32,
                        })
                })
            })
            .collect()
    }
    pub(super) fn live_effect_keys(&self) -> Vec<EffectKey> {
        self.state.effects.iter().filter(|e|!matches!(&e.kind,ResolvedEffectKind::PreventNextDamage{remaining,..} if *remaining<=Quantity::zero())).map(|e|EffectKey::Resolved{id:e.id}).chain(self.static_damage_effects()).collect()
    }
    pub(super) fn execute_effect_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        if !self.prepared.config.effects_profile() {
            return Err("unsupported continuous effect profile".into());
        }
        let (kind, block) = match instruction {
            Instruction::ModifyPowerToughness {
                target_slot,
                power,
                toughness,
                source_block,
                ..
            } => {
                let target = *frame
                    .spell
                    .targets
                    .get(target_slot as usize)
                    .ok_or("missing modifier target")?;
                if !frame
                    .legal_targets
                    .get(target_slot as usize)
                    .copied()
                    .unwrap_or(false)
                    || !self.target_legal(target)
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
            Instruction::PreventCombatDamage { source_block, .. } => {
                (ResolvedEffectKind::PreventCombatDamage, source_block)
            }
            Instruction::PreventNextDamage {
                target_slot,
                amount,
                source_block,
                ..
            } => {
                let target = *frame
                    .spell
                    .entity_targets
                    .get(target_slot as usize)
                    .ok_or("missing prevention target")?;
                if !frame
                    .legal_targets
                    .get(target_slot as usize)
                    .copied()
                    .unwrap_or(false)
                    || !self.entity_legal(target)
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ResolvedEffectKind::PreventNextDamage {
                        target,
                        remaining: amount,
                    },
                    source_block,
                )
            }
            _ => return Err("unexpected effect instruction".into()),
        };
        let handles = match &frame.spell.kind {
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
            StackKind::Spell => frame.spell.effect_source_handles.clone(),
        };
        let target_handles = match &kind {
            ResolvedEffectKind::ModifyPowerToughness { target, .. } => {
                self.capture_trigger_source_handles(*target)?
            }
            ResolvedEffectKind::PreventNextDamage {
                target: TargetEntity::Object { object },
                ..
            } => self.capture_trigger_source_handles(*object)?,
            _ => vec![],
        };
        let effect = ResolvedEffect {
            target_handles,
            id: EffectId(self.state.next_effect_id),
            origin: EffectOrigin {
                mode: frame.spell.mode,
                stack_id: frame.spell.id,
                source: frame.spell.source,
                controller: frame.spell.controller,
                source_handles: handles,
                cursor: frame.cursor,
                source_block: block,
            },
            expires_in_cleanup_of_turn: self.state.turn.number,
            kind,
        };
        let actual = self.commit(
            ProposedEvent::CreateEffect {
                effect: Box::new(effect),
            },
            BatchCause::Instruction {
                stack_id: frame.spell.id,
                player: frame.spell.controller,
                source: frame.spell.source,
                cursor: frame.cursor,
                source_block: block,
            },
        )?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    pub(super) fn prepare_combat_damage(
        &mut self,
        damage: Vec<DamageOccurrence>,
    ) -> Result<(), String> {
        let entries = damage
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                Ok(PreparedDamage {
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
        self.state.damage_choices.clear();
        self.state.player_damage_choices.clear();
        self.begin_damage(
            DamageKind::Combat,
            entries,
            BatchCause::Combat,
            DamageContinuation::Combat,
        )
    }
    pub(super) fn prepare_noncombat_damage(
        &mut self,
        frame: Resolving,
        damage: NoncombatDamageOccurrence,
        block: u32,
    ) -> Result<(), String> {
        let cause = BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block: block,
        };
        let handles = match &frame.spell.kind {
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
            StackKind::Spell => frame.spell.effect_source_handles.clone(),
        };
        let entries = vec![PreparedDamage {
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
    fn begin_damage(
        &mut self,
        kind: DamageKind,
        entries: Vec<PreparedDamage>,
        cause: BatchCause,
        continuation: DamageContinuation,
    ) -> Result<(), String> {
        if self.tokens_enabled() && entries.len() > self.damage_limit() {
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
        let id = DamageTransactionId(self.state.next_damage_transaction_id);
        self.state.next_damage_transaction_id = self
            .state
            .next_damage_transaction_id
            .checked_add(1)
            .ok_or("damage transaction limit")?;
        self.state.frame = Frame::DamageEffects {
            transaction: DamageTransaction {
                allocation_groups: vec![],
                id,
                kind,
                entries,
                applications: vec![],
                cause,
                continuation: Box::new(continuation),
            },
        };
        Ok(())
    }
    pub(super) fn damage_transaction(&self) -> Result<&DamageTransaction, String> {
        if let Frame::DamageEffects { transaction } | Frame::DamageAllocation { transaction, .. } =
            &self.state.frame
        {
            Ok(transaction)
        } else {
            Err("missing damage transaction".into())
        }
    }
    pub(super) fn recipient_chooser(&self, recipient: &DamageRecipient) -> Option<PlayerId> {
        match recipient {
            DamageRecipient::Player { player } => self.is_live(*player).then_some(*player),
            DamageRecipient::Creature { object } => self
                .target_legal(*object)
                .then(|| self.current(*object).ok().map(|o| o.controller))
                .flatten(),
        }
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
        let (after, prevented, budget_before, budget_after) = match &pair.effect {
            EffectKey::Static { source, ability } => {
                let o = self.current(*source)?;
                let a = self
                    .definition(source.card)?
                    .static_abilities
                    .get(*ability as usize)
                    .ok_or("missing static effect")?;
                if o.zone != Zone::Battlefield
                    || a.functioning_zone != FunctioningZone::Battlefield
                    || a.effect != StaticEffect::DoubleDamageToPermanentOrPlayer
                {
                    return Err("inactive static damage effect".into());
                }
                let mut after = d.amount.clone();
                after.add(&d.amount);
                (after, Quantity::zero(), None, None)
            }
            EffectKey::Resolved { id } => match &self
                .state
                .effects
                .iter()
                .find(|e| e.id == *id)
                .ok_or("missing resolved effect")?
                .kind
            {
                ResolvedEffectKind::PreventCombatDamage if tx.kind == DamageKind::Combat => {
                    (Quantity::zero(), d.amount.clone(), None, None)
                }
                ResolvedEffectKind::PreventNextDamage { target, remaining }
                    if *target == d.recipient.entity() && *remaining > Quantity::zero() =>
                {
                    let prevented = std::cmp::min(remaining.clone(), d.amount.clone());
                    let mut after = d.amount.clone();
                    after.subtract(&prevented);
                    let mut budget = remaining.clone();
                    budget.subtract(&prevented);
                    (after, prevented, Some(remaining.clone()), Some(budget))
                }
                _ => return Err("resolved effect not applicable".into()),
            },
        };
        Ok(DamageApplication {
            damage: d.id,
            effect: pair.effect.clone(),
            before: d.amount.clone(),
            after,
            prevented,
            budget_before,
            budget_after,
        })
    }
    pub(super) fn damage_effect_group(
        &self,
    ) -> Result<Option<(PlayerId, TargetEntity, Vec<DamageEffectCandidate>)>, String> {
        let Frame::DamageEffects { transaction: tx } = &self.state.frame else {
            return Ok(None);
        };
        let mut order = self.state.original_seating.clone();
        let anchor = order
            .iter()
            .position(|p| *p == self.state.turn_owner)
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
            let pairs = tx
                .entries
                .iter()
                .filter(|d| d.recipient == recipient)
                .flat_map(|d| {
                    keys.iter().map(move |k| DamageEffectCandidate {
                        effect: k.clone(),
                        damage: d.id,
                    })
                })
                .filter(|p| self.application(tx, p).is_ok())
                .collect::<Vec<_>>();
            let mut canonical = vec![];
            for pair in pairs {
                if self.needs_allocation(tx, &pair.effect)
                    && canonical
                        .iter()
                        .any(|p: &DamageEffectCandidate| p.effect == pair.effect)
                {
                    continue;
                }
                canonical.push(pair);
            }
            if !canonical.is_empty() {
                return Ok(Some((player, recipient.entity(), canonical)));
            }
        }
        Ok(None)
    }
    fn automatic_pair(&self, pairs: &[DamageEffectCandidate]) -> bool {
        pairs.len() == 1
            || (pairs.iter().all(|p| p.effect == pairs[0].effect)
                && !matches!(&pairs[0].effect,EffectKey::Resolved{id} if self.state.effects.iter().any(|e|e.id==*id&&matches!(e.kind,ResolvedEffectKind::PreventNextDamage{..}))))
    }
    pub(super) fn damage_effect_request(&self) -> Result<Option<Request>, String> {
        let Some((player, recipient, candidates)) = self.damage_effect_group()? else {
            return Ok(None);
        };
        if self.automatic_pair(&candidates) {
            return Ok(None);
        }
        Ok(Some(Request::DamageEffect {
            decision_id: self.state.revision,
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
    pub(super) fn damage_effect_step(&mut self) -> Result<(), String> {
        if let Some((_, _, pairs)) = self.damage_effect_group()? {
            let pair = if self.automatic_pair(&pairs) {
                self.state.pending = None;
                pairs[0].clone()
            } else {
                let input = self
                    .state
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
            if self.needs_allocation(&tx, &pair.effect) {
                self.state.frame = Frame::DamageAllocation {
                    transaction: tx,
                    effect: pair.effect,
                };
                return Ok(());
            }
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
            match *tx.continuation {
                DamageContinuation::Combat => self.grant_turn_priority()?,
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
        if let Frame::DamageEffects { .. } | Frame::DamageAllocation { .. } = self.state.frame
            && let Some(input) = &self.state.pending
        {
            let valid = match &input.response {
                Response::DamageAllocation { allocations } => self
                    .validate_damage_allocation(input.player, allocations)
                    .is_ok(),
                Response::DamageEffect { effect, damage } => self
                    .validate_damage_effect(input.player, effect, *damage)
                    .is_ok(),
                _ => false,
            };
            if !valid {
                self.state.pending = None;
            }
        }
        Ok(())
    }
    pub(super) fn apply_effect_event(
        &mut self,
        event: &ProposedEvent,
    ) -> Result<EventOutcome, String> {
        if !self.prepared.config.effects_profile() {
            return Err("unsupported effects event".into());
        }
        match event {
            ProposedEvent::CreateEffect { effect } => {
                if self.removal_enabled() && self.state.effects.len() >= self.effect_limit() {
                    return Err(self.capacity_error(RuleFault::EffectCapacityExceeded {
                        capacity: self.effect_limit(),
                        requested: self.state.effects.len() + 1,
                    }));
                }
                if effect.id != EffectId(self.state.next_effect_id)
                    || self.state.effects.len() >= self.effect_limit()
                    || effect.expires_in_cleanup_of_turn != self.state.turn.number
                    || effect.origin.controller.0 >= 4
                    || effect.origin.source_handles.len() != 4
                {
                    return Err("invalid effect creation identity".into());
                }
                let Frame::Resolve { frame } = &self.state.frame else {
                    return Err("effect outside resolving instruction".into());
                };
                if frame.spell.mode != effect.origin.mode
                    || frame.spell.source != effect.origin.source
                    || frame.spell.id != effect.origin.stack_id
                    || frame.spell.controller != effect.origin.controller
                    || frame.cursor != effect.origin.cursor
                {
                    return Err("unbound continuous effect".into());
                }
                let instruction = self
                    .program_for(&frame.spell)?
                    .instructions
                    .get(frame.cursor as usize)
                    .ok_or("effect instruction absent")?;
                let valid = match (instruction, &effect.kind) {
                    (
                        Instruction::ModifyPowerToughness {
                            target_slot,
                            power,
                            toughness,
                            source_block,
                            ..
                        },
                        ResolvedEffectKind::ModifyPowerToughness {
                            target,
                            power: p,
                            toughness: t,
                        },
                    ) => {
                        frame.spell.targets.get(*target_slot as usize) == Some(target)
                            && frame.legal_targets.get(*target_slot as usize) == Some(&true)
                            && self.target_legal(*target)
                            && power == p
                            && toughness == t
                            && *source_block == effect.origin.source_block
                    }
                    (
                        Instruction::PreventCombatDamage { source_block, .. },
                        ResolvedEffectKind::PreventCombatDamage,
                    ) => *source_block == effect.origin.source_block,
                    (
                        Instruction::PreventNextDamage {
                            target_slot,
                            amount,
                            source_block,
                            ..
                        },
                        ResolvedEffectKind::PreventNextDamage { target, remaining },
                    ) => {
                        frame.spell.entity_targets.get(*target_slot as usize) == Some(target)
                            && frame.legal_targets.get(*target_slot as usize) == Some(&true)
                            && self.entity_legal(*target)
                            && amount == remaining
                            && *source_block == effect.origin.source_block
                    }
                    _ => false,
                };
                let valid_handles = match &frame.spell.kind {
                    StackKind::Activated { source_handles, .. }
                    | StackKind::Triggered { source_handles, .. } => {
                        *source_handles == effect.origin.source_handles
                    }
                    StackKind::Spell => {
                        frame.spell.effect_source_handles == effect.origin.source_handles
                    }
                };
                let valid_target_handles = match &effect.kind {
                    ResolvedEffectKind::ModifyPowerToughness { target, .. } => {
                        effect.target_handles.len() == 4
                            && (0..4).all(|v| {
                                self.handle_for(PlayerId(v), *target).as_ref()
                                    == Ok(&effect.target_handles[v as usize])
                            })
                    }
                    ResolvedEffectKind::PreventNextDamage {
                        target: TargetEntity::Object { object },
                        ..
                    } => {
                        effect.target_handles.len() == 4
                            && (0..4).all(|v| {
                                self.handle_for(PlayerId(v), *object).as_ref()
                                    == Ok(&effect.target_handles[v as usize])
                            })
                    }
                    _ => effect.target_handles.is_empty(),
                };
                if !valid || !valid_handles || !valid_target_handles {
                    return Err("invalid continuous effect facts".into());
                }
                self.state.next_effect_id = self
                    .state
                    .next_effect_id
                    .checked_add(1)
                    .ok_or("effect identity limit")?;
                self.state.effects.push(*effect.clone());
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
                if self.needs_allocation(tx, effect) {
                    return Err("multi-source shield requires complete allocation vector".into());
                }
                let a = self.application(tx, &pair)?;
                if let EffectKey::Resolved { id } = effect
                    && let Some(after) = &a.budget_after
                {
                    let e = self
                        .state
                        .effects
                        .iter_mut()
                        .find(|e| e.id == *id)
                        .ok_or("missing finite shield")?;
                    let ResolvedEffectKind::PreventNextDamage { remaining, .. } = &mut e.kind
                    else {
                        return Err("shield budget invariant".into());
                    };
                    *remaining = after.clone();
                }
                let Frame::DamageEffects { transaction: tx } = &mut self.state.frame else {
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
            ProposedEvent::AllocateDamageEffect {
                transaction,
                effect,
                allocations,
            } => self.apply_damage_allocation(*transaction, effect, allocations),
            ProposedEvent::CommitDamage { transaction } => self.commit_actual_damage(*transaction),
            ProposedEvent::CleanupEffects => {
                let expired = self.state.effects.iter().map(|e| e.id).collect();
                let previous = self
                    .state
                    .objects
                    .values()
                    .filter(|o| o.marked_damage > Quantity::zero())
                    .map(|o| (o.id, o.marked_damage.clone()))
                    .collect();
                self.state.effects.clear();
                for o in self.state.objects.values_mut() {
                    o.marked_damage = Quantity::zero();
                }
                Ok(EventOutcome::EffectsCleaned { expired, previous })
            }
            _ => Err("not an effects event".into()),
        }
    }
    fn commit_actual_damage(&mut self, id: DamageTransactionId) -> Result<EventOutcome, String> {
        if !matches!(self.state.frame, Frame::DamageEffects { .. }) {
            return Err("damage commit while allocation incomplete".into());
        }
        let tx = self.damage_transaction()?.clone();
        self.validate_damage_transaction(&tx)?;
        if tx.id != id || self.damage_effect_group()?.is_some() {
            return Err("incomplete damage transformation".into());
        }
        let mut actual = vec![];
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
                let x = &mut self.state.players[player.0 as usize].life;
                let before = x.clone();
                x.subtract(&amount);
                (DamageRecipient::Player { player }, before, x.clone())
            } else {
                let o = self
                    .state
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
                .commanders
                .history(commander)
                .and_then(|h| h.combat_damage.get(&recipient))
                .cloned()
                .unwrap_or_default();
            self.state
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
        let life_gains = self.damage_keywords(&actual)?;
        Ok(EventOutcome::DamageCommitted {
            life_gains,
            transaction: id,
            actual,
            results,
            commander_damage,
            cancelled,
        })
    }
    pub(super) fn validate_effect_state(&self) -> Result<(), String> {
        if !self.prepared.config.effects_profile() {
            if !self.state.effects.is_empty()
                || self.state.next_effect_id != 0
                || self.state.next_damage_transaction_id != 0
                || matches!(
                    self.state.frame,
                    Frame::DamageEffects { .. } | Frame::DamageAllocation { .. }
                )
            {
                return Err("effects outside admitted profile".into());
            }
            return Ok(());
        }
        let mut ids = BTreeSet::new();
        if self.state.effects.len() > self.effect_limit() {
            return Err("effect capacity".into());
        }
        for e in &self.state.effects {
            if !ids.insert(e.id)
                || e.id.0 >= self.state.next_effect_id
                || e.origin.controller.0 >= 4
                || e.expires_in_cleanup_of_turn != self.state.turn.number
                || e.origin.source_handles.len() != 4
                || !self.state.cards.contains_key(&e.origin.source.card)
            {
                return Err("effect state identity invariant".into());
            }
            if e.origin.stack_id.0 >= self.state.next_stack_id {
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
                        Instruction::ModifyPowerToughness {
                            power,
                            toughness,
                            source_block,
                            ..
                        },
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
                            && self.state.cards.contains_key(&target.card)
                            && self.definition(target.card)?.characteristics.kind
                                == CardKind::Creature
                    }
                    (
                        Instruction::PreventCombatDamage { source_block, .. },
                        ResolvedEffectKind::PreventCombatDamage,
                    ) => *source_block == e.origin.source_block && e.target_handles.is_empty(),
                    (
                        Instruction::PreventNextDamage {
                            amount,
                            source_block,
                            ..
                        },
                        ResolvedEffectKind::PreventNextDamage { target, remaining },
                    ) => {
                        *source_block == e.origin.source_block
                            && !remaining.is_negative()
                            && remaining <= amount
                            && match target {
                                TargetEntity::Player { player } => {
                                    player.0 < 4 && e.target_handles.is_empty()
                                }
                                TargetEntity::Object { object } => {
                                    e.target_handles.len() == 4
                                        && self.state.cards.contains_key(&object.card)
                                        && self.definition(object.card)?.characteristics.kind
                                            == CardKind::Creature
                                }
                            }
                    }
                    _ => false,
                };
            }
            if !bound {
                return Err("effect definition/provenance invariant".into());
            }
        }
        if let Frame::DamageAllocation {
            transaction,
            effect,
        } = &self.state.frame
        {
            let EffectKey::Resolved { id } = effect else {
                return Err("allocation requires finite effect identity".into());
            };
            let Some(ResolvedEffect {
                kind: ResolvedEffectKind::PreventNextDamage { remaining, .. },
                ..
            }) = self.state.effects.iter().find(|e| e.id == *id)
            else {
                return Err("allocation requires live shield".into());
            };
            if !self.prepared.config.choices_profile()
                || remaining <= &Quantity::from(1)
                || transaction
                    .entries
                    .iter()
                    .any(|d| d.applied.contains(effect))
            {
                return Err("allocation selection/prefix invariant".into());
            }
        }
        if let Frame::DamageEffects { transaction: tx }
        | Frame::DamageAllocation {
            transaction: tx, ..
        } = &self.state.frame
        {
            self.validate_damage_transaction(tx)?;
            if tx.id.0 >= self.state.next_damage_transaction_id
                || tx.entries.is_empty()
                || tx.entries.len() > self.damage_limit()
                || tx.applications.len() > self.damage_limit() * (self.effect_limit() + 2)
            {
                return Err("damage transaction capacity invariant".into());
            }
            for (i, d) in tx.entries.iter().enumerate() {
                let unique = d.applied.iter().collect::<BTreeSet<_>>();
                if d.id != DamageId(i as u32)
                    || d.original_amount <= Quantity::zero()
                    || d.amount.is_negative()
                    || d.controller.0 >= 4
                    || d.source.controller.0 >= 4
                    || unique.len() != d.applied.len()
                    || !self.state.cards.contains_key(&d.source.object.card)
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allocation_groups: Vec<DamageAllocationRecord>,
    pub id: DamageTransactionId,
    pub kind: DamageKind,
    pub entries: Vec<PreparedDamage>,
    pub applications: Vec<DamageApplication>,
}
impl From<&DamageTransaction> for DamageTransactionBefore {
    fn from(tx: &DamageTransaction) -> Self {
        Self {
            allocation_groups: tx.allocation_groups.clone(),
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
    PreventNextDamage,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleDamageEffectCandidate {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allocation_required: bool,
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
    pub budget_remaining: Option<Quantity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleEffectSource {
    pub source: VisibleTarget,
    pub oracle_id: String,
    pub controller: PlayerId,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum VisibleEffect {
    ModifyPowerToughness {
        handle: EffectHandle,
        origin: VisibleEffectSource,
        target: VisibleTarget,
        power: Quantity,
        toughness: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    PreventCombatDamage {
        handle: EffectHandle,
        origin: VisibleEffectSource,
        expires_in_cleanup_of_turn: u64,
    },
    PreventNextDamage {
        handle: EffectHandle,
        origin: VisibleEffectSource,
        target: VisibleEntityTarget,
        remaining: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    DoubleDamageToPermanentOrPlayer {
        handle: EffectHandle,
        origin: VisibleEffectSource,
    },
}
impl Game {
    pub(super) fn refresh_effect_handles(&mut self) -> Result<(), String> {
        let keys = self.live_effect_keys();
        let damages = self
            .damage_transaction()
            .ok()
            .map(|tx| tx.entries.iter().map(|d| (tx.id, d.id)).collect::<Vec<_>>())
            .unwrap_or_default();
        for v in 0..4 {
            let k = &mut self.state.visibility[v];
            k.effect_handles.retain(|(key, _)| keys.contains(key));
            k.damage_handles.retain(|(key, _)| damages.contains(key));
            for key in &keys {
                if !k.effect_handles.iter().any(|(id, _)| id == key) {
                    let handle = EffectHandle(k.scoped(format!("v{v}-e{}", k.next_effect_handle)));
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
        Ok(())
    }
    pub(super) fn effect_handle(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<EffectHandle, String> {
        self.state
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| {
                k.effect_handles
                    .iter()
                    .find(|(id, _)| id == key)
                    .map(|(_, h)| h.clone())
            })
            .ok_or_else(|| "unknown visible effect".into())
    }
    fn effect_origin_view(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<VisibleEffectSource, String> {
        let (source, controller, announced) = match key {
            EffectKey::Static { source, .. } => (
                *source,
                self.current(*source)?.controller,
                self.handle_for(viewer, *source)?,
            ),
            EffectKey::Resolved { id } => {
                let e = self
                    .state
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
                        .clone(),
                )
            }
        };
        Ok(VisibleEffectSource {
            source: VisibleTarget {
                announced,
                current: self.handle_for(viewer, source).ok(),
            },
            oracle_id: self
                .state
                .cards
                .get(&source.card)
                .ok_or("missing physical effect source")?
                .oracle_id
                .clone(),
            controller,
        })
    }
    pub(super) fn visible_effect(
        &self,
        viewer: PlayerId,
        key: &EffectKey,
    ) -> Result<VisibleEffect, String> {
        let handle = self.effect_handle(viewer, key)?;
        let origin = self.effect_origin_view(viewer, key)?;
        let EffectKey::Resolved { id } = key else {
            return Ok(VisibleEffect::DoubleDamageToPermanentOrPlayer { handle, origin });
        };
        let e = self
            .state
            .effects
            .iter()
            .find(|e| e.id == *id)
            .ok_or("missing effect row")?;
        let target_view = |object| -> Result<VisibleTarget, String> {
            Ok(VisibleTarget {
                announced: e
                    .target_handles
                    .get(viewer.0 as usize)
                    .ok_or("missing historical effect target")?
                    .clone(),
                current: self.handle_for(viewer, object).ok(),
            })
        };
        Ok(match &e.kind {
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
            ResolvedEffectKind::PreventCombatDamage => VisibleEffect::PreventCombatDamage {
                handle,
                origin,
                expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
            },
            ResolvedEffectKind::PreventNextDamage { target, remaining } => {
                VisibleEffect::PreventNextDamage {
                    handle,
                    origin,
                    target: match target {
                        TargetEntity::Player { player } => {
                            VisibleEntityTarget::Player { player: *player }
                        }
                        TargetEntity::Object { object } => VisibleEntityTarget::Object {
                            object: target_view(*object)?,
                        },
                    },
                    remaining: remaining.clone(),
                    expires_in_cleanup_of_turn: e.expires_in_cleanup_of_turn,
                }
            }
        })
    }
    pub(super) fn visible_effects(&self, viewer: PlayerId) -> Result<Vec<VisibleEffect>, String> {
        self.live_effect_keys()
            .iter()
            .map(|key| self.visible_effect(viewer, key))
            .collect()
    }
    pub(super) fn visible_damage_effect_candidate(
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
        let origin = self.effect_origin_view(viewer, &pair.effect)?;
        let kind = match pair.effect {
            EffectKey::Static { .. } => DamageEffectKind::DoubleDamage,
            EffectKey::Resolved { id } => match self
                .state
                .effects
                .iter()
                .find(|e| e.id == id)
                .ok_or("missing applicable effect")?
                .kind
            {
                ResolvedEffectKind::PreventCombatDamage => DamageEffectKind::PreventCombatDamage,
                ResolvedEffectKind::PreventNextDamage { .. } => DamageEffectKind::PreventNextDamage,
                _ => return Err("non-damage effect in choice".into()),
            },
        };
        let allocation_required = self.needs_allocation(tx, &pair.effect);
        Ok(VisibleDamageEffectCandidate {
            allocation_required,
            effect: self.effect_handle(viewer, &pair.effect)?,
            damage: self
                .state
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
                    .clone(),
                current: self.handle_for(viewer, d.source.object).ok(),
            },
            damage_oracle_id: self
                .state
                .cards
                .get(&d.source.object.card)
                .ok_or("missing physical damage source")?
                .oracle_id
                .clone(),
            amount_before: a.before.clone(),
            amount_after: if allocation_required {
                a.before.clone()
            } else {
                a.after
            },
            budget_remaining: a.budget_before,
        })
    }
    pub(super) fn resolve_damage_effect_pair(
        &self,
        viewer: PlayerId,
        effect: &EffectHandle,
        damage: &DamageHandle,
    ) -> Result<(EffectKey, DamageId), String> {
        let k = self
            .state
            .visibility
            .get(viewer.0 as usize)
            .ok_or("invalid viewer")?;
        let effect = k
            .effect_handles
            .iter()
            .find(|(_, h)| h == effect)
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

impl Game {
    // Printed identity is immutable. P/T in a historical fact is derived at its
    // capture boundary; independent replay proves those past additive effects.
    pub(super) fn validate_characteristic_identity(
        &self,
        source: ObjectId,
        c: &Characteristics,
    ) -> Result<(), String> {
        let mut base = self.definition(source.card)?.characteristics.clone();
        if base.power.is_some() != c.power.is_some()
            || base.toughness.is_some() != c.toughness.is_some()
        {
            return Err("modified source lost printed stat shape".into());
        }
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
    fn frozen_damage_source_matches(&self, frozen: &ObjectBefore, current: ObjectBefore) -> bool {
        if !self.attachments_enabled() {
            return current == *frozen;
        }
        ObjectBefore {
            characteristics: frozen.characteristics.clone(),
            keywords: frozen.keywords.clone(),
            ..current
        } == *frozen
    }
    pub(super) fn validate_damage_transaction(&self, tx: &DamageTransaction) -> Result<(), String> {
        if tx.entries.is_empty()
            || tx.entries.len() > self.damage_limit()
            || tx.applications.len() > self.damage_limit() * (self.effect_limit() + 2)
            || tx.allocation_groups.len() > self.effect_limit()
            || tx.id.0 >= self.state.next_damage_transaction_id
        {
            return Err("damage transaction resource/identity invariant".into());
        }
        let mut powers: BTreeMap<ObjectId, Quantity> = BTreeMap::new();
        for (i, d) in tx.entries.iter().enumerate() {
            let physical = self
                .state
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
            let expected_keywords = if self.prepared.catalog.schema_version >= 12 {
                self.definition(d.source.object.card)?.keywords.as_slice()
            } else {
                &[]
            };
            let keywords_valid = if self.attachments_enabled() {
                let can_have_grants = d.source.zone == Zone::Battlefield
                    && d.source.characteristics.kind == CardKind::Creature;
                expected_keywords.iter().all(|k|d.source.keywords.contains(k))
                    && d.source.keywords.iter().enumerate().all(|(n,k)| {
                        !d.source.keywords[..n].contains(k)
                            && (expected_keywords.contains(k) || (can_have_grants && self.prepared.catalog.definitions.iter()
                                .flat_map(|d| &d.static_abilities).any(|a|matches!(&a.effect,StaticEffect::ModifyCreatures {keywords,..} if keywords.contains(k)))))
                    })
            } else {
                d.source.keywords == expected_keywords
            };
            if !keywords_valid {
                return Err(
                    "damage source keyword LKI outside admitted intrinsic/granted abilities".into(),
                );
            }
            if let Ok(current) = self.object_before(d.source.object)
                && !self.frozen_damage_source_matches(&d.source, current)
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
        match (&tx.kind, tx.continuation.as_ref()) {
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
            (DamageKind::Noncombat, DamageContinuation::Instruction { frame }) => {
                self.validate_resolving(frame)?;
                let instruction = self
                    .program_for(&frame.spell)?
                    .instructions
                    .get(frame.cursor as usize)
                    .ok_or("missing damage continuation instruction")?;
                let (target_slot, amount, source_block, recipient) = match instruction {
                    Instruction::DealDamage {
                        target_slot,
                        amount,
                        source_block,
                    } => (
                        *target_slot,
                        amount,
                        *source_block,
                        frame
                            .spell
                            .entity_targets
                            .get(*target_slot as usize)
                            .copied()
                            .ok_or("missing damage target")?,
                    ),
                    Instruction::DamageTargetController {
                        target_slot,
                        amount,
                        source_block,
                    } if self.removal_enabled() => (
                        *target_slot,
                        amount,
                        *source_block,
                        TargetEntity::Player {
                            player: self.removal_target_controller(frame, *target_slot)?,
                        },
                    ),
                    _ => return Err("damage continuation not damage instruction".into()),
                };
                let expected = BatchCause::Instruction {
                    stack_id: frame.spell.id,
                    player: frame.spell.controller,
                    source: frame.spell.source,
                    cursor: frame.cursor,
                    source_block,
                };
                let d = tx.entries.first().ok_or("empty noncombat damage")?;
                if tx.entries.len() != 1
                    || tx.cause != expected
                    || d.original_amount != *amount
                    || recipient != d.recipient.entity()
                    || frame.legal_targets.get(target_slot as usize) != Some(&true)
                    || d.controller != frame.spell.controller
                    || !self
                        .frozen_damage_source_matches(&d.source, self.source_facts(&frame.spell)?)
                {
                    return Err("unbound noncombat damage transaction".into());
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
        let mut budgets: BTreeMap<EffectId, Quantity> = BTreeMap::new();
        let mut end = 0usize;
        for group in &tx.allocation_groups {
            let start = group.application_start as usize;
            let next = start
                .checked_add(group.application_count as usize)
                .ok_or("allocation prefix range overflow")?;
            if group.application_count == 0 || start < end || next > tx.applications.len() {
                return Err("allocation prefix range invariant".into());
            }
            end = next;
        }
        for (index, a) in tx.applications.iter().enumerate() {
            if let Some(group) = tx
                .allocation_groups
                .iter()
                .find(|g| g.application_start as usize == index)
            {
                self.validate_allocation_prefix_group(tx, group, &amounts, &used)?;
            }
            let allocated = tx.allocation_groups.iter().find(|g| {
                (g.application_start as usize
                    ..g.application_start as usize + g.application_count as usize)
                    .contains(&index)
            });
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
                EffectKey::Static { source, ability } => {
                    let effect = self
                        .definition(source.card)?
                        .static_abilities
                        .get(*ability as usize)
                        .ok_or("unknown applied static effect")?;
                    if effect.effect != StaticEffect::DoubleDamageToPermanentOrPlayer
                        || a.budget_before.is_some()
                        || a.budget_after.is_some()
                    {
                        return Err("invalid doubling prefix facts".into());
                    }
                    let mut after = a.before.clone();
                    after.add(&a.before);
                    (after, Quantity::zero())
                }
                EffectKey::Resolved { id } => {
                    let e = self
                        .state
                        .effects
                        .iter()
                        .find(|e| e.id == *id)
                        .ok_or("missing applied resolved effect")?;
                    match &e.kind {
                        ResolvedEffectKind::PreventCombatDamage
                            if tx.kind == DamageKind::Combat
                                && a.budget_before.is_none()
                                && a.budget_after.is_none() =>
                        {
                            (Quantity::zero(), a.before.clone())
                        }
                        ResolvedEffectKind::PreventNextDamage { target, .. }
                            if *target == d.recipient.entity() =>
                        {
                            let b = a
                                .budget_before
                                .as_ref()
                                .ok_or("missing prior shield budget")?;
                            let after_budget = a
                                .budget_after
                                .as_ref()
                                .ok_or("missing next shield budget")?;
                            if b <= &Quantity::zero()
                                || b > &self.shield_limit(e)?
                                || budgets.get(id).is_some_and(|prior| prior != b)
                            {
                                return Err("invalid shield allocation budget".into());
                            }
                            let prevented = if let Some(group) = allocated {
                                if group.effect != a.effect {
                                    return Err("allocation effect range mismatch".into());
                                }
                                group
                                    .allocations
                                    .iter()
                                    .find(|(id, _)| *id == a.damage)
                                    .ok_or("missing allocated prevention amount")?
                                    .1
                                    .clone()
                            } else {
                                std::cmp::min(b.clone(), a.before.clone())
                            };
                            let mut next = b.clone();
                            next.subtract(&prevented);
                            if next != *after_budget {
                                return Err("invalid shield allocation consumption".into());
                            }
                            budgets.insert(*id, next);
                            let mut after = a.before.clone();
                            after.subtract(&prevented);
                            (after, prevented)
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
        for (id, budget) in budgets {
            let Some(ResolvedEffect {
                kind: ResolvedEffectKind::PreventNextDamage { remaining, .. },
                ..
            }) = self.state.effects.iter().find(|e| e.id == id)
            else {
                return Err("missing allocated shield".into());
            };
            if *remaining != budget {
                return Err("shield budget differs from consumed prefix".into());
            }
        }
        Ok(())
    }
}

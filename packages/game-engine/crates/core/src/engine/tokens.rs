//! Physical tokens and counters, CR 111.1–4, 111.7–8, 122.1a/2/6,
//! 302.6, 603.6a, 613.4c and 704.5d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{CounterRecipient, CreatureToken, PredefinedToken};

// Generic live-object cap. Physical token identities are never reused,
// including after a token ceases; history is retained independently.
pub const TOKEN_CAPACITY: u32 = capacities::TOKEN_CAPACITY;
const TOKEN_ID_START: u32 = 1 << 31;
pub(super) fn quantity_is_zero(n: &Quantity) -> bool {
    *n == Quantity::zero()
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fault", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuleFault {
    TriggerIdCapacityExceeded {
        next: u64,
        additional: u64,
    },
    StackIdCapacityExceeded {
        next: u32,
        additional: u64,
    },
    HistoryCapacityExceeded {
        capacity: usize,
        requested: usize,
    },
    TokenHistoryCapacityExceeded {
        capacity: usize,
        created: usize,
        requested: u32,
    },
    EventGroupDepthCapacityExceeded {
        capacity: u32,
        requested: u32,
    },
    EffectCapacityExceeded {
        capacity: usize,
        requested: usize,
    },
    DamageCapacityExceeded {
        capacity: usize,
        requested: usize,
    },
    StackCapacityExceeded {
        capacity: usize,
        requested: usize,
    },
    EventGroupCapacityExceeded {
        capacity: usize,
        requested: usize,
    },
    TokenCapacityExceeded {
        capacity: u32,
        created: u32,
        requested: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleToken {
    pub name: String,
    pub keywords: Vec<KeywordAbility>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TokenIdentity {
    pub source: ObjectId,
    pub spec: TokenTemplate,
    // Generated runtime characteristics, not an admitted source card. Keeping
    // this immutable after cessation preserves keyword/damage lookback facts,
    // including every grouped witness member's base power. Token identities
    // are retained under TOKEN_HISTORY_LIMIT, not reclaimed from a witness's
    // single subject(), so a multi-source trigger needs no new LKI archive.
    pub definition: Definition,
}
// Preserve the ordinary token template on the wire. A spell copy derives
// its complete definition from a previous physical spell, including when
// the target is itself a copy (CR 707.9b/10).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum TokenTemplate {
    Created(CreatureToken),
    Predefined {
        kind: PredefinedToken,
    },
    SpellCopy {
        original: ObjectId,
        stack_id: StackId,
    },
}

impl TokenIdentity {
    pub(super) fn is_copy(&self) -> bool {
        matches!(self.spec, TokenTemplate::SpellCopy { .. })
    }
    pub(super) fn is_token(&self, object: &Object) -> bool {
        !self.is_copy() || object.zone == Zone::Battlefield || object.id.incarnation == 2
    }
    fn valid_incarnation(&self, object: &Object) -> bool {
        match (self.is_copy(), object.zone) {
            (false, Zone::Battlefield) | (true, Zone::Stack) => object.id.incarnation == 0,
            (false, Zone::Stack) => false,
            (true, Zone::Battlefield) | (false, _) => object.id.incarnation == 1,
            (true, _) => matches!(object.id.incarnation, 1 | 2),
        }
    }
}

pub(super) fn copied_creature_definition(original: &Definition) -> Definition {
    let mut definition = original.clone();
    definition.characteristics.legendary = false;
    definition
}
fn token_definition(source: &Definition, id: CardId, token: &CreatureToken) -> Definition {
    let mut d = source.clone();
    d.oracle_id = format!("token:{}", id.0);
    d.name = format!("{} Token", token.subtypes.join(" "));
    d.characteristics = token.characteristics();
    d.intrinsic_zone = None;
    d.spell = None;
    d.activated.clear();
    d.mana_abilities.clear();
    d.triggered.clear();
    d.static_abilities.clear();
    d.keywords = token.keywords.clone();
    d.capabilities.clear();
    // Provenance names the creating instruction's source. This definition is
    // never exported as source IR, admitted to a catalog, or made castable.
    d
}
impl Game {
    pub fn typed_fault(&self) -> Option<&RuleFault> {
        self.state.rules.typed_fault.as_ref()
    }

    pub(super) fn capacity_error(&mut self, fault: RuleFault) -> String {
        let reason = match fault {
            RuleFault::TriggerIdCapacityExceeded { .. } => "trigger ID resource limit",
            RuleFault::StackIdCapacityExceeded { .. } => "stack ID resource limit",
            RuleFault::HistoryCapacityExceeded { .. } => "history capacity exceeded",
            RuleFault::TokenHistoryCapacityExceeded { .. } => "token history capacity exceeded",
            RuleFault::EventGroupDepthCapacityExceeded { .. } => {
                "event group depth capacity exceeded"
            }
            RuleFault::EffectCapacityExceeded { .. } => "effect capacity exceeded",
            RuleFault::TokenCapacityExceeded { .. } => "token capacity exceeded",
            RuleFault::DamageCapacityExceeded { .. } => "damage capacity exceeded",
            RuleFault::StackCapacityExceeded { .. } => "stack capacity exceeded",
            RuleFault::EventGroupCapacityExceeded { .. } => "event group capacity exceeded",
        };
        self.state.rules.typed_fault = Some(fault);
        reason.into()
    }
    pub(super) fn check_stack_capacity(&mut self, additional: usize) -> Result<(), String> {
        let requested = self
            .state
            .rules
            .stack
            .len()
            .saturating_add(self.state.rules.pending_triggers.len())
            .saturating_add(additional);
        if requested > self.stack_limit() {
            return Err(self.capacity_error(RuleFault::StackCapacityExceeded {
                capacity: self.stack_limit(),
                requested,
            }));
        }
        Ok(())
    }
    pub(super) fn token_capacity(&self) -> u32 {
        capacities::TOKEN_CAPACITY
    }
    pub(super) fn allocated_capacity_count(&self) -> usize {
        {
            self.state
                .rules
                .objects
                .keys()
                .filter(|id| self.state.rules.tokens.contains_key(id))
                .count()
        }
    }
    pub(super) fn event_group_limit(&self) -> usize {
        capacities::EVENT_GROUP_LIMIT
    }
    pub(super) fn check_token_capacity(&mut self, requested: u32) -> Result<(), String> {
        let created =
            u32::try_from(self.allocated_capacity_count()).map_err(|_| "token count overflow")?;
        let capacity = self.token_capacity();
        if requested > capacity.saturating_sub(created) {
            return Err(self.capacity_error(RuleFault::TokenCapacityExceeded {
                capacity,
                created,
                requested,
            }));
        }
        if requested as usize
            > capacities::TOKEN_HISTORY_LIMIT.saturating_sub(self.state.rules.tokens.len())
        {
            return Err(
                self.capacity_error(RuleFault::TokenHistoryCapacityExceeded {
                    capacity: capacities::TOKEN_HISTORY_LIMIT,
                    created: self.state.rules.tokens.len(),
                    requested,
                }),
            );
        }
        Ok(())
    }
    pub(super) fn next_token_card(&self) -> Result<CardId, String> {
        let offset =
            u32::try_from(self.state.rules.tokens.len()).map_err(|_| "token identity overflow")?;
        let card = CardId(
            TOKEN_ID_START
                .checked_add(offset)
                .ok_or("token identity overflow")?,
        );
        if self.state.rules.cards.contains_key(&card) {
            return Err("token/card identity collision".into());
        }
        Ok(card)
    }
    pub(super) fn insert_token_identity(
        &mut self,
        card: CardId,
        controller: PlayerId,
        zone: Zone,
        identity: TokenIdentity,
    ) -> ObjectId {
        let id = ObjectId {
            card,
            incarnation: 0,
        };
        self.state.rules.cards.insert(
            card,
            Card {
                owner: controller,
                oracle_id: identity.definition.oracle_id.clone(),
            },
        );
        self.state.rules.tokens.insert(card, identity);
        self.state.rules.objects.insert(
            card,
            Object {
                hideaway: None,
                phased_out: None,
                attached_to: None,
                attachment_timestamp: None,
                battlefield_entry_turn: (zone == Zone::Battlefield)
                    .then_some(self.state.rules.turn.number),
                keyword_timestamp: (zone == Zone::Battlefield)
                    .then_some(self.state.rules.next_batch_id),
                plus_one_counters: Quantity::zero(),
                lore_counters: Quantity::zero(),
                marked_damage: Quantity::zero(),
                deathtouch_damage: false,
                attack_ready: false,
                id,
                controller,
                default_controller: controller,
                zone,
                tapped: false,
            },
        );
        id
    }
    // CR 111.7, 704.5d/e: a permanent spell copy is still a spell on the
    // stack. Off it, only its resolved battlefield incarnation survives.
    pub(super) fn cessation(&self, object: &Object) -> Option<ProposedEvent> {
        let identity = self.state.rules.tokens.get(&object.id.card)?;
        if matches!(object.zone, Zone::Stack | Zone::Battlefield) {
            return None;
        }
        Some(if identity.is_token(object) {
            ProposedEvent::CeaseToken { object: object.id }
        } else {
            ProposedEvent::CeaseSpellCopy { object: object.id }
        })
    }
    pub(super) fn execute_token_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let (event, block) = match instruction {
            Instruction::CreatePredefinedTokens {
                count,
                kind,
                source_block,
            } => {
                if !self.is_live(frame.spell.controller) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                self.check_token_capacity(count)?;
                (
                    ProposedEvent::CreatePredefinedTokens {
                        controller: frame.spell.controller,
                        source: frame.spell.source,
                        count,
                        kind,
                    },
                    source_block,
                )
            }
            Instruction::CreateCreatureTokens {
                count,
                token,
                source_block,
            } => {
                if !self.is_live(frame.spell.controller) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                self.check_token_capacity(count)?;
                (
                    ProposedEvent::CreateCreatureTokens {
                        controller: frame.spell.controller,
                        source: frame.spell.source,
                        count,
                        token: Box::new(token),
                    },
                    source_block,
                )
            }
            Instruction::PutSourcePlusOneCountersForCastColors { source_block } => {
                let StackKind::Triggered {
                    witness: TriggerEventWitness::SpellCast { cast },
                    ..
                } = &frame.spell.kind
                else {
                    return Err("unbound cast color instruction".into());
                };
                // CR 608.2h/113.7a/201.5: inspect the exact spell incarnation
                // currently on the stack; otherwise the bounded witness is its
                // departure LKI in this immutable-spell-color pool. The frozen
                // cast event is identity evidence, not a universal quantity rule.
                // Spell-color modification has no admitted IR operation. Before
                // adding one, refresh separate departure LKI; never rewrite cast facts.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                let colors = if self.state.rules.stack.iter().any(|s| {
                    s.id == cast.stack_id && s.source == cast.spell.object && s.kind.is_spell()
                }) {
                    self.characteristics(cast.spell.object)?.colors
                } else {
                    cast.spell.characteristics.colors.clone()
                };
                let n = colors
                    .into_iter()
                    .filter(|c| *c != Color::C)
                    .collect::<BTreeSet<_>>()
                    .len();
                if n == 0 || !self.is_battlefield_creature(frame.spell.source) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ProposedEvent::PutPlusOneCounters {
                        object: frame.spell.source,
                        amount: Quantity::from(n as i64),
                    },
                    source_block,
                )
            }
            Instruction::PutPlusOneCounters {
                on,
                amount,
                source_block,
            } => {
                let target = match on {
                    CounterRecipient::Source => frame.spell.source,
                    CounterRecipient::Target { target_slot } => {
                        if frame.legal_targets.get(target_slot as usize) != Some(&true) {
                            return self.finish_instruction(
                                frame,
                                InstructionOutcome::Effect { actual: None },
                            );
                        }
                        *self
                            .bound_object_target(&frame.spell, target_slot as usize)
                            .ok_or("missing counter target")?
                    }
                };
                if !self.is_battlefield_creature(target) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ProposedEvent::PutPlusOneCounters {
                        object: target,
                        amount,
                    },
                    source_block,
                )
            }
            _ => return Err("unexpected token instruction".into()),
        };
        let cause = BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block: block,
        };
        if let ProposedEvent::PutPlusOneCounters { object, amount } = event {
            return self.begin_counter_placement(frame, object, amount, cause);
        }
        let actual = self.commit(event, cause)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    pub(super) fn apply_token_event(
        &mut self,
        event: &ProposedEvent,
    ) -> Result<EventOutcome, String> {
        match event {
            ProposedEvent::CreatePredefinedTokens {
                controller,
                source,
                count,
                kind,
            } => {
                if !self.is_live(*controller) || *count == 0 {
                    return Err("invalid predefined token controller or count".into());
                }
                self.check_token_capacity(*count)?;
                let definition = self.copiable_definition(source)?.clone();
                let mut objects = Vec::new();
                for _ in 0..*count {
                    let card = self.next_token_card()?;
                    let identity = TokenIdentity {
                        source: *source,
                        spec: TokenTemplate::Predefined { kind: *kind },
                        definition: predefined_token_templates::definition(
                            &definition,
                            card,
                            *kind,
                        ),
                    };
                    objects.push(self.insert_token_identity(
                        card,
                        *controller,
                        Zone::Battlefield,
                        identity,
                    ));
                }
                Ok(EventOutcome::PredefinedTokensCreated {
                    objects,
                    controller: *controller,
                    kind: *kind,
                })
            }
            ProposedEvent::CreateCreatureTokens {
                controller,
                source,
                count,
                token,
            } => {
                if !self.is_live(*controller) || *count == 0 {
                    return Err("invalid token creator/count".into());
                }
                token.validate()?;
                self.check_token_capacity(*count)?;
                let source_definition = self.copiable_definition(source)?.clone();
                let mut objects = vec![];
                for _ in 0..*count {
                    let card = self.next_token_card()?;
                    let identity = TokenIdentity {
                        source: *source,
                        spec: TokenTemplate::Created(token.as_ref().clone()),
                        definition: token_definition(&source_definition, card, token),
                    };
                    let id =
                        self.insert_token_identity(card, *controller, Zone::Battlefield, identity);
                    objects.push(id);
                }
                Ok(EventOutcome::TokensCreated {
                    objects,
                    controller: *controller,
                    token: token.clone(),
                })
            }
            ProposedEvent::CeaseToken { object } | ProposedEvent::CeaseSpellCopy { object } => {
                if self.cessation(self.current(*object)?).as_ref() != Some(event) {
                    return Err("invalid token cessation".into());
                }
                self.retain_source_lki(*object)?;
                self.state.rules.objects.remove(&object.card);
                for library in self.state.rules.libraries.values_mut() {
                    library.retain(|c| *c != object.card);
                }
                Ok(match event {
                    ProposedEvent::CeaseSpellCopy { .. } => {
                        EventOutcome::SpellCopyCeased { object: *object }
                    }
                    _ => EventOutcome::TokenCeased { object: *object },
                })
            }
            _ => Err("unexpected token event".into()),
        }
    }
    pub(super) fn validate_tokens_state(&self) -> Result<(), String> {
        if self.allocated_capacity_count() > self.token_capacity() as usize
            || self.state.rules.tokens.len() > capacities::TOKEN_HISTORY_LIMIT
        {
            return Err("snapshot token capacity invariant".into());
        }
        for (i, (id, t)) in self.state.rules.tokens.iter().enumerate() {
            if Some(id.0)
                != u32::try_from(i)
                    .ok()
                    .and_then(|i| TOKEN_ID_START.checked_add(i))
                || self.prepared.config.cards.iter().any(|c| c.id == *id)
                || self.state.rules.commanders.history(*id).is_some()
            {
                return Err("snapshot token physical identity invariant".into());
            }
            let source = self.copiable_definition(t.source)?;
            fn creates(p: &SpellProgram, spec: &CreatureToken) -> bool {
                p.instructions.iter().any(|i| matches!(i, Instruction::CreateCreatureTokens { token, .. } | Instruction::CreateTokensFor {token,..} if token == spec)) || p.modes.iter().any(|m| creates(&m.program,spec))
            }
            let expected = match &t.spec {
                TokenTemplate::Created(spec) => {
                    spec.validate()?;
                    if !source.spell.as_ref().is_some_and(|p| creates(p, spec))
                        && !source.triggered.iter().any(|a| creates(&a.program, spec))
                    {
                        return Err("snapshot token differs from creating instruction".into());
                    }
                    token_definition(source, *id, spec)
                }
                TokenTemplate::Predefined { kind } => {
                    if !predefined_token_templates::can_create(source, *kind) {
                        return Err(
                            "snapshot predefined token differs from creating instruction".into(),
                        );
                    }
                    predefined_token_templates::definition(source, *id, *kind)
                }
                TokenTemplate::SpellCopy { original, stack_id } => {
                    let original_definition = self.copiable_definition(original)?;
                    if (self.state.rules.tokens.contains_key(&original.card)
                        && original.card >= *id)
                        || stack_id.0 >= self.state.rules.next_stack_id
                    {
                        return Err("snapshot spell copy identity order".into());
                    }
                    if t.source == *original
                        && !self.state.rules.tokens.contains_key(&original.card)
                        && source.triggered.iter().any(|trigger| {
                            trigger.condition == TriggerCondition::SelfSpellCast
                                && matches!(
                                    trigger.program.instructions.as_slice(),
                                    [Instruction::Demonstrate { .. }]
                                )
                        })
                    {
                        original_definition.clone()
                    } else if original_definition
                        .characteristics
                        .has_type(CardKind::Creature)
                        && source.spell.as_ref().is_some_and(|p| {
                            matches!(
                                p.instructions.as_slice(),
                                [Instruction::CopyControlledCreatureSpell { .. }]
                            )
                        })
                    {
                        copied_creature_definition(original_definition)
                    } else {
                        return Err("snapshot spell copy source invariant".into());
                    }
                }
            };
            if t.definition != expected
                || self
                    .state
                    .rules
                    .cards
                    .get(id)
                    .is_none_or(|c| c.owner.0 >= 4 || c.oracle_id != t.definition.oracle_id)
            {
                return Err("snapshot token characteristic identity invariant".into());
            }
        }
        for (id, o) in &self.state.rules.objects {
            if o.lore_counters.is_negative()
                || (o.lore_counters != Quantity::zero()
                    && (o.zone != Zone::Battlefield || self.saga_final_chapter(o.id)?.is_none()))
            {
                return Err("snapshot Saga lore invariant".into());
            }
            if o.plus_one_counters.is_negative()
                || (o.plus_one_counters > Quantity::zero()
                    && (o.zone != Zone::Battlefield
                        || self.copiable_definition(o.id)?.characteristics.kind
                            != CardKind::Creature))
            {
                return Err("snapshot counter invariant".into());
            }
            if self.state.rules.tokens.contains_key(id)
                && (o.id.card != *id
                    || o.controller.0 >= 4
                    || o.default_controller.0 >= 4
                    || !self.is_live(o.controller)
                    || !self.is_live(self.state.rules.cards[id].owner)
                    || !self.state.rules.tokens[id].valid_incarnation(o)
                    || (o.zone != Zone::Battlefield
                        && (o.tapped || o.attack_ready || o.deathtouch_damage)))
            {
                return Err("snapshot token object invariant".into());
            }
        }
        if let Some(fault) = self.typed_fault() {
            let expected = match fault {
                RuleFault::TriggerIdCapacityExceeded { next, additional }
                    if *next >= self.state.rules.next_trigger_id
                        && next.checked_add(*additional).is_none() =>
                {
                    "trigger ID resource limit"
                }
                RuleFault::StackIdCapacityExceeded { next, additional }
                    if *next >= self.state.rules.next_stack_id
                        && u64::from(*next)
                            .checked_add(*additional)
                            .is_none_or(|end| end > u64::from(u32::MAX)) =>
                {
                    "stack ID resource limit"
                }
                RuleFault::HistoryCapacityExceeded {
                    capacity,
                    requested,
                } if *capacity == capacities::HISTORY_LIMIT && requested > capacity => {
                    "history capacity exceeded"
                }
                RuleFault::TokenHistoryCapacityExceeded {
                    capacity,
                    created,
                    requested,
                } if *capacity == capacities::TOKEN_HISTORY_LIMIT
                    && *created == self.state.rules.tokens.len()
                    && *requested as usize > capacity - created =>
                {
                    "token history capacity exceeded"
                }
                RuleFault::EventGroupDepthCapacityExceeded {
                    capacity,
                    requested,
                } if *capacity == capacities::EVENT_GROUP_DEPTH && requested > capacity => {
                    "event group depth capacity exceeded"
                }

                RuleFault::EffectCapacityExceeded {
                    capacity,
                    requested,
                } if *capacity == self.effect_limit() && *requested > *capacity => {
                    "effect capacity exceeded"
                }
                RuleFault::TokenCapacityExceeded {
                    capacity,
                    created,
                    requested,
                } if *capacity == self.token_capacity()
                    && *created == self.allocated_capacity_count() as u32
                    && *requested > capacity - created =>
                {
                    "token capacity exceeded"
                }
                RuleFault::DamageCapacityExceeded {
                    capacity,
                    requested,
                } if *capacity == self.damage_limit() && *requested > *capacity => {
                    "damage capacity exceeded"
                }
                RuleFault::StackCapacityExceeded {
                    capacity,
                    requested,
                } if *capacity == self.stack_limit() && *requested > *capacity => {
                    "stack capacity exceeded"
                }
                RuleFault::EventGroupCapacityExceeded {
                    capacity,
                    requested,
                } if *capacity == self.event_group_limit() && *requested > *capacity => {
                    "event group capacity exceeded"
                }
                _ => return Err("snapshot typed capacity fault invariant".into()),
            };
            if !matches!(&self.state.rules.frame,Frame::Fault {reason} if reason==expected) {
                return Err("snapshot typed fault frame invariant".into());
            }
        }
        Ok(())
    }
}

//! Physical tokens and counters, CR 111.1–4, 111.7–8, 122.1a/2/6,
//! 302.6, 603.6a, 613.4c and 704.5d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{CounterRecipient, CreatureToken};

// Lifetime allocation cap, not a reusable slot pool. Reviewed decks create at
// most 9 tokens per seat (36 per pod), with no recasting/recursion in this pool.
// IDs never alias one of the 400 deck cards, nor a ceased token's historical ID.
pub const TOKEN_CAPACITY: u32 = 64;
// Any admitted deck has at most 27 creature cards (BG; white has 16).
// Every trigger source is singleton per seat. Within a nonempty-stack episode
// creatures enter/die at most once; commander recasts require an empty stack.
pub(super) const CREATURE_CARD_BOUND: usize = 4 * 27;
pub(super) const CREATURE_BOUND: usize = CREATURE_CARD_BOUND + TOKEN_CAPACITY as usize;
pub(super) const DAMAGE_BOUND: usize = 2 * CREATURE_BOUND;
// Reserve all 400 physical spells, 4 tap activations, C*4 Artist deaths,
// C*4 Warden entries, 108 self-enters and 4 Bouncer self-dies triggers.
// This deliberately overcounts mutually exclusive white/BG deck allocations.
pub(super) const STACK_BOUND: usize = 400 + 4 + CREATURE_BOUND * 8 + CREATURE_CARD_BOUND + 4;
// All physical cards, allocated tokens and simultaneous player losses.
pub(super) const EVENT_GROUP_BOUND: usize = 400 + TOKEN_CAPACITY as usize + 4;
const TOKEN_ID_START: u32 = 1 << 31;
pub(super) fn quantity_is_zero(n: &Quantity) -> bool {
    *n == Quantity::zero()
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fault", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuleFault {
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
    pub spec: CreatureToken,
    // Generated runtime characteristics, not an admitted source card. Keeping
    // this immutable after cessation preserves keyword/damage lookback facts.
    pub definition: Definition,
}
fn token_definition(source: &Definition, id: CardId, token: &CreatureToken) -> Definition {
    let mut d = source.clone();
    d.oracle_id = format!("token:{}", id.0);
    d.name = format!("{} Token", token.subtypes.join(" "));
    d.characteristics = Characteristics {
        kind: CardKind::Creature,
        subtypes: token.subtypes.clone(),
        basic: false,
        legendary: false,
        mana_cost: ManaCost {
            generic: Quantity::zero(),
            colored: vec![],
        },
        colors: token.colors.clone(),
        color_identity: token.colors.clone(),
        power: Some(token.power.clone()),
        toughness: Some(token.toughness.clone()),
    };
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
        self.state.typed_fault.as_ref()
    }
    pub(super) fn tokens_enabled(&self) -> bool {
        (self.prepared.config.profile == TOKENS_PROFILE
            && self.prepared.catalog.schema_version == 14)
            || self.staples_enabled()
    }
    pub(super) fn capacity_error(&mut self, fault: RuleFault) -> String {
        let reason = match fault {
            RuleFault::TokenCapacityExceeded { .. } => "token capacity exceeded",
            RuleFault::DamageCapacityExceeded { .. } => "damage capacity exceeded",
            RuleFault::StackCapacityExceeded { .. } => "stack capacity exceeded",
            RuleFault::EventGroupCapacityExceeded { .. } => "event group capacity exceeded",
        };
        self.state.typed_fault = Some(fault);
        reason.into()
    }
    pub(super) fn token_capacity(&self) -> u32 {
        if self.staples_enabled() {
            staples::LIVE_TOKEN_CAPACITY
        } else {
            TOKEN_CAPACITY
        }
    }
    pub(super) fn allocated_capacity_count(&self) -> usize {
        if self.staples_enabled() {
            self.state
                .objects
                .keys()
                .filter(|id| self.state.tokens.contains_key(id))
                .count()
        } else {
            self.state.tokens.len()
        }
    }
    pub(super) fn event_group_limit(&self) -> usize {
        if self.staples_enabled() {
            staples::EVENT_GROUP_BOUND
        } else {
            EVENT_GROUP_BOUND
        }
    }
    fn check_token_capacity(&mut self, requested: u32) -> Result<(), String> {
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
        Ok(())
    }
    pub(super) fn execute_token_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        if !self.tokens_enabled() {
            return Err("unsupported token/counter profile".into());
        }
        let (event, block) = match instruction {
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
                        *frame
                            .spell
                            .targets
                            .get(target_slot as usize)
                            .ok_or("missing counter target")?
                    }
                };
                if !self.target_legal(target) {
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
        if !self.tokens_enabled() {
            return Err("unsupported token/counter event profile".into());
        }
        match event {
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
                let source_definition = self.definition(source.card)?.clone();
                let mut objects = vec![];
                for _ in 0..*count {
                    let offset = u32::try_from(self.state.tokens.len())
                        .map_err(|_| "token identity overflow")?;
                    let card = CardId(
                        TOKEN_ID_START
                            .checked_add(offset)
                            .ok_or("token identity overflow")?,
                    );
                    if self.state.cards.contains_key(&card) {
                        return Err("token/card identity collision".into());
                    }
                    let definition = token_definition(&source_definition, card, token);
                    let id = ObjectId {
                        card,
                        incarnation: 0,
                    };
                    self.state.cards.insert(
                        card,
                        Card {
                            owner: *controller,
                            oracle_id: definition.oracle_id.clone(),
                        },
                    );
                    self.state.tokens.insert(
                        card,
                        TokenIdentity {
                            source: *source,
                            spec: token.as_ref().clone(),
                            definition,
                        },
                    );
                    self.state.objects.insert(
                        card,
                        Object {
                            plus_one_counters: Quantity::zero(),
                            marked_damage: Quantity::zero(),
                            deathtouch_damage: false,
                            attack_ready: false,
                            id,
                            controller: *controller,
                            default_controller: *controller,
                            zone: Zone::Battlefield,
                            tapped: false,
                        },
                    );
                    objects.push(id);
                }
                Ok(EventOutcome::TokensCreated {
                    objects,
                    controller: *controller,
                    token: token.clone(),
                })
            }
            ProposedEvent::PutPlusOneCounters { object, amount } => {
                if !self.target_legal(*object) || *amount <= Quantity::zero() {
                    return Err("invalid counter placement".into());
                }
                let o = self
                    .state
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing counter recipient")?;
                let previous = o.plus_one_counters.clone();
                o.plus_one_counters.add(amount);
                Ok(EventOutcome::PlusOneCountersPlaced {
                    object: *object,
                    amount: amount.clone(),
                    previous,
                    current: o.plus_one_counters.clone(),
                })
            }
            ProposedEvent::CeaseToken { object } => {
                if !self.state.tokens.contains_key(&object.card)
                    || self.current(*object)?.zone == Zone::Battlefield
                {
                    return Err("invalid token cessation".into());
                }
                self.retain_source_lki(*object)?;
                self.state.objects.remove(&object.card);
                for library in self.state.libraries.values_mut() {
                    library.retain(|c| *c != object.card);
                }
                Ok(EventOutcome::TokenCeased { object: *object })
            }
            _ => Err("unexpected token event".into()),
        }
    }
    pub(super) fn validate_tokens_state(&self) -> Result<(), String> {
        if (!self.tokens_enabled()
            && (!self.state.tokens.is_empty() || self.state.typed_fault.is_some()))
            || self.allocated_capacity_count() > self.token_capacity() as usize
        {
            return Err("snapshot token profile/capacity invariant".into());
        }
        for (i, (id, t)) in self.state.tokens.iter().enumerate() {
            t.spec.validate()?;
            if Some(id.0)
                != u32::try_from(i)
                    .ok()
                    .and_then(|i| TOKEN_ID_START.checked_add(i))
                || self.prepared.config.cards.iter().any(|c| c.id == *id)
                || self.state.commanders.history(*id).is_some()
            {
                return Err("snapshot token physical identity invariant".into());
            }
            let source = self.definition(t.source.card)?;
            fn creates(p: &SpellProgram, spec: &CreatureToken) -> bool {
                p.instructions.iter().any(|i| matches!(i, Instruction::CreateCreatureTokens { token, .. } if token == spec)) || p.modes.iter().any(|m| creates(&m.program,spec))
            }
            if !source.spell.as_ref().is_some_and(|p| creates(p, &t.spec))
                && !source
                    .triggered
                    .iter()
                    .any(|a| creates(&a.program, &t.spec))
            {
                return Err("snapshot token differs from creating instruction".into());
            }
            if t.definition != token_definition(source, *id, &t.spec)
                || self
                    .state
                    .cards
                    .get(id)
                    .is_none_or(|c| c.owner.0 >= 4 || c.oracle_id != t.definition.oracle_id)
            {
                return Err("snapshot token characteristic identity invariant".into());
            }
        }
        for (id, o) in &self.state.objects {
            if o.plus_one_counters.is_negative()
                || (o.plus_one_counters > Quantity::zero()
                    && (!self.tokens_enabled()
                        || o.zone != Zone::Battlefield
                        || self.definition(*id)?.characteristics.kind != CardKind::Creature))
            {
                return Err("snapshot counter invariant".into());
            }
            if self.state.tokens.contains_key(id)
                && (o.id.card != *id
                    || o.controller.0 >= 4
                    || o.default_controller.0 >= 4
                    || !self.is_live(o.controller)
                    || !self.is_live(self.state.cards[id].owner)
                    || o.id.incarnation > 1
                    || (o.zone == Zone::Battlefield && o.id.incarnation != 0)
                    || (o.zone != Zone::Battlefield && o.id.incarnation != 1)
                    || (o.zone != Zone::Battlefield
                        && (o.tapped || o.attack_ready || o.deathtouch_damage)))
            {
                return Err("snapshot token object invariant".into());
            }
        }
        if let Some(fault) = self.typed_fault() {
            let expected = match fault {
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
            if !matches!(&self.state.frame,Frame::Fault {reason} if reason==expected) {
                return Err("snapshot typed fault frame invariant".into());
            }
        }
        Ok(())
    }
}

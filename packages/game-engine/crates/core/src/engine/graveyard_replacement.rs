//! Printed replacement before a physical object would enter a graveyard.
//! CR 113.6, 400.7, 614.4–6, 616.1f, 701.24c/f/g, 903.9b (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraveyardReplacement {
    pub(super) frame: Resolving,
    completion: GraveyardCompletion,
    source_block: u32,
    proposed: ProposedEvent,
    owner: PlayerId,
    revealed: Vec<ObjectId>,
    commanders: Vec<ObjectId>,
    choices: Vec<bool>,
    phase: ReplacementPhase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "completion", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum GraveyardCompletion {
    Library,
    Counter {
        target: StackId,
        source: ObjectId,
        controller: PlayerId,
        mill: Option<Quantity>,
    },
    MillAfterCounter {
        outcome: Box<InstructionOutcome>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
enum ReplacementPhase {
    BeforeMovement,
    Shuffling {
        actual: Option<Box<EventOutcome>>,
        remaining: usize,
        index: u32,
    },
}

fn library_members(proposed: &ProposedEvent) -> Option<(PlayerId, &[ObjectId])> {
    match proposed {
        ProposedEvent::Mill { player, cards, .. } => Some((*player, cards)),
        ProposedEvent::Surveil {
            player, graveyard, ..
        } => Some((*player, graveyard)),
        ProposedEvent::LibrarySelect {
            player,
            destination: LibraryRemainder::Graveyard,
            remainder,
            ..
        } => Some((*player, remainder)),
        _ => None,
    }
}

impl GraveyardReplacement {
    fn counter_target_left(&self, game: &Game) -> bool {
        matches!(&self.completion, GraveyardCompletion::Counter { target, source, .. }
            if !game.state.rules.stack.iter().any(|entry| entry.id == *target && entry.source == *source))
    }

    fn request(&self, game: &Game) -> Option<Request> {
        if !matches!(self.phase, ReplacementPhase::BeforeMovement)
            || !game.is_live(self.owner)
            || self.counter_target_left(game)
        {
            return None;
        }
        let object = *self.commanders.get(self.choices.len())?;
        Some(Request::CommanderReturn {
            decision_id: game.state.rules.revision,
            player: self.owner,
            object,
            destination: Zone::Library { owner: self.owner },
            previous_choices: self.prior_choices(),
        })
    }

    pub(super) fn prior_choices(&self) -> Vec<(ObjectId, bool)> {
        self.commanders
            .iter()
            .copied()
            .zip(self.choices.iter().copied())
            .collect()
    }

    pub(super) fn reserved_top(&self) -> &[ObjectId] {
        match &self.proposed {
            ProposedEvent::Surveil { top, .. } => top,
            _ => &[],
        }
    }

    fn last_index(&self, game: &Game) -> Result<u32, String> {
        let len = game
            .state
            .rules
            .libraries
            .get(&self.owner)
            .map_or(0, Vec::len);
        let available = len
            .checked_sub(self.reserved_top().len())
            .ok_or("replacement shuffle reserved prefix")?;
        u32::try_from(available.saturating_sub(1))
            .map_err(|_| "replacement shuffle library size".into())
    }
}

impl Game {
    pub(super) fn has_self_graveyard_replacement(&self, object: ObjectId) -> Result<bool, String> {
        self.current(object)?;
        Ok(self
            .copiable_definition(object)?
            .static_abilities
            .iter()
            .any(|ability| matches!(ability.effect, StaticEffect::SelfGraveyardRevealShuffle)))
    }

    pub(super) fn complete_library_effect(
        &mut self,
        frame: Resolving,
        proposed: ProposedEvent,
        source_block: u32,
    ) -> Result<(), String> {
        self.complete_library_effect_with(
            frame,
            proposed,
            source_block,
            GraveyardCompletion::Library,
        )
    }

    pub(super) fn complete_library_effect_with(
        &mut self,
        frame: Resolving,
        proposed: ProposedEvent,
        source_block: u32,
        completion: GraveyardCompletion,
    ) -> Result<(), String> {
        if let Some((owner, members)) = library_members(&proposed) {
            let mut revealed = vec![];
            for &object in members {
                if self.has_self_graveyard_replacement(object)? {
                    if self.zone(object) != Some(Zone::Library { owner })
                        || self.state.rules.cards[&object.card].owner != owner
                    {
                        return Err("replacement source is outside the proposed library".into());
                    }
                    revealed.push(object);
                }
            }
            if !revealed.is_empty() {
                return self.begin_graveyard_effect(
                    frame,
                    proposed,
                    source_block,
                    owner,
                    revealed,
                    completion,
                );
            }
        }
        let actual = self.commit(proposed, self.instruction_cause(&frame, source_block)?)?;
        self.finish_graveyard_effect(frame, source_block, Some(actual), completion)
    }

    pub(super) fn begin_graveyard_effect(
        &mut self,
        frame: Resolving,
        proposed: ProposedEvent,
        source_block: u32,
        owner: PlayerId,
        revealed: Vec<ObjectId>,
        completion: GraveyardCompletion,
    ) -> Result<(), String> {
        if self.state.rules.match_state.is_none() {
            return Err("printed graveyard shuffle requires a seeded match".into());
        }
        self.record_event(Event::GraveyardReplacementRevealed {
            cause: self.instruction_cause(&frame, source_block)?,
            objects: revealed.clone(),
        })?;
        let commanders = revealed
            .iter()
            .copied()
            .filter(|id| self.state.rules.commanders.history(id.card).is_some())
            .collect();
        self.state.rules.frame = Frame::GraveyardReplacement {
            transaction: Box::new(GraveyardReplacement {
                frame,
                proposed,
                source_block,
                owner,
                revealed,
                commanders,
                choices: vec![],
                completion,
                phase: ReplacementPhase::BeforeMovement,
            }),
        };
        Ok(())
    }

    fn finish_graveyard_effect(
        &mut self,
        frame: Resolving,
        source_block: u32,
        actual: Option<EventOutcome>,
        completion: GraveyardCompletion,
    ) -> Result<(), String> {
        match completion {
            GraveyardCompletion::Library => {
                self.finish_instruction(frame, InstructionOutcome::Effect { actual })
            }
            GraveyardCompletion::MillAfterCounter { outcome } => {
                self.finish_instruction(frame, *outcome)
            }
            GraveyardCompletion::Counter {
                target,
                controller,
                mill,
                ..
            } => {
                self.state.rules.frame = Frame::Resolve {
                    frame: frame.clone(),
                };
                self.complete_counter_outcome(frame, source_block, target, controller, mill, actual)
            }
        }
    }

    pub(super) fn replacement_request(&self) -> Option<Request> {
        match &self.state.rules.frame {
            Frame::GraveyardGroup { .. } => self.group_replacement_request(),
            Frame::GraveyardReplacement { transaction } => transaction.request(self),
            _ => None,
        }
    }

    pub(crate) fn replacement_reveals(&self, object: ObjectId) -> bool {
        matches!(&self.state.rules.frame, Frame::GraveyardGroup { tx } if tx.reveals(object))
            || matches!(&self.state.rules.frame, Frame::GraveyardReplacement { transaction }
            if matches!(transaction.phase, ReplacementPhase::BeforeMovement)
            && transaction.revealed.contains(&object))
    }

    // Only the matching complete group and cause obtain this ephemeral proof.
    pub(super) fn prepare_graveyard_batch(
        &self,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<Option<BTreeMap<ObjectId, Zone>>, String> {
        if let Frame::GraveyardGroup { tx } = &self.state.rules.frame {
            return tx.authorize(self, cause, group);
        }
        let Frame::GraveyardReplacement { transaction: tx } = &self.state.rules.frame else {
            return Ok(None);
        };
        if *cause != self.instruction_cause(&tx.frame, tx.source_block)? {
            return Ok(None);
        }
        let EventGroup::Occurrence { proposed } = group else {
            return Err("replacement batch grouping mismatch".into());
        };
        match &tx.phase {
            ReplacementPhase::BeforeMovement
                if *proposed == tx.proposed && tx.commanders.len() == tx.choices.len() =>
            {
                self.validate_graveyard_replacement()?;
                let mut destinations = tx
                    .revealed
                    .iter()
                    .map(|id| (*id, Zone::Library { owner: tx.owner }))
                    .collect::<BTreeMap<_, _>>();
                for (id, command) in tx.commanders.iter().zip(&tx.choices) {
                    if *command {
                        destinations.insert(*id, Zone::Command { owner: tx.owner });
                    }
                }
                Ok(Some(destinations))
            }
            ReplacementPhase::Shuffling { index: 0, .. } if matches!(proposed, ProposedEvent::FinishLibraryShuffle { player } if *player == tx.owner) => {
                Ok(Some(BTreeMap::new()))
            }
            ReplacementPhase::Shuffling { index, .. } if matches!(proposed, ProposedEvent::ShuffleLibraryWord { player, index: i } if *player == tx.owner && *i == *index && *index > 0) => {
                Ok(Some(BTreeMap::new()))
            }
            _ => Err("graveyard replacement cause, group or phase mismatch".into()),
        }
    }

    pub(super) fn replacement_shuffle_offset(&self, player: PlayerId, index: u32) -> Option<usize> {
        let Frame::GraveyardReplacement { transaction } = &self.state.rules.frame else {
            return None;
        };
        matches!(transaction.phase, ReplacementPhase::Shuffling { index: i, .. } if i == index && transaction.owner == player)
            .then(|| transaction.reserved_top().len())
    }

    pub(super) fn graveyard_replacement_step(
        &mut self,
        mut tx: Box<GraveyardReplacement>,
    ) -> Result<(), String> {
        if !self.is_live(tx.owner) {
            self.state.rules.pending = None;
            return match tx.phase {
                ReplacementPhase::Shuffling { actual, .. } => self.finish_graveyard_effect(
                    tx.frame,
                    tx.source_block,
                    actual.map(|event| *event),
                    tx.completion,
                ),
                ReplacementPhase::BeforeMovement => {
                    let outcome = match tx.completion {
                        completion @ GraveyardCompletion::Counter { .. } => {
                            return self.finish_graveyard_effect(
                                tx.frame,
                                tx.source_block,
                                None,
                                completion,
                            );
                        }
                        GraveyardCompletion::MillAfterCounter { outcome } => *outcome,
                        GraveyardCompletion::Library => {
                            InstructionOutcome::PlayerActionSkipped { player: tx.owner }
                        }
                    };
                    self.finish_instruction(tx.frame, outcome)
                }
            };
        }
        let cause = self.instruction_cause(&tx.frame, tx.source_block)?;
        match &tx.phase {
            ReplacementPhase::BeforeMovement if tx.counter_target_left(self) => {
                self.state.rules.pending = None;
                tx.phase = ReplacementPhase::Shuffling {
                    actual: None,
                    remaining: tx.revealed.len(),
                    index: tx.last_index(self)?,
                };
            }
            ReplacementPhase::BeforeMovement if tx.choices.len() < tx.commanders.len() => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing printed commander replacement choice")?;
                let Response::CommanderReturn { to_command } = input.response else {
                    return Err("printed commander replacement response".into());
                };
                if input.player != tx.owner {
                    return Err("commander replacement belongs to its owner".into());
                }
                tx.choices.push(to_command);
            }
            ReplacementPhase::BeforeMovement => {
                let actual = self.commit(tx.proposed.clone(), cause)?;
                tx.phase = ReplacementPhase::Shuffling {
                    actual: Some(Box::new(actual)),
                    remaining: tx.revealed.len(),
                    index: tx.last_index(self)?,
                };
            }
            ReplacementPhase::Shuffling { index: 0, .. } => {
                self.commit(
                    ProposedEvent::FinishLibraryShuffle { player: tx.owner },
                    cause,
                )?;
                let next_index = tx.last_index(self)?;
                let ReplacementPhase::Shuffling {
                    actual,
                    remaining,
                    index,
                } = &mut tx.phase
                else {
                    unreachable!()
                };
                if *remaining == 1 {
                    return self.finish_graveyard_effect(
                        tx.frame,
                        tx.source_block,
                        actual.clone().map(|event| *event),
                        tx.completion,
                    );
                }
                *remaining = remaining
                    .checked_sub(1)
                    .ok_or("missing replacement shuffle")?;
                *index = next_index;
            }
            ReplacementPhase::Shuffling { index, .. } => {
                let actual = self.commit(
                    ProposedEvent::ShuffleLibraryWord {
                        player: tx.owner,
                        index: *index,
                    },
                    cause,
                )?;
                let EventOutcome::LibraryShuffledWord { selected, .. } = actual else {
                    return Err("replacement shuffle word outcome".into());
                };
                if selected.is_some()
                    && let ReplacementPhase::Shuffling { index, .. } = &mut tx.phase
                {
                    *index -= 1;
                }
            }
        }
        self.state.rules.frame = Frame::GraveyardReplacement { transaction: tx };
        Ok(())
    }

    pub(super) fn replacement_charge(&self) -> usize {
        let frame = spell_cascade_grants::retained_effect_frame(&self.state.rules.frame);
        if matches!(
            frame,
            Frame::GraveyardReplacement { .. } | Frame::GraveyardGroup { .. }
        ) {
            stored_history::payload_charge(frame)
        } else {
            0
        }
    }

    pub(super) fn validate_graveyard_replacement(&self) -> Result<(), String> {
        if let Frame::GraveyardGroup { tx } =
            spell_cascade_grants::retained_effect_frame(&self.state.rules.frame)
        {
            return tx.validate(self);
        }
        let Frame::GraveyardReplacement { transaction: tx } =
            spell_cascade_grants::retained_effect_frame(&self.state.rules.frame)
        else {
            return Ok(());
        };
        self.validate_resolving(&tx.frame)?;
        let instruction = self
            .program_for(self.entry(tx.frame.stack_id)?)?
            .instructions
            .get(tx.frame.cursor as usize)
            .ok_or("missing replacement-producing instruction")?;
        let (owner, members, from) = match (&tx.completion, &tx.proposed) {
            (
                GraveyardCompletion::Counter {
                    target,
                    source,
                    controller,
                    mill,
                },
                ProposedEvent::CounterStackObject {
                    target: proposed,
                    countering,
                    domain,
                },
            ) if *target == *proposed
                && *countering == tx.frame.stack_id
                && *domain == TargetDomain::StackSpell =>
            {
                let (slot, count) = match instruction {
                    Instruction::CounterSpellAndMillController {
                        target_slot, count, ..
                    } => (*target_slot, Some(count)),
                    Instruction::CounterStackObject { target_slot, .. }
                    | Instruction::CounterSpellUnlessPays { target_slot, .. } => {
                        (*target_slot, None)
                    }
                    _ => return Err("replacement counter source".into()),
                };
                if mill.as_ref() != count
                    || self
                        .entry(tx.frame.stack_id)?
                        .stack_targets
                        .get(slot as usize)
                        != Some(target)
                    || tx.frame.legal_targets.get(slot as usize) != Some(&true)
                {
                    return Err("counter replacement announcement invariant".into());
                }
                if matches!(tx.phase, ReplacementPhase::BeforeMovement)
                    && self.is_live(tx.owner)
                    && !tx.counter_target_left(self)
                {
                    let entry = self
                        .state
                        .rules
                        .stack
                        .iter()
                        .find(|s| s.id == *target)
                        .ok_or("missing replaced counter target")?;
                    if !entry.kind.is_spell()
                        || entry.source != *source
                        || entry.controller != *controller
                        || self.spell_cannot_be_countered(entry)?
                    {
                        return Err("counter replacement target invariant".into());
                    }
                }
                (
                    self.state
                        .rules
                        .cards
                        .get(&source.card)
                        .ok_or("missing countered card")?
                        .owner,
                    std::slice::from_ref(source),
                    Zone::Stack,
                )
            }
            _ => {
                let (owner, members) = library_members(&tx.proposed)
                    .ok_or("unsupported retained graveyard proposal")?;
                (owner, members, Zone::Library { owner })
            }
        };
        let expected = members
            .iter()
            .copied()
            .filter(|id| {
                self.definition(id.card).is_ok_and(|d| {
                    d.static_abilities
                        .iter()
                        .any(|a| matches!(a.effect, StaticEffect::SelfGraveyardRevealShuffle))
                })
            })
            .collect::<Vec<_>>();
        if self.state.rules.match_state.is_none()
            || owner != tx.owner
            || tx.source_block != instruction.source_block()
            || tx.revealed.is_empty()
            || expected != tx.revealed
            || tx.choices.len() > tx.commanders.len()
            || tx.commanders
                != expected
                    .iter()
                    .copied()
                    .filter(|id| self.state.rules.commanders.history(id.card).is_some())
                    .collect::<Vec<_>>()
            || !matches!(
                (instruction, &tx.proposed),
                (
                    Instruction::CounterSpellAndMillController { .. }
                        | Instruction::CounterSpellUnlessPays { .. }
                        | Instruction::CounterStackObject { .. },
                    ProposedEvent::CounterStackObject { .. }
                ) | (
                    Instruction::CounterSpellAndMillController { .. },
                    ProposedEvent::Mill { .. }
                ) | (
                    Instruction::MillCards { .. } | Instruction::MillAnnouncedX { .. },
                    ProposedEvent::Mill { .. }
                ) | (Instruction::Surveil { .. }, ProposedEvent::Surveil { .. })
                    | (
                        Instruction::SelectLibraryCard { .. },
                        ProposedEvent::LibrarySelect { .. }
                    )
            )
        {
            return Err("snapshot graveyard replacement producer invariant".into());
        }
        if self.is_live(owner) {
            match &tx.phase {
                ReplacementPhase::BeforeMovement
                    if !tx.counter_target_left(self)
                        && members.iter().any(|id| self.zone(*id) != Some(from)) =>
                {
                    return Err("stale replacement proposal".into());
                }
                ReplacementPhase::Shuffling {
                    remaining,
                    index,
                    actual,
                } if (actual.is_some() && tx.choices.len() != tx.commanders.len())
                    || (actual.is_none() && !tx.counter_target_left(self))
                    || *remaining == 0
                    || *remaining > expected.len()
                    || *index > tx.last_index(self)? =>
                {
                    return Err("invalid replacement shuffle cursor".into());
                }
                _ => {}
            }
        }
        Ok(())
    }
}

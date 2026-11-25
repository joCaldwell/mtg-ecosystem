//! One-shot triggered abilities owned by a resolving invocation.
//! CR 400.7e/j, 513.2, 603.6c, 603.7a–e, 603.12, 800.4a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::tokens::TokenTemplate;
use super::*;
use mtg_contracts::kir::{DelayedEvent, Node};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CreatorProgram {
    Spell { mode: Option<u8> },
    Activated { index: u32 },
    Triggered { index: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    target: ObjectId,
    actual: MoveOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Firing {
    trigger: TriggerId,
    occurrence: TriggerOccurrenceId,
    witness: TriggerEventWitness,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionTrigger {
    source: ObjectId,
    controller: PlayerId,
    creator_program: CreatorProgram,
    cursor: u32,
    first_batch: u64,
    turn: u64,
    step: TurnStep,
    source_facts: ObjectBefore,
    capture: Option<Capture>,
    watched: Option<ObjectId>,
    firing: Option<Firing>,
    retired: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepStamp {
    batch: u64,
    turn: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionTriggerClock {
    end: Option<StepStamp>,
    upkeep: [Option<StepStamp>; 4],
}

impl CreatorProgram {
    fn of(entry: &StackInstance) -> Result<Self, String> {
        match entry.kind {
            StackKind::Spell => Ok(Self::Spell {
                mode: entry.mode_mask,
            }),
            StackKind::Activated { ability, .. } => Ok(Self::Activated { index: ability }),
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Intrinsic { index },
                ..
            } => Ok(Self::Triggered { index }),
            StackKind::Triggered { .. } => Err("a resolution trigger cannot create another".into()),
        }
    }

    fn program(self, definition: &Definition) -> Option<&SpellProgram> {
        match self {
            Self::Spell { mode: None } => definition.spell.as_ref(),
            Self::Spell { mode: Some(mask) } => {
                definition.modal_spell.as_ref()?.selections.get(&mask)
            }
            Self::Activated { index } => {
                definition.activated.get(index as usize).map(|a| &a.program)
            }
            Self::Triggered { index } => {
                definition.triggered.get(index as usize).map(|a| &a.program)
            }
        }
    }
}

impl ResolutionTrigger {
    fn due(&self, event: &DelayedEvent, step: TurnStep, owner: PlayerId, turn: u64) -> bool {
        match event {
            DelayedEvent::NextEndStep | DelayedEvent::NextEndStepAfterExile { .. } => {
                let offset = u64::from(matches!(self.step, TurnStep::End | TurnStep::Cleanup));
                step == TurnStep::End && self.turn.checked_add(offset) == Some(turn)
            }
            DelayedEvent::NextTurnUpkeep => {
                step == TurnStep::Upkeep && self.turn.checked_add(1) == Some(turn)
            }
            DelayedEvent::NextUpkeep { .. } => {
                step == TurnStep::Upkeep && turn > self.turn && owner == self.controller
            }
            _ => false,
        }
    }

    pub(super) fn refresh_source(&mut self, facts: &ObjectBefore) {
        if self.source == facts.object && self.firing.is_none() && !self.retired {
            self.source_facts = facts.clone();
        }
    }
}

impl RulesTxn {
    fn resolution_trigger_node(&self, row: &ResolutionTrigger) -> Result<&Node, String> {
        let definition = self.copiable_definition(row.source)?;
        let program = row
            .creator_program
            .program(definition)
            .ok_or("missing resolution trigger creator program")?;
        let [Instruction::Kernel { node, .. }] = program
            .instructions
            .get(row.cursor as usize..)
            .ok_or("invalid resolution trigger creator cursor")?
        else {
            return Err("resolution trigger creator must finish its program".into());
        };
        Ok(node)
    }

    pub(super) fn resolution_trigger_program(
        &self,
        creator: StackId,
    ) -> Result<&SpellProgram, String> {
        let row = self
            .state
            .rules
            .resolution_triggers
            .get(&creator)
            .ok_or("missing resolution trigger creator")?;
        match self.resolution_trigger_node(row)? {
            Node::Delayed { program, .. } | Node::Reflexive { program, .. } => Ok(program),
            _ => Err("invalid resolution trigger creator node".into()),
        }
    }

    fn register_resolution_trigger(
        &mut self,
        frame: &Resolving,
        capture: Option<Capture>,
        watched: Option<ObjectId>,
    ) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        if !self.is_live(entry.controller) {
            return Ok(());
        }
        // CR 608.2h, 702.26b: use the creating invocation's last-known
        // appearance when its source has left or phased out.
        let source_facts = self.source_facts(entry)?;
        let row = ResolutionTrigger {
            source: entry.source,
            controller: entry.controller,
            creator_program: CreatorProgram::of(entry)?,
            cursor: frame.cursor,
            first_batch: self.state.rules.next_batch_id,
            turn: self.state.rules.turn.number,
            step: self.state.rules.turn.step,
            source_facts,
            capture,
            watched,
            firing: None,
            retired: false,
        };
        if self
            .rules_mut()
            .resolution_triggers
            .insert(frame.stack_id, row)
            .is_some()
        {
            return Err("duplicate resolution trigger creator".into());
        }
        self.check_history_capacity()?;
        Ok(())
    }

    pub(super) fn execute_delayed_trigger(
        &mut self,
        frame: Resolving,
        event: &DelayedEvent,
        block: u32,
    ) -> Result<(), String> {
        let mut capture = None;
        let mut watched = None;
        match *event {
            DelayedEvent::NextEndStepAfterExile { slot } => {
                if let Some(target) = self.resolving_creature(&frame, slot)? {
                    let actual = self.move_object_for(
                        target,
                        Zone::Exile,
                        self.instruction_cause(&frame, block)?,
                    )?;
                    capture = Some(Capture { target, actual });
                } else {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
            }
            DelayedEvent::TargetLeavesAfterExilingSource { slot } => {
                watched = self.resolving_creature(&frame, slot)?;
                let StackKind::Triggered {
                    witness: TriggerEventWitness::Moved { movement },
                    ..
                } = &self.entry(frame.stack_id)?.kind
                else {
                    return Err("delayed departure watcher requires a dies trigger".into());
                };
                let target = movement.successor;
                if self
                    .current(target)
                    .is_ok_and(|o| matches!(o.zone, Zone::Graveyard { .. }))
                    && !self.state.rules.tokens.contains_key(&target.card)
                {
                    let actual = self.move_object_for(
                        target,
                        Zone::Exile,
                        self.instruction_cause(&frame, block)?,
                    )?;
                    capture = Some(Capture { target, actual });
                }
                if watched.is_none() {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::Effect {
                            actual: capture.map(|capture| EventOutcome::Moved {
                                movement: capture.actual,
                            }),
                        },
                    );
                }
            }
            _ => {}
        }
        let actual = capture.as_ref().map(|capture| EventOutcome::Moved {
            movement: capture.actual.clone(),
        });
        self.register_resolution_trigger(&frame, capture, watched)?;
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }

    fn resolving_creature(&self, frame: &Resolving, slot: u32) -> Result<Option<ObjectId>, String> {
        Ok(self
            .entry(frame.stack_id)?
            .targets
            .get(slot as usize)
            .copied()
            .filter(|&object| {
                frame.legal_targets.get(slot as usize) == Some(&true)
                    && self
                        .current(object)
                        .is_ok_and(|object| object.present_on_battlefield())
            }))
    }

    pub(super) fn collect_resolution_departure(
        &mut self,
        event: &OccurrenceResult,
        before: &Self,
    ) -> Result<(), String> {
        let (object, witness) = match &event.actual {
            EventOutcome::LeftGame { object, owner }
                if before
                    .current(*object)
                    .is_ok_and(|object| object.present_on_battlefield()) =>
            {
                (
                    *object,
                    TriggerEventWitness::PermanentLeftGame {
                        object: *object,
                        owner: *owner,
                    },
                )
            }
            actual => {
                let Some(movement) = triggers::movement(actual) else {
                    return Ok(());
                };
                if movement.from != Zone::Battlefield || movement.destination == Zone::Battlefield {
                    return Ok(());
                }
                (
                    movement.previous,
                    TriggerEventWitness::Moved {
                        movement: movement.clone(),
                    },
                )
            }
        };
        let ready: Vec<_> = self
            .state
            .rules
            .resolution_triggers
            .iter()
            .filter_map(|(&creator, row)| {
                (row.watched == Some(object)
                    && !row.retired
                    && row.firing.is_none()
                    && self.is_live(row.controller))
                .then_some(creator)
            })
            .collect();
        if ready.is_empty() {
            return Ok(());
        }
        self.preflight_trigger_emission(ready.len())?;
        for creator in ready {
            self.fire_resolution_trigger(
                creator,
                TriggerOccurrenceId::Batch(event.id.clone()),
                witness.clone(),
            )?;
        }
        Ok(())
    }

    // CR 733.1: reversing a paid mana ability cancels any trigger from an
    // undone sacrifice and restores its still-waiting incarnation.
    pub(super) fn undo_resolution_departure(&mut self, triggers: &[TriggerId]) {
        if !self.state.rules.resolution_triggers.values().any(|row| {
            row.watched.is_some()
                && row
                    .firing
                    .as_ref()
                    .is_some_and(|fire| triggers.contains(&fire.trigger))
        }) {
            return;
        }
        for row in self.rules_mut().resolution_triggers.values_mut() {
            if row.watched.is_some()
                && row
                    .firing
                    .as_ref()
                    .is_some_and(|fire| triggers.contains(&fire.trigger))
            {
                row.firing = None;
            }
        }
    }

    pub(super) fn collect_reflexive_payment(
        &mut self,
        cost: &ManaCost,
        batch: &CommittedBatch,
    ) -> Result<(), String> {
        let frame = self.kernel_root()?.clone();
        let controller = self.entry(frame.stack_id)?.controller;
        let [occurrence] = batch.occurrences.as_slice() else {
            return Err("reflexive payment must have one occurrence".into());
        };
        if !matches!(&occurrence.actual, EventOutcome::ManaPaidSelected { player, cost: actual, .. }
            if *player == controller && actual == cost)
        {
            return Err("reflexive trigger requires actual payment".into());
        }
        self.register_resolution_trigger(&frame, None, None)?;
        self.fire_resolution_trigger(
            frame.stack_id,
            TriggerOccurrenceId::Batch(occurrence.id.clone()),
            TriggerEventWitness::ResolutionPaid {
                player: controller,
                cost: cost.clone(),
            },
        )
    }

    pub(super) fn collect_resolution_step(
        &mut self,
        event: &OccurrenceId,
        witness: &TriggerEventWitness,
    ) -> Result<(), String> {
        let TriggerEventWitness::StepBegan { turn, owner, .. } = witness else {
            return Ok(());
        };
        let stamp = Some(StepStamp {
            batch: event.batch,
            turn: turn.number,
        });
        match turn.step {
            TurnStep::End => self.rules_mut().resolution_trigger_clock.end = stamp,
            TurnStep::Upkeep => {
                self.rules_mut().resolution_trigger_clock.upkeep[owner.0 as usize] = stamp
            }
            _ => return Ok(()),
        }
        let mut ready = Vec::new();
        for (&creator, row) in self.state.rules.resolution_triggers.iter() {
            if row.retired || row.firing.is_some() || !self.is_live(row.controller) {
                continue;
            }
            if let Node::Delayed { event: pattern, .. } = self.resolution_trigger_node(row)?
                && row.due(pattern, turn.step, *owner, turn.number)
            {
                ready.push(creator);
            }
        }
        if ready.is_empty() {
            return Ok(());
        }
        self.preflight_trigger_emission(ready.len())?;
        for creator in ready {
            self.fire_resolution_trigger(
                creator,
                TriggerOccurrenceId::Batch(event.clone()),
                witness.clone(),
            )?;
        }
        Ok(())
    }

    fn fire_resolution_trigger(
        &mut self,
        creator: StackId,
        occurrence: TriggerOccurrenceId,
        witness: TriggerEventWitness,
    ) -> Result<(), String> {
        let row = self
            .state
            .rules
            .resolution_triggers
            .get(&creator)
            .ok_or("missing resolution trigger")?;
        if row.firing.is_some() || row.retired {
            return Err("resolution trigger already fired".into());
        }
        let (controller, source_facts) = (row.controller, row.source_facts.clone());
        let id = self.enqueue_trigger(
            EffectiveTriggerAbility::Followup { creator },
            controller,
            source_facts,
            None,
            None,
            (occurrence.clone(), witness.clone()),
        )?;
        self.rules_mut()
            .resolution_triggers
            .get_mut(&creator)
            .ok_or("missing resolution trigger")?
            .firing = Some(Firing {
            trigger: id,
            occurrence,
            witness,
        });
        self.check_history_capacity()?;
        Ok(())
    }

    pub(super) fn resolution_trigger_created_token(
        &self,
        source: ObjectId,
        needle: &TokenTemplate,
    ) -> Result<bool, String> {
        for (&creator, row) in self.state.rules.resolution_triggers.iter() {
            if row.source != source || row.firing.is_none() {
                continue;
            }
            let program = self.resolution_trigger_program(creator)?;
            if program
                .instructions
                .iter()
                .any(|instruction| match (instruction, needle) {
                    (
                        kernel!(CreateTokens {
                            token: mtg_contracts::kir::TokenSpec::Creature { spec },
                            ..
                        }),
                        TokenTemplate::Created(wanted),
                    ) => spec == wanted,
                    (
                        kernel!(CreateTokens {
                            token: mtg_contracts::kir::TokenSpec::Predefined { kind },
                            ..
                        }),
                        TokenTemplate::Predefined { kind: wanted },
                    ) => kind == wanted,
                    _ => false,
                })
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn execute_captured_return(
        &mut self,
        frame: Resolving,
        block: u32,
    ) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        let StackKind::Triggered {
            ability: EffectiveTriggerAbility::Followup { creator },
            ..
        } = entry.kind
        else {
            return Err("captured return has no delayed invocation".into());
        };
        let row = self
            .state
            .rules
            .resolution_triggers
            .get(&creator)
            .ok_or("missing captured return origin")?;
        let target = row.capture.as_ref().map(|capture| capture.actual.successor);
        let actual = if let Some(target) = target
            && self
                .current(target)
                .is_ok_and(|object| object.zone == Zone::Exile)
            && !self.state.rules.tokens.contains_key(&target.card)
        {
            let movement = self.move_object_for(
                target,
                Zone::Battlefield,
                self.instruction_cause(&frame, block)?,
            )?;
            Some(EventOutcome::Moved { movement })
        } else {
            None
        };
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }
}

impl RulesTxn {
    fn resolution_triggers_in_flight(&self) -> BTreeSet<StackId> {
        let creator = |ability| match ability {
            EffectiveTriggerAbility::Followup { creator } => Some(creator),
            EffectiveTriggerAbility::Intrinsic { .. } => None,
        };
        let rules = &self.state.rules;
        rules
            .pending_triggers
            .iter()
            .filter_map(|trigger| creator(trigger.ability))
            .chain(
                rules
                    .stack
                    .iter()
                    .chain(rules.detached.values())
                    .filter_map(|entry| match entry.kind {
                        StackKind::Triggered { ability, .. } => creator(ability),
                        _ => None,
                    }),
            )
            .collect()
    }

    pub(super) fn retire_resolution_triggers(&mut self) {
        if self.state.rules.resolution_triggers.is_empty() {
            return;
        }
        let in_flight = self.resolution_triggers_in_flight();
        let retired: Vec<_> = self
            .state
            .rules
            .resolution_triggers
            .iter()
            .filter_map(|(&id, row)| {
                (!row.retired
                    && (row.firing.is_some()
                        || !self.is_live(row.controller)
                        || self.watch_removed_with_owner(row))
                    && !in_flight.contains(&id))
                .then_some(id)
            })
            .collect();
        if !retired.is_empty() {
            let rows = &mut self.rules_mut().resolution_triggers;
            for id in retired {
                rows.get_mut(&id)
                    .expect("collected from the same table")
                    .retired = true;
            }
        }
    }

    // A phased-out permanent that leaves with its owner does not trigger a
    // leaves-the-battlefield ability (CR 603.6c). Its incarnation is gone.
    fn watch_removed_with_owner(&self, row: &ResolutionTrigger) -> bool {
        row.watched.is_some_and(|watched| {
            self.current(watched).is_err()
                && self
                    .state
                    .rules
                    .cards
                    .get(&watched.card)
                    .is_some_and(|card| !self.is_live(card.owner))
        })
    }

    pub(super) fn resolution_trigger_history_charge(&self) -> usize {
        self.state
            .rules
            .resolution_triggers
            .values()
            .fold(0usize, |total, row| {
                total.saturating_add(stored_history::payload_charge(row))
            })
    }

    pub(super) fn validate_resolution_trigger_invocation(
        &self,
        creator: StackId,
        source: ObjectId,
        controller: PlayerId,
        id: TriggerId,
        occurrence: &TriggerOccurrenceId,
        witness: &TriggerEventWitness,
    ) -> Result<(), String> {
        let row = self
            .state
            .rules
            .resolution_triggers
            .get(&creator)
            .ok_or("missing resolution trigger origin")?;
        if row.retired
            || row.source != source
            || row.controller != controller
            || !row.firing.as_ref().is_some_and(|f| {
                f.trigger == id && &f.occurrence == occurrence && &f.witness == witness
            })
        {
            return Err("resolution trigger firing provenance invariant".into());
        }
        Ok(())
    }

    pub(super) fn validate_resolution_triggers(&self) -> Result<(), String> {
        let rules = &self.state.rules;
        let clock = &rules.resolution_trigger_clock;
        let seated = |player| rules.original_seating.contains(&player);
        for stamp in clock.end.iter().chain(clock.upkeep.iter().flatten()) {
            if stamp.batch >= rules.next_batch_id || stamp.turn > rules.turn.number {
                return Err("resolution trigger step clock invariant".into());
            }
        }
        let in_flight = self.resolution_triggers_in_flight();
        let mut consumed_ids = BTreeSet::new();
        for (&creator, row) in &rules.resolution_triggers {
            if creator.0 >= rules.next_stack_id
                || row.first_batch > rules.next_batch_id
                || row.turn == 0
                || row.turn > rules.turn.number
                || !seated(row.controller)
                || row.source != row.source_facts.object
                || rules.objects.get(&row.source.card).is_some_and(|object| {
                    object.id.incarnation < row.source.incarnation
                        || (object.id == row.source && object.zone != row.source_facts.zone)
                })
                || rules
                    .cards
                    .get(&row.source.card)
                    .is_none_or(|card| card.owner != row.source_facts.owner)
                || !seated(row.source_facts.controller)
                || !seated(row.source_facts.default_controller)
                || (!row.retired && row.firing.is_none() && !self.is_live(row.controller))
                || (row.retired
                    && row.firing.is_none()
                    && self.is_live(row.controller)
                    && !self.watch_removed_with_owner(row))
                || in_flight.contains(&creator) != (row.firing.is_some() && !row.retired)
            {
                return Err("resolution trigger origin invariant".into());
            }
            self.validate_characteristic_identity(row.source, &row.source_facts.characteristics)?;
            if matches!(row.creator_program, CreatorProgram::Spell { .. })
                && (row.source_facts.zone != Zone::Stack
                    || row.source_facts.controller != row.controller)
            {
                return Err("resolution trigger spell controller invariant".into());
            }
            if let Ok(entry) = self.entry(creator)
                && (entry.source != row.source
                    || entry.controller != row.controller
                    || CreatorProgram::of(entry)? != row.creator_program
                    || entry.paid.is_none()
                    || !rules.frame.holds(&rules.frames, creator))
            {
                return Err("resolution trigger creator invocation invariant".into());
            }
            if let Some(firing) = &row.firing
                && (firing.trigger.0 >= rules.next_trigger_id
                    || !consumed_ids.insert(firing.trigger))
            {
                return Err("duplicate or invalid resolution trigger firing".into());
            }
            match self.resolution_trigger_node(row)? {
                Node::Delayed { event, .. } => {
                    let leaves =
                        if let DelayedEvent::TargetLeavesAfterExilingSource { slot } = event {
                            let watched = row.watched.ok_or("missing delayed departure subject")?;
                            self.validate_resolution_departure(creator, row, watched, *slot)?;
                            true
                        } else {
                            if row.watched.is_some() {
                                return Err("unexpected delayed departure subject".into());
                            }
                            false
                        };
                    if let Some(capture) = &row.capture {
                        let movement = &capture.actual;
                        let bound = match event {
                            DelayedEvent::NextEndStepAfterExile { slot } => {
                                movement.from == Zone::Battlefield
                                    && !self.entry(creator).is_ok_and(|e| {
                                        e.targets.get(*slot as usize) != Some(&capture.target)
                                    })
                            }
                            DelayedEvent::TargetLeavesAfterExilingSource { .. } => {
                                movement.from
                                    == (Zone::Graveyard {
                                        owner: row.source_facts.owner,
                                    })
                                    && capture.target.card == row.source.card
                                    && row.source.incarnation.checked_add(1)
                                        == Some(capture.target.incarnation)
                            }
                            _ => false,
                        };
                        if !bound
                            || movement.previous != capture.target
                            || movement.destination != Zone::Exile
                            || movement.successor.card != capture.target.card
                            || capture.target.incarnation.checked_add(1)
                                != Some(movement.successor.incarnation)
                            || !rules.cards.contains_key(&capture.target.card)
                            || self.current(movement.previous).is_ok()
                            || rules
                                .objects
                                .get(&capture.target.card)
                                .is_some_and(|object| {
                                    object.id.incarnation < movement.successor.incarnation
                                        || (object.id == movement.successor
                                            && object.zone != Zone::Exile)
                                })
                        {
                            return Err("resolution trigger captured incarnation invariant".into());
                        }
                    } else if matches!(event, DelayedEvent::NextEndStepAfterExile { .. }) {
                        return Err("resolution trigger lacks exile movement".into());
                    }
                    if leaves {
                        continue;
                    }
                    let after = |stamp: &StepStamp| stamp.batch >= row.first_batch;
                    let due = match event {
                        DelayedEvent::NextEndStep | DelayedEvent::NextEndStepAfterExile { .. } => {
                            clock.end.as_ref().is_some_and(after)
                        }
                        DelayedEvent::NextTurnUpkeep => clock
                            .upkeep
                            .iter()
                            .flatten()
                            .any(|stamp| after(stamp) && stamp.turn > row.turn),
                        DelayedEvent::NextUpkeep { .. } => clock.upkeep[row.controller.0 as usize]
                            .as_ref()
                            .is_some_and(after),
                        _ => return Err("unsupported delayed trigger event".into()),
                    };
                    if let Some(firing) = &row.firing {
                        let (
                            TriggerOccurrenceId::Batch(event_id),
                            TriggerEventWitness::StepBegan {
                                turn,
                                owner,
                                active,
                            },
                        ) = (&firing.occurrence, &firing.witness)
                        else {
                            return Err("invalid delayed trigger occurrence".into());
                        };
                        let last = match turn.step {
                            TurnStep::End => clock.end,
                            TurnStep::Upkeep if seated(*owner) => clock.upkeep[owner.0 as usize],
                            _ => None,
                        };
                        if !due
                            || !seated(*owner)
                            || active.is_some_and(|p| p != *owner)
                            || event_id.batch < row.first_batch
                            || turn.number < row.turn
                            || !row.due(event, turn.step, *owner, turn.number)
                            || last.is_none_or(|stamp| {
                                stamp.batch < event_id.batch
                                    || stamp.turn < turn.number
                                    || ((stamp.turn == turn.number)
                                        != (stamp.batch == event_id.batch))
                            })
                        {
                            return Err("delayed trigger step evidence invariant".into());
                        }
                    } else if due && !row.retired {
                        return Err("delayed trigger already passed its matching step".into());
                    }
                }
                Node::Reflexive { payment, .. } => {
                    if row.capture.is_some()
                        || row.watched.is_some()
                        || !matches!(&row.firing,
                        Some(Firing { occurrence: TriggerOccurrenceId::Batch(event), witness: TriggerEventWitness::ResolutionPaid { player, cost }, .. })
                        if event.path.is_empty() && event.batch.checked_add(1) == Some(row.first_batch)
                            && *player == row.controller && cost == payment)
                    {
                        return Err("reflexive trigger payment evidence invariant".into());
                    }
                }
                _ => return Err("invalid resolution trigger origin node".into()),
            }
        }
        Ok(())
    }

    fn validate_resolution_departure(
        &self,
        creator: StackId,
        row: &ResolutionTrigger,
        watched: ObjectId,
        slot: u32,
    ) -> Result<(), String> {
        let rules = &self.state.rules;
        if !rules.cards.contains_key(&watched.card)
            || self
                .entry(creator)
                .is_ok_and(|e| e.targets.get(slot as usize) != Some(&watched))
        {
            return Err("delayed departure subject invariant".into());
        }
        if let Some(fire) = &row.firing {
            let valid = match &fire.witness {
                TriggerEventWitness::Moved { movement } => {
                    movement.previous == watched
                        && movement.from == Zone::Battlefield
                        && movement.destination != Zone::Battlefield
                        && movement.previous.card == movement.successor.card
                        && watched.incarnation.checked_add(1)
                            == Some(movement.successor.incarnation)
                }
                TriggerEventWitness::PermanentLeftGame { object, owner } => {
                    *object == watched
                        && !self.is_live(*owner)
                        && rules.cards[&object.card].owner == *owner
                }
                _ => false,
            };
            if !valid
                || self.current(watched).is_ok()
                || !matches!(&fire.occurrence, TriggerOccurrenceId::Batch(id)
                    if id.batch >= row.first_batch && id.batch < rules.next_batch_id)
            {
                return Err("delayed departure firing evidence invariant".into());
            }
        } else if !row.retired
            && !self
                .current(watched)
                .is_ok_and(|object| object.zone == Zone::Battlefield)
        {
            return Err("delayed departure already occurred".into());
        }
        Ok(())
    }
}

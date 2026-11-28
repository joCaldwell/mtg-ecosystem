//! P6.10. Pinned rules-20260925.txt: CR 101.2, 104.4a, 107.3a/c,
//! 113.6g, 118.7a, 120.4, 202.3e, 601.2b/f,
//! 603.2c/10a, 608.2c/h, 611.2c, 614.1a/6, 701.8a, 903.9a/b, 514.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::ObjectSet;

/// What a board-wide movement does with each member (CR 701.8, 400.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetMove {
    Destroy,
    Exile,
    Hand,
}
// Set semantics: docs/game-engine-board-mechanics.md#movement-sacrifice-and-board-wide-effects.
// Shared resource bounds: docs/game-engine-capacity-policy.md.

impl RulesTxn {
    pub(crate) fn stack_mana_value(&self, entry: &StackInstance) -> Result<Quantity, String> {
        let mut q = self
            .stack_definition(entry)?
            .characteristics
            .printed_mana_value();
        if let Some(x) = &entry.x {
            q.add(x);
        }
        Ok(q)
    }
    pub(super) fn other_casts_mana_value(&self, spell: &StackInstance) -> Result<Quantity, String> {
        // CR 601.2i, 608.2i: this paid spell was cast in the current turn.
        // Cast history also includes spells cast above it, such as cascade casts.
        // This sorcery-only instruction counts the other casts by its caster.
        if !spell.kind.is_spell() || spell.paid.is_none() {
            return Err("cast mana value sum requires a paid spell".into());
        }
        let mut total = self
            .state
            .rules
            .cast_history
            .get(spell.controller.0 as usize)
            .ok_or("invalid cast history seat")?
            .mana_value
            .clone();
        total.subtract(&self.stack_mana_value(spell)?);
        if total.is_negative() {
            return Err("cast mana value sum is missing this spell".into());
        }
        Ok(total)
    }
    // Before X is bound, the shared legality check evaluates all the
    // printed criteria except its upper bound (CR 601.2b-c).
    pub(super) fn x_target_selection_minimum(&self) -> Result<Quantity, String> {
        let program = self.program()?;
        let Some(spec) = program
            .targets
            .first()
            .filter(|spec| !program.optional_target && spec.mana_value_at_most_announced_x())
        else {
            return Ok(Quantity::zero());
        };
        let entry = self.top()?;
        // With no qualifying card, target announcement rejects the cast.
        let mut least: Option<Quantity> = None;
        for object in self.state.rules.objects.values() {
            if self.object_target_legal_before_x(entry, object.id, spec)?
                && let Some(value) = self.graveyard_mana_value(object.id)
            {
                least = Some(least.map_or(value.clone(), |l| l.min(value)));
            }
        }
        Ok(least.unwrap_or_else(Quantity::zero))
    }

    // CR 601.2b-c: an exact X choice must leave that many legal objects.
    pub(super) fn x_target_selection_limit(&self) -> Result<Option<Quantity>, String> {
        let program = self.program()?;
        if program.variable_target_count()
            != Some(mtg_contracts::kir::VariableTargetCount::ExactlyX)
        {
            return Ok(None);
        }
        let entry = self.top()?;
        let spec = &program.targets[0];
        let mut eligible = 0usize;
        for object in self.state.rules.objects.values() {
            eligible += usize::from(self.object_target_legal_before_x(entry, object.id, spec)?);
        }
        Ok(Some(
            i64::try_from(eligible)
                .map_err(|_| "target choice limit")?
                .into(),
        ))
    }

    pub(super) fn bind_x(&mut self, cast: Casting) -> Result<(), String> {
        let input = self.rules_mut().pending.take().ok_or("missing X input")?;

        let Answer::Number { value } = input.answer.clone() else {
            return Err("X response invariant".into());
        };
        let top = self.rules_mut().stack.last_mut().ok_or("missing X spell")?;
        if top.x.is_some() {
            return Err("X already announced".into());
        }
        top.x = Some(value.clone());

        let stack_id = top.id;
        self.record_event(Event::XAnnounced {
            stack_id,
            player: input.player,
            value,
        })?;
        self.continue_announcement(cast)?;
        Ok(())
    }
    /// CR 702.84a, 614.1a: an unearthed permanent is exiled instead of
    /// leaving the battlefield for any other zone. A commander bound for its
    /// owner's command zone already had that replacement applied first, as
    /// its owner may order them (CR 616.1, 903.9b).
    pub(super) fn unearth_replaces_departure(&self, object: ObjectId, destination: Zone) -> bool {
        !matches!(
            destination,
            Zone::Exile | Zone::Battlefield | Zone::Command { .. }
        ) && self
            .current(object)
            .is_ok_and(|o| o.id == object && o.zone == Zone::Battlefield && o.unearthed)
    }
    pub(super) fn anger_replaces_death(&self, object: ObjectId, destination: Zone) -> bool {
        matches!(destination,Zone::Graveyard { .. }) && self.current(object).is_ok_and(|o|o.zone==Zone::Battlefield) && self.state.rules.effects.iter().any(|e|matches!(&e.kind,ResolvedEffectKind::ExileDamaged {targets} if targets.contains(&object)))
    }
    // CR 608.2h: `objects` is the set as read when the instruction is performed.
    pub(super) fn move_board_objects(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        action: SetMove,
        block: u32,
        from_graveyards: bool,
    ) -> Result<(), String> {
        if action == SetMove::Hand {
            let mut remaining = vec![];
            for n in 0..4 {
                let p = PlayerId((self.state.rules.turn_owner.0 + n) % 4);
                remaining.extend(objects.iter().copied().filter(|o| {
                    self.state.rules.cards[&o.card].owner == p
                        && self.state.rules.commanders.history(o.card).is_some()
                }));
            }
            return self.continue_board_return(frame, objects, remaining, vec![], block);
        }
        self.commit_board_moves(frame, objects, action, vec![], block, from_graveyards)
    }
    fn commit_board_moves(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        action: SetMove,
        choices: Vec<(ObjectId, bool)>,
        block: u32,
        from_graveyards: bool,
    ) -> Result<(), String> {
        let occurrences = objects
            .into_iter()
            .filter(|o| {
                self.current(*o).is_ok_and(|o| {
                    if from_graveyards {
                        matches!(o.zone, Zone::Graveyard { .. })
                    } else {
                        o.present_on_battlefield()
                    }
                })
            })
            .map(|object| {
                let owner = self.state.rules.cards[&object.card].owner;
                match action {
                    SetMove::Destroy => ProposedEvent::DestroyPermanent { object },
                    SetMove::Exile => ProposedEvent::Move {
                        object,
                        destination: Zone::Exile,
                    },
                    SetMove::Hand => ProposedEvent::Move {
                        object,
                        destination: if choices.contains(&(object, true)) {
                            Zone::Command { owner }
                        } else {
                            Zone::Hand { owner }
                        },
                    },
                }
            })
            .collect::<Vec<_>>();
        self.commit_set_moves(frame, occurrences, action, block)
    }
    fn commit_set_moves(
        &mut self,
        frame: Resolving,
        occurrences: Vec<ProposedEvent>,
        action: SetMove,
        block: u32,
    ) -> Result<(), String> {
        if !occurrences.is_empty() && action == SetMove::Destroy {
            return self.complete_graveyard_group(
                self.instruction_cause(&frame, block)?,
                EventGroup::Simultaneous { occurrences },
                GroupFinish::Instruction {
                    frame,
                    result: GroupResult::DestroySet,
                },
            );
        }
        let mut movements = vec![];
        if !occurrences.is_empty() {
            let batch = self.commit_batch(
                self.instruction_cause(&frame, block)?,
                EventGroup::Simultaneous { occurrences },
            )?;
            for r in batch.occurrences {
                if let EventOutcome::Moved { movement } = r.actual {
                    movements.push(movement);
                }
            }
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::SetMoved {
                movements,
                destroyed_creatures: 0,
            },
        )
    }
    pub(super) fn continue_board_return(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        mut remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    ) -> Result<(), String> {
        remaining.retain(|o| {
            self.current(*o).is_ok_and(|v| v.zone == Zone::Battlefield)
                && self.is_live(self.state.rules.cards[&o.card].owner)
        });
        if let Some(object) = remaining.first().copied() {
            self.rules_mut().frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand {
                    owner: self.state.rules.cards[&object.card].owner,
                },
                continuation: CommanderContinuation::BoardReturn {
                    frame,
                    objects,
                    remaining,
                    choices,
                    source_block,
                },
            };
            Ok(())
        } else {
            self.commit_board_moves(frame, objects, SetMove::Hand, choices, source_block, false)
        }
    }
    pub(super) fn validate_board_return(
        &self,
        frame: &Resolving,
        objects: &[ObjectId],
        remaining: &[ObjectId],
        choices: &[(ObjectId, bool)],
        block: u32,
    ) -> Result<(), String> {
        self.validate_resolving(frame)?;
        if !matches!(self.program_for(self.entry(frame.stack_id)?)?.instructions.get(frame.cursor as usize),Some(Instruction::Kernel { node: mtg_contracts::kir::Node::RemoveTargets { removal: mtg_contracts::kir::GroupRemoval::OwnerHand, .. }, source_block }) if *source_block==block)
            || objects.len() > self.event_group_limit()
            || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
            || remaining.iter().any(|o| !objects.contains(o))
            || choices
                .iter()
                .any(|(o, _)| !objects.contains(o) || remaining.contains(o))
        {
            return Err("invalid mass return continuation".into());
        }
        Ok(())
    }

    pub(super) fn create_set_effect(
        &mut self,
        frame: Resolving,
        kind: ResolvedEffectKind,
        block: u32,
    ) -> Result<(), String> {
        let handles = match &self.entry(frame.stack_id)?.kind {
            StackKind::Spell => self.entry(frame.stack_id)?.effect_source_handles.clone(),
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
        };
        let targets = match &kind {
            ResolvedEffectKind::RemoveIndestructibleSet { targets, .. }
            | ResolvedEffectKind::PumpSet { targets, .. }
            | ResolvedEffectKind::GrantKeywordsSet { targets, .. }
            | ResolvedEffectKind::GainControl { targets, .. }
            | ResolvedEffectKind::SetCharacteristics { targets, .. }
            | ResolvedEffectKind::PumpAnnouncedX { targets, .. }
            | ResolvedEffectKind::ExileDamaged { targets } => targets,
            _ => return Err("not set effect".into()),
        };
        let mut target_handles = vec![];
        for object in targets {
            target_handles.extend(self.capture_trigger_source_handles(*object)?);
        }
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
        let actual = self.commit(
            ProposedEvent::CreateEffect {
                effect: Box::new(effect),
            },
            self.instruction_cause(&frame, block)?,
        )?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    pub(crate) fn execute_wipes_instruction(
        &mut self,
        frame: Resolving,
        i: Instruction,
    ) -> Result<(), String> {
        match i {
            i if mtg_contracts::ObjectSet::pumped(&i).is_some() => {
                let (what, power, toughness, source_block) =
                    i.pumped().ok_or("pump set instruction invariant")?;
                let (power, toughness) = (power.clone(), toughness.clone());
                // CR 608.2h: the audited battlefield set, read as the effect
                // is created, through the same reader destroy and move use.
                let mtg_contracts::kir::SetExpr::Matching { zone, filter } = what else {
                    return Err("pump set instruction invariant".into());
                };
                let (targets, _) = self.kernel_set(&frame, zone, filter)?;
                self.create_set_effect(
                    frame,
                    ResolvedEffectKind::PumpSet {
                        targets,
                        power,
                        toughness,
                    },
                    source_block,
                )
            }

            _ => Err("not wipe instruction".into()),
        }
    }
}

impl RulesTxn {
    pub(crate) fn valid_set_effect_targets(
        &self,
        e: &ResolvedEffect,
        targets: &[ObjectId],
        set: Option<&ObjectSet>,
    ) -> bool {
        e.target_handles.len() == targets.len() * 4
            && targets.len() <= self.event_group_limit()
            && targets.iter().collect::<BTreeSet<_>>().len() == targets.len()
            && targets.iter().all(|o| {
                self.state.rules.cards.contains_key(&o.card)
                    && self.copiable_definition(o).is_ok_and(|d| match set {
                        Some(ObjectSet::OpponentNonlandPermanents) => {
                            !d.characteristics.has_type(CardKind::Land)
                        }
                        Some(ObjectSet::ControllerPermanents) => matches!(
                            d.characteristics.kind,
                            CardKind::Creature
                                | CardKind::Artifact
                                | CardKind::Enchantment
                                | CardKind::Land
                        ),
                        _ => self.historical_creature_possible(&d.characteristics),
                    })
            })
    }
}

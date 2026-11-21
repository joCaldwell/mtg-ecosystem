//! CR 117.1a, 305.1–3, 400.7j, 406.3, 514.2, 601.2 and 611.2a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(crate) fn may_play_exile(&self, player: PlayerId, object: ObjectId) -> bool {
        self.current(object)
            .is_ok_and(|o| o.zone == Zone::Exile && o.hideaway.is_none())
            && self.has_exile_play_grant(player, object)
    }

    fn has_exile_play_grant(&self, player: PlayerId, object: ObjectId) -> bool {
        self.is_live(player)
            && self.state.rules.effects.iter().any(|effect| {
                effect.origin.controller == player
                    && matches!(&effect.kind, ResolvedEffectKind::PlayExiled { objects, .. }
                        if objects.contains(&object))
            })
    }

    pub(super) fn paid_exile_cast_authorized(&self, player: PlayerId, source: ObjectId) -> bool {
        // A spell on the stack cannot outlast its permission. Turn transitions
        // and cleanup require an empty stack. Canceling restores the predecessor.
        source
            .incarnation
            .checked_sub(1)
            .is_some_and(|incarnation| {
                self.has_exile_play_grant(
                    player,
                    ObjectId {
                        incarnation,
                        ..source
                    },
                )
            })
    }

    fn impulse_plan(&self, frame: &Resolving) -> Result<Option<(BatchCause, EventGroup)>, String> {
        let Some(Instruction::ExileLibraryForPlay {
            count,
            window,
            source_block,
        }) = self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("missing library play instruction".into());
        };
        let handles = match &frame.spell.kind {
            StackKind::Spell => &frame.spell.effect_source_handles,
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Intrinsic { .. },
                witness: TriggerEventWitness::LoreAdded { .. },
                source_handles,
                ..
            } => source_handles,
            _ => return Err("library play requires a spell or intrinsic chapter".into()),
        };
        if frame.spell.paid.is_none() || !frame.legal_targets.is_empty() {
            return Err("library play requires a completed untargeted invocation".into());
        }
        let player = frame.spell.controller;
        if !self.is_live(player) {
            return Ok(None);
        }
        let cards = self.library_prefix(player, &Quantity::from(i64::from(*count)))?;
        if cards.is_empty() {
            return Ok(None);
        }
        let objects = cards
            .iter()
            .map(|id| {
                Ok(ObjectId {
                    card: id.card,
                    incarnation: id
                        .incarnation
                        .checked_add(1)
                        .ok_or("incarnation resource limit")?,
                })
            })
            .collect::<Result<_, String>>()?;
        let effect = ResolvedEffect {
            target_handles: vec![],
            id: EffectId(self.state.rules.next_effect_id),
            origin: EffectOrigin {
                activated_ability: None,
                mode: frame.spell.mode,
                stack_id: frame.spell.id,
                source: frame.spell.source,
                controller: player,
                source_handles: handles.clone(),
                cursor: frame.cursor,
                source_block: *source_block,
            },
            expires_in_cleanup_of_turn: self.state.rules.turn.number,
            kind: ResolvedEffectKind::PlayExiled {
                objects,
                window: *window,
            },
        };
        let group = EventGroup::Ordered {
            children: vec![
                EventGroup::Simultaneous {
                    occurrences: cards
                        .into_iter()
                        .map(|object| ProposedEvent::Move {
                            object,
                            destination: Zone::Exile,
                        })
                        .collect(),
                },
                EventGroup::Occurrence {
                    proposed: ProposedEvent::CreateEffect {
                        effect: Box::new(effect),
                    },
                },
            ],
        };
        Ok(Some((Self::staple_cause(frame, *source_block), group)))
    }

    pub(super) fn execute_impulse_play(&mut self, frame: Resolving) -> Result<(), String> {
        let Some((cause, group)) = self.impulse_plan(&frame)? else {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        };
        let batch = self.commit_batch(cause, group)?;
        let actual = batch
            .result(&[1])
            .cloned()
            .ok_or("missing play permission outcome")?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }

    pub(super) fn authorize_impulse_batch(
        &self,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<(), String> {
        let Frame::Resolve { frame } = &self.state.rules.frame else {
            return Err("library play grant is outside spell resolution".into());
        };
        self.validate_resolving(frame)?;
        if self
            .impulse_plan(frame)?
            .is_some_and(|(expected, movements)| expected == *cause && movements == *group)
            && !self
                .state
                .rules
                .effects
                .iter()
                .any(|e| e.origin.stack_id == frame.spell.id && e.origin.cursor == frame.cursor)
        {
            return Ok(());
        }
        Err("library play must move the exact original prefix and grant its successors".into())
    }

    pub(super) fn valid_impulse_effect_objects(
        &self,
        effect: &ResolvedEffect,
        objects: &[ObjectId],
        count: u32,
    ) -> bool {
        effect.target_handles.is_empty()
            && !objects.is_empty()
            && objects.len() <= count as usize
            && objects
                .iter()
                .map(|o| o.card)
                .collect::<BTreeSet<_>>()
                .len()
                == objects.len()
            && objects.iter().all(|o| {
                o.incarnation > 0
                    && self
                        .state
                        .rules
                        .cards
                        .get(&o.card)
                        .is_some_and(|c| c.owner == effect.origin.controller)
                    && self
                        .state
                        .rules
                        .objects
                        .get(&o.card)
                        .is_none_or(|r| r.id.incarnation >= o.incarnation)
            })
    }
}

pub(super) fn creates_permission(event: &ProposedEvent) -> bool {
    matches!(event, ProposedEvent::CreateEffect { effect }
        if matches!(effect.kind, ResolvedEffectKind::PlayExiled { .. }))
}

//! CR 601.2c and 608.2b: the caster selects a player and distinct physical
//! cards in that player's graveyard. Recheck the announced incarnations only.
use super::*;

/// The paid target instruction alone can authorize appending multiple
/// graveyard cards before shuffling. The bound group is kept private here.
pub(super) struct GraveyardShuffleMoves {
    cause: BatchCause,
    group: EventGroup,
}
impl GraveyardShuffleMoves {
    pub(super) fn into_batch(self) -> (BatchCause, EventGroup) {
        (self.cause, self.group)
    }
}

impl Game {
    pub(crate) fn player_graveyard_card(&self, object: &Object, player: PlayerId) -> bool {
        object.zone == (Zone::Graveyard { owner: player })
            && !self.state.rules.tokens.contains_key(&object.id.card)
            && self
                .state
                .rules
                .cards
                .get(&object.id.card)
                .is_some_and(|c| c.owner == player)
    }

    pub(super) fn graveyard_group_structure(&self, entities: &[TargetEntity], maximum: u8) -> bool {
        let [TargetEntity::Player { player }, cards @ ..] = entities else {
            return false;
        };
        player.0<4 && cards.len()<=usize::from(maximum) && cards.iter().enumerate().all(|(i,c)| matches!(c,TargetEntity::Object{object} if !cards[..i].contains(c) && self.state.rules.cards.get(&object.card).is_some_and(|card|card.owner==*player)))
    }

    pub(crate) fn announced_graveyard_group(
        &self,
        caster: PlayerId,
        chooser: PlayerId,
        maximum: u8,
        entities: &[TargetEntity],
    ) -> Result<(PlayerId, Vec<ObjectId>), String> {
        let [TargetEntity::Player { player }, cards @ ..] = entities else {
            return Err("missing graveyard owner target".into());
        };
        if chooser != caster
            || !self.is_live(caster)
            || !self.entity_legal(TargetEntity::Player { player: *player })
            || cards.len() > usize::from(maximum)
        {
            return Err("invalid chooser, player or number of graveyard targets".into());
        }
        let mut selected = Vec::with_capacity(cards.len());
        for card in cards {
            let TargetEntity::Object { object } = card else {
                return Err("expected graveyard card".into());
            };
            if selected.contains(object)
                || !self
                    .current(*object)
                    .is_ok_and(|o| self.player_graveyard_card(o, *player))
            {
                return Err("duplicate or ineligible graveyard card target".into());
            }
            selected.push(*object);
        }
        Ok((*player, selected))
    }

    pub(crate) fn remaining_graveyard_targets(
        &self,
        owner: PlayerId,
        cards: &[ObjectId],
    ) -> Vec<ObjectId> {
        cards
            .iter()
            .copied()
            .filter(|id| {
                self.current(*id)
                    .is_ok_and(|o| self.player_graveyard_card(o, owner))
            })
            .collect()
    }
}

impl Game {
    pub(super) fn graveyard_legality<'a>(
        &self,
        frame: &'a Resolving,
    ) -> Result<(&'a [bool], u8), String> {
        let program = self.program_for(self.entry(frame.stack_id)?)?;
        if let Some(children) = self.mixed_mode_children(self.entry(frame.stack_id)?)? {
            let index = frame.cursor as usize;
            let child = children.get(index).ok_or("missing selected shuffle mode")?;
            let max = mtg_contracts::graveyard_shuffle::offered_by(child)
                .ok_or("missing selected graveyard target")?;
            let start = children[..index]
                .iter()
                .map(|p| p.targets.len())
                .sum::<usize>();
            let legal = frame
                .legal_targets
                .get(start..)
                .ok_or("missing selected graveyard legality")?;
            return Ok((legal, max));
        }
        Ok((
            &frame.legal_targets,
            mtg_contracts::graveyard_shuffle::offered_by(program)
                .ok_or("unbound player graveyard program")?,
        ))
    }
    fn targeted_graveyard_moves(
        &self,
        frame: &Resolving,
        block: u32,
    ) -> Result<(PlayerId, Vec<ObjectId>), String> {
        let p = self.program_for(self.entry(frame.stack_id)?)?;
        let (legal, maximum) = self.graveyard_legality(frame)?;
        if !matches!(p.instructions.get(frame.cursor as usize),Some(Instruction::ShufflePlayerGraveyard{source_block}) if *source_block==block)
            || !self.graveyard_group_structure(&self.entry(frame.stack_id)?.entity_targets, maximum)
            || self.entry(frame.stack_id)?.kind.is_spell() == false
            || self.entry(frame.stack_id)?.paid.is_none()
            || legal.len() != self.entry(frame.stack_id)?.entity_targets.len()
        {
            return Err("invalid resolving graveyard shuffle".into());
        }
        let TargetEntity::Player { player } = self.entry(frame.stack_id)?.entity_targets[0] else {
            return Err("missing shuffle recipient".into());
        };
        let ids = self
            .entry(frame.stack_id)?
            .entity_targets
            .iter()
            .zip(legal)
            .skip(1)
            .filter_map(|(target, legal)| match target {
                TargetEntity::Object { object } if *legal => Some(*object),
                _ => None,
            })
            .collect::<Vec<_>>();
        Ok((player, self.remaining_graveyard_targets(player, &ids)))
    }
    pub(super) fn start_graveyard_shuffle(
        &mut self,
        frame: Resolving,
        block: u32,
    ) -> Result<(), String> {
        let (player, objects) = self.targeted_graveyard_moves(&frame, block)?;
        if self.graveyard_legality(&frame)?.0.first() != Some(&true) || !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let remaining = objects
            .iter()
            .copied()
            .filter(|id| self.state.rules.commanders.history(id.card).is_some())
            .collect();
        self.continue_graveyard_shuffle(frame, objects, remaining, vec![], block)
    }
    pub(super) fn validate_graveyard_shuffle_continuation(
        &self,
        frame: &Resolving,
        objects: &[ObjectId],
        remaining: &[ObjectId],
        choices: &[(ObjectId, bool)],
        block: u32,
    ) -> Result<(), String> {
        self.validate_resolving(frame)?;
        let (_, selected) = self.targeted_graveyard_moves(frame, block)?;
        let commanders = objects
            .iter()
            .copied()
            .filter(|id| self.state.rules.commanders.history(id.card).is_some());
        if self.graveyard_legality(frame)?.0.first() != Some(&true)
            || objects != selected
            || !commanders.eq(choices
                .iter()
                .map(|(id, _)| *id)
                .chain(remaining.iter().copied()))
        {
            return Err(
                "graveyard replacement choices are not the exact announced commander prefix".into(),
            );
        }
        Ok(())
    }
    pub(super) fn continue_graveyard_shuffle(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        block: u32,
    ) -> Result<(), String> {
        let (player, _) = self.targeted_graveyard_moves(&frame, block)?;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        self.validate_graveyard_shuffle_continuation(
            &frame, &objects, &remaining, &choices, block,
        )?;
        let destination = Zone::Library { owner: player };
        if let Some(object) = remaining.first().copied() {
            self.rules_mut().frame = Frame::CommanderReturn {
                object,
                destination,
                continuation: Box::new(CommanderContinuation::GraveyardShuffle {
                    frame,
                    objects,
                    remaining,
                    choices,
                    source_block: block,
                }),
            };
            return Ok(());
        }
        if !objects.is_empty() {
            let occurrences = objects
                .iter()
                .map(|object| ProposedEvent::Move {
                    object: *object,
                    destination: if choices.contains(&(*object, true)) {
                        Zone::Command { owner: player }
                    } else {
                        destination
                    },
                })
                .collect();
            self.commit_graveyard_shuffle_moves(GraveyardShuffleMoves {
                cause: self.instruction_cause(&frame, block)?,
                group: EventGroup::Simultaneous { occurrences },
            })?;
        }
        self.begin_player_library_shuffle(frame, player, block)
    }
}

//! CR 601.2c and 608.2b: the caster selects a player and distinct physical
//! cards in that player's graveyard. Recheck the announced incarnations only.
use super::*;

impl Game {
    pub(super) fn player_graveyard_card(&self, object: &Object, player: PlayerId) -> bool {
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

    pub(super) fn announced_graveyard_group(
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

    pub(super) fn remaining_graveyard_targets(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::equipped_indestructible_tests::{object, prepared};

    #[test]
    fn graveyard_target_group_requires_caster_and_preserves_physical_incarnations() {
        let mut p = prepared(
            &[("Forest", false), ("Plains", false), ("Island", false)],
            false,
        );
        for (i, c) in p.config.cards.iter_mut().take(3).enumerate() {
            let owner = PlayerId(if i == 2 { 2 } else { 1 });
            c.owner = owner;
            c.controller = owner;
            c.zone = Zone::Graveyard { owner };
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        let caster = PlayerId(0);
        let owner = PlayerId(1);
        let who = TargetEntity::Player { player: owner };
        let ids = [object(&g, 0), object(&g, 1), object(&g, 2)];
        let targets = ids.map(|object| TargetEntity::Object { object });
        assert_eq!(
            g.announced_graveyard_group(caster, caster, 2, &[who])
                .unwrap(),
            (owner, vec![])
        );
        let entities = [who, targets[0], targets[1]];
        let (_, cards) = g
            .announced_graveyard_group(caster, caster, 2, &entities)
            .unwrap();
        assert!(
            g.announced_graveyard_group(caster, owner, 2, &entities)
                .is_err()
        );
        for (limit, selection) in [
            (1, entities),
            (2, [who, targets[0], targets[0]]),
            (2, [who, targets[0], targets[2]]),
        ] {
            assert!(
                g.announced_graveyard_group(caster, caster, limit, &selection)
                    .is_err()
            );
        }
        let moved = g
            .move_object_for(ids[0], Zone::Hand { owner }, BatchCause::Fixture)
            .unwrap();
        g.move_object_for(
            moved.successor,
            Zone::Graveyard { owner },
            BatchCause::Fixture,
        )
        .unwrap();
        assert_eq!(g.remaining_graveyard_targets(owner, &cards), [ids[1]]);
    }
}

impl Game {
    fn targeted_graveyard_moves(
        &self,
        frame: &Resolving,
        block: u32,
    ) -> Result<(PlayerId, Vec<ObjectId>), String> {
        let p = self.program_for(&frame.spell)?;
        let maximum = mtg_contracts::graveyard_shuffle::offered_by(p)
            .ok_or("unbound player graveyard program")?;
        if !matches!(p.instructions.get(frame.cursor as usize),Some(Instruction::ShufflePlayerGraveyard{source_block}) if *source_block==block)
            || !self.graveyard_group_structure(&frame.spell.entity_targets, maximum)
            || frame.spell.kind.is_spell() == false
            || frame.spell.paid.is_none()
            || frame.legal_targets.len() != frame.spell.entity_targets.len()
        {
            return Err("invalid resolving graveyard shuffle".into());
        }
        let TargetEntity::Player { player } = frame.spell.entity_targets[0] else {
            return Err("missing shuffle recipient".into());
        };
        let ids = frame
            .spell
            .entity_targets
            .iter()
            .zip(&frame.legal_targets)
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
        if frame.legal_targets.first() != Some(&true) || !self.is_live(player) {
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
        if frame.legal_targets.first() != Some(&true)
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
            self.state.rules.frame = Frame::CommanderReturn {
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
            self.commit_batch(
                Self::staple_cause(&frame, block),
                EventGroup::Simultaneous { occurrences },
            )?;
        }
        self.begin_library_shuffle(frame, player, block)
    }
}

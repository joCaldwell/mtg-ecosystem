//! CR 101.4a-b, 608.2d, 701.9a-b, 800.4a and 903.9a. Hidden-zone choices
//! are private in APNAP order. All resulting discards happen simultaneously.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupDiscardProgress {
    pub(super) remaining: Vec<PlayerId>,
    selected: Vec<(PlayerId, Vec<ObjectId>)>,
}

impl GroupDiscardProgress {
    pub(super) fn select(
        &mut self,
        player: PlayerId,
        objects: Vec<ObjectId>,
    ) -> Result<(), String> {
        if self.remaining.first() != Some(&player) {
            return Err("unexpected group discard chooser".into());
        }
        self.remaining.remove(0);
        self.selected.push((player, objects));
        Ok(())
    }
}

impl Game {
    fn group_discard_recipients(
        &self,
        controller: PlayerId,
        scope: &GroupDiscardScope,
    ) -> Vec<PlayerId> {
        self.apnap_players()
            .into_iter()
            .filter(|&p| match scope {
                GroupDiscardScope::EachPlayer => true,
                GroupDiscardScope::EachOpponent => p != controller,
            })
            .collect()
    }

    pub(super) fn group_discard_step(
        &mut self,
        mut frame: Resolving,
        scope: GroupDiscardScope,
        count: Quantity,
        source_block: u32,
    ) -> Result<(), String> {
        if count.is_negative() {
            return Err("negative group discard count".into());
        }
        let mut group = frame
            .group_discard
            .take()
            .unwrap_or_else(|| GroupDiscardProgress {
                remaining: self.group_discard_recipients(frame.spell.controller, &scope),
                selected: vec![],
            });
        while let Some(&player) = group.remaining.first() {
            let count = self.bounded_hand_count(player, &count);
            if count > 0 {
                frame.group_discard = Some(group);
                self.state.rules.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Discard {
                        player,
                        count,
                        source_block,
                    },
                };
                return Ok(());
            }
            group.select(player, vec![])?;
        }
        let occurrences: Vec<_> = group
            .selected
            .into_iter()
            .flat_map(|(player, objects)| {
                objects
                    .into_iter()
                    .map(move |object| ProposedEvent::DiscardCard { player, object })
            })
            .collect();
        if occurrences.is_empty() {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        self.complete_graveyard_group(
            Self::wipe_cause(&frame, source_block),
            EventGroup::Simultaneous { occurrences },
            GroupFinish::Instruction {
                frame,
                result: GroupResult::Effect,
            },
        )
    }

    pub(super) fn remove_group_discard_recipient(&mut self, departed: PlayerId) {
        let (frame, chooser_left) = match &mut self.state.rules.frame {
            Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Discard { player, .. },
            } => (frame, *player == departed),
            Frame::Resolve { frame } => (frame, false),
            _ => return,
        };
        let Some(group) = &mut frame.group_discard else {
            return;
        };
        group.remaining.retain(|&p| p != departed);
        group.selected.retain(|(p, _)| *p != departed);
        if chooser_left {
            self.state.rules.frame = Frame::Resolve {
                frame: frame.clone(),
            };
        }
    }

    pub(super) fn validate_group_discard_progress(&self, frame: &Resolving) -> Result<(), String> {
        let Some(group) = &frame.group_discard else {
            return Ok(());
        };
        let Some(Instruction::DiscardPlayerSet { scope, count, .. }) = self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("group discard instruction invariant".into());
        };
        let recipients = self.group_discard_recipients(frame.spell.controller, scope);
        if !matches!(
            self.state.rules.frame,
            Frame::Resolve { .. }
                | Frame::ResolutionChoice {
                    choice: ResolutionChoice::Discard { .. },
                    ..
                }
        ) || group
            .selected
            .iter()
            .map(|(p, _)| p)
            .chain(&group.remaining)
            .ne(&recipients)
            || group.selected.iter().any(|(p, cards)| {
                !self.valid_discard_selection(*p, self.bounded_hand_count(*p, count), cards)
            })
        {
            return Err("group discard progress invariant".into());
        }
        Ok(())
    }
}

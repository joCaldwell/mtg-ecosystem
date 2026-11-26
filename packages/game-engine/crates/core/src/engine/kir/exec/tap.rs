//! CR 701.26, 608.2h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;

impl RulesTxn {
    pub(in crate::engine) fn kernel_tap_set(
        &mut self,
        frame: Resolving,
        zone: &ZoneExpr,
        filter: &Pred,
        tap: bool,
        source_block: u32,
    ) -> Result<(), String> {
        let objects = match filter.0.as_slice() {
            [
                mtg_contracts::kir::Atom::Type {
                    kind: CardKind::Creature,
                },
                mtg_contracts::kir::Atom::ControlledBy {
                    player:
                        player @ PlayerExpr::Is {
                            who: Ref::Target { .. },
                        },
                },
            ] => match self.kernel_players(&frame, player)? {
                Players::One(player) => self.controlled_creatures(player),
                Players::Nobody => vec![],
                Players::EachOpponent => return Err("expected one targeted player".into()),
            },
            _ => self.kernel_set(&frame, zone, filter)?.0,
        };
        self.kernel_tap_objects(frame, objects, tap, source_block)
    }

    pub(in crate::engine) fn kernel_tap_objects(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        tap: bool,
        source_block: u32,
    ) -> Result<(), String> {
        let mut occurrences = Vec::new();
        for object in objects {
            let Ok(current) = self.current(object) else {
                continue;
            };
            if current.present_on_battlefield() && current.tapped != tap {
                occurrences.push(if tap {
                    ProposedEvent::Tap { object }
                } else {
                    ProposedEvent::Untap { object }
                });
            }
        }
        if !occurrences.is_empty() {
            self.commit_batch(
                self.instruction_cause(&frame, source_block)?,
                EventGroup::Simultaneous { occurrences },
            )?;
        }
        self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
    }
}

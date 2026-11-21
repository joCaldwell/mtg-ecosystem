//! Kernel IR interpreter.
//!
//! One instruction of a resolving program is one kernel node. The interpreter
//! evaluates the node's references, players and amounts against the frozen
//! invocation, then performs the verb through the same event primitives every
//! other executor uses. It owns no rules of its own: it decides *which*
//! players, objects and numbers a verb acts on, never what the action does.
//!
//! Only the cells `mtg_contracts::kir::check` audits are executed. Admission
//! rejects everything else, so reaching an unaudited cell here is an invariant
//! fault, not a rules outcome.
//!
//! CR 608.2b-c: legality of each target is the frame's resolution-start
//! snapshot; an instruction whose only recipient is an illegal target does
//! nothing. CR 608.2h: everything else is read as the verb is performed.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{
    Amount, Context, Destination, Node, PlayerExpr, Pred, Ref, SetExpr, Verb, ZoneExpr,
};

/// The kernel context of a resolving invocation (CR 113.3).
fn context(frame: &Resolving) -> Context {
    match frame.spell.kind {
        StackKind::Spell => Context::Spell,
        StackKind::Activated { .. } => Context::Activated,
        StackKind::Triggered { .. } => Context::Triggered,
    }
}

/// Desugars the not-yet-migrated announced-X life instruction: the same life
/// verb with the announced X as its amount (CR 107.3).
pub(super) fn announced_x_life(
    frame: &Resolving,
    recipient: &LifeRecipient,
    gain: bool,
) -> Result<Node, String> {
    let player = match recipient {
        LifeRecipient::Controller => PlayerExpr::you(context(frame)),
        LifeRecipient::Target { target_slot } => PlayerExpr::target(*target_slot),
        _ => return Err("unsupported announced X life recipient".into()),
    };
    let amount = Amount::Var {
        of: Ref::AnnouncedX,
    };
    Ok(Node::Do(if gain {
        Verb::GainLife { player, amount }
    } else {
        Verb::LoseLife { player, amount }
    }))
}

/// The players a player expression denotes for one resolving invocation.
enum Players {
    One(PlayerId),
    /// The only denoted player is an illegal target or an absent event
    /// player: the verb is performed on no one (CR 608.2b).
    Nobody,
    /// CR 102.3, 101.4: every opponent of the controller.
    EachOpponent,
}

impl Game {
    pub(crate) fn execute_kernel(
        &mut self,
        frame: Resolving,
        node: Node,
        source_block: u32,
    ) -> Result<(), String> {
        let Node::Do(verb) = node else {
            return Err("kernel control node has no audited executor".into());
        };
        match verb {
            Verb::GainLife { player, amount } => {
                self.kernel_life(frame, true, &player, &amount, source_block)
            }
            Verb::LoseLife { player, amount } => {
                self.kernel_life(frame, false, &player, &amount, source_block)
            }
            // CR 701.8, 400.3, 608.2h: read the set now, then move every
            // member in one simultaneous event.
            Verb::Destroy {
                what: SetExpr::Matching { zone, filter },
                ..
            } => {
                let (objects, graveyards) = self.kernel_set(&frame, &zone, &filter)?;
                self.move_board_objects(
                    frame,
                    objects,
                    wipes::SetMove::Destroy,
                    source_block,
                    graveyards,
                )
            }
            Verb::Move {
                what: SetExpr::Matching { zone, filter },
                to,
            } => {
                let action = match to {
                    Destination::Exile => wipes::SetMove::Exile,
                    Destination::OwnerHand => wipes::SetMove::Hand,
                    _ => return Err("kernel set destination has no audited executor".into()),
                };
                let (objects, graveyards) = self.kernel_set(&frame, &zone, &filter)?;
                self.move_board_objects(frame, objects, action, source_block, graveyards)
            }
            _ => Err("kernel verb has no audited executor".into()),
        }
    }

    /// CR 608.2h, 109.5: the members of a board-wide or graveyard-wide set,
    /// relative to the invocation's controller, in object order.
    fn kernel_set(
        &self,
        frame: &Resolving,
        zone: &ZoneExpr,
        filter: &Pred,
    ) -> Result<(Vec<ObjectId>, bool), String> {
        let graveyards = match zone {
            ZoneExpr::Battlefield => false,
            ZoneExpr::Graveyards => true,
            _ => return Err("kernel set zone has no audited evaluation".into()),
        };
        let you = frame.spell.controller;
        let objects = self
            .state
            .rules
            .objects
            .values()
            .filter(|object| {
                crate::query::matches_zone_pred(&self.object_facts(object), you, zone, filter)
                    .is_yes()
            })
            .map(|object| object.id)
            .collect();
        Ok((objects, graveyards))
    }

    fn kernel_players(&self, frame: &Resolving, player: &PlayerExpr) -> Result<Players, String> {
        Ok(match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } => Players::One(frame.spell.controller),
            PlayerExpr::Is {
                who: Ref::EventPlayer { .. },
            } => frame
                .spell
                .kind
                .event_player()?
                .map_or(Players::Nobody, Players::One),
            PlayerExpr::Is {
                who: Ref::Target { slot },
            } => {
                let slot = *slot as usize;
                if frame.legal_targets.get(slot) != Some(&true) {
                    Players::Nobody
                } else {
                    Players::One(
                        *frame
                            .spell
                            .player_targets
                            .get(slot)
                            .ok_or("missing kernel player target")?,
                    )
                }
            }
            PlayerExpr::EachOpponent => Players::EachOpponent,
            _ => return Err("kernel player expression has no audited evaluation".into()),
        })
    }

    fn kernel_amount(&self, frame: &Resolving, amount: &Amount) -> Result<Quantity, String> {
        match amount {
            Amount::Fixed { value } => Ok(value.clone()),
            Amount::Var {
                of: Ref::AnnouncedX,
            } => frame.spell.x.clone().ok_or("missing announced X".into()),
            _ => Err("kernel amount has no audited evaluation".into()),
        }
    }

    // CR 119.3, 608.2b-c, 704.4: one life event per instruction, with no
    // state-based action check until the program finishes. A self-targeted
    // loss may reach zero and recover before the next priority checkpoint.
    fn kernel_life(
        &mut self,
        frame: Resolving,
        gain: bool,
        player: &PlayerExpr,
        amount: &Amount,
        source_block: u32,
    ) -> Result<(), String> {
        let computed = !matches!(amount, Amount::Fixed { .. });
        let amount = self.kernel_amount(&frame, amount)?;
        let skipped = |game: &mut Self, frame| {
            game.finish_instruction(frame, InstructionOutcome::Life { actual: None })
        };
        // CR 119.8, 107.1b: a change of zero is no life event. An announced X
        // may be zero. A fixed zero is the tail of the complete
        // permanent-grant family and nothing else: any other fixed zero
        // reaches the life event, which rejects it.
        if amount == Quantity::zero()
            && (computed
                || (gain
                    && player.is_you()
                    && source_block == 0
                    && frame.cursor == 1
                    && frame.spell.kind.is_spell()
                    && self.program_for(&frame.spell).is_ok_and(|program| {
                        temporary_permanent_keyword_program(program)
                            .is_some_and(|family| family.controller_life == Some(&amount))
                    })))
        {
            return skipped(self, frame);
        }
        let player = match self.kernel_players(&frame, player)? {
            Players::EachOpponent => {
                self.commit_opponent_life(&frame, &amount, source_block, gain)?;
                return skipped(self, frame);
            }
            Players::Nobody => return skipped(self, frame),
            Players::One(player) => player,
        };
        if !self.is_live(player) {
            return skipped(self, frame);
        }
        let proposed = if gain {
            ProposedEvent::GainLife { player, amount }
        } else {
            ProposedEvent::LoseLife { player, amount }
        };
        let actual = self.commit(proposed, Self::staple_cause(&frame, source_block))?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Life {
                actual: Some(actual),
            },
        )
    }
}

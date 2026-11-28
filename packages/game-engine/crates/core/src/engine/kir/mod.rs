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
    Amount, Destination, Node, PlayerExpr, Pred, Ref, SetExpr, Verb, ZoneExpr,
};

mod control;
mod exec;
mod object_choice;
pub use control::{LeafProgram, Task};

/// CR 608.2h: an upper bound on a resolution count recorded when its step
/// was performed. Later instructions of the same program may change the
/// number, so a saved value is bounded, never re-derived. A count is bounded
/// by every object that could exist; a greatest stat or devotion only by
/// being nonnegative.
pub(super) fn resolution_count_bound(amount: &Amount) -> Option<Quantity> {
    match amount {
        Amount::Count { .. } => Some(Quantity::from(
            (4 * capacities::OWNED_LIMIT + capacities::TOKEN_CAPACITY as usize) as i64,
        )),
        Amount::Times { factor, of } => resolution_count_bound(of).map(|mut bound| {
            bound.multiply(factor);
            bound
        }),
        _ => None,
    }
}

/// The players a player expression denotes for one resolving invocation.
pub(super) enum Players {
    One(PlayerId),
    /// The only denoted player is an illegal target or an absent event
    /// player: the verb is performed on no one (CR 608.2b).
    Nobody,
    /// CR 102.3, 101.4: every opponent of the controller.
    EachOpponent,
}

impl RulesTxn {
    /// The target domain of an instruction that destroys one announced
    /// target, if it is one.
    pub(super) fn target_destroy_domain(
        &self,
        entry: &StackInstance,
        instruction: &Instruction,
    ) -> Result<Option<TargetDomain>, String> {
        let Some((slot, ..)) = instruction.target_destroy() else {
            return Ok(None);
        };
        Ok(self
            .program_for(entry)?
            .targets
            .get(slot as usize)
            .map(|t| t.domain))
    }

    pub(crate) fn execute_kernel(
        &mut self,
        frame: Resolving,
        node: Node,
        source_block: u32,
    ) -> Result<(), String> {
        if let Node::SacrificeForTappedReturn { .. } = node {
            let player = self.entry(frame.stack_id)?.controller;
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::TappedReturnPayment {
                    player,
                    source_block,
                },
            };
            return Ok(());
        }
        if let Node::TapTargets { tap, .. } = node {
            let objects = self
                .entry(frame.stack_id)?
                .targets
                .iter()
                .zip(&frame.legal_targets)
                .filter_map(|(&object, &legal)| legal.then_some(object))
                .collect();
            return self.kernel_tap_objects(frame, objects, tap, source_block);
        }
        // CR 115.3, 608.2b: the legal announced members leave or are
        // destroyed in one simultaneous event (CR 603.10a look-back).
        if let Node::RemoveTargets { removal, .. } = node {
            let objects = self
                .entry(frame.stack_id)?
                .targets
                .iter()
                .zip(&frame.legal_targets)
                .filter_map(|(&object, &legal)| legal.then_some(object))
                .collect();
            let action = match removal {
                mtg_contracts::kir::GroupRemoval::Destroy => wipes::SetMove::Destroy,
                mtg_contracts::kir::GroupRemoval::Exile => wipes::SetMove::Exile,
                mtg_contracts::kir::GroupRemoval::OwnerHand => wipes::SetMove::Hand,
            };
            return self.move_board_objects(frame, objects, action, source_block, false);
        }
        if matches!(node, Node::ChooseObject { .. }) {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Object {},
            };
            return Ok(());
        }
        if let Node::Delayed { event, .. } = &node {
            return self.execute_delayed_trigger(frame, event, source_block);
        }
        if matches!(node, Node::Do(Verb::ReturnCapturedExile {})) {
            return self.execute_captured_return(frame, source_block);
        }
        if matches!(node, Node::Do(Verb::ExileCapturedReturn {})) {
            return self.execute_captured_exile(frame, source_block);
        }
        if matches!(node, Node::Do(Verb::ReturnDashedToHand {})) {
            return self.execute_dash_return(frame, source_block);
        }
        if matches!(node, Node::Do(Verb::SacrificeEncoreTokens {})) {
            return self.execute_encore_sacrifice(frame, source_block);
        }
        if matches!(node, Node::Do(Verb::ExileMyriadTokens {})) {
            return self.execute_myriad_exile(frame, source_block);
        }
        if node.verb().is_some_and(Verb::is_evaluated) {
            return self.begin_kernel_control(frame);
        }
        if !matches!(node, Node::Do(_)) {
            let instruction = Instruction::Kernel {
                node: node.clone(),
                source_block,
            };
            if instruction.target_destroy().is_none() {
                return self.begin_kernel_control(frame);
            }
        }
        // CR 608.2b, 608.2d: a verb on one announced target, and the optional
        // destroy, keep the executors whose events, outcomes and choices
        // recovery re-derives: a destroyed creature is destroyed as a
        // creature (CR 701.8), every other target through the removal path.
        let one_target = matches!(
            node,
            Node::Optional { .. }
                | Node::Do(
                    Verb::Destroy {
                        what: SetExpr::One { .. },
                        ..
                    } | Verb::Move {
                        what: SetExpr::One { .. },
                        to: Destination::Exile
                    } | Verb::Tap {
                        what: SetExpr::One { .. }
                    } | Verb::Untap {
                        what: SetExpr::One { .. }
                    }
                )
        );
        if one_target {
            let instruction = Instruction::Kernel { node, source_block };
            let entry = self.entry(frame.stack_id)?;
            return if self.target_destroy_domain(entry, &instruction)?
                == Some(TargetDomain::BattlefieldCreature)
            {
                self.execute_staples_instruction(frame, instruction)
            } else {
                self.execute_removal_instruction(frame, instruction)
            };
        }
        // CR 404.1: exile one player's graveyard.
        if matches!(
            node,
            Node::Do(Verb::Move {
                what: SetExpr::Matching {
                    zone: ZoneExpr::Graveyard { .. },
                    ..
                },
                ..
            })
        ) {
            return self
                .execute_wipes_instruction(frame, Instruction::Kernel { node, source_block });
        }
        if let Node::Do(Verb::PreventDamage { prevention, .. }) = &node {
            return self.kernel_prevent_damage(frame, prevention, source_block);
        }
        if let Node::Do(Verb::RestrictCombat {
            affects,
            restriction,
            ..
        }) = &node
        {
            return self.kernel_restrict_combat(frame, affects, *restriction, source_block);
        }
        if let Node::Do(
            verb @ (Verb::GainControl { .. }
            | Verb::SetCharacteristics { .. }
            | Verb::ModifyUntil {
                change: mtg_contracts::kir::Change::PowerToughnessX { .. },
                ..
            }),
        ) = &node
        {
            return self.kernel_continuous(frame, verb, source_block);
        }
        // CR 611.2c, 613.4c, 613.1f: an end-of-turn change keeps the set,
        // source and target effect executors, whose captured members and
        // effect origins recovery re-derives.
        if let Node::Do(Verb::ModifyUntil {
            what,
            change: mtg_contracts::kir::Change::AddKeywords { keywords },
            ..
        }) = &node
        {
            return self.kernel_grant_keywords(frame, what, keywords, source_block);
        }
        if matches!(node, Node::Do(Verb::ModifyUntil { .. })) {
            let instruction = Instruction::Kernel { node, source_block };
            let set = matches!(instruction.pumped(), Some((SetExpr::Matching { .. }, ..)));
            return if set {
                self.execute_wipes_instruction(frame, instruction)
            } else {
                self.execute_effect_instruction(frame, instruction)
            };
        }
        let Node::Do(verb) = node else {
            return Err("kernel control node has no audited executor".into());
        };
        // CR 701.22-701.25: library verbs and their private choices.
        if matches!(
            verb,
            Verb::Search { .. }
                | Verb::Shuffle { .. }
                | Verb::ReorderLibraryTop { .. }
                | Verb::LookChooseHand { .. }
                | Verb::Scry { .. }
                | Verb::Surveil { .. }
        ) {
            return self.kernel_library(frame, verb, source_block);
        }
        match verb {
            Verb::GiveCounters {
                player,
                counter,
                amount,
            } => self.give_counters(frame, &player, counter, &amount, source_block),
            Verb::RemoveLoreCounter {
                from: Ref::Target { slot },
            } => self.remove_lore_counter(frame, slot, source_block),
            Verb::Populate { .. } => self.begin_keyword_choice(frame, ResolutionChoice::Populate),
            Verb::Proliferate { .. } => {
                self.begin_keyword_choice(frame, ResolutionChoice::Proliferate)
            }
            Verb::Fight { first, second } => {
                self.execute_fight(frame, &first, &second, source_block)
            }
            Verb::Counter { what } => self.execute_counter(frame, what, source_block),
            Verb::Regenerate { what } => self.execute_regenerate(frame, &what, source_block),
            Verb::Attach {
                what: Ref::Source,
                to: Ref::Target { slot: 0 },
            } => self.kernel_equip(frame, source_block),
            Verb::AddMana {
                mana: mtg_contracts::kir::ManaSpec::Color { color, amount },
                ..
            } => {
                let amount = self.kernel_amount(&frame, &amount)?;
                let actual = if amount == Quantity::zero() {
                    None
                } else {
                    let entry = self.entry(frame.stack_id)?;
                    let event = ProposedEvent::AddMana {
                        player: entry.controller,
                        color,
                        amount,
                        source: entry.source,
                        restriction: None,
                    };
                    Some(self.commit(event, self.instruction_cause(&frame, source_block)?)?)
                };
                self.finish_instruction(frame, InstructionOutcome::Effect { actual })
            }
            Verb::Transform { what: Ref::Source } => {
                self.kernel_transform_source(frame, source_block)
            }
            Verb::Tap {
                what: SetExpr::Matching { zone, filter },
            } => self.kernel_tap_set(frame, &zone, &filter, true, source_block),
            Verb::Untap {
                what: SetExpr::Matching { zone, filter },
            } => self.kernel_tap_set(frame, &zone, &filter, false, source_block),
            Verb::Destroy {
                what: SetExpr::Matching { zone, filter },
                ..
            } => self.kernel_set_move(frame, &zone, &filter, None, source_block),
            Verb::Move {
                what: SetExpr::One { object },
                to:
                    to @ (Destination::OwnerHand
                    | Destination::OwnerLibraryTop
                    | Destination::OwnerLibraryBottom
                    | Destination::OwnerLibraryShuffled
                    | Destination::Battlefield { .. }),
            } => self.kernel_single_move(frame, object, to, source_block),
            Verb::Move {
                what: SetExpr::Matching { zone, filter },
                to,
            } => self.kernel_set_move(frame, &zone, &filter, Some(&to), source_block),
            _ => Err("kernel verb has no audited executor".into()),
        }
    }

    /// CR 608.2h, 109.5: the members of a board-wide or graveyard-wide set,
    /// relative to the invocation's controller, in object order.
    pub(super) fn kernel_set(
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
        let entry = self.entry(frame.stack_id)?;
        let you = entry.controller;
        // CR 109.1: a top-level "other" excludes the exact source; the rest
        // of the filter is read as performed.
        let other = mtg_contracts::kir::Atom::IsNot {
            object: Ref::Source,
        };
        let excluded = filter.0.contains(&other).then_some(entry.source);
        let rest;
        let filter = if excluded.is_some() {
            rest = mtg_contracts::kir::Pred(
                filter.0.iter().filter(|a| **a != other).cloned().collect(),
            );
            &rest
        } else {
            filter
        };
        let objects = self
            .state
            .rules
            .objects
            .values()
            .filter(|object| {
                Some(object.id) != excluded
                    && crate::query::matches_zone_pred(
                        &self.object_facts(object),
                        you,
                        zone,
                        filter,
                    )
                    .is_yes()
            })
            .map(|object| object.id)
            .collect();
        Ok((objects, graveyards))
    }

    pub(super) fn kernel_players(
        &self,
        frame: &Resolving,
        player: &PlayerExpr,
    ) -> Result<Players, String> {
        Ok(match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } => Players::One(self.entry(frame.stack_id)?.controller),
            PlayerExpr::Is {
                who: Ref::EventPlayer { .. },
            } => self
                .entry(frame.stack_id)?
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
                        *self
                            .entry(frame.stack_id)?
                            .player_targets
                            .get(slot)
                            .ok_or("missing kernel player target")?,
                    )
                }
            }
            PlayerExpr::EachOpponent => Players::EachOpponent,
            PlayerExpr::ControllerOf {
                object: Ref::Target { slot },
            } => {
                if frame.legal_targets.get(*slot as usize) != Some(&true) {
                    Players::Nobody
                } else {
                    Players::One(self.kernel_target_facts(frame, *slot)?.controller())
                }
            }
            _ => return Err("kernel player expression has no audited evaluation".into()),
        })
    }

    fn kernel_target_facts<'a>(
        &self,
        frame: &'a Resolving,
        slot: u32,
    ) -> Result<&'a removal::TargetFacts, String> {
        frame
            .target_facts
            .get(slot as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| "missing current or last-known target facts".into())
    }

    pub(super) fn kernel_amount(
        &self,
        frame: &Resolving,
        amount: &Amount,
    ) -> Result<Quantity, String> {
        match amount {
            Amount::Fixed { value } => Ok(value.clone()),
            Amount::Var {
                of: Ref::AnnouncedX,
            } => self
                .entry(frame.stack_id)?
                .x
                .clone()
                .ok_or("missing announced X".into()),
            // CR 608.2h: the number of matching battlefield objects now.
            Amount::Count {
                set: SetExpr::Matching { zone, filter },
            } => {
                let count = match zone {
                    ZoneExpr::Battlefield | ZoneExpr::Graveyards => {
                        self.kernel_set(frame, zone, filter)?.0.len()
                    }
                    ZoneExpr::Graveyard { of } if of.is_you() => {
                        let owner = self.entry(frame.stack_id)?.controller;
                        self.kernel_set(frame, &ZoneExpr::Graveyards, filter)?
                            .0
                            .iter()
                            .filter(|object| self.state.rules.cards[&object.card].owner == owner)
                            .count()
                    }
                    ZoneExpr::Hand { of } if of.is_you() && filter.0.is_empty() => self
                        .action_hand(self.entry(frame.stack_id)?.controller)
                        .len(),
                    _ => return Err("kernel count set has no audited evaluation".into()),
                };
                Ok(Quantity::from(
                    i64::try_from(count).map_err(|_| "kernel count exceeds object limit")?,
                ))
            }
            // CR 608.2h: the greatest current power or toughness (CR 613.4)
            // or mana value (CR 202.3) among the matching permanents. Zero
            // when none match, and never below zero (CR 107.1b).
            Amount::Greatest {
                set:
                    SetExpr::Matching {
                        zone: zone @ ZoneExpr::Battlefield,
                        filter,
                    },
                stat,
            } => {
                let mut greatest = Quantity::zero();
                for object in self.kernel_set(frame, zone, filter)?.0 {
                    let c = self.characteristics(object)?;
                    let value = match stat {
                        mtg_contracts::kir::Stat::Power => c.power.unwrap_or_default(),
                        mtg_contracts::kir::Stat::Toughness => c.toughness.unwrap_or_default(),
                        mtg_contracts::kir::Stat::ManaValue => c.printed_mana_value(),
                    };
                    greatest = greatest.max(value);
                }
                Ok(greatest)
            }
            // CR 122.1, 608.2h: the source's counters now, or as it last
            // existed on the battlefield.
            Amount::CountersOn {
                object: Ref::Source,
                counter,
            } => {
                let facts = self.source_facts(self.entry(frame.stack_id)?)?;
                Ok(
                    if *counter == mtg_contracts::kir::CounterKind::PlusOnePlusOne {
                        facts.plus_one_counters
                    } else {
                        facts
                            .counters
                            .iter()
                            .find(|stack| stack.kind == *counter)
                            .map_or_else(Quantity::zero, |stack| stack.count.clone())
                    },
                )
            }
            Amount::Times { factor, of } => {
                let mut value = self.kernel_amount(frame, of)?;
                value.multiply(factor);
                Ok(value)
            }
            // CR 700.5: the controller's devotion as the verb reads it.
            Amount::Devotion { player, colors } if player.is_you() => {
                self.devotion(self.entry(frame.stack_id)?.controller, colors)
            }
            Amount::OtherCastsManaValue { caster } if caster.is_you() => {
                self.other_casts_mana_value(self.entry(frame.stack_id)?)
            }
            // CR 509.1g, 506.4, 702.23a-b: the creatures still blocking the
            // exact source now, read once; a blocker removed from combat or
            // phased out no longer blocks it (CR 702.26b).
            Amount::BlockersOf {
                object: Ref::Source,
                beyond,
            } => {
                let source = self.entry(frame.stack_id)?.source;
                let blocking = self
                    .state
                    .rules
                    .combat
                    .iter()
                    .find(|a| a.attacker == source)
                    .map_or(0, |a| {
                        a.blockers
                            .iter()
                            .filter(|b| {
                                self.state.rules.blocking_creatures.contains(b)
                                    && self
                                        .current(**b)
                                        .is_ok_and(|o| o.id == **b && o.present_on_battlefield())
                            })
                            .count()
                    });
                let count = blocking.saturating_sub(*beyond as usize);
                Ok(Quantity::from(
                    i64::try_from(count).map_err(|_| "blocker count limit")?,
                ))
            }
            // CR 702.121a, 508.6: count opponents attacked directly by the
            // controller's declared attackers this combat.
            Amount::OpponentsAttacked {} => {
                let controller = self.entry(frame.stack_id)?.controller;
                let count = if self.state.rules.active == Some(controller) {
                    self.state
                        .rules
                        .combat_defenders
                        .iter()
                        .filter(|p| **p != controller && self.is_live(**p))
                        .count()
                } else {
                    0
                };
                Ok(Quantity::from(
                    i64::try_from(count).map_err(|_| "defender count limit")?,
                ))
            }
            Amount::StatOf {
                object: Ref::Target { slot },
                stat: mtg_contracts::kir::Stat::ManaValue,
            } => Ok(self.kernel_target_facts(frame, *slot)?.mana_value().clone()),
            Amount::StatOf {
                object: Ref::Target { slot },
                stat: mtg_contracts::kir::Stat::Power,
            } => self
                .kernel_target_facts(frame, *slot)?
                .power()
                .cloned()
                .map(|power| power.max(Quantity::zero()))
                .ok_or_else(|| "measured target has no power".into()),
            _ => Err("kernel amount has no audited evaluation".into()),
        }
    }
}

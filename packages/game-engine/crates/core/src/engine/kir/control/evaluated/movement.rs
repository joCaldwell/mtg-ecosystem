//! CR 400.3, 400.7, 608.2b-c, 608.2h, 903.9b: an evaluated zone change.
//! The members are read once as the step is performed; each goes to its
//! owner's zone, or enters under the mover's control (CR 110.2a). A
//! commander bound for a hand or library waits for its owner's choice, and
//! a card entering the battlefield for its entry questions, before the
//! members move in one event.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::Destination;

impl RulesTxn {
    /// The destination zone of `object` for this step (CR 400.3).
    fn move_zone(&self, object: ObjectId, to: &Destination) -> Result<Zone, String> {
        let owner = self.state.rules.cards[&object.card].owner;
        Ok(match to {
            Destination::Exile => Zone::Exile,
            Destination::OwnerHand => Zone::Hand { owner },
            Destination::OwnerLibraryTop
            | Destination::OwnerLibraryBottom
            | Destination::OwnerLibraryShuffled => Zone::Library { owner },
            Destination::OwnerGraveyard => Zone::Graveyard { owner },
            Destination::Battlefield { .. } => Zone::Battlefield,
            Destination::OneTappedRestOwnerHand => {
                return Err("search destination outside a search".into());
            }
        })
    }

    /// The incarnation `object` still is, in the zone it was read from: a
    /// phased-out permanent is treated as though it does not exist (CR
    /// 702.26b) and an object that changed zones is a new one (CR 400.7).
    fn move_member_present(&self, object: ObjectId, graveyard: bool) -> bool {
        self.current(object).is_ok_and(|o| {
            o.id == object
                && if graveyard {
                    matches!(o.zone, Zone::Graveyard { .. })
                } else {
                    o.present_on_battlefield()
                }
        })
    }

    /// Whether the step's members are graveyard cards rather than
    /// permanents, by the audited `what`.
    fn moves_graveyard_cards(&self, what: &SetExpr) -> Result<bool, String> {
        Ok(match what {
            SetExpr::One {
                object: Ref::Target { slot },
            } => {
                let entry = self.entry(self.kernel_root()?.stack_id)?;
                matches!(
                    self.program_for(entry)?
                        .targets
                        .get(*slot as usize)
                        .ok_or("missing move target")?
                        .domain,
                    TargetDomain::OwnGraveyardCard
                        | TargetDomain::OwnGraveyardCreatureCard
                        | TargetDomain::GraveyardCard { .. }
                )
            }
            SetExpr::Matching { zone, .. } => {
                matches!(zone, ZoneExpr::Graveyards | ZoneExpr::Graveyard { .. })
            }
            _ => false,
        })
    }

    /// CR 400.7, 608.2b-c, 608.2h: the live incarnations `what` names as
    /// the step is performed, in object order. An illegal or unchosen
    /// target names nothing; a graveyard set never includes a token
    /// (CR 111.7).
    pub(in crate::engine::kir::control) fn move_members(
        &self,
        task: &Task,
        what: &SetExpr,
    ) -> Result<Resolved<Vec<ObjectId>>, String> {
        let graveyard = self.moves_graveyard_cards(what)?;
        let frame = self.kernel_root()?;
        let objects = match what {
            SetExpr::One { object } => match self.evaluated_object(task, object)? {
                Resolved::Present(id) => vec![id],
                Resolved::Absent(reason) => return Ok(Resolved::Absent(reason)),
            },
            SetExpr::Matching {
                zone: zone @ ZoneExpr::Battlefield,
                filter,
            } => self.evaluated_set(task, zone, filter)?,
            SetExpr::Matching {
                zone: zone @ ZoneExpr::Graveyards,
                filter,
            } => self.kernel_set(frame, zone, filter)?.0,
            SetExpr::Matching {
                zone:
                    ZoneExpr::Graveyard {
                        of: player @ PlayerExpr::Is { .. },
                    },
                filter,
            } if filter.0.is_empty() => {
                let Resolved::Present(players) = self.evaluated_players(task, player)? else {
                    return Ok(Resolved::Absent(AbsentReason::IllegalTarget));
                };
                self.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| {
                        players
                            .iter()
                            .any(|p| o.zone == Zone::Graveyard { owner: *p })
                    })
                    .map(|o| o.id)
                    .collect()
            }
            _ => return Err("move set has no audited evaluation".into()),
        };
        Ok(Resolved::Present(
            objects
                .into_iter()
                .filter(|id| {
                    !(graveyard && self.state.rules.tokens.contains_key(&id.card))
                        && self.move_member_present(*id, graveyard)
                })
                .collect(),
        ))
    }

    /// The commanders among `objects` bound for a hand or library, by owner
    /// in APNAP order, each owner's in object order (CR 101.4, 903.9b).
    fn commander_moves(&self, objects: &[ObjectId], to: &Destination) -> Vec<(PlayerId, ObjectId)> {
        if !matches!(
            to,
            Destination::OwnerHand
                | Destination::OwnerLibraryTop
                | Destination::OwnerLibraryBottom
                | Destination::OwnerLibraryShuffled
        ) {
            return vec![];
        }
        self.apnap_seats()
            .flat_map(|owner| {
                objects
                    .iter()
                    .filter(move |o| {
                        self.state.rules.cards[&o.card].owner == owner
                            && self.state.rules.commanders.history(o.card).is_some()
                    })
                    .map(move |o| (owner, *o))
            })
            .filter(|(owner, _)| self.is_live(*owner))
            .collect()
    }

    /// The members that enter under `player`'s own entry questions: the
    /// cards `player` owns (CR 110.2a). A card another player owns enters
    /// under `player`'s control without them.
    fn own_entrants(&self, objects: &[ObjectId], player: PlayerId) -> Vec<ObjectId> {
        objects
            .iter()
            .copied()
            .filter(|o| self.state.rules.cards[&o.card].owner == player)
            .collect()
    }

    pub(super) fn begin_move(
        &mut self,
        mut task: Task,
        what: &SetExpr,
        to: &Destination,
        block: u32,
    ) -> Result<(), String> {
        let frame = self.kernel_root()?;
        let entry = self.entry(frame.stack_id)?;
        let objects = match self.move_members(&task, what)? {
            Resolved::Present(objects) => objects,
            Resolved::Absent(_) => {
                let outcome = match what {
                    SetExpr::One {
                        object: Ref::Target { slot },
                    } => match self.bound_object_target(entry, *slot as usize) {
                        Some(target) => InstructionOutcome::IllegalTarget { target: *target },
                        None => InstructionOutcome::Effect { actual: None },
                    },
                    _ => InstructionOutcome::Effect { actual: None },
                };
                return self.complete_evaluated(&task, None, [outcome]);
            }
        };
        if let Destination::Battlefield { .. } = to {
            // CR 110.2a, 800.4a: the mover is the invocation's controller;
            // with that player gone nothing enters.
            let player = entry.controller;
            if !self.is_live(player) {
                return self.complete_evaluated(
                    &task,
                    None,
                    [InstructionOutcome::PlayerActionSkipped { player }],
                );
            }
            let own = self.own_entrants(&objects, player);
            if self
                .next_entry_question(player, &own, &[], &[], &[])?
                .is_none()
            {
                return self.commit_moves(&task, &objects, &[], &[], &[], &[], block);
            }
            task.stage = Stage::Step(StepProgress::Moves {
                objects,
                remaining: vec![],
                choices: vec![],
                controller: Some(player),
                hosts: vec![],
                paid: vec![],
                revealed: vec![],
            });
            self.install_kernel(task);
            return Ok(());
        }
        let remaining = self.commander_moves(&objects, to);
        if remaining.is_empty() {
            return self.commit_moves(&task, &objects, &[], &[], &[], &[], block);
        }
        task.stage = Stage::Step(StepProgress::Moves {
            objects,
            remaining,
            choices: vec![],
            controller: None,
            hosts: vec![],
            paid: vec![],
            revealed: vec![],
        });
        self.install_kernel(task);
        Ok(())
    }

    /// The question a suspended move asks next: a commander owner's choice
    /// (CR 903.9b) or one entry question (CR 303.4f, 614.12).
    pub(in crate::engine::kir::control) fn move_request(
        &self,
        task: &Task,
    ) -> Result<Option<Decision<Rules>>, String> {
        let Stage::Step(StepProgress::Moves {
            objects,
            remaining,
            choices,
            controller,
            hosts,
            paid,
            revealed,
        }) = &task.stage
        else {
            return Ok(None);
        };
        if let Some(&(owner, object)) = remaining.first() {
            let Some(Verb::Move { to, .. }) = self.node_at(task)?.0.verb() else {
                return Err("missing admitted move".into());
            };
            if !self.is_live(owner) || !self.current(object).is_ok_and(|o| o.id == object) {
                return Ok(None);
            }
            let destination = self.move_zone(object, to)?;
            return Ok(Some(self.commander_decision(
                object,
                destination,
                choices.clone(),
            )));
        }
        let Some(player) = controller.filter(|p| self.is_live(*p)) else {
            return Ok(None);
        };
        let own = self.own_entrants(objects, player);
        Ok(self
            .next_entry_question(player, &own, hosts, paid, revealed)?
            .map(|question| self.entry_question_decision(player, question)))
    }

    /// Accept the answer to the open question, then ask the next one or
    /// perform the move.
    pub(in crate::engine::kir::control) fn step_moves(
        &mut self,
        mut task: Task,
    ) -> Result<(), String> {
        let (_, block) = self.node_at(&task)?;
        let Stage::Step(StepProgress::Moves {
            objects,
            mut remaining,
            mut choices,
            controller,
            mut hosts,
            mut paid,
            mut revealed,
        }) = task.stage.clone()
        else {
            return Err("missing move progress".into());
        };
        let asked = self.move_request(&task)?.is_some();
        let input = self.rules_mut().pending.take();
        if let Some(&(owner, object)) = remaining.first() {
            // CR 903.9b: a departed owner or commander is not asked.
            if asked {
                let input = input.ok_or("missing commander return choice")?;
                if input.player != owner {
                    return Err("commander return owner invariant".into());
                }
                choices.push((object, input.flag(Flag::CommandZone)?));
            }
            remaining.remove(0);
            // CR 800.4a: a departed owner's cards have left the game.
            while let Some(&(owner, object)) = remaining.first() {
                if self.is_live(owner) && self.current(object).is_ok_and(|o| o.id == object) {
                    break;
                }
                remaining.remove(0);
            }
        } else if let Some(player) = controller {
            let own = self.own_entrants(&objects, player);
            if asked
                && let Some(question) =
                    self.next_entry_question(player, &own, &hosts, &paid, &revealed)?
            {
                let input = input.ok_or("missing entry answer")?;
                if input.player != player {
                    return Err("entry answer actor invariant".into());
                }
                Self::answer_entry_question(
                    question,
                    &input,
                    &mut hosts,
                    &mut paid,
                    &mut revealed,
                )?;
            }
        } else {
            return Err("move progress has nothing to ask".into());
        }
        task.stage = Stage::Step(StepProgress::Moves {
            objects: objects.clone(),
            remaining: remaining.clone(),
            choices: choices.clone(),
            controller,
            hosts: hosts.clone(),
            paid: paid.clone(),
            revealed: revealed.clone(),
        });
        if self.move_request(&task)?.is_some() {
            self.install_kernel(task);
            return Ok(());
        }
        self.commit_moves(&task, &objects, &choices, &hosts, &paid, &revealed, block)
    }

    /// CR 101.4, 400.3, 608.2c: the members still present move together
    /// to their destinations, as chosen, in one event.
    #[allow(clippy::too_many_arguments)]
    fn commit_moves(
        &mut self,
        task: &Task,
        objects: &[ObjectId],
        choices: &[(ObjectId, bool)],
        hosts: &[(ObjectId, ObjectId)],
        paid: &[(ObjectId, bool)],
        revealed: &[(ObjectId, Option<ObjectId>)],
        block: u32,
    ) -> Result<(), String> {
        let (node, _) = self.node_at(task)?;
        let Some(Verb::Move { what, to }) = node.verb().cloned() else {
            return Err("missing admitted move".into());
        };
        let graveyard = self.moves_graveyard_cards(&what)?;
        let live: Vec<ObjectId> = objects
            .iter()
            .copied()
            .filter(|o| {
                self.move_member_present(*o, graveyard)
                    && self.is_live(self.state.rules.cards[&o.card].owner)
            })
            .collect();
        let cause = self.instruction_cause(self.kernel_root()?, block)?;
        let mut controllers = BTreeMap::new();
        for object in &live {
            controllers.insert(*object, self.current(*object)?.controller);
        }
        let intended: BTreeMap<ObjectId, Zone> = live
            .iter()
            .map(|o| Ok((*o, self.move_zone(*o, &to)?)))
            .collect::<Result<_, String>>()?;
        let batch =
            if let Destination::Battlefield { tapped, .. } = &to {
                let player = self.entry(self.kernel_root()?.stack_id)?.controller;
                let own: Vec<_> = self.own_entrants(&live, player);
                let mut entering = self.search_entrants(&own, hosts)?;
                for object in &live {
                    if !own.contains(object) && self.may_enter_battlefield(object.card)? {
                        entering.push((*object, None));
                    }
                }
                entering.sort();
                if entering.is_empty() {
                    None
                } else {
                    Some(self.commit_entry_group(
                        cause,
                        player,
                        &entering,
                        *tapped,
                        Some(player),
                        paid,
                        revealed,
                        vec![],
                    )?)
                }
            } else {
                let mut occurrences = Vec::with_capacity(live.len());
                for object in &live {
                    let owner = self.state.rules.cards[&object.card].owner;
                    occurrences.push(if choices.contains(&(*object, true)) {
                        ProposedEvent::Move {
                            object: *object,
                            destination: Zone::Command { owner },
                        }
                    } else if to == Destination::OwnerLibraryTop {
                        ProposedEvent::PutOnTopOwnerLibrary { object: *object }
                    } else {
                        ProposedEvent::Move {
                            object: *object,
                            destination: intended[object],
                        }
                    });
                }
                let mut group = match occurrences.len() {
                    0 => None,
                    1 => Some(EventGroup::Occurrence {
                        proposed: occurrences.remove(0),
                    }),
                    _ => Some(EventGroup::Simultaneous { occurrences }),
                };
                // CR 701.24c: the object's owner shuffles even when the object
                // went elsewhere or had already left that zone.
                if to == Destination::OwnerLibraryShuffled {
                    let mut children: Vec<_> = group.into_iter().collect();
                    let mut owners = BTreeSet::new();
                    for object in objects {
                        owners.insert(self.state.rules.cards[&object.card].owner);
                    }
                    if let SetExpr::One {
                        object: Ref::Target { slot },
                    } = &what
                        && objects.is_empty()
                        && self.kernel_root()?.legal_targets.get(*slot as usize) == Some(&true)
                    {
                        let entry = self.entry(self.kernel_root()?.stack_id)?;
                        let target = *self
                            .bound_object_target(entry, *slot as usize)
                            .ok_or("missing move target")?;
                        owners.insert(self.state.rules.cards[&target.card].owner);
                    }
                    children.extend(owners.into_iter().filter(|p| self.is_live(*p)).map(
                        |player| EventGroup::Occurrence {
                            proposed: ProposedEvent::ShuffleLibrary { player },
                        },
                    ));
                    group = match children.len() {
                        0 => None,
                        1 => children.pop(),
                        _ => Some(EventGroup::Ordered { children }),
                    };
                }
                group
                    .map(|group| self.commit_batch(cause, group))
                    .transpose()?
            };
        let mut result = StepResult {
            amount: Some(Quantity::zero()),
            ..StepResult::default()
        };
        let mut movements = Vec::new();
        for receipt in batch.iter().flat_map(|batch| &batch.occurrences) {
            let movement = match &receipt.actual {
                EventOutcome::Moved { movement }
                | EventOutcome::LibraryTopMoved { movement, .. }
                | EventOutcome::EnteredAttached { movement, .. } => movement,
                _ => continue,
            };
            movements.push(movement.clone());
            // CR 608.2c: the actual result after replacement: an object
            // that went elsewhere was not moved "this way".
            let reached = intended.get(&movement.previous).is_some_and(|zone| {
                *zone == movement.destination
                    || (matches!(zone, Zone::Battlefield)
                        && movement.destination == Zone::Battlefield)
            });
            if !reached {
                continue;
            }
            if !result.events.contains(&receipt.id) {
                result.events.push(receipt.id.clone());
            }
            let controller = *controllers
                .get(&movement.previous)
                .ok_or("moved object outside its step")?;
            let facts = self.public_move_characteristics(movement)?;
            result.record_move(controller, movement, facts);
        }
        let outcome = match (&what, movements.as_slice()) {
            (SetExpr::One { .. }, [movement]) => InstructionOutcome::Moved {
                movement: movement.clone(),
            },
            (SetExpr::One { object }, []) => match self.evaluated_object(task, object)? {
                Resolved::Present(target) => InstructionOutcome::ObjectNoLongerPresent { target },
                Resolved::Absent(_) => InstructionOutcome::Effect { actual: None },
            },
            _ => InstructionOutcome::SetMoved {
                movements,
                destroyed_creatures: 0,
            },
        };
        self.complete_evaluated(task, Some(result), [outcome])
    }

    /// Whether performing the move could change a zone now (CR 608.2d).
    pub(in crate::engine::kir::control) fn move_performable(
        &self,
        task: &Task,
        what: &SetExpr,
        to: &Destination,
    ) -> Result<bool, String> {
        let Resolved::Present(objects) = self.move_members(task, what)? else {
            return Ok(false);
        };
        if !matches!(to, Destination::Battlefield { .. }) {
            return Ok(!objects.is_empty());
        }
        if !self.is_live(self.entry(self.kernel_root()?.stack_id)?.controller) {
            return Ok(false);
        }
        for object in objects {
            if self.may_enter_battlefield(object.card)?
                && self
                    .entry_hosts(object)?
                    .is_none_or(|hosts| !hosts.is_empty())
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A suspended move holds the members it read and the answers given so
    /// far, in the order the rules ask them.
    pub(in crate::engine::kir::control) fn validate_moves(
        &self,
        task: &Task,
        progress: &StepProgress,
    ) -> Result<(), String> {
        let StepProgress::Moves {
            objects,
            remaining,
            choices,
            controller,
            hosts,
            paid,
            revealed,
        } = progress
        else {
            return Err("missing move progress".into());
        };
        let (node, _) = self.node_at(task)?;
        let Some(Verb::Move { what, to }) = node.verb() else {
            return Err("move progress outside a move".into());
        };
        let entry = self.entry(self.kernel_root()?.stack_id)?;
        let order: Vec<_> = self.apnap_seats().collect();
        let position = |p: &PlayerId| order.iter().position(|q| q == p);
        let named = |object: &ObjectId| match what {
            SetExpr::One {
                object: Ref::Source,
            } => *object == entry.source,
            SetExpr::One {
                object: Ref::Target { slot },
            } => {
                self.kernel_root()
                    .is_ok_and(|f| f.legal_targets.get(*slot as usize) == Some(&true))
                    && self.bound_object_target(entry, *slot as usize) == Some(object)
            }
            _ => true,
        };
        let battlefield = matches!(to, Destination::Battlefield { .. });
        if objects.is_empty()
            || objects.len() > self.event_group_limit()
            || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
            || objects
                .iter()
                .any(|o| !self.state.rules.cards.contains_key(&o.card) || !named(o))
            || remaining.iter().any(|(owner, object)| {
                !objects.contains(object)
                    || self.state.rules.cards[&object.card].owner != *owner
                    || self.state.rules.commanders.history(object.card).is_none()
                    || choices.iter().any(|(chosen, _)| chosen == object)
            })
            || remaining
                .windows(2)
                .any(|w| position(&w[0].0) > position(&w[1].0))
            || choices.iter().any(|(object, _)| {
                !objects.contains(object)
                    || self.state.rules.commanders.history(object.card).is_none()
            })
            || choices
                .iter()
                .map(|(o, _)| o)
                .collect::<BTreeSet<_>>()
                .len()
                != choices.len()
            || battlefield != controller.is_some()
            || controller.is_some_and(|p| p != entry.controller)
            || (battlefield && !(remaining.is_empty() && choices.is_empty()))
            || (!battlefield
                && (remaining.is_empty()
                    || !hosts.is_empty()
                    || !paid.is_empty()
                    || !revealed.is_empty()))
        {
            return Err("invalid move progress".into());
        }
        if let Some(player) = controller
            && self.is_live(*player)
        {
            let own: Vec<_> = self
                .own_entrants(objects, *player)
                .into_iter()
                .filter(|o| self.current(*o).is_ok_and(|c| c.id == *o))
                .collect();
            self.next_entry_question(*player, &own, hosts, paid, revealed)?;
        }
        Ok(())
    }
}

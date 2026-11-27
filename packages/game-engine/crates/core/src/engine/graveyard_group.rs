//! CR 614.4–6, 616.1/1f, 701.24c/f, 903.9b. Resolve choices against the
//! common prior. Sample into a draft permutation one word per budget unit;
//! publish movement and the resulting shuffles together at the batch boundary.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "completion", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupFinish {
    Instruction {
        frame: Resolving,
        result: GroupResult,
    },
    Cleanup,
    Payment {
        resume: FrameLink,
    },
    StateBased {
        resume: FrameLink,
        checked: Vec<ObjectId>,
        kept: Vec<ObjectId>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupResult {
    Effect,
    ActualEffect,
    DestroySet,
    Sacrifice { player: PlayerId },
    Discard { player: PlayerId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    object: ObjectId,
    owner: PlayerId,
    affected: PlayerId,
    printed: Option<bool>,
    commander: bool,
    command: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Shuffle {
    stream: u64,
    owner: PlayerId,
    order: Vec<usize>,
    index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Randomization {
    ordinals: [u64; 4],
    random: MatchRandom,
    shuffles: Vec<Shuffle>,
    cursor: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraveyardGroup {
    pub(super) finish: GroupFinish,
    cause: BatchCause,
    group: EventGroup,
    members: Vec<Member>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) shields: Vec<regeneration::ShieldMember>,
    owed: Vec<PlayerId>,
    draft: Option<Randomization>,
    #[serde(skip)]
    publishing_shuffle: Option<usize>,
}

impl GroupFinish {
    pub(super) fn frame_mut(&mut self) -> Option<&mut Resolving> {
        match self {
            Self::Instruction { frame, .. } => Some(frame),
            _ => None,
        }
    }
    fn complete(self, game: &mut RulesTxn, batch: Option<&CommittedBatch>) -> Result<(), String> {
        let occurrences = batch.map_or(&[][..], |b| b.occurrences.as_slice());
        let single = batch.and_then(|b| b.result(&[]));
        match self {
            Self::Payment { .. } => {
                return Err("payment completion belongs to its announcement".into());
            }
            Self::Cleanup => game.rules_mut().frame = Frame::CleanupClear,
            Self::StateBased {
                resume, checked, ..
            } => {
                game.consume_checked_deathtouch(&checked)?;
                if matches!(game.state.rules.frame, Frame::GraveyardGroup { .. }) {
                    game.rules_mut().frame = game.saved_frame(resume)?.clone();
                }
                if let Some(batch) = batch
                    && game.finish_committed_player_losses(batch)?
                {
                    return Ok(());
                }
                game.rules_mut().frame = game.saved_frame(resume)?.clone();
            }
            Self::Instruction { frame, result } => {
                let outcome = match result {
                    GroupResult::DestroySet => {
                        let mut movements = vec![];
                        let mut destroyed_creatures = 0;
                        for r in occurrences {
                            if let EventOutcome::Moved { movement } = &r.actual {
                                if r.before.objects.iter().any(|o| {
                                    o.object == movement.previous
                                        && o.characteristics.has_type(CardKind::Creature)
                                }) {
                                    destroyed_creatures += 1;
                                }
                                movements.push(movement.clone());
                            }
                        }
                        InstructionOutcome::SetMoved {
                            movements,
                            destroyed_creatures,
                        }
                    }
                    GroupResult::Discard { player } => {
                        let mut movements = vec![];
                        for r in occurrences {
                            let EventOutcome::CardDiscarded { movement, .. } = &r.actual else {
                                return Err("unexpected discard result".into());
                            };
                            movements.push(movement.clone());
                        }
                        InstructionOutcome::Discarded { player, movements }
                    }
                    GroupResult::Sacrifice { player } => {
                        let movement = match single {
                            Some(EventOutcome::Sacrificed { movement, .. }) => {
                                Some(movement.clone())
                            }
                            None if occurrences.is_empty() => None,
                            _ => return Err("unexpected sacrifice result".into()),
                        };
                        if movement.is_some() {
                            let entry = game.entry(frame.stack_id)?;
                            if let Some(Instruction::Kernel {
                                node: mtg_contracts::kir::Node::SacrificeForTappedReturn { .. },
                                source_block,
                            }) = game
                                .program_for(entry)?
                                .instructions
                                .get(frame.cursor as usize)
                            {
                                return game.return_announced_creatures(frame, *source_block);
                            }
                        }
                        InstructionOutcome::Sacrificed { player, movement }
                    }
                    GroupResult::ActualEffect => InstructionOutcome::Effect {
                        actual: single.cloned(),
                    },
                    GroupResult::Effect => InstructionOutcome::Effect { actual: None },
                };
                return game.finish_instruction(frame, outcome);
            }
        }
        Ok(())
    }
}

impl GraveyardGroup {
    pub(super) fn leaves(group: &EventGroup) -> Result<&[ProposedEvent], String> {
        match group {
            EventGroup::Occurrence { proposed } => Ok(std::slice::from_ref(proposed)),
            EventGroup::Simultaneous { occurrences } => Ok(occurrences),
            EventGroup::Ordered { .. } => Err("replacement wave must have one common prior".into()),
        }
    }

    pub(super) fn is_publication(&self, cause: &BatchCause, group: &EventGroup) -> bool {
        cause == &self.cause && group == &self.group && self.publishing_shuffle.is_none()
    }

    fn uses_shuffle_transaction(&self) -> bool {
        !self.owed.is_empty() || !self.members.is_empty()
    }

    fn next(&self, game: &RulesTxn) -> Option<Decision<Rules>> {
        for player in game.apnap_seats() {
            if let Some(m) = self
                .members
                .iter()
                .find(|m| m.affected == player && m.printed.is_none())
            {
                return Some(game.pick_flags(
                    player,
                    Reason::GraveyardReplacementOrder {
                        object: m.object,
                        offered: None,
                    },
                    &[Flag::Accept, Flag::Decline],
                ));
            }
            if let Some(m) = self
                .shields
                .iter()
                .find(|m| m.affected == player && m.regenerate.is_none())
            {
                return Some(game.pick_flags(
                    player,
                    Reason::GraveyardReplacementOrder {
                        object: m.object,
                        offered: Some(GraveyardOrderOption::Regenerate),
                    },
                    &[Flag::Accept, Flag::Decline],
                ));
            }
        }
        for player in game.apnap_seats() {
            if let Some(m) = self.members.iter().find(|m| {
                m.owner == player && m.commander && m.printed == Some(true) && m.command.is_none()
            }) {
                return Some(game.commander_decision(
                    m.object,
                    Zone::Library { owner: player },
                    self.prior_choices(game),
                ));
            }
        }
        None
    }

    pub(super) fn prior_choices(&self, game: &RulesTxn) -> Vec<(ObjectId, bool)> {
        game.apnap_seats()
            .flat_map(|owner| {
                self.members
                    .iter()
                    .filter(move |member| member.owner == owner)
                    .filter_map(|member| member.command.map(|choice| (member.object, choice)))
            })
            .collect()
    }

    pub(super) fn reveals(&self, object: ObjectId) -> bool {
        self.members
            .iter()
            .any(|m| m.object == object && m.printed == Some(true))
    }

    fn destinations(&self) -> Result<BTreeMap<ObjectId, Zone>, String> {
        self.members
            .iter()
            .map(|m| {
                let destination = match (m.printed, m.commander, m.command) {
                    (Some(false), _, None) => Zone::Exile,
                    (Some(true), true, Some(true)) => Zone::Command { owner: m.owner },
                    (Some(true), false, None) | (Some(true), true, Some(false)) => {
                        Zone::Library { owner: m.owner }
                    }
                    _ => return Err("unresolved graveyard replacement order".into()),
                };
                Ok((m.object, destination))
            })
            .collect()
    }

    fn randomization(&self, game: &RulesTxn) -> Result<Randomization, String> {
        let destinations = self.destinations()?;
        let state = game
            .state
            .rules
            .match_state
            .as_ref()
            .ok_or("replacement requires seeded randomness")?;
        let mut ordinals = state.library_shuffles;
        let mut shuffles = vec![];
        for owner in self
            .members
            .iter()
            .filter(|m| m.printed == Some(true))
            .map(|m| m.owner)
            .chain(self.owed.iter().copied())
        {
            let current = game.state.rules.libraries.get(&owner).map_or(0, Vec::len);
            let added = destinations
                .values()
                .filter(|zone| **zone == (Zone::Library { owner }))
                .count();
            let ceased = Self::leaves(&self.group)?
                .iter()
                .filter(|event| {
                    matches!(event,
                ProposedEvent::CeaseToken { object } | ProposedEvent::CeaseSpellCopy { object }
                if game.zone(*object) == Some(Zone::Library {owner}))
                })
                .count();
            let len = current
                .checked_sub(ceased)
                .and_then(|remaining| remaining.checked_add(added))
                .ok_or("replacement library size overflow")?;
            if len > capacities::OWNED_LIMIT + capacities::TOKEN_CAPACITY as usize {
                return Err("replacement library exceeds the physical card and token bound".into());
            }
            let ordinal = &mut ordinals[owner.0 as usize];
            let stream = setup::stream_id(setup::LIBRARY_SHUFFLE, owner, *ordinal)?;
            *ordinal += 1;
            shuffles.push(Shuffle {
                stream,
                owner,
                order: (0..len).collect(),
                index: len.saturating_sub(1),
            });
        }
        Ok(Randomization {
            ordinals: state.library_shuffles,
            random: state.random.clone(),
            shuffles,
            cursor: 0,
        })
    }

    fn object(event: &ProposedEvent) -> Option<ObjectId> {
        match event {
            ProposedEvent::Sacrifice { object, .. }
            | ProposedEvent::DiscardCard { object, .. }
            | ProposedEvent::Move { object, .. }
            | ProposedEvent::DestroyPermanent { object }
            | ProposedEvent::DestroyCreature { object }
            | ProposedEvent::StateBasedGraveyard { object, .. }
            | ProposedEvent::CeaseToken { object }
            | ProposedEvent::CeaseSpellCopy { object }
            | ProposedEvent::Attach { object, .. } => Some(*object),
            _ => None,
        }
    }
    pub(super) fn validate(&self, game: &RulesTxn) -> Result<(), String> {
        let leaves = Self::leaves(&self.group)?;
        let limit = game.event_group_limit();
        if self.publishing_shuffle.is_some()
            || leaves.len() > limit
            || self.members.len().saturating_add(self.owed.len()) > limit
            || self.owed.iter().any(|p| p.0 >= 4 || !game.is_live(*p))
        {
            return Err("graveyard wave retention bounds".into());
        }
        let mut seen = BTreeSet::new();
        for event in leaves {
            if let Some(object) = Self::object(event)
                && !seen.insert(object)
            {
                return Err("duplicate object in retained wave".into());
            }
        }
        // No admitted printed graveyard replacement produces wave members.
        if !self.members.is_empty() {
            return Err("graveyard replacement member invariant".into());
        }
        let expected_shields = game.regeneration_members(&self.group, &self.finish)?;
        if self.shields.len() != expected_shields.len()
            || self.shields.iter().zip(expected_shields).any(|(m, c)| {
                m.object != c.object
                    || m.affected != c.affected
                    || m.shield != c.shield
                    || c.regenerate.is_some() && m.regenerate != c.regenerate
            })
        {
            return Err("regeneration wave member invariant".into());
        }
        let producing = match &self.finish {
            GroupFinish::Instruction { frame, result } => {
                game.validate_resolving(frame)?;
                let entry = game.entry(frame.stack_id)?;
                let i = game.program_for(entry)?.instructions.get(frame.cursor as usize).ok_or("missing retained instruction")?;
                let kind = match result {
                    GroupResult::Effect if matches!(i, kernel!(Sacrifice { player: you!(), count: fixed!(count), .. }) if *count > Quantity::from(1)) => true,
                    GroupResult::Effect => game.target_destroy_domain(entry, i)? == Some(TargetDomain::BattlefieldCreature) || matches!(i, kernel!(Discard { player: mtg_contracts::kir::PlayerExpr::EachPlayer | mtg_contracts::kir::PlayerExpr::EachOpponent, amount: fixed!(_), random: false }) | kernel!(Sacrifice { player: mtg_contracts::kir::PlayerExpr::EachPlayer, .. }) | kernel!(Sacrifice { player: mtg_contracts::kir::PlayerExpr::EachOpponent, .. })),
                    GroupResult::ActualEffect => game.target_destroy_domain(entry, i)? == Some(TargetDomain::BattlefieldPermanent),
                    GroupResult::DestroySet => matches!(i, kernel!(Destroy {what: mtg_contracts::kir::SetExpr::Matching {..}, ..}) | Instruction::Kernel { node: mtg_contracts::kir::Node::RemoveTargets { removal: mtg_contracts::kir::GroupRemoval::Destroy, .. }, .. }),
                    GroupResult::Discard { player } if matches!(i, kernel!(Discard {
                        player: you!(), amount: fixed!(_), random: true
                    })) => *player == entry.controller && matches!(entry.kind, StackKind::Triggered {
                        ability: EffectiveTriggerAbility::Followup { .. }, ..
                    }),
                    GroupResult::Discard {..} => matches!(i, kernel!(Discard { player: you!() | event_player!(), amount: fixed!(_), random: false }) | kernel!(Discard { player: target_player!(_), amount: fixed!(_), random: false }) | kernel!(Discard { player: target_player!(_), random: true, .. })),
                    GroupResult::Sacrifice {player} if matches!(i, Instruction::Kernel { node: mtg_contracts::kir::Node::SacrificeForTappedReturn { .. }, .. }) => *player == entry.controller,
                    GroupResult::Sacrifice {player} if matches!(i, kernel!(Sacrifice { player: you!(), .. })) => *player == entry.controller,
                    GroupResult::Sacrifice {..} => matches!(i, kernel!(Sacrifice { player: event_player!(), .. }) | kernel!(Sacrifice { player: target_player!(_), .. })),
                };
                kind && self.cause == game.instruction_cause(frame,i.source_block())? && leaves.iter().all(|e| match (i,e) {
                    (i,ProposedEvent::DestroyPermanent {object}) if i.target_destroy().is_some() => i.target_destroy().is_some_and(|(slot,..)| game.bound_object_target(entry,slot as usize) == Some(object) && frame.legal_targets.get(slot as usize) == Some(&true)),
                    (i,ProposedEvent::DestroyCreature {object}) if i.target_destroy().is_some() => i.target_destroy().is_some_and(|(slot,..)| entry.targets.get(slot as usize) == Some(object) && frame.legal_targets.get(slot as usize) == Some(&true)),
                    (kernel!(Destroy {what: mtg_contracts::kir::SetExpr::Matching {..}, ..}),ProposedEvent::DestroyPermanent {..}) => true,
                    // CR 608.2b: a group destroys only its legal announced members.
                    (Instruction::Kernel { node: mtg_contracts::kir::Node::RemoveTargets { removal: mtg_contracts::kir::GroupRemoval::Destroy, .. }, .. },ProposedEvent::DestroyPermanent {object}) => entry.targets.iter().zip(&frame.legal_targets).any(|(target, legal)| target == object && *legal),
                    (kernel!(Discard { player: mtg_contracts::kir::PlayerExpr::EachPlayer | mtg_contracts::kir::PlayerExpr::EachOpponent, amount: fixed!(_), random: false }),ProposedEvent::DiscardCard {..}) => true,
                    (_,ProposedEvent::DiscardCard {player,..}) => matches!(result, GroupResult::Discard {player:p} if p == player),
                    (kernel!(Sacrifice { player: mtg_contracts::kir::PlayerExpr::EachOpponent, .. })|kernel!(Sacrifice { player: mtg_contracts::kir::PlayerExpr::EachPlayer, .. }),ProposedEvent::Sacrifice {..}) => true,
                    (kernel!(Sacrifice { player: you!(), .. }), ProposedEvent::Sacrifice { player, .. }) => *player == entry.controller,
                    (_,ProposedEvent::Sacrifice {player,..}) => matches!(result, GroupResult::Sacrifice {player:p} if p == player),
                    _ => false,
                })
            }
            GroupFinish::Payment { resume } => {
                let Frame::SacrificeCost {cast} = game.saved_frame(*resume)? else {return Err("invalid retained cost kind".into());};
                let top = game.top()?;
                game.check_sacrifice_payment(top,cast)?;
                top.id == cast.stack_id && top.paid.is_none()
                    && self.cause == (BatchCause::Payment { stack_id:top.id, player:top.controller })
                    && Some(&self.group) == game.sacrifice_payment_group(top,&cast.sacrifices)?.as_ref()
            }
            GroupFinish::Cleanup => {
                self.cause == BatchCause::Cleanup && leaves.iter().all(|e| {
                    matches!(e, ProposedEvent::DiscardCard { object, player }
                        if *player == game.state.rules.turn_owner && game.zone(*object) == Some(Zone::Hand { owner: *player }))
                })
            }
            GroupFinish::StateBased {resume,checked,..} => self.cause == BatchCause::StateBased && matches!(game.saved_frame(*resume)?, Frame::Stabilize {changed:true,..}) && checked.windows(2).all(|w| w[0] < w[1]) && leaves.iter().all(|e| matches!(e, ProposedEvent::StateBasedGraveyard {..} | ProposedEvent::Move {destination: Zone::Graveyard {..},..} | ProposedEvent::CeaseToken {..} | ProposedEvent::CeaseSpellCopy {..} | ProposedEvent::Attach {target:None,..} | ProposedEvent::Lose {..})),
        };
        if !producing {
            return Err("graveyard wave producer mismatch".into());
        }
        if self.uses_shuffle_transaction() && game.state.rules.match_state.is_none() {
            return Err("missing graveyard replacement entropy".into());
        }
        if let Some(d) = &self.draft {
            let seeded = game
                .state
                .rules
                .match_state
                .as_ref()
                .ok_or("missing graveyard replacement entropy")?;
            let expected = self.randomization(game)?;
            if d.ordinals != seeded.library_shuffles
                || d.random.seed != seeded.random.seed
                || d.random.version != seeded.random.version
                || d.random.seat_random != seeded.random.seat_random
                || d.random.table != seeded.random.table
                || d.random.open.is_some_and(|p| {
                    Some(p) != seeded.random.open
                        && !d
                            .shuffles
                            .get(d.cursor)
                            .is_some_and(|s| s.stream == p.stream && p.word > 0)
                })
                || d.cursor > d.shuffles.len()
                || d.shuffles.len() != expected.shuffles.len()
            {
                return Err("graveyard shuffle draft stream invariant".into());
            }
            for (i, (s, e)) in d.shuffles.iter().zip(&expected.shuffles).enumerate() {
                if s.stream != e.stream
                    || s.owner != e.owner
                    || s.order.len() != e.order.len()
                    || s.index > e.index
                    || (i < d.cursor && s.index != 0)
                    || (i > d.cursor && s != e)
                    || s.order
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .ne(0..e.order.len())
                {
                    return Err("graveyard draft permutation invariant".into());
                }
            }
        }
        Ok(())
    }

    pub(super) fn authorize(
        &self,
        game: &RulesTxn,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<Option<BTreeMap<ObjectId, Zone>>, String> {
        if cause != &self.cause {
            return Ok(None);
        }
        if let Some(shuffle) = self
            .publishing_shuffle
            .and_then(|index| self.draft.as_ref()?.shuffles.get(index))
        {
            return if matches!(group, EventGroup::Occurrence { proposed: ProposedEvent::FinishLibraryShuffle { player }} if *player == shuffle.owner)
            {
                Ok(Some(BTreeMap::new()))
            } else {
                Err("replacement shuffle publication order".into())
            };
        }
        if matches!(
            (&self.finish, group),
            (
                GroupFinish::Payment { .. },
                EventGroup::Occurrence {
                    proposed: ProposedEvent::PayManaSelected { .. }
                }
            )
        ) {
            return Ok(None);
        }
        if group != &self.group
            || (self.uses_shuffle_transaction()
                && !self
                    .draft
                    .as_ref()
                    .is_some_and(|d| d.cursor == d.shuffles.len()))
        {
            return Err("graveyard wave origin or preparation mismatch".into());
        }
        if self.next(game).is_some() {
            return Err("outstanding graveyard choice".into());
        }
        if !self.shields.is_empty() {
            self.validate(game)?;
        }
        self.destinations().map(Some)
    }
}

impl RulesTxn {
    pub(super) fn complete_graveyard_group(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
        finish: GroupFinish,
    ) -> Result<(), String> {
        let leaves = GraveyardGroup::leaves(&group)?;
        if leaves.len() > self.event_group_limit() {
            return Err(self.capacity_error(RuleFault::EventGroupCapacityExceeded {
                requested: leaves.len(),
                capacity: self.event_group_limit(),
            }));
        }
        let shields = self.regeneration_members(&group, &finish)?;
        if shields.is_empty() {
            let batch = self.commit_batch(cause, group)?;
            return finish.complete(self, Some(&batch));
        }
        self.rules_mut().frame = Frame::GraveyardGroup {
            tx: Box::new(GraveyardGroup {
                finish,
                cause,
                group,
                members: vec![],
                shields,
                owed: vec![],
                draft: None,
                publishing_shuffle: None,
            }),
        };
        Ok(())
    }

    // Concession may interleave between any two paid work units (CR 104.3a,
    // 800.4a). The original unpaid cost is reversible. A committed choice to
    // shuffle for a resolving effect still has a live library under 701.24c.
    pub(super) fn revalidate_graveyard_departure(&mut self) -> Result<(), String> {
        let Frame::GraveyardGroup { tx } = &self.state.rules.frame else {
            return Ok(());
        };
        let mut tx = tx.clone();
        if let GroupFinish::Payment { resume } = &tx.finish {
            let Frame::SacrificeCost { cast } = self.saved_frame(*resume)? else {
                return Err("invalid replacement cost resume".into());
            };
            if self.top().is_err() || self.top()?.id != cast.stack_id {
                self.rules_mut().frame = self.saved_frame(*resume)?.clone();
                self.rules_mut().pending = None;
                return Ok(());
            }
            if self.check_sacrifice_payment(self.top()?, cast).is_err() {
                let mut cast = cast.clone();
                cast.sacrifices.clear();
                self.rules_mut().frame = Frame::SacrificeCost { cast };
                self.rules_mut().pending = None;
                return Ok(());
            }
        }
        let mut aura_sbas = vec![];
        let mut legend_moves = BTreeSet::new();
        if let GroupFinish::StateBased { kept, .. } = &tx.finish {
            self.attachment_sbas(&mut aura_sbas)?;
            for group in self.legend_groups()? {
                let kept_here = group.iter().filter(|o| kept.contains(o)).count();
                if kept_here == 1 {
                    legend_moves.extend(group.into_iter().filter(|o| !kept.contains(o)));
                }
            }
        }
        let mut leaves = vec![];
        for event in GraveyardGroup::leaves(&tx.group)? {
            let keep = match event {
                ProposedEvent::Sacrifice { object, player } => {
                    self.current(*object).is_ok_and(|o| {
                        o.present_on_battlefield()
                            && o.controller == *player
                            && self.is_live(*player)
                    })
                }
                ProposedEvent::DiscardCard { object, player } => {
                    self.zone(*object) == Some(Zone::Hand { owner: *player })
                        && self.is_live(*player)
                }
                ProposedEvent::DestroyCreature { object } => self.is_battlefield_creature(*object),
                ProposedEvent::DestroyPermanent { object } => self
                    .current(*object)
                    .is_ok_and(|o| o.present_on_battlefield()),
                ProposedEvent::StateBasedGraveyard { object, reason } => {
                    self.creature_sba_reason(*object)? == Some(*reason)
                }
                ProposedEvent::Move { object, .. } => {
                    aura_sbas.contains(event) || legend_moves.contains(object)
                }
                ProposedEvent::Attach { .. } => aura_sbas.contains(event),
                ProposedEvent::CeaseToken { object } | ProposedEvent::CeaseSpellCopy { object } => {
                    self.current(*object)
                        .is_ok_and(|o| self.cessation(o).as_ref() == Some(event))
                }
                ProposedEvent::Lose { player, .. } => self.is_live(*player),
                _ => return Err("unsupported retained graveyard wave leaf".into()),
            };
            if keep {
                leaves.push(event.clone());
            }
        }
        tx.owed.retain(|p| self.is_live(*p));
        let next_group = if matches!(tx.group, EventGroup::Occurrence { .. }) && leaves.len() == 1 {
            EventGroup::Occurrence {
                proposed: leaves.remove(0),
            }
        } else {
            EventGroup::Simultaneous {
                occurrences: leaves,
            }
        };
        let mut shields = self.regeneration_members(&next_group, &tx.finish)?;
        for m in &mut shields {
            if m.regenerate.is_none()
                && let Some(old) = tx.shields.iter().find(|old| {
                    old.object == m.object && old.shield == m.shield && old.affected == m.affected
                })
            {
                m.regenerate = old.regenerate;
            }
        }
        if tx.group != next_group || tx.shields != shields {
            self.rules_mut().pending = None;
        }
        tx.group = next_group;
        tx.shields = shields;
        // No uncommitted trials have changed the live random stream. Rebuild
        // against the remaining libraries before sampling the next paid word.
        tx.draft = None;
        self.rules_mut().frame = Frame::GraveyardGroup { tx };
        Ok(())
    }

    pub(super) fn finish_payment_shuffles(&mut self, cast: &Casting) -> Result<(), String> {
        if let Frame::GraveyardGroup { tx } = &self.state.rules.frame
            && let GroupFinish::Payment { resume } = &tx.finish
        {
            let Frame::SacrificeCost { cast: retained } = self.saved_frame(*resume)? else {
                return Err("lost sacrifice payment".into());
            };
            if cast != retained {
                return Err("replacement payment identity mismatch".into());
            }
            let draft = tx
                .draft
                .clone()
                .ok_or("unprepared replacement payment entropy")?;
            self.finish_group_shuffles(&draft)?;
        }
        Ok(())
    }

    fn finish_group_shuffles(&mut self, draft: &Randomization) -> Result<(), String> {
        let state = self
            .rules_mut()
            .match_state
            .as_mut()
            .ok_or("missing replacement entropy")?;
        if state.library_shuffles != draft.ordinals {
            return Err("stale replacement entropy".into());
        }

        for (index, shuffle) in draft.shuffles.iter().enumerate() {
            let Frame::GraveyardGroup { tx } = &mut self.rules_mut().frame else {
                return Err("missing shuffle publication frame".into());
            };
            tx.publishing_shuffle = Some(index);
            let cause = tx.cause.clone();
            self.commit(
                ProposedEvent::FinishLibraryShuffle {
                    player: shuffle.owner,
                },
                cause,
            )?;
        }
        Ok(())
    }

    pub(super) fn publish_group_permutation(&mut self, player: PlayerId) -> Result<bool, String> {
        let rules = self.rules_mut();
        let Frame::GraveyardGroup { tx } = &rules.frame else {
            return Ok(false);
        };
        let Some(shuffle) = tx
            .publishing_shuffle
            .and_then(|index| tx.draft.as_ref()?.shuffles.get(index))
        else {
            return Ok(false);
        };
        let library = rules.libraries.entry(player).or_default();
        if shuffle.owner != player || shuffle.order.len() != library.len() {
            return Err("replacement permutation membership changed".into());
        }
        *library = shuffle
            .order
            .iter()
            .map(|&index| {
                library
                    .get(index)
                    .copied()
                    .ok_or("invalid replacement permutation")
            })
            .collect::<Result<_, _>>()?;
        Ok(true)
    }

    pub(super) fn group_replacement_request(&self) -> Option<Decision<Rules>> {
        match &self.state.rules.frame {
            Frame::GraveyardGroup { tx } => tx.next(self),
            _ => None,
        }
    }

    pub(super) fn graveyard_group_step(
        &mut self,
        mut tx: Box<GraveyardGroup>,
    ) -> Result<(), String> {
        if let Some(request) = tx.next(self) {
            let input = self
                .rules_mut()
                .pending
                .take()
                .ok_or("missing graveyard replacement choice")?;
            if input.player != request.player {
                return Err("unexpected graveyard chooser".into());
            }
            match request.reason {
                Reason::GraveyardReplacementOrder {
                    object,
                    offered: Some(_),
                } => {
                    tx.shields
                        .iter_mut()
                        .find(|m| m.object == object && m.regenerate.is_none())
                        .ok_or("missing regeneration choice")?
                        .regenerate = Some(input.flag(Flag::Accept)?);
                }
                Reason::GraveyardReplacementOrder {
                    object,
                    offered: None,
                } => {
                    let perform = input.flag(Flag::Accept)?;
                    tx.members
                        .iter_mut()
                        .find(|m| m.object == object)
                        .ok_or("missing affected object")?
                        .printed = Some(perform);
                    if perform {
                        self.record_event(Event::GraveyardReplacementRevealed {
                            cause: tx.cause.clone(),
                            objects: vec![object],
                        })?;
                    }
                }
                Reason::CommanderReturn {
                    object: Some(object),
                    ..
                } => {
                    tx.members
                        .iter_mut()
                        .find(|m| m.object == object)
                        .ok_or("missing commander")?
                        .command = Some(input.flag(Flag::CommandZone)?);
                }
                _ => return Err("unexpected graveyard choice".into()),
            }
        } else if !tx.uses_shuffle_transaction()
            && !matches!(tx.finish, GroupFinish::Payment { .. })
        {
            let batch = if GraveyardGroup::leaves(&tx.group)?.is_empty() {
                None
            } else {
                Some(self.commit_batch(tx.cause.clone(), tx.group.clone())?)
            };
            return tx.finish.complete(self, batch.as_ref());
        } else if tx.draft.is_none() {
            tx.draft = Some(tx.randomization(self)?);
        } else {
            let draft = tx.draft.as_mut().expect("initialized draft");
            if let Some(shuffle) = draft.shuffles.get_mut(draft.cursor) {
                if shuffle.index == 0 {
                    draft.random.open = None;
                    draft.cursor += 1;
                } else {
                    let upper =
                        u32::try_from(shuffle.index + 1).map_err(|_| "shuffle index overflow")?;
                    if let Some(selected) =
                        setup::sample_bounded_word(draft.random.word(shuffle.stream)?, upper)
                    {
                        shuffle.order.swap(shuffle.index, selected as usize);
                        shuffle.index -= 1;
                    }
                }
            } else {
                if let GroupFinish::Payment { resume } = &tx.finish {
                    let Frame::SacrificeCost { cast } = self.saved_frame(*resume)? else {
                        return Err("lost sacrifice payment continuation".into());
                    };
                    return self.payment_step(cast.clone());
                }
                let batch = if GraveyardGroup::leaves(&tx.group)?.is_empty() {
                    None
                } else {
                    Some(self.commit_batch(tx.cause.clone(), tx.group)?)
                };
                self.finish_group_shuffles(draft)?;
                let Frame::GraveyardGroup { tx: updated } = &self.state.rules.frame else {
                    return Err("lost graveyard group continuation".into());
                };
                return updated.finish.clone().complete(self, batch.as_ref());
            }
        }
        self.rules_mut().frame = Frame::GraveyardGroup { tx };
        Ok(())
    }
}

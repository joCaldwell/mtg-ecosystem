//! CR 614.4–6, 616.1/1f, 701.24c/f, 903.9b. Resolve choices against the
//! common prior. Sample into a draft permutation one word per budget unit;
//! publish movement and the resulting shuffles together at the batch boundary.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "completion", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum GroupFinish {
    Instruction {
        frame: Resolving,
        result: GroupResult,
    },
    Cleanup,
    Payment {
        resume: Box<Frame>,
    },
    StateBased {
        resume: Box<Frame>,
        checked: Vec<ObjectId>,
        kept: Vec<ObjectId>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum GroupResult {
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
    owner: PlayerId,
    order: Vec<usize>,
    index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Randomization {
    start: RngPosition,
    random: MatchRandom,
    shuffles: Vec<Shuffle>,
    cursor: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GraveyardGroup {
    pub(super) finish: GroupFinish,
    cause: BatchCause,
    group: EventGroup,
    members: Vec<Member>,
    owed: Vec<PlayerId>,
    draft: Option<Randomization>,
    #[serde(skip)]
    publishing_shuffle: Option<usize>,
}

impl GroupFinish {
    pub(super) fn frame(&self) -> Option<&Resolving> {
        match self {
            Self::Instruction { frame, .. } => Some(frame),
            _ => None,
        }
    }
    pub(super) fn frame_mut(&mut self) -> Option<&mut Resolving> {
        match self {
            Self::Instruction { frame, .. } => Some(frame),
            _ => None,
        }
    }
    fn complete(self, game: &mut Game, batch: Option<&CommittedBatch>) -> Result<(), String> {
        let occurrences = batch.map_or(&[][..], |b| b.occurrences.as_slice());
        let single = batch.and_then(|b| b.result(&[]));
        match self {
            Self::Payment { .. } => {
                return Err("payment completion belongs to its announcement".into());
            }
            Self::Cleanup => game.state.rules.frame = Frame::CleanupClear,
            Self::StateBased {
                resume, checked, ..
            } => {
                game.consume_checked_deathtouch(&checked)?;
                if matches!(game.state.rules.frame, Frame::GraveyardGroup { .. }) {
                    game.state.rules.frame = *resume.clone();
                }
                if let Some(batch) = batch
                    && game.finish_committed_player_losses(batch)?
                {
                    return Ok(());
                }
                game.state.rules.frame = *resume;
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
                                        && o.characteristics.kind == CardKind::Creature
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

    fn candidate(game: &Game, event: &ProposedEvent) -> Result<Option<Member>, String> {
        let object = match event {
            ProposedEvent::DestroyPermanent { object }
            | ProposedEvent::DestroyCreature { object }
                if !game.has_keyword(*object, KeywordAbility::Indestructible) =>
            {
                *object
            }
            ProposedEvent::StateBasedGraveyard { object, .. }
            | ProposedEvent::Move {
                object,
                destination: Zone::Graveyard { .. },
            }
            | ProposedEvent::Sacrifice { object, .. }
            | ProposedEvent::DiscardCard { object, .. } => *object,
            _ => return Ok(None),
        };
        if !game.has_self_graveyard_replacement(object)? {
            return Ok(None);
        }
        let owner = game.state.rules.cards[&object.card].owner;
        let current = game.current(object)?;
        let affected = if matches!(current.zone, Zone::Battlefield | Zone::Stack) {
            current.controller
        } else {
            owner
        };
        let competing = game.anger_replaces_death(object, Zone::Graveyard { owner });
        Ok(Some(Member {
            object,
            owner,
            affected,
            printed: (!competing).then_some(true),
            commander: game.state.rules.commanders.history(object.card).is_some(),
            command: None,
        }))
    }

    fn next(&self, game: &Game) -> Option<Request> {
        let decision_id = game.state.rules.revision;
        for player in game.apnap_seats() {
            if let Some(m) = self
                .members
                .iter()
                .find(|m| m.affected == player && m.printed.is_none())
            {
                return Some(Request::GraveyardReplacementOrder {
                    decision_id,
                    player,
                    object: m.object,
                });
            }
        }
        for player in game.apnap_seats() {
            if let Some(m) = self.members.iter().find(|m| {
                m.owner == player && m.commander && m.printed == Some(true) && m.command.is_none()
            }) {
                return Some(Request::CommanderReturn {
                    decision_id,
                    player,
                    object: m.object,
                    destination: Zone::Library { owner: player },
                    previous_choices: self.prior_choices(game),
                });
            }
        }
        None
    }

    pub(super) fn prior_choices(&self, game: &Game) -> Vec<(ObjectId, bool)> {
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

    fn randomization(&self, game: &Game) -> Result<Randomization, String> {
        let destinations = self.destinations()?;
        let random = game
            .state
            .rules
            .match_state
            .as_ref()
            .ok_or("replacement requires seeded randomness")?
            .random
            .clone();
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
            shuffles.push(Shuffle {
                owner,
                order: (0..len).collect(),
                index: len.saturating_sub(1),
            });
        }
        Ok(Randomization {
            start: random.word_position,
            random,
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
    pub(super) fn validate(&self, game: &Game) -> Result<(), String> {
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
        let mut candidates = Vec::new();
        for event in leaves {
            if let Some(object) = Self::object(event)
                && !seen.insert(object)
            {
                return Err("duplicate object in retained wave".into());
            }
            if let Some(m) = Self::candidate(game, event)? {
                candidates.push(m);
            }
        }
        if candidates.len() != self.members.len()
            || candidates.iter().zip(&self.members).any(|(c, m)| {
                c.object != m.object
                    || c.owner != m.owner
                    || c.affected != m.affected
                    || c.commander != m.commander
                    || (m.printed.is_none() && c.printed == Some(true))
                    || (m.command.is_some() && (!m.commander || m.printed != Some(true)))
            })
        {
            return Err("graveyard replacement member invariant".into());
        }
        let producing = match &self.finish {
            GroupFinish::Instruction { frame, result } => {
                game.validate_resolving(frame)?;
                let i = game.program_for(&frame.spell)?.instructions.get(frame.cursor as usize).ok_or("missing retained instruction")?;
                let kind = match result {
                    GroupResult::Effect => matches!(i, Instruction::DestroyTargetCreature {..} | Instruction::DiscardPlayerSet {..} | Instruction::EachPlayerSacrifice {..} | Instruction::EachOpponentSacrifice {..}),
                    GroupResult::ActualEffect => matches!(i, Instruction::DestroyPermanent {..}),
                    GroupResult::DestroySet => matches!(i, Instruction::MoveSet {action:SetMove::Destroy,..} | Instruction::DestroySet {..} | Instruction::DestroyAllCreatures {..}),
                    GroupResult::Discard {..} => matches!(i, Instruction::DiscardCards {..} | Instruction::DiscardTargetPlayer {..} | Instruction::DiscardTargetPlayerAtRandom {..}),
                    GroupResult::Sacrifice {..} => matches!(i, Instruction::SacrificeCreature {..} | Instruction::TargetPlayerSacrificeCreature {..}),
                };
                kind && self.cause == Game::staple_cause(frame,i.source_block()) && leaves.iter().all(|e| match (i,e) {
                    (Instruction::DestroyPermanent {target_slot,..},ProposedEvent::DestroyPermanent {object})
                    | (Instruction::DestroyTargetCreature {target_slot,..},ProposedEvent::DestroyCreature {object}) => frame.spell.targets.get(*target_slot as usize) == Some(object) && frame.legal_targets.get(*target_slot as usize) == Some(&true),
                    (Instruction::MoveSet {..}|Instruction::DestroySet {..}|Instruction::DestroyAllCreatures {..},ProposedEvent::DestroyPermanent {..}) => true,
                    (Instruction::DiscardPlayerSet {..},ProposedEvent::DiscardCard {..}) => true,
                    (_,ProposedEvent::DiscardCard {player,..}) => matches!(result, GroupResult::Discard {player:p} if p == player),
                    (Instruction::EachOpponentSacrifice {..}|Instruction::EachPlayerSacrifice {..},ProposedEvent::Sacrifice {..}) => true,
                    (_,ProposedEvent::Sacrifice {player,..}) => matches!(result, GroupResult::Sacrifice {player:p} if p == player),
                    _ => false,
                })
            }
            GroupFinish::Payment { resume } => {
                let Frame::SacrificeCost {cast} = resume.as_ref() else {return Err("invalid retained cost kind".into());};
                let top = game.top()?;
                game.check_sacrifice_payment(top,cast)?;
                top.id == cast.stack_id && top.paid.is_none()
                    && self.cause == (BatchCause::Payment { stack_id:top.id, player:top.controller })
                    && Some(&self.group) == game.sacrifice_payment_group(top,&cast.sacrifices)?.as_ref()
            }
            GroupFinish::Cleanup => {
                self.cause == BatchCause::Cleanup && leaves.iter().all(|e| {
                    matches!(e, ProposedEvent::Move { object, destination: Zone::Graveyard {owner} }
                        if *owner == game.state.rules.turn_owner && game.zone(*object) == Some(Zone::Hand { owner: *owner }))
                })
            }
            GroupFinish::StateBased {resume,checked,..} => self.cause == BatchCause::StateBased && matches!(resume.as_ref(), Frame::Stabilize {changed:true,..}) && checked.windows(2).all(|w| w[0] < w[1]) && leaves.iter().all(|e| matches!(e, ProposedEvent::StateBasedGraveyard {..} | ProposedEvent::Move {destination: Zone::Graveyard {..},..} | ProposedEvent::CeaseToken {..} | ProposedEvent::CeaseSpellCopy {..} | ProposedEvent::Attach {target:None,..} | ProposedEvent::Lose {..})),
        };
        if !producing {
            return Err("graveyard wave producer mismatch".into());
        }
        let seeded = game
            .state
            .rules
            .match_state
            .as_ref()
            .ok_or("missing graveyard replacement entropy")?;
        if let Some(d) = &self.draft {
            let expected = self.randomization(game)?;
            if d.start != seeded.random.word_position
                || d.random.seed != seeded.random.seed
                || d.random.version != seeded.random.version
                || d.random.stream != seeded.random.stream
                || d.random.word_position.0 < d.start.0
                || d.cursor > d.shuffles.len()
                || d.shuffles.len() != expected.shuffles.len()
            {
                return Err("graveyard shuffle draft stream invariant".into());
            }
            for (i, (s, e)) in d.shuffles.iter().zip(&expected.shuffles).enumerate() {
                if s.owner != e.owner
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
        game: &Game,
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
            || !self
                .draft
                .as_ref()
                .is_some_and(|d| d.cursor == d.shuffles.len())
        {
            return Err("graveyard wave origin or preparation mismatch".into());
        }
        if self.next(game).is_some() {
            return Err("outstanding graveyard choice".into());
        }
        self.destinations().map(Some)
    }
}

impl Game {
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
        let mut members = vec![];
        for event in leaves {
            if let Some(m) = GraveyardGroup::candidate(self, event)? {
                members.push(m)
            }
        }
        if members.is_empty() {
            let batch = self.commit_batch(cause, group)?;
            return finish.complete(self, Some(&batch));
        }
        if self.state.rules.match_state.is_none() {
            return Err("printed graveyard shuffle requires a seeded match".into());
        }
        let revealed: Vec<_> = members
            .iter()
            .filter(|m| m.printed == Some(true))
            .map(|m| m.object)
            .collect();
        if !revealed.is_empty() {
            self.record_event(Event::GraveyardReplacementRevealed {
                cause: cause.clone(),
                objects: revealed,
            })?;
        }
        self.state.rules.frame = Frame::GraveyardGroup {
            tx: Box::new(GraveyardGroup {
                finish,
                cause,
                group,
                members,
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
            let Frame::SacrificeCost { cast } = resume.as_ref() else {
                return Err("invalid replacement cost resume".into());
            };
            if self.top().is_err() || self.top()?.id != cast.stack_id {
                self.state.rules.frame = *resume.clone();
                self.state.rules.pending = None;
                return Ok(());
            }
            if self.check_sacrifice_payment(self.top()?, cast).is_err() {
                let mut cast = cast.clone();
                cast.sacrifices.clear();
                self.state.rules.frame = Frame::SacrificeCost { cast };
                self.state.rules.pending = None;
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
                ProposedEvent::Move {
                    object,
                    destination: Zone::Graveyard { owner },
                } if matches!(tx.finish, GroupFinish::Cleanup) => {
                    self.is_live(*owner) && self.zone(*object) == Some(Zone::Hand { owner: *owner })
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
        let mut next = vec![];
        let mut reveal = vec![];
        for event in &leaves {
            let Some(mut m) = GraveyardGroup::candidate(self, event)? else {
                continue;
            };
            if let Some(old) = tx.members.iter().find(|o| o.object == m.object) {
                if let Some(selected) = old.printed {
                    m.printed = Some(selected);
                    m.command = old.command;
                } else if m.printed == Some(true) {
                    reveal.push(m.object);
                }
            } else if m.printed == Some(true) {
                reveal.push(m.object);
            }
            next.push(m);
        }
        tx.owed.retain(|p| self.is_live(*p));
        for old in &tx.members {
            if old.printed == Some(true)
                && self.is_live(old.owner)
                && !next.iter().any(|m| m.object == old.object)
            {
                tx.owed.push(old.owner);
            }
        }
        if !reveal.is_empty() {
            self.record_event(Event::GraveyardReplacementRevealed {
                cause: tx.cause.clone(),
                objects: reveal,
            })?;
        }
        let next_group = if matches!(tx.group, EventGroup::Occurrence { .. }) && leaves.len() == 1 {
            EventGroup::Occurrence {
                proposed: leaves.remove(0),
            }
        } else {
            EventGroup::Simultaneous {
                occurrences: leaves,
            }
        };
        if tx.members != next || tx.group != next_group {
            self.state.rules.pending = None;
        }
        tx.group = next_group;
        tx.members = next;
        // No uncommitted trials have changed the live random stream. Rebuild
        // against the remaining libraries before sampling the next paid word.
        tx.draft = None;
        self.state.rules.frame = Frame::GraveyardGroup { tx };
        Ok(())
    }

    pub(super) fn finish_payment_shuffles(&mut self, cast: &Casting) -> Result<(), String> {
        if let Frame::GraveyardGroup { tx } = &self.state.rules.frame
            && let GroupFinish::Payment { resume } = &tx.finish
        {
            let Frame::SacrificeCost { cast: retained } = resume.as_ref() else {
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
            .state
            .rules
            .match_state
            .as_mut()
            .ok_or("missing replacement entropy")?;
        if state.random.word_position != draft.start {
            return Err("stale replacement entropy".into());
        }
        state.random = draft.random.clone();
        for (index, shuffle) in draft.shuffles.iter().enumerate() {
            let Frame::GraveyardGroup { tx } = &mut self.state.rules.frame else {
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
        let Frame::GraveyardGroup { tx } = &self.state.rules.frame else {
            return Ok(false);
        };
        let Some(shuffle) = tx
            .publishing_shuffle
            .and_then(|index| tx.draft.as_ref()?.shuffles.get(index))
        else {
            return Ok(false);
        };
        let library = self.state.rules.libraries.entry(player).or_default();
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

    pub(super) fn group_replacement_request(&self) -> Option<Request> {
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
                .state
                .rules
                .pending
                .take()
                .ok_or("missing graveyard replacement choice")?;
            if input.player != request.player() {
                return Err("unexpected graveyard chooser".into());
            }
            match (request, input.response) {
                (
                    Request::GraveyardReplacementOrder { object, .. },
                    Response::OptionalEffect { perform },
                ) => {
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
                (
                    Request::CommanderReturn { object, .. },
                    Response::CommanderReturn { to_command },
                ) => {
                    tx.members
                        .iter_mut()
                        .find(|m| m.object == object)
                        .ok_or("missing commander")?
                        .command = Some(to_command);
                }
                _ => return Err("unexpected graveyard response".into()),
            }
        } else if tx.draft.is_none() {
            tx.draft = Some(tx.randomization(self)?);
        } else {
            let draft = tx.draft.as_mut().expect("initialized draft");
            if let Some(shuffle) = draft.shuffles.get_mut(draft.cursor) {
                if shuffle.index == 0 {
                    draft.cursor += 1;
                } else {
                    let upper =
                        u32::try_from(shuffle.index + 1).map_err(|_| "shuffle index overflow")?;
                    if let Some(selected) = setup::sample_bounded_word(draft.random.word()?, upper)
                    {
                        shuffle.order.swap(shuffle.index, selected as usize);
                        shuffle.index -= 1;
                    }
                }
            } else {
                if let GroupFinish::Payment { resume } = &tx.finish {
                    let Frame::SacrificeCost { cast } = resume.as_ref() else {
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
        self.state.rules.frame = Frame::GraveyardGroup { tx };
        Ok(())
    }
}

use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cause", rename_all = "snake_case", deny_unknown_fields)]
pub enum BatchCause {
    ManaActivation {
        decision_id: u64,
        player: PlayerId,
        source: ObjectId,
    },
    Casting {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
    },
    Payment {
        stack_id: StackId,
        player: PlayerId,
    },
    CastReversal {
        stack_id: StackId,
        player: PlayerId,
    },
    Instruction {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
        cursor: u32,
        source_block: u32,
    },
    SpellCompletion {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
    },
    Departure {
        player: PlayerId,
    },
    Fixture,
}
impl BatchCause {
    fn rules(&self) -> Vec<String> {
        let rules: &[&str] = match self {
            Self::ManaActivation { .. } => &["106.3", "305.6", "605.3b"],
            Self::Casting { .. } | Self::Payment { .. } => &["601.2"],
            Self::CastReversal { .. } => &["733.1", "733.2"],
            Self::Instruction { .. } | Self::SpellCompletion { .. } => &["400.7", "608.2"],
            Self::Departure { .. } => &["800.4a"],
            Self::Fixture => &["700.1"],
        };
        rules.iter().map(|r| (*r).into()).collect()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "group", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventGroup {
    Occurrence { proposed: ProposedEvent },
    Ordered { children: Vec<EventGroup> },
    Simultaneous { occurrences: Vec<ProposedEvent> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBefore {
    pub object: ObjectId,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub default_controller: PlayerId,
    pub zone: Zone,
    pub tapped: bool,
    pub characteristics: Characteristics,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerBefore {
    pub player: PlayerId,
    pub life: Quantity,
    pub mana: BTreeMap<Color, Quantity>,
    pub status: PlayerStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreEventFacts {
    pub objects: Vec<ObjectBefore>,
    pub players: Vec<PlayerBefore>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OccurrenceId {
    pub batch: u64,
    pub path: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OccurrenceResult {
    pub id: OccurrenceId,
    pub before: PreEventFacts,
    pub proposed: ProposedEvent,
    pub actual: EventOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedBatch {
    pub batch_id: u64,
    pub cause: BatchCause,
    pub rules: Vec<String>,
    pub group: EventGroup,
    pub before: PreEventFacts,
    pub occurrences: Vec<OccurrenceResult>,
}
impl CommittedBatch {
    pub fn result(&self, path: &[u32]) -> Option<&EventOutcome> {
        self.occurrences
            .iter()
            .find(|r| r.id.path == path)
            .map(|r| &r.actual)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FactKey {
    Object(CardId),
    Mana(PlayerId),
}
fn access(event: &ProposedEvent) -> (BTreeSet<FactKey>, BTreeSet<FactKey>) {
    match event {
        ProposedEvent::Tap { object }
        | ProposedEvent::Move { object, .. }
        | ProposedEvent::LeaveGame { object, .. } => {
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            (keys.clone(), keys)
        }
        ProposedEvent::AddMana { player, source, .. } => (
            BTreeSet::from([FactKey::Object(source.card), FactKey::Mana(*player)]),
            BTreeSet::from([FactKey::Mana(*player)]),
        ),
        ProposedEvent::PayMana { player, .. } => {
            let keys = BTreeSet::from([FactKey::Mana(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::ReverseMana { player, source, .. } => {
            let mut keys = BTreeSet::from([FactKey::Mana(*player)]);
            if let Some(object) = source {
                keys.insert(FactKey::Object(object.card));
            }
            (keys.clone(), keys)
        }
    }
}
fn collect<'a>(
    group: &'a EventGroup,
    depth: u32,
    out: &mut Vec<&'a ProposedEvent>,
) -> Result<(), String> {
    if depth > 4 {
        return Err("event group depth resource limit".into());
    }
    match group {
        EventGroup::Occurrence { proposed } => out.push(proposed),
        EventGroup::Ordered { children } => {
            if children.is_empty() || children.len() > 16 {
                return Err("empty or oversized ordered group".into());
            }
            for child in children {
                collect(child, depth + 1, out)?;
            }
        }
        EventGroup::Simultaneous { occurrences } => {
            if occurrences.is_empty() || occurrences.len() > 16 {
                return Err("empty or oversized simultaneous group".into());
            }
            out.extend(occurrences);
            let keys = occurrences.iter().map(access).collect::<Vec<_>>();
            for i in 0..keys.len() {
                for j in i + 1..keys.len() {
                    if !keys[i].1.is_disjoint(&keys[j].0) || !keys[j].1.is_disjoint(&keys[i].0) {
                        return Err("unsupported simultaneous read/write dependency".into());
                    }
                }
            }
        }
    }
    if out.len() > 16 {
        return Err("event occurrence resource limit".into());
    }
    Ok(())
}
impl Game {
    fn facts(&self, events: &[&ProposedEvent]) -> Result<PreEventFacts, String> {
        let mut objects = BTreeSet::new();
        let mut players = BTreeSet::new();
        for event in events {
            match event {
                ProposedEvent::Tap { object } | ProposedEvent::Move { object, .. } => {
                    objects.insert(*object);
                }
                ProposedEvent::LeaveGame {
                    object,
                    departing_owner,
                } => {
                    objects.insert(*object);
                    players.insert(*departing_owner);
                }
                ProposedEvent::AddMana { player, source, .. } => {
                    objects.insert(*source);
                    players.insert(*player);
                }
                ProposedEvent::PayMana { player, .. } => {
                    players.insert(*player);
                }
                ProposedEvent::ReverseMana { player, source, .. } => {
                    players.insert(*player);
                    if let Some(object) = source {
                        objects.insert(*object);
                    }
                }
            }
        }
        let objects = objects
            .into_iter()
            .map(|object| {
                let o = self.current(object)?;
                Ok(ObjectBefore {
                    object,
                    owner: self.state.cards[&object.card].owner,
                    controller: o.controller,
                    default_controller: o.default_controller,
                    zone: o.zone,
                    tapped: o.tapped,
                    characteristics: self.characteristics(object)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let players = players
            .into_iter()
            .map(|player| {
                let p = self
                    .state
                    .players
                    .get(player.0 as usize)
                    .ok_or("invalid event player")?;
                Ok(PlayerBefore {
                    player,
                    life: p.life.clone(),
                    mana: p.mana.clone(),
                    status: p.status,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(PreEventFacts { objects, players })
    }
    // CR 700.1: grouping is explicit. These profiles contain no replacement,
    // prevention, or trigger consumers; those extend this same actual path.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn commit_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
    ) -> Result<CommittedBatch, String> {
        let mut events = vec![];
        collect(&group, 0, &mut events)?;
        let before = self.facts(&events)?;
        let batch_id = self.state.next_batch_id;
        let next_batch = batch_id.checked_add(1).ok_or("batch ID resource limit")?;
        let mut staged = self.fork();
        staged.state.next_batch_id = next_batch;
        let mut occurrences = vec![];
        staged.apply_group(&group, batch_id, &mut vec![], &mut occurrences)?;
        let result = CommittedBatch {
            batch_id,
            rules: cause.rules(),
            cause,
            group,
            before,
            occurrences,
        };
        staged.record_evidence(EvidenceRecord::Batch {
            batch: Box::new(result.clone()),
        })?;
        self.state = staged.state;
        self.trace = staged.trace;
        self.state_token = Arc::new(());
        Ok(result)
    }
    fn apply_group(
        &mut self,
        group: &EventGroup,
        batch: u64,
        path: &mut Vec<u32>,
        out: &mut Vec<OccurrenceResult>,
    ) -> Result<(), String> {
        match group {
            EventGroup::Occurrence { proposed } => {
                let before = self.facts(&[proposed])?;
                self.apply_leaf(proposed, batch, path, before, out)?;
            }
            EventGroup::Ordered { children } => {
                for (i, child) in children.iter().enumerate() {
                    path.push(u32::try_from(i).map_err(|_| "event path resource limit")?);
                    self.apply_group(child, batch, path, out)?;
                    path.pop();
                }
            }
            EventGroup::Simultaneous { occurrences } => {
                // Independent writes only: validate every leaf against the same
                // pre-state before committing any leaf; no replacement ordering
                // or aggregation of overlapping writes is inferred.
                let common = self.fork();
                let mut facts = vec![];
                for event in occurrences {
                    facts.push(common.facts(&[event])?);
                    common.fork().apply_occurrence(event)?;
                }
                for (i, (event, before)) in occurrences.iter().zip(facts).enumerate() {
                    path.push(u32::try_from(i).map_err(|_| "event path resource limit")?);
                    self.apply_leaf(event, batch, path, before, out)?;
                    path.pop();
                }
            }
        }
        Ok(())
    }
    fn apply_leaf(
        &mut self,
        proposed: &ProposedEvent,
        batch: u64,
        path: &[u32],
        before: PreEventFacts,
        out: &mut Vec<OccurrenceResult>,
    ) -> Result<(), String> {
        let actual = self.apply_occurrence(proposed)?;
        // Keep the pre-existing native flat trace as a compatibility view.
        let event = match &actual {
            EventOutcome::LeftGame { object, owner } => Event::ObjectLeftGame {
                object: *object,
                owner: *owner,
            },
            _ => Event::Committed {
                proposed: proposed.clone(),
                actual: actual.clone(),
            },
        };
        self.record_event(event)?;
        out.push(OccurrenceResult {
            id: OccurrenceId {
                batch,
                path: path.to_vec(),
            },
            before,
            proposed: proposed.clone(),
            actual,
        });
        Ok(())
    }
    pub(super) fn commit(
        &mut self,
        proposed: ProposedEvent,
        cause: BatchCause,
    ) -> Result<EventOutcome, String> {
        let result = self.commit_batch(cause, EventGroup::Occurrence { proposed })?;
        result
            .result(&[])
            .cloned()
            .ok_or_else(|| "missing singleton actual outcome".into())
    }
    pub(super) fn move_object_for(
        &mut self,
        object: ObjectId,
        destination: Zone,
        cause: BatchCause,
    ) -> Result<MoveOutcome, String> {
        match self.commit(
            ProposedEvent::Move {
                object,
                destination,
            },
            cause,
        )? {
            EventOutcome::Moved { movement } => Ok(movement),
            _ => Err("movement outcome invariant".into()),
        }
    }
    #[cfg(test)]
    pub(super) fn move_object(
        &mut self,
        object: ObjectId,
        destination: Zone,
    ) -> Result<MoveOutcome, String> {
        self.move_object_for(object, destination, BatchCause::Fixture)
    }
}

impl Game {
    fn apply_occurrence(&mut self, proposed: &ProposedEvent) -> Result<EventOutcome, String> {
        let actual = match proposed {
            ProposedEvent::Tap { object } => {
                let o = self.current(*object)?;
                if o.zone != Zone::Battlefield || o.tapped {
                    return Err("invalid tap event".into());
                }
                self.state
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing tap object")?
                    .tapped = true;
                EventOutcome::Tapped { object: *object }
            }
            ProposedEvent::AddMana {
                player,
                color,
                amount,
                source,
            } => {
                self.current(*source)?;
                if amount.is_negative() || !self.is_live(*player) {
                    return Err("negative mana or departed recipient".into());
                }
                self.state
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing mana player")?
                    .mana
                    .entry(*color)
                    .or_default()
                    .add(amount);
                EventOutcome::ManaAdded {
                    player: *player,
                    color: *color,
                    amount: amount.clone(),
                }
            }
            ProposedEvent::PayMana { player, cost } => {
                if cost.generic.is_negative() || !self.is_live(*player) {
                    return Err("invalid payment cost or player".into());
                }
                let pool = self.payment_pool(*player, cost)?;
                self.state
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing payment player")?
                    .mana = pool;
                EventOutcome::ManaPaid {
                    player: *player,
                    cost: cost.clone(),
                }
            }
            ProposedEvent::ReverseMana {
                player,
                color,
                amount,
                source,
                remove_zero,
            } => {
                // CR 733.1/733.2: undo only recorded casting-stage mana. A
                // departed source stays absent; a surviving source may have a
                // new controller after the intervening departure boundary.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                if *amount <= Quantity::zero() || !self.is_live(*player) {
                    return Err("invalid mana reversal quantity or player".into());
                }
                if let Some(object) = source {
                    let o = self.current(*object)?;
                    if o.zone != Zone::Battlefield
                        || !o.tapped
                        || self.characteristics(*object)?.kind != CardKind::Land
                    {
                        return Err("invalid mana reversal source".into());
                    }
                }
                let pool = &mut self
                    .state
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing mana reversal player")?
                    .mana;
                let remaining = pool.get_mut(color).ok_or("missing reversal mana")?;
                if *remaining < *amount {
                    return Err("insufficient reversal mana".into());
                }
                remaining.subtract(amount);
                let removed_entry = *remove_zero && *remaining == Quantity::zero();
                if removed_entry {
                    pool.remove(color);
                }
                if let Some(object) = source {
                    self.state
                        .objects
                        .get_mut(&object.card)
                        .ok_or("missing mana reversal source")?
                        .tapped = false;
                }
                EventOutcome::ManaReversed {
                    player: *player,
                    color: *color,
                    amount: amount.clone(),
                    source: *source,
                    removed_entry,
                }
            }
            ProposedEvent::LeaveGame {
                object,
                departing_owner,
            } => {
                self.current(*object)?;
                if self.state.cards[&object.card].owner != *departing_owner
                    || self.is_live(*departing_owner)
                {
                    return Err("invalid leave-game ownership/status".into());
                }
                self.state.objects.remove(&object.card);
                EventOutcome::LeftGame {
                    object: *object,
                    owner: *departing_owner,
                }
            }
            ProposedEvent::Move {
                object,
                destination,
            } => {
                let o = self.current(*object)?.clone();
                let owner = self
                    .state
                    .cards
                    .get(&object.card)
                    .ok_or("missing movement card")?
                    .owner;
                if let Zone::Hand { owner: p }
                | Zone::Graveyard { owner: p }
                | Zone::Library { owner: p }
                | Zone::Command { owner: p } = destination
                    && *p != owner
                {
                    return Err("zone owner invariant".into());
                }
                // CR 400.7: a zone move creates a new incarnation, and an actual
                // outcome explicitly provides its successor. No stale lookup.
                let successor = ObjectId {
                    card: object.card,
                    incarnation: object
                        .incarnation
                        .checked_add(1)
                        .ok_or("incarnation resource limit")?,
                };
                let controller = if *destination == Zone::Battlefield {
                    o.controller
                } else {
                    owner
                };
                self.state.objects.insert(
                    object.card,
                    Object {
                        id: successor,
                        controller,
                        default_controller: controller,
                        zone: *destination,
                        tapped: false,
                    },
                );
                EventOutcome::Moved {
                    movement: MoveOutcome {
                        previous: *object,
                        successor,
                        from: o.zone,
                        destination: *destination,
                    },
                }
            }
        };
        Ok(actual)
    }
}

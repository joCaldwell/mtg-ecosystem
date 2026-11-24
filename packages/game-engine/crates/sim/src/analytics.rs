//! Trusted post-game analysis of verified, terminal bench journals. This module
//! does not provide any inputs to live play or persist private replay material.
use crate::policy::can_fund;
use mtg_contracts::{CardKind, Catalog, Quantity, digest};
use mtg_core::{
    Advance, BatchCause, Budget, CardId, DeckRole, Decklist, DynamicDeckEntry, Event, EventGroup,
    EventOutcome, EvidenceRecord, Game, MoveOutcome, PlayerId, Prepared, QueryResult, QueryValue,
    ReplayJournal, SetupOutcome, TraceMode, TurnStep, Zone,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::Path,
};

/// Deck slots and catalog identity of a bench source, embedded or bulk.
pub struct AnalyticsProfile<'a> {
    pub build: mtg_core::BuildIdentity,
    pub name: &'a str,
    pub catalog: &'a Catalog,
    pub catalog_sha256: &'a str,
    pub decks: [AnalyticsDeck; 4],
}

/// Sorted commander designations and physical quantities, commanders included.
pub struct AnalyticsDeck {
    commanders: Vec<String>,
    cards: BTreeMap<String, u64>,
}
impl AnalyticsDeck {
    pub fn embedded(deck: &Decklist) -> Self {
        let mut cards = BTreeMap::new();
        for card in &deck.cards {
            *cards.entry(card.oracle_id.clone()).or_default() += u64::from(card.count);
        }
        Self {
            commanders: vec![deck.commander.clone()],
            cards,
        }
    }
    pub fn dynamic(rows: &[DynamicDeckEntry]) -> Self {
        let mut commanders = Vec::with_capacity(2);
        let mut cards = BTreeMap::new();
        for row in rows {
            if row.role == DeckRole::Commander {
                commanders.push(row.oracle_id.clone());
            }
            *cards.entry(row.oracle_id.clone()).or_default() += u64::from(row.quantity);
        }
        commanders.sort_unstable();
        commanders.dedup();
        Self { commanders, cards }
    }
}

pub struct PodOutcome {
    pub index: usize,
    pub slots: [usize; 4],
    pub starter: usize,
    pub terminal: bool,
    pub decision_capped: bool,
    pub budget_capped: bool,
    pub winner: Option<PlayerId>,
    pub placements: [Option<usize>; 4],
    pub turns: u64,
    pub decisions: usize,
}

pub struct ReplayInput {
    prepared: Prepared,
    journal: ReplayJournal,
    boundary: u64,
    state_sha256: String,
}
impl ReplayInput {
    pub fn capture(prepared: Prepared, game: &Game) -> Result<Self, String> {
        Ok(Self {
            prepared,
            journal: game.replay_journal(),
            boundary: game.rule_boundary(),
            state_sha256: game.state_digest()?,
        })
    }

    fn analyze(self, outcome: &PodOutcome, catalog: &Catalog) -> Result<[SeatFacts; 4], String> {
        if !self.journal.concessions.is_empty() || self.journal.decisions.len() != outcome.decisions
        {
            return Err("unexpected bench input journal".into());
        }
        let mut game = Game::with_trace(self.prepared, TraceMode::Detailed);
        let first_turn = game.turn().number;
        let mut probes = std::array::from_fn(|_| BTreeMap::new());
        for input in &self.journal.decisions {
            let Advance::AwaitingDecision(request) = advance_to(&mut game, self.boundary)? else {
                return Err("analytics decision is unreachable".into());
            };
            let player = request.player;
            if player != input.player || request.id != input.decision_id {
                return Err("analytics replay decision diverged".into());
            }
            if matches!(request.reason, mtg_core::Reason::Priority)
                && game.active_player() == Some(player)
                && matches!(
                    game.turn().step,
                    TurnStep::PrecombatMain | TurnStep::PostcombatMain
                )
            {
                let player_probes = &mut probes[seat(player)];
                let turn = game.turn().number;
                if player_probes.len() < 4 || player_probes.contains_key(&turn) {
                    let status = player_probes.entry(turn).or_insert_with(ManaProbe::default);
                    if !status.payable {
                        status.sample(&game, player, catalog)?;
                    }
                }
            }
            game.respond(player, input.decision_id, input.answer.clone())?;
        }
        match (advance_to(&mut game, self.boundary)?, outcome.winner) {
            (Advance::Finished { winner }, Some(expected)) if winner == expected => {}
            (Advance::Draw, None) => {}
            _ => return Err("analytics replay result diverged".into()),
        }
        if game.rule_boundary() != self.boundary
            || game.state_digest()? != self.state_sha256
            || game.journal().len() != self.journal.decisions.len()
        {
            return Err("analytics replay state diverged".into());
        }
        collect(&game, catalog, outcome, first_turn, &probes)
    }
}

fn advance_to(game: &mut Game, boundary: u64) -> Result<Advance, String> {
    let remaining = boundary
        .checked_sub(game.rule_boundary())
        .ok_or("analytics boundary overshot")?;
    Ok(game.advance(Budget::new(remaining)))
}

fn seat(player: PlayerId) -> usize {
    usize::from(player.0)
}

#[derive(Default)]
struct ManaProbe {
    spell_seen: bool,
    payable: bool,
    payable_without_color: bool,
}
impl ManaProbe {
    // A mana-only estimate for a nonland card in the chooser's hand. This does
    // not solve timing, targets, cost modifications, X, or alternative costs.
    fn sample(&mut self, game: &Game, player: PlayerId, catalog: &Catalog) -> Result<(), String> {
        let QueryResult::Complete(QueryValue::Observation(observation)) =
            game.observe(player, Budget::unlimited())
        else {
            return Err("early analytics observation unavailable".into());
        };
        let view = &observation.view;
        if !view.stack.is_empty() {
            return Ok(());
        }
        for object in view
            .objects
            .iter()
            .filter(|o| o.zone == (Zone::Hand { owner: player }))
        {
            let definition = catalog
                .definition_by_oracle_id(&object.oracle_id)
                .ok_or("missing analytics hand definition")?;
            let c = &definition.characteristics;
            if c.has_type(CardKind::Land) || definition.spell.is_none() {
                continue;
            }
            self.spell_seen = true;
            if can_fund(catalog, view, c.mana_cost.clone(), &c.hybrid_mana) {
                self.payable = true;
                break;
            }
            if !self.payable_without_color {
                let mut generic = c.mana_cost.clone();
                generic.generic.add(&Quantity::from(
                    (generic.colored.len() + c.hybrid_mana.len()) as i64,
                ));
                generic.colored.clear();
                self.payable_without_color = can_fund(catalog, view, generic, &[]);
            }
        }
        Ok(())
    }
}

#[derive(Default, Serialize)]
struct EarlyTurns {
    completed: u64,
    missed_land_drop: u64,
    with_spell_probe: u64,
    mana_blocked: u64,
    color_blocked: u64,
}
impl EarlyTurns {
    fn observe(&mut self, lands_played: u32, probe: Option<&ManaProbe>) {
        self.completed += 1;
        self.missed_land_drop += u64::from(lands_played == 0);
        if let Some(probe) = probe.filter(|probe| probe.spell_seen) {
            self.with_spell_probe += 1;
            self.mana_blocked += u64::from(!probe.payable);
            self.color_blocked += u64::from(!probe.payable && probe.payable_without_color);
        }
    }

    fn add(&mut self, other: Self) {
        self.completed += other.completed;
        self.missed_land_drop += other.missed_land_drop;
        self.with_spell_probe += other.with_spell_probe;
        self.mana_blocked += other.mana_blocked;
        self.color_blocked += other.color_blocked;
    }
}

#[derive(Default)]
struct SeatFacts {
    opening: BTreeSet<CardId>,
    in_hand: BTreeSet<CardId>,
    drawn: BTreeSet<CardId>,
    drawn_then_cast: BTreeSet<CardId>,
    played_lands: BTreeSet<CardId>,
    casts: BTreeMap<CardId, (u64, u64)>,
    first_commander_cast: Option<u64>,
    placement: Option<usize>,
    early: EarlyTurns,
    cards: BTreeMap<String, CardFacts>,
    unresolved_draws: u64,
}

#[derive(Default)]
struct CardFacts {
    opening: bool,
    in_hand: bool,
    drawn: bool,
    drawn_cast: bool,
    land_played: bool,
    casts: u64,
    cast_turn_sum: u64,
}

#[derive(Clone, Copy)]
enum DrawKind {
    Draw,
    Acquisition,
    Unresolved,
}

fn collect(
    game: &Game,
    catalog: &Catalog,
    outcome: &PodOutcome,
    first_turn: u64,
    probes: &[BTreeMap<u64, ManaProbe>; 4],
) -> Result<[SeatFacts; 4], String> {
    let config = game.config();
    let cards: BTreeMap<_, _> = config.cards.iter().map(|card| (card.id, card)).collect();
    let mut facts: [SeatFacts; 4] = std::array::from_fn(|_| SeatFacts::default());
    let mut casting = BTreeMap::new();
    let mut copied_spells = BTreeMap::new();
    let mut opening_complete = false;
    let mut turn = first_turn;
    let mut remaining = 4usize;

    for record in game.trace().records.iter() {
        match &record.record {
            EvidenceRecord::Batch { batch } => {
                let mut exiled_land = None;
                for occurrence in &batch.occurrences {
                    match &occurrence.actual {
                        EventOutcome::Setup {
                            actual: SetupOutcome::Moved { player, moves, .. },
                        } => {
                            let opening = &mut facts[seat(*player)].opening;
                            for moved in moves {
                                let card = moved.previous.card;
                                if moved.destination == (Zone::Hand { owner: *player }) {
                                    opening.insert(card);
                                } else if moved.from == (Zone::Hand { owner: *player }) {
                                    opening.remove(&card);
                                }
                            }
                        }
                        EventOutcome::Setup {
                            actual: SetupOutcome::Complete { .. },
                        } => {
                            opening_complete = true;
                            for row in &mut facts {
                                row.in_hand.clone_from(&row.opening);
                            }
                        }
                        EventOutcome::TurnTransitioned {
                            previous,
                            current,
                            previous_owner,
                            ..
                        } => {
                            if previous.step == TurnStep::Cleanup
                                && previous.number != current.number
                            {
                                let seat = seat(*previous_owner);
                                if facts[seat].placement.is_none()
                                    && facts[seat].early.completed < 4
                                {
                                    facts[seat].early.observe(
                                        previous.lands_played,
                                        probes[seat].get(&previous.number),
                                    );
                                }
                            }
                            turn = current.number;
                        }
                        EventOutcome::LandPlayCounted { player, .. } => {
                            let card = match batch.cause {
                                BatchCause::LandPlay { source, .. } => Some(source.card),
                                // The linked special action moves its land and
                                // charges a land play in the same committed batch.
                                BatchCause::Instruction { .. } => exiled_land,
                                _ => None,
                            };
                            if let Some(card) = card {
                                facts[seat(*player)].played_lands.insert(card);
                            }
                        }
                        EventOutcome::Moved { movement } if opening_complete => {
                            if movement.from == Zone::Exile
                                && movement.destination == Zone::Battlefield
                            {
                                exiled_land = Some(movement.previous.card);
                            }
                            if let BatchCause::Casting {
                                stack_id,
                                player,
                                source,
                            } = batch.cause
                                && movement.destination == Zone::Stack
                                && movement.previous == source
                            {
                                casting.insert(stack_id, (player, source.card));
                            }
                            if let Zone::Hand { owner } = movement.destination {
                                let draw = if movement.from != (Zone::Library { owner }) {
                                    DrawKind::Acquisition
                                } else {
                                    match batch.cause {
                                        BatchCause::DrawStep { player } if player == owner => {
                                            DrawKind::Draw
                                        }
                                        BatchCause::Instruction {
                                            stack_id, source, ..
                                        } if matches!(
                                            batch.group,
                                            EventGroup::Occurrence { .. }
                                        ) =>
                                        {
                                            let id = copied_spells
                                                .get(&stack_id)
                                                .copied()
                                                .unwrap_or(source.card);
                                            match cards.get(&id).and_then(|card| {
                                                catalog.definition_by_oracle_id(&card.oracle_id)
                                            }) {
                                                Some(_) => DrawKind::Draw,
                                                None => DrawKind::Unresolved,
                                            }
                                        }
                                        BatchCause::Instruction { .. }
                                            if matches!(
                                                batch.group,
                                                EventGroup::Simultaneous { .. }
                                            ) =>
                                        {
                                            DrawKind::Acquisition
                                        } // Typed land splits and colored search.
                                        _ => DrawKind::Unresolved,
                                    }
                                };
                                note_hand(&mut facts, movement, draw);
                            }
                        }

                        _ => {}
                    }
                }
            }
            EvidenceRecord::Event { event } => match event.as_ref() {
                Event::SpellCast { stack_id } => {
                    let &(player, card) = casting
                        .get(stack_id)
                        .ok_or("paid cast has no casting movement")?;
                    if cards
                        .get(&card)
                        .is_some_and(|source| source.owner == player)
                    {
                        let row = &mut facts[seat(player)];
                        let cast = row.casts.entry(card).or_default();
                        cast.0 += 1;
                        cast.1 += turn;
                        if row.drawn.contains(&card) {
                            row.drawn_then_cast.insert(card);
                        }
                        if config.commanders.contains(&card) {
                            row.first_commander_cast.get_or_insert(turn);
                        }
                    }
                }
                Event::SpellCopied { target, copy, .. } => {
                    if let Some(card) = copied_spells
                        .get(target)
                        .copied()
                        .or_else(|| casting.get(target).map(|&(_, card)| card))
                    {
                        copied_spells.insert(*copy, card);
                    }
                }
                Event::PlayersLost { losses } => {
                    remaining = remaining
                        .checked_sub(losses.len())
                        .ok_or("invalid analytics loss wave")?;
                    for (player, _) in losses {
                        facts[seat(*player)].placement = Some(remaining + 1);
                    }
                }
                _ => {}
            },
        }
    }
    if !opening_complete {
        return Err("terminal analytics replay has no completed opening".into());
    }
    if let Some(winner) = outcome.winner {
        facts[seat(winner)].placement = Some(1);
    }
    for (&id, card) in &cards {
        let row = &mut facts[seat(card.owner)];
        let aggregate = row.cards.entry(card.oracle_id.clone()).or_default();
        aggregate.opening |= row.opening.contains(&id);
        aggregate.in_hand |= row.in_hand.contains(&id);
        aggregate.land_played |= row.played_lands.contains(&id);
        let (casts, cast_turn_sum) = row.casts.get(&id).copied().unwrap_or_default();
        if row.drawn.contains(&id) {
            aggregate.drawn = true;
            aggregate.drawn_cast |= row.drawn_then_cast.contains(&id);
        }
        aggregate.casts += casts;
        aggregate.cast_turn_sum += cast_turn_sum;
    }
    Ok(facts)
}

fn note_hand(facts: &mut [SeatFacts; 4], movement: &MoveOutcome, draw: DrawKind) {
    if let Zone::Hand { owner } = movement.destination {
        let row = &mut facts[seat(owner)];
        row.in_hand.insert(movement.successor.card);
        match draw {
            DrawKind::Draw => {
                row.drawn.insert(movement.successor.card);
            }
            DrawKind::Unresolved => row.unresolved_draws += 1,
            DrawKind::Acquisition => {}
        }
    }
}

#[derive(Default)]
struct CardStats {
    opening: u64,
    opening_wins: u64,
    in_hand: u64,
    in_hand_wins: u64,
    drawn: u64,
    drawn_cast: u64,
    land_played: u64,
    cast_games: u64,
    casts: u64,
    cast_turn_sum: u64,
}
impl CardStats {
    fn add(&mut self, card: CardFacts, won: bool) {
        self.opening += u64::from(card.opening);
        self.opening_wins += u64::from(card.opening && won);
        self.in_hand += u64::from(card.in_hand);
        self.in_hand_wins += u64::from(card.in_hand && won);
        self.drawn += u64::from(card.drawn);
        self.drawn_cast += u64::from(card.drawn_cast);
        self.land_played += u64::from(card.land_played);
        self.cast_games += u64::from(card.casts > 0);
        self.casts += card.casts;
        self.cast_turn_sum += card.cast_turn_sum;
    }
}

#[derive(Default)]
struct DeckStats {
    attempts: u64,
    completed: u64,
    wins: u64,
    decision_capped: u64,
    budget_capped: u64,
    placements: [u64; 5],
    turn_order_attempts: [u64; 4],
    turn_order_completed: [u64; 4],
    turn_order_wins: [u64; 4],
    turns: u64,
    decisions: u64,
    completed_turns: u64,
    completed_decisions: u64,
    first_commander_cast: BTreeMap<u64, u64>,
    commander_never_cast: u64,
    early: EarlyTurns,
    cards: BTreeMap<String, CardStats>,
    unresolved_draws: u64,
}
impl DeckStats {
    fn add(
        &mut self,
        outcome: &PodOutcome,
        position: usize,
        won: bool,
        rank: Option<usize>,
        facts: Option<SeatFacts>,
    ) {
        self.attempts += 1;
        self.turn_order_attempts[position] += 1;
        self.decision_capped += u64::from(outcome.decision_capped);
        self.budget_capped += u64::from(outcome.budget_capped);
        self.turns += outcome.turns;
        self.decisions += outcome.decisions as u64;
        let Some(facts) = facts else { return };
        self.completed += 1;
        self.completed_turns += outcome.turns;
        self.completed_decisions += outcome.decisions as u64;
        self.wins += u64::from(won);
        self.turn_order_completed[position] += 1;
        self.turn_order_wins[position] += u64::from(won);
        self.placements[facts.placement.or(rank).unwrap_or(0)] += 1;
        if let Some(turn) = facts.first_commander_cast {
            *self.first_commander_cast.entry(turn).or_default() += 1;
        } else {
            self.commander_never_cast += 1;
        }
        self.early.add(facts.early);
        self.unresolved_draws += facts.unresolved_draws;
        for (id, card) in facts.cards {
            self.cards.entry(id).or_default().add(card, won);
        }
    }
}

fn ratio(count: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then(|| count as f64 / denominator as f64)
}
fn rate(count: u64, denominator: u64) -> Value {
    json!({"count": count, "denominator": denominator, "rate": ratio(count, denominator)})
}
fn percent(count: u64, denominator: u64) -> String {
    ratio(count, denominator).map_or_else(|| "—".into(), |n| format!("{:.1}%", n * 100.0))
}
fn average(count: u64, denominator: u64) -> String {
    ratio(count, denominator).map_or_else(|| "—".into(), |n| format!("{n:.2}"))
}
fn markdown_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

pub fn write(
    dest: &Path,
    profile: &AnalyticsProfile<'_>,
    workers: usize,
    seed_start: u32,
    rotation: bool,
    decision_limit: usize,
    input: impl Iterator<Item = (PodOutcome, Option<ReplayInput>)>,
) -> Result<(), String> {
    let catalog = profile.catalog;
    let mut groups = BTreeMap::<String, (Vec<usize>, DeckStats)>::new();
    let mut keys = Vec::with_capacity(4);
    for (slot, deck) in profile.decks.iter().enumerate() {
        let key = digest(&(&deck.commanders, &deck.cards))?;
        let (slots, stats) = groups.entry(key.clone()).or_default();
        slots.push(slot);
        for id in deck.cards.keys() {
            stats.cards.entry(id.clone()).or_default();
        }
        keys.push(key);
    }
    let workers = workers.min(8);
    let mut games = 0;
    let mut completed = 0;
    let mut input = input;
    loop {
        // Bound simultaneous Detailed traces and discard them before processing
        // another wave. Each worker emits aggregates with no private objects.
        let wave: Vec<_> = input.by_ref().take(workers).collect();
        if wave.is_empty() {
            break;
        }
        let measured = std::thread::scope(|scope| {
            let handles: Vec<_> = wave
                .into_iter()
                .map(|(outcome, replay)| {
                    scope.spawn(move || {
                        if outcome.terminal != replay.is_some() {
                            return Err(format!(
                                "pod {} has inconsistent terminal input",
                                outcome.index
                            ));
                        }
                        let facts = replay
                            .map(|replay| replay.analyze(&outcome, catalog))
                            .transpose()
                            .map_err(|error| format!("pod {} analytics: {error}", outcome.index))?;
                        Ok((outcome, facts))
                    })
                })
                .collect();
            let results: Vec<_> = handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .map_err(|_| "analytics worker panicked".to_string())
                        .and_then(|result| result)
                })
                .collect();
            results.into_iter().collect::<Result<Vec<_>, String>>()
        })?;
        for (outcome, facts) in measured {
            games += 1;
            completed += u64::from(outcome.terminal);
            let facts = facts.map_or_else(|| std::array::from_fn(|_| None), |rows| rows.map(Some));
            for (seat, facts) in facts.into_iter().enumerate() {
                let deck = &mut groups.get_mut(&keys[outcome.slots[seat]]).unwrap().1;
                deck.add(
                    &outcome,
                    (seat + 4 - outcome.starter) % 4,
                    outcome.winner == Some(PlayerId(seat as u8)),
                    outcome.placements[seat],
                    facts,
                );
            }
        }
    }
    let (decks, summary) = reports(profile, catalog, groups, games, completed)?;
    let json = serde_json::to_vec_pretty(&json!({
        "schema": "deck-analytics-v1",
        "profile": profile.name,
        "build_id": profile.build.id,
        "catalog_sha256": profile.catalog_sha256,
        "seed_start": seed_start, "rotate_seats": rotation, "decision_limit": decision_limit,
        "pods": games, "verified_terminal_replays": completed, "analytics_workers": workers,
        "definitions": {
            "win_rates": "Distinct completed pod seats. Rules draws count as nonwins. Unfinished seats contribute to cap/unfinished and observed length only.",
            "placement": "Ranks 1–4 from committed loss waves. Simultaneous losses tie. Bucket 0 is unplaced.",
            "opening": "Final kept opening cards after redraws and bottoming, including cards used for pregame actions.",
            "in_hand": "Opening cards and subsequent entries into that seat's hand, including draws, searches, and returns.",
            "drawn": "Post-setup draws into hand. Search, library selection, and look-top placement are not draws. Unclassified library entries are reported separately.",
            "cast": "Committed spell casts attributed to their casting seat and physical owner. Drawn cast metrics require a later committed cast of the same physical card. Reversals, copies, land plays and activations are excluded.",
            "turn": "Global turn number for casts. Early turns use each seat's first four observed completed turns.",
            "mana": "Among completed early turns with a nonland spell sampled at own empty-stack main priority, none paid its printed mana cost in any sampled window. Color-blocked turns had enough mana in the color-relaxed proxy. These are planner estimates.",
            "seat": "Seat order is relative to the starting player. Bench rotates deck slots around that fixed starting seat per seed."
        },
        "decks": decks,
    })).map_err(|e| e.to_string())?;
    fs::create_dir(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    fs::write(dest.join("analytics.json"), json).map_err(|e| e.to_string())?;
    fs::write(dest.join("summary.md"), summary).map_err(|e| e.to_string())
}

fn reports(
    profile: &AnalyticsProfile<'_>,
    catalog: &Catalog,
    groups: BTreeMap<String, (Vec<usize>, DeckStats)>,
    games: u64,
    completed: u64,
) -> Result<(Vec<Value>, String), String> {
    let mut summary = format!(
        "# Commander deck analytics\n\n{} · {completed}/{games} pods completed and replayed. Rates use completed seats unless stated otherwise. Early mana flags are estimates.\n\n",
        markdown_cell(profile.name)
    );
    summary.push_str("| Deck | Seats | Win | Mean rank | Unfinished | Decision cap | Mean turns | Mean decisions | Missed land | Mana blocked | Color blocked |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut seats_md = String::from(
        "\n| Deck | Seat 0 win | Seat 1 win | Seat 2 win | Seat 3 win | Mean first commander cast | Never cast commander |\n|---|---:|---:|---:|---:|---:|---:|\n",
    );
    let mut cards_md = String::new();
    let mut decks = Vec::with_capacity(groups.len());
    for (key, (slots, stats)) in groups {
        let commanders = &profile.decks[slots[0]].commanders;
        let names = commanders
            .iter()
            .map(|id| {
                catalog
                    .definition_by_oracle_id(id)
                    .map(|definition| definition.name.as_str())
                    .ok_or("missing commander definition")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let label = format!(
            "{} (slots {})",
            markdown_cell(&names.join(" + ")),
            slots
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let ranks = stats.placements.iter().skip(1).sum();
        let rank_sum = stats
            .placements
            .iter()
            .enumerate()
            .map(|(rank, n)| rank as u64 * n)
            .sum();
        let unfinished = stats.attempts - stats.completed;
        let early = &stats.early;
        writeln!(
            summary,
            "| {label} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            stats.attempts,
            percent(stats.wins, stats.completed),
            average(rank_sum, ranks),
            percent(unfinished, stats.attempts),
            percent(stats.decision_capped, stats.attempts),
            average(stats.completed_turns, stats.completed),
            average(stats.completed_decisions, stats.completed),
            percent(early.missed_land_drop, early.completed),
            percent(early.mana_blocked, early.with_spell_probe),
            percent(early.color_blocked, early.with_spell_probe)
        )
        .unwrap();
        let first_casts: u64 = stats.first_commander_cast.values().sum();
        let first_cast_turn_sum = stats
            .first_commander_cast
            .iter()
            .map(|(turn, count)| turn * count)
            .sum();
        let seat_wr = std::array::from_fn::<_, 4, _>(|seat| {
            percent(
                stats.turn_order_wins[seat],
                stats.turn_order_completed[seat],
            )
        });
        writeln!(
            seats_md,
            "| {label} | {} | {} | {} | {} | {} | {} |",
            seat_wr[0],
            seat_wr[1],
            seat_wr[2],
            seat_wr[3],
            average(first_cast_turn_sum, first_casts),
            percent(stats.commander_never_cast, stats.completed)
        )
        .unwrap();
        writeln!(cards_md, "\n## {label}\n\n| Card | OH WR | GIH WR | Games drawn | Cast rate | Drawn never cast | Avg turn cast | Land played |\n|---|---:|---:|---:|---:|---:|---:|---:|").unwrap();
        let mut cards = BTreeMap::new();
        let mut ordered_cards: Vec<_> = stats
            .cards
            .into_iter()
            .map(|(id, card)| {
                let definition = catalog
                    .definition_by_oracle_id(&id)
                    .ok_or("missing analytics card definition")?;
                Ok::<_, String>((
                    &definition.name,
                    id,
                    card,
                    definition.characteristics.has_type(CardKind::Land),
                ))
            })
            .collect::<Result<_, _>>()?;
        ordered_cards.sort_unstable_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        for (name, id, card, land) in ordered_cards {
            let dead = card.drawn - card.drawn_cast;
            let cast_samples = if land { 0 } else { stats.completed };
            let drawn_samples = if land { 0 } else { card.drawn };
            let dead = if land { 0 } else { dead };
            writeln!(
                cards_md,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                markdown_cell(name),
                percent(card.opening_wins, card.opening),
                percent(card.in_hand_wins, card.in_hand),
                card.drawn,
                percent(card.cast_games, cast_samples),
                percent(dead, drawn_samples),
                average(card.cast_turn_sum, card.casts),
                card.land_played
            )
            .unwrap();
            cards.insert(
                id,
                json!({
                    "name": name, "opening_hand_win_rate": rate(card.opening_wins, card.opening),
                    "games_in_hand_win_rate": rate(card.in_hand_wins, card.in_hand),
                    "games_drawn": card.drawn, "cast_rate": rate(card.cast_games, cast_samples),
                    "cast_when_drawn_rate": rate(card.drawn_cast, drawn_samples),
                    "drawn_never_cast_rate": rate(dead, drawn_samples),
                    "casts": card.casts, "average_turn_cast": ratio(card.cast_turn_sum, card.casts),
                    "games_land_played": card.land_played,
                }),
            );
        }
        decks.push(json!({
            "deck_sha256": key, "profile_slots": slots, "commander_oracle_ids": commanders,
            "commanders": names, "attempts": stats.attempts, "completed": stats.completed,
            "win_rate": rate(stats.wins, stats.completed),
            "unfinished_rate": rate(unfinished, stats.attempts),
            "decision_cap_rate": rate(stats.decision_capped, stats.attempts),
            "budget_cap_rate": rate(stats.budget_capped, stats.attempts),
            "placement_counts_0_to_4": stats.placements, "expected_placement": ratio(rank_sum, ranks),
            "seat_order": (0..4).map(|position| json!({
                "seat": position, "attempts": stats.turn_order_attempts[position],
                "win_rate": rate(stats.turn_order_wins[position], stats.turn_order_completed[position]),
            })).collect::<Vec<_>>(),
            "average_game_turns": ratio(stats.completed_turns, stats.completed),
            "average_game_decisions": ratio(stats.completed_decisions, stats.completed),
            "average_observed_turns": ratio(stats.turns, stats.attempts),
            "average_observed_decisions": ratio(stats.decisions, stats.attempts),
            "first_commander_cast_by_turn": stats.first_commander_cast,
            "commander_never_cast_in_completed_game": stats.commander_never_cast,
            "early_turns": early,
            "early_missed_land_drop_rate": rate(early.missed_land_drop, early.completed),
            "early_uncastable_hand_rate": rate(early.mana_blocked, early.with_spell_probe),
            "early_color_blocked_hand_rate": rate(early.color_blocked, early.with_spell_probe),
            "unclassified_library_to_hand_moves": stats.unresolved_draws,
            "cards_by_oracle_id": cards,
        }));
    }
    summary.push_str(&seats_md);
    summary.push_str(&cards_md);
    summary.push_str("\nOH WR uses the final kept opener. GIH WR includes any later hand entry. Drawn metrics use post-setup draws and require a later cast of the same physical copy. Land plays have a separate count and land spell-cast rates are undefined.\n");
    Ok((decks, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_decks_keep_both_commanders_and_merge_rows() {
        let row = |id: &str, quantity, role| DynamicDeckEntry {
            oracle_id: id.into(),
            quantity,
            role,
        };
        let deck = AnalyticsDeck::dynamic(&[
            row("b", 1, DeckRole::Commander),
            row("x", 2, DeckRole::Main),
            row("a", 1, DeckRole::Commander),
            row("x", 3, DeckRole::Main),
        ]);
        assert_eq!(deck.commanders, ["a", "b"]);
        assert_eq!(
            deck.cards.into_iter().collect::<Vec<_>>(),
            [("a".into(), 1), ("b".into(), 1), ("x".into(), 5)]
        );
    }
}

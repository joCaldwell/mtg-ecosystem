//! Library verbs: search, shuffle, scry and surveil (CR 701.22-701.25).
//!
//! The acting player looks at hidden library cards through one resolution
//! choice. Only that player sees them (`visible_to`): a searcher sees the
//! whole searched library (CR 701.23a) while only matching cards are
//! selectable, so no other seat learns an identity, a candidate count or an
//! order. Candidates follow object order; view rows, like every permitted
//! view object, follow handle order (handles are minted in object order), so
//! neither follows library position. Printed reveals happen once, in the
//! library; a found commander bound for its owner's hand then offers the
//! command zone (CR 903.9b), and the found cards move together. Revealed cards in their owner's hand stay
//! publicly counted by Oracle identity (CR 701.23e). A departed actor's
//! library left the game with them (CR 800.4a), so their pending choice is
//! skipped.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;
use crate::engine::battlefield_entry::EntryChoice;
use mtg_contracts::kir::{self, Destination, Node, PlayerExpr, SearchSpec, SelectionZone};

/// The bounds of a search: the cards that may be found, in object order, and
/// how many must and may be found (CR 701.23b, d).
pub(in crate::engine) struct SearchBounds {
    pub candidates: Vec<ObjectId>,
    pub minimum: usize,
    pub maximum: usize,
    /// The number of cards in the searched library or chosen-from hand.
    pub library: usize,
    /// The zone is public, so only an open choice is asked.
    pub public: bool,
}

impl SearchBounds {
    /// CR 701.23a: the searcher looks at every card of a non-empty library,
    /// even when none matches. Only a search that must find every card,
    /// including an empty library, has nothing to decide. A hand choice is
    /// likewise asked whenever the hand holds a card the choice may leave,
    /// so no other seat learns whether a card matched (CR 402.3). A
    /// graveyard choice is asked only when it leaves something to decide.
    pub fn needs_choice(&self) -> bool {
        if self.public {
            return self.minimum < self.candidates.len();
        }
        self.library > 0
            && (self.minimum < self.candidates.len() || self.candidates.len() < self.library)
    }
}

/// What a battlefield search asks next (CR 303.4f, 614.1c).
pub(in crate::engine) enum EntryQuestion {
    /// The host a found Aura enters attached to.
    Attach {
        aura: ObjectId,
        hosts: Vec<ObjectId>,
    },
    /// Whether to pay a found entrant's entry life cost.
    Pay { entrant: ObjectId },
    /// Which hand card, if any, to reveal for a found entrant.
    Reveal {
        entrant: ObjectId,
        cards: Vec<ObjectId>,
    },
}

/// CR 701.20a, 614.12: each asked entrant's reveal, `None` when declined.
type EntryReveals = Vec<(ObjectId, Option<ObjectId>)>;

impl RulesTxn {
    /// The admitted library verb at this frame's cursor.
    fn library_verb(&self, frame: &Resolving) -> Result<(Verb, u32), String> {
        match self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::Kernel {
                node: Node::Do(verb),
                source_block,
            }) => Ok((verb.clone(), *source_block)),
            _ => Err("missing library instruction".into()),
        }
    }

    /// The live player whose library the verb acts on, or none when that
    /// player has left the game (CR 800.4a).
    fn library_actor(
        &self,
        frame: &Resolving,
        player: &PlayerExpr,
    ) -> Result<Option<PlayerId>, String> {
        Ok(match self.kernel_players(frame, player)? {
            Players::One(player) if self.is_live(player) => Some(player),
            Players::One(_) | Players::Nobody => None,
            Players::EachOpponent => return Err("library verb for each opponent".into()),
        })
    }

    /// CR 701.23a-d: the cards of `player`'s library matching the search's
    /// printed characteristics, and the found-card bounds.
    pub(in crate::engine) fn search_bounds(
        &self,
        player: PlayerId,
        search: &SearchSpec,
    ) -> Result<SearchBounds, String> {
        let mut candidates = match search.from {
            SelectionZone::Library => self
                .state
                .rules
                .libraries
                .get(&player)
                .into_iter()
                .flatten()
                .map(|card| self.object_id(*card).ok_or("missing library card"))
                .collect::<Result<Vec<_>, _>>()?,
            // CR 400.1: the chooser's own hand or graveyard, in object order.
            SelectionZone::Hand | SelectionZone::Graveyard => {
                let zone = Self::selection_zone(search.from, player);
                self.state
                    .rules
                    .objects
                    .values()
                    .filter(|object| object.zone == zone)
                    .map(|object| object.id)
                    .collect()
            }
        };
        let library = candidates.len();
        candidates.retain(|id| {
            self.current(*id).is_ok_and(|object| {
                crate::query::matches_pred(&self.object_facts(object), player, &search.filter)
                    .is_yes()
            })
        });
        // The order of a library is hidden even from the searcher.
        candidates.sort();
        let maximum = search
            .maximum
            .as_fixed()
            .and_then(Quantity::to_u32)
            .ok_or("search maximum has no audited evaluation")? as usize;
        // CR 701.23b, d: a library search for a stated quality may fail to
        // find. A hand or graveyard choice takes as many as it can unless
        // it is optional (CR 608.2d).
        let minimum = match search.from {
            SelectionZone::Library if !search.filter.0.is_empty() => 0,
            SelectionZone::Hand | SelectionZone::Graveyard if search.optional => 0,
            _ => maximum.min(candidates.len()),
        };
        Ok(SearchBounds {
            maximum: maximum.min(candidates.len()),
            candidates,
            minimum,
            library,
            public: search.from == SelectionZone::Graveyard,
        })
    }

    /// The zone of `player` a search or card choice selects from.
    fn selection_zone(from: SelectionZone, player: PlayerId) -> Zone {
        match from {
            SelectionZone::Library => Zone::Library { owner: player },
            SelectionZone::Hand => Zone::Hand { owner: player },
            SelectionZone::Graveyard => Zone::Graveyard { owner: player },
        }
    }

    /// The zone the search or card choice at this frame's cursor selects
    /// `player`'s cards from.
    pub(in crate::engine) fn search_source(
        &self,
        frame: &Resolving,
        player: PlayerId,
    ) -> Result<Zone, String> {
        let (Verb::Search { search, .. }, _) = self.library_verb(frame)? else {
            return Err("search continuation lost its instruction".into());
        };
        Ok(Self::selection_zone(search.from, player))
    }

    pub(in crate::engine) fn kernel_library(
        &mut self,
        frame: Resolving,
        verb: Verb,
        source_block: u32,
    ) -> Result<(), String> {
        let player = match &verb {
            Verb::Search { player, .. }
            | Verb::Shuffle { player }
            | Verb::ReorderLibraryTop { player, .. }
            | Verb::LookChooseHand { player, .. }
            | Verb::Scry { player, .. }
            | Verb::Surveil { player, .. } => player.clone(),
            _ => return Err("not a library verb".into()),
        };
        // CR 608.2d: sampled with the seeded match randomness; the sampler
        // owns its progress and the departure of its controller.
        if matches!(&verb, Verb::Search { search, .. } if search.random) {
            return self.random_discard_step(frame);
        }
        let Some(actor) = self.library_actor(&frame, &player)? else {
            let outcome = match self.kernel_players(&frame, &player)? {
                Players::One(player) => InstructionOutcome::PlayerActionSkipped { player },
                _ => InstructionOutcome::Effect { actual: None },
            };
            return self.finish_instruction(frame, outcome);
        };
        match verb {
            Verb::Shuffle { .. } => {
                let actual = self.commit(
                    ProposedEvent::ShuffleLibrary { player: actor },
                    self.instruction_cause(&frame, source_block)?,
                )?;
                self.finish_instruction(
                    frame,
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    },
                )
            }
            Verb::Search { search, .. } => {
                let bounds = self.search_bounds(actor, &search)?;
                // CR 701.23a, d: nothing to choose when every card must be
                // found, including from an empty library.
                if !bounds.needs_choice() {
                    return self.complete_search(frame, actor, bounds.candidates, source_block);
                }
                self.rules_mut().frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Search {
                        player: actor,
                        source_block,
                    },
                };
                Ok(())
            }
            Verb::ReorderLibraryTop { ref amount, .. } => {
                let looked = self.library_prefix(actor, &self.kernel_amount(&frame, amount)?)?;
                if looked.is_empty() {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                // Even a one-card prefix has a mandatory private look.
                self.rules_mut().frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::ReorderLibraryTop {
                        player: actor,
                        looked,
                        source_block,
                    },
                };
                Ok(())
            }
            Verb::LookChooseHand { ref amount, .. } => {
                let looked = self.library_prefix(actor, &self.kernel_amount(&frame, amount)?)?;
                if looked.is_empty() {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                self.rules_mut().frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::LookChooseHand {
                        player: actor,
                        looked,
                        source_block,
                    },
                };
                Ok(())
            }
            Verb::Scry { ref amount, .. } | Verb::Surveil { ref amount, .. } => {
                let amount = self.kernel_amount(&frame, amount)?;
                let looked = self.library_prefix(actor, &amount)?;
                // CR 701.22b, d, 701.25c, d: a positive scry or surveil of
                // an empty library is still that event, with nothing moved.
                if looked.is_empty() {
                    let event = if matches!(verb, Verb::Scry { .. }) {
                        ProposedEvent::Scry {
                            player: actor,
                            looked,
                            top: vec![],
                            bottom: vec![],
                        }
                    } else {
                        ProposedEvent::Surveil {
                            player: actor,
                            looked,
                            top: vec![],
                            graveyard: vec![],
                        }
                    };
                    return self.commit_look(frame, event, source_block);
                }
                let choice = if matches!(verb, Verb::Scry { .. }) {
                    ResolutionChoice::Scry {
                        player: actor,
                        looked,
                        bottom: None,
                        top: None,
                        source_block,
                    }
                } else {
                    ResolutionChoice::Surveil {
                        player: actor,
                        looked,
                        graveyard: None,
                        source_block,
                    }
                };
                self.rules_mut().frame = Frame::ResolutionChoice { frame, choice };
                Ok(())
            }
            _ => Err("not a library verb".into()),
        }
    }

    /// The audited search at this frame's cursor and the zone its found
    /// cards go to; `None` is the battlefield, tapped (CR 110.2, 614.1c).
    fn search_destination(&self, frame: &Resolving) -> Result<(SearchSpec, Option<Zone>), String> {
        let (verb, _) = self.library_verb(frame)?;
        let Verb::Search { player, search } = verb else {
            return Err("search continuation lost its instruction".into());
        };
        let Players::One(player) = self.kernel_players(frame, &player)? else {
            return Err("search without one player".into());
        };
        let to = match search.to {
            Destination::Battlefield { tapped, .. } => (!tapped).then_some(Zone::Battlefield),
            Destination::OneTappedRestOwnerHand => None,
            Destination::OwnerHand => Some(Zone::Hand { owner: player }),
            Destination::OwnerGraveyard => Some(Zone::Graveyard { owner: player }),
            _ => return Err("search destination has no audited executor".into()),
        };
        Ok((search, to))
    }

    /// CR 903.9b: found commanders bound for their owner's hand, in object
    /// order, each of whose owner may choose the command zone instead.
    fn found_commanders(
        &self,
        frame: &Resolving,
        found: &[ObjectId],
    ) -> Result<Vec<ObjectId>, String> {
        let (search, _) = self.search_destination(frame)?;
        Ok(Self::search_hand_cards(&search.to, found)
            .iter()
            .copied()
            .filter(|o| self.state.rules.commanders.history(o.card).is_some())
            .collect())
    }

    fn search_hand_cards<'a>(destination: &Destination, found: &'a [ObjectId]) -> &'a [ObjectId] {
        match destination {
            Destination::OwnerHand => found,
            Destination::OneTappedRestOwnerHand => found.get(1..).unwrap_or(&[]),
            _ => &[],
        }
    }

    fn entry_cards<'a>(destination: &Destination, found: &'a [ObjectId]) -> &'a [ObjectId] {
        if *destination == Destination::OneTappedRestOwnerHand {
            &found[..found.len().min(1)]
        } else {
            found
        }
    }

    /// CR 701.23a, e, 903.9b: reveal printed found cards once, still in the
    /// library, then ask each found commander's owner where it goes before
    /// the found cards move together.
    pub(in crate::engine) fn complete_search(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        // One canonical order for every continuation that saves the set.
        let mut found = found;
        found.sort();
        let (Verb::Search { search, .. }, _) = self.library_verb(&frame)? else {
            return Err("missing search instruction".into());
        };
        if search.reveal && !found.is_empty() {
            self.commit(
                ProposedEvent::RevealFound {
                    player,
                    objects: found.clone(),
                },
                self.instruction_cause(&frame, source_block)?,
            )?;
        }
        if search.to == Destination::OwnerLibraryTop {
            let top = match found.as_slice() {
                [] => None,
                [object] => Some(*object),
                _ => return Err("a library-top search finds at most one card".into()),
            };
            let actual = self.commit(
                ProposedEvent::ShuffleLibraryToTop { player, found: top },
                self.instruction_cause(&frame, source_block)?,
            )?;
            if let Some(object) = top.filter(|_| search.reveal) {
                self.remember_revealed_library_top(object);
            }
            return self.finish_instruction(
                frame,
                InstructionOutcome::Effect {
                    actual: Some(actual),
                },
            );
        }
        if search.to == Destination::OneTappedRestOwnerHand
            && let [first, second] = found.as_slice()
        {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::SearchSplit {
                    player,
                    found: [*first, *second],
                    source_block,
                },
            };
            return Ok(());
        }
        self.route_found_cards(frame, player, found, source_block)
    }

    fn route_found_cards(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let remaining = self.found_commanders(&frame, &found)?;
        self.continue_search(frame, player, found, remaining, vec![], source_block)
    }

    /// Offer the next found commander its command-zone choice, or commit
    /// the search once every commander's owner has chosen.
    pub(in crate::engine) fn continue_search(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    ) -> Result<(), String> {
        // CR 800.4a: the searcher's library left the game with them.
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if let Some(object) = remaining.first().copied() {
            self.rules_mut().frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand { owner: player },
                continuation: CommanderContinuation::Search {
                    frame,
                    player,
                    found,
                    remaining,
                    choices,
                    source_block,
                },
            };
            return Ok(());
        }
        // CR 303.4f, 614.1c: an entry onto the battlefield first collects
        // each found Aura's host and each entrant's entry life payment.
        if self.search_onto_battlefield(&frame)? {
            return self.continue_card_entry(
                frame,
                player,
                found,
                choices,
                vec![],
                vec![],
                vec![],
                source_block,
            );
        }
        self.commit_card_moves(frame, player, found, &choices, &[], &[], &[], source_block)
    }

    /// The admitted search puts its found cards onto the battlefield,
    /// tapped or not.
    fn search_onto_battlefield(&self, frame: &Resolving) -> Result<bool, String> {
        Ok(matches!(
            self.search_destination(frame)?.0.to,
            Destination::Battlefield { .. } | Destination::OneTappedRestOwnerHand
        ))
    }

    /// CR 303.4f, 119.4, 614.1c, 614.12: ask the next entry question for found or
    /// targeted cards, then move the entrants together.
    pub(in crate::engine) fn continue_card_entry(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        command_choices: Vec<(ObjectId, bool)>,
        hosts: Vec<(ObjectId, ObjectId)>,
        paid: Vec<(ObjectId, bool)>,
        revealed: EntryReveals,
        source_block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if self
            .card_entry_question(&frame, player, &found, &hosts, &paid, &revealed)?
            .is_some()
        {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::SearchEntry {
                    player,
                    found,
                    command_choices,
                    hosts,
                    paid,
                    revealed,
                    source_block,
                },
            };
            return Ok(());
        }
        self.commit_card_moves(
            frame,
            player,
            found,
            &command_choices,
            &hosts,
            &paid,
            &revealed,
            source_block,
        )
    }

    pub(in crate::engine) fn card_entry_question(
        &self,
        frame: &Resolving,
        player: PlayerId,
        found: &[ObjectId],
        hosts: &[(ObjectId, ObjectId)],
        paid: &[(ObjectId, bool)],
        revealed: &[(ObjectId, Option<ObjectId>)],
    ) -> Result<Option<EntryQuestion>, String> {
        let entrants = match self.library_verb(frame)?.0 {
            Verb::Search { search, .. } => Self::entry_cards(&search.to, found),
            Verb::Move {
                to: Destination::Battlefield { .. },
                ..
            } => found,
            _ => return Err("entry choice lost its instruction".into()),
        };
        self.next_entry_question(player, entrants, hosts, paid, revealed)
    }

    /// CR 304.4, 307.4: an instant or sorcery never enters the battlefield.
    pub(in crate::engine) fn may_enter_battlefield(&self, card: CardId) -> Result<bool, String> {
        let kind = self.definition(card)?.characteristics.kind;
        Ok(!matches!(kind, CardKind::Instant | CardKind::Sorcery))
    }

    /// CR 303.4f: the objects a found Aura could enchant as it enters: on
    /// the battlefield already, never chosen as a target (Hexproof and
    /// Shroud do not apply), but never one with protection from it. `None`
    /// for a card that is no Aura.
    pub(in crate::engine) fn entry_hosts(
        &self,
        aura: ObjectId,
    ) -> Result<Option<Vec<ObjectId>>, String> {
        let characteristics = self.structural_characteristics(aura)?;
        if characteristics.kind != CardKind::Enchantment || characteristics.subtypes != ["Aura"] {
            return Ok(None);
        }
        let mut hosts = vec![];
        for object in self.state.rules.objects.values() {
            if object.id != aura
                && object.present_on_battlefield()
                && self.is_live(object.controller)
                && self.attachment_target_type_valid(aura, object.id)?
                && self.protected_from(object.id, &characteristics) == Ok(false)
            {
                hosts.push(object.id);
            }
        }
        hosts.sort();
        Ok(Some(hosts))
    }

    /// CR 303.4a: check whether a departed host could have been legal.
    /// Replay authenticates the selected host and its layered types.
    fn historical_host_compatible(&self, aura: ObjectId, host: ObjectId) -> Result<bool, String> {
        let printed = &self.copiable_definition(host)?.characteristics;
        Ok(self
            .copiable_definition(aura)?
            .spell
            .as_ref()
            .and_then(|spell| spell.targets.first())
            .is_some_and(|spec| match spec.domain {
                TargetDomain::BattlefieldCreature => self.historical_creature_possible(printed),
                TargetDomain::BattlefieldPermanent => spec.filter.as_ref().is_some_and(|filter| {
                    filter.types.is_empty()
                        || filter.types.iter().any(|kind| printed.has_type(*kind))
                }),
                _ => false,
            }))
    }

    /// CR 303.4f-g, 304.4, 307.4: the found cards that enter, each Aura on
    /// its chosen host; the others stay in the library.
    fn search_entrants(
        &self,
        found: &[ObjectId],
        hosts: &[(ObjectId, ObjectId)],
    ) -> Result<Vec<(ObjectId, Option<ObjectId>)>, String> {
        let mut entrants = vec![];
        for object in found {
            if !self.may_enter_battlefield(object.card)? {
                continue;
            }
            match self.entry_hosts(*object)? {
                None => entrants.push((*object, None)),
                // CR 303.4g: a chosen host that has since left (its
                // controller conceded) leaves the Aura in its original zone.
                Some(legal) => {
                    if let Some((_, host)) = hosts.iter().find(|(aura, _)| aura == object)
                        && legal.contains(host)
                    {
                        entrants.push((*object, Some(*host)));
                    }
                }
            }
        }
        Ok(entrants)
    }

    /// The next entry question of a battlefield search: first each found
    /// Aura's host, in object order (CR 303.4f; an Aura with no legal host
    /// is never asked and stays in the library, CR 303.4g), then each
    /// entrant whose entry life cost its controller can still pay after
    /// the payments already chosen (CR 119.4, 614.1c), then each entrant
    /// with an entry reveal, whether or not a hand card matches (CR
    /// 701.20a). `hosts`, `paid` and `revealed` must be exactly the answers
    /// given so far, in that order.
    fn next_entry_question(
        &self,
        player: PlayerId,
        found: &[ObjectId],
        hosts: &[(ObjectId, ObjectId)],
        paid: &[(ObjectId, bool)],
        revealed: &[(ObjectId, Option<ObjectId>)],
    ) -> Result<Option<EntryQuestion>, String> {
        let mut attached = hosts.iter().peekable();
        for object in found {
            if !self.may_enter_battlefield(object.card)? {
                continue;
            }
            let Some(legal) = self.entry_hosts(*object)? else {
                continue;
            };
            // An answered Aura stays answered, even if its host has left
            // since (it then stays in the library at entry, CR 303.4g). A
            // host still here is exactly the legal incarnation chosen; one
            // gone left with its departed owner and could have been chosen.
            if let Some((_, host)) = attached.next_if(|(aura, _)| aura == object) {
                let chosen = match self.state.rules.objects.get(&host.card) {
                    Some(current) => current.id == *host && legal.contains(host),
                    None => {
                        self.state
                            .rules
                            .cards
                            .get(&host.card)
                            .is_some_and(|card| !self.is_live(card.owner))
                            && self.historical_host_compatible(*object, *host)?
                    }
                };
                if !chosen {
                    return Err("search entry host invariant".into());
                }
                continue;
            }
            // CR 303.4g: no legal host, so nothing to ask; this includes
            // the last host leaving while the question was open.
            if legal.is_empty() {
                continue;
            }
            if attached.peek().is_some() {
                return Err("search entry attachment order".into());
            }
            if !paid.is_empty() || !revealed.is_empty() {
                return Err("search entry payment before an attachment".into());
            }
            return Ok(Some(EntryQuestion::Attach {
                aura: *object,
                hosts: legal,
            }));
        }
        if attached.next().is_some() {
            return Err("search entry attachment without an Aura".into());
        }
        let mut committed = Quantity::zero();
        let mut answered = paid.iter();
        for (object, _) in self.search_entrants(found, hosts)? {
            let Some(amount) = self.entry_life_cost(object)? else {
                continue;
            };
            let mut total = committed.clone();
            total.add(&amount);
            if !self.can_pay_entry_life(player, &total) {
                continue;
            }
            match answered.next() {
                Some((asked, pays)) if *asked == object => {
                    if *pays {
                        committed = total;
                    }
                }
                Some(_) => return Err("search entry payment order".into()),
                None if revealed.is_empty() => {
                    return Ok(Some(EntryQuestion::Pay { entrant: object }));
                }
                None => return Err("search entry reveal before a payment".into()),
            }
        }
        if answered.next().is_some() {
            return Err("search entry payment without an entrant".into());
        }
        let mut answered = revealed.iter();
        for (object, _) in self.search_entrants(found, hosts)? {
            let Some(EntryChoice::Reveal(filter)) = self.entry_choice(object)? else {
                continue;
            };
            // Asked even with no matching card, so the hidden hand stays
            // hidden (CR 400.2).
            let cards = self.entry_reveal_candidates(player, object, &filter);
            match answered.next() {
                Some((asked, card)) if *asked == object => {
                    if card.is_some_and(|card| !cards.contains(&card)) {
                        return Err("search entry reveal is not a qualifying card".into());
                    }
                }
                Some(_) => return Err("search entry reveal order".into()),
                None => {
                    return Ok(Some(EntryQuestion::Reveal {
                        entrant: object,
                        cards,
                    }));
                }
            }
        }
        if answered.next().is_some() {
            return Err("search entry reveal without an entrant".into());
        }
        Ok(None)
    }

    /// The saved answers of a battlefield search are, structurally, its own
    /// whether or not the searcher is still in the game: found cards in
    /// strictly increasing object order, distinct cards of the searcher's
    /// library within the maximum, and hosts then payments answered for
    /// found cards in that order, each at most once, with no earlier
    /// payable entrant skipped (life is read even after a departure).
    fn valid_entry_answers(
        &self,
        player: PlayerId,
        maximum: usize,
        found: &[ObjectId],
        hosts: &[(ObjectId, ObjectId)],
        paid: &[(ObjectId, bool)],
        revealed: &[(ObjectId, Option<ObjectId>)],
    ) -> Result<bool, String> {
        let owned = |o: &ObjectId| {
            self.state
                .rules
                .cards
                .get(&o.card)
                .is_some_and(|card| card.owner == player)
        };
        let ordered = |ids: &[ObjectId]| {
            ids.windows(2).all(|w| w[0] < w[1]) && ids.iter().all(|o| found.contains(o))
        };
        let auras: Vec<_> = hosts.iter().map(|(aura, _)| *aura).collect();
        for (aura, host) in hosts {
            let printed = &self.definition(aura.card)?.characteristics;
            if printed.kind != CardKind::Enchantment || printed.subtypes != ["Aura"] {
                return Ok(false);
            }
            // CR 303.4f: the chosen host is still the exact permanent the
            // Aura could enchant, or left with its departed owner; read by
            // printed definitions, so this holds after the searcher left.
            let chosen = match self.state.rules.objects.get(&host.card) {
                Some(current) => {
                    current.id == *host
                        && current.present_on_battlefield()
                        && self.attachment_target_type_valid(*aura, *host)?
                        && self.protected_from(*host, printed) == Ok(false)
                }
                None => {
                    self.state
                        .rules
                        .cards
                        .get(&host.card)
                        .is_some_and(|card| !self.is_live(card.owner))
                        && self.historical_host_compatible(*aura, *host)?
                }
            };
            if !chosen {
                return Ok(false);
            }
        }
        let asked: Vec<_> = paid.iter().map(|(o, _)| *o).collect();
        let shown: Vec<_> = revealed.iter().map(|(o, _)| *o).collect();
        for (entrant, card) in revealed {
            let reveals = self
                .definition(entrant.card)?
                .static_abilities
                .iter()
                .any(|a| matches!(a.effect, StaticEffect::RevealOrEnterTapped { .. }));
            if !reveals
                || card.is_some_and(|card| card.card == entrant.card || found.contains(&card))
            {
                return Ok(false);
            }
        }
        if found.is_empty()
            || found.len() > maximum
            || !found
                .windows(2)
                .all(|w| w[0].card < w[1].card && w[0] < w[1])
            || !found.iter().all(owned)
            || !ordered(&auras)
            || !ordered(&asked)
            || !ordered(&shown)
            || hosts
                .iter()
                .any(|(aura, host)| aura == host || found.contains(host))
        {
            return Ok(false);
        }
        // No answered payment may skip an earlier entrant whose cost the
        // remaining life covered (CR 119.4), by the printed definitions.
        let mut committed = Quantity::zero();
        let mut answered = paid.iter().peekable();
        let life = self
            .state
            .rules
            .players
            .get(usize::from(player.0))
            .ok_or("missing searcher")?;
        for object in found {
            let Some(next) = answered.peek() else {
                break;
            };
            let definition = self.definition(object.card)?;
            let Some(amount) = definition
                .static_abilities
                .iter()
                .find_map(|a| match &a.effect {
                    StaticEffect::PayLifeOrEnterTapped { amount } => Some(amount.clone()),
                    _ => None,
                })
            else {
                continue;
            };
            let mut total = committed.clone();
            total.add(&amount);
            // Only an affordable payment is ever asked (CR 119.4), and an
            // affordable one is never skipped.
            let affordable = life.can_pay_life(&total);
            if next.0 == *object {
                if !affordable {
                    return Ok(false);
                }
                if next.1 {
                    committed = total;
                }
                answered.next();
            } else if affordable {
                return Ok(false);
            }
        }
        Ok(answered.next().is_none())
    }

    /// CR 701.23a, e, 110.2, 303.4f, 614.1c, 903.9b: move the (already
    /// revealed, when printed) found cards together, each commander to the
    /// zone its owner chose. A battlefield search pays the chosen entry
    /// life first, then enters its entrants in one simultaneous batch, each
    /// Aura attached to its chosen host, tapped when printed (a shockland
    /// paid for still enters tapped then). Revealed cards that reach the
    /// hand stay publicly counted there by Oracle identity.
    fn commit_card_moves(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        choices: &[(ObjectId, bool)],
        hosts: &[(ObjectId, ObjectId)],
        paid: &[(ObjectId, bool)],
        revealed: &[(ObjectId, Option<ObjectId>)],
        source_block: u32,
    ) -> Result<(), String> {
        let (destination, to, reveal) = match self.library_verb(&frame)?.0 {
            Verb::Move {
                to: to @ Destination::Battlefield { tapped, .. },
                ..
            } => (to, (!tapped).then_some(Zone::Battlefield), false),
            _ => {
                let (search, to) = self.search_destination(&frame)?;
                (search.to, to, search.reveal)
            }
        };
        let battlefield = matches!(
            destination,
            Destination::Battlefield { .. } | Destination::OneTappedRestOwnerHand
        );
        let tapped = battlefield && to.is_none();
        let split_hand = if destination == Destination::OneTappedRestOwnerHand {
            Self::search_hand_cards(&destination, &found)
        } else {
            &[]
        };
        let entering: Vec<(ObjectId, Option<ObjectId>)> = if battlefield {
            self.search_entrants(Self::entry_cards(&destination, &found), hosts)?
        } else {
            found.iter().map(|o| (*o, None)).collect()
        };
        if entering.is_empty() && split_hand.is_empty() {
            return self.finish_instruction(
                frame,
                InstructionOutcome::SetMoved {
                    movements: vec![],
                    destroyed_creatures: 0,
                },
            );
        }
        let cause = self.instruction_cause(&frame, source_block)?;
        let hand_destination = |object| {
            if choices.iter().any(|(o, yes)| *o == object && *yes) {
                Zone::Command { owner: player }
            } else {
                Zone::Hand { owner: player }
            }
        };
        let mut occurrences: Vec<_> = entering
            .iter()
            .map(|(object, host)| {
                let command = choices.iter().any(|(o, yes)| o == object && *yes);
                match (to, host) {
                    _ if command => ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Command { owner: player },
                    },
                    (_, Some(target)) => ProposedEvent::EnterAttached {
                        object: *object,
                        target: *target,
                    },
                    (Some(destination), None) => ProposedEvent::Move {
                        object: *object,
                        destination,
                    },
                    (None, None) => ProposedEvent::EnterTapped { object: *object },
                }
            })
            .collect();
        occurrences.extend(split_hand.iter().map(|object| ProposedEvent::Move {
            object: *object,
            destination: hand_destination(*object),
        }));
        let entries = EventGroup::Simultaneous { occurrences };
        // CR 614.1c, 614.12: entrants with an entry life cost enter paid or
        // tapped as chosen; the payments precede the one simultaneous entry.
        let mut overrides = vec![];
        let mut payments = vec![];
        if battlefield {
            let mut shown = vec![];
            for (object, host) in &entering {
                let choice = self.entry_choice(*object)?;
                let accepted = match &choice {
                    Some(EntryChoice::Life(amount)) => {
                        let pays = paid.iter().any(|(o, yes)| o == object && *yes);
                        if pays {
                            payments.push(EventGroup::Occurrence {
                                proposed: ProposedEvent::LoseLife {
                                    player,
                                    amount: amount.clone(),
                                },
                            });
                        }
                        pays
                    }
                    Some(EntryChoice::Reveal(_)) => {
                        let card = revealed
                            .iter()
                            .find(|(o, _)| o == object)
                            .and_then(|(_, card)| *card);
                        shown.extend(card);
                        card.is_some()
                    }
                    None => false,
                };
                if tapped && (choice.is_some() || host.is_some()) {
                    overrides.push((*object, events::EntryOverride::ForceTapped));
                } else if choice.is_some() {
                    overrides.push((*object, events::EntryOverride::Choice { accepted }));
                }
            }
            // CR 701.20a: the revealed cards stay in the searcher's hand,
            // publicly known there by identity.
            if !shown.is_empty() {
                self.remember_revealed_hand_cards(player, &shown)?;
            }
        }
        let batch = if overrides.is_empty() {
            self.commit_batch(cause, entries)?
        } else {
            payments.push(entries);
            self.commit_entry_batch(
                cause,
                EventGroup::Ordered { children: payments },
                &overrides,
            )?
        };
        let movements = batch
            .occurrences
            .iter()
            .filter_map(|occurrence| match &occurrence.actual {
                EventOutcome::Moved { movement }
                | EventOutcome::EnteredAttached { movement, .. } => Some(movement.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if movements.len() != entering.len() + split_hand.len() {
            return Err("search movement invariant".into());
        }
        if reveal {
            let revealed: Vec<_> = movements
                .iter()
                .filter(|m| m.destination == (Zone::Hand { owner: player }))
                .map(|m| m.successor)
                .collect();
            if !revealed.is_empty() {
                self.remember_revealed_hand(player, &revealed)?;
            }
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::SetMoved {
                movements,
                destroyed_creatures: 0,
            },
        )
    }

    /// The suspended search continuation is this instruction's own: the
    /// found cards are a legal answer and the commanders are asked in order.
    pub(in crate::engine) fn valid_search_continuation(
        &self,
        object: ObjectId,
        frame: &Resolving,
        player: PlayerId,
        found: &[ObjectId],
        remaining: &[ObjectId],
        choices: &[(ObjectId, bool)],
        source_block: u32,
    ) -> Result<bool, String> {
        let (verb, block) = self.library_verb(frame)?;
        let Verb::Search {
            player: who,
            search,
        } = &verb
        else {
            return Ok(false);
        };
        if block != source_block
            || !matches!(self.kernel_players(frame, who)?, Players::One(p) if p == player)
            || !self.is_live(player)
        {
            return Ok(false);
        }
        let bounds = self.search_bounds(player, search)?;
        let commanders = self.found_commanders(frame, found)?;
        let asked: Vec<_> = choices.iter().map(|(o, _)| *o).collect();
        Ok(Self::subset(&bounds.candidates, found)
            && (search.to == Destination::OneTappedRestOwnerHand
                || found.windows(2).all(|w| w[0] < w[1]))
            && (bounds.minimum..=bounds.maximum).contains(&found.len())
            && remaining.first() == Some(&object)
            && asked.iter().chain(remaining).copied().collect::<Vec<_>>() == commanders)
    }

    /// The decision a library choice waits for, or `None` when its actor
    /// has left the game.
    pub(in crate::engine) fn library_request(
        &self,
        frame: &Resolving,
        choice: &ResolutionChoice,
    ) -> Result<Option<Decision<Rules>>, String> {
        let objects = |cards: &[ObjectId]| -> Vec<Candidate<Rules>> {
            cards
                .iter()
                .map(|object| Candidate::Object { object: *object })
                .collect()
        };
        Ok(match choice {
            ResolutionChoice::Search { player, .. } if self.is_live(*player) => {
                let (Verb::Search { search, .. }, _) = self.library_verb(frame)? else {
                    return Err("search choice lost its instruction".into());
                };
                // CR 701.23a: the searcher sees the whole library; only the
                // matching cards are offered, possibly none at all.
                let bounds = self.search_bounds(*player, &search)?;
                Some(self.pick(
                    *player,
                    if search.from == SelectionZone::Library {
                        Reason::SearchLibrary
                    } else {
                        Reason::ResolveCardChoice
                    },
                    objects(&bounds.candidates),
                    bounds.minimum as u32,
                    bounds.maximum as u32,
                ))
            }
            ResolutionChoice::SearchSplit { player, found, .. } if self.is_live(*player) => {
                Some(self.pick(*player, Reason::SearchSplitBattlefield, objects(found), 1, 1))
            }
            // CR 303.4f: the host a found Aura enters attached to; CR 119.4,
            // 614.1c: whether to pay the next entrant's entry life cost. The
            // entrant is the slot's subject, so the answer binds to it.
            ResolutionChoice::SearchEntry {
                player,
                found,
                hosts,
                paid,
                revealed,
                ..
            // No question: the last open one lost its subject (CR 303.4g)
            // and the step commits the entry without a decision.
            } if self.is_live(*player) => self
                .card_entry_question(frame, *player, found, hosts, paid, revealed)?
                .map(|question| match question {
                    EntryQuestion::Attach { aura, hosts } => {
                        let mut candidates = vec![Candidate::Object { object: aura }];
                        candidates.extend(objects(&hosts));
                        self.decision(
                            *player,
                            Reason::EntryAttachment,
                            candidates,
                            Shape::Pick {
                                slots: vec![Slot {
                                    role: SlotRole::Choice,
                                    subject: Some(0),
                                    allowed: (1..=hosts.len() as u32).collect(),
                                    min: 1,
                                    max: 1,
                                    repeat: false,
                                }],
                                rules: vec![],
                            },
                        )
                    }
                    EntryQuestion::Pay { entrant } => self.entry_payment_decision(*player, entrant),
                    EntryQuestion::Reveal { entrant, cards } => {
                        self.entry_reveal_decision(*player, entrant, &cards)
                    }
                }),
            ResolutionChoice::ReorderLibraryTop { player, looked, .. } if self.is_live(*player) => {
                Some(self.order(*player, Reason::OrderLibraryTop, objects(looked), looked.len() as u32))
            }
            ResolutionChoice::LookChooseHand { player, looked, .. } if self.is_live(*player) => {
                Some(self.pick(*player, Reason::LookChooseHand, objects(looked), 1, 1))
            }
            ResolutionChoice::LookOrderBottom { player, looked, hand, .. } if self.is_live(*player) => {
                let rest = Self::rest(looked, &[*hand]);
                Some(self.order(*player, Reason::OrderLibraryBottom, objects(&rest), rest.len() as u32))
            }
            ResolutionChoice::Scry {
                player,
                looked,
                bottom,
                top,
                ..
            } if self.is_live(*player) => Some(match (bottom, top) {
                (None, _) => self.pick(
                    *player,
                    Reason::ScryBottom,
                    objects(looked),
                    0,
                    looked.len() as u32,
                ),
                (Some(bottom), None) => {
                    let rest = Self::rest(looked, bottom);
                    self.order(
                        *player,
                        Reason::OrderLibraryTop,
                        objects(&rest),
                        rest.len() as u32,
                    )
                }
                (Some(bottom), Some(_)) => self.order(
                    *player,
                    Reason::OrderLibraryBottom,
                    objects(bottom),
                    bottom.len() as u32,
                ),
            }),
            ResolutionChoice::Surveil {
                player,
                looked,
                graveyard,
                ..
            } if self.is_live(*player) => Some(match graveyard {
                None => self.pick(
                    *player,
                    Reason::SurveilGraveyard,
                    objects(looked),
                    0,
                    looked.len() as u32,
                ),
                Some(graveyard) => {
                    let rest = Self::rest(looked, graveyard);
                    self.order(
                        *player,
                        Reason::OrderLibraryTop,
                        objects(&rest),
                        rest.len() as u32,
                    )
                }
            }),
            _ => None,
        })
    }

    /// The looked-at cards outside `chosen`, top first.
    fn rest(looked: &[ObjectId], chosen: &[ObjectId]) -> Vec<ObjectId> {
        looked
            .iter()
            .filter(|o| !chosen.contains(o))
            .copied()
            .collect()
    }

    /// Accept the answer to a library choice, then ask the next question or
    /// perform the action (CR 701.22a, 701.23a, 701.25a).
    pub(in crate::engine) fn library_choice_step(
        &mut self,
        frame: Resolving,
        choice: ResolutionChoice,
    ) -> Result<(), String> {
        let player = match &choice {
            ResolutionChoice::Search { player, .. }
            | ResolutionChoice::SearchSplit { player, .. }
            | ResolutionChoice::SearchEntry { player, .. }
            | ResolutionChoice::ReorderLibraryTop { player, .. }
            | ResolutionChoice::LookChooseHand { player, .. }
            | ResolutionChoice::LookOrderBottom { player, .. }
            | ResolutionChoice::Scry { player, .. }
            | ResolutionChoice::Surveil { player, .. } => *player,
            _ => return Err("not a library choice".into()),
        };
        if let ResolutionChoice::SearchEntry {
            found,
            command_choices,
            hosts,
            paid,
            revealed,
            source_block,
            ..
        } = &choice
            && self.is_live(player)
            && self
                .card_entry_question(&frame, player, found, hosts, paid, revealed)?
                .is_none()
        {
            // The open question lost its subject: enter what remains.
            self.rules_mut().pending = None;
            let (found, hosts, paid, revealed) =
                (found.clone(), hosts.clone(), paid.clone(), revealed.clone());
            return self.continue_card_entry(
                frame,
                player,
                found,
                command_choices.clone(),
                hosts,
                paid,
                revealed,
                *source_block,
            );
        }
        if self.library_request(&frame, &choice)?.is_none() {
            self.rules_mut().pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing library choice")?;
        if input.player != player {
            return Err("library choice actor invariant".into());
        }
        if let ResolutionChoice::SearchEntry {
            found,
            command_choices,
            mut hosts,
            mut paid,
            mut revealed,
            source_block,
            ..
        } = choice
        {
            match self
                .card_entry_question(&frame, player, &found, &hosts, &paid, &revealed)?
                .ok_or("search entry answer without a question")?
            {
                EntryQuestion::Attach { aura, hosts: legal } => {
                    let host = input.object()?;
                    if !legal.contains(&host) {
                        return Err("search entry host is not a legal host".into());
                    }
                    hosts.push((aura, host));
                }
                EntryQuestion::Pay { entrant } => paid.push((entrant, input.flag(Flag::Accept)?)),
                EntryQuestion::Reveal { entrant, cards } => {
                    revealed.push((entrant, Self::entry_reveal_answer(&input, &cards)?));
                }
            }
            return self.continue_card_entry(
                frame,
                player,
                found,
                command_choices,
                hosts,
                paid,
                revealed,
                source_block,
            );
        }
        let chosen = input.objects()?;
        match choice {
            ResolutionChoice::ReorderLibraryTop {
                looked,
                source_block,
                ..
            } => self.commit_look(
                frame,
                ProposedEvent::ReorderLibraryTop {
                    player,
                    looked,
                    top: chosen,
                },
                source_block,
            ),
            ResolutionChoice::Search { source_block, .. } => {
                self.complete_search(frame, player, chosen, source_block)
            }
            ResolutionChoice::SearchSplit {
                mut found,
                source_block,
                ..
            } => {
                match chosen.as_slice() {
                    [first] if *first == found[0] => {}
                    [first] if *first == found[1] => found.swap(0, 1),
                    _ => return Err("search split answer is outside the found cards".into()),
                }
                self.route_found_cards(frame, player, found.to_vec(), source_block)
            }
            ResolutionChoice::LookChooseHand {
                looked,
                source_block,
                ..
            } => {
                let [hand] = chosen.as_slice() else {
                    return Err("expected one looked-at card".into());
                };
                let rest = Self::rest(&looked, &[*hand]);
                if rest.len() > 1 {
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::LookOrderBottom {
                            player,
                            looked,
                            hand: *hand,
                            source_block,
                        },
                    };
                    Ok(())
                } else {
                    self.route_look_selection(frame, player, looked, *hand, rest, source_block)
                }
            }
            ResolutionChoice::LookOrderBottom {
                looked,
                hand,
                source_block,
                ..
            } => self.route_look_selection(frame, player, looked, hand, chosen, source_block),
            ResolutionChoice::Scry {
                looked,
                bottom: None,
                source_block,
                ..
            } => self.advance_scry(frame, player, looked, chosen, None, source_block),
            ResolutionChoice::Scry {
                looked,
                bottom: Some(bottom),
                top: None,
                source_block,
                ..
            } => self.advance_scry(frame, player, looked, bottom, Some(chosen), source_block),
            ResolutionChoice::Scry {
                looked,
                top: Some(top),
                source_block,
                ..
            } => self.commit_look(
                frame,
                ProposedEvent::Scry {
                    player,
                    looked,
                    top,
                    bottom: chosen,
                },
                source_block,
            ),
            ResolutionChoice::Surveil {
                looked,
                graveyard: None,
                source_block,
                ..
            } => self.advance_surveil(frame, player, looked, chosen, source_block),
            ResolutionChoice::Surveil {
                looked,
                graveyard: Some(graveyard),
                source_block,
                ..
            } => self.commit_look(
                frame,
                ProposedEvent::Surveil {
                    player,
                    looked,
                    top: chosen,
                    graveyard,
                },
                source_block,
            ),
            _ => Err("not a library choice".into()),
        }
    }

    fn route_look_selection(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        hand: ObjectId,
        bottom: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        if !self.valid_look_selection(&frame, player, &looked, hand, &bottom, source_block)? {
            return Err("invalid library prefix selection".into());
        }
        let destination = Zone::Hand { owner: player };
        if self.state.rules.commanders.history(hand.card).is_some() {
            self.rules_mut().frame = Frame::CommanderReturn {
                object: hand,
                destination,
                continuation: CommanderContinuation::LookHand {
                    frame,
                    player,
                    looked,
                    bottom,
                    source_block,
                },
            };
            Ok(())
        } else {
            self.commit_look_selection(
                frame,
                player,
                looked,
                hand,
                bottom,
                destination,
                source_block,
            )
        }
    }

    pub(in crate::engine) fn valid_look_selection(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        hand: ObjectId,
        bottom: &[ObjectId],
        source_block: u32,
    ) -> Result<bool, String> {
        let (
            Verb::LookChooseHand {
                player: chooser,
                amount,
            },
            block,
        ) = self.library_verb(frame)?
        else {
            return Ok(false);
        };
        Ok(block == source_block
            && self.library_actor(frame, &chooser)? == Some(player)
            && looked.contains(&hand)
            && Self::same_cards(&Self::rest(looked, &[hand]), bottom)
            && self.library_prefix(player, &self.kernel_amount(frame, &amount)?)? == looked)
    }

    pub(in crate::engine) fn commit_look_selection(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        hand: ObjectId,
        bottom: Vec<ObjectId>,
        destination: Zone,
        source_block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let cause = self.instruction_cause(&frame, source_block)?;
        let move_card = ProposedEvent::Move {
            object: hand,
            destination,
        };
        let actual = if bottom.is_empty() {
            self.commit(move_card, cause)?
        } else {
            // No priority or intervening trigger processing. The hand
            // movement clears the chosen card from the inspected prefix.
            let batch = self.commit_batch(
                cause,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence {
                            proposed: move_card,
                        },
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::PutLibraryPrefixOnBottom {
                                player,
                                looked: Self::rest(&looked, &[hand]),
                                bottom,
                            },
                        },
                    ],
                },
            )?;
            batch
                .result(&[1])
                .cloned()
                .ok_or("missing bottom placement")?
        };
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }

    /// CR 701.22a: an order of one card or none needs no decision.
    fn advance_scry(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        bottom: Vec<ObjectId>,
        top: Option<Vec<ObjectId>>,
        source_block: u32,
    ) -> Result<(), String> {
        let top = match top {
            Some(top) => top,
            None => {
                let rest = Self::rest(&looked, &bottom);
                if rest.len() > 1 {
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Scry {
                            player,
                            looked,
                            bottom: Some(bottom),
                            top: None,
                            source_block,
                        },
                    };
                    return Ok(());
                }
                rest
            }
        };
        if bottom.len() > 1 {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Scry {
                    player,
                    looked,
                    bottom: Some(bottom),
                    top: Some(top),
                    source_block,
                },
            };
            return Ok(());
        }
        self.commit_look(
            frame,
            ProposedEvent::Scry {
                player,
                looked,
                top,
                bottom,
            },
            source_block,
        )
    }

    /// CR 701.25a: an order of one card or none needs no decision.
    fn advance_surveil(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        graveyard: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let top = Self::rest(&looked, &graveyard);
        if top.len() > 1 {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Surveil {
                    player,
                    looked,
                    graveyard: Some(graveyard),
                    source_block,
                },
            };
            return Ok(());
        }
        self.commit_look(
            frame,
            ProposedEvent::Surveil {
                player,
                looked,
                top,
                graveyard,
            },
            source_block,
        )
    }

    fn commit_look(
        &mut self,
        frame: Resolving,
        event: ProposedEvent,
        source_block: u32,
    ) -> Result<(), String> {
        let actual = self.commit(event, self.instruction_cause(&frame, source_block)?)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }

    /// CR 701.20a: the split choice follows a public reveal of the found
    /// cards. Only their Oracle identities leave the private choice.
    pub(in crate::engine) fn revealed_split_search_cards(
        &self,
    ) -> Result<Vec<(PlayerId, String)>, String> {
        let (frame, player, found): (_, _, &[_]) = match &self.state.rules.frame {
            Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::SearchSplit { player, found, .. },
            } => (frame, player, found),
            Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::SearchEntry { player, found, .. },
            }
            | Frame::CommanderReturn {
                continuation:
                    CommanderContinuation::Search {
                        frame,
                        player,
                        found,
                        ..
                    },
                ..
            } => (frame, player, found),
            _ => return Ok(vec![]),
        };
        if !self.is_live(*player)
            || !matches!(self.library_verb(frame)?.0, Verb::Search { search, .. }
                if search.to == Destination::OneTappedRestOwnerHand)
        {
            return Ok(vec![]);
        }
        let mut revealed = found
            .iter()
            .map(|object| Ok((*player, self.definition(object.card)?.oracle_id.clone())))
            .collect::<Result<Vec<_>, String>>()?;
        revealed.sort();
        Ok(revealed)
    }

    /// A library card the viewer is choosing among right now.
    pub(in crate::engine) fn library_choice_visible(
        &self,
        viewer: PlayerId,
        object: ObjectId,
    ) -> bool {
        let choice = match &self.state.rules.frame {
            Frame::ResolutionChoice { choice, .. } => choice,
            Frame::CommanderReturn {
                continuation: CommanderContinuation::LookHand { player, looked, .. },
                ..
            } => return *player == viewer && looked.contains(&object),
            _ => return false,
        };
        match choice {
            ResolutionChoice::ReorderLibraryTop { player, looked, .. }
            | ResolutionChoice::LookChooseHand { player, looked, .. }
            | ResolutionChoice::LookOrderBottom { player, looked, .. }
            | ResolutionChoice::Scry { player, looked, .. }
            | ResolutionChoice::Surveil { player, looked, .. } => {
                *player == viewer && looked.contains(&object)
            }
            // The searcher still sees the found cards whose entry it decides.
            ResolutionChoice::SearchSplit { player, found, .. } => {
                *player == viewer && found.contains(&object)
            }
            ResolutionChoice::SearchEntry { player, found, .. } => {
                *player == viewer && found.contains(&object)
            }
            // CR 701.23a: every card of the searched library, matching or
            // not; no position is published.
            ResolutionChoice::Search { player, .. } if *player == viewer => {
                self.current(object).is_ok_and(|current| {
                    current.id == object && current.zone == (Zone::Library { owner: viewer })
                })
            }
            _ => false,
        }
    }

    /// The suspended library choice is the admitted instruction's own: its
    /// actor, block and partial answers agree with the node and the library.
    pub(in crate::engine) fn valid_library_choice(
        &self,
        frame: &Resolving,
        choice: &ResolutionChoice,
    ) -> Result<bool, String> {
        let (verb, block) = self.library_verb(frame)?;
        let (player, source_block) = match choice {
            ResolutionChoice::Search {
                player,
                source_block,
            }
            | ResolutionChoice::SearchSplit {
                player,
                source_block,
                ..
            }
            | ResolutionChoice::ReorderLibraryTop {
                player,
                source_block,
                ..
            }
            | ResolutionChoice::LookChooseHand {
                player,
                source_block,
                ..
            }
            | ResolutionChoice::LookOrderBottom {
                player,
                source_block,
                ..
            }
            | ResolutionChoice::Scry {
                player,
                source_block,
                ..
            }
            | ResolutionChoice::Surveil {
                player,
                source_block,
                ..
            }
            | ResolutionChoice::SearchEntry {
                player,
                source_block,
                ..
            } => (*player, *source_block),
            _ => return Ok(false),
        };
        // CR 601.2c, 608.2b, 303.4f: an entry choice from a graveyard
        // move belongs solely to the legal, announced incarnation at this
        // cursor. The same question/answer and atomic entry code handles
        // both a found card and a targeted one.
        if let (
            ResolutionChoice::SearchEntry {
                found,
                command_choices,
                hosts,
                paid,
                revealed,
                ..
            },
            Verb::Move {
                what:
                    kir::SetExpr::One {
                        object: kir::Ref::Target { slot },
                    },
                to: Destination::Battlefield { .. },
            },
        ) = (choice, &verb)
        {
            let entry = self.entry(frame.stack_id)?;
            let slot = *slot as usize;
            let [target] = found.as_slice() else {
                return Ok(false);
            };
            return Ok(source_block == block
                && entry.controller == player
                && command_choices.is_empty()
                && frame.legal_targets.get(slot) == Some(&true)
                && self
                    .program_for(entry)?
                    .targets
                    .get(slot)
                    .is_some_and(|spec| spec.domain == TargetDomain::OwnGraveyardCard)
                && self.bound_object_target(entry, slot) == Some(target)
                && self.valid_entry_answers(player, 1, found, hosts, paid, revealed)?
                && (!self.is_live(player)
                    || (self
                        .current(*target)
                        .is_ok_and(|o| self.player_graveyard_card(o, player))
                        && self
                            .next_entry_question(player, found, hosts, paid, revealed)?
                            .is_some())));
        }
        let expected = match &verb {
            Verb::Search { player, .. }
            | Verb::ReorderLibraryTop { player, .. }
            | Verb::LookChooseHand { player, .. }
            | Verb::Scry { player, .. }
            | Verb::Surveil { player, .. } => match self.kernel_players(frame, player)? {
                Players::One(player) => player,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if expected != player || source_block != block {
            return Ok(false);
        }
        // The choice variant must be this verb's own, and its partial
        // answers well formed, before the departure fast path accepts it.
        let staged = match (choice, &verb) {
            // CR 608.2d: a random selection never waits on its chooser.
            (ResolutionChoice::Search { .. }, Verb::Search { search, .. }) => !search.random,
            (ResolutionChoice::SearchSplit { found, .. }, Verb::Search { search, .. }) => {
                search.to == Destination::OneTappedRestOwnerHand
                    && found[0].card < found[1].card
                    && self.valid_looked(frame, player, found, &search.maximum)?
            }
            (
                ResolutionChoice::ReorderLibraryTop { looked, .. },
                Verb::ReorderLibraryTop { amount, .. },
            ) => self.valid_looked(frame, player, looked, amount)?,
            (
                ResolutionChoice::LookChooseHand { looked, .. },
                Verb::LookChooseHand { amount, .. },
            ) => self.valid_looked(frame, player, looked, amount)?,
            (
                ResolutionChoice::LookOrderBottom { looked, hand, .. },
                Verb::LookChooseHand { amount, .. },
            ) => {
                looked.len() > 2
                    && looked.contains(hand)
                    && self.valid_looked(frame, player, looked, amount)?
            }
            // A battlefield search's saved answers, checked before the
            // departure fast path accepts them.
            (
                ResolutionChoice::SearchEntry {
                    found,
                    command_choices,
                    hosts,
                    paid,
                    revealed,
                    ..
                },
                Verb::Search { search, .. },
            ) => {
                let maximum = search
                    .maximum
                    .as_fixed()
                    .and_then(Quantity::to_u32)
                    .ok_or("search maximum has no audited evaluation")?
                    as usize;
                self.search_onto_battlefield(frame)?
                    && self.valid_looked(frame, player, found, &search.maximum)?
                    && command_choices
                        .iter()
                        .map(|(o, _)| *o)
                        .eq(self.found_commanders(frame, found)?)
                    && self.valid_entry_answers(
                        player,
                        maximum,
                        Self::entry_cards(&search.to, found),
                        hosts,
                        paid,
                        revealed,
                    )?
            }
            (
                ResolutionChoice::Scry {
                    looked,
                    bottom,
                    top,
                    ..
                },
                Verb::Scry { amount, .. },
            ) => {
                self.valid_looked(frame, player, looked, amount)?
                    && match (bottom, top) {
                        (None, None) => true,
                        (Some(bottom), None) => {
                            Self::subset(looked, bottom) && Self::rest(looked, bottom).len() > 1
                        }
                        (Some(bottom), Some(top)) => {
                            bottom.len() > 1
                                && Self::subset(looked, bottom)
                                && Self::same_cards(&Self::rest(looked, bottom), top)
                        }
                        (None, Some(_)) => false,
                    }
            }
            (
                ResolutionChoice::Surveil {
                    looked, graveyard, ..
                },
                Verb::Surveil { amount, .. },
            ) => {
                self.valid_looked(frame, player, looked, amount)?
                    && graveyard.as_ref().is_none_or(|graveyard| {
                        Self::subset(looked, graveyard) && Self::rest(looked, graveyard).len() > 1
                    })
            }
            _ => false,
        };
        if !staged {
            return Ok(false);
        }
        // A departed actor's library left the game; the step skips it.
        if !self.is_live(player) {
            return Ok(true);
        }
        Ok(match (choice, &verb) {
            (ResolutionChoice::Search { .. }, Verb::Search { search, .. }) => {
                self.search_bounds(player, search)?.needs_choice()
            }
            (ResolutionChoice::SearchSplit { found, .. }, Verb::Search { search, .. }) => {
                Self::subset(&self.search_bounds(player, search)?.candidates, found)
            }
            (
                ResolutionChoice::SearchEntry {
                    found,
                    hosts,
                    paid,
                    revealed,
                    ..
                },
                Verb::Search { search, .. },
            ) => {
                let bounds = self.search_bounds(player, search)?;
                Self::subset(&bounds.candidates, found)
                    && (bounds.minimum..=bounds.maximum).contains(&found.len())
                    && self
                        .card_entry_question(frame, player, found, hosts, paid, revealed)
                        .is_ok_and(|next| next.is_some())
            }
            (
                ResolutionChoice::ReorderLibraryTop { looked, .. },
                Verb::ReorderLibraryTop { amount, .. },
            )
            | (
                ResolutionChoice::LookChooseHand { looked, .. },
                Verb::LookChooseHand { amount, .. },
            )
            | (
                ResolutionChoice::LookOrderBottom { looked, .. },
                Verb::LookChooseHand { amount, .. },
            )
            | (ResolutionChoice::Scry { looked, .. }, Verb::Scry { amount, .. })
            | (ResolutionChoice::Surveil { looked, .. }, Verb::Surveil { amount, .. }) => {
                let amount = self.kernel_amount(frame, amount)?;
                self.library_prefix(player, &amount)? == *looked
            }
            _ => false,
        })
    }

    /// CR 401.2, 701.20e: the completed reorder retains the exact private
    /// prefix so that state validation can check the permutation after
    /// later instructions change that library.
    pub(in crate::engine) fn validate_library_reorder_outcome(
        &self,
        frame: &Resolving,
        instruction: Option<&Instruction>,
        outcome: &InstructionOutcome,
    ) -> Result<(), String> {
        let invalid = || "snapshot completed library reorder invariant".to_owned();
        let result = match outcome {
            InstructionOutcome::Effect {
                actual:
                    Some(EventOutcome::LibraryTopReordered {
                        player,
                        looked,
                        top,
                    }),
            } => Some((player, looked, top)),
            InstructionOutcome::Damage {
                actual: Some(EventOutcome::LibraryTopReordered { .. }),
            }
            | InstructionOutcome::Life {
                actual: Some(EventOutcome::LibraryTopReordered { .. }),
            } => return Err(invalid()),
            InstructionOutcome::Kernel { outcomes } => {
                for child in outcomes {
                    self.validate_library_reorder_outcome(frame, None, child)?;
                }
                None
            }
            _ => None,
        };
        if let Some(Verb::ReorderLibraryTop { amount, .. }) =
            instruction.and_then(Instruction::verb)
        {
            let controller = self.entry(frame.stack_id)?.controller;
            if let Some((player, looked, top)) = result {
                if *player == controller
                    && self.valid_looked(frame, *player, looked, amount)?
                    && Self::same_cards(looked, top)
                    && looked.iter().all(|id| {
                        self.state.rules.objects.get(&id.card).map_or_else(
                            || !self.is_live(*player),
                            |current| {
                                // A later move can create a successor. If the
                                // same incarnation lives, it still belongs to
                                // this library (CR 400.7).
                                current.id.incarnation > id.incarnation
                                    || (current.id == *id
                                        && current.zone == (Zone::Library { owner: *player }))
                            },
                        )
                    })
                {
                    return Ok(());
                }
            } else if matches!(outcome, InstructionOutcome::Effect { actual: None })
                || matches!(outcome, InstructionOutcome::PlayerActionSkipped { player }
                    if *player == controller && !self.is_live(*player))
            {
                return Ok(());
            }
            return Err(invalid());
        }
        if result.is_some() {
            return Err(invalid());
        }
        Ok(())
    }

    /// CR 401.4, 701.20e: a completed private selection can retain an
    /// ordered remainder or the one movement when the prefix was a singleton.
    pub(in crate::engine) fn validate_look_selection_outcome(
        &self,
        frame: &Resolving,
        instruction: &Instruction,
        outcome: &InstructionOutcome,
    ) -> Result<(), String> {
        let result = match outcome {
            InstructionOutcome::Effect {
                actual: Some(actual),
            } => Some(actual),
            _ => None,
        };
        let Some(Verb::LookChooseHand { amount, .. }) = instruction.verb() else {
            return if matches!(result, Some(EventOutcome::LibraryPrefixBottomed { .. })) {
                Err("snapshot bottom placement on another instruction".into())
            } else {
                Ok(())
            };
        };
        let player = self.entry(frame.stack_id)?.controller;
        let valid = match result {
            Some(EventOutcome::LibraryPrefixBottomed {
                player: actor,
                looked,
                bottom,
            }) => {
                *actor == player
                    && self.valid_looked(frame, player, looked, amount)?
                    && Quantity::from(looked.len() as i64) < self.kernel_amount(frame, amount)?
                    && Self::same_cards(looked, bottom)
            }
            Some(EventOutcome::Moved { movement }) => {
                movement.from == (Zone::Library { owner: player })
                    && (movement.destination == (Zone::Hand { owner: player })
                        || (movement.destination == (Zone::Command { owner: player })
                            && self
                                .state
                                .rules
                                .commanders
                                .history(movement.previous.card)
                                .is_some()))
            }
            None => {
                matches!(outcome, InstructionOutcome::Effect { actual: None })
                    || matches!(outcome, InstructionOutcome::PlayerActionSkipped { player: actor }
                    if *actor == player && !self.is_live(player))
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err("snapshot completed library selection invariant".into())
        }
    }

    /// CR 701.22a, 701.25a: the looked-at cards are distinct cards of the
    /// chooser's own library, at least one and at most the printed amount;
    /// this holds even after the chooser left with that library.
    fn valid_looked(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        amount: &mtg_contracts::kir::Amount,
    ) -> Result<bool, String> {
        let amount = self.kernel_amount(frame, amount)?;
        let count = Quantity::from(i64::try_from(looked.len()).map_err(|_| "look limit")?);
        Ok(!looked.is_empty()
            && count <= amount
            // Distinct physical cards: two incarnations are one card.
            && looked.iter().map(|o| o.card).collect::<BTreeSet<_>>().len() == looked.len()
            && looked.iter().all(|o| {
                self.state
                    .rules
                    .cards
                    .get(&o.card)
                    .is_some_and(|card| card.owner == player)
            }))
    }

    /// Distinct members of `looked`.
    fn subset(looked: &[ObjectId], chosen: &[ObjectId]) -> bool {
        chosen.iter().collect::<BTreeSet<_>>().len() == chosen.len()
            && chosen.iter().all(|o| looked.contains(o))
    }

    fn same_cards(left: &[ObjectId], right: &[ObjectId]) -> bool {
        left.len() == right.len()
            && left.iter().collect::<BTreeSet<_>>() == right.iter().collect::<BTreeSet<_>>()
    }
}

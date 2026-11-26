//! CR 119.4, 614.1c and 614.12.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use events::EntryOverride;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingEntry {
    object: ObjectId,
    controller: PlayerId,
    cause: BatchCause,
    pub(super) resume: FrameLink,
    /// The move already enters tapped; paying the life is still allowed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    force_tapped: bool,
}

fn entry_face(object: ObjectId, cause: &BatchCause) -> ObjectId {
    match cause {
        BatchCause::LandPlay { face, .. } => ObjectId {
            face: *face,
            ..object
        },
        _ => object,
    }
}

enum EntryPayment {
    NoneRequired,
    ForceTapped,
    Declined,
    Paid { player: PlayerId, amount: Quantity },
    Revealed { player: PlayerId, card: ObjectId },
}

/// CR 614.12: the choice an entering permanent's own replacement offers
/// its incoming controller; declining it, or being unable to make it,
/// makes the permanent enter tapped.
#[derive(Clone, Debug)]
pub(in crate::engine) enum EntryChoice {
    /// CR 119.4: pay this much life.
    Life(Quantity),
    /// CR 701.20a: reveal one card matching this filter from your hand.
    Reveal(mtg_contracts::kir::Pred),
}

impl RulesTxn {
    pub(in crate::engine) fn entry_choice(
        &self,
        object: ObjectId,
    ) -> Result<Option<EntryChoice>, String> {
        if self.entry_abilities_lost(object)? {
            return Ok(None);
        }
        Ok(self
            .copiable_definition(object)?
            .static_abilities
            .iter()
            .find_map(|ability| match &ability.effect {
                StaticEffect::PayLifeOrEnterTapped { amount } => {
                    Some(EntryChoice::Life(amount.clone()))
                }
                StaticEffect::RevealOrEnterTapped { filter } => {
                    Some(EntryChoice::Reveal(filter.clone()))
                }
                _ => None,
            }))
    }

    pub(super) fn entry_life_cost(&self, object: ObjectId) -> Result<Option<Quantity>, String> {
        Ok(match self.entry_choice(object)? {
            Some(EntryChoice::Life(amount)) => Some(amount),
            _ => None,
        })
    }

    pub(super) fn can_pay_entry_life(&self, controller: PlayerId, amount: &Quantity) -> bool {
        self.is_live(controller)
            && self.state.rules.players[usize::from(controller.0)].can_pay_life(amount)
    }

    /// CR 701.20a, 614.12: the cards of the incoming controller's hand that
    /// `entrant`'s reveal could show, in object order. The entrant itself
    /// is entering, so it is never one of them.
    pub(in crate::engine) fn entry_reveal_candidates(
        &self,
        controller: PlayerId,
        entrant: ObjectId,
        filter: &mtg_contracts::kir::Pred,
    ) -> Vec<ObjectId> {
        if !self.is_live(controller) {
            return vec![];
        }
        let mut cards: Vec<_> = self
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == (Zone::Hand { owner: controller })
                    && o.id.card != entrant.card
                    && crate::query::matches_pred(&self.object_facts(o), controller, filter)
                        .is_yes()
            })
            .map(|o| o.id)
            .collect();
        cards.sort();
        cards
    }

    /// Whether `entrant`'s entry choice is offered now. A live controller is
    /// always asked about a reveal, even with no matching card (Decline is
    /// then the only option), so whether the entry waits never discloses
    /// the hidden hand (CR 400.2, 701.20a).
    pub(in crate::engine) fn entry_choice_open(
        &self,
        controller: PlayerId,
        _entrant: ObjectId,
        choice: &EntryChoice,
    ) -> bool {
        match choice {
            EntryChoice::Life(amount) => self.can_pay_entry_life(controller, amount),
            EntryChoice::Reveal(_) => self.is_live(controller),
        }
    }

    /// CR 119.4 and 614.1c: the decision paying `entrant`'s entry life
    /// cost, with the entrant as the slot's subject so the answer binds to
    /// it. Paying is allowed even when the entrant enters tapped anyway:
    /// "If an effect puts this land onto the battlefield tapped, you may pay
    /// 2 life, but it still enters tapped." (Lorwyn Eclipsed release notes,
    /// https://magic.wizards.com/en/news/feature/lorwyn-eclipsed-release-notes)
    pub(in crate::engine) fn entry_payment_decision(
        &self,
        controller: PlayerId,
        entrant: ObjectId,
    ) -> Decision<Rules> {
        self.decision(
            controller,
            Reason::OptionalEffect,
            vec![
                Candidate::Flag { flag: Flag::Accept },
                Candidate::Flag {
                    flag: Flag::Decline,
                },
                Candidate::Object { object: entrant },
            ],
            Shape::Pick {
                slots: vec![Slot {
                    role: SlotRole::Choice,
                    subject: Some(2),
                    allowed: vec![0, 1],
                    min: 1,
                    max: 1,
                    repeat: false,
                }],
                rules: vec![],
            },
        )
    }

    /// CR 701.20a, 614.12: decline, or pick the one hand card to reveal as
    /// `entrant` enters. The entrant is the slot's subject; only the
    /// controller sees the offered hand cards, possibly none.
    pub(in crate::engine) fn entry_reveal_decision(
        &self,
        controller: PlayerId,
        entrant: ObjectId,
        cards: &[ObjectId],
    ) -> Decision<Rules> {
        let mut candidates = vec![
            Candidate::Flag {
                flag: Flag::Decline,
            },
            Candidate::Object { object: entrant },
        ];
        candidates.extend(
            cards
                .iter()
                .map(|object| Candidate::Object { object: *object }),
        );
        let mut allowed = vec![0];
        allowed.extend(2..candidates.len() as u32);
        self.decision(
            controller,
            Reason::OptionalEffect,
            candidates,
            Shape::Pick {
                slots: vec![Slot {
                    role: SlotRole::Choice,
                    subject: Some(1),
                    allowed,
                    min: 1,
                    max: 1,
                    repeat: false,
                }],
                rules: vec![],
            },
        )
    }

    /// The open entry decision for `entrant`, when its controller can make it.
    pub(in crate::engine) fn entry_choice_decision(
        &self,
        controller: PlayerId,
        entrant: ObjectId,
        choice: &EntryChoice,
    ) -> Option<Decision<Rules>> {
        match choice {
            EntryChoice::Life(amount) => self
                .can_pay_entry_life(controller, amount)
                .then(|| self.entry_payment_decision(controller, entrant)),
            EntryChoice::Reveal(filter) => {
                let cards = self.entry_reveal_candidates(controller, entrant, filter);
                self.is_live(controller)
                    .then(|| self.entry_reveal_decision(controller, entrant, &cards))
            }
        }
    }

    /// The reveal an answered entry reveal decision chose: `None` when
    /// declined, otherwise one of the offered hand cards.
    pub(in crate::engine) fn entry_reveal_answer(
        input: &PendingInput,
        cards: &[ObjectId],
    ) -> Result<Option<ObjectId>, String> {
        match input.one()? {
            Candidate::Flag {
                flag: Flag::Decline,
            } => Ok(None),
            Candidate::Object { object } if cards.contains(object) => Ok(Some(*object)),
            _ => Err("invalid entry reveal answer".into()),
        }
    }

    /// Own a battlefield move until the entry decision and the move are complete.
    /// The calling rule resumes only after the entire entry batch commits.
    /// A forced tapped entry still offers an open entry choice and enters
    /// tapped either way (CR 614.1c, 614.12).
    pub(in crate::engine) fn begin_battlefield_entry(
        &mut self,
        object: ObjectId,
        cause: BatchCause,
        resume: Frame,
        force_tapped: bool,
    ) -> Result<(), String> {
        let controller = self.current(object)?.controller;
        if self
            .entry_choice(entry_face(object, &cause))?
            .is_some_and(|choice| self.entry_choice_open(controller, object, &choice))
        {
            let resume = self.save_frame(resume);
            self.rules_mut().frame = Frame::BattlefieldEntry {
                entry: Box::new(PendingEntry {
                    object,
                    controller,
                    cause,
                    resume,
                    force_tapped,
                }),
            };
            return Ok(());
        }
        let payment = if force_tapped {
            EntryPayment::ForceTapped
        } else {
            EntryPayment::NoneRequired
        };
        self.finish_battlefield_entry(object, cause, resume, payment, force_tapped)
    }

    /// The entering card its controller decides about while the entry
    /// waits, even from a hidden zone; nobody else sees it.
    pub(in crate::engine) fn battlefield_entry_visible(
        &self,
        viewer: PlayerId,
        object: ObjectId,
    ) -> bool {
        matches!(&self.state.rules.frame, Frame::BattlefieldEntry { entry }
            if entry.controller == viewer && entry.object == object)
    }

    pub(super) fn battlefield_entry_payment_source(
        &self,
        viewer: PlayerId,
        entry: &PendingEntry,
    ) -> Option<DecisionSource> {
        if viewer != entry.controller {
            return None;
        }
        // Bind the explanation to the card's current handle and use the
        // prospective face only for its printed name and entry cost.
        let face = entry_face(entry.object, &entry.cause);
        let definition = self.copiable_definition(face).ok()?;
        let consequence = if entry.force_tapped {
            "It enters tapped either way."
        } else {
            "It enters tapped if you decline."
        };
        let question = match self.entry_choice(face).ok()?? {
            EntryChoice::Life(amount) => {
                format!("Pay {amount} life as {} enters?", definition.name)
            }
            EntryChoice::Reveal(_) => format!(
                "Reveal a qualifying card from your hand as {} enters?",
                definition.name
            ),
        };
        Some(DecisionSource {
            kind: DecisionSourceKind::Effect,
            oracle_id: definition.oracle_id.clone(),
            object: Some(self.handle_for(viewer, entry.object).ok()?),
            ability: None,
            text: format!("{question} {consequence}"),
        })
    }

    pub(super) fn battlefield_entry_request(
        &self,
        entry: &PendingEntry,
    ) -> Result<Option<Decision<Rules>>, String> {
        if self.current(entry.object).is_err() || !self.is_live(entry.controller) {
            return Ok(None);
        }
        let choice = self
            .entry_choice(entry_face(entry.object, &entry.cause))?
            .ok_or("missing entry choice")?;
        Ok(self.entry_choice_decision(entry.controller, entry.object, &choice))
    }

    pub(super) fn battlefield_entry_step(&mut self, entry: PendingEntry) -> Result<(), String> {
        let resume = self.saved_frame(entry.resume)?.clone();
        let input = self.rules_mut().pending.take();
        if self.current(entry.object).is_err() || !self.is_live(entry.controller) {
            self.rules_mut().frame = resume;
            return Ok(());
        }
        let choice = self
            .entry_choice(entry_face(entry.object, &entry.cause))?
            .ok_or("missing entry choice")?;
        let payment = if self.entry_choice_open(entry.controller, entry.object, &choice) {
            let input = input.ok_or("missing entry decision")?;
            if input.player != entry.controller {
                return Err("wrong entering controller".into());
            }
            match choice {
                EntryChoice::Life(amount) => {
                    if input.flag(Flag::Accept)? {
                        EntryPayment::Paid {
                            player: entry.controller,
                            amount,
                        }
                    } else {
                        EntryPayment::Declined
                    }
                }
                EntryChoice::Reveal(filter) => {
                    let cards =
                        self.entry_reveal_candidates(entry.controller, entry.object, &filter);
                    match Self::entry_reveal_answer(&input, &cards)? {
                        Some(card) => EntryPayment::Revealed {
                            player: entry.controller,
                            card,
                        },
                        None => EntryPayment::Declined,
                    }
                }
            }
        } else {
            EntryPayment::Declined
        };
        self.finish_battlefield_entry(
            entry.object,
            entry.cause,
            resume,
            payment,
            entry.force_tapped,
        )
    }

    fn finish_battlefield_entry(
        &mut self,
        object: ObjectId,
        cause: BatchCause,
        resume: Frame,
        payment: EntryPayment,
        force_tapped: bool,
    ) -> Result<(), String> {
        let mut children = vec![];
        if let EntryPayment::Paid { player, amount } = &payment {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::LoseLife {
                    player: *player,
                    amount: amount.clone(),
                },
            });
        }
        // CR 701.20a: the revealed card stays in its owner's hand, publicly
        // known there by identity.
        if let EntryPayment::Revealed { player, card } = &payment {
            self.remember_revealed_hand_cards(*player, &[*card])?;
        }
        children.push(EventGroup::Occurrence {
            proposed: ProposedEvent::Move {
                object,
                destination: Zone::Battlefield,
            },
        });
        let land_play = matches!(cause, BatchCause::LandPlay { .. });
        if let BatchCause::LandPlay { player, .. } = &cause {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::LandPlayed { player: *player },
            });
        }
        self.rules_mut().frame = resume;
        match payment {
            EntryPayment::NoneRequired if !land_play => {
                self.commit_batch(cause, children.pop().ok_or("missing entry move")?)?;
            }
            EntryPayment::NoneRequired => {
                self.commit_batch(cause, EventGroup::Ordered { children })?;
            }
            EntryPayment::ForceTapped
            | EntryPayment::Declined
            | EntryPayment::Paid { .. }
            | EntryPayment::Revealed { .. } => {
                let entry = if force_tapped {
                    EntryOverride::ForceTapped
                } else {
                    EntryOverride::Choice {
                        accepted: matches!(
                            payment,
                            EntryPayment::Paid { .. } | EntryPayment::Revealed { .. }
                        ),
                    }
                };
                self.commit_entry_batch(
                    cause,
                    EventGroup::Ordered { children },
                    &[(object, entry)],
                )?;
            }
        }
        if land_play {
            self.rules_mut().passes.clear();
            self.stabilize()?;
        }
        Ok(())
    }

    pub(super) fn validate_battlefield_entry(&self) -> Result<(), String> {
        let Frame::BattlefieldEntry { entry } = &self.state.rules.frame else {
            return Ok(());
        };
        let resume = self.saved_frame(entry.resume)?;
        match entry.cause {
            BatchCause::LandPlay {
                player,
                source,
                decision_id,
                face,
            } => {
                let live = self.is_live(player);
                // The only intervening decision can be the optional entry choice.
                // Departure clears that pending input but retains the journal.
                let accepted = self
                    .state
                    .audit
                    .journal
                    .iter()
                    .rev()
                    .skip(usize::from(self.state.rules.pending.is_some()))
                    .take(if live { 1 } else { 2 })
                    .any(|input| input.player == player && input.decision_id == decision_id);
                if player != entry.controller
                    || entry.force_tapped
                    || source != entry.object
                    || !matches!(resume, Frame::Priority)
                    || !accepted
                {
                    return Err("invalid suspended land play origin".into());
                }
                if live {
                    self.validate_land_play_in_frame(player, source.card, face, resume)
                        .map_err(|error| format!("invalid suspended land play: {error}"))?;
                }
            }
            // A future resolving caller must validate its own saved continuation.
            BatchCause::Fixture if cfg!(test) => {}
            _ => return Err("invalid suspended battlefield entry cause".into()),
        }
        if let Ok(object) = self.current(entry.object) {
            if object.zone == Zone::Battlefield
                || object.controller != entry.controller
                || self
                    .entry_choice(entry_face(entry.object, &entry.cause))?
                    .is_none()
            {
                return Err("invalid suspended battlefield entry".into());
            }
        } else if self.is_live(entry.controller) {
            return Err("missing suspended battlefield entrant".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{Board, Driver, FixtureCatalog};
    use mtg_contracts::kir::{Atom, Cmp, EntryCondition, Pred};

    fn entry_land_board(effect: StaticEffect) -> Board {
        let mut catalog = FixtureCatalog::Witness.get().clone();
        let land = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Forest")
            .unwrap();
        land.source_labels = None;
        land.static_abilities.push(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        });
        land.capabilities = land.requirements();
        Board::new(catalog, "entry-replacement-move").step(TurnStep::PrecombatMain)
    }

    #[test]
    fn rules_move_offers_the_same_payment_from_library_or_graveyard() {
        for (zone, life_frozen) in [
            (Zone::Library { owner: PlayerId(0) }, false),
            (Zone::Graveyard { owner: PlayerId(0) }, false),
            (Zone::Graveyard { owner: PlayerId(0) }, true),
        ] {
            let board = entry_land_board(StaticEffect::PayLifeOrEnterTapped { amount: 2.into() })
                .card("Forest", 0, 0, zone)
                .libraries("Forest", 4);
            let mut game = Game::new(board.prepare());
            let object = Driver::object(&game, 0);
            let starting_life = game.state.rules.players[0].life.clone();
            game.rules_mut().players[0].life_frozen_until_next_turn = life_frozen;
            game.begin_battlefield_entry(object, BatchCause::Fixture, Frame::Priority, false)
                .unwrap();
            let decision = Driver::request(&mut game, u64::MAX);
            let mut expected_life = starting_life;
            if life_frozen {
                assert_ne!(decision.reason, Reason::OptionalEffect);
            } else {
                assert_eq!(decision.reason, Reason::OptionalEffect);
                let answer = decision
                    .pick_one(&Candidate::Flag { flag: Flag::Accept })
                    .unwrap();
                game.respond(decision.player, decision.id, answer).unwrap();
                let _ = Driver::request(&mut game, u64::MAX);
                expected_life.subtract(&2.into());
            }
            assert_eq!(game.state.rules.players[0].life, expected_life);
            let successor = Driver::object(&game, 0);
            assert_ne!(object, successor);
            assert_eq!(game.current(successor).unwrap().tapped, life_frozen);
            assert_eq!(game.current(successor).unwrap().zone, Zone::Battlefield);
            assert_eq!(game.state.rules.turn.lands_played, 0);
            // This private board injects the life lock to exercise payment
            // eligibility. Remove it before validating the catalog's state.
            game.rules_mut().players[0].life_frozen_until_next_turn = false;
            game.validate_state().unwrap();
        }
    }

    /// "If an effect puts this land onto the battlefield tapped, you may pay
    /// 2 life, but it still enters tapped." (Lorwyn Eclipsed release notes)
    #[test]
    fn forced_tapped_entry_still_offers_the_payment_and_enters_tapped() {
        for pay in [true, false] {
            let board = entry_land_board(StaticEffect::PayLifeOrEnterTapped { amount: 2.into() })
                .card("Forest", 0, 0, Zone::Library { owner: PlayerId(0) })
                .libraries("Forest", 4);
            let mut game = Game::new(board.prepare());
            let object = Driver::object(&game, 0);
            let before = game.state_digest().unwrap();
            assert_eq!(
                game.move_object(object, Zone::Battlefield).unwrap_err(),
                "payable entry requires a battlefield entry continuation"
            );
            assert_eq!(game.state_digest().unwrap(), before);
            let mut life = game.state.rules.players[0].life.clone();
            game.begin_battlefield_entry(object, BatchCause::Fixture, Frame::Priority, true)
                .unwrap();
            // A rules transaction publishes on commit; this white-box call
            // outside one publishes explicitly.
            game.refresh_visibility().unwrap();
            let decision = Driver::request(&mut game, u64::MAX);
            assert_eq!(decision.reason, Reason::OptionalEffect);
            // The entrant is the slot's subject.
            let Shape::Pick { slots, .. } = &decision.shape else {
                panic!("{decision:?}")
            };
            assert_eq!(
                slots[0]
                    .subject
                    .map(|i| decision.candidates[i as usize].clone()),
                Some(Candidate::Object { object })
            );
            // Its controller observes the library entrant it decides about;
            // nobody else sees a library card.
            let observed = game.observe(PlayerId(0), Budget::unlimited());
            let mine = match observed {
                QueryResult::Complete(QueryValue::Observation(mine)) => mine,
                QueryResult::Rejected(rejection) => panic!("observation rejected: {rejection:?}"),
                _ => panic!("the entering controller observes its decision"),
            };
            let pending = mine.decision.as_ref().expect("a pending entry payment");
            let Candidate::Object { object: handle } = &pending.candidates[2] else {
                panic!("{pending:?}")
            };
            assert!(
                mine.view
                    .objects
                    .iter()
                    .any(|o| &o.handle == handle && matches!(o.zone, Zone::Library { .. }))
            );
            let QueryResult::Complete(QueryValue::Observation(theirs)) =
                game.observe(PlayerId(1), Budget::unlimited())
            else {
                panic!("an opponent observes")
            };
            assert!(
                theirs
                    .view
                    .objects
                    .iter()
                    .all(|o| !matches!(o.zone, Zone::Library { .. }))
            );
            game.validate_state().unwrap();
            let flag = if pay { Flag::Accept } else { Flag::Decline };
            let answer = decision.pick_one(&Candidate::Flag { flag }).unwrap();
            game.respond(decision.player, decision.id, answer).unwrap();
            let _ = Driver::request(&mut game, u64::MAX);
            if pay {
                life.subtract(&2.into());
            }
            let successor = game.current(Driver::object(&game, 0)).unwrap();
            assert_eq!(successor.zone, Zone::Battlefield);
            assert!(successor.tapped);
            assert_eq!(game.state.rules.players[0].life, life);
            game.validate_state().unwrap();
        }
    }

    #[test]
    fn simultaneous_fastlands_both_count_the_same_prior_lands() {
        let board = entry_land_board(StaticEffect::EntersTappedIf {
            condition: EntryCondition::UnlessControlled {
                filter: Pred(vec![Atom::Type {
                    kind: CardKind::Land,
                }]),
                cmp: Cmp::AtMost,
                count: 2.into(),
            },
        })
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, true)
        .libraries("Forest", 4);
        let mut game = Game::new(board.prepare());
        let occurrences = [2, 3].map(|id| ProposedEvent::Move {
            object: Driver::object(&game, id),
            destination: Zone::Battlefield,
        });
        game.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: occurrences.into(),
            },
        )
        .unwrap();
        for id in [2, 3] {
            let entrant = game.current(Driver::object(&game, id)).unwrap();
            assert_eq!(entrant.zone, Zone::Battlefield);
            assert!(!entrant.tapped);
        }
        game.validate_state().unwrap();
    }
}

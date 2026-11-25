//! CR 119.4, 614.1c and 614.12.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingEntry {
    object: ObjectId,
    controller: PlayerId,
    cause: BatchCause,
    pub(super) resume: FrameLink,
}

enum EntryPayment {
    NoneRequired,
    Declined,
    Paid { player: PlayerId, amount: Quantity },
}

impl RulesTxn {
    pub(super) fn entry_life_cost(&self, object: ObjectId) -> Result<Option<Quantity>, String> {
        Ok(self
            .copiable_definition(object)?
            .static_abilities
            .iter()
            .find_map(|ability| match &ability.effect {
                StaticEffect::PayLifeOrEnterTapped { amount } => Some(amount.clone()),
                _ => None,
            }))
    }

    pub(super) fn can_pay_entry_life(&self, controller: PlayerId, amount: &Quantity) -> bool {
        self.is_live(controller)
            && self.state.rules.players[usize::from(controller.0)].can_pay_life(amount)
    }

    /// Own a battlefield move until the entry decision and the move are complete.
    /// The calling rule resumes only after the entire entry batch commits.
    pub(in crate::engine) fn begin_battlefield_entry(
        &mut self,
        object: ObjectId,
        cause: BatchCause,
        resume: Frame,
    ) -> Result<(), String> {
        let controller = self.current(object)?.controller;
        if self
            .entry_life_cost(object)?
            .is_some_and(|amount| self.can_pay_entry_life(controller, &amount))
        {
            let resume = self.save_frame(resume);
            self.rules_mut().frame = Frame::BattlefieldEntry {
                entry: Box::new(PendingEntry {
                    object,
                    controller,
                    cause,
                    resume,
                }),
            };
            return Ok(());
        }
        self.finish_battlefield_entry(object, cause, resume, EntryPayment::NoneRequired)
    }

    pub(super) fn battlefield_entry_request(
        &self,
        entry: &PendingEntry,
    ) -> Result<Option<Decision<Rules>>, String> {
        if self.current(entry.object).is_err() || !self.is_live(entry.controller) {
            return Ok(None);
        }
        let amount = self
            .entry_life_cost(entry.object)?
            .ok_or("missing entry life cost")?;
        Ok(self.can_pay_entry_life(entry.controller, &amount).then(|| {
            self.pick_flags(
                entry.controller,
                Reason::OptionalEffect,
                &[Flag::Accept, Flag::Decline],
            )
        }))
    }

    pub(super) fn battlefield_entry_step(&mut self, entry: PendingEntry) -> Result<(), String> {
        let resume = self.saved_frame(entry.resume)?.clone();
        let input = self.rules_mut().pending.take();
        if self.current(entry.object).is_err() || !self.is_live(entry.controller) {
            self.rules_mut().frame = resume;
            return Ok(());
        }
        let amount = self
            .entry_life_cost(entry.object)?
            .ok_or("missing entry life cost")?;
        let payment = if self.can_pay_entry_life(entry.controller, &amount) {
            let input = input.ok_or("missing entry life decision")?;
            if input.player != entry.controller {
                return Err("wrong entering controller".into());
            }
            if input.flag(Flag::Accept)? {
                EntryPayment::Paid {
                    player: entry.controller,
                    amount,
                }
            } else {
                EntryPayment::Declined
            }
        } else {
            EntryPayment::Declined
        };
        self.finish_battlefield_entry(entry.object, entry.cause, resume, payment)
    }

    fn finish_battlefield_entry(
        &mut self,
        object: ObjectId,
        cause: BatchCause,
        resume: Frame,
        payment: EntryPayment,
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
            EntryPayment::Declined | EntryPayment::Paid { .. } => {
                self.commit_entry_batch(
                    cause,
                    EventGroup::Ordered { children },
                    object,
                    matches!(payment, EntryPayment::Paid { .. }),
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
        let _ = self.saved_frame(entry.resume)?;
        if let Ok(object) = self.current(entry.object) {
            if object.zone == Zone::Battlefield
                || object.controller != entry.controller
                || self.entry_life_cost(entry.object)?.is_none()
            {
                return Err("invalid suspended battlefield entry".into());
            }
            if let BatchCause::LandPlay { player, source, .. } = entry.cause
                && (player != entry.controller
                    || source != object.id
                    || object.zone != (Zone::Hand { owner: player }))
            {
                return Err("invalid suspended land play".into());
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

    #[test]
    fn rules_move_offers_the_same_payment_from_library_or_graveyard() {
        for (zone, life_frozen) in [
            (Zone::Library { owner: PlayerId(0) }, false),
            (Zone::Graveyard { owner: PlayerId(0) }, false),
            (Zone::Graveyard { owner: PlayerId(0) }, true),
        ] {
            let mut catalog = FixtureCatalog::Witness.get().clone();
            let land = catalog
                .definitions
                .iter_mut()
                .find(|d| d.name == "Forest")
                .unwrap();
            land.source_labels = None;
            land.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::PayLifeOrEnterTapped { amount: 2.into() },
            });
            land.capabilities = land.requirements();
            let board = Board::new(catalog, "entry-life-rules-move")
                .step(TurnStep::PrecombatMain)
                .card("Forest", 0, 0, zone)
                .libraries("Forest", 4);
            let mut game = Game::new(board.prepare());
            let object = Driver::object(&game, 0);
            let starting_life = game.state.rules.players[0].life.clone();
            game.rules_mut().players[0].life_frozen_until_next_turn = life_frozen;
            game.begin_battlefield_entry(object, BatchCause::Fixture, Frame::Priority)
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
            game.validate_state().unwrap();
        }
    }
}

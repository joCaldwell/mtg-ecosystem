use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStep {
    Untap,
    Upkeep,
    Draw,
    PrecombatMain,
    BeginningCombat,
    DeclareAttackers,
    EndCombat,
    PostcombatMain,
    End,
    Cleanup,
}
impl TurnStep {
    pub fn phase(self) -> Phase {
        match self {
            Self::Untap | Self::Upkeep | Self::Draw => Phase::Beginning,
            Self::PrecombatMain => Phase::PrecombatMain,
            Self::BeginningCombat | Self::DeclareAttackers | Self::EndCombat => Phase::Combat,
            Self::PostcombatMain => Phase::PostcombatMain,
            Self::End | Self::Cleanup => Phase::Ending,
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Untap => Self::Upkeep,
            Self::Upkeep => Self::Draw,
            Self::Draw => Self::PrecombatMain,
            Self::PrecombatMain => Self::BeginningCombat,
            Self::BeginningCombat => Self::DeclareAttackers,
            Self::DeclareAttackers => Self::EndCombat,
            Self::EndCombat => Self::PostcombatMain,
            Self::PostcombatMain => Self::End,
            Self::End => Self::Cleanup,
            Self::Cleanup => Self::Untap,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnState {
    pub number: u64,
    pub step: TurnStep,
    pub lands_played: u32,
}

impl Game {
    pub fn turn(&self) -> &TurnState {
        &self.state.turn
    }
    pub(super) fn turns_enabled(&self) -> bool {
        self.prepared.config.profile == "four-seat-turns-permanents-v1"
    }
    pub(super) fn main_phase_action(&self, player: PlayerId) -> bool {
        self.is_live(player)
            && self.state.active == Some(player)
            && self.state.priority == player
            && matches!(self.state.frame, Frame::Priority)
            && self.state.stack.is_empty()
            && matches!(
                self.state.turn.step,
                TurnStep::PrecombatMain | TurnStep::PostcombatMain
            )
    }
    // CR 305.1-3: playing a land is a special action, once per turn, during
    // the active player's main phase with priority and an empty stack.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn validate_land_play(&self, player: PlayerId, card: CardId) -> Result<(), String> {
        if !self.turns_enabled()
            || !self.main_phase_action(player)
            || self.state.turn.lands_played != 0
        {
            return Err(
                "land play requires unused active-player main-phase land play and empty stack"
                    .into(),
            );
        }
        let object = self.state.objects.get(&card).ok_or("invalid land source")?;
        let definition = self.definition(card)?;
        if object.zone != (Zone::Hand { owner: player })
            || self.state.cards[&card].owner != player
            || definition.characteristics.kind != CardKind::Land
            || !definition.capabilities.contains(&Capability::LandPlayV1)
        {
            return Err("invalid or unsupported land play".into());
        }
        Ok(())
    }
    pub(super) fn play_land(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        card: CardId,
    ) -> Result<(), String> {
        self.validate_land_play(player, card)?;
        let object = self.object_id(card).ok_or("missing land source")?;
        self.commit_batch(
            BatchCause::LandPlay {
                player,
                decision_id,
                source: object,
            },
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Move {
                            object,
                            destination: Zone::Battlefield,
                        },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::LandPlayed { player },
                    },
                ],
            },
        )?;
        self.state.passes.clear();
        self.stabilize()
    }
    // CR 500.5, 106.4: pools empty at each step/phase ending. CR 500-514:
    // transition and entry work are separate serializable, budgeted frames.
    // A departed active player's turn continues (800.4j), without its personal
    // turn actions, until cleanup advances to the next living seat.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn transition_turn_step(&mut self) -> Result<(), String> {
        if !self.turns_enabled() || !self.state.stack.is_empty() || self.state.pending.is_some() {
            return Err("unsupported phase transition boundary".into());
        }
        let (next, owner) = self.next_turn_state()?;
        let mut children = self
            .live_players()
            .into_iter()
            .map(|player| EventGroup::Occurrence {
                proposed: ProposedEvent::ClearMana { player },
            })
            .collect::<Vec<_>>();
        children.push(EventGroup::Occurrence {
            proposed: ProposedEvent::TurnTransition { next, owner },
        });
        self.commit_batch(
            BatchCause::TurnProgression,
            EventGroup::Ordered { children },
        )?;
        self.state.passes.clear();
        self.state.frame = Frame::EnterStep;
        Ok(())
    }
    pub(super) fn next_turn_state(&self) -> Result<(TurnState, PlayerId), String> {
        let mut next = self.state.turn.clone();
        next.step = next.step.next();
        let mut owner = self.state.turn_owner;
        if self.state.turn.step == TurnStep::Cleanup {
            next.number = next
                .number
                .checked_add(1)
                .ok_or("turn number resource limit")?;
            next.lands_played = 0;
            owner = self.next_live(owner).ok_or("no next turn owner")?;
        }
        Ok((next, owner))
    }
    pub(super) fn enter_turn_step(&mut self) -> Result<(), String> {
        if !self.turns_enabled() || !self.state.stack.is_empty() || self.state.pending.is_some() {
            return Err("unsupported turn entry boundary".into());
        }
        match self.state.turn.step {
            TurnStep::Untap => {
                // CR 502.3-4: simultaneous untap, with no priority in this step.
                let occurrences = self
                    .state
                    .objects
                    .values()
                    .filter(|object| {
                        object.zone == Zone::Battlefield
                            && object.tapped
                            && Some(object.controller) == self.state.active
                    })
                    .map(|object| ProposedEvent::Untap { object: object.id })
                    .collect::<Vec<_>>();
                if !occurrences.is_empty() {
                    self.commit_batch(
                        BatchCause::UntapStep,
                        EventGroup::Simultaneous { occurrences },
                    )?;
                }
                self.state.frame = Frame::Transition;
                return Ok(());
            }
            TurnStep::Draw => {
                // CR 504.1: mandatory turn-based draw before priority. Only the
                // reviewed single-card library domain has an ordering here.
                if let Some(player) = self.state.active {
                    let library = self
                        .state
                        .objects
                        .values()
                        .filter(|object| object.zone == (Zone::Library { owner: player }))
                        .map(|object| object.id)
                        .collect::<Vec<_>>();
                    if library.len() != 1 {
                        return Err(
                            "unsupported draw: library must contain exactly one card".into()
                        );
                    }
                    self.move_object_for(
                        library[0],
                        Zone::Hand { owner: player },
                        BatchCause::DrawStep { player },
                    )?;
                }
            }
            TurnStep::DeclareAttackers => {
                // CR 508.1/508.8: this slice supports only forced empty
                // declaration, skipping blockers/damage. Combat with any
                // controlled creature rejects, even if eligibility is unknown.
                if self.state.objects.values().any(|object| {
                    object.zone == Zone::Battlefield
                        && Some(object.controller) == self.state.active
                        && self
                            .definition(object.id.card)
                            .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
                }) {
                    return Err("unsupported combat declaration with controlled creatures".into());
                }
            }
            TurnStep::Cleanup => {
                // CR 514.1/514.3: no cleanup priority in this restricted domain;
                // discard and any damage/effect cleanup remain unsupported.
                if let Some(player) = self.state.active {
                    let cards = self
                        .state
                        .objects
                        .values()
                        .filter(|object| object.zone == (Zone::Hand { owner: player }))
                        .count();
                    if cards > 7 {
                        return Err("unsupported cleanup discard requirement".into());
                    }
                }
                self.stabilize()?;
                self.state.frame = Frame::Transition;
                return Ok(());
            }
            _ => {}
        }
        self.stabilize()?;
        self.state.priority = self
            .state
            .active
            .or_else(|| self.next_live(self.state.turn_owner))
            .ok_or("no living priority recipient")?;
        self.state.frame = Frame::Priority;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new(
            Prepared::admit(
                Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
                ScenarioConfig::turns_permanents(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn land_and_absent_active_transition_capture_consumed_turn_facts() {
        let mut game = game();
        let landed = game
            .commit(
                ProposedEvent::LandPlayed {
                    player: PlayerId(0),
                },
                BatchCause::Fixture,
            )
            .unwrap();
        assert_eq!(
            landed,
            EventOutcome::LandPlayCounted {
                player: PlayerId(0),
                previous: 0,
                current: 1
            }
        );
        let batch = game.trace.records.iter().find_map(|record| {
            if let EvidenceRecord::Batch { batch } = &record.record {
                Some(batch)
            } else {
                None
            }
        });
        let facts = batch.unwrap().before.turn.as_ref().unwrap();
        assert_eq!(facts.turn.step, TurnStep::PostcombatMain);
        assert_eq!(facts.turn.lands_played, 0);
        assert_eq!(facts.priority, Some(PlayerId(0)));
        assert_eq!(
            facts.live_turn_order,
            vec![PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)]
        );
        assert!(facts.stack_empty);
        game.concede(PlayerId(0)).unwrap();
        let _ = game.advance(Budget::new(1));
        game.state.turn.step = TurnStep::Cleanup;
        game.state.frame = Frame::Transition;
        let (next, owner) = game.next_turn_state().unwrap();
        let batch = game
            .commit_batch(
                BatchCause::TurnProgression,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::TurnTransition { next, owner },
                },
            )
            .unwrap();
        let facts = batch.occurrences[0].before.turn.as_ref().unwrap();
        assert_eq!(facts.active, None);
        assert_eq!(facts.priority, None);
        assert_eq!(facts.owner, PlayerId(0));
        assert_eq!(
            facts.live_turn_order,
            vec![PlayerId(1), PlayerId(2), PlayerId(3)]
        );
        assert_eq!(game.state.active, Some(PlayerId(1)));
        assert_eq!(game.state.turn.lands_played, 0);
    }

    #[test]
    fn failed_land_count_rolls_back_earlier_movement_and_evidence() {
        let mut game = game();
        game.state.turn.lands_played = 1;
        let before = game.snapshot().unwrap();
        let object = game.object_id(CardId(1)).unwrap();
        assert!(
            game.commit_batch(
                BatchCause::Fixture,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::Move {
                                object,
                                destination: Zone::Battlefield
                            }
                        },
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::LandPlayed {
                                player: PlayerId(0)
                            }
                        },
                    ]
                },
            )
            .is_err()
        );
        assert_eq!(game.snapshot().unwrap(), before);
    }

    #[test]
    fn invalid_turn_transition_rolls_back_mana_and_turn_facts() {
        let mut game = game();
        game.state.frame = Frame::Transition;
        game.state.players[0]
            .mana
            .insert(Color::G, Quantity::from(2));
        let before = game.snapshot().unwrap();
        let mut next = game.state.turn.clone();
        next.step = TurnStep::PrecombatMain;
        assert!(
            game.commit_batch(
                BatchCause::TurnProgression,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::ClearMana {
                                player: PlayerId(0)
                            }
                        },
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::TurnTransition {
                                next,
                                owner: PlayerId(0)
                            }
                        },
                    ]
                },
            )
            .is_err()
        );
        assert_eq!(game.snapshot().unwrap(), before);
        game.state.turn.number = u64::MAX;
        game.state.turn.step = TurnStep::Cleanup;
        let before = game.snapshot().unwrap();
        assert!(game.transition_turn_step().is_err());
        assert_eq!(game.snapshot().unwrap(), before);
    }

    #[test]
    fn untap_simultaneity_rejects_interacting_writes_and_keeps_pre_event_facts() {
        let mut game = game();
        let object = game.object_id(CardId(0)).unwrap();
        game.state.objects.get_mut(&CardId(0)).unwrap().tapped = true;
        let before = game.snapshot().unwrap();
        assert!(
            game.commit_batch(
                BatchCause::UntapStep,
                EventGroup::Simultaneous {
                    occurrences: vec![
                        ProposedEvent::Untap { object },
                        ProposedEvent::Tap { object },
                    ]
                },
            )
            .is_err()
        );
        assert_eq!(game.snapshot().unwrap(), before);
        let batch = game
            .commit_batch(
                BatchCause::UntapStep,
                EventGroup::Simultaneous {
                    occurrences: vec![ProposedEvent::Untap { object }],
                },
            )
            .unwrap();
        assert!(batch.before.objects[0].tapped);
        assert_eq!(batch.result(&[0]), Some(&EventOutcome::Untapped { object }));
        assert!(!game.state.objects[&CardId(0)].tapped);
    }
}

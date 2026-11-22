use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case", deny_unknown_fields)]
pub enum StackKind {
    #[default]
    Spell,
    Triggered {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_condition: Option<InterveningCheck>,
        ability: EffectiveTriggerAbility,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grant: Option<CascadeGrantProvenance>,
        source_handles: Vec<ObjectHandle>,
        trigger_id: TriggerId,
        event: TriggerOccurrenceId,
        witness: TriggerEventWitness,
    },
    Activated {
        ability: u32,
        source_handles: Vec<ObjectHandle>,
    },
}
impl StackKind {
    pub(super) fn activated_index(&self) -> Option<u32> {
        match self {
            Self::Activated { ability, .. } => Some(*ability),
            _ => None,
        }
    }
    pub fn event_player(&self) -> Result<Option<PlayerId>, String> {
        match self {
            Self::Triggered {
                witness:
                    witness @ (TriggerEventWitness::StepBegan { .. }
                    | TriggerEventWitness::SpellCast { .. }),
                ..
            } => Ok(witness.event_player()),
            _ => Err("unbound event-player recipient".into()),
        }
    }
    pub fn is_spell(&self) -> bool {
        matches!(self, Self::Spell)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "choice", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResolutionChoice {
    Demonstrate {
        source_block: u32,
        first: Option<spell_copy::CopyIdentity>,
    },
    HandReturn {
        player: PlayerId,
        hand: Vec<ObjectId>,
        source_block: u32,
    },
    LibraryOrder {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    OptionalShuffle {
        player: PlayerId,
        source_block: u32,
    },
    LookTopMana {
        player: PlayerId,
        looked: Vec<ObjectId>,
        revealed: bool,
    },
    Hideaway {
        player: PlayerId,
        looked: Vec<ObjectId>,
        bottom: Option<hideaway::RandomBottom>,
    },
    LibrarySelect {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    Surveil {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    Scry {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    Search {
        player: PlayerId,
        maximum: u32,
        split: bool,
        source_block: u32,
    },
    ColoredSearch {
        player: PlayerId,
        source_block: u32,
        revealed: Option<Vec<(Color, ObjectId)>>,
    },
    SearchSplit {
        player: PlayerId,
        objects: Vec<ObjectId>,
        source_block: u32,
    },
    Shuffle {
        player: PlayerId,
        index: u32,
        source_block: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fetched_land: Option<MoveOutcome>,
    },
    OptionalEffect {
        player: PlayerId,
        target: ObjectId,
        source_block: u32,
    },
    ChooseOptional {
        player: PlayerId,
        source_block: u32,
    },
    OptionalDraw {
        player: PlayerId,
        source_block: u32,
    },
    SetColor {
        source_block: u32,
    },
    Proliferate {
        source_block: u32,
        choices: proliferate::ProliferateChoices,
    },
    OptionalRecursion {
        player: PlayerId,
        target: ObjectId,
        source_block: u32,
    },
    Sacrifice {
        player: PlayerId,
        source_block: u32,
    },
    Discard {
        player: PlayerId,
        count: usize,
        source_block: u32,
    },
}
impl Game {
    pub(super) fn opponents(&self, player: PlayerId) -> Vec<PlayerId> {
        self.live_players()
            .into_iter()
            .filter(|p| *p != player)
            .collect()
    }
    pub(super) fn bound_target_count(&self, entry: &StackInstance) -> usize {
        entry.targets.len()
            + entry.player_targets.len()
            + entry.stack_targets.len()
            + entry.entity_targets.len()
    }
    pub(super) fn top_has_target_domain(&self) -> Result<bool, String> {
        let top = self.top()?;
        Ok(self.has_target_domain(
            self.program_for(top)?,
            top.controller,
            self.targeting_characteristics(top)?.as_ref(),
        ))
    }

    pub(crate) fn has_target_domain(
        &self,
        program: &SpellProgram,
        player: PlayerId,
        source: &Characteristics,
    ) -> bool {
        if let SpellResolution::ChooseModes { exactly } = program.resolution {
            return program
                .modes
                .iter()
                .filter(|mode| self.has_target_domain(&mode.program, player, source))
                .take(exactly as usize)
                .count()
                == exactly as usize;
        }
        if !program.modes.is_empty() {
            return program
                .modes
                .iter()
                .any(|mode| self.has_target_domain(&mode.program, player, source));
        }
        match program.targets.first().map(|target| target.domain) {
            Some(TargetDomain::PlayerGraveyardCards { .. }) => false,
            None => true,
            Some(TargetDomain::AnyDamageTarget) => {
                self.live_players()
                    .into_iter()
                    .any(|p| self.targetable_player(p))
                    || self.state.rules.objects.values().any(|o| {
                        self.is_battlefield_creature(o.id) && !self.protected_from(o.id, source)
                    })
            }
            Some(
                domain @ (TargetDomain::AnyPlayer
                | TargetDomain::PlayerOrPlaneswalker
                | TargetDomain::OpponentPlayer),
            ) => !self.eligible_players(domain, player).is_empty(),
            Some(
                TargetDomain::StackSpell
                | TargetDomain::ControlledCreatureSpell
                | TargetDomain::StackActivatedOrTriggeredAbility,
            ) => !self
                .eligible_stack_targets_for(&program.targets[0], player, None)
                .is_empty(),
            Some(
                TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent,
            ) => self.state.rules.objects.values().any(|o| {
                !self.protected_from(o.id, source)
                    && self.permanent_target_legal(o.id, player, &program.targets[0])
            }),
            Some(
                domain @ (TargetDomain::OwnGraveyardCreatureCard | TargetDomain::OwnGraveyardCard),
            ) => self
                .state
                .rules
                .objects
                .values()
                .any(|object| self.graveyard_target_legal(object, player, domain)),
            Some(TargetDomain::BlockingBattlefieldCreature) => {
                self.state.rules.blocking_creatures.iter().any(|o| {
                    self.is_blocking_creature(*o)
                        && self.targetable_by(*o, player)
                        && !self.protected_from(*o, source)
                })
            }
            Some(
                domain @ (TargetDomain::CreatureOrPlaneswalker
                | TargetDomain::BattlefieldCreature
                | TargetDomain::ControlledBattlefieldCreature),
            ) => self.state.rules.objects.values().any(|o| {
                (if domain == TargetDomain::CreatureOrPlaneswalker {
                    self.creature_or_planeswalker(o.id)
                } else {
                    self.is_battlefield_creature(o.id)
                }) && self.targetable_by(o.id, player)
                    && !self.protected_from(o.id, source)
                    && (domain != TargetDomain::ControlledBattlefieldCreature
                        || o.controller == player)
            }),
        }
    }
    pub(crate) fn announced_target_legality(&self, entry: &StackInstance) -> Vec<bool> {
        if entry.mode_subset.is_some() {
            let Ok(program) = self.program_for(entry) else {
                return vec![];
            };
            let mut legal = vec![];
            for (slot, target) in program.targets.iter().enumerate() {
                match target.domain {
                    TargetDomain::PlayerGraveyardCards { .. } => {
                        if let Some((TargetEntity::Player { player }, cards)) =
                            entry.entity_targets.split_first()
                        {
                            legal.extend(cards.iter().map(|target|
                                matches!(target, TargetEntity::Object { object } if self.current(*object).is_ok_and(|o| self.player_graveyard_card(o,*player)))
                            ));
                        }
                    }
                    TargetDomain::AnyPlayer
                        if matches!(program.targets.get(slot + 1).map(|t|t.domain),
                        Some(TargetDomain::PlayerGraveyardCards {owner_slot,..}) if owner_slot as usize==slot) =>
                    {
                        legal.push(matches!(entry.entity_targets.first(),Some(TargetEntity::Player{player}) if self.player_target_legal(entry,slot,*player)));
                    }
                    TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer => {
                        let index = self.selected_storage_index(entry, slot, |d| {
                            matches!(d, TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer)
                        });
                        legal.push(
                            index
                                .and_then(|i| entry.player_targets.get(i))
                                .is_some_and(|p| self.player_target_legal(entry, slot, *p)),
                        );
                    }
                    TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility => {
                        legal.push(self.bound_stack_target(entry, slot).is_some_and(|id| {
                            self.eligible_stack_targets_for(
                                target,
                                entry.controller,
                                Some(entry.id),
                            )
                            .contains(id)
                        }));
                    }
                    TargetDomain::AnyDamageTarget => legal.push(
                        self.bound_damage_target(entry, slot)
                            .is_some_and(|t| self.entity_target_legal(*t, entry)),
                    ),
                    _ => {
                        legal.push(self.bound_object_target(entry, slot).is_some_and(|id| {
                            self.object_target_legal_for(entry, *id, Some(target))
                        }))
                    }
                }
            }
            return legal;
        }
        if !entry.entity_targets.is_empty() {
            entry
                .entity_targets
                .iter()
                .map(|t| self.entity_target_legal(*t, entry))
                .collect()
        } else if !entry.stack_targets.is_empty() {
            entry
                .stack_targets
                .iter()
                .enumerate()
                .map(|(slot, target)| {
                    self.program_for(entry).is_ok_and(|program| {
                        program.targets.get(slot).is_some_and(|spec| {
                            self.eligible_stack_targets_for(spec, entry.controller, Some(entry.id))
                                .contains(target)
                        })
                    })
                })
                .collect()
        } else if !entry.player_targets.is_empty() {
            entry
                .player_targets
                .iter()
                .enumerate()
                .map(|(slot, p)| self.player_target_legal(entry, slot, *p))
                .collect()
        } else {
            entry
                .targets
                .iter()
                .map(|target| self.object_target_legal(entry, *target))
                .collect()
        }
    }
    pub(crate) fn controlled_creatures(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.controller == player && self.is_battlefield_creature(o.id))
            .map(|o| o.id)
            .collect()
    }
    // CR 603.3a/608.2c/701.9a-b/701.21a: the damaged player is frozen by the
    // actual trigger occurrence. The invocation's controller remains "you";
    // neither source control changes nor departures rebind either player.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn resolution_player(
        &self,
        frame: &Resolving,
        recipient: Option<&ActionRecipient>,
    ) -> Result<PlayerId, String> {
        match recipient {
            None | Some(ActionRecipient::Controller) => Ok(self.entry(frame.stack_id)?.controller),
            Some(ActionRecipient::EventPlayer) => {
                let StackKind::Triggered { witness, .. } = &self.entry(frame.stack_id)?.kind else {
                    return Err("unbound damage-recipient action".into());
                };
                let event = match witness {
                    TriggerEventWitness::Equipped {
                        occurrence: event, ..
                    } => event.as_ref(),
                    other => other,
                };
                match event {
                    TriggerEventWitness::DamageDealt { damage, .. } => match damage.recipient {
                        DamageRecipient::Player { player } => Ok(player),
                        DamageRecipient::Creature { .. } => {
                            Err("action event requires a player damage recipient".into())
                        }
                    },
                    _ => Err("unbound damage-recipient action".into()),
                }
            }
        }
    }
    fn discard_spec<'a>(
        &self,
        frame: &Resolving,
        instruction: &'a Instruction,
    ) -> Result<(PlayerId, &'a Quantity, u32), String> {
        match instruction {
            kernel!(Discard { player: player @ (you!() | event_player!()), amount: fixed!(count), random: false } @ source_block) =>
            {
                let recipient = player
                    .is_event_player()
                    .then_some(ActionRecipient::EventPlayer);
                Ok((
                    self.resolution_player(frame, recipient.as_ref())?,
                    count,
                    *source_block,
                ))
            }
            kernel!(Discard { player: mtg_contracts::kir::PlayerExpr::EachPlayer | mtg_contracts::kir::PlayerExpr::EachOpponent, amount: fixed!(count), random: false } @ source_block) =>
            {
                let player = frame
                    .group_discard
                    .as_ref()
                    .and_then(|group| group.remaining.first())
                    .ok_or("missing group discard chooser")?;
                Ok((*player, count, *source_block))
            }
            kernel!(Discard { player: target_player!(target_slot), amount: fixed!(count), random: false } @ source_block) =>
            {
                // A sole illegal target ends resolution before instruction dispatch.
                let slot = *target_slot as usize;
                if frame.legal_targets.get(slot) != Some(&true) {
                    return Err("missing legal discard target".into());
                }
                let player = *self
                    .entry(frame.stack_id)?
                    .player_targets
                    .get(slot)
                    .ok_or("missing discard player target")?;
                Ok((player, count, *source_block))
            }
            _ => Err("expected discard instruction".into()),
        }
    }
    pub(crate) fn action_hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: player }))
            .map(|o| o.id)
            .collect()
    }
    pub(crate) fn validate_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        let source = self.current(object)?;
        let definition = self.copiable_definition(object)?;
        let ability = definition
            .activated
            .get(ability as usize)
            .ok_or("unknown activated ability")?;
        if !self.is_live(player)
            || match ability.functioning_zone {
                FunctioningZone::Battlefield => {
                    !source.present_on_battlefield() || source.controller != player
                }
                FunctioningZone::Hand => !self.hand_activation_source_payable(player, object),
                FunctioningZone::Stack | FunctioningZone::Any => true,
            }
        {
            return Err("invalid activated ability source/permission".into());
        }
        if ability.program.timing == SpellTiming::MainPhase && !self.main_phase_action(player) {
            return Err("equip requires sorcery timing".into());
        }
        if let ActivationCost::TapPayLifeSacrificeSource { life } = &ability.cost {
            self.validate_tap_life_sacrifice_source(player, object, life)?;
        }
        let tap = ability.cost == ActivationCost::TapSource
            || matches!(
                ability.cost,
                ActivationCost::TapPayLifeSacrificeSource { .. }
            )
            || matches!(&ability.cost, ActivationCost::Composite { costs } if costs.tap);
        if tap
            && (source.tapped
                || (self.characteristics(object)?.kind == CardKind::Creature
                    && !source.attack_ready
                    && !self.has_keyword(source.id, KeywordAbility::Haste)))
        {
            return Err("tap ability requires an untapped continuously controlled creature".into());
        }
        if let ActivationCost::SacrificePermanent { filter, count, .. } = &ability.cost
            && usize::try_from(*count).map_or(true, |n| {
                self.sacrifice_candidates(player, filter).len() < n
            })
        {
            return Err("no payable sacrifice permanent".into());
        }
        if !self.has_target_domain(&ability.program, player, self.color_layer(object)?.as_ref()) {
            return Err("no eligible ability targets".into());
        }
        Ok(())
    }
    // CR 602.2, 113.7a/113.8: a nonmana ability is its own stack object;
    // sacrificing its source pays the cost without moving a card to the stack.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn activate_ability(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        self.check_stack_capacity(1)?;
        self.validate_activation(player, object, ability)?;
        self.announce_ability(player, decision_id, object, ability)
    }
    pub(crate) fn actions_request(&self) -> Result<Option<Request>, String> {
        let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame else {
            return Ok(None);
        };
        let decision_id = self.state.rules.revision;
        Ok(match choice {
            ResolutionChoice::SetColor { .. } => self.target_color_request(frame),
            ResolutionChoice::Proliferate { choices, .. } => {
                self.proliferate_request(frame, choices)
            }
            ResolutionChoice::ColoredSearch {
                player,
                revealed: None,
                ..
            } if self.is_live(*player) => Some(self.colored_request(frame, *player)?),
            ResolutionChoice::Demonstrate { first, .. }
                if self.is_live(self.entry(frame.stack_id)?.controller) =>
            {
                let player = self.entry(frame.stack_id)?.controller;
                if first.is_none() {
                    Some(Request::OptionalEffect {
                        player,
                        decision_id,
                    })
                } else {
                    let candidates = self.opponents(player);
                    (!candidates.is_empty()).then_some(Request::ChooseOpponent {
                        player,
                        decision_id,
                        candidates,
                    })
                }
            }
            ResolutionChoice::HandReturn { player, .. } if self.is_live(*player) => {
                Some(Request::HandReturn {
                    decision_id,
                    player: *player,
                    count: self.hand_return_count_for_current()?,
                    candidates: self.action_hand(*player),
                })
            }
            ResolutionChoice::LibraryOrder { player, looked, .. } if self.is_live(*player) => {
                Some(Request::LibraryOrder {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                    count: looked.len(),
                })
            }
            ResolutionChoice::LookTopMana {
                player,
                looked,
                revealed,
            } if self.is_live(*player) => {
                Some(self.look_top_mana_request(frame, *player, looked, *revealed)?)
            }
            ResolutionChoice::Hideaway {
                player,
                looked,
                bottom: None,
            } if self.is_live(*player) => Some(Request::Hideaway {
                decision_id,
                player: *player,
                candidates: looked.clone(),
            }),
            ResolutionChoice::LibrarySelect { player, looked, .. } if self.is_live(*player) => {
                Some(Request::LibrarySelect {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                    remainder: self.selection_destination(frame)?,
                })
            }
            ResolutionChoice::Surveil { player, looked, .. } if self.is_live(*player) => {
                Some(Request::Surveil {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                })
            }
            ResolutionChoice::Scry { player, looked, .. } if self.is_live(*player) => {
                Some(Request::Scry {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                })
            }
            ResolutionChoice::Search {
                player, maximum, ..
            } if self.is_live(*player) => Some(Request::SearchLands {
                decision_id,
                player: *player,
                maximum: *maximum,
                candidates: self.search_lands(*player),
            }),
            ResolutionChoice::SearchSplit {
                player, objects, ..
            } if self.is_live(*player) && objects.len() == 2 => {
                Some(Request::SearchLandBattlefield {
                    decision_id,
                    player: *player,
                    candidates: objects.clone(),
                })
            }
            ResolutionChoice::OptionalShuffle { player, .. }
            | ResolutionChoice::OptionalEffect { player, .. }
            | ResolutionChoice::ChooseOptional { player, .. }
            | ResolutionChoice::OptionalDraw { player, .. }
                if self.is_live(*player) =>
            {
                Some(Request::OptionalEffect {
                    decision_id,
                    player: *player,
                })
            }
            ResolutionChoice::OptionalRecursion { player, .. } if self.is_live(*player) => {
                Some(Request::OptionalRecursion {
                    decision_id,
                    player: *player,
                })
            }
            ResolutionChoice::Sacrifice { player, .. } if self.is_live(*player) => {
                let candidates = self.controlled_creatures(*player);
                (!candidates.is_empty()).then_some(Request::Sacrifice {
                    decision_id,
                    player: *player,
                    candidates,
                })
            }
            ResolutionChoice::Discard { player, count, .. } if self.is_live(*player) => {
                let candidates = self.action_hand(*player);
                (!candidates.is_empty()).then_some(Request::ResolveDiscard {
                    decision_id,
                    player: *player,
                    count: std::cmp::min(*count, candidates.len()),
                    candidates,
                })
            }
            _ => None,
        })
    }
    pub(crate) fn validate_actions_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>> {
        if let Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::SetColor { .. },
        } = &self.state.rules.frame
        {
            return Some(self.validate_target_color_response(frame, player, response));
        }
        if let Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Proliferate { choices, .. },
        } = &self.state.rules.frame
        {
            return Some(self.validate_proliferate_choice(frame, choices, player, response));
        }
        if let Some(result) = self.validate_look_top_mana_response(player, response) {
            return Some(result);
        }
        if let (
            Frame::ResolutionChoice {
                choice:
                    ResolutionChoice::OptionalShuffle {
                        player: chooser, ..
                    }
                    | ResolutionChoice::OptionalEffect {
                        player: chooser, ..
                    }
                    | ResolutionChoice::ChooseOptional {
                        player: chooser, ..
                    }
                    | ResolutionChoice::OptionalDraw {
                        player: chooser, ..
                    },
                ..
            },
            Response::OptionalEffect { .. },
        ) = (&self.state.rules.frame, response)
        {
            return Some(if *chooser == player && self.is_live(player) {
                Ok(())
            } else {
                Err("invalid optional effect choice".into())
            });
        }
        if let Frame::ResolutionChoice {
            frame,
            choice:
                ResolutionChoice::ColoredSearch {
                    player: actor,
                    revealed: None,
                    ..
                },
        } = &self.state.rules.frame
        {
            return Some(match response {
                Response::ColoredSearch { assignments }
                    if player == *actor && self.is_live(player) =>
                {
                    self.validate_colored_answer(frame, player, assignments)
                }
                _ => Err("invalid colored library decision".into()),
            });
        }
        if let Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Demonstrate { first, .. },
        } = &self.state.rules.frame
        {
            return Some(
                if self.is_live(player)
                    && self
                        .entry(frame.stack_id)
                        .is_ok_and(|entry| entry.controller == player)
                    && match (first, response) {
                        (None, Response::OptionalEffect { .. }) => true,
                        (Some(_), Response::ChooseOpponent { opponent }) => {
                            *opponent != player && self.is_live(*opponent)
                        }
                        _ => false,
                    }
                {
                    Ok(())
                } else {
                    Err("invalid demonstrate choice".into())
                },
            );
        }
        if let Some(result) = self.validate_staples_response(player, response) {
            return Some(result);
        }
        if let Some(result) = self.validate_trigger_response(player, response) {
            return Some(result);
        }
        match (&self.state.rules.frame, response) {
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::HandReturn {
                            player: chooser,
                            source_block,
                            ..
                        },
                },
                Response::HandReturn { objects },
            ) => Some(
                self.validate_hand_return(frame, *chooser, *source_block, objects)
                    .and_then(|_| {
                        if player == *chooser {
                            Ok(())
                        } else {
                            Err("wrong hand return chooser".into())
                        }
                    }),
            ),
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::LibraryOrder {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::LibraryOrder { objects },
            ) => Some(
                self.validate_library_order_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid ordering chooser".into());
                        }
                        self.validate_library_partition(player, looked, objects, &[])
                    }),
            ),
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::Hideaway {
                            player: chooser,
                            looked,
                            bottom: None,
                        },
                },
                Response::Hideaway { object },
            ) => Some(
                self.validate_hideaway_choice(frame, *chooser, looked, None)
                    .and_then(|_| {
                        if player == *chooser && self.is_live(player) && looked.contains(object) {
                            Ok(())
                        } else {
                            Err("invalid hideaway selection".into())
                        }
                    }),
            ),
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::LibrarySelect {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::LibrarySelect { hand, remainder },
            ) => Some(
                self.validate_selection_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid library chooser".into());
                        }
                        self.validate_selection_partition(player, looked, hand, remainder)
                    }),
            ),

            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::Surveil {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::Surveil { top, graveyard },
            ) => Some(
                self.validate_surveil_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid surveil chooser".into());
                        }
                        self.validate_library_partition(player, looked, top, graveyard)
                    }),
            ),
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::Scry {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::Scry { top, bottom },
            ) => Some(
                self.validate_scry_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid scry chooser".into());
                        }
                        self.validate_library_partition(player, looked, top, bottom)
                    }),
            ),
            (
                Frame::EntityTargets { .. } | Frame::TriggerTargets { .. },
                Response::EntityTargets { entities },
            ) => Some(self.validate_entity_targets(player, entities)),
            (Frame::StackTargets { .. }, Response::StackTargets { entries }) => {
                Some(self.validate_stack_targets(player, entries))
            }
            (Frame::Priority, Response::ActivateAbility { object, ability })
                if self.state.rules.priority == player =>
            {
                Some(self.validate_activation(player, *object, *ability))
            }
            (
                Frame::PlayerTargets { .. } | Frame::TriggerTargets { .. },
                Response::PlayerTargets { players },
            ) => Some((|| {
                let top = self.top()?;
                let caster = top.controller;
                if player != caster
                    || players.len() != self.program()?.targets.len()
                    || players.iter().enumerate().any(|(slot, p)| {
                        !self.program().is_ok_and(|program| {
                            program.targets.get(slot).is_some_and(|spec| {
                                self.eligible_players(spec.domain, caster).contains(p)
                            })
                        })
                    })
                {
                    return Err("invalid player targets".into());
                }
                Ok(())
            })()),
            (
                Frame::ResolutionChoice {
                    choice:
                        ResolutionChoice::Sacrifice {
                            player: chooser, ..
                        },
                    ..
                },
                Response::Sacrifice { object },
            ) => Some(
                if *chooser == player
                    && self.is_live(player)
                    && self.controlled_creatures(player).contains(object)
                {
                    Ok(())
                } else {
                    Err("invalid sacrifice choice".into())
                },
            ),
            (
                Frame::ResolutionChoice {
                    choice:
                        ResolutionChoice::Discard {
                            player: chooser,
                            count,
                            ..
                        },
                    ..
                },
                Response::ResolveDiscard { objects },
            ) => Some((|| {
                if *chooser != player
                    || !self.is_live(player)
                    || !self.valid_discard_selection(player, *count, objects)
                {
                    return Err("invalid resolution discard choice".into());
                }
                Ok(())
            })()),
            _ => None,
        }
    }
    pub(super) fn actions_step(&mut self, frame: Frame) -> Result<(), String> {
        match frame {
            Frame::EntityTargets { cast } => self.bind_entity_targets(cast)?,
            Frame::StackTargets { cast } => self.bind_stack_targets(cast)?,
            Frame::PlayerTargets { cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing player targets")?;
                let Response::PlayerTargets { players } = input.response else {
                    return Err("player target response invariant".into());
                };
                self.rules_mut()
                    .stack
                    .last_mut()
                    .ok_or("missing player-target stack entry")?
                    .player_targets
                    .extend(players);
                if cast.origin == AnnouncementOrigin::Activated {
                    self.continue_activation_after_targets(cast, input.decision_id)?;
                } else {
                    self.rules_mut().frame = self.continue_spell_targeting(cast)?;
                }
            }
            Frame::ResolutionChoice { mut frame, choice } => {
                if let ResolutionChoice::SetColor { source_block } = choice {
                    return self.target_color_step(frame, source_block);
                }
                if let ResolutionChoice::Proliferate {
                    source_block,
                    choices,
                } = choice
                {
                    return self.proliferate_step(frame, source_block, choices);
                }
                if let ResolutionChoice::Demonstrate {
                    first,
                    source_block,
                } = choice
                {
                    return self.demonstrate_choice_step(frame, first, source_block);
                }
                if let ResolutionChoice::HandReturn {
                    player,
                    source_block,
                    ..
                } = choice
                {
                    return self.hand_return_step(frame, player, source_block);
                }
                if let ResolutionChoice::LibraryOrder {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.library_order_step(frame, player, looked, source_block);
                }
                if let ResolutionChoice::OptionalShuffle {
                    player,
                    source_block,
                } = choice
                {
                    return self.optional_shuffle_step(frame, player, source_block);
                }

                if let ResolutionChoice::LookTopMana {
                    player,
                    looked,
                    revealed,
                } = choice
                {
                    return self.look_top_mana_step(frame, player, looked, revealed);
                }
                if let ResolutionChoice::Hideaway {
                    player,
                    looked,
                    bottom,
                } = choice
                {
                    return self.hideaway_step(frame, player, looked, bottom);
                }
                if let ResolutionChoice::LibrarySelect {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.library_selection_step(frame, player, looked, source_block);
                }
                if let ResolutionChoice::Surveil {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.surveil_choice_step(frame, player, looked, source_block);
                }
                if let ResolutionChoice::Scry {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.scry_choice_step(frame, player, looked, source_block);
                }
                if let ResolutionChoice::ChooseOptional {
                    player,
                    source_block,
                } = choice
                {
                    return self.choose_optional_block(frame, player, source_block);
                }
                if let ResolutionChoice::OptionalEffect {
                    player,
                    target,
                    source_block,
                } = choice
                {
                    return self.removal_choice_step(frame, player, target, source_block);
                }
                if let ResolutionChoice::ColoredSearch {
                    player,
                    source_block,
                    revealed,
                } = choice
                {
                    return self.colored_step(frame, player, source_block, revealed);
                }
                if matches!(
                    choice,
                    ResolutionChoice::OptionalEffect { .. }
                        | ResolutionChoice::Search { .. }
                        | ResolutionChoice::ColoredSearch { .. }
                        | ResolutionChoice::SearchSplit { .. }
                        | ResolutionChoice::Shuffle { .. }
                        | ResolutionChoice::OptionalRecursion { .. }
                        | ResolutionChoice::OptionalDraw { .. }
                ) {
                    return self.staples_choice_step(frame, choice);
                }
                let Some(request) = self.actions_request()? else {
                    let outcome = match choice {
                        ResolutionChoice::SetColor { .. }
                        | ResolutionChoice::Proliferate { .. }
                        | ResolutionChoice::Demonstrate { .. }
                        | ResolutionChoice::HandReturn { .. }
                        | ResolutionChoice::LibraryOrder { .. }
                        | ResolutionChoice::OptionalShuffle { .. }
                        | ResolutionChoice::LookTopMana { .. }
                        | ResolutionChoice::Hideaway { .. }
                        | ResolutionChoice::LibrarySelect { .. }
                        | ResolutionChoice::Surveil { .. }
                        | ResolutionChoice::Scry { .. }
                        | ResolutionChoice::OptionalEffect { .. }
                        | ResolutionChoice::Search { .. }
                        | ResolutionChoice::ColoredSearch { .. }
                        | ResolutionChoice::SearchSplit { .. }
                        | ResolutionChoice::Shuffle { .. }
                        | ResolutionChoice::OptionalRecursion { .. }
                        | ResolutionChoice::OptionalDraw { .. }
                        | ResolutionChoice::ChooseOptional { .. } => unreachable!(),
                        ResolutionChoice::Sacrifice { player, .. } => {
                            InstructionOutcome::Sacrificed {
                                player,
                                movement: None,
                            }
                        }
                        ResolutionChoice::Discard { player, .. } => InstructionOutcome::Discarded {
                            player,
                            movements: vec![],
                        },
                    };
                    self.rules_mut().pending = None;
                    self.finish_instruction(frame, outcome)?;
                    return Ok(());
                };
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing resolution choice")?;
                self.validate_choice_input(&choice, input.player, &input.response)?;
                let source_block = match choice {
                    ResolutionChoice::SetColor { .. }
                    | ResolutionChoice::Proliferate { .. }
                    | ResolutionChoice::Demonstrate { .. }
                    | ResolutionChoice::HandReturn { .. }
                    | ResolutionChoice::LibraryOrder { .. }
                    | ResolutionChoice::OptionalShuffle { .. }
                    | ResolutionChoice::LookTopMana { .. }
                    | ResolutionChoice::Hideaway { .. }
                    | ResolutionChoice::LibrarySelect { .. }
                    | ResolutionChoice::Surveil { .. }
                    | ResolutionChoice::Scry { .. }
                    | ResolutionChoice::OptionalEffect { .. }
                    | ResolutionChoice::Search { .. }
                    | ResolutionChoice::ColoredSearch { .. }
                    | ResolutionChoice::SearchSplit { .. }
                    | ResolutionChoice::Shuffle { .. }
                    | ResolutionChoice::OptionalRecursion { .. }
                    | ResolutionChoice::OptionalDraw { .. }
                    | ResolutionChoice::ChooseOptional { .. } => unreachable!(),
                    ResolutionChoice::Sacrifice { source_block, .. }
                    | ResolutionChoice::Discard { source_block, .. } => source_block,
                };
                let cause = BatchCause::Instruction {
                    stack_id: frame.stack_id,
                    player: self.entry(frame.stack_id)?.controller,
                    source: self.entry(frame.stack_id)?.source,
                    cursor: frame.cursor,
                    source_block,
                };
                match input.response {
                    Response::Sacrifice { object } => {
                        return self.complete_graveyard_group(
                            cause,
                            EventGroup::Occurrence {
                                proposed: ProposedEvent::Sacrifice {
                                    object,
                                    player: request.player(),
                                },
                            },
                            GroupFinish::Instruction {
                                frame,
                                result: GroupResult::Sacrifice {
                                    player: request.player(),
                                },
                            },
                        );
                    }
                    Response::ResolveDiscard { objects } => {
                        if let Some(group) = &mut frame.group_discard {
                            group.select(request.player(), objects)?;
                            self.rules_mut().frame = Frame::Resolve { frame };
                            return Ok(());
                        }
                        return self.complete_resolving_discards(
                            frame,
                            cause,
                            request.player(),
                            objects,
                        );
                    }
                    _ => return Err("resolution response invariant".into()),
                };
            }
            _ => return Err("action frame invariant".into()),
        }
        Ok(())
    }
    pub(super) fn valid_discard_selection(
        &self,
        player: PlayerId,
        count: usize,
        objects: &[ObjectId],
    ) -> bool {
        let hand = self.action_hand(player);
        objects.len() == count.min(hand.len())
            && objects.iter().collect::<BTreeSet<_>>().len() == objects.len()
            && objects.iter().all(|o| hand.contains(o))
    }
    pub(super) fn complete_resolving_discards(
        &mut self,
        frame: Resolving,
        cause: BatchCause,
        player: PlayerId,
        objects: Vec<ObjectId>,
    ) -> Result<(), String> {
        if objects.is_empty() {
            return self.finish_instruction(
                frame,
                InstructionOutcome::Discarded {
                    player,
                    movements: vec![],
                },
            );
        }
        let occurrences = objects
            .into_iter()
            .map(|object| ProposedEvent::DiscardCard { object, player })
            .collect();
        self.complete_graveyard_group(
            cause,
            EventGroup::Simultaneous { occurrences },
            GroupFinish::Instruction {
                frame,
                result: GroupResult::Discard { player },
            },
        )
    }
    fn validate_choice_input(
        &self,
        choice: &ResolutionChoice,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        let Frame::ResolutionChoice {
            choice: current, ..
        } = &self.state.rules.frame
        else {
            return Err("missing resolution choice frame".into());
        };
        if choice != current {
            return Err("resolution choice changed".into());
        }
        self.validate_actions_response(player, response)
            .ok_or("resolution choice response mismatch")?
    }
    pub(super) fn finish_instruction(
        &mut self,
        mut frame: Resolving,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        if frame.group_discard.is_some() {
            return Err("cannot advance an unfinished group discard".into());
        }
        if frame.random_discard.is_some() {
            return Err("cannot advance an unfinished random discard".into());
        }
        if frame.draw_progress.is_some() || frame.each_player_draw.is_some() {
            return Err("cannot advance an unfinished counted draw".into());
        }
        frame.entrant_draw = None;
        frame.outcomes.push(outcome);
        frame.cursor = frame
            .cursor
            .checked_add(1)
            .ok_or("instruction cursor resource limit")?;
        self.rules_mut().frame = Frame::Resolve { frame };
        Ok(())
    }
    pub(super) fn execute_action_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        match instruction {
            Instruction::Kernel { node, source_block } => {
                self.execute_kernel(frame, node, source_block)
            }
            Instruction::PlayHideawayLinked {
                hideaway_block,
                minimum_total_power,
                source_block,
            } => self.begin_linked_play(frame, hideaway_block, minimum_total_power, source_block),
            Instruction::LoseGameTargetPlayer { .. } => self.resolve_targeted_loss(frame),
            Instruction::TransformSource { source_block } => {
                let source = self.entry(frame.stack_id)?.source;
                // CR 701.27f: this supported printed ability can transform
                // its original incarnation once. A queued duplicate observes
                // the new face and does nothing.
                let actual = if self.may_transform_source(source) {
                    Some(self.commit(
                        ProposedEvent::Transform { object: source },
                        self.instruction_cause(&frame, source_block)?,
                    )?)
                } else {
                    None
                };
                self.finish_instruction(frame, InstructionOutcome::Effect { actual })
            }
            Instruction::RegisterControllerCreatureEntryThisTurn { .. }
            | Instruction::RegisterControllerSpellCastThisTurn { .. } => {
                self.register_delayed_chapter(frame)
            }
            Instruction::RevealAndCast { source_block } => {
                self.begin_library_shuffle(frame, source_block)
            }
            Instruction::OfferHandCastWithoutManaCost { .. } => {
                self.rules_mut().frame = Frame::HandCast {
                    frame: Box::new(frame),
                    child: None,
                };
                Ok(())
            }
            Instruction::Cascade { source_block, .. } => self.begin_cascade(frame, source_block),
            Instruction::PutHandOnLibrary {
                count,
                source_block,
            } => self.begin_hand_return(frame, count, source_block),
            Instruction::OrderLibraryTop {
                amount,
                source_block,
            } => self.begin_library_order(frame, amount, source_block),
            Instruction::ShufflePlayerGraveyard { source_block } => {
                self.start_graveyard_shuffle(frame, source_block)
            }
            Instruction::OptionalShuffleLibrary { source_block } => {
                self.begin_optional_shuffle(frame, source_block)
            }
            Instruction::LookTopMayRevealForMana { .. } => self.begin_look_top_mana(frame),
            Instruction::Hideaway { .. } => self.begin_hideaway(frame),
            Instruction::SelectLibraryCard {
                amount,
                source_block,
                ..
            } => self.begin_library_selection(frame, amount, source_block),
            Instruction::Scry {
                amount,
                source_block,
            } => self.begin_scry(frame, amount, source_block),
            Instruction::Surveil {
                amount,
                source_block,
            } => self.begin_surveil(frame, amount, source_block),
            Instruction::FreezeControllerLifeUntilNextTurn { source_block } => {
                let actual = self.commit(
                    ProposedEvent::FreezeLifeUntilNextTurn {
                        player: self.entry(frame.stack_id)?.controller,
                    },
                    self.instruction_cause(&frame, source_block)?,
                )?;
                self.finish_instruction(
                    frame,
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    },
                )
            }
            Instruction::PhaseOutTargetPermanent {
                target_slot,
                source_block,
            } => {
                let slot = target_slot as usize;
                let target = *self
                    .bound_object_target(self.entry(frame.stack_id)?, slot)
                    .ok_or("missing phasing target")?;
                // CR 608.2b, 702.26b: keep resolution-start legality, but a
                // previous mode may already have phased this incarnation out.
                let outcome = if frame.legal_targets.get(slot) != Some(&true) {
                    InstructionOutcome::IllegalTarget { target }
                } else if !self
                    .current(target)
                    .is_ok_and(|o| o.present_on_battlefield())
                {
                    InstructionOutcome::ObjectNoLongerPresent { target }
                } else {
                    let actual = self.commit(
                        ProposedEvent::PhaseOutTarget { object: target },
                        self.instruction_cause(&frame, source_block)?,
                    )?;
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    }
                };
                self.finish_instruction(frame, outcome)
            }
            Instruction::PhaseOutControllerPermanents { source_block } => {
                let actual = self.commit(
                    ProposedEvent::PhaseOutControlled {
                        controller: self.entry(frame.stack_id)?.controller,
                    },
                    self.instruction_cause(&frame, source_block)?,
                )?;
                self.finish_instruction(
                    frame,
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    },
                )
            }
            i @ (Instruction::RemoveOpponentNonlandIndestructible { .. }
            | Instruction::GrantKeywordsSet { .. }
            | Instruction::GainLifeDestroyed { .. }
            | Instruction::ExileDamagedThisTurn { .. }
            | Instruction::LoseLifeDeathController { .. }) => {
                self.execute_wipes_instruction(frame, i)
            }
            i @ Instruction::SacrificeDamage { .. } => self.execute_sacrifice_instruction(frame, i),
            i @ (Instruction::ReturnEntryTargetCreature { .. }
            | Instruction::ChangeLifeFromTarget { .. }
            | Instruction::CreateTokensFor { .. }
            | Instruction::DamageTargetController { .. }) => {
                self.execute_removal_instruction(frame, i)
            }
            i @ (Instruction::AttachEquipment { .. }
            | Instruction::ReturnAuraFromGraveyard { .. }) => {
                self.execute_attachment_instruction(frame, i)
            }
            op @ (Instruction::ReturnOwnGraveyardCreatureToHand { .. }
            | Instruction::ReturnOwnGraveyardCreatureToBattlefield { .. }
            | Instruction::ReturnOwnGraveyardCardToHand { .. }
            | Instruction::SearchRecipientBasicLand { .. }
            | Instruction::SearchBasicLands { .. }
            | Instruction::FetchBasicLand { .. }
            | Instruction::SearchBasicLandsSplit { .. }
            | Instruction::SearchColoredLibrary { .. }
            | Instruction::SearchTypedLands { .. }) => self.execute_staples_instruction(frame, op),
            Instruction::SetTargetColor { source_block, .. } => {
                self.rules_mut().frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::SetColor { source_block },
                };
                Ok(())
            }
            Instruction::Proliferate { source_block } => {
                self.rules_mut().frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Proliferate {
                        source_block,
                        choices: vec![],
                    },
                };
                Ok(())
            }
            Instruction::DistributePlusOneCounters { source_block, .. } => {
                let allocations = self.allocated_counter_targets(&frame)?;
                let cause = self.instruction_cause(&frame, source_block)?;
                self.begin_counter_placements(frame, allocations, cause)
            }
            Instruction::Demonstrate { source_block } => {
                self.rules_mut().frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Demonstrate {
                        source_block,
                        first: None,
                    },
                };
                Ok(())
            }
            Instruction::CopyControlledCreatureSpell {
                target_slot,
                source_block,
            } => self.copy_creature_spell(frame, target_slot, source_block),
            op @ Instruction::PutSourcePlusOneCountersForCastColors { .. } => {
                self.execute_token_instruction(frame, op)
            }
            op @ (Instruction::GrantKeywords { .. }
            | Instruction::RegenerateSource { .. }
            | Instruction::GrantCreatureRestriction { .. }
            | Instruction::ProtectControllerUntilNextTurn { .. }
            | Instruction::PreventCombatDamage { .. }
            | Instruction::GrantLandPlaysThisTurn { .. }
            | Instruction::GrantHandCastCascade { .. }
            | Instruction::PreventNextDamage { .. }) => self.execute_effect_instruction(frame, op),
            Instruction::ExileLibraryForPlay { .. } => self.execute_impulse_play(frame),
            Instruction::AddResolutionMana {
                color,
                quantity,
                source_block,
            } => self.produce_resolution_mana(frame, color, quantity, source_block),
            Instruction::DrawCardsIfAdditionalCost { cost_identity, .. } => {
                if !self
                    .additional_cost_was_selected(self.entry(frame.stack_id)?, &cost_identity)?
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                self.draw_multiple(frame)
            }
            Instruction::DrawDamageAmount { .. } => self.draw_multiple(frame),
            // CR 603.5/608.2d: offer the option only during resolution, even
            // when the library is empty. Accepting performs the ordinary draw.
            // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
            Instruction::OptionalDrawForSoleGreatestEntrant { source_block } => {
                let entrant = self.resolving_entrant(&frame)?;
                let power = entrant
                    .characteristics
                    .power
                    .as_ref()
                    .ok_or("entrant without power")?;
                for o in self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.present_on_battlefield() && o.id != entrant.object)
                {
                    if self.is_battlefield_creature(o.id)
                        && self
                            .characteristics(o.id)?
                            .power
                            .as_ref()
                            .is_some_and(|p| p >= power)
                    {
                        return self.finish_instruction(
                            frame,
                            InstructionOutcome::Effect { actual: None },
                        );
                    }
                }
                let player = entrant.controller;
                let mut frame = frame;
                frame.entrant_draw = Some(Box::new(entrant));
                self.offer_player_draw(frame, player, source_block)
            }
            Instruction::ChooseOptional {
                chooser,
                following,
                source_block,
            } => self.begin_optional_block(frame, chooser, following, source_block),
            Instruction::OptionalDrawCards { source_block } => {
                self.offer_controller_draw(frame, source_block)
            }
            Instruction::CounterStackObject {
                target_slot,
                source_block,
            } => self.resolve_counter_instruction(frame, target_slot, source_block, None),
            Instruction::CounterSpellUnlessPays { .. }
            | Instruction::OptionalDrawUnlessCasterPays { .. } => {
                self.begin_resolution_payment(frame)
            }
            Instruction::CounterSpellAndMillController {
                target_slot,
                count,
                source_block,
            } => self.resolve_counter_instruction(frame, target_slot, source_block, Some(count)),
            _ => Err("unexpected action instruction".into()),
        }
    }
    /// CR 701.17a, 701.9a-b, 701.21a: a kernel mill, discard or sacrifice,
    /// performed by the executors whose events, choices and outcomes
    /// recovery re-derives.
    pub(super) fn execute_player_action(
        &mut self,
        frame: Resolving,
        op: Instruction,
    ) -> Result<(), String> {
        match op {
            kernel!(Mill { player, amount } @ source_block) => {
                let recipient = match player {
                    you!() => None,
                    target_player!(target_slot) => Some(LifeRecipient::Target { target_slot }),
                    _ => return Err("kernel mill player has no audited executor".into()),
                };
                let count = match amount {
                    fixed!(value) => value,
                    announced_x!() => self
                        .entry(frame.stack_id)?
                        .x
                        .clone()
                        .ok_or("missing announced X")?,
                    _ => return Err("kernel mill amount has no audited executor".into()),
                };
                self.execute_mill(frame, recipient, count, source_block)
            }
            op @ kernel!(Sacrifice {
                player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                ..
            }) => self.execute_wipes_instruction(frame, op),
            op @ kernel!(Sacrifice {
                player: mtg_contracts::kir::PlayerExpr::EachOpponent,
                ..
            }) => self.execute_sacrifice_instruction(frame, op),
            kernel!(Sacrifice { player: event_player!(), .. } @ source_block) => {
                let actor = self.resolution_player(&frame, Some(&ActionRecipient::EventPlayer))?;
                if !self.is_live(actor) || self.controlled_creatures(actor).is_empty() {
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Sacrificed {
                            player: actor,
                            movement: None,
                        },
                    )
                } else {
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Sacrifice {
                            player: actor,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
            kernel!(Sacrifice { player: target_player!(target_slot), .. } @ source_block) => {
                let target = *self
                    .entry(frame.stack_id)?
                    .player_targets
                    .get(target_slot as usize)
                    .ok_or("missing player target binding")?;
                if !frame
                    .legal_targets
                    .get(target_slot as usize)
                    .copied()
                    .unwrap_or(false)
                    || !self.is_live(target)
                    || self.controlled_creatures(target).is_empty()
                {
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Sacrificed {
                            player: target,
                            movement: None,
                        },
                    )
                } else {
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Sacrifice {
                            player: target,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
            kernel!(Discard { player: player @ (mtg_contracts::kir::PlayerExpr::EachPlayer | mtg_contracts::kir::PlayerExpr::EachOpponent), amount: fixed!(count), random: false } @ source_block) =>
            {
                let scope = if player == mtg_contracts::kir::PlayerExpr::EachPlayer {
                    GroupDiscardScope::EachPlayer
                } else {
                    GroupDiscardScope::EachOpponent
                };
                self.group_discard_step(frame, scope, count, source_block)
            }
            kernel!(Discard {
                player: target_player!(_),
                random: true,
                ..
            }) => self.random_discard_step(frame),
            op @ (kernel!(Discard {
                player: you!() | event_player!(),
                amount: fixed!(_),
                random: false
            })
            | kernel!(Discard {
                player: target_player!(_),
                amount: fixed!(_),
                random: false
            })) => {
                let (player, count, source_block) = self.discard_spec(&frame, &op)?;
                if count.is_negative() {
                    return Err("negative discard count".into());
                }
                if !self.is_live(player) {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::PlayerActionSkipped { player },
                    );
                }
                let actual_count = self.bounded_hand_count(player, count);
                if actual_count == 0 {
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Discarded {
                            player,
                            movements: vec![],
                        },
                    )
                } else {
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Discard {
                            player,
                            count: actual_count,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
            _ => Err("kernel player action has no audited executor".into()),
        }
    }
    pub(super) fn resolving_entrant(&self, frame: &Resolving) -> Result<ObjectBefore, String> {
        let StackKind::Triggered {
            witness:
                TriggerEventWitness::CreatureEnteredWithPower {
                    entrant,
                    last_appearance: Some(last),
                    ..
                },
            ..
        } = &self.entry(frame.stack_id)?.kind
        else {
            return Err("missing resolving entrant".into());
        };
        if self
            .current(entrant.object)
            .is_ok_and(|o| o.present_on_battlefield())
        {
            self.object_before(entrant.object)
        } else {
            Ok((**last).clone())
        }
    }
    pub(super) fn offer_controller_draw(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let player = self.entry(frame.stack_id)?.controller;
        self.offer_player_draw(frame, player, source_block)
    }
    fn offer_player_draw(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        self.rules_mut().frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::OptionalDraw {
                player,
                source_block,
            },
        };
        Ok(())
    }

    pub(super) fn draw_one(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            return self
                .finish_draw_attempt(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if let Some(card) = self
            .state
            .rules
            .libraries
            .get(&player)
            .and_then(|cards| cards.first())
            .copied()
        {
            let object = self
                .object_id(card)
                .ok_or("missing instruction draw source")?;
            if self.state.rules.commanders.history(card).is_some() {
                self.rules_mut().frame = Frame::CommanderReturn {
                    object,
                    destination: Zone::Hand { owner: player },
                    continuation: CommanderContinuation::ResolutionDraw {
                        frame,
                        player,
                        source_block,
                    },
                };
                Ok(())
            } else {
                let movement = self.move_object_for(
                    object,
                    Zone::Hand { owner: player },
                    BatchCause::Instruction {
                        stack_id: frame.stack_id,
                        player,
                        source: self.entry(frame.stack_id)?.source,
                        cursor: frame.cursor,
                        source_block,
                    },
                )?;
                self.finish_draw_attempt(
                    frame,
                    InstructionOutcome::Drawn {
                        player,
                        movement: Some(movement),
                        failed: false,
                    },
                )
            }
        } else {
            self.commit(
                ProposedEvent::FailedDraw { player },
                BatchCause::Instruction {
                    stack_id: frame.stack_id,
                    player,
                    source: self.entry(frame.stack_id)?.source,
                    cursor: frame.cursor,
                    source_block,
                },
            )?;
            self.finish_draw_attempt(
                frame,
                InstructionOutcome::Drawn {
                    player,
                    movement: None,
                    failed: true,
                },
            )
        }
    }
    fn counted_draw_spec(
        &self,
        frame: &Resolving,
        instruction: &Instruction,
    ) -> Result<Option<(Quantity, u32)>, String> {
        let (amount, source_block) = match instruction {
            // CR 121.2: every kernel draw other than one card by one player
            // is a counted sequence of single draws.
            kernel!(Draw { player, amount } @ source_block) => {
                if mtg_contracts::kir::single_draw(player, amount) {
                    return Ok(None);
                }
                // CR 608.2b: an announced player draws only while that target
                // is legal.
                if let Some(slot) = player.target_slot()
                    && frame.legal_targets.get(slot as usize) != Some(&true)
                {
                    return Err("missing legal counted draw target".into());
                }
                let amount = self.kernel_amount(frame, amount)?;
                if amount.is_negative() {
                    return Err("negative counted draw amount".into());
                }
                (amount, *source_block)
            }
            Instruction::DrawDamageAmount { source_block } => {
                (self.damage_draw_amount(frame)?, *source_block)
            }
            Instruction::DrawCardsIfAdditionalCost {
                cost_identity,
                count,
                source_block,
            } => {
                if !self.additional_cost_was_selected(self.entry(frame.stack_id)?, cost_identity)? {
                    return Ok(None);
                }
                (count.clone(), *source_block)
            }
            _ => return Ok(None),
        };
        Ok(Some((amount, source_block)))
    }
    /// CR 120.4b/603.2c: "that many" is the positive final member of this
    /// invocation's actual damage witness, never printed power or life changes.
    pub(super) fn damage_draw_amount(&self, frame: &Resolving) -> Result<Quantity, String> {
        if let Some(Instruction::DrawCardsIfAdditionalCost {
            cost_identity,
            count,
            ..
        }) = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        {
            return if self
                .additional_cost_was_selected(self.entry(frame.stack_id)?, cost_identity)?
            {
                Ok(count.clone())
            } else {
                Err("unselected optional-cost draw progress".into())
            };
        }
        match &self.entry(frame.stack_id)?.kind {
            StackKind::Triggered {
                witness: TriggerEventWitness::DamageDealt { damage, .. },
                ..
            } if damage.amount > Quantity::zero() => Ok(damage.amount.clone()),
            _ => Err("damage-quantity draw requires a positive actual damage witness".into()),
        }
    }
    // CR 121.2/121.4/704.4: one attempt per rule unit, including repeated failed
    // attempts. No SBA, card-count clamp or machine-integer conversion interrupts
    // the exact instruction. Commander replacement consumes just its attempt.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn draw_multiple(&mut self, mut frame: Resolving) -> Result<(), String> {
        self.validate_draw_progress(&frame)?;
        let program = self.program_for(self.entry(frame.stack_id)?)?;
        let instruction = program
            .instructions
            .get(frame.cursor as usize)
            .ok_or("missing counted draw instruction")?;
        let (amount, source_block) = self
            .counted_draw_spec(&frame, instruction)?
            .ok_or("counted draw source invariant")?;
        // CR 121.2c: freeze the live APNAP order once, then perform all of
        // one player's draws before starting the next player's attempts.
        if matches!(
            instruction,
            kernel!(Draw {
                player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                ..
            })
        ) && frame.each_player_draw.is_none()
        {
            frame.each_player_draw = Some(EachPlayerDrawProgress {
                remaining: self.apnap_players(),
                completed: vec![],
            });
        }
        let player = self.draw_player_for_instruction(&frame, instruction)?;
        if amount == Quantity::zero() {
            return self.finish_player_draws(frame, player, Quantity::zero(), Quantity::zero());
        }
        if frame.draw_progress.is_none() {
            frame.draw_progress = Some(DrawProgress {
                cursor: frame.cursor,
                source_block,
                completed: Quantity::zero(),
                remaining: amount,
                failed: Quantity::zero(),
            });
        }
        self.draw_one(frame, player, source_block)
    }
    /// Finish one ordinary draw attempt. The same owned continuation is used
    /// after a Commander replacement, preserving progress without another draw.
    pub(super) fn finish_draw_attempt(
        &mut self,
        mut frame: Resolving,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        if frame.draw_progress.is_none() {
            let program = self.program_for(self.entry(frame.stack_id)?)?;
            let instruction = program
                .instructions
                .get(frame.cursor as usize)
                .ok_or("missing draw instruction")?;
            if self.counted_draw_spec(&frame, instruction)?.is_some() {
                return Err("missing counted draw progress".into());
            }
            return self.finish_instruction(frame, outcome);
        }
        self.validate_draw_progress(&frame)?;
        let player = self.resolution_draw_player(&frame)?;
        if matches!(outcome, InstructionOutcome::PlayerActionSkipped { player: actor } if actor == player && !self.is_live(player))
        {
            let progress = frame.draw_progress.take().unwrap();
            return self.finish_player_draws(frame, player, progress.completed, progress.failed);
        }
        let failed = match outcome {
            InstructionOutcome::Drawn {
                player: actor,
                movement,
                failed,
            } if actor == player
                && (if failed {
                    movement.is_none()
                } else {
                    movement.is_some()
                }) =>
            {
                failed
            }
            _ => return Err("invalid counted draw attempt outcome".into()),
        };
        let progress = frame.draw_progress.as_mut().unwrap();
        progress.completed.add(&Quantity::from(1));
        progress.remaining.subtract(&Quantity::from(1));
        if failed {
            progress.failed.add(&Quantity::from(1));
        }
        if progress.remaining == Quantity::zero() {
            let progress = frame.draw_progress.take().unwrap();
            self.finish_player_draws(frame, player, progress.completed, progress.failed)
        } else {
            self.rules_mut().frame = Frame::Resolve { frame };
            Ok(())
        }
    }
    /// Structural guards complement accepted-input replay: neither coherent
    /// counters nor a substituted witness can establish an unrecorded draw.
    fn validate_draw_progress(&self, frame: &Resolving) -> Result<(), String> {
        let program = self.program_for(self.entry(frame.stack_id)?)?;
        if let Some(scope) = &frame.each_player_draw {
            let Some(
                instruction @ kernel!(Draw {
                    player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                    ..
                }),
            ) = program.instructions.get(frame.cursor as usize)
            else {
                return Err("each-player draw progress on another instruction".into());
            };
            let (amount, _) = self
                .counted_draw_spec(frame, instruction)?
                .ok_or("missing each-player draw count")?;
            if scope.remaining.is_empty()
                || (scope.completed.is_empty() && frame.draw_progress.is_none())
            {
                return Err("each-player draw cursor invariant".into());
            }
            self.validate_each_player_draws(&scope.completed, &scope.remaining, &amount)?;
        }
        if let Some(progress) = &frame.draw_progress {
            let instruction = program
                .instructions
                .get(frame.cursor as usize)
                .ok_or("missing counted draw instruction")?;
            let (amount, source_block) = self
                .counted_draw_spec(frame, instruction)?
                .ok_or("counted draw progress on another instruction")?;
            self.draw_player_for_instruction(frame, instruction)?;
            let mut total = progress.completed.clone();
            total.add(&progress.remaining);
            if progress.cursor != frame.cursor
                || progress.source_block != source_block
                || progress.completed.is_negative()
                || progress.remaining <= Quantity::zero()
                || progress.failed.is_negative()
                || progress.failed > progress.completed
                || total != amount
            {
                return Err("counted draw progress invariant".into());
            }
        }
        for (instruction, outcome) in program.instructions.iter().zip(&frame.outcomes) {
            let spec = self.counted_draw_spec(frame, instruction)?;
            if matches!(
                instruction,
                kernel!(Draw {
                    player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                    ..
                })
            ) {
                let InstructionOutcome::EachPlayerDrawAttempts { players } = outcome else {
                    return Err("missing each-player draw outcome".into());
                };
                let (amount, _) = spec.ok_or("missing each-player draw count")?;
                self.validate_each_player_draws(players, &[], &amount)?;
                continue;
            }
            match (spec, outcome) {
                (
                    Some((amount, _)),
                    InstructionOutcome::DrawAttempts {
                        player,
                        attempted,
                        failed,
                    },
                ) => {
                    if *player != self.draw_player_for_instruction(frame, instruction)?
                        || attempted.is_negative()
                        || *attempted > amount
                        || failed.is_negative()
                        || failed > attempted
                        || (*attempted != amount && self.is_live(*player))
                    {
                        return Err("counted draw completed outcome invariant".into());
                    }
                }
                (Some(_), _)
                | (
                    None,
                    InstructionOutcome::DrawAttempts { .. }
                    | InstructionOutcome::EachPlayerDrawAttempts { .. },
                ) => {
                    return Err("counted draw outcome on another instruction".into());
                }
                _ => (),
            }
        }
        Ok(())
    }
    pub(super) fn finish_ability(&mut self, id: StackId, resolved: bool) -> Result<(), String> {
        let triggered = matches!(self.entry(id)?.kind, StackKind::Triggered { .. });
        self.remove_resolved_entry(id)?;
        if triggered {
            self.record_event(Event::TriggeredAbilityCompleted {
                stack_id: id,
                resolved,
            })?;
        } else {
            self.record_event(Event::AbilityCompleted { stack_id: id })?;
        }
        self.rules_mut().frame = Frame::Priority;
        self.rules_mut().priority = self
            .state
            .rules
            .active
            .or_else(|| self.next_live(self.state.rules.turn_owner))
            .ok_or("no live player after ability")?;
        Ok(self.stabilize()?)
    }
    pub(super) fn remove_resolved_entry(&mut self, id: StackId) -> Result<(), String> {
        if let Some(index) = self.resolving_entry_index(id)? {
            self.unstack(index)?;
        }
        Ok(())
    }
    /// The stack position of the resolving invocation `id`, with exactly its
    /// completed children above it; `None` once a departure detached it.
    pub(super) fn resolving_entry_index(&self, id: StackId) -> Result<Option<usize>, String> {
        if let Some(index) = self.state.rules.stack.iter().position(|s| s.id == id) {
            let children = match &self.state.rules.frame {
                Frame::Resolve { frame } if frame.stack_id == id => {
                    self.resolution_copy_children(frame)?
                }
                Frame::PlayHideawayLinked { progress } if progress.frame.stack_id == id => {
                    self.linked_play_children(progress)
                }
                _ => self.resolution_child(id).into_iter().collect(),
            };
            let suffix = &self.state.rules.stack[index + 1..];
            if !suffix.iter().map(|s| s.id).eq(children)
                || !suffix.iter().all(|s| self.stack_entry_complete(s))
            {
                return Err("exact resolving owner completion invariant".into());
            }
            return Ok(Some(index));
        } else if self.invocation_lives(self.entry(id)?) {
            return Err("missing live resolving owner".into());
        }
        Ok(None)
    }
    pub(super) fn invocation_lives(&self, entry: &StackInstance) -> bool {
        if entry.kind.is_spell() {
            self.state
                .rules
                .cards
                .get(&entry.source.card)
                .is_some_and(|card| self.is_live(card.owner) && self.is_live(entry.controller))
        } else {
            self.is_live(entry.controller)
        }
    }
    pub(crate) fn validate_resolving(&self, frame: &Resolving) -> Result<(), String> {
        self.validate_resolving_children(frame, &self.resolution_copy_children(frame)?)
    }
    pub(super) fn validate_resolving_owned(
        &self,
        frame: &Resolving,
        child: Option<StackId>,
    ) -> Result<(), String> {
        if frame.copied_child().is_some()
            || matches!(
                frame.outcomes.as_slice(),
                [InstructionOutcome::Demonstrated { .. }]
            )
        {
            return Err("copy outcome overlaps cast continuation".into());
        }
        self.validate_resolving_children(frame, child.as_slice())
    }
    pub(super) fn validate_resolving_children(
        &self,
        frame: &Resolving,
        children: &[StackId],
    ) -> Result<(), String> {
        self.validate_trigger_entry(self.entry(frame.stack_id)?)?;
        self.validate_cost_lock(self.entry(frame.stack_id)?)?;
        self.validate_additional_cost_entry(self.entry(frame.stack_id)?)?;
        self.validate_mode_binding(self.entry(frame.stack_id)?)?;
        self.validate_condition_cache(frame)?;
        self.validate_target_facts(frame)?;
        self.validate_set_damage_facts(frame)?;
        self.validate_draw_progress(frame)?;
        self.validate_random_discard_progress(frame)?;
        self.validate_group_discard_progress(frame)?;
        let entry = self.entry(frame.stack_id)?;
        let program = self.program_for(entry)?;
        if let Some(fact) = &frame.entrant_draw {
            let StackKind::Triggered {
                witness:
                    TriggerEventWitness::CreatureEnteredWithPower {
                        entrant,
                        last_appearance: Some(_),
                        ..
                    },
                ..
            } = &entry.kind
            else {
                return Err("detached entrant choice".into());
            };
            if !matches!(
                program.instructions.get(frame.cursor as usize),
                Some(Instruction::OptionalDrawForSoleGreatestEntrant { .. })
            ) || entrant.object != fact.object
                || entrant.owner != fact.owner
                || fact.zone != Zone::Battlefield
                || fact.controller != self.resolving_entrant(frame)?.controller
            {
                return Err("invalid resolving entrant identity".into());
            }
            self.validate_characteristic_identity(fact.object, fact.zone, &fact.characteristics)?;
        }
        self.validate_damage_bindings(entry)?;
        if entry.id != frame.stack_id
            || entry.id.0 >= self.state.rules.next_stack_id
            || !self.announced_target_count_valid(program, entry)
            || !self.stack_entry_complete(entry)
            // Immediate loss and completion of its activated ability share one
            // atomic rule step. There is no post-loss resolving continuation.
            || (matches!(program.instructions.first(), Some(Instruction::LoseGameTargetPlayer { .. })) && frame.cursor != 0)
            || frame.cursor as usize > program.instructions.len()
            || frame.outcomes.len() != frame.cursor as usize
            || frame.legal_targets.len() != self.bound_target_count(entry)
        {
            return Err("snapshot resolving invocation invariant".into());
        }
        if self
            .state
            .rules
            .stack
            .iter()
            .any(|item| item.id == entry.id)
        {
            let position = self
                .state
                .rules
                .stack
                .iter()
                .position(|s| s.id == entry.id)
                .ok_or("missing resolving entry")?;
            if self.state.rules.detached.contains_key(&entry.id)
                || !self.state.rules.stack[position + 1..]
                    .iter()
                    .map(|s| s.id)
                    .eq(children.iter().copied())
                || children.first().is_some_and(|child| *child <= entry.id)
            {
                return Err("snapshot resolving owner/child binding".into());
            }
        } else if self.invocation_lives(entry) {
            return Err("snapshot detached invocation invariant".into());
        }
        Ok(())
    }
    pub(super) fn revalidate_actions_departure(&mut self) -> Result<(), String> {
        if let Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::OptionalEffect { target, .. },
        } = &self.state.rules.frame
            && self.current(*target).is_err()
            && matches!(
                self.program_for(self.entry(frame.stack_id)?)?
                    .instructions
                    .get(frame.cursor as usize),
                Some(Instruction::ReturnEntryTargetCreature { .. })
            )
        {
            let outcome = InstructionOutcome::ObjectNoLongerPresent { target: *target };
            let frame = frame.clone();
            self.rules_mut().pending = None;
            return self.finish_instruction(frame, outcome);
        }
        if matches!(
            self.state.rules.frame,
            Frame::ResolutionChoice { .. }
                | Frame::ForcedSacrifice { .. }
                | Frame::OptionalDamage { .. }
                | Frame::SacrificeCost { .. }
        ) && self.state.rules.pending.as_ref().is_some_and(|input| {
            self.validate_response(input.player, &input.response)
                .is_err()
        }) {
            self.rules_mut().pending = None;
        }
        Ok(())
    }
    pub(crate) fn validate_actions_state(&self) -> Result<(), String> {
        for entry in &self.state.rules.stack {
            let StackKind::Activated {
                ability,
                source_handles,
            } = &entry.kind
            else {
                continue;
            };
            self.validate_activated_entry(entry, *ability, source_handles)?;
        }
        if let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame {
            let instruction = self
                .program_for(self.entry(frame.stack_id)?)?
                .instructions
                .get(frame.cursor as usize)
                .ok_or("snapshot missing choice instruction")?;
            let valid = match (choice, instruction) {
                (
                    ResolutionChoice::SetColor { source_block },
                    Instruction::SetTargetColor {
                        source_block: expected,
                        ..
                    },
                ) => {
                    source_block == expected
                        && self.entry(frame.stack_id)?.kind.activated_index().is_some()
                }
                (
                    ResolutionChoice::Proliferate {
                        source_block,
                        choices,
                    },
                    Instruction::Proliferate {
                        source_block: expected,
                    },
                ) => {
                    source_block == expected
                        && choices.len() <= self.state.rules.cards.len()
                        && choices.windows(2).all(|p| p[0].0 < p[1].0)
                }
                (
                    ResolutionChoice::ChooseOptional {
                        player,
                        source_block,
                    },
                    Instruction::ChooseOptional {
                        chooser,
                        source_block: expected,
                        ..
                    },
                ) => {
                    source_block == expected
                        && self.optional_recipient(frame, chooser)? == Some(*player)
                }
                (
                    ResolutionChoice::Demonstrate { source_block, .. },
                    Instruction::Demonstrate {
                        source_block: expected,
                    },
                ) => expected == source_block && self.demonstrate_cast(frame).is_ok(),
                (
                    ResolutionChoice::HandReturn {
                        player,
                        hand,
                        source_block,
                    },
                    Instruction::PutHandOnLibrary { .. },
                ) => {
                    self.validate_hand_return_source(frame, *player, *source_block)
                        .is_ok()
                        && (!self.is_live(*player) || *hand == self.action_hand(*player))
                }
                (
                    ResolutionChoice::LibraryOrder {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::OrderLibraryTop { .. },
                ) => self
                    .validate_library_order_choice(frame, *player, looked, *source_block)
                    .is_ok(),
                (
                    ResolutionChoice::OptionalShuffle {
                        player,
                        source_block,
                    },
                    Instruction::OptionalShuffleLibrary { source_block: b },
                ) => *player == self.entry(frame.stack_id)?.controller && source_block == b,

                (
                    ResolutionChoice::LookTopMana {
                        player,
                        looked,
                        revealed,
                    },
                    Instruction::LookTopMayRevealForMana { .. },
                ) => self
                    .validate_look_top_mana_choice(frame, *player, looked, *revealed)
                    .is_ok(),
                (
                    ResolutionChoice::Hideaway {
                        player,
                        looked,
                        bottom,
                    },
                    Instruction::Hideaway { .. },
                ) => self
                    .validate_hideaway_choice(frame, *player, looked, bottom.as_ref())
                    .is_ok(),
                (
                    ResolutionChoice::LibrarySelect {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::SelectLibraryCard { .. },
                ) => self
                    .validate_selection_choice(frame, *player, looked, *source_block)
                    .is_ok(),
                (
                    ResolutionChoice::Surveil {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::Surveil { .. },
                ) => self
                    .validate_surveil_choice(frame, *player, looked, *source_block)
                    .is_ok(),
                (
                    ResolutionChoice::Scry {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::Scry { .. },
                ) => self
                    .validate_scry_choice(frame, *player, looked, *source_block)
                    .is_ok(),
                (
                    ResolutionChoice::Sacrifice {
                        player,
                        source_block,
                    },
                    kernel!(Sacrifice { player: event_player!(), .. } @ expected),
                ) => {
                    source_block == expected
                        && self
                            .resolution_player(frame, Some(&ActionRecipient::EventPlayer))
                            .is_ok_and(|actor| actor == *player)
                }
                (
                    ResolutionChoice::Sacrifice {
                        player,
                        source_block,
                    },
                    kernel!(Sacrifice { player: target_player!(target_slot), .. } @ expected),
                ) => {
                    *source_block == *expected
                        && self
                            .entry(frame.stack_id)?
                            .player_targets
                            .get(*target_slot as usize)
                            == Some(player)
                }
                (
                    ResolutionChoice::Discard {
                        player,
                        count,
                        source_block,
                    },
                    op,
                ) => self
                    .discard_spec(frame, op)
                    .is_ok_and(|(actor, expected, block)| {
                        actor == *player
                            && *count > 0
                            && (!self.is_live(*player)
                                || *count == self.bounded_hand_count(*player, expected))
                            && Quantity::from(*count as i64) <= *expected
                            && *source_block == block
                    }),
                _ => self.valid_staples_choice(frame, choice, instruction),
            };
            if !valid {
                return Err("snapshot resolution choice continuation invariant".into());
            }
        }
        Ok(())
    }
}

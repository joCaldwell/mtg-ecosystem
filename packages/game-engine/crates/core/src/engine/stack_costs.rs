use super::*;
use crate::query::{self, ObjectFacts, Tri};
use mtg_contracts::kir::{ActivationCondition, Cond as KirCond};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnouncementOrigin {
    #[default]
    Spell,
    Activated,
}
impl AnnouncementOrigin {
    pub(super) fn is_spell(&self) -> bool {
        matches!(self, Self::Spell)
    }
}

/// CR 602.5b-c, 400.7: a turn limit follows the ability slot of this
/// incarnation through control changes. A zone change creates a new object;
/// a transformed permanent's other face has its own abilities (CR 712.18).
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationUse {
    pub source: ObjectId,
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub face: CardFace,
    pub ability: u32,
    /// CR 602.5b: the zero-based use this turn of a limited ability.
    #[serde(default, skip_serializing_if = "is_zero_use")]
    pub nth: u32,
}
fn is_zero_use(n: &u32) -> bool {
    *n == 0
}

/// CR 602.1b, 602.5, 605.3a: the counts an "Activate only if" condition
/// read as the activation began, one per
/// [`ActivationCondition::leaves`] entry in printed order. The
/// activation keeps them as evidence; the condition is never read again.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationConditionCheck {
    pub counts: Vec<u32>,
}

impl RulesTxn {
    /// The one eligibility measurement shared by "is this activation
    /// offered" and "may this activation begin" for stack and mana
    /// abilities. An unaudited condition never holds.
    pub(super) fn activation_condition_check(
        &self,
        player: PlayerId,
        source: ObjectId,
        condition: &KirCond,
    ) -> Result<ActivationConditionCheck, String> {
        let form = ActivationCondition::of(condition).map_err(|e| e.to_string())?;
        let counts = form
            .leaves()
            .into_iter()
            .map(|leaf| self.activation_leaf_count(player, source, leaf))
            .collect::<Result<Vec<_>, _>>()?;
        if !form.passes(&counts) {
            return Err("activation condition is not met".into());
        }
        Ok(ActivationConditionCheck { counts })
    }

    fn activation_leaf_count(
        &self,
        player: PlayerId,
        source: ObjectId,
        leaf: &ActivationCondition,
    ) -> Result<u32, String> {
        use ActivationCondition as C;
        let objects = self.state.rules.objects.values();
        fn objects_iter(txn: &RulesTxn) -> impl Iterator<Item = &Object> {
            txn.state.rules.objects.values()
        }
        fn saturate(value: &Quantity) -> u32 {
            value
                .to_u32()
                .unwrap_or(if value.is_negative() { 0 } else { u32::MAX })
        }
        let count = match leaf {
            // CR 109.5, 613: phased-in permanents with their derived facts.
            C::Controlled { filter, .. } => objects
                .filter(|o| {
                    query::matches_battlefield_pred(&self.object_facts(o), player, filter)
                        == Tri::Yes
                })
                .count(),
            // CR 404.1, 111.8: cards the player owns in their graveyard.
            C::GraveyardCards { filter, .. } => objects
                .filter(|o| {
                    let facts = self.object_facts(o);
                    o.zone == (Zone::Graveyard { owner: player })
                        && !facts.is_token()
                        && query::matches_pred(&facts, player, filter) == Tri::Yes
                })
                .count(),
            C::HandCards { .. } => objects
                .filter(|o| o.zone == (Zone::Hand { owner: player }))
                .count(),
            // CR 506.4, 208.1: the source's own current facts.
            C::Source { filter } => usize::from(
                query::matches_pred(&self.object_facts(self.current(source)?), player, filter)
                    == Tri::Yes,
            ),
            // CR 500: the activator's turn facts. A clamped amount compares
            // exactly: `ActivationCondition::of` admits only thresholds
            // within `u32` and "at least" for the clamped measures.
            C::TurnHistory { fact, .. } => {
                let observed = triggers::TurnCastHistory::measure(
                    &self.state.rules.cast_history,
                    player,
                    *fact,
                )?;
                return Ok(saturate(&observed));
            }
            C::OpponentControlsMore { kind } => {
                let lands = |p: PlayerId| {
                    objects_iter(self)
                        .filter(|o| {
                            o.present_on_battlefield()
                                && o.controller == p
                                && self
                                    .object_facts(o)
                                    .derived()
                                    .is_some_and(|c| c.has_type(*kind))
                        })
                        .count()
                };
                let own = lands(player);
                usize::from(self.opponents(player).into_iter().any(|p| lands(p) > own))
            }
            // CR 102.2-102.3: one opponent's own permanents.
            C::OpponentControls { filter, .. } => self
                .opponents(player)
                .into_iter()
                .map(|p| {
                    objects_iter(self)
                        .filter(|o| {
                            let facts = self.object_facts(o);
                            facts.controller() == p
                                && query::matches_battlefield_pred(&facts, player, filter)
                                    == Tri::Yes
                        })
                        .count()
                })
                .max()
                .unwrap_or(0),
            // CR 201.2, 707.2: copiable names of the matching permanents.
            // CR 201.2a: objects with no name (face-down, CR 708.2a) share
            // no name with anything.
            C::SharedName { filter, .. } => {
                let mut names: BTreeMap<String, usize> = BTreeMap::new();
                for o in objects_iter(self) {
                    if query::matches_battlefield_pred(&self.object_facts(o), player, filter)
                        == Tri::Yes
                    {
                        let name = &self.copiable_definition(o.id)?.name;
                        if !name.is_empty() {
                            *names.entry(name.clone()).or_default() += 1;
                        }
                    }
                }
                names.into_values().max().unwrap_or(0)
            }
            // CR 205.2a, 404: every card type of the player's graveyard cards.
            C::GraveyardCardTypes { .. } => {
                let mut types = [false; CardKind::CARD_TYPES.len()];
                for o in objects_iter(self) {
                    let facts = self.object_facts(o);
                    if o.zone != (Zone::Graveyard { owner: player }) || facts.is_token() {
                        continue;
                    }
                    let c = facts.derived().ok_or("graveyard card characteristics")?;
                    for kind in std::iter::once(c.kind).chain(c.additional_types.iter().copied()) {
                        types[kind.card_type_index()] = true;
                    }
                }
                types.iter().filter(|t| **t).count()
            }
            // CR 208.1, 107.1b: negative powers count as themselves.
            C::TotalPower { filter, .. } => {
                let mut total = Quantity::zero();
                for o in objects_iter(self) {
                    let facts = self.object_facts(o);
                    if query::matches_battlefield_pred(&facts, player, filter) == Tri::Yes {
                        let power = facts.derived().and_then(|c| c.power.clone());
                        total.add(&power.unwrap_or_else(Quantity::zero));
                    }
                }
                return Ok(saturate(&total.max(Quantity::zero())));
            }
            // CR 400.7: this incarnation's own battlefield entry.
            C::SourceEnteredThisTurn => {
                let current = self.current(source)?;
                usize::from(
                    current.present_on_battlefield()
                        && current.battlefield_entry_turn == Some(self.state.rules.turn.number),
                )
            }
            // CR 103.4: the life total against the configured starting one.
            C::LifeAboveStarting { .. } => {
                let mut above = self.state.rules.players[player.0 as usize].life.clone();
                above.subtract(&self.prepared.config.initial_life(player));
                return Ok(saturate(&above.max(Quantity::zero())));
            }
            C::All(_) | C::Any(_) => return Err("activation condition leaf invariant".into()),
        };
        u32::try_from(count).map_err(|_| "activation condition count limit".into())
    }

    /// Snapshot validation: an activation of an ability with a condition
    /// carries the counts it read, and those counts satisfy the condition
    /// within what the game could contain. Any other ability carries none.
    pub(super) fn validate_activation_condition_check(
        &self,
        player: PlayerId,
        condition: Option<&KirCond>,
        check: Option<&ActivationConditionCheck>,
    ) -> Result<(), String> {
        let invalid = || "snapshot activation condition evidence invariant".to_string();
        let (condition, check) = match (condition, check) {
            (None, None) => return Ok(()),
            (Some(condition), Some(check)) => (condition, check),
            _ => return Err(invalid()),
        };
        let form = ActivationCondition::of(condition).map_err(|_| invalid())?;
        let leaves = form.leaves();
        let owned = self
            .state
            .rules
            .cards
            .values()
            .filter(|c| c.owner == player)
            .count();
        let objects = self.state.rules.objects.len();
        let bounded = leaves.len() == check.counts.len()
            && leaves.iter().zip(&check.counts).all(|(leaf, &count)| {
                use ActivationCondition as C;
                let bound = match leaf {
                    C::Controlled { .. } | C::OpponentControls { .. } | C::SharedName { .. } => {
                        objects
                    }
                    C::Source { .. }
                    | C::OpponentControlsMore { .. }
                    | C::SourceEnteredThisTurn => 1,
                    C::GraveyardCardTypes { .. } => CardKind::CARD_TYPES.len(),
                    // CR 500.2: an activation still on the stack began this
                    // turn, and each turn fact only grows during a turn.
                    C::TurnHistory { fact, .. } => {
                        match triggers::TurnCastHistory::measure(
                            &self.state.rules.cast_history,
                            player,
                            *fact,
                        ) {
                            Ok(now) => now.to_u32().map_or(usize::MAX, |n| n as usize),
                            Err(_) => return false,
                        }
                    }
                    // Life can since have fallen; any amount was possible.
                    C::LifeAboveStarting { .. } | C::TotalPower { .. } => usize::MAX,
                    C::GraveyardCards { .. } | C::HandCards { .. } | C::All(_) | C::Any(_) => owned,
                };
                count as usize <= bound
            });
        if bounded && form.passes(&check.counts) {
            Ok(())
        } else {
            Err(invalid())
        }
    }

    /// The condition of printed stack ability `ability` of `source`'s
    /// `face` (CR 113.7a: the face it was activated from).
    pub(super) fn activated_condition(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: u32,
    ) -> Result<Option<KirCond>, String> {
        Ok(self
            .face_definition(source.card, face)?
            .activated
            .get(ability as usize)
            .ok_or("missing activated ability")?
            .restriction
            .as_ref()
            .and_then(|r| r.condition.clone()))
    }

    pub(super) fn activation_restriction_allows(
        &self,
        player: PlayerId,
        source: ObjectId,
        ability: u32,
        restriction: &ActivationRestriction,
    ) -> bool {
        let own_turn = self.state.rules.active == Some(player);
        let step = self.state.rules.turn.step;
        let in_window = match restriction.window {
            None => true,
            Some(ActivationWindow::YourTurn) => own_turn,
            Some(ActivationWindow::YourUpkeep) => own_turn && step == TurnStep::Upkeep,
            Some(ActivationWindow::Combat) => step.phase() == Phase::Combat,
            Some(ActivationWindow::EndOfCombat) => step == TurnStep::EndCombat,
            Some(ActivationWindow::NotYourTurn) => self.state.rules.active.is_some() && !own_turn,
            Some(ActivationWindow::YourTurnBeforeAttackers) => {
                own_turn
                    && matches!(
                        step,
                        TurnStep::Upkeep
                            | TurnStep::Draw
                            | TurnStep::PrecombatMain
                            | TurnStep::BeginningCombat
                    )
            }
        };
        in_window
            && restriction
                .condition
                .as_ref()
                .is_none_or(|c| self.activation_condition_check(player, source, c).is_ok())
            && restriction.each_turn_limit().is_none_or(|limit| {
                let face = self.current(source).map_or(source.face, Object::face);
                self.activation_use_count(source, face, ability) < limit
            })
    }

    /// CR 602.5b: completed activations this turn of one ability of this
    /// incarnation's face.
    fn activation_use_count(&self, source: ObjectId, face: CardFace, ability: u32) -> u32 {
        let uses = self
            .state
            .rules
            .activation_uses
            .iter()
            .filter(|k| k.source == source && k.face == face && k.ability == ability)
            .count();
        u32::try_from(uses).unwrap_or(u32::MAX)
    }

    pub(super) fn validate_activation_uses(&self) -> Result<(), String> {
        for key in &self.state.rules.activation_uses {
            // Uses are numbered from zero without gaps, below the limit.
            let earlier = key.nth.checked_sub(1).is_none_or(|nth| {
                self.state
                    .rules
                    .activation_uses
                    .contains(&ActivationUse { nth, ..*key })
            });
            if self.current(key.source)?.zone != Zone::Battlefield
                || !earlier
                || self
                    .activation_limit(key.source, key.face, key.ability)?
                    .is_none_or(|limit| key.nth >= limit)
            {
                return Err("snapshot activation use invariant".into());
            }
        }
        Ok(())
    }

    fn activation_limit(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: u32,
    ) -> Result<Option<u32>, String> {
        Ok(self
            .face_definition(source.card, face)?
            .activated
            .get(ability as usize)
            .ok_or("missing limited activation slot")?
            .restriction
            .as_ref()
            .and_then(ActivationRestriction::each_turn_limit))
    }

    /// CR 601.2i, 602.2b, 602.5b: only a successfully completed activation
    /// uses the limit. A reversed announcement never used that ability.
    pub(super) fn record_ability_activated(
        &mut self,
        stack_id: StackId,
        source: ObjectId,
        player: PlayerId,
        ability: u32,
    ) -> Result<(), String> {
        // The activated face, fixed as the ability was put on the stack.
        let entry = self.entry(stack_id)?;
        let face = entry.source_face;
        let from_graveyard = matches!(entry.cast_from, Zone::Graveyard { .. });
        if let Some(limit) = self.activation_limit(source, face, ability)?
            && self.current(source).is_ok()
        {
            let nth = self.activation_use_count(source, face, ability);
            if nth >= limit
                || !self.rules_mut().activation_uses.insert(ActivationUse {
                    source,
                    face,
                    ability,
                    nth,
                })
            {
                return Err("activation limit already used".into());
            }
        }
        // CR 602.2b: count completed activations, even if a cost moved the
        // card out of the graveyard. The announcement pins the original zone.
        if from_graveyard {
            let count = &mut self.rules_mut().cast_history[player.0 as usize].graveyard_activations;
            *count = count.checked_add(1).ok_or("turn history count limit")?;
        }
        self.record_event(Event::AbilityActivated {
            stack_id,
            source,
            player,
            ability,
        })?;
        self.finish_target_triggers(stack_id)
    }

    /// CR 602.2a, 113.6j: a graveyard card's activated ability is
    /// activated by that card's owner, from that owner's own graveyard.
    pub(super) fn graveyard_activation_source_payable(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> bool {
        self.is_live(player)
            && self
                .current(source)
                .is_ok_and(|o| o.zone == (Zone::Graveyard { owner: player }))
            && self
                .state
                .rules
                .cards
                .get(&source.card)
                .is_some_and(|c| c.owner == player)
    }

    /// The off-battlefield source still sits where its cost moves it from.
    pub(super) fn zone_source_payable(
        &self,
        zone: FunctioningZone,
        player: PlayerId,
        source: ObjectId,
    ) -> bool {
        match zone {
            FunctioningZone::Hand => self.hand_activation_source_payable(player, source),
            FunctioningZone::Graveyard => self.graveyard_activation_source_payable(player, source),
            FunctioningZone::Battlefield | FunctioningZone::Stack | FunctioningZone::Any => false,
        }
    }

    // CR602.2/702.29a: a hand card's owner activates it. A stale battlefield
    // controller never supplies (or removes) permission outside battlefield.
    pub(super) fn hand_activation_source_payable(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> bool {
        self.is_live(player)
            && self
                .current(source)
                .is_ok_and(|o| o.zone == (Zone::Hand { owner: player }))
            && self
                .state
                .rules
                .cards
                .get(&source.card)
                .is_some_and(|c| c.owner == player)
    }
    // CR 118.3/119.4/602.2: payer is the current controller, and paying
    // exactly all remaining life is legal. Zero is payable at any life total.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(crate) fn validate_tap_life_sacrifice_source(
        &self,
        player: PlayerId,
        source: ObjectId,
        life: &Quantity,
    ) -> Result<(), String> {
        let object = self.current(source)?;
        if !object.present_on_battlefield()
            || object.controller != player
            || !self.is_live(player)
            || object.tapped
            || !self.state.rules.players[player.0 as usize].can_pay_life(life)
        {
            return Err("unpayable source tap, life and sacrifice cost".into());
        }
        Ok(())
    }
    fn pay_tap_life_sacrifice_source(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        life: &Quantity,
        cause: BatchCause,
    ) -> Result<(), String> {
        self.validate_tap_life_sacrifice_source(player, source, life)?;
        let mut children = vec![EventGroup::Occurrence {
            proposed: ProposedEvent::Tap { object: source },
        }];
        // Paying life is loss of life, not damage. Reuse the existing exact
        // life-change occurrence; zero costs deliberately emit no life event.
        if *life > Quantity::zero() {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::LoseLife {
                    player,
                    amount: life.clone(),
                },
            });
        }
        children.push(EventGroup::Occurrence {
            proposed: ProposedEvent::Sacrifice {
                object: source,
                player,
            },
        });
        // One ordered batch inside the staged rule step: no SBA/priority or
        // observable partial tap/debit before the complete cost has committed.
        self.commit_batch(cause, EventGroup::Ordered { children })?;
        Ok(())
    }
    /// CR 602.2b, 601.2c: the announcement stage for an activation's first
    /// target domain.
    pub(super) fn activation_target_frame(domain: TargetDomain, cast: Casting) -> Frame {
        match domain {
            TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer => Frame::PlayerTargets { cast },
            TargetDomain::StackSpell
            | TargetDomain::ControlledCreatureSpell
            | TargetDomain::StackActivatedOrTriggeredAbility => Frame::StackTargets { cast },
            TargetDomain::ControlledBattlefieldCreature
            | TargetDomain::ControlledBattlefieldPermanent
            | TargetDomain::BattlefieldCreature
            | TargetDomain::BattlefieldPermanent
            | TargetDomain::GraveyardCard { .. }
            | TargetDomain::OwnGraveyardCard
            | TargetDomain::OwnGraveyardCreatureCard => Frame::Targets { cast },
            _ => Frame::EntityTargets { cast },
        }
    }
    // CR 606.3–4: track the permanent incarnation, across its abilities
    // and control changes. This payment never opens a mana window.
    pub(super) fn loyalty_after_cost(
        &self,
        player: PlayerId,
        object: ObjectId,
        change: &Quantity,
    ) -> Result<Quantity, String> {
        let source = self.current(object)?;
        let mut remaining = source.loyalty_counters.clone();
        remaining.add(change);
        if !source.present_on_battlefield()
            || source.controller != player
            || source.loyalty_used_turn == Some(self.state.rules.turn.number)
            || remaining.is_negative()
        {
            return Err("unpayable loyalty activation cost".into());
        }
        Ok(remaining)
    }

    /// CR 606.3–4: pay a loyalty ability's cost. The balance change and the
    /// activation-use stamp are separate writes, so a `[0]` ability stamps
    /// the use and changes no counter.
    pub(super) fn pay_loyalty(
        &mut self,
        cause: BatchCause,
        change: &Quantity,
    ) -> Result<(), String> {
        let BatchCause::AbilityActivation { player, source, .. } = cause else {
            return Err("loyalty payment requires an ability activation".into());
        };
        let remaining = self.loyalty_after_cost(player, source, change)?;
        self.commit_counter_cost_batch(
            cause,
            EventGroup::Occurrence {
                proposed: ProposedEvent::PayCounterCost {
                    object: source,
                    cost: CounterCost::Loyalty {
                        change: change.clone(),
                    },
                },
            },
        )?;
        if self.current(source)?.loyalty_counters != remaining {
            return Err("loyalty cost balance invariant".into());
        }
        Ok(())
    }

    /// CR 606.4: put `change` loyalty counters on the source or remove
    /// exactly `-change`. Zero changes no balance.
    pub(super) fn pay_loyalty_counters(
        &mut self,
        object: ObjectId,
        change: &Quantity,
    ) -> Result<(), String> {
        let zero = Quantity::zero();
        if *change == zero {
            return Ok(());
        }
        let controller = self.current(object)?.controller;
        let kind = mtg_contracts::kir::CounterKind::Loyalty;
        // CR 118.11, 614.16: a cost's placement is not produced by an
        // effect; only any-origin quantity replacements see it.
        let placed = if *change > zero {
            let creature = self.is_battlefield_creature(object)?;
            Some(self.replace_quantity(
                replacement::QuantityMember::Counters {
                    holder: object,
                    controller,
                    creature,
                    counter: &kind,
                    shown: Some(object),
                },
                change,
                false,
            )?)
        } else {
            None
        };
        // Loyalty is a balance with no stack timestamp.
        let source = self
            .rules_mut()
            .objects
            .get_mut(&object.card)
            .ok_or("missing loyalty source")?;
        if let Some(placed) = placed {
            source.loyalty_counters.add(&placed.amount);
            self.record_quantity(placed);
        } else {
            let mut removed = zero;
            removed.subtract(change);
            source.pay_counters(&kind, &removed)?;
        }
        Ok(())
    }

    /// CR 606.3: the activation-use half of a loyalty cost, paid even when
    /// the cost changes no counter.
    pub(super) fn stamp_loyalty_use(&mut self, object: ObjectId) -> Result<(), String> {
        let turn = self.state.rules.turn.number;
        self.rules_mut()
            .objects
            .get_mut(&object.card)
            .ok_or("missing loyalty source")?
            .loyalty_used_turn = Some(turn);
        Ok(())
    }

    pub(crate) fn announce_ability(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        // CR602.2a/701.20a: reveal only after the ordinary priority authority
        // and exact source/ability permission have succeeded. This precedes
        // every opponent handle allocation; action queries allocate nothing.
        if !matches!(self.state.rules.frame, Frame::Priority) || self.state.rules.priority != player
        {
            return Err("ability proposal requires its ordinary priority owner".into());
        }
        self.validate_activation(player, object, ability)?;
        // CR 602.5: the condition is read once, as the activation begins.
        let condition = self
            .activated_condition(object, self.current(object)?.face(), ability)?
            .map(|c| self.activation_condition_check(player, object, &c))
            .transpose()?;
        let cost = self
            .copiable_definition(object)?
            .activated
            .get(ability as usize)
            .ok_or("missing activated cost")?
            .cost
            .clone();
        let cast_from = self.current(object)?.zone;
        let source_handles = self.capture_trigger_source_handles(object)?;
        let id = StackId(self.state.rules.next_stack_id);
        self.rules_mut().next_stack_id = self
            .state
            .rules
            .next_stack_id
            .checked_add(1)
            .ok_or("stack ID resource limit")?;
        let cost_lock = self.activation_cost_lock(
            object,
            &self
                .copiable_definition(object)?
                .activated
                .get(ability as usize)
                .ok_or("missing activated cost")?
                .cost_changes,
        )?;
        let total_cost = costs::locked_activation_total(cost.fixed_mana(), cost_lock.as_ref())?;
        let chosen = matches!(
            cost,
            ActivationCost::SacrificePermanent { .. } | ActivationCost::ChooseObjects(_)
        );
        let mana = matches!(
            cost,
            ActivationCost::Mana { .. }
                | ActivationCost::DiscardSource { .. }
                | ActivationCost::ExileSource { .. }
                | ActivationCost::SacrificePermanent { mana: Some(_), .. }
        ) || matches!(&cost, ActivationCost::Composite { costs } if costs.mana.is_some())
            || matches!(&cost, ActivationCost::ChooseObjects(cost) if cost.mana.is_some());
        let activated = &self.copiable_definition(object)?.activated[ability as usize];
        // CR 107.3a, 107.4e-f, 602.2b: hybrid symbols, then X, are announced
        // before targets and the mana window.
        let announces = activated.x_mana_cost || !activated.hybrid_mana.is_empty();
        // CR 602.2b, 700.2a: modes are chosen first, then their targets.
        let modal = activated.modal.is_some();
        let targeted = !activated.program.targets.is_empty() || modal;
        let entry = StackInstance {
            target_triggers: vec![],
            mode_mask: None,
            hybrid_colors: vec![],
            cost_lock,
            selected_additional_costs: None,

            x: None,
            death_controller: None,
            sacrificed: None,
            sacrificed_set: vec![],
            chosen_payment: vec![],
            effect_source_handles: vec![],
            source_facts: { Some(self.object_before(object)?) },
            entity_targets: vec![],
            announced_entity_targets: vec![],
            kind: StackKind::Activated {
                ability,
                source_handles,
                condition,
            },
            // CR 602.2a, 113.7a: the activated face, fixed as it is put on
            // the stack (CR 701.27f).
            source_face: self.current(object)?.face(),
            source_transforms: self.current(object)?.transforms,
            stack_targets: vec![],
            announced_stack_targets: vec![],
            player_targets: vec![],
            id,
            source: object,
            controller: player,
            targets: vec![],
            announced_targets: vec![vec![]; 4],
            paid: (!mana && !targeted && !chosen).then_some(total_cost.clone()),
            mana_uncounterable: false,
            total_cost,
            cast_from,
        };
        self.rules_mut().stack.push(entry);
        if announces {
            let cast = Casting {
                sacrifices: vec![],
                origin: AnnouncementOrigin::Activated,
                spending: None,
                delve: None,
                tap_payment: None,
                spending_restricted: vec![],
                stack_id: id,
                original_object: object,
                prior_passes: self.state.rules.passes.clone(),
                mana_activations: vec![],
                undo_mana: false,
                revealed_hand_consumed: false,
            };
            self.continue_announcement(cast)?;
        } else if targeted {
            if cost != ActivationCost::TapSource
                && !matches!(
                    cost,
                    ActivationCost::Mana { .. }
                        | ActivationCost::ExileSource { .. }
                        | ActivationCost::Composite { .. }
                        | ActivationCost::SacrificePermanent { .. }
                        | ActivationCost::ChooseObjects(_)
                        | ActivationCost::SacrificeSource
                        | ActivationCost::Loyalty { .. }
                )
            {
                return Err("unsupported targeted activation cost".into());
            }
            let cast = Casting {
                sacrifices: vec![],
                origin: AnnouncementOrigin::Activated,
                spending: None,
                delve: None,
                tap_payment: None,
                spending_restricted: vec![],
                stack_id: id,
                original_object: object,
                prior_passes: self.state.rules.passes.clone(),
                mana_activations: vec![],
                undo_mana: false,
                revealed_hand_consumed: false,
            };
            self.rules_mut().frame = if modal {
                Frame::ChooseModes { cast }
            } else {
                Self::activation_target_frame(
                    self.copiable_definition(object)?.activated[ability as usize]
                        .program
                        .targets[0]
                        .domain,
                    cast,
                )
            };
        } else if mana {
            self.rules_mut().frame = Frame::Mana {
                cast: Casting {
                    sacrifices: vec![],
                    origin: AnnouncementOrigin::Activated,
                    spending: None,
                    delve: None,
                    tap_payment: None,
                    spending_restricted: vec![],
                    stack_id: id,
                    original_object: object,
                    prior_passes: self.state.rules.passes.clone(),
                    mana_activations: vec![],
                    undo_mana: false,
                    revealed_hand_consumed: false,
                },
            };
        } else if chosen {
            // CR 602.2b/601.2g-h: no mana opportunity for a cost with no mana.
            let cast = Casting {
                sacrifices: vec![],
                origin: AnnouncementOrigin::Activated,
                spending: Some(BTreeMap::new()),
                delve: None,
                tap_payment: None,
                spending_restricted: vec![],
                stack_id: id,
                original_object: object,
                prior_passes: self.state.rules.passes.clone(),
                mana_activations: vec![],
                undo_mana: false,
                revealed_hand_consumed: false,
            };
            self.rules_mut().frame = if matches!(cost, ActivationCost::ChooseObjects(_)) {
                Self::chosen_cost_frame(cast)
            } else {
                Frame::SacrificeCost { cast }
            };
        } else {
            if let ActivationCost::TapPayLifeSacrificeSource { life } = &cost {
                self.pay_tap_life_sacrifice_source(
                    player,
                    object,
                    life,
                    BatchCause::AbilityActivation {
                        stack_id: id,
                        player,
                        source: object,
                        decision_id,
                        ability,
                    },
                )?;
            } else if let ActivationCost::Loyalty { change } = &cost {
                self.pay_loyalty(
                    BatchCause::AbilityActivation {
                        stack_id: id,
                        player,
                        source: object,
                        decision_id,
                        ability,
                    },
                    change,
                )?;
            } else if let ActivationCost::Composite { costs } = &cost {
                self.pay_permanent_source_costs(
                    object,
                    player,
                    costs,
                    BatchCause::AbilityActivation {
                        stack_id: id,
                        player,
                        source: object,
                        decision_id,
                        ability,
                    },
                )?;
            } else {
                let proposed = match cost {
                    ActivationCost::SacrificeSource => ProposedEvent::Sacrifice { object, player },
                    ActivationCost::TapSource => ProposedEvent::Tap { object },
                    ActivationCost::TapPayLifeSacrificeSource { .. }
                    | ActivationCost::Composite { .. }
                    | ActivationCost::Mana { .. }
                    | ActivationCost::DiscardSource { .. }
                    | ActivationCost::ExileSource { .. }
                    | ActivationCost::ChooseObjects(_)
                    | ActivationCost::Loyalty { .. }
                    | ActivationCost::SacrificePermanent { .. } => {
                        return Err("activated cost branch invariant".into());
                    }
                };
                self.commit(
                    proposed,
                    BatchCause::AbilityActivation {
                        stack_id: id,
                        player,
                        source: object,
                        decision_id,
                        ability,
                    },
                )?;
            }
            self.record_ability_activated(id, object, player, ability)?;
            self.rules_mut().priority = player;
            self.stabilize()?;
        }
        self.rules_mut().passes.clear();
        Ok(())
    }
    /// CR 107.3a, 107.4e-f, 601.2f, 602.2b: the next announcement stage of
    /// an activation whose cost has hybrid symbols or X, then its targets or
    /// mana window. The total cost always includes what is announced; with
    /// no mana left to pay (2 life for every Phyrexian symbol, X of 0) there
    /// is no mana window and the remaining costs are paid at once (CR
    /// 601.2g-h).
    fn activated_announcement(&mut self, cast: Casting) -> Result<(), String> {
        let entry = self.top()?;
        let StackKind::Activated { ability, .. } = entry.kind else {
            return Err("activation announcement binding invariant".into());
        };
        if entry.id != cast.stack_id || cast.origin != AnnouncementOrigin::Activated {
            return Err("activation announcement binding invariant".into());
        }
        let activated = self
            .stack_definition(entry)?
            .activated
            .get(ability as usize)
            .ok_or("missing announced activated ability")?
            .clone();
        if !entry.hybrid_colors.is_empty() {
            validate_hybrid_choice(&activated.hybrid_mana, &entry.hybrid_colors)?;
        }
        if entry.x.as_ref().is_some_and(Quantity::is_negative) {
            return Err("negative announced X".into());
        }
        let total = costs::locked_activation_total(
            activated.announced_cost(&entry.hybrid_colors, entry.x.as_ref()),
            entry.cost_lock.as_ref(),
        )?;
        self.rules_mut()
            .stack
            .last_mut()
            .ok_or("missing announced activation")?
            .total_cost = total;
        let entry = self.top()?;
        self.rules_mut().frame =
            if !activated.hybrid_mana.is_empty() && entry.hybrid_colors.is_empty() {
                Frame::HybridCost { cast }
            } else if activated.x_mana_cost && entry.x.is_none() {
                Frame::AnnounceX { cast }
            } else if let Some(target) = activated.program.targets.first() {
                Self::activation_target_frame(target.domain, cast)
            } else if self.announced_without_mana(entry)? {
                let mut cast = cast;
                cast.spending = Some(BTreeMap::new());
                // The announcement stage ends before the costs are paid.
                self.rules_mut().frame = Frame::Payment { cast: cast.clone() };
                return self.payment_step(cast);
            } else {
                Frame::Mana { cast }
            };
        Ok(())
    }
    /// CR 601.2g: an activation whose announcements leave no mana to pay
    /// has no mana window.
    pub(super) fn announced_without_mana(&self, entry: &StackInstance) -> Result<bool, String> {
        let StackKind::Activated { ability, .. } = entry.kind else {
            return Ok(false);
        };
        let activated = self
            .stack_definition(entry)?
            .activated
            .get(ability as usize)
            .ok_or("missing announced activated ability")?;
        Ok((activated.x_mana_cost || !activated.hybrid_mana.is_empty())
            && entry.total_cost == zero_mana_cost())
    }
    /// The spell or activation announcement stage after a hybrid or X
    /// choice.
    pub(super) fn continue_announcement(&mut self, cast: Casting) -> Result<(), String> {
        if cast.origin == AnnouncementOrigin::Activated {
            self.activated_announcement(cast)
        } else {
            self.rules_mut().frame = self.spell_announcement_frame(cast)?;
            Ok(())
        }
    }
    pub(super) fn validate_activated_entry(
        &self,
        entry: &StackInstance,
        ability: u32,
        source_handles: &[publish::StoredObjectHandle],
    ) -> Result<(), String> {
        let definition = self.stack_definition(entry)?;
        let activated = definition
            .activated
            .get(ability as usize)
            .ok_or("snapshot missing activated definition")?;
        // CR 606.3: payment consumes the turn's activation for this incarnation.
        if entry.paid.is_some()
            && matches!(activated.cost, ActivationCost::Loyalty { .. })
            && self.current(entry.source).is_ok_and(|source| {
                source.zone == Zone::Battlefield
                    && source.loyalty_used_turn != Some(self.state.rules.turn.number)
            })
        {
            return Err("snapshot paid loyalty activation lacks its turn marker".into());
        }
        let expected = costs::locked_activation_total(
            activated.announced_cost(&entry.hybrid_colors, entry.x.as_ref()),
            entry.cost_lock.as_ref(),
        )?;
        let program = self.program_for(entry)?;
        // CR 602.5b: a paid activation recorded its use; an unpaid one may
        // still begin, and a once-each-turn ability has no other use.
        if let Some(limit) = activated
            .restriction
            .as_ref()
            .and_then(ActivationRestriction::each_turn_limit)
            && self.current(entry.source).is_ok()
        {
            let uses = self.activation_use_count(entry.source, entry.source_face, ability);
            // Every paid activation of this ability still on the stack
            // recorded one use; resolved ones may account for the rest.
            let paid_waiting = self
                .state
                .rules
                .stack
                .iter()
                .filter(|e| {
                    e.paid.is_some()
                        && e.source == entry.source
                        && e.source_face == entry.source_face
                        && e.kind.activated_index() == Some(ability)
                })
                .count();
            let paid_waiting = u32::try_from(paid_waiting).unwrap_or(u32::MAX);
            let consistent = match (limit, entry.paid.is_some()) {
                (1, paid) => uses == u32::from(paid),
                (_, true) => uses >= paid_waiting,
                (_, false) => uses < limit,
            };
            if !consistent {
                return Err("snapshot activation payment/use invariant".into());
            }
        }
        let stage = self.underlying_mana_frame(&self.state.rules.frame)?;
        let announcing = match stage {
            Frame::HybridCost { cast } | Frame::AnnounceX { cast }
                if cast.stack_id == entry.id
                    && !activated.x_mana_cost
                    && activated.hybrid_mana.is_empty() =>
            {
                return Err("snapshot activation announcement invariant".into());
            }
            Frame::ChooseModes { cast }
            | Frame::HybridCost { cast }
            | Frame::AnnounceX { cast }
            | Frame::Targets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::Mana { cast }
            | Frame::SacrificeCost { cast }
            | Frame::ChosenObjectCost { cast, .. }
            | Frame::SpendMana { cast }
            | Frame::Payment { cast } => {
                cast.stack_id == entry.id && cast.origin == AnnouncementOrigin::Activated
            }
            _ => matches!(
                self.state.rules.frame,
                Frame::Fault { .. } | Frame::Finished { .. }
            ),
        };
        let player_target_domain = program.targets.first().is_some_and(|t| {
            matches!(
                t.domain,
                TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer
            )
        });
        if source_handles.len() != 4
            || (!entry.targets.is_empty()
                && program.targets.first().is_none_or(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::ControlledBattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::GraveyardCard { .. }
                            | TargetDomain::OwnGraveyardCard
                            | TargetDomain::OwnGraveyardCreatureCard
                    )
                }))
            || (!entry.player_targets.is_empty()
                && (!player_target_domain
                    || !matches!(entry.player_targets.as_slice(), [p] if p.0 < 4)))
            || (entry.paid.is_some() && player_target_domain && entry.player_targets.len() != 1)
            || (!entry.stack_targets.is_empty()
                && program.targets.first().is_none_or(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::StackSpell
                            | TargetDomain::ControlledCreatureSpell
                            | TargetDomain::StackActivatedOrTriggeredAbility
                    )
                }))
            || entry.total_cost != expected
            || entry.cast_from
                != match activated.functioning_zone {
                    FunctioningZone::Hand => Zone::Hand {
                        owner: entry.controller,
                    },
                    FunctioningZone::Graveyard => Zone::Graveyard {
                        owner: entry.controller,
                    },
                    FunctioningZone::Battlefield => Zone::Battlefield,
                    FunctioningZone::Stack | FunctioningZone::Any => {
                        return Err("invalid activation functioning zone".into());
                    }
                }
            || match &entry.paid {
                Some(paid) => *paid != expected,
                None => {
                    !announcing
                        || !(matches!(
                            activated.cost,
                            ActivationCost::Mana { .. }
                                | ActivationCost::DiscardSource { .. }
                                | ActivationCost::ExileSource { .. }
                                | ActivationCost::Composite { .. }
                                | ActivationCost::SacrificePermanent { .. }
                                | ActivationCost::ChooseObjects(_)
                                | ActivationCost::SacrificeSource
                                | ActivationCost::Loyalty { .. }
                        ) || (!program.targets.is_empty()))
                }
            }
        {
            return Err("snapshot activated invocation invariant".into());
        }
        if let ActivationCost::ChooseObjects(cost) = &activated.cost
            && entry.paid.is_none()
        {
            self.validate_chosen_announcement_stage(entry, cost, stage)?;
        }

        if matches!(
            activated.functioning_zone,
            FunctioningZone::Hand | FunctioningZone::Graveyard
        ) {
            let fact = entry
                .source_facts
                .as_ref()
                .ok_or("missing hand activation source facts")?;
            // CR 602.2b, 601.2h: the source card is the cost, so an unpaid
            // entry keeps it in place and a paid entry has moved it away.
            let shape = match activated.cost {
                ActivationCost::ExileSource { .. } => {
                    activated.functioning_zone == FunctioningZone::Graveyard
                }
                ActivationCost::DiscardSource { .. } => hand_discard_activation(activated),
                // CR 702.84a: unearth pays mana; its card stays until resolution.
                _ => activated.is_unearth(),
            };
            if !shape
                || fact.object != entry.source
                || fact.owner != entry.controller
                || fact.zone != entry.cast_from
                || (entry.paid.is_none()
                    && !self.zone_source_payable(
                        activated.functioning_zone,
                        entry.controller,
                        entry.source,
                    ))
                || (entry.paid.is_some()
                    && !activated.is_unearth()
                    && self.current(entry.source).is_ok())
            {
                return Err("snapshot hand activation cost/source invariant".into());
            }
        }
        Ok(())
    }
    pub(super) fn validate_stack_bindings(&self, entry: &StackInstance) -> Result<(), String> {
        if entry.mana_uncounterable && (!entry.kind.is_spell() || entry.paid.is_none()) {
            return Err("spending rider requires a paid spell".into());
        }
        self.validate_paid_chosen_cost(entry)?;
        // CR 601.2a, 702.34a, 903.8: a spell is cast by its owner from that
        // owner's own hand, from the command zone as that owner's commander,
        // or from that owner's own graveyard only with flashback. No other
        // origin exists for a cast spell (a copy is never cast).
        if entry.kind.is_spell() && !self.is_spell_copy(entry) {
            let owner = self
                .state
                .rules
                .cards
                .get(&entry.source.card)
                .ok_or("missing spell physical card")?
                .owner;
            let consistent = entry.controller == owner
                && match entry.cast_from {
                    Zone::Hand { owner: from } => from == owner,
                    Zone::Command { owner: from } => {
                        from == owner
                            && self
                                .state
                                .rules
                                .commanders
                                .history(entry.source.card)
                                .is_some()
                    }
                    Zone::Graveyard { owner: from } => {
                        from == owner && {
                            let d = self.stack_definition(entry)?;
                            d.flashback.is_some() || d.retrace || d.escape.is_some()
                        }
                    }
                    // CR 715.3d: an adventurer exiled by its resolving
                    // Adventure, cast normally. Replay authenticates the
                    // exiled incarnation's permission.
                    Zone::Exile => self.adventurer_cast_from_exile(entry)?,
                    Zone::Battlefield | Zone::Library { .. } | Zone::Stack => false,
                };
            if !consistent {
                return Err("snapshot spell casting origin invariant".into());
            }
        }
        self.validate_modes(entry)?;
        self.validate_cost_lock(entry)?;
        self.validate_damage_bindings(entry)?;
        self.validate_target_triggers(entry)?;
        self.validate_additional_cost_entry(entry)?;

        if entry.kind.is_spell()
            && entry.cast_from == Zone::Exile
            && !self.adventurer_cast_from_exile(entry)?
        {
            return Err("snapshot alternative casting cost/origin invariant".into());
        }
        // CR 107.3a, 602.2b: a spell or activated ability announces its X.
        let x_spell = match entry.kind {
            StackKind::Spell => self.stack_definition(entry)?.x_mana_cost,
            StackKind::Activated { ability, .. } => self
                .stack_definition(entry)?
                .activated
                .get(ability as usize)
                .is_some_and(|a| a.x_mana_cost),
            _ => false,
        };
        let announcing_x = matches!(&self.state.rules.frame, Frame::AnnounceX { cast } if cast.stack_id == entry.id);
        // CR 601.2b: optional additional costs and hybrid symbols are
        // announced before X, so X stays unbound through those stages.
        let before_x = announcing_x
            || matches!(&self.state.rules.frame,
                Frame::ChooseModes { cast } | Frame::AdditionalCosts { cast } | Frame::HybridCost { cast } if cast.stack_id == entry.id);
        let x_bound = x_spell && !before_x;
        // A game can end (CR 104.2a) while a spell is still being announced;
        // its unpaid entry then keeps whatever it had announced.
        let unfinished = entry.paid.is_none()
            && matches!(
                self.state.rules.frame,
                Frame::Fault { .. } | Frame::Finished { .. }
            );
        if entry.x.as_ref().is_some_and(|q| q.is_negative())
            || (entry.x.is_some() != x_bound && !(unfinished && x_spell))
            || (announcing_x && !x_spell)
            || entry.death_controller.is_some_and(|p| p.0 >= 4)
            || entry.death_controller.is_some()
                != matches!(&entry.kind,StackKind::Triggered {ability: ability @ EffectiveTriggerAbility::Intrinsic { .. },..} if triggers::binds_death_controller(&*self.effective_trigger(entry.source, entry.source_face, *ability)?))
        {
            return Err("snapshot X/death LKI binding invariant".into());
        }
        let program = self.program_for(entry)?;
        let stack_domain = program.targets.first().is_some_and(|target| {
            matches!(
                target.domain,
                TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility
            )
        });
        if !entry.stack_targets.is_empty() {
            if !stack_domain
                || !entry.targets.is_empty()
                || !entry.player_targets.is_empty()
                || entry.stack_targets.len() > program.targets.len()
                || entry.announced_stack_targets.len() != 4
                || entry
                    .announced_stack_targets
                    .iter()
                    .any(|entries| entries.len() != entry.stack_targets.len())
                || entry.stack_targets.iter().any(|target| *target >= entry.id)
            {
                return Err("snapshot stack target binding invariant".into());
            }
        } else if !entry.announced_stack_targets.is_empty() {
            return Err("snapshot stack target announcement invariant".into());
        }
        if stack_domain && (!entry.targets.is_empty() || !entry.player_targets.is_empty()) {
            return Err("snapshot wrong target binding domain".into());
        }
        Ok(())
    }
    pub(super) fn eligible_stack_targets(
        &self,
        domain: TargetDomain,
        exclude: Option<StackId>,
    ) -> Vec<StackId> {
        self.state
            .rules
            .stack
            .iter()
            .filter(|entry| {
                Some(entry.id) != exclude
                    && self.stack_entry_complete(entry)
                    && match domain {
                        TargetDomain::StackSpell => entry.kind.is_spell(),
                        TargetDomain::StackActivatedOrTriggeredAbility => !entry.kind.is_spell(),
                        _ => false,
                    }
            })
            .map(|entry| entry.id)
            .collect()
    }
    // CR 101.2/101.3, 113.6 and 608.2b: protection applies when countering
    // the source spell. Its stack target legality is evaluated separately.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn spell_cannot_be_countered(&self, entry: &StackInstance) -> Result<bool, String> {
        if !entry.kind.is_spell() {
            return Ok(false);
        }
        Ok(entry.mana_uncounterable
            || self.has_uncounterable_static(entry.source, entry.controller)?)
    }
}

/// CR 107.4e-f: one allowed alternative per printed symbol, in order.
pub(crate) fn validate_hybrid_choice(
    symbols: &[ColorHybridSymbol],
    colors: &[HybridPayment],
) -> Result<(), String> {
    if symbols.is_empty()
        || symbols.len() != colors.len()
        || !symbols
            .iter()
            .zip(colors)
            .all(|(s, c)| s.colors.contains(c))
    {
        return Err("hybrid announcement requires one allowed color per printed symbol".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A land-return scenario whose battlefield land is given three loyalty
    /// counters; loyalty payment reads only the row, not the card type.
    fn loyal_source() -> (RulesTxn, ObjectId) {
        let prepared = Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap();
        let mut txn = RulesTxn::new(prepared);
        let object = txn.state.rules.objects[&CardId(0)].id;
        let row = txn.rules_mut().objects.get_mut(&object.card).unwrap();
        assert!(row.present_on_battlefield() && row.controller == PlayerId(0));
        row.loyalty_counters = 3.into();
        (txn, object)
    }

    /// CR 606.4: a counter-only change never stamps the separate use.
    #[test]
    fn loyalty_counter_payment_is_exact() {
        for (change, current) in [(0, 3), (2, 5), (-3, 0)] {
            let (mut txn, object) = loyal_source();
            txn.pay_loyalty_counters(object, &change.into()).unwrap();
            let row = txn.current(object).unwrap();
            assert_eq!(row.loyalty_counters, current.into(), "{change}");
            assert_eq!(row.loyalty_used_turn, None, "{change}");
        }
        let (mut txn, object) = loyal_source();
        let before = txn.state.rules.objects[&object.card].clone();
        assert!(txn.pay_loyalty_counters(object, &(-4).into()).is_err());
        assert_eq!(txn.state.rules.objects[&object.card], before);
    }
}

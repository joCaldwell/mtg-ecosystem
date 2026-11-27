//! Finite mode selection, independent target occurrences and intervening-if.
//! CR 601.2b-c, 603.3d/603.4/603.10a and 608.2a-b, pinned official rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Atom, Cmp, ControllerMeasure, Intervening, Pred};
#[cfg(feature = "testkit")]
pub const SEEDS: &str = "dd3d4d96-1e88-49e1-8276-7f62c22ad91a";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceInformation {
    Current,
    LastKnown,
}
/// Historical battlefield incarnation and its derived power at this check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualifyingCreature {
    pub object: ObjectId,
    pub power: Quantity,
}
/// CR 400.7c, 603.4 and 608.3a. The successful permanent spell and its
/// actual battlefield successor identify the entry through source departure.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryCast {
    pub stack_id: StackId,
    pub spell: ObjectId,
    pub caster: PlayerId,
    /// CR 118.9, 702.74a: the caster announced and paid the spell's printed
    /// alternative cost.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub alternative_cost_paid: bool,
}

/// The actual value read for an intervening comparison (CR 603.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerMeasurement {
    Life(Quantity),
    HandCount(u32),
    GraveyardCount(u32),
    CreatureTotalPower(Quantity),
}

impl ControllerMeasurement {
    fn quantity(&self) -> Quantity {
        match self {
            Self::Life(value) | Self::CreatureTotalPower(value) => value.clone(),
            Self::HandCount(count) | Self::GraveyardCount(count) => {
                Quantity::from(i64::from(*count))
            }
        }
    }

    fn validates(&self, measure: &ControllerMeasure, owned_limit: u32) -> bool {
        match (self, measure) {
            (Self::Life(_), ControllerMeasure::Life) => true,
            (Self::HandCount(count), ControllerMeasure::HandCount)
            | (Self::GraveyardCount(count), ControllerMeasure::GraveyardCount(_)) => {
                *count <= owned_limit
            }
            _ => false,
        }
    }
}

/// Evidence of the information actually read, never a fabricated hand count.
/// The hand-empty variant retains its existing serialized field shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case", deny_unknown_fields)]
pub enum InterveningCheck {
    TurnHistory {
        controller: PlayerId,
        observed: Quantity,
        passed: bool,
    },
    SourceCastByController {
        entrant: ObjectId,
        controller: PlayerId,
        cast: Option<EntryCast>,
    },
    /// CR 702.74a: whether the entry's spell paid its alternative cost.
    SourceCostPaid {
        entrant: ObjectId,
        controller: PlayerId,
        cast: Option<EntryCast>,
    },
    ControlsOwnCommander {
        controller: PlayerId,
        commander: Option<ObjectId>,
        passed: bool,
    },
    OpponentControlsMore {
        controller: PlayerId,
        own: u32,
        opponents: Vec<(PlayerId, u32)>,
        passed: bool,
    },
    ControllerHandEmpty {
        controller: PlayerId,
        hand_count: u32,
        passed: bool,
    },
    ControllerComparison {
        controller: PlayerId,
        observed: ControllerMeasurement,
        passed: bool,
    },
    SourceStatus {
        controller: PlayerId,
        source: ObjectId,
        status: TapStatus,
        tapped: bool,
        information: SourceInformation,
        passed: bool,
    },
    /// CR 109.5, 608.2h: how many battlefield permanents satisfied the
    /// clause's filter at this check.
    BattlefieldCount {
        controller: PlayerId,
        count: u32,
        passed: bool,
    },
    ControllerHasCreatureWithPower {
        controller: PlayerId,
        minimum_power: Quantity,
        qualifying: Option<QualifyingCreature>,
    },
    /// CR 702.30a: the controller's upkeep count when the source came
    /// under that player's control (absent if another player controls it
    /// or it never entered), and the controller's upkeep count now.
    ControlledSinceLastUpkeep {
        controller: PlayerId,
        source: ObjectId,
        since: Option<u64>,
        upkeeps: u64,
    },
}
impl InterveningCheck {
    pub fn passed(&self) -> bool {
        match self {
            Self::TurnHistory { passed, .. }
            | Self::ControlsOwnCommander { passed, .. }
            | Self::OpponentControlsMore { passed, .. }
            | Self::ControllerHandEmpty { passed, .. }
            | Self::ControllerComparison { passed, .. }
            | Self::SourceStatus { passed, .. }
            | Self::BattlefieldCount { passed, .. } => *passed,
            Self::ControllerHasCreatureWithPower { qualifying, .. } => qualifying.is_some(),
            Self::SourceCastByController {
                controller, cast, ..
            } => cast.is_some_and(|c| c.caster == *controller),
            Self::SourceCostPaid { cast, .. } => cast.is_some_and(|c| c.alternative_cost_paid),
            // The last upkeep is the one before the current count.
            Self::ControlledSinceLastUpkeep { since, upkeeps, .. } => {
                since.is_some_and(|n| n.saturating_add(1) >= *upkeeps)
            }
        }
    }
    fn validates(
        &self,
        condition: &Intervening,
        controller: PlayerId,
        source: ObjectId,
        card_bound: u32,
        witness: &TriggerEventWitness,
    ) -> bool {
        match (condition, self) {
            (
                Intervening::TurnHistory { fact, cmp, value },
                Self::TurnHistory {
                    controller: checked,
                    observed,
                    passed,
                },
            ) => {
                use mtg_contracts::kir::TurnFact::*;
                *checked == controller
                    && !observed.is_negative()
                    && (!matches!(
                        fact,
                        Attacked
                            | CreatureDied
                            | CreatureDiedUnderYourControl
                            | PermanentLeftUnderYourControl
                    ) || *observed <= Quantity::from(1))
                    && *passed == compare(observed, *cmp, value)
            }
            (
                Intervening::SourceCastByController,
                Self::SourceCastByController {
                    entrant,
                    controller: checked,
                    cast,
                },
            )
            | (
                Intervening::SourceCostPaid { .. },
                Self::SourceCostPaid {
                    entrant,
                    controller: checked,
                    cast,
                },
            ) => {
                *checked == controller
                    && *entrant == source
                    && cast.is_none_or(|c| {
                        c.caster.0 < 4
                            && c.spell.card == entrant.card
                            && c.spell.incarnation.checked_add(1) == Some(entrant.incarnation)
                    })
            }
            (
                Intervening::ControlsOwnCommander,
                Self::ControlsOwnCommander {
                    controller: checked,
                    commander,
                    passed,
                },
            ) => *checked == controller && *passed == commander.is_some(),
            (
                Intervening::OpponentControlsMore { .. }
                | Intervening::EventOpponentControlsMore { .. },
                Self::OpponentControlsMore {
                    controller: checked,
                    own,
                    opponents,
                    passed,
                },
            ) => {
                *checked == controller
                    && (!matches!(condition, Intervening::EventOpponentControlsMore { .. })
                        || witness.permanent_count_opponent().is_some_and(|actor| {
                            opponents
                                .as_slice()
                                .first()
                                .is_none_or(|(p, _)| *p == actor)
                                && opponents.len() <= 1
                        }))
                    && opponents.len() <= 3
                    && opponents.iter().all(|(p, _)| p.0 < 4 && *p != controller)
                    && opponents.windows(2).all(|pair| pair[0].0 < pair[1].0)
                    && *passed == opponents.iter().any(|(_, count)| count > own)
            }
            (
                Intervening::ControllerHandEmpty,
                Self::ControllerHandEmpty {
                    controller: c,
                    hand_count,
                    passed,
                },
            ) => *c == controller && *hand_count <= card_bound && *passed == (*hand_count == 0),
            (
                Intervening::ControllerCreatureTotalPower { cmp, value },
                Self::ControllerComparison {
                    controller: checked,
                    observed: ControllerMeasurement::CreatureTotalPower(total),
                    passed,
                },
            ) => *checked == controller && *passed == compare(total, *cmp, value),
            (
                Intervening::ControllerComparison {
                    measure,
                    cmp,
                    value,
                },
                Self::ControllerComparison {
                    controller: checked,
                    observed,
                    passed,
                },
            ) => {
                *checked == controller
                    && observed.validates(measure, card_bound)
                    && *passed == compare(&observed.quantity(), *cmp, value)
            }
            (
                Intervening::SourceTapped { tapped: wanted },
                Self::SourceStatus {
                    controller: c,
                    source: s,
                    status: expected,
                    tapped,
                    passed,
                    ..
                },
            ) => {
                *c == controller
                    && *s == source
                    && *expected == tap_status(*wanted)
                    && *passed == (*wanted == *tapped)
            }
            (
                Intervening::SourceControlledSinceLastUpkeep,
                Self::ControlledSinceLastUpkeep {
                    controller: c,
                    source: s,
                    since,
                    upkeeps,
                },
            ) => *c == controller && *s == source && since.is_none_or(|n| n <= *upkeeps),
            (
                Intervening::BattlefieldCount { cmp, value, .. },
                Self::BattlefieldCount {
                    controller: c,
                    count,
                    passed,
                },
            ) => {
                *c == controller
                    && *passed == compare(&Quantity::from(i64::from(*count)), *cmp, value)
            }
            (
                Intervening::ControllerHasCreatureWithPower { minimum_power },
                Self::ControllerHasCreatureWithPower {
                    controller: c,
                    minimum_power: recorded,
                    qualifying,
                },
            ) => {
                *c == controller
                    && recorded == minimum_power
                    && qualifying
                        .as_ref()
                        .is_none_or(|fact| &fact.power >= minimum_power)
            }
            _ => false,
        }
    }
}
/// CR 603.4: the audited intervening clause of a triggered ability, if any.
pub(super) fn intervening(definition: &TriggeredAbility) -> Result<Option<Intervening>, String> {
    definition
        .condition
        .intervening_if()
        .map(Intervening::of)
        .transpose()
        .map_err(|e| format!("unaudited intervening condition: {e}"))
}
fn compare(observed: &Quantity, cmp: Cmp, value: &Quantity) -> bool {
    match cmp {
        Cmp::AtLeast => observed >= value,
        Cmp::AtMost => observed <= value,
        Cmp::Exactly => observed == value,
    }
}
fn tap_status(tapped: bool) -> TapStatus {
    if tapped {
        TapStatus::Tapped
    } else {
        TapStatus::Untapped
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub enum InterveningStage {
    Match { occurrence: TriggerOccurrenceId },
    ResolutionStart { stack_id: StackId },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerPlacementFailure {
    NoLegalTargets,
    NoModesChosen,
}

impl RulesTxn {
    pub(super) fn bound_player_target<'a>(
        &self,
        entry: &'a StackInstance,
        slot: usize,
    ) -> Option<&'a PlayerId> {
        entry.player_targets.get(slot)
    }
    pub(super) fn bound_damage_target<'a>(
        &self,
        entry: &'a StackInstance,
        slot: usize,
    ) -> Option<&'a TargetEntity> {
        entry.entity_targets.get(slot)
    }
    pub(super) fn bound_object_target<'a>(
        &self,
        entry: &'a StackInstance,
        slot: usize,
    ) -> Option<&'a ObjectId> {
        entry.targets.get(slot)
    }

    pub(super) fn bound_stack_target<'a>(
        &self,
        entry: &'a StackInstance,
        slot: usize,
    ) -> Option<&'a StackId> {
        entry.stack_targets.get(slot)
    }
    pub(super) fn completed_entry_cast(
        &self,
        cause: &BatchCause,
        witness: &TriggerEventWitness,
    ) -> Option<EntryCast> {
        let BatchCause::SpellCompletion {
            stack_id,
            player,
            source,
        } = cause
        else {
            return None;
        };
        let TriggerEventWitness::Moved { movement } = witness else {
            return None;
        };
        let spell = self.state.rules.stack.last()?;
        let alternative_cost_paid = self
            .offered_alternative_cost(spell)
            .ok()
            .flatten()
            .is_some_and(|cost| {
                spell
                    .selected_additional_costs
                    .as_ref()
                    .is_some_and(|selected| selected.contains(&cost.identity))
            });
        (movement.from == Zone::Stack
            && movement.destination == Zone::Battlefield
            && movement.previous == *source
            && movement.successor.card == source.card
            && source.incarnation.checked_add(1) == Some(movement.successor.incarnation)
            && matches!(&self.state.rules.frame, Frame::Resolve { frame } if frame.stack_id == *stack_id && self.entry(frame.stack_id).is_ok_and(|entry| entry.source == *source))
            && spell.source == *source
            && spell.id == *stack_id
            && spell.controller == *player
            && spell.kind.is_spell()
            && spell.paid.as_ref() == Some(&spell.total_cost))
        .then_some(EntryCast {
            stack_id: *stack_id,
            spell: *source,
            caster: *player,
            alternative_cost_paid,
        })
    }
    // CR603.4 checks twice; CR608.2h/113.7a use current expected-zone
    // information or the last battlefield incarnation (CR400.7,110.5d).
    // The existing per-invocation ObjectBefore is refreshed at departure by
    // retain_source_lki, including taps AFTER collection. Its lifetime is
    // bounded by outstanding stack/pending entries, independent of tracing.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn check_intervening(
        &self,
        condition: &Intervening,
        history: &[super::triggers::TurnCastHistory; 4],
        controller: PlayerId,
        source: ObjectId,
        last_known: &ObjectBefore,
        event: (&TriggerEventWitness, Option<EntryCast>),
    ) -> Result<InterveningCheck, String> {
        if !self.is_live(controller) {
            return Err("intervening condition requested after controller departure".into());
        }
        let (witness, cast) = event;
        Ok(match condition {
            Intervening::TurnHistory { fact, cmp, value } => {
                use mtg_contracts::kir::TurnFact::*;
                let row = &history[controller.0 as usize];
                let quantity = |n: u32| Quantity::from(i64::from(n));
                let spell_count = |n| {
                    u32::try_from(n)
                        .map(quantity)
                        .map_err(|_| "turn history count limit")
                };
                let observed = match fact {
                    Attacked => quantity(u32::from(row.attacked)),
                    CreatureDied => {
                        quantity(u32::from(history.iter().any(|seat| seat.creature_died)))
                    }
                    CreatureDiedUnderYourControl => quantity(u32::from(row.creature_died)),
                    PermanentLeftUnderYourControl => quantity(u32::from(row.permanent_left)),
                    SpellsCast => spell_count(row.spells.len())?,
                    NoncreatureSpellsCast => spell_count(
                        row.spells
                            .iter()
                            .filter(|spell| !spell.characteristics.has_type(CardKind::Creature))
                            .count(),
                    )?,
                    SpellsCastFromHand => quantity(row.hand_casts),
                    CardsDrawn => quantity(row.draws),
                    LifeGained => row.life_gained.clone(),
                    ArtifactsEnteredUnderYourControl => row.artifacts_entered.clone(),
                    CreaturesEnteredUnderYourControl => row.creatures_entered.clone(),
                    NonlandPermanentsEnteredUnderYourControl => row.nonlands_entered.clone(),
                    CreaturesEnteredUnderOpponentsControl => {
                        let mut total = Quantity::zero();
                        // CR 102.3: this is turn history, including a physical
                        // opponent seat whose player has since left the game.
                        for (seat, row) in history.iter().enumerate() {
                            if seat != controller.0 as usize {
                                total.add(&row.creatures_entered);
                            }
                        }
                        total
                    }
                };
                let passed = compare(&observed, *cmp, value);
                InterveningCheck::TurnHistory {
                    controller,
                    observed,
                    passed,
                }
            }
            Intervening::SourceCastByController => InterveningCheck::SourceCastByController {
                entrant: source,
                controller,
                cast,
            },
            Intervening::SourceCostPaid { .. } => InterveningCheck::SourceCostPaid {
                entrant: source,
                controller,
                cast,
            },
            // CR 603.4, 608.2h: the present source, or its last known
            // information once it has left the battlefield.
            Intervening::SourceControlledSinceLastUpkeep => {
                let (holder, since) = match self.current(source) {
                    Ok(o) if o.id == source && o.zone == Zone::Battlefield => {
                        (o.controller, o.control_upkeeps)
                    }
                    _ => (last_known.controller, last_known.control_upkeeps),
                };
                InterveningCheck::ControlledSinceLastUpkeep {
                    controller,
                    source,
                    since: since.filter(|_| holder == controller),
                    upkeeps: self.state.rules.players[controller.0 as usize].upkeeps,
                }
            }
            Intervening::ControlsOwnCommander => {
                let commander = self
                    .state
                    .rules
                    .objects
                    .values()
                    .find(|object| {
                        object.present_on_battlefield()
                            && object.controller == controller
                            && self
                                .state
                                .rules
                                .commanders
                                .history(object.id.card)
                                .is_some_and(|history| history.owner == controller)
                    })
                    .map(|object| object.id);
                InterveningCheck::ControlsOwnCommander {
                    controller,
                    commander,
                    passed: commander.is_some(),
                }
            }
            Intervening::OpponentControlsMore { kind }
            | Intervening::EventOpponentControlsMore { kind } => {
                let actor = if matches!(condition, Intervening::EventOpponentControlsMore { .. }) {
                    Some(
                        witness
                            .permanent_count_opponent()
                            .ok_or("missing comparison opponent")?,
                    )
                } else {
                    None
                };
                let mut counts = [0_u32; 4];
                for object in self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.present_on_battlefield())
                {
                    if self.characteristics(object.id)?.has_type(*kind) {
                        counts[object.controller.0 as usize] += 1;
                    }
                }
                let own = counts[controller.0 as usize];
                let mut opponents = self
                    .opponents(controller)
                    .into_iter()
                    .filter(|p| actor.is_none_or(|actor| *p == actor))
                    .map(|p| (p, counts[p.0 as usize]))
                    .collect::<Vec<_>>();
                opponents.sort_unstable();
                InterveningCheck::OpponentControlsMore {
                    controller,
                    own,
                    passed: opponents.iter().any(|(_, count)| *count > own),
                    opponents,
                }
            }
            Intervening::ControllerHandEmpty => {
                let hand_count = self.trigger_hand_count(controller)?;
                InterveningCheck::ControllerHandEmpty {
                    controller,
                    hand_count,
                    passed: hand_count == 0,
                }
            }
            Intervening::ControllerCreatureTotalPower { cmp, value } => {
                let mut total = Quantity::zero();
                for object in self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.present_on_battlefield() && o.controller == controller)
                {
                    let characteristics = self.characteristics(object.id)?;
                    if characteristics.has_type(CardKind::Creature) {
                        total.add(
                            &characteristics
                                .power
                                .ok_or("missing battlefield creature power")?,
                        );
                    }
                }
                InterveningCheck::ControllerComparison {
                    controller,
                    passed: compare(&total, *cmp, value),
                    observed: ControllerMeasurement::CreatureTotalPower(total),
                }
            }
            Intervening::ControllerComparison {
                measure,
                cmp,
                value,
            } => {
                let observed = match measure {
                    ControllerMeasure::Life => ControllerMeasurement::Life(
                        self.state.rules.players[controller.0 as usize].life.clone(),
                    ),
                    ControllerMeasure::HandCount => {
                        ControllerMeasurement::HandCount(self.trigger_hand_count(controller)?)
                    }
                    ControllerMeasure::GraveyardCount(filter) => {
                        let count = self
                            .state
                            .rules
                            .objects
                            .values()
                            .filter(|o| o.zone == Zone::Graveyard { owner: controller })
                            .filter(|o| self.intervening_card_matches(o.id.card, filter))
                            .count();
                        ControllerMeasurement::GraveyardCount(
                            u32::try_from(count).map_err(|_| "graveyard count limit")?,
                        )
                    }
                };
                InterveningCheck::ControllerComparison {
                    controller,
                    passed: compare(&observed.quantity(), *cmp, value),
                    observed,
                }
            }
            Intervening::SourceTapped { tapped: wanted } => {
                let (tapped, information) = match self.current(source) {
                    Ok(o) if o.zone == Zone::Battlefield => (o.tapped, SourceInformation::Current),
                    _ if last_known.object == source && last_known.zone == Zone::Battlefield => {
                        (last_known.tapped, SourceInformation::LastKnown)
                    }
                    _ => return Err("missing last-known battlefield source status".into()),
                };
                InterveningCheck::SourceStatus {
                    controller,
                    source,
                    status: tap_status(*wanted),
                    tapped,
                    information,
                    passed: *wanted == tapped,
                }
            }
            Intervening::BattlefieldCount { filter, cmp, value } => {
                // CR 603.4: the board as it is now; the source compares by
                // its identity, including after it has left.
                let identity = |r: &mtg_contracts::kir::Ref| {
                    (*r == mtg_contracts::kir::Ref::Source).then_some(source)
                };
                let count = self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.present_on_battlefield())
                    .filter(|o| {
                        triggers::holds(
                            filter,
                            Some(&self.object_facts(o)),
                            controller,
                            o.id,
                            &identity,
                        )
                    })
                    .count();
                let count = u32::try_from(count).map_err(|_| "battlefield count limit")?;
                InterveningCheck::BattlefieldCount {
                    controller,
                    count,
                    passed: compare(&Quantity::from(i64::from(count)), *cmp, value),
                }
            }
            Intervening::ControllerHasCreatureWithPower { minimum_power } => {
                let mut qualifying = None;
                for object in self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Battlefield && o.controller == controller)
                {
                    let characteristics = self.characteristics(object.id)?;
                    if characteristics.has_type(CardKind::Creature) {
                        let power = characteristics
                            .power
                            .ok_or("missing battlefield creature power")?;
                        if &power >= minimum_power {
                            qualifying = Some(QualifyingCreature {
                                object: object.id,
                                power,
                            });
                            break;
                        }
                    }
                }
                InterveningCheck::ControllerHasCreatureWithPower {
                    controller,
                    minimum_power: minimum_power.clone(),
                    qualifying,
                }
            }
        })
    }
    fn trigger_hand_count(&self, controller: PlayerId) -> Result<u32, String> {
        u32::try_from(
            self.state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == Zone::Hand { owner: controller })
                .count(),
        )
        .map_err(|_| "hand count limit".into())
    }
    pub(crate) fn check_resolution_condition(
        &mut self,
        stack: &StackInstance,
    ) -> Result<Option<InterveningCheck>, String> {
        let StackKind::Triggered {
            ability,
            match_condition,
            witness,
            ..
        } = &stack.kind
        else {
            return Ok(None);
        };
        if matches!(ability, EffectiveTriggerAbility::Followup { .. }) {
            return Ok(None);
        }
        let definition = self.effective_trigger(stack.source, stack.program_face(), *ability)?;
        let Some(condition) = intervening(&definition)? else {
            return Ok(None);
        };
        let check = self.check_intervening(
            &condition,
            &self.state.rules.cast_history,
            stack.controller,
            stack.source,
            stack
                .source_facts
                .as_ref()
                .ok_or("missing trigger source facts")?,
            (
                witness,
                match match_condition {
                    Some(
                        InterveningCheck::SourceCastByController { cast, .. }
                        | InterveningCheck::SourceCostPaid { cast, .. },
                    ) => *cast,
                    _ => None,
                },
            ),
        )?;
        self.record_event(Event::TriggerConditionChecked {
            source: stack.source,
            controller: stack.controller,
            ability: *ability,
            stage: InterveningStage::ResolutionStart { stack_id: stack.id },
            check: check.clone(),
        })?;
        Ok(Some(check))
    }
    pub(super) fn validate_choices_state(&self) -> Result<(), String> {
        if let Frame::Resolve { frame } | Frame::ResolutionChoice { frame, .. } =
            &self.state.rules.frame
        {
            self.validate_condition_cache(frame)?;
        }
        Ok(())
    }
    pub(crate) fn validate_condition_cache(&self, frame: &Resolving) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        let definition = match entry.kind {
            StackKind::Triggered {
                ability: ability @ EffectiveTriggerAbility::Intrinsic { .. },
                ..
            } => Some(self.effective_trigger(entry.source, entry.program_face(), ability)?),
            _ => None,
        };
        let expected = match &definition {
            Some(definition) => intervening(definition)?,
            _ => None,
        };
        if matches!(
            expected,
            Some(Intervening::SourceCastByController | Intervening::SourceCostPaid { .. })
        ) && !matches!(&entry.kind, StackKind::Triggered { match_condition, .. }
                if match_condition == &frame.condition_check)
        {
            return Err("cast-entry resolution lost its match evidence".into());
        }
        if let Some(
            InterveningCheck::TurnHistory { passed: false, .. }
            | InterveningCheck::ControllerComparison { passed: false, .. }
            | InterveningCheck::ControlsOwnCommander { passed: false, .. }
            | InterveningCheck::OpponentControlsMore { passed: false, .. },
        ) = &frame.condition_check
            && (frame.cursor != 0
                || frame.draw_progress.is_some()
                || frame.each_player_draw.is_some()
                || frame.set_damage.is_some()
                || frame.random_discard.is_some()
                || frame.group_discard.is_some())
        {
            return Err("snapshot progressed after failed intervening comparison".into());
        }
        match (expected, &frame.condition_check, &entry.kind) {
            (None, None, _) => Ok(()),
            (Some(condition), Some(check), StackKind::Triggered { witness, .. })
                if check.validates(
                    &condition,
                    entry.controller,
                    entry.source,
                    self.intervening_card_bound(entry.controller, &condition),
                    witness,
                ) =>
            {
                Ok(())
            }
            _ => Err("invalid resolution condition cache".into()),
        }
    }

    /// CR 400.7, 404: in the graveyard, owned physical cards have their printed
    /// types. Use the same filter for the live count and its historical bound.
    fn intervening_card_matches(&self, card: CardId, filter: &Pred) -> bool {
        !self.state.rules.tokens.contains_key(&card)
            && (filter.0.is_empty()
                || self.definition(card).is_ok_and(|definition| {
                    let printed = &definition.characteristics;
                    filter.0.iter().all(|atom| match atom {
                        Atom::Type { kind } => printed.has_type(*kind),
                        Atom::AnyType { kinds } => kinds.iter().any(|kind| printed.has_type(*kind)),
                        _ => false,
                    })
                }))
    }

    fn intervening_card_bound(&self, controller: PlayerId, condition: &Intervening) -> u32 {
        let all_cards = Pred::default();
        let filter = match condition {
            Intervening::ControllerComparison {
                measure: ControllerMeasure::GraveyardCount(filter),
                ..
            } => filter,
            Intervening::ControllerHandEmpty
            | Intervening::ControllerComparison {
                measure: ControllerMeasure::HandCount,
                ..
            } => &all_cards,
            _ => return 0,
        };
        let count = self
            .state
            .rules
            .cards
            .iter()
            .filter(|(id, card)| {
                card.owner == controller && self.intervening_card_matches(**id, filter)
            })
            .count();
        count.min(self.prepared.config.owned_limit()) as u32
    }
}
impl RulesTxn {
    pub(super) fn validate_match_condition(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        check: Option<&InterveningCheck>,
        witness: &TriggerEventWitness,
    ) -> Result<(), String> {
        if matches!(ability, EffectiveTriggerAbility::Followup { .. }) {
            return if check.is_none() {
                Ok(())
            } else {
                Err("resolution trigger has an intervening condition".into())
            };
        }
        if let Some(
            InterveningCheck::SourceCastByController {
                entrant,
                cast: Some(cast),
                ..
            }
            | InterveningCheck::SourceCostPaid {
                entrant,
                cast: Some(cast),
                ..
            },
        ) = check
            && (cast.stack_id.0 >= self.state.rules.next_stack_id
                || self.state.rules.tokens.contains_key(&cast.spell.card)
                || !matches!(witness, TriggerEventWitness::Moved { movement } if movement.from == Zone::Stack
                    && movement.destination == Zone::Battlefield && movement.previous == cast.spell
                    && movement.successor == *entrant))
        {
            return Err("cast-entry condition does not identify its completed movement".into());
        }
        if let Some(InterveningCheck::ControlsOwnCommander {
            commander: Some(object),
            ..
        }) = check
            && (self
                .state
                .rules
                .commanders
                .history(object.card)
                .is_none_or(|c| c.owner != controller)
                || self
                    .state
                    .rules
                    .objects
                    .get(&object.card)
                    .is_none_or(|current| current.id.incarnation < object.incarnation))
        {
            return Err("invalid intervening commander evidence".into());
        }
        let definition = self.effective_trigger(source, face, ability)?;
        let expected = intervening(&definition)?;
        match (expected, check) {
            (None, None) => Ok(()),
            (Some(condition), Some(check))
                if check.validates(
                    &condition,
                    controller,
                    source,
                    self.intervening_card_bound(controller, &condition),
                    witness,
                ) && check.passed()
                    && !matches!(
                        check,
                        InterveningCheck::SourceStatus {
                            information: SourceInformation::LastKnown,
                            ..
                        }
                    ) =>
            {
                Ok(())
            }
            _ => Err("invalid trigger match condition proof".into()),
        }
    }
}
impl RulesTxn {
    pub(super) fn origin_programs<'a>(
        &'a self,
        origin: &EffectOrigin,
    ) -> Result<Vec<&'a SpellProgram>, String> {
        // CR 113.7a: the program of the face that created the effect.
        let d = self.face_definition(origin.source.card, origin.source_face)?;
        if let Some(index) = origin.activated_ability {
            return Ok(d
                .activated
                .get(index as usize)
                .ok_or("missing activated effect origin")?
                .programs()
                .collect());
        }
        Ok(d.spell_programs()
            .chain(d.activated.iter().flat_map(|a| a.programs()))
            .chain(d.triggered.iter().flat_map(|a| a.programs()))
            .collect())
    }
}

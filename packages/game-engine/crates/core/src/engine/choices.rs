//! Finite mode selection, independent target occurrences and intervening-if.
//! CR 601.2b-c, 603.3d/603.4/603.10a and 608.2a-b, pinned official rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
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
}
/// Evidence of the information actually read, never a fabricated hand count.
/// The hand-empty variant retains its existing serialized field shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case", deny_unknown_fields)]
pub enum InterveningCheck {
    SourceCastByController {
        entrant: ObjectId,
        controller: PlayerId,
        cast: Option<EntryCast>,
    },
    ControllerHandEmpty {
        controller: PlayerId,
        hand_count: u32,
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
    ControllerHasCreatureWithPower {
        controller: PlayerId,
        minimum_power: Quantity,
        qualifying: Option<QualifyingCreature>,
    },
}
impl InterveningCheck {
    pub fn passed(&self) -> bool {
        match self {
            Self::ControllerHandEmpty { passed, .. } | Self::SourceStatus { passed, .. } => *passed,
            Self::ControllerHasCreatureWithPower { qualifying, .. } => qualifying.is_some(),
            Self::SourceCastByController {
                controller, cast, ..
            } => cast.is_some_and(|c| c.caster == *controller),
        }
    }
    fn validates(
        &self,
        condition: &mtg_contracts::kir::Intervening,
        controller: PlayerId,
        source: ObjectId,
        hand_limit: u32,
    ) -> bool {
        match (condition, self) {
            (
                mtg_contracts::kir::Intervening::SourceCastByController,
                Self::SourceCastByController {
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
                mtg_contracts::kir::Intervening::ControllerHandEmpty,
                Self::ControllerHandEmpty {
                    controller: c,
                    hand_count,
                    passed,
                },
            ) => *c == controller && *hand_count <= hand_limit && *passed == (*hand_count == 0),
            (
                mtg_contracts::kir::Intervening::SourceTapped { tapped: wanted },
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
                mtg_contracts::kir::Intervening::ControllerHasCreatureWithPower { minimum_power },
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
pub(super) fn intervening(
    definition: &TriggeredAbility,
) -> Result<Option<mtg_contracts::kir::Intervening>, String> {
    definition
        .condition
        .intervening_if()
        .map(mtg_contracts::kir::Intervening::of)
        .transpose()
        .map_err(|e| format!("unaudited intervening condition: {e}"))
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
        condition: &mtg_contracts::kir::Intervening,
        controller: PlayerId,
        source: ObjectId,
        last_known: &ObjectBefore,
        cast: Option<EntryCast>,
    ) -> Result<InterveningCheck, String> {
        if !self.is_live(controller) {
            return Err("intervening condition requested after controller departure".into());
        }
        Ok(match condition {
            mtg_contracts::kir::Intervening::SourceCastByController => {
                InterveningCheck::SourceCastByController {
                    entrant: source,
                    controller,
                    cast,
                }
            }
            mtg_contracts::kir::Intervening::ControllerHandEmpty => {
                let hand_count = u32::try_from(
                    self.state
                        .rules
                        .objects
                        .values()
                        .filter(|o| o.zone == Zone::Hand { owner: controller })
                        .count(),
                )
                .map_err(|_| "hand count limit")?;
                InterveningCheck::ControllerHandEmpty {
                    controller,
                    hand_count,
                    passed: hand_count == 0,
                }
            }
            mtg_contracts::kir::Intervening::SourceTapped { tapped: wanted } => {
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
            mtg_contracts::kir::Intervening::ControllerHasCreatureWithPower { minimum_power } => {
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
    pub(crate) fn check_resolution_condition(
        &mut self,
        stack: &StackInstance,
    ) -> Result<Option<InterveningCheck>, String> {
        let StackKind::Triggered {
            ability,
            match_condition,
            ..
        } = &stack.kind
        else {
            return Ok(None);
        };
        let definition = self.effective_trigger(stack.source, *ability)?;
        let Some(condition) = intervening(&definition)? else {
            return Ok(None);
        };
        let check = self.check_intervening(
            &condition,
            stack.controller,
            stack.source,
            stack
                .source_facts
                .as_ref()
                .ok_or("missing trigger source facts")?,
            match match_condition {
                Some(InterveningCheck::SourceCastByController { cast, .. }) => *cast,
                _ => None,
            },
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
        let definition = match self.entry(frame.stack_id)?.kind {
            StackKind::Triggered { ability, .. } => {
                Some(self.effective_trigger(self.entry(frame.stack_id)?.source, ability)?)
            }
            _ => None,
        };
        let expected = match &definition {
            Some(definition) => intervening(definition)?,
            _ => None,
        };
        if expected == Some(mtg_contracts::kir::Intervening::SourceCastByController)
            && !matches!(&self.entry(frame.stack_id)?.kind, StackKind::Triggered { match_condition, .. }
                if match_condition == &frame.condition_check)
        {
            return Err("cast-entry resolution lost its match evidence".into());
        }
        match (expected, &frame.condition_check) {
            (None, None) => Ok(()),
            (Some(condition), Some(check))
                if check.validates(
                    &condition,
                    self.entry(frame.stack_id)?.controller,
                    self.entry(frame.stack_id)?.source,
                    self.prepared.config.owned_limit() as u32,
                ) =>
            {
                Ok(())
            }
            _ => Err("invalid resolution condition cache".into()),
        }
    }
}
impl RulesTxn {
    pub(super) fn validate_match_condition(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        check: Option<&InterveningCheck>,
        witness: &TriggerEventWitness,
    ) -> Result<(), String> {
        if let Some(InterveningCheck::SourceCastByController {
            entrant,
            cast: Some(cast),
            ..
        }) = check
            && (cast.stack_id.0 >= self.state.rules.next_stack_id
                || self.state.rules.tokens.contains_key(&cast.spell.card)
                || !matches!(witness, TriggerEventWitness::Moved { movement } if movement.from == Zone::Stack
                    && movement.destination == Zone::Battlefield && movement.previous == cast.spell
                    && movement.successor == *entrant))
        {
            return Err("cast-entry condition does not identify its completed movement".into());
        }
        let definition = self.effective_trigger(source, ability)?;
        let expected = intervening(&definition)?;
        match (expected, check) {
            (None, None) => Ok(()),
            (Some(condition), Some(check))
                if check.validates(
                    &condition,
                    controller,
                    source,
                    self.prepared.config.owned_limit() as u32,
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
        let d = self.copiable_definition(origin.source)?;
        if let Some(index) = origin.activated_ability {
            let root = &d
                .activated
                .get(index as usize)
                .ok_or("missing activated effect origin")?
                .program;
            return Ok(vec![root]);
        }
        Ok(d.spell
            .iter()
            .chain(d.activated.iter().map(|a| &a.program))
            .chain(d.triggered.iter().map(|a| &a.program))
            .collect())
    }
}

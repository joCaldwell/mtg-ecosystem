//! Finite mode selection, independent target occurrences and intervening-if.
//! Exact44-card bounds:10 creatures *2 Artists +one Bouncer self-death
//! <=21 triggers per nonempty-stack episode; <=14 ordinary entries gives35
//! <128. Main-phase permanent recasts require empty stack; no recursion/untap
//! is admitted. Seeds3 +two Salves +Bouncer +Healer <=7 resolved effects <16.
//! CR 601.2b-c, 603.3d/603.4/603.10a and 608.2a-b, pinned official rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(super) const SEEDS: &str = "dd3d4d96-1e88-49e1-8276-7f62c22ad91a";
pub(super) const SALVE: &str = "8da8644c-75a1-4fe9-8e94-900d948d631c";
pub(super) const BOUNCER: &str = "82076865-7917-4d3b-8fe8-c21bfcd64d29";
pub(super) const CHOICE_EXTRA: [(&str, &str); 3] = [
    (
        SEEDS,
        definition_digest!("MTG_DEFINITION_choices_dd3d4d96-1e88-49e1-8276-7f62c22ad91a"),
    ),
    (
        SALVE,
        definition_digest!("MTG_DEFINITION_choices_8da8644c-75a1-4fe9-8e94-900d948d631c"),
    ),
    (
        BOUNCER,
        definition_digest!("MTG_DEFINITION_choices_82076865-7917-4d3b-8fe8-c21bfcd64d29"),
    ),
];
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
/// Evidence of the information actually read, never a fabricated hand count.
/// The hand-empty variant retains its existing serialized field shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case", deny_unknown_fields)]
pub enum InterveningCheck {
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
        }
    }
    fn validates(
        &self,
        condition: &InterveningCondition,
        controller: PlayerId,
        source: ObjectId,
        hand_limit: u32,
    ) -> bool {
        match (condition, self) {
            (
                InterveningCondition::ControllerHandEmpty,
                Self::ControllerHandEmpty {
                    controller: c,
                    hand_count,
                    passed,
                },
            ) => *c == controller && *hand_count <= hand_limit && *passed == (*hand_count == 0),
            (
                InterveningCondition::SourceStatus { status },
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
                    && *expected == *status
                    && *passed == status.matches(*tapped)
            }
            (
                InterveningCondition::ControllerHasCreatureWithPower { minimum_power },
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
impl ScenarioConfig {
    pub fn choices() -> Self {
        let cards = (0..44)
            .map(|id| {
                let owner = PlayerId(match id {
                    0..=15 => 0,
                    16..=31 => 1,
                    32..=39 => 2,
                    _ => 3,
                });
                CardSetup {
                    id: CardId(id),
                    owner,
                    controller: owner,
                    oracle_id: match id {
                        0 => ISAMARU,
                        1 => OX,
                        2 => BOUNCER,
                        3 => AZURE,
                        4 => SEEDS,
                        5 | 14 => SALVE,
                        6 | 19 | 20 => SPELL,
                        7 | 13 | 29 | 31 | 37 | 39 | 41..=43 => FOREST,
                        8 | 9 | 15 => PLAINS,
                        12 => SWAMP,
                        16 | 17 | 35 => BEARS,
                        18 | 27 => BOLT,
                        21 => COUNTERSPELL,
                        22 => STIFLE,
                        23 | 28 | 36 | 38 => MOUNTAIN,
                        32 | 40 => BLOOD_ARTIST,
                        33 => HEALER,
                        34 => FURNACE,
                        _ => ISLAND,
                    }
                    .into(),
                    zone: match id {
                        4..=6 | 18..=22 | 27 | 34 => Zone::Hand { owner },
                        14 | 15 | 30 | 31 | 38 | 39 | 42 | 43 => Zone::Library { owner },
                        _ => Zone::Battlefield,
                    },
                }
            })
            .collect();
        Self {
            initial: ScenarioInitial {
                turn_step: TurnStep::PrecombatMain,
                ..ScenarioInitial::default()
            },
            mode: Mode::Scenario,
            players: 4,
            profile: "four-seat-choices-v1".into(),
            commanders: vec![CardId(0)],
            cards,
        }
    }
    pub fn choices_furnace() -> Self {
        let mut c = Self::choices();
        c.profile = "four-seat-choices-furnace-v1".into();
        c.cards[34].zone = Zone::Battlefield;
        c
    }
    pub fn conditions() -> Self {
        let mut c = Self::choices();
        c.profile = "four-seat-conditions-v1".into();
        c.cards
            .retain(|c| !matches!(c.id, CardId(4) | CardId(5) | CardId(6)));
        c
    }
    pub fn conditions_nonempty() -> Self {
        let mut c = Self::choices();
        c.profile = "four-seat-conditions-nonempty-v1".into();
        c.cards.retain(|c| !matches!(c.id, CardId(4) | CardId(6)));
        c
    }
    pub fn conditions_borrowed() -> Self {
        let mut c = Self::conditions();
        c.profile = "four-seat-conditions-borrowed-v1".into();
        c.cards
            .iter_mut()
            .find(|c| c.id == CardId(2))
            .expect("exact Bouncer")
            .owner = PlayerId(2);
        c.cards
            .iter_mut()
            .find(|c| c.id == CardId(39))
            .expect("exact Forest")
            .zone = Zone::Hand { owner: PlayerId(2) };
        c
    }
    pub fn conditions_no_targets() -> Self {
        let mut c = Self::conditions();
        c.profile = "four-seat-conditions-no-targets-v1".into();
        c.commanders.clear();
        c.cards.retain(|c| {
            !matches!(
                c.id,
                CardId(0)
                    | CardId(1)
                    | CardId(3)
                    | CardId(16)
                    | CardId(17)
                    | CardId(32)
                    | CardId(33)
                    | CardId(35)
                    | CardId(40)
            )
        });
        c
    }
}
impl Game {
    pub(super) fn selected_program<'a>(
        &self,
        root: &'a SpellProgram,
        mode: Option<u32>,
    ) -> Result<&'a SpellProgram, String> {
        match mode {
            None => Ok(root),
            Some(mode) => root
                .modes
                .get(mode as usize)
                .map(|m| &m.program)
                .ok_or_else(|| "invalid frozen spell mode".into()),
        }
    }
    pub(super) fn modes_request(&self) -> Result<Option<Request>, String> {
        if !matches!(self.state.rules.frame, Frame::Modes { .. }) {
            return Ok(None);
        }
        let top = self.top()?;
        let root = self
            .definition(top.source.card)?
            .spell
            .as_ref()
            .ok_or("modal root missing")?;
        let candidates = root
            .modes
            .iter()
            .enumerate()
            .filter(|(_, m)| self.has_target_domain(&m.program, top.controller))
            .map(|(i, _)| i as u32)
            .collect();
        Ok(Some(Request::Modes {
            decision_id: self.state.rules.revision,
            player: top.controller,
            candidates,
        }))
    }
    pub(super) fn validate_mode(&self, player: PlayerId, mode: u32) -> Result<(), String> {
        let Some(Request::Modes {
            player: p,
            candidates,
            ..
        }) = self.modes_request()?
        else {
            return Err("no mode choice".into());
        };
        if p != player || !candidates.contains(&mode) {
            return Err("invalid mode choice".into());
        }
        Ok(())
    }
    pub(super) fn bind_mode(&mut self, cast: Casting) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing mode input")?;
        let Response::Modes { mode } = input.response else {
            return Err("mode response invariant".into());
        };
        self.validate_mode(input.player, mode)?;
        let top = self
            .state
            .rules
            .stack
            .last_mut()
            .ok_or("missing modal spell")?;
        if top.mode.is_some() {
            return Err("mode already selected".into());
        }
        top.mode = Some(mode);
        let stack_id = top.id;
        self.record_event(Event::ModeSelected {
            stack_id,
            player: input.player,
            mode,
        })?;
        self.state.rules.frame = self.spell_announcement_frame(cast)?;
        Ok(())
    }
    // CR603.4 checks twice; CR608.2h/113.7a use current expected-zone
    // information or the last battlefield incarnation (CR400.7,110.5d).
    // The existing per-invocation ObjectBefore is refreshed at departure by
    // retain_source_lki, including taps AFTER collection. Its lifetime is
    // bounded by outstanding stack/pending entries, independent of tracing.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn check_intervening(
        &self,
        condition: &InterveningCondition,
        controller: PlayerId,
        source: ObjectId,
        last_known: &ObjectBefore,
    ) -> Result<InterveningCheck, String> {
        if !self.is_live(controller) {
            return Err("intervening condition requested after controller departure".into());
        }
        Ok(match condition {
            InterveningCondition::ControllerHandEmpty => {
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
            InterveningCondition::SourceStatus { status } => {
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
                    status: *status,
                    tapped,
                    information,
                    passed: status.matches(tapped),
                }
            }
            InterveningCondition::ControllerHasCreatureWithPower { minimum_power } => {
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
    pub(super) fn check_resolution_condition(
        &mut self,
        stack: &StackInstance,
    ) -> Result<Option<InterveningCheck>, String> {
        let StackKind::Triggered { ability, .. } = &stack.kind else {
            return Ok(None);
        };
        let definition = self.effective_trigger(stack.source, *ability)?;
        let Some(condition) = definition.intervening_if.as_ref() else {
            return Ok(None);
        };
        let check = self.check_intervening(
            condition,
            stack.controller,
            stack.source,
            stack
                .source_facts
                .as_ref()
                .ok_or("missing trigger source facts")?,
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
        if let Frame::Modes { cast } = &self.state.rules.frame {
            let top = self.top()?;
            let root = self
                .definition(top.source.card)?
                .spell
                .as_ref()
                .ok_or("missing modal announcement definition")?;
            if !cast.origin.is_spell()
                || root.modes.is_empty()
                || top.mode.is_some()
                || top.paid.is_some()
                || self.bound_target_count(top) != 0
            {
                return Err("modal announcement invariant".into());
            }
        }
        for entry in &self.state.rules.stack {
            self.validate_mode_binding(entry)?;
        }
        if let Frame::Resolve { frame }
        | Frame::ResolutionChoice { frame, .. }
        | Frame::LandEntry {
            origin: EntryOrigin::Search { frame },
            ..
        } = &self.state.rules.frame
        {
            self.validate_mode_binding(&frame.spell)?;
            self.validate_condition_cache(frame)?;
        }
        Ok(())
    }
    pub(super) fn validate_mode_binding(&self, entry: &StackInstance) -> Result<(), String> {
        let root = match entry.kind {
            StackKind::Spell => self
                .definition(entry.source.card)?
                .spell
                .as_ref()
                .ok_or("missing mode root")?,
            _ => {
                if entry.mode.is_some() {
                    return Err("nonspell selected mode".into());
                }
                return Ok(());
            }
        };
        if root.modes.is_empty() {
            if entry.mode.is_some() {
                return Err("nonmodal spell selected mode".into());
            }
        } else {
            match entry.mode {
                Some(mode) if (mode as usize) < root.modes.len() => {}
                None if matches!(&self.state.rules.frame,Frame::Modes{cast} if cast.stack_id==entry.id) =>
                    {}
                _ => return Err("missing/invalid modal selection".into()),
            }
        }
        Ok(())
    }
    pub(super) fn validate_condition_cache(&self, frame: &Resolving) -> Result<(), String> {
        let definition = match frame.spell.kind {
            StackKind::Triggered { ability, .. } => {
                Some(self.effective_trigger(frame.spell.source, ability)?)
            }
            _ => None,
        };
        let expected = match &definition {
            Some(definition) => definition.intervening_if.as_ref(),
            _ => None,
        };
        match (expected, &frame.condition_check) {
            (None, None) => Ok(()),
            (Some(condition), Some(check))
                if check.validates(
                    condition,
                    frame.spell.controller,
                    frame.spell.source,
                    self.prepared.config.owned_limit() as u32,
                ) =>
            {
                Ok(())
            }
            _ => Err("invalid resolution condition cache".into()),
        }
    }
}
impl Game {
    pub(super) fn validate_match_condition(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        check: Option<&InterveningCheck>,
    ) -> Result<(), String> {
        let definition = self.effective_trigger(source, ability)?;
        let expected = definition.intervening_if.as_ref();
        match (expected, check) {
            (None, None) => Ok(()),
            (Some(condition), Some(check))
                if check.validates(
                    condition,
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
impl Game {
    pub(super) fn origin_programs<'a>(
        &'a self,
        origin: &EffectOrigin,
    ) -> Result<Vec<&'a SpellProgram>, String> {
        let d = self.definition(origin.source.card)?;
        if let Some(mode) = origin.mode {
            let root = d.spell.as_ref().ok_or("mode on nonspell effect origin")?;
            return Ok(vec![self.selected_program(root, Some(mode))?]);
        }
        Ok(d.spell
            .iter()
            .filter(|p| p.modes.is_empty())
            .chain(d.activated.iter().map(|a| &a.program))
            .chain(d.triggered.iter().map(|a| &a.program))
            .collect())
    }
    pub(super) fn shield_limit(&self, effect: &ResolvedEffect) -> Result<Quantity, String> {
        for p in self.origin_programs(&effect.origin)? {
            if let Some(Instruction::PreventNextDamage {
                amount,
                source_block,
                ..
            }) = p.instructions.get(effect.origin.cursor as usize)
                && *source_block == effect.origin.source_block
            {
                return Ok(amount.clone());
            }
        }
        Err("missing shield origin quantity".into())
    }
}

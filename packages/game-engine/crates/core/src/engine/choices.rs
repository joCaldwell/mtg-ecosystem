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
        condition: &InterveningCondition,
        controller: PlayerId,
        source: ObjectId,
        hand_limit: u32,
    ) -> bool {
        match (condition, self) {
            (
                InterveningCondition::SourceCastByController,
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
// Only canonical indices enter the snapshot and replay identity. The selected
// instruction sequence is derived afresh from the build-fenced catalog after restore.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BoundModeSubset {
    pub(super) indices: Vec<u32>,
    #[serde(skip)]
    compiled: std::sync::OnceLock<Result<SpellProgram, String>>,
}
impl PartialEq for BoundModeSubset {
    fn eq(&self, other: &Self) -> bool {
        self.indices == other.indices
    }
}
impl Eq for BoundModeSubset {}
impl BoundModeSubset {
    fn build(root: &SpellProgram, chosen: &[u32]) -> Result<Self, String> {
        let indices = mtg_contracts::modal_targets::ModeCount::for_root(root)
            .ok_or("not a subset selection")?
            .selected(root.modes.len(), chosen)?;
        let plan = Self {
            indices,
            compiled: Default::default(),
        };
        plan.validate(root)?;
        Ok(plan)
    }
    fn validate(&self, root: &SpellProgram) -> Result<(), String> {
        let count = mtg_contracts::modal_targets::ModeCount::for_root(root)
            .ok_or("invalid mode subset source")?;
        if count.selected(root.modes.len(), &self.indices)? != self.indices {
            return Err("selected modes out of printed order".into());
        }
        Ok(())
    }
    pub(super) fn program(&self, root: &SpellProgram) -> Result<&SpellProgram, String> {
        self.compiled
            .get_or_init(|| {
                self.validate(root)?;
                let indices = &self.indices;
                let mut instructions = Vec::with_capacity(indices.len());
                let mut targets = vec![];
                for index in indices {
                    let child = &root
                        .modes
                        .get(*index as usize)
                        .ok_or("unknown printed mode")?
                        .program;
                    let [instruction] = child.instructions.as_slice() else {
                        return Err("unexpected mode instructions".into());
                    };
                    let offset = u32::try_from(targets.len())
                        .map_err(|_| "mode target capacity exceeded")?;
                    let mut instruction = instruction.clone();
                    match (&mut instruction, child.targets.as_slice()) {
                        (_, []) => {}
                        (Instruction::ShufflePlayerGraveyard { .. }, _)
                            if mtg_contracts::graveyard_shuffle::offered_by(child).is_some() => {}
                        (
                            Instruction::GainLife {
                                recipient: LifeRecipient::Target { target_slot },
                                ..
                            }
                            | Instruction::LoseLife {
                                recipient: LifeRecipient::Target { target_slot },
                                ..
                            }
                            | Instruction::CounterStackObject { target_slot, .. }
                            | Instruction::ReturnToOwnerHand { target_slot, .. }
                            | Instruction::PutPlusOneCounters {
                                on: mtg_contracts::CounterRecipient::Target { target_slot },
                                ..
                            },
                            [target],
                        ) if *target_slot == 0 && target.slot == 0 => {
                            *target_slot = offset;
                        }
                        _ => return Err("unsupported targeted mode effect".into()),
                    }
                    for (local, target) in child.targets.iter().enumerate() {
                        if usize::try_from(target.slot).ok() != Some(local) {
                            return Err("noncontiguous child target occurrence".into());
                        }
                        let mut target = target.clone();
                        target.slot = offset
                            .checked_add(target.slot)
                            .ok_or("mode target capacity exceeded")?;
                        if let TargetDomain::PlayerGraveyardCards { owner_slot, .. } =
                            &mut target.domain
                        {
                            *owner_slot = offset
                                .checked_add(*owner_slot)
                                .ok_or("mode owner occurrence exceeds capacity")?;
                        }
                        targets.push(target);
                    }
                    instructions.push(instruction);
                }
                let program = SpellProgram {
                    timing: root.timing,
                    functioning_zone: FunctioningZone::Stack,
                    resolution: SpellResolution::Instructions,
                    targets,
                    modes: vec![],
                    instructions,
                    additional_sacrifice: None,
                    optional_additional_costs: vec![],
                };
                Ok(program)
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}
impl Game {
    // Fixed heterogeneous modes announce in printed order. The typed target
    // vectors are the receipt, including a player with zero graveyard cards.
    pub(super) fn mixed_mode_children<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Result<Option<Vec<&'a SpellProgram>>, String> {
        let Some(plan) = &entry.mode_subset else {
            return Ok(None);
        };
        let root = self.root_program_for(entry)?;
        let children = plan
            .indices
            .iter()
            .map(|index| {
                root.modes
                    .get(*index as usize)
                    .map(|m| &m.program)
                    .ok_or_else(|| "missing selected mode".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(children
            .iter()
            .any(|p| {
                p.targets.iter().any(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::CreatureOrPlaneswalker
                            | TargetDomain::StackSpell
                            | TargetDomain::PlayerGraveyardCards { .. }
                            | TargetDomain::BattlefieldCreature
                    )
                })
            })
            .then_some(children))
    }

    pub(super) fn next_mode_target<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Result<Option<&'a SpellProgram>, String> {
        let Some(children) = self.mixed_mode_children(entry)? else {
            return Ok(None);
        };
        let mut objects = entry.targets.len();
        let mut stack = entry.stack_targets.len();
        let mut group = usize::from(!entry.entity_targets.is_empty());
        for child in children {
            let used = match child.targets.first().map(|t| t.domain) {
                None => continue,
                Some(TargetDomain::BattlefieldCreature | TargetDomain::CreatureOrPlaneswalker) => {
                    &mut objects
                }
                Some(TargetDomain::StackSpell) => &mut stack,
                Some(TargetDomain::AnyPlayer)
                    if mtg_contracts::graveyard_shuffle::offered_by(child).is_some() =>
                {
                    &mut group
                }
                _ => return Err("unsupported selected-mode target group".into()),
            };
            if *used == 0 {
                return Ok(Some(child));
            }
            *used -= 1;
        }
        Ok(None)
    }

    pub(super) fn valid_mixed_target_receipt(
        &self,
        entry: &StackInstance,
        complete: bool,
    ) -> Result<bool, String> {
        let Some(children) = self.mixed_mode_children(entry)? else {
            return Ok(false);
        };
        if !entry.player_targets.is_empty() {
            return Ok(false);
        }
        let mut objects = 0;
        let mut stack = 0;
        let mut entities = 0;
        for child in children {
            let bound = match child.targets.first().map(|t| t.domain) {
                None => continue,
                Some(TargetDomain::CreatureOrPlaneswalker | TargetDomain::BattlefieldCreature) => {
                    objects += 1;
                    objects <= entry.targets.len()
                }
                Some(TargetDomain::StackSpell) => {
                    stack += 1;
                    stack <= entry.stack_targets.len()
                }
                Some(TargetDomain::AnyPlayer) => {
                    let Some(maximum) = mtg_contracts::graveyard_shuffle::offered_by(child) else {
                        return Ok(false);
                    };
                    entities += 1;
                    entities == 1 && self.graveyard_group_structure(&entry.entity_targets, maximum)
                }
                _ => return Ok(false),
            };
            if !bound {
                return Ok(!complete
                    && entry.targets.len() <= objects
                    && entry.stack_targets.len() <= stack
                    && entry.entity_targets.is_empty());
            }
        }
        Ok(objects == entry.targets.len()
            && stack == entry.stack_targets.len()
            && entities == usize::from(!entry.entity_targets.is_empty()))
    }
    pub(super) fn continue_spell_targeting(&mut self, cast: Casting) -> Result<Frame, String> {
        if self.mixed_mode_children(self.top()?)?.is_some() {
            self.spell_announcement_frame(cast)
        } else {
            self.announcement_mana_frame(cast)
        }
    }
    /// Stack targets and permanent targets retain separate typed vectors. A
    /// selected mode operand is numbered in the complete printed occurrence list.
    pub(super) fn selected_storage_index(
        &self,
        entry: &StackInstance,
        slot: usize,
        matches: impl Fn(TargetDomain) -> bool,
    ) -> Option<usize> {
        if entry.mode_subset.is_none() {
            return Some(slot);
        }
        let targets = &self.program_for(entry).ok()?.targets;
        matches(targets.get(slot)?.domain).then(|| {
            targets[..slot]
                .iter()
                .filter(|spec| matches(spec.domain))
                .count()
        })
    }

    pub(super) fn bound_object_target<'a>(
        &self,
        entry: &'a StackInstance,
        slot: usize,
    ) -> Option<&'a ObjectId> {
        let index = self.selected_storage_index(entry, slot, |d| {
            matches!(
                d,
                TargetDomain::CreatureOrPlaneswalker
                    | TargetDomain::BattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature
                    | TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::OwnGraveyardCard
                    | TargetDomain::OwnGraveyardCreatureCard
            )
        })?;
        entry.targets.get(index)
    }

    pub(super) fn bound_stack_target<'a>(
        &self,
        entry: &'a StackInstance,
        slot: usize,
    ) -> Option<&'a StackId> {
        let index = self.selected_storage_index(entry, slot, |d| {
            matches!(
                d,
                TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility
            )
        })?;
        entry.stack_targets.get(index)
    }
    pub(super) fn authorize_mode_subset(
        &self,
        player: PlayerId,
        indices: &[u32],
    ) -> Result<BoundModeSubset, String> {
        let top = self.top()?;
        if !matches!(&self.state.rules.frame,Frame::Modes{cast} if cast.stack_id==top.id)
            || !top.kind.is_spell()
            || top.controller != player
            || top.mode.is_some()
            || top.mode_subset.is_some()
        {
            return Err("unauthorized mode subset".into());
        }
        let Some(Request::Modes { candidates, .. }) = self.modes_request()? else {
            return Err("missing printed mode choices".into());
        };
        if indices.iter().any(|index| !candidates.contains(index)) {
            return Err("a selected mode has no legal target".into());
        }
        BoundModeSubset::build(self.root_program_for(top)?, indices)
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
        let root = self.root_program_for(top)?;
        // CR 601.2b, 700.2a, 903.3: choose both at announcement using the
        // current controller of a physically designated battlefield commander.
        let may_choose_both = !top.kind.is_spell()
            || !akromas_will_program(root)
            || self.state.rules.commanders.records.keys().any(|card| {
                self.state.rules.objects.get(card).is_some_and(|object| {
                    object.zone == Zone::Battlefield && object.controller == top.controller
                })
            });
        let source = self.targeting_characteristics(top)?;
        let candidates = root
            .modes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 2 || may_choose_both)
            .filter(|(_, m)| self.has_target_domain(&m.program, top.controller, &source))
            .map(|(i, _)| i as u32)
            .collect();
        Ok(Some(Request::Modes {
            decision_id: self.state.rules.revision,
            player: top.controller,
            one_or_more: root.resolution == SpellResolution::OneOrMoreModes,
            exactly: match root.resolution {
                SpellResolution::ChooseModes { exactly } => Some(exactly),
                _ => None,
            },
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
        if matches!(self.root_program_for(self.top()?)?.resolution,SpellResolution::ChooseModes{exactly} if exactly!=1)
            || p != player
            || !candidates.contains(&mode)
        {
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
        let root = self.root_program_for(self.top()?)?;
        if mtg_contracts::modal_targets::ModeCount::for_root(root).is_some() {
            let modes = match input.response {
                Response::ModeSubset { modes } => modes,
                Response::Modes { mode } => vec![mode],
                _ => return Err("expected mode subset".into()),
            };
            let selected = self.authorize_mode_subset(input.player, &modes)?;
            let indices = selected.indices.clone();
            let top = self
                .state
                .rules
                .stack
                .last_mut()
                .ok_or("missing modal spell")?;
            top.mode_subset = Some(selected);
            let stack_id = top.id;
            self.record_event(Event::ModeSubsetSelected {
                stack_id,
                player: input.player,
                modes: indices,
            })?;
        } else {
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
        }
        if cast.origin.is_spell() {
            self.state.rules.frame = self.spell_announcement_frame(cast)?;
        } else {
            match self.program()?.targets.first().map(|t| t.domain) {
                Some(TargetDomain::AnyDamageTarget) => {
                    self.state.rules.frame = Frame::EntityTargets { cast }
                }
                Some(TargetDomain::PlayerOrPlaneswalker) => {
                    self.state.rules.frame = Frame::PlayerTargets { cast }
                }
                Some(TargetDomain::BattlefieldCreature) => {
                    self.state.rules.frame = Frame::Targets { cast }
                }
                None => self.continue_activation_after_targets(cast, input.decision_id)?,
                Some(_) => return Err("unsupported activated mode target".into()),
            }
        }
        Ok(())
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
            && matches!(&self.state.rules.frame, Frame::Resolve { frame } if frame.stack_id == *stack_id && frame.spell.source == *source)
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
        condition: &InterveningCondition,
        controller: PlayerId,
        source: ObjectId,
        last_known: &ObjectBefore,
        cast: Option<EntryCast>,
    ) -> Result<InterveningCheck, String> {
        if !self.is_live(controller) {
            return Err("intervening condition requested after controller departure".into());
        }
        Ok(match condition {
            InterveningCondition::SourceCastByController => {
                InterveningCheck::SourceCastByController {
                    entrant: source,
                    controller,
                    cast,
                }
            }
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
        let StackKind::Triggered {
            ability,
            match_condition,
            ..
        } = &stack.kind
        else {
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
        if let Frame::Modes { cast } = &self.state.rules.frame {
            let top = self.top()?;
            let root = self.root_program_for(top)?;
            if top.kind.is_spell() != cast.origin.is_spell()
                || cast.stack_id != top.id
                || root.modes.is_empty()
                || top.mode.is_some()
                || top.mode_subset.is_some()
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
        if let Some(subset) = &entry.mode_subset {
            let root = self.root_program_for(entry)?;
            if !entry.kind.is_spell() || entry.mode.is_some() || subset.validate(root).is_err() {
                return Err("invalid reconstructed subset program".into());
            }
            return Ok(());
        }
        if matches!(entry.kind, StackKind::Triggered { .. }) {
            return if entry.mode.is_none() {
                Ok(())
            } else {
                Err("unexpected triggered mode".into())
            };
        }
        let root = self.root_program_for(entry)?;
        if root.modes.is_empty() {
            if entry.mode.is_some() {
                return Err("nonmodal spell selected mode".into());
            }
        } else {
            match entry.mode {
                Some(mode)
                    if mtg_contracts::modal_targets::ModeCount::for_root(root).is_none()
                        && (mode as usize) < root.modes.len() => {}
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
        if expected == Some(&InterveningCondition::SourceCastByController)
            && !matches!(&frame.spell.kind, StackKind::Triggered { match_condition, .. }
                if match_condition == &frame.condition_check)
        {
            return Err("cast-entry resolution lost its match evidence".into());
        }
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
        if let Some(index) = origin.activated_ability {
            let root = &d
                .activated
                .get(index as usize)
                .ok_or("missing activated effect origin")?
                .program;
            if origin.mode.is_some() == root.modes.is_empty() {
                return Err("invalid activated effect mode".into());
            }
            return Ok(vec![self.selected_program(root, origin.mode)?]);
        }
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

#[cfg(test)]
mod fixed_target_mode_tests {
    use super::*;
    use crate::engine::equipped_indestructible_tests::{
        answer, object, pay_and_resolve, pay_to_priority, prepared, recover, request,
    };
    #[test]
    fn paid_mixed_target_modes_place_counters_then_shuffle_opposing_graveyard_and_replay() {
        let result = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            include_str!(
                "../../../../tests/fixtures/ramos/9c0f7cdc-9095-40ae-bf83-75a5ffda72b9.json"
            ),
        );
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        let mut p = prepared(
            &[
                ("Forest", true),
                ("Forest", false),
                ("Forest", false),
                ("Island", false),
                ("Grizzly Bears", false),
                ("Forest", false),
            ],
            false,
        );
        let owner = PlayerId(1);
        p.config.cards[0].oracle_id = definition.oracle_id.clone();
        p.config.cards[4].owner = PlayerId(2);
        p.config.cards[4].controller = PlayerId(2);
        p.config.cards[5].owner = owner;
        p.config.cards[5].controller = owner;
        p.config.cards[5].zone = Zone::Graveyard { owner };
        let c = Arc::make_mut(&mut p.catalog);
        c.definitions.push(*definition);
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        c.validate().unwrap();
        p.catalog_digest = digest(&*p.catalog).unwrap();
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        request(&mut g, 1);
        let card = object(&g, 0).card;
        let creature = object(&g, 4);
        let buried = object(&g, 5);
        let mut departed = g.fork();
        answer(&mut departed, 1, Response::Cast { card });
        answer(&mut departed, 1, Response::ModeSubset { modes: vec![0, 3] });
        answer(
            &mut departed,
            1,
            Response::Targets {
                objects: vec![creature],
            },
        );
        answer(
            &mut departed,
            1,
            Response::EntityTargets {
                entities: vec![TargetEntity::Player { player: owner }],
            },
        );
        pay_to_priority(&mut departed, 1, vec![]);
        departed.concede(PlayerId(2)).unwrap();
        let mut recovered = false;
        for _ in 0..500 {
            if let Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Shuffle { player, .. },
            } = &departed.state.rules.frame
            {
                assert_eq!(*player, owner);
                assert_eq!(frame.legal_targets, [false, true]);
                recover(&departed);
                recovered = true;
                break;
            }
            match departed.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(r @ Request::Priority { .. }) => departed
                    .respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap(),
                other => panic!("expected shuffle after target departure: {other:?}"),
            }
        }
        assert!(recovered, "missing shuffle continuation");
        answer(&mut g, 1, Response::Cast { card });
        answer(&mut g, 1, Response::ModeSubset { modes: vec![3, 2] });
        assert!(
            matches!(request(&mut g,1),Request::Targets{candidates,..} if candidates.contains(&creature))
        );
        answer(
            &mut g,
            1,
            Response::Targets {
                objects: vec![creature],
            },
        );
        assert!(matches!(request(&mut g, 1), Request::EntityTargets { .. }));
        recover(&g);
        let mut forged = g.fork();
        let Frame::EntityTargets { cast } = &forged.state.rules.frame else {
            panic!("expected dependent player selection")
        };
        forged.state.rules.frame = Frame::Targets { cast: cast.clone() };
        assert!(
            Game::restore(
                forged.prepared.as_ref().clone(),
                &forged.snapshot().unwrap()
            )
            .is_err()
        );
        for select_card in [false, true] {
            let mut branch = g.fork();
            let mut entities = vec![TargetEntity::Player { player: owner }];
            if select_card {
                entities.push(TargetEntity::Object { object: buried });
            }
            answer(&mut branch, 1, Response::EntityTargets { entities });
            pay_and_resolve(&mut branch, 1, vec![]);
            assert_eq!(
                branch.current(creature).unwrap().plus_one_counters,
                2.into()
            );
            assert_eq!(
                branch.zone(object(&branch, 5)),
                Some(if select_card {
                    Zone::Library { owner }
                } else {
                    Zone::Graveyard { owner }
                })
            );
            recover(&branch);
        }
    }

    #[test]
    fn paid_exact_two_targeted_modes_bind_distinct_occurrences_and_replay_print_order() {
        let raw = include_str!("../../../../tests/fixtures/synthetic-fixed-target-modes.json");
        let result = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        );
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        let mut p = prepared(&[("Forest", true), ("Forest", false)], false);
        p.config.cards[0].oracle_id = definition.oracle_id.clone();
        let c = Arc::make_mut(&mut p.catalog);
        c.definitions.push(*definition);
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        c.validate().unwrap();
        p.catalog_digest = digest(&*p.catalog).unwrap();
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        request(&mut g, 1);
        let card = object(&g, 0).card;
        answer(&mut g, 1, Response::Cast { card });
        let modes = request(&mut g, 1);
        assert!(matches!(
            modes,
            Request::Modes {
                exactly: Some(2),
                ..
            }
        ));
        let caster = modes.player();
        let revision = modes.decision_id();
        for ids in [vec![0], vec![1, 1], vec![0, 1, 2]] {
            assert!(
                g.respond(caster, revision, Response::ModeSubset { modes: ids })
                    .is_err()
            );
        }
        answer(&mut g, 1, Response::ModeSubset { modes: vec![1, 0] });
        let r = request(&mut g, 1);
        assert!(
            matches!(&r,Request::PlayerTargets{occurrences,..} if occurrences.iter().map(|s|s.slot).collect::<Vec<_>>()==[0,1])
        );
        let mut split = g.fork();
        answer(
            &mut split,
            1,
            Response::PlayerTargets {
                players: vec![PlayerId(1), PlayerId(2)],
            },
        );
        loop {
            let r = request(&mut split, 1);
            let response = match &r {
                Request::Priority { .. } => break,
                Request::Mana { cost, .. } if split.payment_pool(caster, cost).is_ok() => {
                    Response::FinishMana { undo_mana: false }
                }
                Request::Mana { .. } => Response::ActivateMana {
                    object: object(&split, 1),
                },
                Request::SpendMana { .. } => Response::SpendMana {
                    spending: BTreeMap::from([(Color::G, 1.into())]),
                },
                other => panic!("unexpected paid mode request {other:?}"),
            };
            answer(&mut split, 1, response);
        }
        let id = split.top().unwrap().id;
        recover(&split);
        for all in [false, true] {
            let mut branch = split.fork();
            branch.concede(PlayerId(1)).unwrap();
            if all {
                branch.concede(PlayerId(2)).unwrap();
            }
            while !branch.state.rules.stack.is_empty() {
                let r = request(&mut branch, 1);
                if branch.state.rules.stack.is_empty() {
                    break;
                }
                assert!(matches!(r, Request::Priority { .. }));
                answer(&mut branch, 1, Response::Pass);
            }
            assert!(branch.events().iter().any(|e|matches!(e,Event::SpellCompleted{stack_id,resolved} if *stack_id==id && *resolved==!all)));
            if !all {
                assert_eq!(branch.state.rules.players[2].life, 38.into());
            }
            recover(&branch);
        }
        // Reusing the same player across distinct occurrences is legal.
        answer(
            &mut g,
            1,
            Response::PlayerTargets {
                players: vec![PlayerId(1), PlayerId(1)],
            },
        );
        recover(&g);
        pay_and_resolve(&mut g, 1, vec![]);
        assert_eq!(g.state.rules.players[1].life, 41.into());
        recover(&g);
    }
}

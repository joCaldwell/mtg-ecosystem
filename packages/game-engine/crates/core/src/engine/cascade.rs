//! CR 107.3b, 117.2e/5, 118.9, 601.2, 608.2g/h, 702.85a, 733.1, 903.9b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! One resolving owner travels through Casting and nested ManaAbility.resume.
//! No global parent slot, priority permission, recursive child resolution or
//! whole-library shuffle participates in this continuation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlternativeCost {
    Bestow,
    WithoutManaCost,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastPermission {
    pub(super) object: ObjectId,
    pub(super) caster: PlayerId,
    owner: StackId,
    cursor: u32,
    source_block: u32,
    pub(super) alternative_cost: AlternativeCost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resulting_mana_value_below: Option<Quantity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "return_to", rename_all = "snake_case", deny_unknown_fields)]
pub enum CastCompletion {
    Priority,
    HandTrigger {
        frame: Box<Resolving>,
    },
    Resolution {
        permission: CastPermission,
        progress: CastOwner,
    },
}
/// The original cascade progress remains its serialized resolution owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum CastOwner {
    LinkedPlay {
        progress: Box<LinkedPlayProgress>,
        object: ObjectId,
        receipt: HideawayLink,
    },
    Cascade(Box<CascadeProgress>),
    RevealAndCast(Box<RevealAndCastProgress>),
}
impl CastOwner {
    pub(super) fn permission(&self, game: &RulesTxn) -> Result<CastPermission, String> {
        match self {
            Self::LinkedPlay {
                progress, object, ..
            } => progress.permission(game, *object),
            Self::Cascade(progress) => progress.permission(game),
            Self::RevealAndCast(progress) => progress.permission(game),
        }
    }
    pub(super) fn resume(self) -> Frame {
        match self {
            Self::LinkedPlay { progress, .. } => Frame::PlayHideawayLinked { progress },
            Self::Cascade(progress) => Frame::Cascade { progress },
            Self::RevealAndCast(progress) => Frame::RevealAndCast { progress },
        }
    }
}
impl CastPermission {
    pub(super) fn from_instruction(
        game: &RulesTxn,
        frame: &Resolving,
        source_block: u32,
        object: ObjectId,
        ceiling: Option<Quantity>,
    ) -> Result<Self, String> {
        Ok(Self {
            caster: game.entry(frame.stack_id)?.controller,
            object,
            owner: frame.stack_id,
            cursor: frame.cursor,
            source_block,
            alternative_cost: AlternativeCost::WithoutManaCost,
            resulting_mana_value_below: ceiling,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CascadeStage {
    Scan,
    LandOffer { provider: ObjectId },
    Offer,
    Randomize,
    Bottom,
    Finish,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CascadeLandChoice {
    provider: ObjectId,
    entered: Option<ObjectId>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CascadeProgress {
    pub frame: Resolving,
    pub(super) source_block: u32,
    threshold: Quantity,
    pub stage: CascadeStage,
    exiled: Vec<ObjectId>,
    hit: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    land_choices: Vec<CascadeLandChoice>,
    pub(super) child: Option<StackId>,
    remainder: Vec<ObjectId>,
    permutation: Vec<u32>,
    random_index: u32,
    commander_cursor: u32,
    redirected: Vec<ObjectId>,
}
impl CascadeProgress {
    fn caster(&self, game: &RulesTxn) -> Result<PlayerId, String> {
        Ok(game.entry(self.frame.stack_id)?.controller)
    }
    fn permission(&self, game: &RulesTxn) -> Result<CastPermission, String> {
        if self.stage != CascadeStage::Offer || self.child.is_some() {
            return Err("cascade permission outside its offer".into());
        }
        CastPermission::from_instruction(
            game,
            &self.frame,
            self.source_block,
            self.hit.ok_or("cascade offer without hit")?,
            Some(self.threshold.clone()),
        )
    }
}
impl RulesTxn {
    fn cascade_threshold(&self, frame: &Resolving) -> Result<Quantity, String> {
        let StackKind::Triggered {
            witness: TriggerEventWitness::SpellCast { cast },
            ..
        } = &self.entry(frame.stack_id)?.kind
        else {
            return Err("cascade requires its real self-cast occurrence".into());
        };
        if cast.spell.object != self.entry(frame.stack_id)?.source
            || cast.caster != self.entry(frame.stack_id)?.controller
        {
            return Err("cascade self source identity".into());
        }
        // Supported immutable normal faces: current exact stack characteristics
        // or the cast witness's bounded last-known spell cost/X. Paid costs are
        // deliberately absent from this calculation (CR202.3/608.2h).
        if let Some(spell) = self
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.id == cast.stack_id && s.source == cast.spell.object)
        {
            return self.stack_mana_value(spell);
        }
        let mut value = cast.spell.characteristics.printed_mana_value();
        if let Some(x) = &cast.x {
            value.add(x);
        }
        Ok(value)
    }

    fn nonstack_mana_value(&self, object: ObjectId) -> Result<Quantity, String> {
        Ok(self
            .copiable_definition(object)?
            .characteristics
            .printed_mana_value())
    }
    fn cascade_hit(&self, object: ObjectId, threshold: &Quantity) -> Result<bool, String> {
        Ok(!self
            .copiable_definition(object)?
            .characteristics
            .has_type(CardKind::Land)
            && self.nonstack_mana_value(object)? < *threshold)
    }
    pub(super) fn resolution_offer_legal(
        &self,
        permission: &CastPermission,
    ) -> Result<bool, String> {
        if self.current(permission.object)?.zone != Zone::Exile
            || self.state.rules.cards[&permission.object.card].owner != permission.caster
            || self
                .copiable_definition(permission.object)?
                .characteristics
                .has_type(CardKind::Land)
            || match &permission.resulting_mana_value_below {
                Some(limit) => self.nonstack_mana_value(permission.object)? >= *limit,
                None => false,
            }
            || self.top()?.id != permission.owner
        {
            return Err("resolution cast permission invariant".into());
        }
        self.resolution_spell_proposable(permission.caster, permission.object)
    }
    pub(super) fn resolution_spell_proposable(
        &self,
        caster: PlayerId,
        object: ObjectId,
    ) -> Result<bool, String> {
        if !self.is_live(caster)
            || self.cast_is_prohibited(caster)?
            || self
                .definition(object.card)?
                .characteristics
                .has_type(CardKind::Land)
        {
            return Ok(false);
        }
        let definition = self.copiable_definition(object)?;
        let program = definition
            .spell
            .as_ref()
            .ok_or("revealed nonland is not a supported spell")?;
        Ok(self
            .spell_proposal_obstacle(program, caster, &definition.characteristics)
            .is_none())
    }
    pub(crate) fn resulting_permission_matches(
        &self,
        permission: &CastPermission,
        stack: &StackInstance,
    ) -> Result<bool, String> {
        Ok(stack.kind.is_spell()
            && stack.cast_from == Zone::Exile
            && stack.controller == permission.caster
            && stack.source.card == permission.object.card
            && permission.object.incarnation.checked_add(1) == Some(stack.source.incarnation)
            && stack.alternative_cost == Some(permission.alternative_cost)
            && match &permission.resulting_mana_value_below {
                Some(limit) => self.stack_mana_value(stack)? < *limit,
                None => true,
            })
    }
    pub(super) fn announcement_mana_frame(&mut self, mut cast: Casting) -> Result<Frame, String> {
        self.lock_spell_cost()?;
        let stack = self.top()?;
        if !matches!(cast.completion, CastCompletion::Priority)
            && !self.has_selected_additional_mana(stack)
            && stack.total_cost.generic == Quantity::zero()
            && stack.total_cost.colored.is_empty()
        {
            // No mana-cost payment exists for the free alternative. Mandatory
            // nonmana additionals still use their ordinary payment authority.
            // No unrestricted 601.2g mana window is granted.
            cast.spending = Some(BTreeMap::new());
            Ok(if self.invocation_sacrifice(stack).is_some() {
                Frame::SacrificeCost { cast }
            } else {
                Frame::Payment { cast }
            })
        } else {
            Ok(Frame::Mana { cast })
        }
    }
    // CR 702.85b, 608.2c, 110.2a, 614.12. The provider modifies the
    // resolving cascade after its scan, even when there is no castable card.
    // https://magic.wizards.com/en/news/feature/commander-legends-release-notes-2020-11-06
    fn cascade_land_ability(
        &self,
        object: &Object,
        controller: PlayerId,
    ) -> Result<Option<u32>, String> {
        if object.zone != Zone::Battlefield
            || object.controller != controller
            || !self.is_live(controller)
        {
            return Ok(None);
        }
        self.copiable_definition(object.id)?
            .static_abilities
            .iter()
            .position(|a| {
                a.functioning_zone == FunctioningZone::Battlefield
                    && matches!(a.effect, StaticEffect::CascadeLandEntry)
            })
            .map(|i| u32::try_from(i).map_err(|_| "static ability index limit".into()))
            .transpose()
    }
    pub(super) fn cascade_land_source(
        &self,
        progress: &CascadeProgress,
    ) -> Result<Option<EffectKey>, String> {
        let CascadeStage::LandOffer { provider } = progress.stage else {
            return Ok(None);
        };
        let Ok(object) = self.current(provider) else {
            return Ok(None);
        };
        Ok(self
            .cascade_land_ability(object, progress.caster(self)?)?
            .map(|ability| EffectKey::Static {
                source: provider,
                ability,
            }))
    }
    fn cascade_exiled_land(&self, object: ObjectId) -> Result<bool, String> {
        Ok(self.current(object).is_ok_and(|o| o.zone == Zone::Exile)
            && self
                .copiable_definition(object)?
                .characteristics
                .has_type(CardKind::Land))
    }
    pub(super) fn cascade_land_available(
        &self,
        progress: &CascadeProgress,
        object: ObjectId,
    ) -> Result<bool, String> {
        Ok(self.cascade_land_source(progress)?.is_some()
            && progress.exiled.contains(&object)
            && self.cascade_exiled_land(object)?)
    }
    fn next_cascade_offer(&self, progress: &mut CascadeProgress) -> Result<(), String> {
        // Each live instance grants the same optional action. A concession can
        // change controllers before state-based actions check the legend rule.
        for object in self.state.rules.objects.values() {
            if !progress
                .land_choices
                .iter()
                .any(|choice| choice.provider == object.id)
                && self
                    .cascade_land_ability(object, progress.caster(self)?)?
                    .is_some()
            {
                for &id in &progress.exiled {
                    if self.cascade_exiled_land(id)? {
                        progress.stage = CascadeStage::LandOffer {
                            provider: object.id,
                        };
                        return Ok(());
                    }
                }
                break;
            }
        }
        if progress.hit.is_some() {
            progress.stage = CascadeStage::Offer;
        } else {
            progress.stage = CascadeStage::Randomize;
            self.prepare_cascade_remainder(progress)?;
        }
        Ok(())
    }
    pub(super) fn resume_cascade_land(
        &mut self,
        mut progress: Box<CascadeProgress>,
        entered: Option<ObjectId>,
    ) -> Result<(), String> {
        let CascadeStage::LandOffer { provider } = progress.stage else {
            return Err("missing cascade land provider".into());
        };
        progress
            .land_choices
            .push(CascadeLandChoice { provider, entered });
        self.next_cascade_offer(&mut progress)?;
        self.rules_mut().frame = Frame::Cascade { progress };
        Ok(())
    }
    // The public query walks this cascade's publicly observed scan in order.
    pub(super) fn cascade_land_query_len(&self) -> usize {
        match &self.state.rules.frame {
            Frame::Cascade { progress } => progress.exiled.len(),
            _ => 0,
        }
    }
    pub(super) fn cascade_land_query_row(
        &self,
        viewer: PlayerId,
        index: usize,
    ) -> Result<Option<ObjectHandle>, String> {
        let Frame::Cascade { progress } = &self.state.rules.frame else {
            return Ok(None);
        };
        if viewer == progress.caster(self)?
            && let Some(&object) = progress.exiled.get(index)
            && self.cascade_exiled_land(object)?
        {
            return self.handle_for(viewer, object).map(Some);
        }
        Ok(None)
    }
    pub(super) fn cascade_request(&self) -> Result<Option<Request>, String> {
        let Frame::Cascade { progress } = &self.state.rules.frame else {
            return Ok(None);
        };
        if matches!(progress.stage, CascadeStage::LandOffer { .. }) {
            if self.cascade_land_source(progress)?.is_none() {
                return Ok(None);
            }
            let mut candidates = vec![];
            for &object in &progress.exiled {
                if self.cascade_exiled_land(object)? {
                    candidates.push(object);
                }
            }
            return Ok(
                (!candidates.is_empty()).then_some(Request::CascadeLandEntry {
                    decision_id: self.state.rules.revision,
                    player: progress.caster(self)?,
                    candidates,
                }),
            );
        }
        Ok((self.is_live(progress.caster(self)?)
            && progress.stage == CascadeStage::Offer
            && self.resolution_offer_legal(&progress.permission(self)?)?)
        .then_some(Request::OptionalEffect {
            decision_id: self.state.rules.revision,
            player: progress.caster(self)?,
        }))
    }
    pub(super) fn validate_cascade_response(
        &self,
        progress: &CascadeProgress,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if let Response::CascadeLandEntry { object } = response {
            let Some(Request::CascadeLandEntry {
                player: chooser,
                candidates,
                ..
            }) = self.cascade_request()?
            else {
                return Err("no cascade land offer".into());
            };
            return if player == chooser && object.is_none_or(|id| candidates.contains(&id)) {
                Ok(())
            } else {
                Err("invalid cascade land choice".into())
            };
        }
        if player != progress.caster(self)? || !matches!(response, Response::OptionalEffect { .. })
        {
            return Err("cascade offer belongs only to its caster".into());
        }
        if self.resolution_offer_legal(&progress.permission(self)?)? {
            Ok(())
        } else {
            Err("no legal cascade casting path".into())
        }
    }
    pub(super) fn prepare_cascade_remainder(
        &self,
        progress: &mut CascadeProgress,
    ) -> Result<(), String> {
        progress.remainder = progress
            .exiled
            .iter()
            .copied()
            .filter(|id| self.current(*id).is_ok_and(|o| o.zone == Zone::Exile))
            .collect();
        if progress.remainder.len() > capacities::OWNED_LIMIT {
            return Err("cascade physical-card bound".into());
        }
        progress.permutation = (0..u32::try_from(progress.remainder.len())
            .map_err(|_| "cascade permutation bound")?)
            .collect();
        progress.random_index = progress.permutation.len().saturating_sub(1) as u32;
        Ok(())
    }
    pub(super) fn cascade_step(&mut self, mut progress: CascadeProgress) -> Result<(), String> {
        if !self.is_live(progress.caster(self)?) {
            return self.finish_ability(progress.frame.stack_id, false);
        }
        match progress.stage {
            CascadeStage::Scan => {
                let card = self
                    .state
                    .rules
                    .libraries
                    .get(&progress.caster(self)?)
                    .and_then(|v| v.first())
                    .copied();
                if let Some(card) = card {
                    let object = self.state.rules.objects[&card].id;
                    let successor = self
                        .move_object_for(
                            object,
                            Zone::Exile,
                            self.instruction_cause(&progress.frame, progress.source_block)?,
                        )?
                        .successor;
                    progress.exiled.push(successor);
                    if self.cascade_hit(successor, &progress.threshold)? {
                        progress.hit = Some(successor);
                        self.next_cascade_offer(&mut progress)?;
                    }
                } else {
                    self.next_cascade_offer(&mut progress)?;
                }
            }
            CascadeStage::LandOffer { .. } => {
                if let Some(input) = self.rules_mut().pending.take() {
                    let Response::CascadeLandEntry { object } = input.response else {
                        return Err("cascade land response shape invariant".into());
                    };
                    if input.player != progress.caster(self)? {
                        return Err("cascade land chooser invariant".into());
                    }
                    if let Some(object) = object
                        && self.cascade_land_available(&progress, object)?
                    {
                        self.begin_land_entry(
                            EntryOrigin::Cascade {
                                progress: Box::new(progress),
                            },
                            vec![object],
                        )?;
                        return Ok(());
                    }
                } else if self.cascade_request()?.is_some() {
                    return Err("missing accepted cascade land choice".into());
                }
                return self.resume_cascade_land(Box::new(progress), None);
            }
            CascadeStage::Offer => {
                let permission = progress.permission(self)?;
                let legal = self.resolution_offer_legal(&permission)?;
                // CR608.2g/800.4: an already accepted choice belongs to THIS
                // offer even if a concession removed its last legal target.
                // Consume only the exact frame-bound response; an invariant
                // error rolls the whole step back, retaining unrelated input.
                if let Some(input) = self.rules_mut().pending.take() {
                    if input.player != progress.caster(self)? {
                        return Err("cascade accepted choice caster invariant".into());
                    }
                    let Response::OptionalEffect { perform } = input.response else {
                        return Err("cascade accepted choice shape invariant".into());
                    };
                    if legal && perform {
                        return self
                            .announce_resolution_spell(CastOwner::Cascade(Box::new(progress)));
                    }
                } else if legal {
                    return Err("missing accepted cascade choice".into());
                }
                progress.stage = CascadeStage::Randomize;
                self.prepare_cascade_remainder(&mut progress)?;
            }
            CascadeStage::Randomize => {
                if progress.random_index > 0 {
                    let random = &mut self
                        .rules_mut()
                        .match_state
                        .as_mut()
                        .ok_or("missing trusted match randomness for random bottom")?
                        .random;
                    let word = random.word()?;
                    if let Some(j) = setup::sample_shuffle_word(word, progress.random_index + 1) {
                        progress
                            .permutation
                            .swap(progress.random_index as usize, j as usize);
                        progress.random_index -= 1;
                    }
                } else {
                    progress.stage = CascadeStage::Bottom;
                }
            }
            CascadeStage::Bottom => {
                // Commander choices follow the PUBLIC scan order, never secret
                // permutation order. All ordinary bottom movement waits until
                // these pre-movement 903.9b replacements are settled.
                while let Some(object) = progress
                    .remainder
                    .get(progress.commander_cursor as usize)
                    .copied()
                {
                    progress.commander_cursor += 1;
                    if self.current(object).is_ok_and(|o| o.zone == Zone::Exile)
                        && self.state.rules.commanders.history(object.card).is_some()
                    {
                        let owner = self.state.rules.cards[&object.card].owner;
                        self.rules_mut().frame = Frame::CommanderReturn {
                            object,
                            destination: Zone::Library { owner },
                            continuation: CommanderContinuation::CascadeBottom {
                                progress: Box::new(progress),
                            },
                        };
                        return Ok(());
                    }
                }
                let caster = progress.caster(self)?;
                let children = progress
                    .permutation
                    .iter()
                    .map(|i| progress.remainder[*i as usize])
                    .filter(|id| {
                        !progress.redirected.contains(id)
                            && self.current(*id).is_ok_and(|o| o.zone == Zone::Exile)
                    })
                    .map(|object| EventGroup::Occurrence {
                        proposed: ProposedEvent::Move {
                            object,
                            destination: Zone::Library { owner: caster },
                        },
                    })
                    .collect::<Vec<_>>();
                if !children.is_empty() {
                    self.commit_batch(
                        self.instruction_cause(&progress.frame, progress.source_block)?,
                        EventGroup::Ordered { children },
                    )?;
                }
                progress.stage = CascadeStage::Finish;
            }
            CascadeStage::Finish => {
                // Keep this exact owned completion visible to the narrow outer
                // removal guard; child remains paid ABOVE its resolving owner.
                self.rules_mut().frame = Frame::Cascade {
                    progress: Box::new(progress.clone()),
                };
                return self.finish_ability(progress.frame.stack_id, true);
            }
        }
        self.rules_mut().frame = Frame::Cascade {
            progress: Box::new(progress),
        };
        Ok(())
    }
    pub(super) fn validate_cascade_commander_choice(
        &self,
        progress: &CascadeProgress,
        object: ObjectId,
        destination: Zone,
    ) -> Result<(), String> {
        if progress.stage != CascadeStage::Bottom
            || progress
                .commander_cursor
                .checked_sub(1)
                .and_then(|i| progress.remainder.get(i as usize))
                .copied()
                != Some(object)
            || progress.redirected.contains(&object)
            || destination
                != (Zone::Library {
                    owner: progress.caster(self)?,
                })
        {
            return Err("cascade commander exact bottom choice invariant".into());
        }
        Ok(())
    }
    pub(super) fn resume_cascade_commander(
        &mut self,
        mut progress: Box<CascadeProgress>,
        object: ObjectId,
        to_command: bool,
    ) -> Result<(), String> {
        if to_command && self.current(object).is_ok_and(|o| o.zone == Zone::Exile) {
            let owner = self.state.rules.cards[&object.card].owner;
            self.move_object_for(object, Zone::Command { owner }, BatchCause::CommanderReturn)?;
            progress.redirected.push(object);
        }
        self.rules_mut().frame = Frame::Cascade { progress };
        Ok(())
    }
    pub(super) fn resolution_child(&self, owner: StackId) -> Option<StackId> {
        match &self.state.rules.frame {
            Frame::RevealAndCast { progress } if progress.frame.stack_id == owner => {
                progress.child()
            }
            Frame::HandCast { frame, child } if frame.stack_id == owner => *child,
            Frame::Cascade { progress } if progress.frame.stack_id == owner => progress.child,
            _ => None,
        }
    }
    pub(super) fn revalidate_cascade_departure(&mut self) -> Result<(), String> {
        let Frame::Cascade { progress } = &self.state.rules.frame else {
            return Ok(());
        };
        if matches!(progress.stage, CascadeStage::LandOffer { .. })
            && self.is_live(progress.caster(self)?)
            && self.cascade_request()?.is_none()
        {
            let progress = progress.clone();
            self.rules_mut().pending = None;
            return self.resume_cascade_land(progress, None);
        }
        let caster = progress.caster(self)?;
        if progress.stage == CascadeStage::Offer
            && self.is_live(caster)
            && self.state.rules.pending.as_ref().is_some_and(|input| {
                input.player == caster && matches!(input.response, Response::OptionalEffect { .. })
            })
            && !self.resolution_offer_legal(&progress.permission(self)?)?
        {
            // The accepted journal entry remains historical. Departure consumes
            // this exact invalidated offer input before exposing/recovering the
            // next boundary, as for the other owned decision continuations.
            self.rules_mut().pending = None;
        }
        Ok(())
    }
    pub(crate) fn cascade_charge(&self) -> usize {
        let frame = spell_cascade_grants::retained_effect_frame(
            &self.state.rules.frames,
            &self.state.rules.frame,
        );
        match frame {
            Frame::PlayHideawayLinked { .. }
            | Frame::Cascade { .. }
            | Frame::HandCast { .. }
            | Frame::RevealAndCast { .. } => stored_history::payload_charge(frame),
            Frame::CommanderReturn { continuation, .. }
                if matches!(continuation, CommanderContinuation::CascadeBottom { .. }) =>
            {
                stored_history::payload_charge(frame)
            }
            Frame::AdditionalCosts { cast }
            | Frame::HybridCost { cast }
            | Frame::AnnounceX { cast }
            | Frame::Modes { cast }
            | Frame::Targets { cast }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::Mana { cast }
            | Frame::SpendMana { cast }
            | Frame::Payment { cast }
            | Frame::SacrificeCost { cast }
                if !matches!(cast.completion, CastCompletion::Priority) =>
            {
                stored_history::payload_charge(&cast.completion)
            }
            _ => 0,
        }
    }
    pub(super) fn validate_cascade_state(&self) -> Result<(), String> {
        let (progress, permission, announcing) = match self
            .underlying_mana_frame(&self.state.rules.frame)?
        {
            Frame::PlayHideawayLinked { progress } => {
                return self.validate_linked_play(progress, None);
            }
            Frame::RevealAndCast { progress } => {
                return self.validate_reveal_and_cast(progress, None);
            }
            Frame::Cascade { progress }
            | Frame::LandEntry {
                origin: EntryOrigin::Cascade { progress },
                ..
            } => (progress.as_ref(), None, false),
            Frame::AdditionalCosts { cast }
            | Frame::HybridCost { cast }
            | Frame::AnnounceX { cast }
            | Frame::Modes { cast }
            | Frame::Targets { cast }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::Mana { cast }
            | Frame::SpendMana { cast }
            | Frame::Payment { cast }
            | Frame::SacrificeCost { cast } => {
                let CastCompletion::Resolution {
                    permission,
                    progress,
                } = &cast.completion
                else {
                    return Ok(());
                };
                if cast.original_object != permission.object
                    || !self.resulting_permission_matches(permission, self.top()?)?
                    || self
                        .top()?
                        .x
                        .as_ref()
                        .is_some_and(|x| *x != Quantity::zero())
                {
                    return Err("snapshot permission announcement invariant".into());
                }
                match progress {
                    CastOwner::LinkedPlay {
                        progress,
                        object,
                        receipt,
                    } => {
                        if !progress.receipt_matches(self, receipt, *object)? {
                            return Err("snapshot linked announcement receipt".into());
                        }
                        return self.validate_linked_play(progress, Some((permission, *object)));
                    }
                    CastOwner::Cascade(progress) => (progress.as_ref(), Some(permission), true),
                    CastOwner::RevealAndCast(progress) => {
                        return self.validate_reveal_and_cast(progress, Some(permission));
                    }
                }
            }
            Frame::CommanderReturn { continuation, .. } => {
                let CommanderContinuation::CascadeBottom { progress } = continuation else {
                    return Ok(());
                };
                (progress.as_ref(), None, false)
            }
            _ => return Ok(()),
        };
        let caster = progress.caster(self)?;
        self.validate_resolving_owned(
            &progress.frame,
            if announcing {
                Some(self.top()?.id)
            } else {
                progress.child
            },
        )?;
        let program = self.program_for(self.entry(progress.frame.stack_id)?)?;
        {
            return Err("snapshot cascade source/progress invariant".into());
        }
        let mut used_providers = BTreeSet::new();
        let mut entered_lands = BTreeSet::new();
        if progress.land_choices.len() > self.state.rules.cards.len()
            || (progress.stage == CascadeStage::Scan && !progress.land_choices.is_empty())
        {
            return Err("snapshot cascade land choice bound/stage".into());
        }
        for choice in &progress.land_choices {
            if !used_providers.insert(choice.provider)
                || !self
                    .copiable_definition(choice.provider)?
                    .static_abilities
                    .iter()
                    .any(|a| {
                        a.functioning_zone == FunctioningZone::Battlefield
                            && matches!(a.effect, StaticEffect::CascadeLandEntry)
                    })
            {
                return Err("snapshot cascade land provider invariant".into());
            }
            let Some(land) = choice.entered else {
                continue;
            };
            // The free child may subsequently tap or sacrifice this permanent.
            // Check the first successor while it exists and retain the old exile ID.
            if !entered_lands.insert(land)
                || !progress.exiled.contains(&land)
                || !self
                    .copiable_definition(land)?
                    .characteristics
                    .has_type(CardKind::Land)
                || self.is_live(caster)
                    && self.state.rules.objects.get(&land.card).is_none_or(|o| {
                        o.id.incarnation <= land.incarnation
                            || land.incarnation.checked_add(1) == Some(o.id.incarnation)
                                && (o.zone != Zone::Battlefield || o.default_controller != caster)
                    })
            {
                return Err("snapshot cascade entered land invariant".into());
            }
        }
        if let CascadeStage::LandOffer { provider } = progress.stage
            && self.is_live(caster)
            && (used_providers.contains(&provider)
                || self.cascade_land_source(progress)?.is_none()
                || !progress
                    .exiled
                    .iter()
                    .any(|id| self.cascade_exiled_land(*id) == Ok(true))
                || progress.hit.is_none()
                    && self
                        .state
                        .rules
                        .libraries
                        .get(&caster)
                        .is_some_and(|l| !l.is_empty()))
        {
            return Err("snapshot cascade land offer source/scan invariant".into());
        }
        if matches!(
            progress.stage,
            CascadeStage::Scan | CascadeStage::LandOffer { .. } | CascadeStage::Offer
        ) {
            if !progress.remainder.is_empty()
                || progress.random_index != 0
                || progress.commander_cursor != 0
                || !progress.redirected.is_empty()
                || !progress.permutation.is_empty()
                || progress.child.is_some()
                || (progress.stage == CascadeStage::Scan && progress.hit.is_some())
                || (progress.stage == CascadeStage::Offer && progress.hit.is_none())
            {
                return Err("snapshot cascade scan/offer invariant".into());
            }
            for x in &progress.exiled {
                if entered_lands.contains(x) || announcing && Some(*x) == progress.hit {
                    continue;
                }
                if self.is_live(caster) && self.current(*x)?.zone != Zone::Exile {
                    return Err("snapshot cascade exact exile invariant".into());
                }
            }
        } else {
            let expected = progress
                .exiled
                .iter()
                .copied()
                .filter(|x| {
                    !entered_lands.contains(x)
                        && !(progress.child.is_some() && Some(*x) == progress.hit)
                })
                .collect::<Vec<_>>();
            let mut sorted = progress.permutation.clone();
            sorted.sort_unstable();
            if progress.remainder != expected
                || sorted != (0..progress.remainder.len() as u32).collect::<Vec<_>>()
                || progress.random_index as usize >= progress.remainder.len().max(1)
                || progress.commander_cursor as usize > progress.remainder.len()
                || progress.redirected.iter().collect::<BTreeSet<_>>().len()
                    != progress.redirected.len()
                || progress
                    .redirected
                    .iter()
                    .any(|x| !progress.remainder.contains(x))
            {
                return Err("snapshot cascade bottom/permutation invariant".into());
            }
            if progress.stage == CascadeStage::Randomize
                && (progress.commander_cursor != 0 || !progress.redirected.is_empty())
                || matches!(progress.stage, CascadeStage::Bottom | CascadeStage::Finish)
                    && progress.random_index != 0
                || progress.stage == CascadeStage::Finish
                    && progress.commander_cursor as usize != progress.remainder.len()
            {
                return Err("snapshot cascade stage cursor invariant".into());
            }
            if self.is_live(caster) {
                for object in &progress.remainder {
                    let redirected = progress.redirected.contains(object);
                    if redirected
                        && (!progress.remainder[..progress.commander_cursor as usize]
                            .contains(object)
                            || self.state.rules.commanders.history(object.card).is_none())
                    {
                        return Err("snapshot cascade redirected membership invariant".into());
                    }
                    let current = self
                        .state
                        .rules
                        .objects
                        .get(&object.card)
                        .ok_or("snapshot missing cascade remainder")?;
                    if redirected || progress.stage == CascadeStage::Finish {
                        if object.incarnation.checked_add(1) != Some(current.id.incarnation)
                            || current.zone
                                != if redirected {
                                    Zone::Command { owner: caster }
                                } else {
                                    Zone::Library { owner: caster }
                                }
                        {
                            return Err("snapshot cascade bottom successor invariant".into());
                        }
                    } else if current.id != *object || current.zone != Zone::Exile {
                        return Err("snapshot cascade unbottomed remainder invariant".into());
                    }
                }
                if progress.stage == CascadeStage::Finish {
                    let bottom = progress
                        .permutation
                        .iter()
                        .map(|i| progress.remainder[*i as usize])
                        .filter(|o| !progress.redirected.contains(o))
                        .map(|o| o.card)
                        .collect::<Vec<_>>();
                    if !self
                        .state
                        .rules
                        .libraries
                        .get(&caster)
                        .is_some_and(|library| library.ends_with(&bottom))
                    {
                        return Err("snapshot cascade bottom order invariant".into());
                    }
                }
            }
            if let Some(child) = progress.child.filter(|_| self.is_live(caster)) {
                let top = self.top()?;
                if top.id != child
                    || Some(top.source.card) != progress.hit.map(|h| h.card)
                    || progress.hit.and_then(|h| h.incarnation.checked_add(1))
                        != Some(top.source.incarnation)
                    || top.paid.is_none()
                    || top.cast_from != Zone::Exile
                    || top.alternative_cost != Some(AlternativeCost::WithoutManaCost)
                {
                    return Err("snapshot cascade completed child invariant".into());
                }
            }
        }
        Ok(())
    }
}

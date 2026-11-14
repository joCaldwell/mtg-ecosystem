//! CR 107.3b, 117.2e/5, 118.9, 601.2, 608.2g/h, 702.85a, 733.1, 903.9b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! One resolving owner travels through Casting and nested ManaAbility.resume.
//! No global parent slot, priority permission, recursive child resolution or
//! whole-library shuffle participates in this continuation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AlternativeCost {
    WithoutManaCost,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CastPermission {
    pub(super) object: ObjectId,
    pub(super) caster: PlayerId,
    owner: StackId,
    cursor: u32,
    source_block: u32,
    pub(super) alternative_cost: AlternativeCost,
    resulting_mana_value_below: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "return_to", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CastCompletion {
    Priority,
    Resolution {
        permission: CastPermission,
        progress: Box<CascadeProgress>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CascadeStage {
    Scan,
    Offer,
    Randomize,
    Bottom,
    Finish,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CascadeProgress {
    pub(super) frame: Resolving,
    source_block: u32,
    threshold: Quantity,
    pub(super) stage: CascadeStage,
    exiled: Vec<ObjectId>,
    hit: Option<ObjectId>,
    pub(super) child: Option<StackId>,
    remainder: Vec<ObjectId>,
    permutation: Vec<u32>,
    random_index: u32,
    commander_cursor: u32,
    redirected: Vec<ObjectId>,
}
impl CascadeProgress {
    fn caster(&self) -> PlayerId {
        self.frame.spell.controller
    }
    fn permission(&self) -> Result<CastPermission, String> {
        Ok(CastPermission {
            object: self.hit.ok_or("cascade offer without hit")?,
            caster: self.caster(),
            owner: self.frame.stack_id,
            cursor: self.frame.cursor,
            source_block: self.source_block,
            alternative_cost: AlternativeCost::WithoutManaCost,
            resulting_mana_value_below: self.threshold.clone(),
        })
    }
}
impl Game {
    fn cascade_threshold(&self, frame: &Resolving) -> Result<Quantity, String> {
        let StackKind::Triggered {
            witness: TriggerEventWitness::SpellCast { cast },
            ..
        } = &frame.spell.kind
        else {
            return Err("cascade requires its real self-cast occurrence".into());
        };
        if cast.spell.object != frame.spell.source || cast.caster != frame.spell.controller {
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
        let cost = &cast.spell.characteristics.mana_cost;
        let mut value = cost.generic.clone();
        value.add(&Quantity::from(cost.colored.len() as i64));
        if let Some(x) = &cast.x {
            value.add(x);
        }
        Ok(value)
    }
    pub(super) fn begin_cascade(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let threshold = self.cascade_threshold(&frame)?;
        let progress = CascadeProgress {
            frame,
            source_block,
            threshold,
            stage: CascadeStage::Scan,
            exiled: vec![],
            hit: None,
            child: None,
            remainder: vec![],
            permutation: vec![],
            random_index: 0,
            commander_cursor: 0,
            redirected: vec![],
        };
        self.state.rules.frame = Frame::Cascade {
            progress: Box::new(progress),
        };
        Ok(())
    }
    fn nonstack_mana_value(&self, object: ObjectId) -> Result<Quantity, String> {
        let cost = &self.definition(object.card)?.characteristics.mana_cost;
        let mut value = cost.generic.clone();
        value.add(&Quantity::from(cost.colored.len() as i64));
        Ok(value) // undefined X is zero outside stack (CR202.3e)
    }
    fn cascade_hit(&self, object: ObjectId, threshold: &Quantity) -> Result<bool, String> {
        Ok(!self
            .definition(object.card)?
            .characteristics
            .has_type(CardKind::Land)
            && self.nonstack_mana_value(object)? < *threshold)
    }
    fn resolution_permission_identity(
        &self,
        permission: &CastPermission,
        progress: &CascadeProgress,
    ) -> Result<(), String> {
        if progress.stage != CascadeStage::Offer
            || &progress.permission()? != permission
            || self.current(permission.object)?.zone != Zone::Exile
            || self.state.rules.cards[&permission.object.card].owner != permission.caster
            || !self.cascade_hit(permission.object, &permission.resulting_mana_value_below)?
            || progress.child.is_some()
            || self.top()?.id != permission.owner
        {
            return Err("resolution cast permission invariant".into());
        }
        Ok(())
    }
    fn cascade_offer_legal(&self, progress: &CascadeProgress) -> Result<bool, String> {
        let permission = progress.permission()?;
        self.resolution_permission_identity(&permission, progress)?;
        if !self.is_live(permission.caster) || self.cast_is_prohibited(permission.caster)? {
            return Ok(false);
        }
        let program = self
            .definition(permission.object.card)?
            .spell
            .as_ref()
            .ok_or("cascade nonland is not a supported spell")?;
        Ok(self.has_target_domain(program, permission.caster)
            && program
                .additional_sacrifice
                .as_ref()
                .is_none_or(|f| !self.sacrifice_candidates(permission.caster, f).is_empty()))
    }
    pub(super) fn validate_resolution_permission(
        &self,
        permission: &CastPermission,
        progress: &CascadeProgress,
    ) -> Result<(), String> {
        self.resolution_permission_identity(permission, progress)?;
        if !self.cascade_offer_legal(progress)? {
            return Err("no legal cascade casting path".into());
        }
        Ok(())
    }
    pub(super) fn resulting_permission_matches(
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
            && self.stack_mana_value(stack)? < permission.resulting_mana_value_below)
    }
    pub(super) fn announcement_mana_frame(&self, mut cast: Casting) -> Result<Frame, String> {
        let stack = self.top()?;
        if matches!(cast.completion, CastCompletion::Resolution { .. })
            && stack.total_cost.generic == Quantity::zero()
            && stack.total_cost.colored.is_empty()
        {
            // No mana-cost payment exists for the free alternative. Mandatory
            // nonmana additionals still use their ordinary payment authority.
            // No unrestricted 601.2g mana window is granted.
            cast.spending = Some(BTreeMap::new());
            Ok(if self.invocation_sacrifice_filter(stack).is_some() {
                Frame::SacrificeCost { cast }
            } else {
                Frame::Payment { cast }
            })
        } else {
            Ok(Frame::Mana { cast })
        }
    }
    pub(super) fn cascade_request(&self) -> Result<Option<Request>, String> {
        let Frame::Cascade { progress } = &self.state.rules.frame else {
            return Ok(None);
        };
        Ok((self.is_live(progress.caster())
            && progress.stage == CascadeStage::Offer
            && self.cascade_offer_legal(progress)?)
        .then_some(Request::OptionalEffect {
            decision_id: self.state.rules.revision,
            player: progress.caster(),
        }))
    }
    pub(super) fn validate_cascade_response(
        &self,
        progress: &CascadeProgress,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if player != progress.caster() || !matches!(response, Response::OptionalEffect { .. }) {
            return Err("cascade offer belongs only to its caster".into());
        }
        self.validate_resolution_permission(&progress.permission()?, progress)
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
        if !self.is_live(progress.caster()) {
            return self.finish_ability(&progress.frame.spell, false);
        }
        match progress.stage {
            CascadeStage::Scan => {
                let card = self
                    .state
                    .rules
                    .libraries
                    .get(&progress.caster())
                    .and_then(|v| v.first())
                    .copied();
                if let Some(card) = card {
                    let object = self.state.rules.objects[&card].id;
                    let successor = self
                        .move_object_for(
                            object,
                            Zone::Exile,
                            Self::wipe_cause(&progress.frame, progress.source_block),
                        )?
                        .successor;
                    progress.exiled.push(successor);
                    if self.cascade_hit(successor, &progress.threshold)? {
                        progress.hit = Some(successor);
                        progress.stage = CascadeStage::Offer;
                    }
                } else {
                    progress.stage = CascadeStage::Randomize;
                    self.prepare_cascade_remainder(&mut progress)?;
                }
            }
            CascadeStage::Offer => {
                let permission = progress.permission()?;
                let legal = self.cascade_offer_legal(&progress)?;
                // CR608.2g/800.4: an already accepted choice belongs to THIS
                // offer even if a concession removed its last legal target.
                // Consume only the exact frame-bound response; an invariant
                // error rolls the whole step back, retaining unrelated input.
                if let Some(input) = self.state.rules.pending.take() {
                    if input.player != progress.caster() {
                        return Err("cascade accepted choice caster invariant".into());
                    }
                    let Response::OptionalEffect { perform } = input.response else {
                        return Err("cascade accepted choice shape invariant".into());
                    };
                    self.resolution_permission_identity(&permission, &progress)?;
                    if legal && perform {
                        return self.announce_resolution_spell(permission, Box::new(progress));
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
                        .state
                        .rules
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
                        self.state.rules.frame = Frame::CommanderReturn {
                            object,
                            destination: Zone::Library { owner },
                            continuation: Box::new(CommanderContinuation::CascadeBottom {
                                progress: Box::new(progress),
                            }),
                        };
                        return Ok(());
                    }
                }
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
                            destination: Zone::Library {
                                owner: progress.caster(),
                            },
                        },
                    })
                    .collect::<Vec<_>>();
                if !children.is_empty() {
                    self.commit_batch(
                        Self::wipe_cause(&progress.frame, progress.source_block),
                        EventGroup::Ordered { children },
                    )?;
                }
                progress.stage = CascadeStage::Finish;
            }
            CascadeStage::Finish => {
                // Keep this exact owned completion visible to the narrow outer
                // removal guard; child remains paid ABOVE its resolving owner.
                self.state.rules.frame = Frame::Cascade {
                    progress: Box::new(progress.clone()),
                };
                return self.finish_ability(&progress.frame.spell, true);
            }
        }
        self.state.rules.frame = Frame::Cascade {
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
                    owner: progress.caster(),
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
        self.state.rules.frame = Frame::Cascade { progress };
        Ok(())
    }
    pub(super) fn resolution_child(&self, owner: StackId) -> Option<StackId> {
        match &self.state.rules.frame {
            Frame::Cascade { progress } if progress.frame.stack_id == owner => progress.child,
            _ => None,
        }
    }
    pub(super) fn revalidate_cascade_departure(&mut self) -> Result<(), String> {
        let Frame::Cascade { progress } = &self.state.rules.frame else {
            return Ok(());
        };
        if progress.stage == CascadeStage::Offer
            && self.is_live(progress.caster())
            && self.state.rules.pending.as_ref().is_some_and(|input| {
                input.player == progress.caster()
                    && matches!(input.response, Response::OptionalEffect { .. })
            })
            && !self.cascade_offer_legal(progress)?
        {
            // The accepted journal entry remains historical. Departure consumes
            // this exact invalidated offer input before exposing/recovering the
            // next boundary, as for the other owned decision continuations.
            self.state.rules.pending = None;
        }
        Ok(())
    }
    pub(super) fn cascade_charge(&self) -> usize {
        let frame = self.underlying_mana_frame(&self.state.rules.frame);
        match frame {
            Frame::Cascade { .. } => stored_history::payload_charge(frame),
            Frame::CommanderReturn { continuation, .. }
                if matches!(
                    continuation.as_ref(),
                    CommanderContinuation::CascadeBottom { .. }
                ) =>
            {
                stored_history::payload_charge(frame)
            }
            Frame::AnnounceX { cast }
            | Frame::Modes { cast }
            | Frame::Targets { cast }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::Mana { cast }
            | Frame::SpendMana { cast }
            | Frame::Payment { cast }
            | Frame::SacrificeCost { cast }
                if matches!(cast.completion, CastCompletion::Resolution { .. }) =>
            {
                stored_history::payload_charge(&cast.completion)
            }
            _ => 0,
        }
    }
    pub(super) fn validate_cascade_state(&self) -> Result<(), String> {
        let (progress, permission, announcing) = match self
            .underlying_mana_frame(&self.state.rules.frame)
        {
            Frame::Cascade { progress } => (progress.as_ref(), None, false),
            Frame::AnnounceX { cast }
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
                    || self.top()?.cast_from != Zone::Exile
                    || self.top()?.alternative_cost != Some(AlternativeCost::WithoutManaCost)
                    || self.stack_mana_value(self.top()?)? >= permission.resulting_mana_value_below
                    || self
                        .top()?
                        .x
                        .as_ref()
                        .is_some_and(|x| *x != Quantity::zero())
                {
                    return Err("snapshot permission announcement invariant".into());
                }
                (progress.as_ref(), Some(permission), true)
            }
            Frame::CommanderReturn { continuation, .. } => {
                let CommanderContinuation::CascadeBottom { progress } = continuation.as_ref()
                else {
                    return Ok(());
                };
                (progress.as_ref(), None, false)
            }
            _ => return Ok(()),
        };
        self.validate_resolving_owned(
            &progress.frame,
            if announcing {
                Some(self.top()?.id)
            } else {
                progress.child
            },
        )?;
        let program = self.program_for(&progress.frame.spell)?;
        if !matches!(program.instructions.get(progress.frame.cursor as usize), Some(Instruction::Cascade { source_block, .. }) if *source_block == progress.source_block)
            || self.cascade_threshold(&progress.frame)? != progress.threshold
            || progress.exiled.len() > capacities::OWNED_LIMIT
            || progress
                .exiled
                .iter()
                .map(|x| x.card)
                .collect::<BTreeSet<_>>()
                .len()
                != progress.exiled.len()
            || progress.exiled.iter().any(|x| {
                self.state
                    .rules
                    .cards
                    .get(&x.card)
                    .is_none_or(|c| c.owner != progress.caster())
            })
            || progress.hit.is_some_and(|hit| {
                progress.exiled.last() != Some(&hit)
                    || !self.cascade_hit(hit, &progress.threshold).unwrap_or(false)
            })
            || progress.exiled.iter().any(|x| {
                Some(*x) != progress.hit
                    && self.cascade_hit(*x, &progress.threshold).unwrap_or(false)
            })
            || (announcing && (progress.stage != CascadeStage::Offer || progress.child.is_some()))
            || permission.is_some_and(|p| progress.permission().as_ref() != Ok(p))
        {
            return Err("snapshot cascade source/progress invariant".into());
        }
        if matches!(progress.stage, CascadeStage::Scan | CascadeStage::Offer) {
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
                if announcing && Some(*x) == progress.hit {
                    continue;
                }
                if self.is_live(progress.caster()) && self.current(*x)?.zone != Zone::Exile {
                    return Err("snapshot cascade exact exile invariant".into());
                }
            }
        } else {
            let expected = progress
                .exiled
                .iter()
                .copied()
                .filter(|x| !(progress.child.is_some() && Some(*x) == progress.hit))
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
            if self.is_live(progress.caster()) {
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
                                    Zone::Command {
                                        owner: progress.caster(),
                                    }
                                } else {
                                    Zone::Library {
                                        owner: progress.caster(),
                                    }
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
                        .get(&progress.caster())
                        .is_some_and(|library| library.ends_with(&bottom))
                    {
                        return Err("snapshot cascade bottom order invariant".into());
                    }
                }
            }
            if let Some(child) = progress.child.filter(|_| self.is_live(progress.caster())) {
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

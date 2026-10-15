use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AnnouncementOrigin {
    #[default]
    Spell,
    Activated,
}
impl AnnouncementOrigin {
    pub(super) fn is_spell(&self) -> bool {
        matches!(self, Self::Spell)
    }
}

impl Game {
    pub(super) fn announce_ability(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        let cost = self
            .definition(object.card)?
            .activated
            .get(ability as usize)
            .ok_or("missing activated cost")?
            .cost
            .clone();
        let source_handles = (0..4)
            .map(|viewer| self.handle_for(PlayerId(viewer), object))
            .collect::<Result<Vec<_>, _>>()?;
        let id = StackId(self.state.next_stack_id);
        self.state.next_stack_id = self
            .state
            .next_stack_id
            .checked_add(1)
            .ok_or("stack ID resource limit")?;
        let total_cost = match &cost {
            ActivationCost::Mana { cost } => cost.clone(),
            _ => ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
        };
        let mana = matches!(cost, ActivationCost::Mana { .. });
        let targeted = !self.definition(object.card)?.activated[ability as usize]
            .program
            .targets
            .is_empty();
        self.state.stack.push(StackInstance {
            mode: None,
            effect_source_handles: vec![],
            source_facts: if self.prepared.config.damage_life_profile() {
                Some(self.object_before(object)?)
            } else {
                None
            },
            entity_targets: vec![],
            announced_entity_targets: vec![],
            kind: StackKind::Activated {
                ability,
                source_handles,
            },
            stack_targets: vec![],
            announced_stack_targets: vec![],
            player_targets: vec![],
            id,
            source: object,
            controller: player,
            targets: vec![],
            announced_targets: vec![vec![]; 4],
            paid: (!mana && !targeted).then_some(total_cost.clone()),
            total_cost,
            cast_from: Zone::Battlefield,
        });
        if targeted {
            if cost != ActivationCost::TapSource {
                return Err("unsupported targeted activation cost".into());
            }
            self.state.frame = Frame::EntityTargets {
                cast: Casting {
                    origin: AnnouncementOrigin::Activated,
                    spending: None,
                    stack_id: id,
                    original_object: object,
                    prior_passes: self.state.passes.clone(),
                    mana_activations: vec![],
                    undo_mana: false,
                },
            };
        } else if mana {
            self.state.frame = Frame::Mana {
                cast: Casting {
                    origin: AnnouncementOrigin::Activated,
                    spending: None,
                    stack_id: id,
                    original_object: object,
                    prior_passes: self.state.passes.clone(),
                    mana_activations: vec![],
                    undo_mana: false,
                },
            };
        } else {
            let proposed = match cost {
                ActivationCost::SacrificeSource => ProposedEvent::Sacrifice { object, player },
                ActivationCost::TapSource => ProposedEvent::Tap { object },
                ActivationCost::Mana { .. } => return Err("activated cost branch invariant".into()),
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
            self.record_event(Event::AbilityActivated {
                stack_id: id,
                source: object,
                player,
                ability,
            })?;
            self.state.priority = player;
            self.stabilize()?;
        }
        self.state.passes.clear();
        Ok(())
    }
    pub(super) fn validate_activated_entry(
        &self,
        entry: &StackInstance,
        ability: u32,
        source_handles: &[ObjectHandle],
    ) -> Result<(), String> {
        let definition = self.definition(entry.source.card)?;
        let activated = definition
            .activated
            .get(ability as usize)
            .ok_or("snapshot missing activated definition")?;
        let expected = match &activated.cost {
            ActivationCost::Mana { cost } => cost.clone(),
            _ => ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
        };
        let announcing = match &self.state.frame {
            Frame::EntityTargets { cast }
            | Frame::Mana { cast }
            | Frame::SpendMana { cast }
            | Frame::Payment { cast } => {
                cast.stack_id == entry.id && cast.origin == AnnouncementOrigin::Activated
            }
            _ => matches!(
                self.state.frame,
                Frame::Fault { .. } | Frame::Finished { .. }
            ),
        };
        if !self.prepared.config.actions_profile()
            || source_handles.len() != 4
            || !entry.targets.is_empty()
            || !entry.player_targets.is_empty()
            || !entry.stack_targets.is_empty()
            || entry.total_cost != expected
            || entry.cast_from != Zone::Battlefield
            || match &entry.paid {
                Some(paid) => *paid != expected,
                None => {
                    !announcing
                        || !(matches!(activated.cost, ActivationCost::Mana { .. })
                            || (!activated.program.targets.is_empty()
                                && self.prepared.config.damage_life_profile()))
                }
            }
        {
            return Err("snapshot activated invocation invariant".into());
        }
        Ok(())
    }
    pub(super) fn validate_stack_bindings(&self, entry: &StackInstance) -> Result<(), String> {
        self.validate_damage_bindings(entry)?;
        let program = self.program_for(entry)?;
        let stack_domain = program.targets.first().is_some_and(|target| {
            matches!(
                target.domain,
                TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility
            )
        });
        if !entry.stack_targets.is_empty() {
            if !self.prepared.config.stack_costs_profile()
                || !stack_domain
                || !entry.targets.is_empty()
                || !entry.player_targets.is_empty()
                || entry.stack_targets.len() > program.targets.len()
                || entry.announced_stack_targets.len() != 4
                || entry
                    .announced_stack_targets
                    .iter()
                    .any(|entries| entries.len() != entry.stack_targets.len())
                || entry.stack_targets.contains(&entry.id)
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
            .stack
            .iter()
            .filter(|entry| {
                Some(entry.id) != exclude
                    && entry.paid.is_some()
                    && match domain {
                        TargetDomain::StackSpell => entry.kind.is_spell(),
                        TargetDomain::StackActivatedOrTriggeredAbility => !entry.kind.is_spell(),
                        _ => false,
                    }
            })
            .map(|entry| entry.id)
            .collect()
    }
    pub(super) fn stack_target_legal(
        &self,
        target: StackId,
        countering: StackId,
        domain: TargetDomain,
    ) -> bool {
        self.eligible_stack_targets(domain, Some(countering))
            .contains(&target)
    }
    pub(super) fn validate_stack_targets(
        &self,
        player: PlayerId,
        entries: &[StackId],
    ) -> Result<(), String> {
        let top = self.top()?;
        let program = self.program_for(top)?;
        if !self.prepared.config.stack_costs_profile()
            || top.controller != player
            || entries.len() != program.targets.len()
            || entries.iter().enumerate().any(|(slot, target)| {
                !self.stack_target_legal(*target, top.id, program.targets[slot].domain)
            })
        {
            return Err("invalid stack targets".into());
        }
        Ok(())
    }
    pub(super) fn bind_stack_targets(&mut self, cast: Casting) -> Result<(), String> {
        let input = self
            .state
            .pending
            .take()
            .ok_or("missing stack target input")?;
        let Response::StackTargets { entries } = input.response else {
            return Err("stack target response invariant".into());
        };
        self.validate_stack_targets(input.player, &entries)?;
        let announced = (0..4)
            .map(|viewer| {
                entries
                    .iter()
                    .map(|target| self.stack_handle_for(PlayerId(viewer), *target))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .state
            .stack
            .last_mut()
            .ok_or("missing stack target invocation")?;
        top.stack_targets = entries;
        top.announced_stack_targets = announced;
        self.state.frame = Frame::Mana { cast };
        Ok(())
    }
    // CR 701.6a-b/608.2b: remove precisely the bound stack object. Paid costs
    // and physical Commander history are never refunded by countering it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn resolve_counter_instruction(
        &mut self,
        frame: Resolving,
        target_slot: u32,
        source_block: u32,
    ) -> Result<(), String> {
        let target = *frame
            .spell
            .stack_targets
            .get(target_slot as usize)
            .ok_or("missing counter target binding")?;
        let outcome = if !frame
            .legal_targets
            .get(target_slot as usize)
            .copied()
            .unwrap_or(false)
        {
            InstructionOutcome::StackTargetIllegal { target }
        } else if !self.state.stack.iter().any(|entry| entry.id == target) {
            InstructionOutcome::StackObjectNoLongerPresent { target }
        } else {
            let domain = self
                .program_for(&frame.spell)?
                .targets
                .get(target_slot as usize)
                .ok_or("missing counter target domain")?
                .domain;
            let EventOutcome::StackObjectCountered { movement, .. } = self.commit(
                ProposedEvent::CounterStackObject {
                    target,
                    countering: frame.stack_id,
                    domain,
                },
                BatchCause::Instruction {
                    stack_id: frame.stack_id,
                    player: frame.spell.controller,
                    source: frame.spell.source,
                    cursor: frame.cursor,
                    source_block,
                },
            )?
            else {
                return Err("missing actual counter outcome".into());
            };
            InstructionOutcome::Countered { target, movement }
        };
        self.finish_instruction(frame, outcome)
    }
}

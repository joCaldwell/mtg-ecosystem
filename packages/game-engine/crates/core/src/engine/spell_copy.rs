//! CR 111.13, 601.2, 608.2h, 702.144, 704.5d/e, 707.9b/f, 707.10/10f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use tokens::{TokenIdentity, TokenTemplate, copied_creature_definition};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyIdentity {
    card: CardId,
    stack_id: StackId,
}

impl Game {
    pub(crate) fn is_spell_copy(&self, entry: &StackInstance) -> bool {
        entry.kind.is_spell()
            && entry.source.incarnation == 0
            && self.state.rules.tokens.get(&entry.source.card).is_some_and(|identity|
                matches!(identity.spec, TokenTemplate::SpellCopy { stack_id, .. } if stack_id == entry.id))
    }

    pub(super) fn stack_entry_complete(&self, entry: &StackInstance) -> bool {
        entry.paid.as_ref() == Some(&entry.total_cost) || self.is_spell_copy(entry)
    }

    pub(super) fn copy_creature_spell(
        &mut self,
        frame: Resolving,
        target_slot: u32,
        source_block: u32,
    ) -> Result<(), String> {
        let slot = target_slot as usize;
        let target = *frame
            .spell
            .stack_targets
            .get(slot)
            .ok_or("missing copy target")?;
        // The common target authority froze CR 608.2b legality for this
        // invocation. A departure may since have removed the bound spell.
        if frame.legal_targets.get(slot) != Some(&true) {
            return self
                .finish_instruction(frame, InstructionOutcome::StackTargetIllegal { target });
        }
        let Some(original) = self
            .state
            .rules
            .stack
            .iter()
            .find(|entry| entry.id == target)
            .cloned()
        else {
            return self.finish_instruction(
                frame,
                InstructionOutcome::StackObjectNoLongerPresent { target },
            );
        };
        let controller = frame.spell.controller;
        if !self.is_live(controller) {
            return self.finish_instruction(
                frame,
                InstructionOutcome::PlayerActionSkipped { player: controller },
            );
        }
        let definition = copied_creature_definition(self.definition(original.source.card)?);
        let copy = self.place_spell_copy(&frame, source_block, controller, original, definition)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::SpellCopied {
                copy: copy.stack_id,
            },
        )
    }

    fn place_spell_copy(
        &mut self,
        frame: &Resolving,
        source_block: u32,
        controller: PlayerId,
        mut copy: StackInstance,
        definition: Definition,
    ) -> Result<CopyIdentity, String> {
        self.check_stack_capacity(1)?;
        self.preflight_stack_ids(1)?;
        self.check_token_capacity(1)?;
        let card = self.next_token_card()?;
        let stack_id = StackId(self.state.rules.next_stack_id);
        let target = copy.id;
        let identity = TokenIdentity {
            source: frame.spell.source,
            spec: TokenTemplate::SpellCopy {
                original: copy.source,
                stack_id,
            },
            definition,
        };
        self.state.rules.next_stack_id += 1; // Reserved above along with outstanding triggers' IDs.
        let object = self.insert_token_identity(card, controller, Zone::Stack, identity);
        // Keep all announcement choices, including the original objects used
        // to pay additional costs (CR 707.10). Rebind only the new source,
        // identity and controller. The copy never went through CR 601.2.
        copy.id = stack_id;
        copy.source = object;
        copy.controller = controller;
        copy.source_facts = Some(self.object_before(object)?);
        copy.effect_source_handles = self.capture_trigger_source_handles(object)?;
        copy.paid = None;
        copy.cost_lock = None;
        copy.total_cost = zero_mana_cost();
        copy.cast_from = Zone::Stack;
        self.state.rules.stack.push(copy);
        self.record_event(Event::SpellCopied {
            cause: Self::wipe_cause(frame, source_block),
            target,
            copy: stack_id,
            object,
        })?;
        Ok(CopyIdentity { card, stack_id })
    }
    pub(super) fn demonstrate_cast<'a>(
        &self,
        frame: &'a Resolving,
    ) -> Result<&'a CastOccurrence, String> {
        let StackKind::Triggered {
            ability,
            witness: TriggerEventWitness::SpellCast { cast },
            ..
        } = &frame.spell.kind
        else {
            return Err("demonstrate requires a cast trigger".into());
        };
        if self
            .effective_trigger(frame.spell.source, *ability)?
            .condition
            != TriggerCondition::SelfSpellCast
            || cast.spell.object != frame.spell.source
            || cast.caster != frame.spell.controller
            || cast.stack_id >= frame.stack_id
        {
            return Err("demonstrate cast/source binding".into());
        }
        Ok(cast)
    }

    fn copy_demonstrated_spell(
        &mut self,
        frame: &Resolving,
        source_block: u32,
        controller: PlayerId,
    ) -> Result<CopyIdentity, String> {
        let cast = self.demonstrate_cast(frame)?;
        let definition = self.definition(cast.spell.object.card)?.clone();
        // CR 608.2h/707.10: if the spell was countered, use the stack object
        // as it last existed. The admitted spell has no targets, modes, X, or
        // additional costs. The successful-cast facts distinguish an ordinary
        // paid exile cast from a no-mana-cost resolution cast.
        let original = if let Some(entry) = self
            .state
            .rules
            .stack
            .iter()
            .find(|entry| entry.id == cast.stack_id)
        {
            if !entry.kind.is_spell() || entry.source != cast.spell.object || entry.paid.is_none() {
                return Err("demonstrate original stack identity".into());
            }
            entry.clone()
        } else {
            let spell = definition
                .spell
                .as_ref()
                .ok_or("missing demonstrated spell program")?;
            if !spell.targets.is_empty()
                || !spell.modes.is_empty()
                || definition.x_mana_cost
                || !spell.optional_additional_costs.is_empty()
                || spell.additional_sacrifice.is_some()
            {
                return Err("unsupported demonstrate last-known announcement choices".into());
            }
            let mut original = frame.spell.clone();
            original.kind = StackKind::Spell;
            original.id = cast.stack_id;
            original.alternative_cost = (cast.cast_from == Zone::Exile
                && !cast.priority_paid_exile)
                .then_some(cascade::AlternativeCost::WithoutManaCost);
            original
        };
        self.place_spell_copy(frame, source_block, controller, original, definition)
    }

    pub(super) fn demonstrate_choice_step(
        &mut self,
        frame: Resolving,
        first: Option<CopyIdentity>,
        source_block: u32,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        if !self.is_live(player) || (first.is_some() && self.opponents(player).is_empty()) {
            self.state.rules.pending = None;
            return self.finish_instruction(
                frame,
                match first {
                    Some(first) => InstructionOutcome::Demonstrated {
                        first,
                        opponent: None,
                    },
                    None => InstructionOutcome::PlayerActionSkipped { player },
                },
            );
        }
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing demonstrate choice")?;
        self.validate_actions_response(input.player, &input.response)
            .ok_or("missing demonstrate chooser")??;
        match (first, input.response) {
            (None, Response::OptionalEffect { perform: false }) => {
                self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
            }
            (None, Response::OptionalEffect { perform: true }) => {
                let first = self.copy_demonstrated_spell(&frame, source_block, player)?;
                self.state.rules.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Demonstrate {
                        source_block,
                        first: Some(first),
                    },
                };
                Ok(())
            }
            (Some(first), Response::ChooseOpponent { opponent }) => {
                let opponent = self.copy_demonstrated_spell(&frame, source_block, opponent)?;
                self.finish_instruction(
                    frame,
                    InstructionOutcome::Demonstrated {
                        first,
                        opponent: Some(opponent),
                    },
                )
            }
            _ => Err("demonstrate choice stage".into()),
        }
    }

    // Only these exact instructions may leave spell copies above their still
    // resolving owner. Concessions can remove a copy between rule steps.
    pub(super) fn resolution_copy_children(
        &self,
        frame: &Resolving,
    ) -> Result<Vec<StackId>, String> {
        if let Some(copy) = frame.copied_child() {
            if !matches!(
                self.program_for(&frame.spell)?.instructions.as_slice(),
                [Instruction::CopyControlledCreatureSpell { .. }]
            ) || self.invocation_lives(&frame.spell)
                && !self.state.rules.stack.last().is_some_and(|top| {
                    top.id == copy
                        && self.is_spell_copy(top)
                        && self.state.rules.tokens[&top.source.card].source == frame.spell.source
                })
            {
                return Err("snapshot resolving spell copy authority".into());
            }
            return Ok(vec![copy]);
        }
        let (first, opponent) = match (frame.outcomes.as_slice(), &self.state.rules.frame) {
            ([InstructionOutcome::Demonstrated { first, opponent }], _) => {
                (first, opponent.as_ref())
            }
            (
                [],
                Frame::ResolutionChoice {
                    frame: owner,
                    choice:
                        ResolutionChoice::Demonstrate {
                            first: Some(first), ..
                        },
                },
            ) if owner.stack_id == frame.stack_id => (first, None),
            _ => return Ok(vec![]),
        };
        if !matches!(
            self.program_for(&frame.spell)?.instructions.as_slice(),
            [Instruction::Demonstrate { .. }]
        ) {
            return Err("demonstrate resolving instruction binding".into());
        }
        let cast = self.demonstrate_cast(frame)?;
        let mut children = vec![];
        let mut previous = frame.stack_id;
        for (index, child) in std::iter::once(first).chain(opponent).enumerate() {
            let owner = self
                .state
                .rules
                .cards
                .get(&child.card)
                .ok_or("missing demonstrated copy card")?
                .owner;
            let identity = self
                .state
                .rules
                .tokens
                .get(&child.card)
                .ok_or("missing demonstrated copy identity")?;
            if previous >= child.stack_id
                || (owner == cast.caster) != (index == 0)
                || identity.source != cast.spell.object
                || !matches!(identity.spec, TokenTemplate::SpellCopy { original, stack_id } if original == cast.spell.object && stack_id == child.stack_id)
            {
                return Err("demonstrate copy identity/owner binding".into());
            }
            if self.is_live(owner) {
                if !self.state.rules.stack.iter().any(|entry| {
                    entry.id == child.stack_id
                        && entry.source.card == child.card
                        && entry.controller == owner
                        && self.is_spell_copy(entry)
                }) {
                    return Err("missing live demonstrated copy".into());
                }
                children.push(child.stack_id);
            }
            previous = child.stack_id;
        }
        Ok(children)
    }
}

impl Resolving {
    pub fn copied_child(&self) -> Option<StackId> {
        match self.outcomes.as_slice() {
            [InstructionOutcome::SpellCopied { copy }] => Some(*copy),
            _ => None,
        }
    }
}

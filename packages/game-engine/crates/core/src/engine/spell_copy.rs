//! CR 111.13, 601.2, 704.5d/e, 707.9b/f, 707.10/10f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use tokens::{TokenIdentity, TokenTemplate, copied_creature_definition};

impl Game {
    pub(super) fn is_spell_copy(&self, entry: &StackInstance) -> bool {
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
        let Some(mut copy) = self
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
        self.check_stack_capacity(1)?;
        self.preflight_stack_ids(1)?;
        self.check_token_capacity(1)?;
        let card = self.next_token_card()?;
        let stack_id = StackId(self.state.rules.next_stack_id);
        let identity = TokenIdentity {
            source: frame.spell.source,
            spec: TokenTemplate::SpellCopy {
                original: copy.source,
                stack_id,
            },
            definition: copied_creature_definition(self.definition(copy.source.card)?),
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
            cause: Self::wipe_cause(&frame, source_block),
            target,
            copy: stack_id,
            object,
        })?;
        self.finish_instruction(frame, InstructionOutcome::SpellCopied { copy: stack_id })
    }
}

impl Resolving {
    pub(super) fn copied_child(&self) -> Option<StackId> {
        match self.outcomes.as_slice() {
            [InstructionOutcome::SpellCopied { copy }] => Some(*copy),
            _ => None,
        }
    }
}

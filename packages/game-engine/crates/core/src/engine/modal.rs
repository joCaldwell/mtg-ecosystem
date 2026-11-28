//! CR 601.2b–c, 602.2b, 603.3c, 700.2a–d: a stack occurrence owns its
//! selected modes before targets or costs. Decisions contain only
//! selections with legal targets. A spell or activated ability chooses in
//! `Frame::ChooseModes`; a triggered ability chooses while it is placed, in
//! its `Frame::TriggerTargets`, and with no offerable selection it is
//! removed from the stack (CR 603.3c, 700.2b).
//! Distinct selections use bitmasks as internal program keys. A declaration
//! that chooses three modes and permits repeats uses keys from the sorted
//! lists of printed mode indices. Each occurrence has its own targets and
//! antecedent scope, and resolves in printed order (CR 700.2d, 608.2c).
//! The public stack exposes the full index list and its union mask.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::ModalSpell;

impl RulesTxn {
    pub(super) fn modal_has_selection(
        &self,
        modal: &ModalSpell,
        player: PlayerId,
        source: &Characteristics,
        source_object: Option<ObjectId>,
    ) -> Result<bool, String> {
        for program in modal.selections.values() {
            if self.has_target_domain(program, player, source, source_object)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The printed mode declaration of this stack occurrence's spell or
    /// ability, if it is modal.
    pub(super) fn modal_declaration(
        &self,
        entry: &StackInstance,
    ) -> Result<Option<&ModalSpell>, String> {
        // CR 113.7a: an ability keeps the face it came from.
        let definition = self.stack_definition(entry)?;
        Ok(match &entry.kind {
            StackKind::Spell => definition.modal_spell.as_ref(),
            StackKind::Activated { ability, .. } => definition
                .activated
                .get(*ability as usize)
                .ok_or("missing activated ability")?
                .modal
                .as_ref(),
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Intrinsic { index },
                ..
            } => definition
                .triggered
                .get(*index as usize)
                .ok_or("missing triggered ability")?
                .modal
                .as_ref(),
            // A follow-up trigger's program comes from its creator and is
            // never modal.
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Followup { .. },
                ..
            } => None,
        })
    }

    /// A modal occurrence whose modes are not yet chosen.
    pub(super) fn unbound_modes(&self, entry: &StackInstance) -> Result<bool, String> {
        Ok(entry.mode_mask.is_none() && self.modal_declaration(entry)?.is_some())
    }

    pub(super) fn mode_decision(&self) -> Result<Option<Decision<Rules>>, String> {
        let entry = self.top()?;
        let definition = self.stack_definition(entry)?;
        let modal = self
            .modal_declaration(entry)?
            .ok_or("missing modal declaration")?;
        let source = self.targeting_characteristics(entry)?;
        let choices = ModalSpell::choices(modal.labels.len(), modal.min, modal.max, modal.repeat);
        let mut legal = BTreeMap::new();
        for (mask, program) in &modal.selections {
            legal.insert(
                *mask,
                self.has_target_domain(program, entry.controller, &source, Some(entry.source))?,
            );
        }
        let candidates: Vec<_> = modal
            .selections
            .iter()
            .filter(|(mask, _)| legal[*mask])
            .map(|(&mask, _)| Candidate::Modes {
                mask,
                chosen_modes: choices[&mask].iter().map(|&index| index as u8).collect(),
                labels: if mask == 0 {
                    vec!["No modes".into()]
                } else {
                    choices[&mask]
                        .iter()
                        .map(|&index| modal.labels[index].replace('~', &definition.name))
                        .collect()
                },
            })
            .collect();
        // Proposal and placement admit a modal occurrence only with an
        // offerable selection, so an empty decision is an invariant fault.
        if candidates.is_empty() {
            return Err("no offerable mode selection at the mode decision".into());
        }
        Ok(Some(self.pick(
            entry.controller,
            Reason::ChooseModes,
            candidates,
            1,
            1,
        )))
    }

    pub(super) fn announced_mode_indices(
        &self,
        entry: &StackInstance,
    ) -> Result<Option<Vec<u8>>, String> {
        let Some(key) = entry.mode_mask else {
            return Ok(None);
        };
        let modal = self
            .modal_declaration(entry)?
            .ok_or("missing modal declaration")?;
        let mut choices =
            ModalSpell::choices(modal.labels.len(), modal.min, modal.max, modal.repeat);
        let indices = choices
            .remove(&key)
            .ok_or("missing announced mode selection")?;
        Ok(Some(indices.into_iter().map(|index| index as u8).collect()))
    }

    /// Binds the answered selection to the top occurrence and returns the
    /// answer's decision ID.
    fn bind_mode_answer(&mut self, stack_id: StackId) -> Result<u64, String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing mode answer")?;
        let Candidate::Modes { mask, .. } = input.one()? else {
            return Err("expected a mode selection".into());
        };
        let mask = *mask;
        let entry = self.top()?;
        if entry.id != stack_id
            || !self
                .modal_declaration(entry)?
                .is_some_and(|modal| modal.selections.contains_key(&mask))
        {
            return Err("modal announcement binding invariant".into());
        }
        let entry = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing modal occurrence")?;
        if entry.mode_mask.replace(mask).is_some() {
            return Err("modal announcement binding invariant".into());
        }
        Ok(input.decision_id)
    }

    pub(super) fn bind_modes(&mut self, cast: Casting) -> Result<(), String> {
        let decision_id = self.bind_mode_answer(cast.stack_id)?;
        if cast.origin.is_spell() {
            self.rules_mut().frame = self.spell_announcement_frame(cast)?;
            return Ok(());
        }
        // CR 602.2b, 601.2c: the selection's target, then its costs.
        let domain = self.program()?.targets.first().map(|t| t.domain);
        match domain {
            None => self.continue_activation_after_targets(cast, decision_id),
            Some(domain) => {
                self.rules_mut().frame = Self::activation_target_frame(domain, cast);
                Ok(())
            }
        }
    }

    /// CR 603.3c: the trigger keeps its placement frame; a selection with a
    /// target asks for it next. With no chosen mode, remove the trigger (CR 700.2b).
    pub(super) fn bind_trigger_modes(
        &mut self,
        placement: TriggerPlacement,
        stack_id: StackId,
    ) -> Result<(), String> {
        self.bind_mode_answer(stack_id)?;
        if self.top()?.mode_mask == Some(0) {
            return self.reject_trigger_placement(
                placement,
                stack_id,
                TriggerPlacementFailure::NoModesChosen,
            );
        }
        if !self.program()?.targets.is_empty() {
            self.rules_mut().frame = Frame::TriggerTargets {
                placement,
                stack_id,
            };
            return Ok(());
        }
        let top = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing modal trigger")?;
        let StackKind::Triggered { trigger_id, .. } = top.kind else {
            return Err("modal trigger kind invariant".into());
        };
        top.paid = Some(top.total_cost.clone());
        let player = top.controller;
        self.record_event(Event::TriggerPlaced {
            trigger_id,
            stack_id,
            player,
        })?;
        self.rules_mut().frame = Frame::TriggerOrder { placement };
        Ok(())
    }

    /// Saved state: a mask exists only for a modal occurrence, names one of
    /// its admitted selections, and is absent exactly while its modes are
    /// being chosen (or the game stopped during that announcement).
    pub(super) fn validate_modes(&self, entry: &StackInstance) -> Result<(), String> {
        if matches!(entry.kind, StackKind::Triggered { .. }) && entry.mode_mask == Some(0) {
            return Err("zero-mode trigger remained on the stack".into());
        }
        let modal = self.modal_declaration(entry)?;
        let frame = self.underlying_mana_frame(&self.state.rules.frame)?;
        let top = self
            .state
            .rules
            .stack
            .last()
            .is_some_and(|t| t.id == entry.id);
        let choosing = top
            && match frame {
                Frame::ChooseModes { cast } => cast.stack_id == entry.id,
                Frame::TriggerTargets { stack_id, .. } => *stack_id == entry.id,
                _ => false,
            };
        let stopped = matches!(frame, Frame::Finished { .. } | Frame::Fault { .. })
            && entry.paid.is_none()
            && entry.cost_lock.is_none();
        let announcing = matches!(frame, Frame::ChooseModes { cast } if cast.stack_id == entry.id);
        // A placed trigger's bound selection is still in its placement frame
        // only while that selection's target is chosen.
        let placing =
            matches!(frame, Frame::TriggerTargets { stack_id, .. } if *stack_id == entry.id);
        if placing
            && let (Some(modal), Some(mask)) = (modal, entry.mode_mask)
            && modal
                .selections
                .get(&mask)
                .is_none_or(|program| program.targets.is_empty())
        {
            return Err("modal trigger placement binding invariant".into());
        }
        match (modal, entry.mode_mask) {
            (None, None) if !announcing => Ok(()),
            (Some(_), None) if choosing || stopped => Ok(()),
            (Some(modal), Some(mask)) if modal.selections.contains_key(&mask) && !announcing => {
                Ok(())
            }
            _ => Err("modal announcement binding invariant".into()),
        }
    }
}

//! CR 601.2b–c, 700.2a–d: a stack occurrence owns its selected modes before
//! targets or costs. Decisions contain only selections with legal targets.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::ModalSpell;

impl RulesTxn {
    pub(super) fn modal_has_selection(
        &self,
        modal: &ModalSpell,
        player: PlayerId,
        source: &Characteristics,
    ) -> bool {
        modal
            .selections
            .values()
            .any(|program| self.has_target_domain(program, player, source))
    }

    pub(super) fn mode_decision(&self) -> Result<Option<Decision<Rules>>, String> {
        let entry = self.top()?;
        let definition = self.stack_definition(entry)?;
        let modal = definition
            .modal_spell
            .as_ref()
            .ok_or("missing modal declaration")?;
        let source = self.targeting_characteristics(entry)?;
        let candidates: Vec<_> = modal
            .selections
            .iter()
            .filter(|(_, program)| self.has_target_domain(program, entry.controller, &source))
            .map(|(&mask, _)| Candidate::Modes {
                mask,
                labels: modal
                    .labels
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, label)| label.replace('~', &definition.name))
                    .collect(),
            })
            .collect();
        Ok((!candidates.is_empty())
            .then(|| self.pick(entry.controller, Reason::ChooseModes, candidates, 1, 1)))
    }

    pub(super) fn bind_modes(&mut self, cast: Casting) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing mode answer")?;
        let Candidate::Modes { mask, .. } = input.one()? else {
            return Err("expected a mode selection".into());
        };
        let entry = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing modal spell")?;
        if entry.id != cast.stack_id || entry.mode_mask.replace(*mask).is_some() {
            return Err("modal announcement binding invariant".into());
        }
        self.rules_mut().frame = self.spell_announcement_frame(cast)?;
        Ok(())
    }

    pub(super) fn validate_modes(&self, entry: &StackInstance) -> Result<(), String> {
        let definition = self.stack_definition(entry)?;
        let modal = definition
            .modal_spell
            .as_ref()
            .filter(|_| entry.kind.is_spell());
        let frame = self.underlying_mana_frame(&self.state.rules.frame)?;
        let choosing = matches!(frame, Frame::ChooseModes { cast } if cast.stack_id == entry.id);
        let stopped = matches!(frame, Frame::Finished { .. } | Frame::Fault { .. })
            && entry.paid.is_none()
            && entry.cost_lock.is_none();
        match (modal, entry.mode_mask) {
            (None, None) if !choosing => Ok(()),
            (Some(_), None) if choosing || stopped => Ok(()),
            (Some(modal), Some(mask)) if !choosing && modal.selections.contains_key(&mask) => {
                Ok(())
            }
            _ => Err("modal announcement binding invariant".into()),
        }
    }
}

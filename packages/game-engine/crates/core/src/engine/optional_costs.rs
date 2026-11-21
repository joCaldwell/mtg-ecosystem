//! CR 601.2b/f and 702.33d/e: announcement declarations persist on this exact
//! invocation. A spell copy retains those decisions under CR 707.10.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(super) fn validate_additional_cost_selection(
        &self,
        player: PlayerId,
        selected: &[String],
    ) -> Result<(), String> {
        let top = self.top()?;
        let declarations = &self.program()?.optional_additional_costs;
        if top.controller != player
            || !top.kind.is_spell()
            || declarations.is_empty()
            || top.selected_additional_costs.is_some()
            || selected.len() > declarations.len()
            || selected.iter().enumerate().any(|(i, identity)| {
                selected[..i].contains(identity)
                    || !declarations.iter().any(|c| c.identity == *identity)
            })
        {
            return Err("foreign, duplicate or stale optional cost selection".into());
        }
        Ok(())
    }
    pub(super) fn bind_additional_costs(&mut self, cast: Casting) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing optional cost input")?;
        let Response::AdditionalCosts { selected } = input.response else {
            return Err("optional cost response invariant".into());
        };
        self.validate_additional_cost_selection(input.player, &selected)?;
        self.rules_mut()
            .stack
            .last_mut()
            .ok_or("missing optional cost invocation")?
            .selected_additional_costs = Some(selected);
        self.rules_mut().frame = self.spell_announcement_frame(cast)?;
        Ok(())
    }
    pub(super) fn public_additional_cost_choices(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<visibility::AdditionalCostChoice>, String> {
        let top = self.top()?;
        if top.controller != viewer
            || !matches!(self.state.rules.frame, Frame::AdditionalCosts { .. })
        {
            return Err("optional cost choices require current caster authority".into());
        }
        let handle = self.stack_handle_for(viewer, top.id)?;
        Ok(self
            .program()?
            .optional_additional_costs
            .iter()
            .map(|c| visibility::AdditionalCostChoice {
                id: format!("{}/cost/{}", handle.0, c.identity),
                mana: c.mana.clone(),
            })
            .collect())
    }
    pub(super) fn has_selected_additional_mana(&self, entry: &StackInstance) -> bool {
        // The supported declarations are typed mana costs. A selected {0} is
        // still a mana payment (CR 118.5/601.2g), unlike no mana payment.
        entry
            .selected_additional_costs
            .as_ref()
            .is_some_and(|s| !s.is_empty())
    }
    pub(super) fn validate_additional_cost_entry(
        &self,
        entry: &StackInstance,
    ) -> Result<(), String> {
        let program = self.program_for(entry)?;
        if program.optional_additional_costs.is_empty() {
            return if entry.selected_additional_costs.is_none() {
                Ok(())
            } else {
                Err("optional cost selection without source declaration".into())
            };
        }
        let Some(selected) = &entry.selected_additional_costs else {
            return if entry.paid.is_none()
                && !self.is_spell_copy(entry)
                && (matches!(&self.state.rules.frame, Frame::AdditionalCosts {cast} if cast.stack_id == entry.id)
                    || matches!(
                        self.state.rules.frame,
                        Frame::Fault { .. } | Frame::Finished { .. }
                    ))
            {
                Ok(())
            } else {
                Err("missing optional cost announcement identity".into())
            };
        };
        if !entry.kind.is_spell()
            || selected.len() > program.optional_additional_costs.len()
            || selected.iter().enumerate().any(|(i, id)| {
                selected[..i].contains(id)
                    || !program
                        .optional_additional_costs
                        .iter()
                        .any(|c| c.identity == *id)
            })
        {
            return Err("optional cost declaration/selection/locked total invariant".into());
        }
        Ok(())
    }
    pub(super) fn additional_cost_was_selected(
        &self,
        entry: &StackInstance,
        identity: &str,
    ) -> Result<bool, String> {
        if !entry.kind.is_spell() || !self.stack_entry_complete(entry) {
            return Err("conditional draw requires a completed spell".into());
        }
        let selected = entry
            .selected_additional_costs
            .as_ref()
            .ok_or("missing optional cost declaration")?;
        if !self
            .program_for(entry)?
            .optional_additional_costs
            .iter()
            .any(|c| c.identity == identity)
        {
            return Err("conditional draw linked to foreign cost".into());
        }
        Ok(selected.iter().any(|id| id == identity))
    }
}

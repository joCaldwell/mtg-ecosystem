//! CR 601.2b/f and 702.33d/e: announcement declarations persist on this exact
//! invocation. A spell copy retains those decisions under CR 707.10.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl RulesTxn {
    /// CR 118.9, 601.2b: the printed alternative cost an original cast may
    /// announce. Only casts from hand or the command zone offer it; a
    /// graveyard cast already pays its flashback cost (CR 118.9a). A copy
    /// retains the original's announcement (CR 707.10).
    pub(super) fn offered_alternative_cost(
        &self,
        entry: &StackInstance,
    ) -> Result<Option<&OptionalAdditionalManaCost>, String> {
        if !entry.kind.is_spell()
            || !(self.is_spell_copy(entry)
                || matches!(entry.cast_from, Zone::Hand { .. } | Zone::Command { .. }))
        {
            return Ok(None);
        }
        Ok(self.stack_definition(entry)?.alternative_cost.as_ref())
    }
    /// CR 601.2b: every optional cost this invocation announces at once:
    /// its additional costs, then any offered alternative cost.
    pub(super) fn optional_cost_declarations<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Result<Vec<&'a OptionalAdditionalManaCost>, String> {
        let mut declarations = self
            .program_for(entry)?
            .optional_additional_costs
            .iter()
            .collect::<Vec<_>>();
        declarations.extend(self.offered_alternative_cost(entry)?);
        Ok(declarations)
    }
    pub(super) fn validate_additional_cost_selection(
        &self,
        player: PlayerId,
        selected: &[String],
    ) -> Result<(), String> {
        let top = self.top()?;
        let declarations = self.optional_cost_declarations(top)?;
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
        let selected = input.additional_costs()?;
        self.validate_additional_cost_selection(input.player, &selected)?;
        self.rules_mut()
            .stack
            .last_mut()
            .ok_or("missing optional cost invocation")?
            .selected_additional_costs = Some(selected);
        self.rules_mut().frame = self.spell_announcement_frame(cast)?;
        Ok(())
    }

    pub(super) fn validate_additional_cost_entry(
        &self,
        entry: &StackInstance,
    ) -> Result<(), String> {
        let declarations = self.optional_cost_declarations(entry)?;
        if declarations.is_empty() {
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
            || selected.len() > declarations.len()
            || selected.iter().enumerate().any(|(i, id)| {
                selected[..i].contains(id) || !declarations.iter().any(|c| c.identity == *id)
            })
        {
            return Err("optional cost declaration/selection/locked total invariant".into());
        }
        Ok(())
    }
}

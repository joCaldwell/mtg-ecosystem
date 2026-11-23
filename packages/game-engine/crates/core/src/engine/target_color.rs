//! CR 105.4, 608.2b/d, 611.2c and 613.1e/7 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(super) fn target_color_target(&self, frame: &Resolving) -> Option<ObjectId> {
        let entry = self.entry(frame.stack_id).ok()?;
        let [target] = entry.targets.as_slice() else {
            return None;
        };
        (self.is_live(entry.controller)
            && frame.legal_targets.as_slice() == [true]
            && self
                .current(*target)
                .is_ok_and(|object| object.present_on_battlefield()))
        .then_some(*target)
    }

    pub(super) fn target_color_request(&self, frame: &Resolving) -> Option<Request> {
        Some(Request::ChooseColor {
            decision_id: self.state.rules.revision,
            player: self.entry(frame.stack_id).ok()?.controller,
            target: self.target_color_target(frame)?,
        })
    }

    pub(super) fn validate_target_color_response(
        &self,
        frame: &Resolving,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if self.target_color_target(frame).is_none()
            || player != self.entry(frame.stack_id)?.controller
            || !matches!(response, Response::ChooseColor { color } if *color != Color::C)
        {
            return Err("invalid color choice".into());
        }
        Ok(())
    }

    pub(super) fn target_color_pending(&self, frame: &Resolving) -> Result<Color, String> {
        let input = self
            .state
            .rules
            .pending
            .as_ref()
            .ok_or("missing color choice")?;
        self.validate_target_color_response(frame, input.player, &input.response)?;
        let Response::ChooseColor { color } = input.response else {
            return Err("missing color response".into());
        };
        Ok(color)
    }

    pub(super) fn latest_color_set(&self, target: ObjectId) -> Option<(u64, Color)> {
        self.state
            .rules
            .effects
            .iter()
            .filter_map(|effect| match effect.kind {
                _ => None,
            })
            .max_by_key(|(timestamp, _)| *timestamp)
    }
}

//! CR 704.3, 704.5j / 101.4: controller choices precede one simultaneous SBA
//! wave, in APNAP order. A legend put into a graveyard is not destroyed.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn legend_groups(&self) -> Result<Vec<Vec<ObjectId>>, String> {
        let mut by_name: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
        for object in self
            .state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
        {
            let definition = self.definition(object.id.card)?;
            if definition.characteristics.legendary {
                by_name
                    .entry((object.controller, definition.name.clone()))
                    .or_default()
                    .push(object.id);
            }
        }
        let mut groups = by_name
            .into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .collect::<Vec<_>>();
        groups
            .sort_by_key(|((p, name), _)| ((p.0 + 4 - self.state.turn_owner.0) % 4, name.clone()));
        Ok(groups.into_iter().map(|(_, ids)| ids).collect())
    }
    pub(super) fn legend_request(&self) -> Result<Option<Request>, String> {
        let Frame::LegendRule { groups, kept, .. } = &self.state.frame else {
            return Ok(None);
        };
        let Some(candidates) = groups.get(kept.len()) else {
            return Ok(None);
        };
        Ok(Some(Request::LegendRule {
            decision_id: self.state.revision,
            player: self.current(candidates[0])?.controller,
            candidates: candidates.clone(),
            previous_choices: kept.clone(),
        }))
    }
    pub(super) fn validate_legend_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        match (self.legend_request()?, response) {
            (
                Some(Request::LegendRule {
                    player: chooser,
                    candidates,
                    ..
                }),
                Response::LegendRule { keep },
            ) if player == chooser && candidates.contains(keep) => Ok(()),
            _ => Err("invalid legend-rule keep choice".into()),
        }
    }
    pub(super) fn legend_step(&mut self) -> Result<(), String> {
        let Frame::LegendRule {
            resume,
            groups,
            mut kept,
        } = self.state.frame.clone()
        else {
            return Err("legend frame".into());
        };
        if kept.len() < groups.len() {
            let input = self.state.pending.take().ok_or("missing legend choice")?;
            self.validate_legend_response(input.player, &input.response)?;
            let Response::LegendRule { keep } = input.response else {
                return Err("legend response".into());
            };
            kept.push(keep);
            self.state.frame = Frame::LegendRule {
                resume,
                groups,
                kept,
            };
            return Ok(());
        }
        let Frame::Stabilize {
            resume,
            cleanup,
            changed,
        } = *resume
        else {
            return Err("legend resume".into());
        };
        self.stabilization_wave_with_legends(*resume, cleanup, changed, Some(kept))
    }
    pub(super) fn validate_legend_state(&self) -> Result<(), String> {
        if let Frame::LegendRule {
            resume,
            groups,
            kept,
        } = &self.state.frame
            && (!matches!(resume.as_ref(), Frame::Stabilize { .. })
                || *groups != self.legend_groups()?
                || groups.is_empty()
                || kept.len() > groups.len()
                || kept
                    .iter()
                    .zip(groups)
                    .any(|(keep, group)| !group.contains(keep)))
        {
            return Err("snapshot legend-rule choice invariant".into());
        }
        Ok(())
    }
}

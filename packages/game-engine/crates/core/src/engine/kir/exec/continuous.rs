//! CR 611.2c, 613.1b, 613.7b, 608.2b-c: resolve fixed recipients and
//! commit one effect for the complete captured set.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;
use mtg_contracts::kir::Change;

impl RulesTxn {
    pub(in crate::engine) fn continuous_kind(
        &self,
        frame: &Resolving,
        verb: &Verb,
    ) -> Result<Option<ResolvedEffectKind>, String> {
        let (Verb::GainControl { what, .. }
        | Verb::SetCharacteristics { what, .. }
        | Verb::ModifyUntil {
            what,
            change: Change::PowerToughnessX { .. },
            ..
        }) = verb
        else {
            return Err("unexpected continuous effect".into());
        };
        let entry = self.entry(frame.stack_id)?;
        let mut targets = match what {
            SetExpr::One {
                object: Ref::Source,
            } => vec![entry.source],
            SetExpr::One {
                object: Ref::Target { slot },
            } => {
                if frame.legal_targets.get(*slot as usize) != Some(&true) {
                    return Ok(None);
                }
                vec![
                    *self
                        .bound_object_target(entry, *slot as usize)
                        .ok_or("missing continuous effect target")?,
                ]
            }
            SetExpr::Matching { zone, filter } => self.kernel_set(frame, zone, filter)?.0,
            _ => return Err("unexpected continuous effect operand".into()),
        };
        targets.retain(|&object| {
            self.current(object)
                .is_ok_and(|o| o.present_on_battlefield())
        });
        if targets.is_empty() {
            return Ok(None);
        }
        let kind = match verb {
            Verb::GainControl {
                controller, until, ..
            } => {
                let Players::One(controller) = self.kernel_players(frame, controller)? else {
                    return Ok(None);
                };
                if !self.is_live(controller)
                    || (*until == mtg_contracts::kir::Until::SourceLeaves
                        && !self.control_source_remains(entry.source))
                {
                    return Ok(None);
                }
                ResolvedEffectKind::GainControl {
                    targets,
                    controller,
                    until: *until,
                }
            }
            Verb::SetCharacteristics { values, .. } => ResolvedEffectKind::SetCharacteristics {
                targets,
                values: values.clone(),
            },
            Verb::ModifyUntil {
                change: Change::PowerToughnessX { power, toughness },
                ..
            } => {
                if !self.target_is_creature(targets[0]) {
                    return Ok(None);
                }
                // CR 608.2h, 611.2c: a counted change reads its count once,
                // now; the saved effect keeps that value.
                let x = match verb {
                    Verb::ModifyUntil { change, .. } if change.counted_x().is_some() => {
                        let x = self.kernel_amount(frame, change.counted_x().expect("guarded"))?;
                        if x.is_negative() {
                            return Err("negative counted pump".into());
                        }
                        x
                    }
                    _ => entry.x.clone().ok_or("missing announced pump X")?,
                };
                ResolvedEffectKind::PumpAnnouncedX {
                    targets,
                    modifier: CreatureStatModifier {
                        power: power.with_x(&x),
                        toughness: toughness.with_x(&x),
                    },
                    x,
                }
            }
            _ => unreachable!("checked continuous effect verb"),
        };
        Ok(Some(kind))
    }

    pub(in crate::engine) fn kernel_continuous(
        &mut self,
        frame: Resolving,
        verb: &Verb,
        source_block: u32,
    ) -> Result<(), String> {
        let Some(kind) = self.continuous_kind(&frame, verb)? else {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        };
        let (ResolvedEffectKind::PumpAnnouncedX { targets, .. }
        | ResolvedEffectKind::SetCharacteristics { targets, .. }
        | ResolvedEffectKind::GainControl { targets, .. }) = &kind
        else {
            unreachable!("checked continuous effect kind");
        };
        let capacity = self.event_group_limit();
        if targets.len() > capacity {
            return Err(self.capacity_error(RuleFault::EventGroupCapacityExceeded {
                capacity,
                requested: targets.len(),
            }));
        }
        self.create_set_effect(frame, kind, source_block)
    }
}

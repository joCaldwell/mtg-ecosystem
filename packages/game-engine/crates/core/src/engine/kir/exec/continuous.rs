//! CR 611.2c, 613.1b, 613.7b, 608.2b-c: resolve fixed recipients and
//! commit one effect for the complete captured set.
//!
//! The shared executor captures legal announced objects, the exact ability
//! source, an event-bound creature or a matching battlefield set. An
//! incarnation that arrives later cannot join the capture (CR 400.7, 611.2c).
//!
//! Layer 4 adds creature or artifact, with fixed creature subtypes or all
//! creature types. Layer 5 sets colors. Layer 6 adds admitted keywords.
//! Layer 7b sets fixed base stats or reads one shared value as the verb
//! begins. Layer 7c adds signed fixed, announced or counted deltas
//! (CR 107.3, 608.2h, 611.2d, 613.4). Counters survive expiry (CR 122.2).
//! Characteristic setters can expire at cleanup, at the beginning of the
//! controller's next turn (including a skipped seat, CR 800.4m), or when
//! their exact source leaves or phases out (CR 611.2b, 702.26f).
//! Signed pumps expire at cleanup. The compiler rejects copy effects,
//! resolving ability removal, arbitrary duration predicates, chosen
//! characteristics and quoted grants without an admitted recipient and
//! runtime ability representation.
//!
//! CR 305.2 and 613.11: the checked additional-land-plays verb adds a
//! fixed allowance for its controller this turn. Normal main-phase
//! timing, actual land-play counts and cleanup continue to govern play.
//!
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
        | Verb::ModifyUntil { what, .. }) = verb
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
            SetExpr::One {
                object: Ref::EventObject { .. },
            } => vec![self.event_object_target(frame)?],
            SetExpr::Matching { zone, filter } => self.kernel_set(frame, zone, filter)?.0,
            _ => return Err("unexpected continuous effect operand".into()),
        };
        targets.retain(|&object| {
            self.current(object)
                .is_ok_and(|o| o.present_on_battlefield())
        });
        if matches!(verb, Verb::ModifyUntil { .. }) && !matches!(what, SetExpr::Matching { .. }) {
            targets.retain(|&object| self.target_is_creature(object));
        }
        if targets.is_empty() {
            return Ok(None);
        }
        let is_set = matches!(what, SetExpr::Matching { .. });
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
            Verb::SetCharacteristics { values, until, .. } => {
                if *until == mtg_contracts::kir::Until::SourceLeaves
                    && !self.control_source_remains(entry.source)
                {
                    return Ok(None);
                }
                let mut values = values.clone();
                if let Some(amount) = values.base_from.take() {
                    let stat = self.kernel_amount(frame, &amount)?;
                    if stat.is_negative() {
                        return Err("negative shared base stat".into());
                    }
                    values.base_power = Some(stat.clone());
                    values.base_toughness = Some(stat);
                }
                ResolvedEffectKind::SetCharacteristics {
                    targets,
                    values,
                    until: *until,
                }
            }
            Verb::ModifyUntil {
                change: Change::PowerToughness { power, toughness },
                ..
            } => {
                let power = power.as_fixed().ok_or("nonfixed pump power")?.clone();
                let toughness = toughness
                    .as_fixed()
                    .ok_or("nonfixed pump toughness")?
                    .clone();
                if is_set {
                    ResolvedEffectKind::PumpSet {
                        targets,
                        power,
                        toughness,
                    }
                } else {
                    ResolvedEffectKind::ModifyPowerToughness {
                        target: targets[0],
                        power,
                        toughness,
                    }
                }
            }
            Verb::ModifyUntil {
                change: change @ Change::PowerToughnessX { power, toughness },
                ..
            } => {
                // CR 608.2h, 611.2c: read the count once, as the effect begins.
                let x = match change.counted_x() {
                    Some(amount) => self.kernel_amount(frame, amount)?,
                    None => entry.x.clone().ok_or("missing announced pump X")?,
                };
                if x.is_negative() {
                    return Err("negative pump count".into());
                }
                ResolvedEffectKind::PumpAnnouncedX {
                    targets,
                    modifier: CreatureStatModifier {
                        power: power.with_x(&x),
                        toughness: toughness.with_x(&x),
                    },
                    x,
                }
            }
            Verb::ModifyUntil {
                change: Change::AddKeywords { keywords },
                ..
            } => {
                if is_set {
                    ResolvedEffectKind::GrantKeywordsSet {
                        targets,
                        modifier: None,
                        keywords: keywords.clone(),
                    }
                } else {
                    ResolvedEffectKind::GrantKeywords {
                        target: targets[0],
                        modifier: None,
                        keywords: keywords.clone(),
                    }
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
        self.commit_resolved_effect(frame, kind, source_block)
    }
}

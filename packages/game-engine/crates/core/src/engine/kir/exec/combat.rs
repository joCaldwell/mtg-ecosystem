//! CR 608.2b, 611.2c, 514.2: a temporary combat restriction follows either
//! the legal target incarnation or the current members of a stated class.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;
use mtg_contracts::kir::{CombatProhibitionScope as Scope, CombatRequirement};

impl RulesTxn {
    pub(in crate::engine) fn combat_prohibition_kind(
        &self,
        frame: &Resolving,
        affects: &Scope<Ref>,
        restriction: CombatRequirement,
    ) -> Result<Option<ResolvedEffectKind>, String> {
        let affects = match affects {
            Scope::Target {
                target: Ref::Target { slot },
            } => {
                let slot = *slot as usize;
                if frame.legal_targets.get(slot) != Some(&true) {
                    return Ok(None);
                }
                let object = *self
                    .bound_object_target(self.entry(frame.stack_id)?, slot)
                    .ok_or("missing combat prohibition target")?;
                if !self.is_battlefield_creature(object)? {
                    return Ok(None);
                }
                Scope::Target { target: object }
            }
            // CR 611.2c: the ability's own source, while that incarnation
            // remains a battlefield creature.
            Scope::Target {
                target: Ref::Source,
            } => {
                let object = self.entry(frame.stack_id)?.source;
                if !self.is_battlefield_creature(object)? {
                    return Ok(None);
                }
                Scope::Target { target: object }
            }
            Scope::Creatures {} => Scope::Creatures {},
            Scope::CreaturesWithoutFlying {} => Scope::CreaturesWithoutFlying {},
            Scope::Target { .. } => return Err("unaudited combat prohibition target".into()),
        };
        Ok(Some(ResolvedEffectKind::CombatProhibition {
            affects,
            restriction,
        }))
    }

    pub(in crate::engine) fn kernel_restrict_combat(
        &mut self,
        frame: Resolving,
        affects: &Scope<Ref>,
        restriction: CombatRequirement,
        source_block: u32,
    ) -> Result<(), String> {
        match self.combat_prohibition_kind(&frame, affects, restriction)? {
            Some(kind) => self.commit_resolved_effect(frame, kind, source_block),
            None => self.finish_instruction(frame, InstructionOutcome::Effect { actual: None }),
        }
    }
}

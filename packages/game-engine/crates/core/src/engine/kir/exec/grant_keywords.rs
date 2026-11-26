//! CR 608.2b, 611.2c, 613.7b: capture the legal battlefield incarnations
//! when the grant resolves. The effect ID orders resolved layer 6 grants.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;

impl RulesTxn {
    pub(in crate::engine) fn keyword_grant_kind(
        &self,
        frame: &Resolving,
        what: &SetExpr,
        keywords: &[KeywordAbility],
    ) -> Result<Option<ResolvedEffectKind>, String> {
        let target = match what {
            SetExpr::One {
                object: Ref::Source,
            } => self.entry(frame.stack_id)?.source,
            SetExpr::One {
                object: Ref::Target { slot },
            } => {
                if frame.legal_targets.get(*slot as usize) != Some(&true) {
                    return Ok(None);
                }
                *self
                    .bound_object_target(self.entry(frame.stack_id)?, *slot as usize)
                    .ok_or("missing keyword grant target")?
            }
            SetExpr::Matching { zone, filter } => {
                let (targets, _) = self.kernel_set(frame, zone, filter)?;
                return Ok(
                    (!targets.is_empty()).then(|| ResolvedEffectKind::GrantKeywordsSet {
                        targets,
                        modifier: None,
                        keywords: keywords.to_vec(),
                        timestamp: self.state.rules.next_batch_id,
                    }),
                );
            }
            _ => return Err("keyword grant has no audited recipient".into()),
        };
        Ok(self
            .is_battlefield_creature(target)
            .then(|| ResolvedEffectKind::GrantKeywords {
                target,
                modifier: None,
                keywords: keywords.to_vec(),
                timestamp: self.state.rules.next_batch_id,
            }))
    }

    pub(in crate::engine) fn kernel_grant_keywords(
        &mut self,
        frame: Resolving,
        what: &SetExpr,
        keywords: &[KeywordAbility],
        source_block: u32,
    ) -> Result<(), String> {
        let Some(kind) = self.keyword_grant_kind(&frame, what, keywords)? else {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        };
        if let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &kind {
            let capacity = self.event_group_limit();
            if targets.len() > capacity {
                return Err(self.capacity_error(RuleFault::EventGroupCapacityExceeded {
                    capacity,
                    requested: targets.len(),
                }));
            }
            self.create_set_effect(frame, kind, source_block)
        } else {
            self.commit_resolved_effect(frame, kind, source_block)
        }
    }
}

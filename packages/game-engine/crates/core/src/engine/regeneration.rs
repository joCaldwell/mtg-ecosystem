//! CR 701.8, 701.19, 616.1, 704.3/5f-h, 506.4. Only destruction is
//! replaceable. Determine applicability against the common prior, before
//! revealing a shuffle or applying any member of the wave.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Ref, SetExpr};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShieldMember {
    pub(super) object: ObjectId,
    pub(super) affected: PlayerId,
    pub(super) shield: EffectId,
    pub(super) regenerate: Option<bool>,
}

pub(super) fn creates_shield(event: &ProposedEvent) -> bool {
    matches!(event, ProposedEvent::CreateEffect { effect } if matches!(effect.kind, ResolvedEffectKind::RegenerationShield { .. }))
}

pub(super) fn destruction_object(e: &ProposedEvent) -> Option<ObjectId> {
    match e {
        ProposedEvent::DestroyPermanent { object }
        | ProposedEvent::DestroyCreature { object }
        | ProposedEvent::StateBasedGraveyard {
            object,
            reason: CreatureLossReason::LethalDamage | CreatureLossReason::Deathtouch,
        } => Some(*object),
        _ => None,
    }
}

impl RulesTxn {
    pub(super) fn regeneration_target(
        &self,
        frame: &Resolving,
        what: &SetExpr,
    ) -> Result<Option<ObjectId>, String> {
        let entry = self.entry(frame.stack_id)?;
        let target = match what {
            SetExpr::One {
                object: Ref::Source,
            } => entry.source,
            SetExpr::One {
                object: Ref::Target { slot },
            } => {
                if frame.legal_targets.get(*slot as usize) != Some(&true) {
                    return Ok(None);
                }
                *self
                    .bound_object_target(entry, *slot as usize)
                    .ok_or("missing regeneration target")?
            }
            _ => return Err("regeneration requires one permanent".into()),
        };
        Ok(self
            .current(target)
            .is_ok_and(|o| o.present_on_battlefield())
            .then_some(target))
    }

    pub(super) fn execute_regenerate(
        &mut self,
        frame: Resolving,
        what: &SetExpr,
        source_block: u32,
    ) -> Result<(), String> {
        let Some(target) = self.regeneration_target(&frame, what)? else {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        };
        self.commit_resolved_effect(
            frame,
            ResolvedEffectKind::RegenerationShield { target },
            source_block,
        )
    }

    pub(super) fn remove_regeneration_shields(&mut self, object: ObjectId) {
        self.rules_mut().effects.retain(|effect| {
            !matches!(effect.kind, ResolvedEffectKind::RegenerationShield { target } if target == object)
        });
    }

    pub(super) fn regeneration_members(
        &self,
        group: &EventGroup,
        finish: &GroupFinish,
    ) -> Result<Vec<ShieldMember>, String> {
        // Each admitted shield on an incarnation performs exactly the same
        // replacement and expires in the same cleanup. Use the oldest such
        // equivalent shield; other shields remain available.
        let mut shields = BTreeMap::new();
        for e in &self.state.rules.effects {
            if let ResolvedEffectKind::RegenerationShield { target } = e.kind {
                shields.entry(target).or_insert(e.id);
            }
        }
        if shields.is_empty() {
            return Ok(vec![]);
        }
        let mut members = vec![];
        for event in graveyard_group::GraveyardGroup::leaves(group)? {
            let Some(object) = destruction_object(event) else {
                continue;
            };
            let Some(shield) = shields.get(&object) else {
                continue;
            };
            let current = self.current(object)?;
            if !current.present_on_battlefield()
                || self.has_keyword(object, KeywordAbility::Indestructible)?
            {
                continue;
            }
            let forbidden = match finish {
                GroupFinish::Instruction { frame, .. } => {
                    let instruction = self
                        .program_for(self.entry(frame.stack_id)?)?
                        .instructions
                        .get(frame.cursor as usize)
                        .ok_or("missing destroy instruction")?;
                    match instruction {
                        kernel!(Destroy {
                            regeneration_forbidden,
                            ..
                        }) => *regeneration_forbidden,
                        i if i.target_destroy().is_some() => false,
                        _ => return Err("regeneration requires a destruction producer".into()),
                    }
                }
                GroupFinish::StateBased { .. } => false,
                _ => return Err("unsupported regeneration producer".into()),
            };
            let owner = self.state.rules.cards[&object.card].owner;
            let anger = self.anger_replaces_death(object, Zone::Graveyard { owner });
            members.push(ShieldMember {
                object,
                shield: *shield,
                affected: current.controller,
                regenerate: if forbidden {
                    Some(false)
                } else if anger {
                    None
                } else {
                    Some(true)
                },
            });
        }
        Ok(members)
    }

    pub(super) fn replace_destruction(
        &mut self,
        object: ObjectId,
        proof: &[ShieldMember],
        prior: Option<&PreEventFacts>,
    ) -> Result<Option<EventOutcome>, String> {
        let Some(shield) = self
            .state
            .rules
            .effects
            .iter()
            .find(|e| {
                matches!(e.kind, ResolvedEffectKind::RegenerationShield { target } if target == object)
            })
            .map(|e| e.id)
        else {
            return Ok(None);
        };
        let member = proof
            .iter()
            .find(|m| m.object == object && m.shield == shield)
            .ok_or("regeneration requires its prepared destruction wave")?;
        if !member
            .regenerate
            .ok_or("unresolved destruction replacement")?
        {
            return Ok(None);
        }
        if !prior.is_some_and(|facts| {
            facts.effects.iter().any(|e| {
                e.id == shield
                    && matches!(e.kind, ResolvedEffectKind::RegenerationShield { target } if target == object)
            })
        }) {
            return Err("missing common-prior regeneration shield".into());
        }
        let o = self
            .rules_mut()
            .objects
            .get_mut(&object.card)
            .ok_or("missing regeneration object")?;
        let tapped = !o.tapped;
        o.tapped = true;
        o.marked_damage = Quantity::zero();
        // CR 704.5h checks damage since the preceding SBA observation.
        // The SBA wave separately consumes its checked deathtouch windows.
        self.remove_from_combat(object);
        self.rules_mut().effects.retain(|e| e.id != shield);
        Ok(Some(EventOutcome::Regenerated {
            object,
            shield,
            tapped,
        }))
    }
}

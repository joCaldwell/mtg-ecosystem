//! CR 701.8, 701.19, 616.1, 704.3/5f-h, 506.4. Only destruction is
//! replaceable. Determine applicability against the common prior, before
//! revealing a shuffle or applying any member of the wave.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShieldMember {
    pub(super) object: ObjectId,
    pub(super) affected: PlayerId,
    pub(super) shield: EffectId,
    pub(super) regenerate: Option<bool>,
}

pub(super) fn creates_shield(event: &ProposedEvent) -> bool {
    matches!(event, ProposedEvent::CreateEffect { effect } if matches!(effect.kind, ResolvedEffectKind::RegenerationShield))
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

impl Game {
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
            if matches!(e.kind, ResolvedEffectKind::RegenerationShield) {
                shields.entry(e.origin.source).or_insert(e.id);
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
                || self.has_keyword(object, KeywordAbility::Indestructible)
            {
                continue;
            }
            let forbidden = match finish {
                GroupFinish::Instruction { frame, .. } => {
                    let instruction = self
                        .program_for(&frame.spell)?
                        .instructions
                        .get(frame.cursor as usize)
                        .ok_or("missing destroy instruction")?;
                    match instruction {
                        Instruction::DestroyPermanent {
                            regeneration_forbidden,
                            ..
                        }
                        | kernel!(Destroy {
                            regeneration_forbidden,
                            ..
                        }) => *regeneration_forbidden,
                        Instruction::DestroyTargetCreature { .. }
                        | Instruction::DestroyAllCreatures { .. } => false,
                        _ => return Err("regeneration requires a destruction producer".into()),
                    }
                }
                GroupFinish::StateBased { .. } => false,
                _ => return Err("unsupported regeneration producer".into()),
            };
            // The only shield producer is a self activation. No admitted
            // copiable definition combines it with the printed shuffle, and
            // copying on entry creates a new incarnation.
            if !forbidden && self.has_self_graveyard_replacement(object)? {
                return Err("unsupported printed shuffle with self regeneration".into());
            }
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
                matches!(e.kind, ResolvedEffectKind::RegenerationShield)
                    && e.origin.source == object
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
                    && e.origin.source == object
                    && matches!(e.kind, ResolvedEffectKind::RegenerationShield)
            })
        }) {
            return Err("missing common-prior regeneration shield".into());
        }
        let o = self
            .state
            .rules
            .objects
            .get_mut(&object.card)
            .ok_or("missing regeneration object")?;
        let tapped = !o.tapped;
        o.tapped = true;
        o.marked_damage = Quantity::zero();
        // CR 704.5h checks damage since the preceding SBA observation.
        // The SBA wave separately consumes its checked deathtouch windows.
        self.remove_from_combat(object);
        self.state.rules.effects.retain(|e| e.id != shield);
        Ok(Some(EventOutcome::Regenerated {
            object,
            shield,
            tapped,
        }))
    }
}
